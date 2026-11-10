//! Onglets vidéo du débogueur : registres LSPC et couche fix, palette, sprites.

use eframe::egui::{self, Color32, RichText, Stroke};
use ng_core::debug::{FIX_H, FIX_W};
use ng_core::DebugSnapshot;
use ng_lspc::palette::Palette;
use ng_lspc::sprites::sprite_on_scanline;

use crate::debugger::Tab;

/// Nombre de sprites (SCB2/3/4 : 381 entrées).
pub const SPRITES: usize = 381;
/// Agrandissement de l'image de la couche fix quand le panneau est assez large (sinon 1).
const FIX_ZOOM: f32 = 2.0;
/// Côté d'une pastille de palette, en points.
const SWATCH: f32 = 14.0;

fn pen_color(pen: u32) -> Color32 {
    Color32::from_rgb((pen >> 16) as u8, (pen >> 8) as u8, pen as u8)
}

/// Couleur DAC d'un mot de palette (0x0000 = noir).
pub fn word_color(word: u16) -> Color32 {
    let mut p = Palette::new();
    p.write16(0, word);
    pen_color(p.pens[0])
}

/// Couleurs d'une suite de mots de palette (une seule palette temporaire).
pub fn palette_colors(words: &[u16]) -> Vec<Color32> {
    let mut p = Palette::new();
    words
        .iter()
        .enumerate()
        .map(|(i, &w)| {
            p.write16(i & 0xFFF, w);
            pen_color(p.pens[i & 0xFFF])
        })
        .collect()
}

/// « #RRGGBB ».
pub fn hex_color(c: Color32) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b())
}

/// Texte d'une couleur : « 010 : 0F00 → #F80000 » (+ « sombre » si le bit 15 est mis).
pub fn pen_label(index: usize, word: u16) -> String {
    format!(
        "{index:03X} : {word:04X} → {}{}",
        hex_color(word_color(word)),
        if word & 0x8000 != 0 { " (sombre)" } else { "" }
    )
}

/// SCB2 décodé : (zoom horizontal 0-15, zoom vertical 0-255).
pub fn decode_scb2(v: u16) -> (u8, u8) {
    (((v >> 8) & 0xF) as u8, (v & 0xFF) as u8)
}

/// SCB3 décodé : (y brut, chaîné, taille en tuiles).
pub fn decode_scb3(v: u16) -> (u16, bool, u8) {
    (v >> 7, v & 0x40 != 0, (v & 0x3F) as u8)
}

/// SCB4 décodé : X sur 9 bits signés.
pub fn decode_scb4(v: u16) -> i16 {
    let x = (v >> 7) as i16;
    if x & 0x100 != 0 {
        x - 0x200
    } else {
        x
    }
}

/// Un sprite actif, avec position et taille effectives (héritées de la tête de chaîne).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpriteInfo {
    pub n: usize,
    /// Première tuile (20 bits), palette et retournement (bit 0 = H, bit 1 = V).
    pub tile: u32,
    pub pal: u8,
    pub flip: u8,
    pub zoom_h: u8,
    pub zoom_v: u8,
    /// X effectif, signé sur 9 bits : une tuile chaînée suit la précédente.
    pub x: i16,
    /// Y effectif = 0x200 - SCB3.y (ligne MAME du haut du sprite, modulo 512).
    pub y: u16,
    /// Hauteur en tuiles.
    pub rows: u8,
    pub chained: bool,
}

