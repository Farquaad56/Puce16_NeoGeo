// T405a — ng-lspc : dessin des sprites d'une ligne (zoom, flips, animation, décodage C ROM)
// Port de neogeo_spr.cpp draw_sprites() et helpers

use crate::sprites::{parse_sprites, sprite_on_scanline, MAX_SPRITES_PER_LINE};
use crate::zoom_table::ZOOM_X;

/// Compute the address mask for sprite graphics based on region size.
/// spec: docs/spec/04 §sprites (MAME neogeo_spr.cpp l.542-562 get_region_mask)
pub fn sprite_gfx_mask(len: usize) -> u32 {
    let mut mask = 0xFFFF_FFFF;
    for bit in (0..32).rev().map(|b| 1u32 << b) {
        if ((len as u32) * 2 - 1) & bit != 0 {
            break;
        }
        mask >>= 1;
    }
    mask
}

/// Draw a single sprite pixel from C ROM data.
/// spec: docs/spec/04 §sprites (MAME neogeo_spr.cpp l.631-654 draw_pixel)
fn draw_pixel(c_rom: &[u8], romaddr: u32, dst: &mut u32, line_pens: &[u32]) {
    let s = ((romaddr & !0xFF) >> 1) | (((romaddr & 8) ^ 8) << 3) | ((romaddr & 0xF0) >> 2);
    let x = romaddr & 7;

    // Extract 4 color planes at pixel position x
    // Adresse au-delà de la ROM C (région non puissance de 2, ex. mslugx 0x3000000) : rien n'est
    // câblé, on lit 0 (pixel transparent) au lieu de paniquer.
    let at = |i: usize| c_rom.get(i).copied().unwrap_or(0);
    let gfx = (bit(at(s as usize + 3), x) << 3)
        | (bit(at(s as usize + 1), x) << 2)
        | (bit(at(s as usize + 2), x) << 1)
        | bit(at(s as usize), x);

    if gfx != 0 {
        *dst = line_pens[gfx as usize];
    }
}

fn bit(byte: u8, pos: u32) -> u8 {
    (byte >> pos) & 1
}

