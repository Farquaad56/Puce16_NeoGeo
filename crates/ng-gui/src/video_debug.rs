//! Débogage vidéo : palette 16x16, table des sprites, image de la couche fix.

use eframe::egui::{self, Color32};
use ng_core::debug::{FIX_H, FIX_W};
use ng_core::DebugSnapshot;
use ng_lspc::palette::Palette;

/// Nombre de sprites (SCB2/3/4 : 381 entrées).
pub const SPRITES: usize = 381;

/// Couleur DAC d'un mot de palette (0x0000 = noir).
pub fn word_color(word: u16) -> Color32 {
    let mut p = Palette::new();
    p.write16(0, word);
    pen_color(p.pens[0])
}

fn pen_color(pen: u32) -> Color32 {
    Color32::from_rgb((pen >> 16) as u8, (pen >> 8) as u8, pen as u8)
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

/// SCB3 décodé : (y, chaîné, taille en tuiles).
pub fn decode_scb3(v: u16) -> (u16, bool, u8) {
    (v >> 7, v & 0x40 != 0, (v & 0x3F) as u8)
}

/// Une ligne de la table des sprites, de largeur fixe.
pub fn sprite_row(i: usize, scb1: [u16; 2], scb2: u16, scb3: u16, scb4: u16) -> String {
    let (y, chain, size) = decode_scb3(scb3);
    let tile = scb1[0] as u32 | (((scb1[1] >> 4) & 0xF) as u32) << 16;
    format!(
        "{i:03} tile={tile:05X} pal={:02X} flip={} zoom={scb2:04X} y={y:03X} ch={} sz={size:02X} x={:03X}",
        scb1[1] >> 8,
        scb1[1] & 3,
        u8::from(chain),
        scb4 >> 7,
    )
}

/// Indices des sprites actifs (taille effective non nulle, chaînage inclus).
pub fn active_sprites(vram: &[u16]) -> Vec<usize> {
    let mut rows = 0;
    let mut out = Vec::new();
    for n in 0..SPRITES {
        let scb3 = vram[0x8200 + n];
        if scb3 & 0x40 == 0 {
            rows = scb3 & 0x3F;
        }
        if rows != 0 {
            out.push(n);
        }
    }
    out
}

#[derive(Default)]
pub struct VideoDebug {
    page: usize,
    tex: Option<egui::TextureHandle>,
}

impl VideoDebug {
    pub fn show(&mut self, ctx: &egui::Context, s: &DebugSnapshot) {
        egui::Window::new("Vidéo").show(ctx, |ui| {
            ui.monospace(format!(
                "offset={:04X} modulo={:04X} irq2={:04X} disp={:08X} anim={}",
                s.lspc.offset,
                s.lspc.modulo,
                s.lspc.irq2_ctrl,
                s.lspc.display_counter,
                s.lspc.anim_counter
            ));
            ui.horizontal(|ui| {
                ui.label("Page palette");
                ui.add(egui::DragValue::new(&mut self.page).range(0..=31));
            });
            let base = self.page * 256;
            let colors = palette_colors(&s.palette[base..base + 256]);
            egui::Grid::new("pal").spacing([1.0, 1.0]).show(ui, |ui| {
                for (i, c) in colors.iter().enumerate() {
                    let (r, resp) =
                        ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                    ui.painter().rect_filled(r, 0.0, *c);
                    resp.on_hover_text(format!("{:02X} : {:04X}", i, s.palette[base + i]));
                    if i % 16 == 15 {
                        ui.end_row();
                    }
                }
            });
            ui.separator();
            let img = egui::ColorImage::new(
                [FIX_W, FIX_H],
                s.fix_image
                    .iter()
                    .map(|&p| Color32::from_rgb((p >> 16) as u8, (p >> 8) as u8, p as u8))
                    .collect(),
            );
            let tex = match &mut self.tex {
                Some(t) => {
                    t.set(img, egui::TextureOptions::NEAREST);
                    t.clone()
                }
                None => {
                    let t = ctx.load_texture("fixdbg", img, egui::TextureOptions::NEAREST);
                    self.tex = Some(t.clone());
                    t
                }
            };
            ui.image((tex.id(), egui::vec2(FIX_W as f32, FIX_H as f32)));
            ui.separator();
            let act = active_sprites(&s.vram);
            ui.label(format!("{} sprites actifs", act.len()));
            egui::ScrollArea::vertical().max_height(200.0).show_rows(
                ui,
                ui.text_style_height(&egui::TextStyle::Monospace),
                act.len(),
                |ui, rows| {
                    for r in rows {
                        let n = act[r];
                        ui.monospace(sprite_row(
                            n,
                            [s.vram[n * 64], s.vram[n * 64 + 1]],
                            s.vram[0x8000 + n],
                            s.vram[0x8200 + n],
                            s.vram[0x8400 + n],
                        ));
                    }
                },
            );
        });
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
    fn scb3_decoding() {
        assert_eq!(decode_scb3(0x0000), (0, false, 0));
        assert_eq!(decode_scb3(0xFE00 | 0x40 | 0x05), (0x1FC, true, 5));
        assert_eq!(decode_scb3((0x20 << 7) | 0x21), (0x20, false, 0x21));
    }

    #[test]
    fn sprite_row_fixed_format() {
        assert_eq!(
            sprite_row(5, [0x1234, 0x2A51], 0x0FFF, (0x20 << 7) | 0x21, 0x1234),
            "005 tile=51234 pal=2A flip=1 zoom=0FFF y=020 ch=0 sz=21 x=024"
        );
    }

    #[test]
    fn active_sprites_follow_chains() {
        let mut v = vec![0u16; 0x8800];
        v[0x8200] = 3; // taille 3
        v[0x8201] = 0x40; // chaîné : hérite de la taille
        v[0x8202] = 0; // taille 0 : inactif
        assert_eq!(active_sprites(&v), vec![0, 1]);
    }
}
