//! Débogueur : une fenêtre à onglets (CPU, Mémoire, Vidéo, Palette, Sprites).
//!
//! Le cœur ne copie que ce que l'onglet visible demande (`Tab::level`). Les valeurs qui ont
//! changé depuis l'instantané précédent sont surlignées.

use eframe::egui::{self, text::LayoutJob, Color32, RichText, TextFormat};
use ng_core::{DebugLevel, DebugSnapshot};
use puce_m68k::state::CpuState;

use crate::video_debug::VideoDebug;

/// Couleur des valeurs modifiées depuis l'instantané précédent.
const CHANGED: Color32 = Color32::from_rgb(255, 200, 60);
/// Couleur d'un drapeau à 1.
const FLAG_ON: Color32 = Color32::from_rgb(110, 220, 120);

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Tab {
    #[default]
    Cpu,
    Memory,
    Video,
    Palette,
    Sprites,
}

impl Tab {
    pub const ALL: [Tab; 5] = [
        Tab::Cpu,
        Tab::Memory,
        Tab::Video,
        Tab::Palette,
        Tab::Sprites,
    ];

    /// Nom stable (gui.toml).
    pub fn name(self) -> &'static str {
        match self {
            Tab::Cpu => "cpu",
            Tab::Memory => "memory",
            Tab::Video => "video",
            Tab::Palette => "palette",
            Tab::Sprites => "sprites",
        }
    }

    pub fn from_name(name: &str) -> Option<Tab> {
        Tab::ALL.into_iter().find(|t| t.name() == name)
    }

    pub fn label(self) -> &'static str {
        match self {
            Tab::Cpu => "CPU",
            Tab::Memory => "Mémoire",
            Tab::Video => "Vidéo",
            Tab::Palette => "Palette",
            Tab::Sprites => "Sprites",
        }
    }

    /// Données que le cœur doit copier pour cet onglet.
    pub fn level(self) -> DebugLevel {
        match self {
            Tab::Cpu => DebugLevel::Cpu,
            Tab::Memory => DebugLevel::Memory,
            Tab::Video | Tab::Palette | Tab::Sprites => DebugLevel::Video,
        }
    }
}

/// Région affichée par l'onglet Mémoire.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Region {
    /// RAM de travail du 68000, $100000-$10FFFF (octets).
    #[default]
    Work,
    /// RAM du Z80, $F800-$FFFF (octets).
    Z80,
    /// RAM palette, 0x2000 mots (index de couleur).
    Palette,
    /// VRAM du LSPC, adresses $0000-$87FF (mots).
    Vram,
}

impl Region {
    pub const ALL: [Region; 4] = [Region::Work, Region::Z80, Region::Palette, Region::Vram];

    pub fn name(self) -> &'static str {
        match self {
            Region::Work => "work",
            Region::Z80 => "z80",
            Region::Palette => "palette",
            Region::Vram => "vram",
        }
    }

    pub fn from_name(name: &str) -> Option<Region> {
        Region::ALL.into_iter().find(|r| r.name() == name)
    }

    pub fn label(self) -> &'static str {
        match self {
            Region::Work => "RAM 68000 ($100000)",
            Region::Z80 => "RAM Z80 ($F800)",
            Region::Palette => "RAM palette",
            Region::Vram => "VRAM",
        }
    }

    /// Adresse de la première cellule.
    pub fn base(self) -> u32 {
        match self {
            Region::Work => 0x10_0000,
            Region::Z80 => 0xF800,
            Region::Palette | Region::Vram => 0,
        }
    }

    /// Cellules de 16 bits (sinon des octets).
    pub fn words(self) -> bool {
        matches!(self, Region::Palette | Region::Vram)
    }

    /// Nombre de cellules par ligne.
    pub fn per_line(self) -> usize {
        if self.words() {
            8
        } else {
            16
        }
    }

    /// Nombre de chiffres de l'adresse.
    fn addr_digits(self) -> usize {
        match self {
            Region::Work => 6,
            _ => 4,
        }
    }

    /// Nombre de cellules de la région dans `s` (0 si l'instantané ne la contient pas).
    pub fn len(self, s: &DebugSnapshot) -> usize {
        match self {
            Region::Work => s.work_ram.len(),
            Region::Z80 => s.z80_ram.len(),
            Region::Palette => s.palette.len(),
            Region::Vram => s.vram.len(),
        }
    }

    /// Valeur de la cellule `i`.
    fn cell(self, s: &DebugSnapshot, i: usize) -> Option<u16> {
        match self {
            Region::Work => s.work_ram.get(i).map(|&b| u16::from(b)),
            Region::Z80 => s.z80_ram.get(i).map(|&b| u16::from(b)),
            Region::Palette => s.palette.get(i).copied(),
            Region::Vram => s.vram.get(i).copied(),
        }
    }
}