/// Draw all sprites for a single scanline.
/// spec: docs/spec/04 §sprites (MAME neogeo_spr.cpp l.277-461 draw_sprites)
#[allow(clippy::too_many_arguments)]
pub fn draw_sprites_line(
    vram: &[u16],
    c_rom: &[u8],
    zoomy: &[u8],
    pens: &[u32],
    anim_counter: u8,
    anim_disabled: bool,
    vpos: u16,
    out: &mut [u32; 320],
) {
    // Parse sprites for this scanline (builds active list in VRAM)
    let mut vram_mut = vram.to_vec();
    parse_sprites(&mut vram_mut, vpos);

    // Select the active sprite list based on scanline parity
    let base = if vpos & 1 != 0 { 0x8680 } else { 0x8600 };

    // Find last non-zero entry (optimization from MAME)
    let mut max_sprite_index = MAX_SPRITES_PER_LINE - 1;
    while max_sprite_index > 0 && vram_mut[base + max_sprite_index] == 0 {
        max_sprite_index -= 1;
    }
    // Add +1 if not at the end (the zero might be a real sprite)
    if max_sprite_index != MAX_SPRITES_PER_LINE - 1 {
        max_sprite_index += 1;
    }

    let mut y: i32 = 0;
    let mut x: i32 = 0;
    let mut rows: i32 = 0;
    let mut zoom_y: u8 = 0;
    let mut zoom_x: u8 = 0;

    for sprite_index in 0..=max_sprite_index {
        let sprite_number = vram_mut[base + sprite_index] & 0x1FF;

        let y_control = vram_mut[(0x8200 | sprite_number) as usize];
        let zoom_control = vram_mut[(0x8000 | sprite_number) as usize];

        // If chained, go to next X coordinate and get new X zoom
        if y_control & 0x40 != 0 {
            x = (x + zoom_x as i32 + 1) & 0x1FF;
            zoom_x = (zoom_control >> 8) as u8 & 0xF;
        } else {
            // New sprite block
            y = 0x200 - ((y_control >> 7) as i32);
            x = (vram_mut[(0x8400 | sprite_number) as usize] >> 7) as i32;
            zoom_y = zoom_control as u8;
            zoom_x = (zoom_control >> 8) as u8 & 0xF;
            rows = (y_control & 0x3F) as i32;
        }

        // Skip if completely outside screen
        if (0x140..=0x1F0).contains(&x) {
            continue;
        }

        // Double-check Y coordinate
        if !sprite_on_scanline(vpos as i32, y, rows) {
            continue;
        }

        // Calculate sprite line and zoom line
        let sprite_line = (vpos as i32 - y) & 0x1FF;
        let mut zoom_line = sprite_line & 0xFF;
        let mut invert = (sprite_line >> 8) != 0;

        if invert {
            zoom_line ^= 0xFF;
        }

        if rows > 0x20 {
            zoom_line %= (zoom_y as i32 + 1) << 1;
            if zoom_line > zoom_y as i32 {
                zoom_line = ((zoom_y as i32 + 1) << 1) - 1 - zoom_line;
                invert = !invert;
            }
        }

        // Look up sprite Y and tile from zoom table
        let idx = ((zoom_y as usize) << 8) | (zoom_line & 0xFF) as usize;
        let sprite_y_and_tile = zoomy[idx];

        let mut sprite_y = sprite_y_and_tile & 0xF;
        let mut tile = sprite_y_and_tile >> 4;

        if invert {
            sprite_y ^= 0xF;
            tile ^= 0x1F;
        }

        // Get attribute and code from VRAM
        let attr_offs = (sprite_number << 6) | ((tile as u16) << 1);
        let attr = vram_mut[(attr_offs + 1) as usize];
        let mut code: u32 = ((attr as u32) << 12 & 0xF0000) | (vram_mut[attr_offs as usize] as u32);

        // Auto-animation substitution
        if !anim_disabled {
            if attr & 8 != 0 {
                code = (code & !7) | ((anim_counter & 7) as u32);
            } else if attr & 4 != 0 {
                code = (code & !3) | ((anim_counter & 3) as u32);
            }
        }

        // Vertical flip?
        if attr & 2 != 0 {
            sprite_y ^= 0xF;
        }

        let mut zoom_x_table = ZOOM_X[zoom_x as usize];

        // Compute offset in gfx ROM and mask it
        let mut gfx_base =
            (((code << 8) | ((sprite_y as u32) << 4)) & sprite_gfx_mask(c_rom.len())) as i32;

        // Get line pens for this palette bank
        let pen_offset = ((attr >> 8) as usize) << 4;
        let line_pens = &pens[pen_offset..pen_offset + 16];

        let mut x_inc: i32 = 1;
        if attr & 1 != 0 {
            // Horizontal flip
            gfx_base += 0xF;
            x_inc = -1;
        }

        // Draw the line
        if x <= 0x1F0 {
            // No wrap-around
            let mut pixel_addr = x as usize;
            for _ in 0..16 {
                if zoom_x_table & 0x8000 != 0 {
                    if pixel_addr < 320 {
                        draw_pixel(c_rom, gfx_base as u32, &mut out[pixel_addr], line_pens);
                    }
                    pixel_addr += 1;
                }
                zoom_x_table <<= 1;
                if zoom_x_table == 0 {
                    break;
                }
                gfx_base += x_inc;
            }
        } else {
            // Wrap-around
            let x_save = x;
            let mut pixel_addr = 0usize;
            let mut local_x = x;
            for _ in 0..16 {
                if zoom_x_table & 0x8000 != 0 {
                    if local_x >= 0x200 {
                        draw_pixel(c_rom, gfx_base as u32, &mut out[pixel_addr], line_pens);
                        pixel_addr += 1;
                    }
                    local_x += 1;
                }
                zoom_x_table <<= 1;
                if zoom_x_table == 0 {
                    break;
                }
                gfx_base += x_inc;
            }
            x = x_save;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sprite_gfx_mask_basic() {
        assert_eq!(sprite_gfx_mask(0x400000), 0x7FFFFF);
        assert_eq!(sprite_gfx_mask(0x800000), 0xFFFFFF);
        assert_eq!(sprite_gfx_mask(0x20000), 0x3FFFF);
    }

    #[test]
    fn draw_pixel_basic() {
        // romaddr=0: s = ((0 & !0xFF) >> 1) | (((0 & 8) ^ 8) << 3) | ((0 & 0xF0) >> 2)
        //          = 0 | (8 << 3) | 0 = 0x40
        let mut c_rom = [0u8; 1024];
        c_rom[0x40] = 0x01; // plane 0, bit 0 set
        let mut pens = [0u32; 16];
        for (i, p) in pens.iter_mut().enumerate().skip(1) {
            *p = i as u32 * 0x1000000; // red channel
        }

        let mut dst = 0u32;
        draw_pixel(&c_rom, 0, &mut dst, &pens);
        assert_eq!(dst, 0x1000000); // gfx=1 (plane 0 only)
    }

    #[test]
    fn draw_pixel_plane3() {
        // romaddr=8: s = ((8 & !0xFF) >> 1) | (((8 & 8) ^ 8) << 3) | ((8 & 0xF0) >> 2)
        //          = 0 | 0 | 0 = 0
        let mut c_rom = [0u8; 1024];
        c_rom[3] = 0x01; // plane 3, bit 0 set (offset s+3=3)
        let mut pens = [0u32; 16];
        for (i, p) in pens.iter_mut().enumerate().skip(1) {
            *p = i as u32 * 0x1000000;
        }

        let mut dst = 0u32;
        draw_pixel(&c_rom, 8, &mut dst, &pens);
        assert_eq!(dst, 0x8000000); // gfx=8 (plane 3 only)
    }

    #[test]
    fn zoom_full_width() {
        // ZOOM_X[15] = 0xFFFF means all 16 pixels drawn
        assert_eq!(ZOOM_X[15], 0xFFFF);
    }

    #[test]
    fn zoom_single_pixel() {
        // ZOOM_X[0] = 0x0080 means only first pixel drawn
        assert_eq!(ZOOM_X[0], 0x0080);
    }
}