/// Sprites actifs (hauteur effective non nulle), comme `parse_sprites` les parcourt.
pub fn sprite_list(vram: &[u16]) -> Vec<SpriteInfo> {
    let mut out = Vec::new();
    let (mut x, mut y, mut rows, mut zoom_v) = (0i16, 0u16, 0u8, 0u8);
    // zoom horizontal du sprite précédent (actif ou non) : largeur à sauter en cas de chaînage
    let mut prev_zoom_h = 0u8;
    for n in 0..SPRITES {
        let (zoom_h, zv) = decode_scb2(vram[0x8000 + n]);
        let (raw_y, chained, size) = decode_scb3(vram[0x8200 + n]);
        if chained {
            // même repli sur 9 bits que le matériel
            x = (((x + i16::from(prev_zoom_h) + 1) & 0x1FF) ^ 0x100) - 0x100;
        } else {
            x = decode_scb4(vram[0x8400 + n]);
            y = (0x200 - raw_y) & 0x1FF;
            rows = size;
            zoom_v = zv;
        }
        prev_zoom_h = zoom_h;
        let info = SpriteInfo {
            n,
            tile: u32::from(vram[n * 64]) | (u32::from((vram[n * 64 + 1] >> 4) & 0xF) << 16),
            pal: (vram[n * 64 + 1] >> 8) as u8,
            flip: (vram[n * 64 + 1] & 3) as u8,
            zoom_h,
            zoom_v,
            x,
            y,
            rows,
            chained,
        };
        if rows != 0 {
            out.push(info);
        }
    }
    out
}

/// Le sprite couvre-t-il la ligne `line` (numérotation MAME : 16-239 visibles) ?
pub fn on_line(s: &SpriteInfo, line: u16) -> bool {
    sprite_on_scanline(i32::from(line), i32::from(s.y), i32::from(s.rows))
}

/// En-tête du tableau des sprites (mêmes largeurs que `sprite_row`).
pub const SPRITE_HEADER: &str = "  N  TUILE  PL  F ZH ZV    X    Y  H  C";

/// Une ligne du tableau des sprites, de largeur fixe (X et Y en décimal, comme les lignes).
pub fn sprite_row(s: &SpriteInfo) -> String {
    format!(
        "{:3}  {:05X}  {:02X}  {} {:2X} {:02X} {:4} {:4} {:2}  {}",
        s.n,
        s.tile,
        s.pal,
        s.flip,
        s.zoom_h,
        s.zoom_v,
        s.x,
        s.y,
        s.rows,
        if s.chained { "→" } else { " " }
    )
}

/// Critère de tri du tableau des sprites.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SpriteSort {
    #[default]
    Number,
    X,
    Y,
    Height,
    Tile,
}

impl SpriteSort {
    const ALL: [SpriteSort; 5] = [
        SpriteSort::Number,
        SpriteSort::X,
        SpriteSort::Y,
        SpriteSort::Height,
        SpriteSort::Tile,
    ];

    fn label(self) -> &'static str {
        match self {
            SpriteSort::Number => "Numéro",
            SpriteSort::X => "X",
            SpriteSort::Y => "Y",
            SpriteSort::Height => "Hauteur",
            SpriteSort::Tile => "Tuile",
        }
    }
}

/// Trie les sprites (tri stable : à égalité, l'ordre des numéros est gardé).
pub fn sort_sprites(list: &mut [SpriteInfo], by: SpriteSort) {
    match by {
        SpriteSort::Number => list.sort_by_key(|s| s.n),
        SpriteSort::X => list.sort_by_key(|s| s.x),
        SpriteSort::Y => list.sort_by_key(|s| s.y),
        SpriteSort::Height => list.sort_by_key(|s| std::cmp::Reverse(s.rows)),
        SpriteSort::Tile => list.sort_by_key(|s| s.tile),
    }
}

/// Tuile de la couche fix sous le point (x, y) de l'image agrandie `zoom` fois.
pub fn tile_at(x: f32, y: f32, zoom: f32) -> Option<(usize, usize)> {
    if x < 0.0 || y < 0.0 || zoom <= 0.0 {
        return None;
    }
    let (col, row) = ((x / zoom) as usize / 8, (y / zoom) as usize / 8);
    (col < FIX_W / 8 && row < FIX_H / 8).then_some((col, row))
}

/// Adresse VRAM de la tuile fix (colonne, ligne) : $7000 + colonne * 32 + ligne.
pub fn fix_tile_addr(col: usize, row: usize) -> usize {
    0x7000 + col * 0x20 + row
}

/// « (39,31) VRAM 74FF  code=123 pal=4 ».
pub fn fix_tile_label(col: usize, row: usize, word: u16) -> String {
    format!(
        "({col},{row}) VRAM {:04X}  code={:03X} pal={:X}",
        fix_tile_addr(col, row),
        word & 0xFFF,
        word >> 12
    )
}