/// Colonne ASCII d'une ligne d'octets : caractères imprimables, « . » sinon.
pub fn ascii_col(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| {
            if (0x20..0x7F).contains(&b) {
                b as char
            } else {
                '.'
            }
        })
        .collect()
}

/// Ligne (0 = première) à afficher pour l'adresse saisie : « 10F300 », « $10F300 », « 0x10F300 »
/// ou un décalage dans la région. `len` = nombre de cellules.
pub fn parse_goto(text: &str, region: Region, len: usize) -> Option<usize> {
    let t = text.trim();
    let t = t
        .strip_prefix('$')
        .or_else(|| t.strip_prefix("0x"))
        .or_else(|| t.strip_prefix("0X"))
        .unwrap_or(t);
    let addr = u32::from_str_radix(t, 16).ok()?;
    let base = region.base();
    let off = if addr >= base && ((addr - base) as usize) < len {
        (addr - base) as usize
    } else if (addr as usize) < len {
        addr as usize
    } else {
        return None;
    };
    Some(off / region.per_line())
}

/// Un drapeau du registre d'état : (nom, actif).
pub type Flag = (&'static str, bool);

/// Drapeaux du SR du 68000 et masque d'interruption.
pub fn sr_flags(sr: u16) -> ([Flag; 7], u8) {
    let bit = |n: u16| sr & (1 << n) != 0;
    (
        [
            ("T", bit(15)),
            ("S", bit(13)),
            ("X", bit(4)),
            ("N", bit(3)),
            ("Z", bit(2)),
            ("V", bit(1)),
            ("C", bit(0)),
        ],
        ((sr >> 8) & 7) as u8,
    )
}

/// Drapeaux du registre F du Z80.
pub fn z80_flags(f: u8) -> [Flag; 8] {
    let bit = |n: u8| f & (1 << n) != 0;
    [
        ("S", bit(7)),
        ("Z", bit(6)),
        ("Y", bit(5)),
        ("H", bit(4)),
        ("X", bit(3)),
        ("P", bit(2)),
        ("N", bit(1)),
        ("C", bit(0)),
    ]
}

/// Registres du 68000 dans l'ordre d'affichage : (nom, valeur).
pub fn m68k_regs(c: &CpuState) -> Vec<(&'static str, u32)> {
    const D: [&str; 8] = ["D0", "D1", "D2", "D3", "D4", "D5", "D6", "D7"];
    const A: [&str; 7] = ["A0", "A1", "A2", "A3", "A4", "A5", "A6"];
    let mut v: Vec<(&'static str, u32)> = D.into_iter().zip(c.d).collect();
    v.extend(A.into_iter().zip(c.a));
    v.extend([
        ("USP", c.usp),
        ("SSP", c.ssp),
        ("SR", u32::from(c.sr)),
        ("PC", c.pc),
    ]);
    v
}

/// Noms des registres du 68000 qui diffèrent entre deux instantanés.
pub fn changed_regs(prev: &CpuState, cur: &CpuState) -> Vec<&'static str> {
    m68k_regs(prev)
        .into_iter()
        .zip(m68k_regs(cur))
        .filter(|(p, c)| p.1 != c.1)
        .map(|(_, c)| c.0)
        .collect()
}

/// Ce que l'utilisateur demande depuis la fenêtre.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DebugAction {
    Pause(bool),
    Step,
}

#[derive(Default)]
pub struct Debugger {
    pub open: bool,
    pub tab: Tab,
    pub region: Region,
    snap: Option<DebugSnapshot>,
    /// Instantané précédent (surlignage des changements).
    prev: Option<DebugSnapshot>,
    goto: String,
    goto_error: bool,
    /// Ligne à atteindre au prochain dessin de l'onglet Mémoire.
    scroll_to: Option<usize>,
    video: VideoDebug,
}

fn mono(text: impl Into<String>) -> RichText {
    RichText::new(text).monospace()
}

fn value(ui: &mut egui::Ui, text: String, changed: bool) {
    let t = mono(text);
    ui.label(if changed { t.color(CHANGED) } else { t });
}

