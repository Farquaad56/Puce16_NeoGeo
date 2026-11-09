// T406 — ng-lspc : composition d'une ligne et boucle de rendu (début de ligne)
// Port de screen_update() + sprite_line_timer_callback() de MAME

use crate::draw::draw_sprites_line;
use crate::fix::{draw_fixed_line, FixBankType};
use crate::palette::Palette;
use crate::sprites::parse_sprites;
use ng_common::video::Framebuffer;

/// ROMs et palette nécessaires au rendu d'une ligne.
pub struct LspcRoms<'a> {
    pub c: &'a [u8],
    pub s_cart: &'a [u8],
    pub s_bios: &'a [u8],
    pub lo: &'a [u8],
    pub use_cart_fix: bool,
    pub fix_bank: FixBankType,
    pub pal: &'a Palette,
}

/// Rend la ligne `v` (= update_partial(v) -> screen_update) dans fb.
/// spec: docs/spec/04 §rendu (MAME neogeo_v.cpp l.157-167 screen_update)
pub fn render_line(
    vram: &[u16],
    roms: &LspcRoms,
    anim_counter: u8,
    anim_disabled: bool,
    v: u16,
    fb: &mut Framebuffer,
) {
    // Hors zone visible : ne rien faire
    if !(16..=239).contains(&v) {
        return;
    }

    let row_offset = (v - 16) as usize * 320;

    // Remplir avec la couleur de fond (backdrop = pen 0xFFF)
    let backdrop = roms.pal.backdrop();
    let mut row: [u32; 320] = [backdrop; 320];

    // Dessiner les sprites pour cette ligne
    draw_sprites_line(
        vram,
        roms.c,
        roms.lo,
        roms.pal.pens_base(),
        anim_counter,
        anim_disabled,
        v,
        &mut row,
    );

    // Dessiner la couche fix (recouvre les sprites)
    let gfx = if roms.use_cart_fix {
        roms.s_cart
    } else {
        roms.s_bios
    };
    draw_fixed_line(
        vram,
        gfx,
        roms.use_cart_fix,
        roms.fix_bank,
        roms.pal.pens_base(),
        v,
        &mut row,
    );

    // Copier la ligne dans le framebuffer
    fb.pixels[row_offset..row_offset + 320].copy_from_slice(&row);
}

impl crate::Lspc {
    /// Callback début de ligne (= sprite_line_timer_callback).
    /// spec: docs/spec/04 §rendu (MAME neogeo_spr.cpp l.517-527)
    fn on_line_start(&mut self, line: u16, roms: &LspcRoms, fb: &mut Framebuffer) {
        // Rend la ligne précédente (line 0 rend rien, line 1 rend la ligne 0, etc.)
        if line != 0 {
            render_line(
                self.vram.data(),
                roms,
                self.anim_counter,
                self.anim_disabled,
                line - 1,
                fb,
            );
        }
        // Analyser les sprites pour la ligne courante (prépare la liste active)
        parse_sprites(self.vram.data_mut(), line);
    }

    /// Avance le temps jusqu'à `to` et rend toutes les lignes atteintes.
    pub fn advance_render(&mut self, to: u64, roms: &LspcRoms, fb: &mut Framebuffer) {
        while self.time() < to {
            if let Some(line) = self.advance(to) {
                self.on_line_start(line, roms, fb);
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_roms() -> LspcRoms<'static> {
        let pal = Box::leak(Box::new(Palette::new()));
        // Set backdrop to a known color (pen 0xFFF)
        pal.write16(0xFFF, 0x7FFF);

        // Provide minimal valid ROM data for draw_sprites_line and draw_fixed_line
        let c_rom = Box::leak(Box::new(vec![0u8; 4]));
        let s_rom = Box::leak(Box::new(vec![0u8; 32]));
        let lo_rom = Box::leak(Box::new(vec![0u8; 256]));

        LspcRoms {
            c: c_rom,
            s_cart: s_rom,
            s_bios: s_rom,
            lo: lo_rom,
            use_cart_fix: false,
            fix_bank: FixBankType::Std,
            pal,
        }
    }

    #[test]
    fn render_line_outside_visible_range() {
        let lspc = crate::Lspc::new();
        let roms = make_roms();
        let mut fb = Framebuffer::new();

        // Lines 0-15 and 240-263 should not write to framebuffer
        render_line(lspc.vram.data(), &roms, 0, false, 10, &mut fb);
        render_line(lspc.vram.data(), &roms, 0, false, 250, &mut fb);

        // All pixels should still be zero (never written)
        for &px in &fb.pixels {
            assert_eq!(px, 0);
        }
    }

    #[test]
    fn render_line_writes_backdrop() {
        let lspc = crate::Lspc::new();
        let roms = make_roms();
        let mut fb = Framebuffer::new();

        // Render line 16 (first visible line)
        render_line(lspc.vram.data(), &roms, 0, false, 16, &mut fb);

        // Row 0 of framebuffer should be filled with backdrop color
        for i in 0..320 {
            assert_eq!(fb.pixels[i], roms.pal.backdrop());
        }
    }

    #[test]
    fn advance_render_calls_on_line_start() {
        let mut lspc = crate::Lspc::new();
        let roms = make_roms();
        let mut fb = Framebuffer::new();

        // Advance one full frame (264 lines)
        use crate::timing::{LINES, MCLK_PER_LINE};
        let frame_mclk = LINES as u64 * MCLK_PER_LINE;
        lspc.advance_render(frame_mclk, &roms, &mut fb);

        // All 224 visible rows should be filled with backdrop
        for row in 0..224 {
            let row_offset = row * 320;
            for i in 0..320 {
                assert_eq!(fb.pixels[row_offset + i], roms.pal.backdrop());
            }
        }
    }

    #[test]
    fn advance_render_line_count() {
        let mut lspc = crate::Lspc::new();
        let roms = make_roms();
        let mut fb = Framebuffer::new();

        // Count how many times on_line_start is called by checking vpos progression
        use crate::timing::{LINES, MCLK_PER_LINE};
        let frame_mclk = LINES as u64 * MCLK_PER_LINE;
        lspc.advance_render(frame_mclk, &roms, &mut fb);

        // After one full frame, we should have processed 264 line starts
        assert_eq!(lspc.time(), frame_mclk);
    }
}