/// Bits du registre de contrôle du timer ($3C0006, bits 4-7).
pub fn irq2_text(ctrl: u16) -> String {
    let bit = |n: u16| ctrl & (1 << n) != 0;
    let mut parts = Vec::new();
    if bit(4) {
        parts.push("IRQ activée");
    }
    if bit(5) {
        parts.push("recharge à l'écriture");
    }
    if bit(6) {
        parts.push("recharge au vblank");
    }
    if bit(7) {
        parts.push("recharge à zéro");
    }
    if parts.is_empty() {
        "désactivé".into()
    } else {
        parts.join(", ")
    }
}

pub struct VideoDebug {
    /// Page de 256 couleurs (0-15) et banque (0-1) affichées par l'onglet Palette.
    page: usize,
    bank: Option<usize>,
    /// Dernière couleur cliquée : (index, mot).
    picked: Option<(usize, u16)>,
    grid: bool,
    visible_area: bool,
    line_filter: bool,
    line: u16,
    sort: SpriteSort,
    tex: Option<egui::TextureHandle>,
}

impl Default for VideoDebug {
    fn default() -> Self {
        Self {
            page: 0,
            bank: None,
            picked: None,
            grid: false,
            visible_area: true,
            line_filter: false,
            // milieu de l'écran visible
            line: 128,
            sort: SpriteSort::default(),
            tex: None,
        }
    }
}

fn mono(text: impl Into<String>) -> RichText {
    RichText::new(text).monospace()
}