fn flags_row(ui: &mut egui::Ui, flags: &[Flag]) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for (name, on) in flags {
            let t = mono(*name);
            ui.label(if *on {
                t.strong().color(FLAG_ON)
            } else {
                t.weak()
            });
        }
    });
}

impl Debugger {
    pub fn new(open: bool, tab: Tab, region: Region) -> Self {
        Self {
            open,
            tab,
            region,
            ..Self::default()
        }
    }

    /// Données à demander au cœur (rien si la fenêtre est fermée).
    pub fn level(&self) -> DebugLevel {
        if self.open {
            self.tab.level()
        } else {
            DebugLevel::Off
        }
    }

    /// Reçoit un nouvel instantané ; l'ancien sert de référence pour le surlignage.
    pub fn set_snapshot(&mut self, s: DebugSnapshot) {
        self.prev = self.snap.replace(s);
    }

    /// Affiche la fenêtre ; renvoie l'action demandée (pause, pas à pas).
    pub fn show(&mut self, ctx: &egui::Context, paused: bool) -> Option<DebugAction> {
        let mut action = None;
        let mut open = self.open;
        egui::Window::new("Débogueur")
            .open(&mut open)
            .default_size([700.0, 560.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for t in Tab::ALL {
                        ui.selectable_value(&mut self.tab, t, t.label());
                    }
                    ui.separator();
                    let label = if paused {
                        "▶ Reprendre (F5)"
                    } else {
                        "⏸ Pause (F5)"
                    };
                    if ui.button(label).clicked() {
                        action = Some(DebugAction::Pause(!paused));
                    }
                    if ui.button("⏭ Trame suivante (F6)").clicked() {
                        action = Some(DebugAction::Step);
                    }
                });
                ui.separator();
                let Some(s) = self.snap.as_ref() else {
                    ui.label("En attente d'un jeu…");
                    return;
                };
                ui.label(mono(format!(
                    "trame {}   ligne {:3}   t = {} mclk   {}",
                    s.frame,
                    s.vpos,
                    s.now,
                    if paused { "PAUSE" } else { "MARCHE" }
                )));
                ui.separator();
                match self.tab {
                    Tab::Cpu => cpu_tab(ui, s, self.prev.as_ref()),
                    Tab::Memory => self.memory_tab(ui),
                    Tab::Video | Tab::Palette | Tab::Sprites => {
                        if s.vram.is_empty() {
                            ui.label("Chargement…");
                        } else {
                            self.video.ui(ui, s);
                        }
                    }
                }
            });
        self.open = open;
        action
    }

    fn memory_tab(&mut self, ui: &mut egui::Ui) {
        let Some(s) = self.snap.as_ref() else { return };
        ui.horizontal(|ui| {
            for r in Region::ALL {
                if ui
                    .selectable_value(&mut self.region, r, r.label())
                    .changed()
                {
                    self.goto_error = false;
                }
            }
        });
        let region = self.region;
        let len = region.len(s);
        ui.horizontal(|ui| {
            ui.label("Aller à");
            let r = ui.add(
                egui::TextEdit::singleline(&mut self.goto)
                    .desired_width(80.0)
                    .hint_text("adresse hexa")
                    .font(egui::TextStyle::Monospace),
            );
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if enter || ui.button("OK").clicked() {
                self.scroll_to = parse_goto(&self.goto, region, len);
                self.goto_error = self.scroll_to.is_none();
            }
            if self.goto_error {
                ui.colored_label(Color32::LIGHT_RED, "adresse hors de la région");
            }
        });
        if len == 0 {
            ui.label("Chargement…");
            return;
        }
        let per = region.per_line();
        let row_h = ui.text_style_height(&egui::TextStyle::Monospace);
        let mut area = egui::ScrollArea::vertical().auto_shrink([false, false]);
        if let Some(row) = self.scroll_to.take() {
            area = area.vertical_scroll_offset(row as f32 * (row_h + ui.spacing().item_spacing.y));
        }
        let font = egui::TextStyle::Monospace.resolve(ui.style());
        let normal = ui.visuals().text_color();
        let weak = ui.visuals().weak_text_color();
        let prev = self.prev.as_ref();
        area.show_rows(ui, row_h, len.div_ceil(per), |ui, rows| {
            for row in rows {
                let start = row * per;
                let end = (start + per).min(len);
                let mut job = LayoutJob::default();
                let mut put = |text: String, color: Color32| {
                    job.append(
                        &text,
                        0.0,
                        TextFormat {
                            font_id: font.clone(),
                            color,
                            ..TextFormat::default()
                        },
                    );
                };
                put(
                    format!(
                        "{:0w$X}:",
                        region.base() as usize + start,
                        w = region.addr_digits()
                    ),
                    weak,
                );
                let mut bytes = Vec::with_capacity(per);
                for i in start..end {
                    let v = region.cell(s, i).unwrap_or(0);
                    let changed = prev
                        .and_then(|p| region.cell(p, i))
                        .is_some_and(|old| old != v);
                    let color = if changed { CHANGED } else { normal };
                    if region.words() {
                        put(format!(" {v:04X}"), color);
                    } else {
                        put(format!(" {v:02X}"), color);
                        bytes.push(v as u8);
                    }
                }
                if !region.words() {
                    put(format!("  {}", ascii_col(&bytes)), weak);
                }
                ui.label(job);
            }
        });
    }
}