impl VideoDebug {
    pub fn ui(&mut self, ui: &mut egui::Ui, s: &DebugSnapshot, tab: Tab) {
        if tab == Tab::Sprites {
            // le tableau des sprites a son propre défilement
            self.sprites_tab(ui, s);
            return;
        }
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| match tab {
                Tab::Palette => self.palette_tab(ui, s),
                _ => self.video_tab(ui, s),
            });
    }

    fn video_tab(&mut self, ui: &mut egui::Ui, s: &DebugSnapshot) {
        let l = &s.lspc;
        egui::Grid::new("lspc_regs")
            .num_columns(4)
            .striped(true)
            .show(ui, |ui| {
                let row = |ui: &mut egui::Ui, a: &str, av: String, b: &str, bv: String| {
                    ui.label(a);
                    ui.label(mono(av));
                    ui.label(b);
                    ui.label(mono(bv));
                    ui.end_row();
                };
                row(
                    ui,
                    "Ligne (vpos)",
                    format!("{}", s.vpos),
                    "IRQ demandée",
                    format!("niveau {}", l.irq_level),
                );
                row(
                    ui,
                    "Adresse VRAM",
                    format!("{:04X}", l.offset),
                    "Pas (modulo)",
                    format!("{:04X}", l.modulo),
                );
                row(
                    ui,
                    "Compteur timer",
                    format!("{:08X}", l.display_counter),
                    "Contrôle timer",
                    format!("{:02X} : {}", l.irq2_ctrl, irq2_text(l.irq2_ctrl)),
                );
                row(
                    ui,
                    "Auto-animation",
                    format!(
                        "compteur {} vitesse {}{}",
                        l.anim_counter,
                        l.anim_speed,
                        if l.anim_disabled { " (arrêtée)" } else { "" }
                    ),
                    "Palette",
                    format!(
                        "banque {}{}",
                        l.pal_bank,
                        if l.shadow { ", ombre" } else { "" }
                    ),
                );
            });
        ui.separator();
        ui.horizontal(|ui| {
            ui.strong("Couche fix");
            ui.label(if l.fix_from_cart {
                "(cartouche)"
            } else {
                "(BIOS)"
            });
            ui.checkbox(&mut self.grid, "Grille 8x8");
            ui.checkbox(&mut self.visible_area, "Zone visible");
        });
        if s.fix_image.len() != FIX_W * FIX_H {
            ui.label("Chargement…");
            return;
        }
        let img = egui::ColorImage::new(
            [FIX_W, FIX_H],
            s.fix_image.iter().map(|&p| pen_color(p)).collect(),
        );
        let tex = match &mut self.tex {
            Some(t) => {
                t.set(img, egui::TextureOptions::NEAREST);
                t.clone()
            }
            None => {
                let t = ui
                    .ctx()
                    .load_texture("fixdbg", img, egui::TextureOptions::NEAREST);
                self.tex = Some(t.clone());
                t
            }
        };
        let zoom = if ui.available_width() >= FIX_W as f32 * FIX_ZOOM {
            FIX_ZOOM
        } else {
            1.0
        };
        let size = egui::vec2(FIX_W as f32, FIX_H as f32) * zoom;
        let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::hover());
        let p = ui.painter_at(rect);
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        p.image(tex.id(), rect, uv, Color32::WHITE);
        if self.grid {
            let line = Stroke::new(1.0, Color32::from_white_alpha(40));
            for c in 1..FIX_W / 8 {
                p.vline(rect.left() + c as f32 * 8.0 * zoom, rect.y_range(), line);
            }
            for r in 1..FIX_H / 8 {
                p.hline(rect.x_range(), rect.top() + r as f32 * 8.0 * zoom, line);
            }
        }
        if self.visible_area {
            // lignes 16 à 239 : les 224 lignes affichées
            let vis = egui::Rect::from_min_max(
                egui::pos2(rect.left(), rect.top() + 16.0 * zoom),
                egui::pos2(rect.right(), rect.top() + 240.0 * zoom),
            );
            p.rect_stroke(
                vis,
                0.0,
                Stroke::new(1.0, Color32::YELLOW),
                egui::StrokeKind::Inside,
            );
        }
        let hovered = resp.hover_pos().and_then(|pos| {
            let d = pos - rect.min;
            tile_at(d.x, d.y, zoom)
        });
        match hovered {
            Some((col, row)) => {
                let cell = egui::Rect::from_min_size(
                    rect.min + egui::vec2(col as f32, row as f32) * 8.0 * zoom,
                    egui::vec2(8.0, 8.0) * zoom,
                );
                p.rect_stroke(
                    cell,
                    0.0,
                    Stroke::new(1.0, Color32::WHITE),
                    egui::StrokeKind::Outside,
                );
                let word = s.vram.get(fix_tile_addr(col, row)).copied().unwrap_or(0);
                ui.label(mono(fix_tile_label(col, row, word)));
            }
            None => {
                ui.label(mono("Survolez une tuile pour voir son code et sa palette"));
            }
        }
    }

    fn palette_tab(&mut self, ui: &mut egui::Ui, s: &DebugSnapshot) {
        let active = usize::from(s.lspc.pal_bank & 1);
        let bank = *self.bank.get_or_insert(active);
        ui.horizontal(|ui| {
            ui.label("Banque");
            for b in 0..2 {
                let label = if b == active {
                    format!("{b} (active)")
                } else {
                    format!("{b}")
                };
                if ui.selectable_label(bank == b, label).clicked() {
                    self.bank = Some(b);
                }
            }
            ui.separator();
            ui.label("Palettes");
            ui.add(
                egui::DragValue::new(&mut self.page)
                    .range(0..=15)
                    .custom_formatter(|v, _| {
                        let p = v as usize * 16;
                        format!("{:02X}-{:02X}", p, p + 15)
                    }),
            );
            if ui.small_button("<").clicked() {
                self.page = self.page.saturating_sub(1);
            }
            if ui.small_button(">").clicked() {
                self.page = (self.page + 1).min(15);
            }
        });
        let base = bank * 0x1000 + self.page * 256;
        let Some(words) = s.palette.get(base..base + 256) else {
            ui.label("Chargement…");
            return;
        };
        let colors = palette_colors(words);
        let mut hovered = None;
        egui::Grid::new("pal")
            .spacing([1.0, 1.0])
            .min_col_width(0.0)
            .show(ui, |ui| {
                ui.label("");
                for c in 0..16 {
                    ui.label(mono(format!("{c:X}")).weak());
                }
                ui.end_row();
                for (i, c) in colors.iter().enumerate() {
                    if i % 16 == 0 {
                        ui.label(mono(format!("{:02X} ", self.page * 16 + i / 16)).weak());
                    }
                    let (r, resp) =
                        ui.allocate_exact_size(egui::vec2(SWATCH, SWATCH), egui::Sense::click());
                    ui.painter().rect_filled(r, 0.0, *c);
                    let index = (base & 0xFFF) + i;
                    if self.picked == Some((index, words[i])) {
                        ui.painter().rect_stroke(
                            r,
                            0.0,
                            Stroke::new(1.0, Color32::WHITE),
                            egui::StrokeKind::Inside,
                        );
                    }
                    if resp.hovered() {
                        hovered = Some((index, words[i]));
                    }
                    if resp.clicked() {
                        self.picked = Some((index, words[i]));
                        ui.ctx().copy_text(hex_color(*c));
                    }
                    if i % 16 == 15 {
                        ui.end_row();
                    }
                }
            });
        ui.separator();
        let text = match hovered.or(self.picked) {
            Some((index, word)) => pen_label(index, word),
            None => "Survolez une couleur ; un clic copie #RRGGBB".to_string(),
        };
        ui.label(mono(text));
        if let Some(&back) = s.palette.get(bank * 0x1000 + 0xFFF) {
            ui.horizontal(|ui| {
                let (r, _) =
                    ui.allocate_exact_size(egui::vec2(SWATCH, SWATCH), egui::Sense::hover());
                ui.painter().rect_filled(r, 0.0, word_color(back));
                ui.label(mono(format!("Fond d'écran  {}", pen_label(0xFFF, back))));
            });
        }
    }

    fn sprites_tab(&mut self, ui: &mut egui::Ui, s: &DebugSnapshot) {
        let mut list = sprite_list(&s.vram);
        let total = list.len();
        let on = list.iter().filter(|x| on_line(x, self.line)).count();
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.line_filter, "Seulement la ligne");
            ui.add(egui::DragValue::new(&mut self.line).range(0..=263));
            ui.weak("(16-239 visibles)");
            ui.separator();
            egui::ComboBox::from_label("Tri")
                .selected_text(self.sort.label())
                .show_ui(ui, |ui| {
                    for k in SpriteSort::ALL {
                        ui.selectable_value(&mut self.sort, k, k.label());
                    }
                });
        });
        ui.label(format!(
            "{total} sprites actifs, {on} sur la ligne {} (96 au plus par ligne)",
            self.line
        ));
        if self.line_filter {
            list.retain(|x| on_line(x, self.line));
        }
        sort_sprites(&mut list, self.sort);
        ui.label(mono(SPRITE_HEADER).strong());
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show_rows(
                ui,
                ui.text_style_height(&egui::TextStyle::Monospace),
                list.len(),
                |ui, rows| {
                    for r in rows {
                        ui.add(
                            egui::Label::new(mono(sprite_row(&list[r])))
                                .wrap_mode(egui::TextWrapMode::Extend),
                        );
                    }
                },
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_black_and_white() {
        assert_eq!(word_color(0x0000), Color32::from_rgb(0, 0, 0));
        let w = word_color(0x7FFF);
        assert!(w.r() > 0xF0 && w.g() > 0xF0 && w.b() > 0xF0, "{w:?}");
    }

    #[test]
    fn pen_labels() {
        assert_eq!(pen_label(0x10, 0x0000), "010 : 0000 → #000000");
        let red = pen_label(0x10, 0x0F00);
        assert!(red.starts_with("010 : 0F00 → #"), "{red}");
        let c = word_color(0x0F00);
        assert!(c.r() > 0xE0 && c.g() == 0 && c.b() == 0, "{c:?}");
        assert_eq!(&red[red.len() - 7..], hex_color(c));
        assert!(pen_label(0, 0x8000).ends_with("(sombre)"));
    }

    #[test]
    fn scb_decoding() {
        assert_eq!(decode_scb2(0x0FFF), (0xF, 0xFF));
        assert_eq!(decode_scb2(0x0380), (0x3, 0x80));
        assert_eq!(decode_scb3(0x0000), (0, false, 0));
        assert_eq!(decode_scb3(0xFE00 | 0x40 | 0x05), (0x1FC, true, 5));
        assert_eq!(decode_scb3((0x20 << 7) | 0x21), (0x20, false, 0x21));
        assert_eq!(decode_scb4(0x1234), 0x24);
        assert_eq!(decode_scb4(0xFF80), -1);
        assert_eq!(decode_scb4(0x8000), -256);
    }

    fn vram_with_chain() -> Vec<u16> {
        let mut v = vec![0u16; 0x8800];
        // sprite 0 : tête, X = 0x40, Y brut 0x1E0 (haut à la ligne 0x20), 2 tuiles, zoom H plein
        v[0] = 0x1234;
        v[1] = 0x2A51;
        v[0x8000] = 0x0FFF;
        v[0x8200] = (0x1E0 << 7) | 2;
        v[0x8400] = 0x40 << 7;
        // sprite 1 : chaîné
        v[0x8001] = 0x07FF;
        v[0x8201] = 0x40;
        // sprite 2 : taille nulle
        v[0x8202] = 0;
        v
    }

    #[test]
    fn sprite_list_follows_chains() {
        let l = sprite_list(&vram_with_chain());
        assert_eq!(l.len(), 2);
        assert_eq!(
            l[0],
            SpriteInfo {
                n: 0,
                tile: 0x51234,
                pal: 0x2A,
                flip: 1,
                zoom_h: 0xF,
                zoom_v: 0xFF,
                x: 0x40,
                y: 0x20,
                rows: 2,
                chained: false
            }
        );
        // le sprite chaîné hérite de Y, de la hauteur et du zoom V ; X suit (zoom H 15 + 1)
        assert_eq!(
            (l[1].x, l[1].y, l[1].rows, l[1].zoom_v),
            (0x50, 0x20, 2, 0xFF)
        );
        assert!(l[1].chained);
        assert_eq!(l[1].zoom_h, 7);
    }

    #[test]
    fn line_filter() {
        let l = sprite_list(&vram_with_chain());
        assert!(on_line(&l[0], 0x30));
        assert!(!on_line(&l[0], 0x50));
        assert!(on_line(&l[0], 0x20) && on_line(&l[0], 0x3F) && !on_line(&l[0], 0x40));
    }

    #[test]
    fn sprite_row_has_the_header_width() {
        let l = sprite_list(&vram_with_chain());
        let row = sprite_row(&l[0]);
        assert_eq!(row, "  0  51234  2A  1  F FF   64   32  2   ");
        assert_eq!(row.chars().count(), SPRITE_HEADER.chars().count());
        assert!(sprite_row(&l[1]).ends_with('→'));
    }

    #[test]
    fn sorting() {
        let mut l = sprite_list(&vram_with_chain());
        sort_sprites(&mut l, SpriteSort::X);
        assert_eq!(l[0].n, 0);
        l[0].x = 0x100;
        sort_sprites(&mut l, SpriteSort::X);
        assert_eq!(l[0].n, 1);
        sort_sprites(&mut l, SpriteSort::Number);
        assert_eq!(l[0].n, 0);
    }

    #[test]
    fn fix_tiles() {
        assert_eq!(tile_at(319.0, 255.0, 1.0), Some((39, 31)));
        assert_eq!(tile_at(639.9, 511.9, 2.0), Some((39, 31)));
        assert_eq!(tile_at(17.0, 9.0, 2.0), Some((1, 0)));
        assert_eq!(tile_at(320.0, 0.0, 1.0), None);
        assert_eq!(tile_at(-1.0, 0.0, 1.0), None);
        assert_eq!(fix_tile_addr(39, 31), 0x74FF);
        assert_eq!(
            fix_tile_label(39, 31, 0x4123),
            "(39,31) VRAM 74FF  code=123 pal=4"
        );
    }

    #[test]
    fn timer_control_text() {
        assert_eq!(irq2_text(0x00), "désactivé");
        assert_eq!(irq2_text(0x50), "IRQ activée, recharge au vblank");
    }
}