/// Onglet CPU : 68000 à gauche, Z80 à droite, état des interruptions dessous.
fn cpu_tab(ui: &mut egui::Ui, s: &DebugSnapshot, prev: Option<&DebugSnapshot>) {
    let c = &s.m68k;
    let changed = prev.map(|p| changed_regs(&p.m68k, c)).unwrap_or_default();
    let is_changed = |name: &str| changed.contains(&name);
    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        ui.strong(if s.m68k_halted {
            "68000 (ARRÊTÉ)"
        } else {
            "68000"
        });
        let (flags, int) = sr_flags(c.sr);
        egui::Grid::new("m68k_top").num_columns(2).show(ui, |ui| {
            ui.label(mono("PC"));
            // `pc` est l'adresse du prochain mot à précharger : instruction courante + 4
            value(
                ui,
                format!("{:06X}", c.pc.wrapping_sub(4) & 0xFF_FFFF),
                is_changed("PC"),
            );
            ui.end_row();
            ui.label(mono("IR"));
            ui.label(mono(format!(
                "{:04X}  {:04X}",
                c.prefetch[0], c.prefetch[1]
            )));
            ui.end_row();
            ui.label(mono("SR"));
            value(ui, format!("{:04X}", c.sr), is_changed("SR"));
            ui.end_row();
            ui.label("");
            ui.horizontal(|ui| {
                flags_row(ui, &flags);
                ui.label(mono(format!("INT={int}")));
            });
            ui.end_row();
        });
        ui.add_space(4.0);
        egui::Grid::new("m68k_regs")
            .num_columns(4)
            .striped(true)
            .show(ui, |ui| {
                for i in 0..8 {
                    let d = ["D0", "D1", "D2", "D3", "D4", "D5", "D6", "D7"][i];
                    ui.label(mono(d));
                    value(ui, format!("{:08X}", c.d[i]), is_changed(d));
                    if i < 7 {
                        let a = ["A0", "A1", "A2", "A3", "A4", "A5", "A6"][i];
                        ui.label(mono(a));
                        value(ui, format!("{:08X}", c.a[i]), is_changed(a));
                    } else {
                        // A7 = pile active selon le bit S
                        let sup = c.sr & 0x2000 != 0;
                        let (name, v) = if sup { ("SSP", c.ssp) } else { ("USP", c.usp) };
                        ui.label(mono("A7"));
                        value(ui, format!("{v:08X}"), is_changed(name));
                    }
                    ui.end_row();
                }
                ui.label(mono("USP"));
                value(ui, format!("{:08X}", c.usp), is_changed("USP"));
                ui.label(mono("SSP"));
                value(ui, format!("{:08X}", c.ssp), is_changed("SSP"));
                ui.end_row();
            });

        let ui = &mut cols[1];
        let z = &s.z80;
        let p = prev.map(|p| &p.z80);
        ui.strong(if z.halted { "Z80 (HALT)" } else { "Z80" });
        egui::Grid::new("z80_regs")
            .num_columns(4)
            .striped(true)
            .show(ui, |ui| {
                let pair = |ui: &mut egui::Ui, name: &str, v: u16, old: Option<u16>| {
                    ui.label(mono(name));
                    value(ui, format!("{v:04X}"), old.is_some_and(|o| o != v));
                };
                pair(ui, "PC", z.pc, p.map(|p| p.pc));
                pair(ui, "SP", z.sp, p.map(|p| p.sp));
                ui.end_row();
                pair(ui, "AF", z.af(), p.map(|p| p.af()));
                pair(ui, "AF'", z.af2(), p.map(|p| p.af2()));
                ui.end_row();
                pair(ui, "BC", z.bc(), p.map(|p| p.bc()));
                pair(ui, "BC'", z.bc2(), p.map(|p| p.bc2()));
                ui.end_row();
                pair(ui, "DE", z.de(), p.map(|p| p.de()));
                pair(ui, "DE'", z.de2(), p.map(|p| p.de2()));
                ui.end_row();
                pair(ui, "HL", z.hl(), p.map(|p| p.hl()));
                pair(ui, "HL'", z.hl2(), p.map(|p| p.hl2()));
                ui.end_row();
                pair(ui, "IX", z.ix, p.map(|p| p.ix));
                pair(ui, "IY", z.iy, p.map(|p| p.iy));
                ui.end_row();
            });
        flags_row(ui, &z80_flags(z.f));
        ui.label(mono(format!(
            "I={:02X} R={:02X} IM={} IFF1={} IFF2={}",
            z.i,
            z.r,
            z.im,
            u8::from(z.iff1),
            u8::from(z.iff2)
        )));
    });
    ui.separator();
    ui.label(mono(format!(
        "Interruption demandée au 68000 : niveau {}   (masque INT={})",
        s.lspc.irq_level,
        (c.sr >> 8) & 7
    )));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_column() {
        assert_eq!(ascii_col(&[0x41, 0x42, 0x00]), "AB.");
        assert_eq!(ascii_col(&[0x1F, 0x20, 0x7E, 0x7F, 0xFF]), ". ~..");
    }

    #[test]
    fn sr_decoding() {
        let (f, int) = sr_flags(0x2704);
        assert_eq!(int, 7);
        let on: Vec<_> = f.iter().filter(|x| x.1).map(|x| x.0).collect();
        assert_eq!(on, ["S", "Z"]);
        let (f, int) = sr_flags(0x8011);
        assert_eq!(int, 0);
        let on: Vec<_> = f.iter().filter(|x| x.1).map(|x| x.0).collect();
        assert_eq!(on, ["T", "X", "C"]);
    }

    #[test]
    fn z80_flag_decoding() {
        let on: Vec<_> = z80_flags(0x41)
            .iter()
            .filter(|x| x.1)
            .map(|x| x.0)
            .collect();
        assert_eq!(on, ["Z", "C"]);
    }

    #[test]
    fn changed_registers_are_exactly_the_different_ones() {
        let a = CpuState::default();
        let mut b = a;
        assert!(changed_regs(&a, &b).is_empty());
        b.d[3] = 1;
        b.a[6] = 2;
        b.sr = 0x2000;
        b.pc = 0x100;
        assert_eq!(changed_regs(&a, &b), ["D3", "A6", "SR", "PC"]);
    }

    #[test]
    fn goto_parsing() {
        let w = Region::Work;
        assert_eq!(parse_goto("10F300", w, 0x10000), Some(0xF30));
        assert_eq!(parse_goto("$10F30F", w, 0x10000), Some(0xF30));
        assert_eq!(parse_goto("0x100000", w, 0x10000), Some(0));
        // décalage dans la région
        assert_eq!(parse_goto("20", w, 0x10000), Some(2));
        assert_eq!(parse_goto("200000", w, 0x10000), None);
        assert_eq!(parse_goto("zz", w, 0x10000), None);
        assert_eq!(parse_goto("7000", Region::Vram, 0x8800), Some(0x7000 / 8));
        assert_eq!(parse_goto("F810", Region::Z80, 0x800), Some(1));
        assert_eq!(parse_goto("10F300", w, 0), None);
    }

    #[test]
    fn tabs_and_regions_have_stable_names() {
        for t in Tab::ALL {
            assert_eq!(Tab::from_name(t.name()), Some(t));
        }
        for r in Region::ALL {
            assert_eq!(Region::from_name(r.name()), Some(r));
        }
        assert_eq!(Tab::from_name("bidule"), None);
        assert_eq!(Tab::Cpu.level(), DebugLevel::Cpu);
        assert_eq!(Tab::Sprites.level(), DebugLevel::Video);
        let closed = Debugger::default();
        assert_eq!(closed.level(), DebugLevel::Off);
    }
}
