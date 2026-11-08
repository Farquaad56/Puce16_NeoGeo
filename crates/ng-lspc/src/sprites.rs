// T404 — ng-lspc : analyse des sprites d'une ligne (parse_sprites)
// Port de neogeo_spr.cpp parse_sprites() et sprite_on_scanline()

pub const MAX_SPRITES_PER_SCREEN: usize = 381;
pub const MAX_SPRITES_PER_LINE: usize = 96;

/// Test if a sprite is on the given scanline.
/// spec: docs/spec/04 §sprites (MAME neogeo_spr.cpp l.277-280)
pub fn sprite_on_scanline(scanline: i32, y: i32, rows: i32) -> bool {
    rows == 0 || rows >= 0x20 || ((scanline - y) & 0x1FF) < rows * 0x10
}

/// Parse sprites for the given scanline and build the active sprite list.
/// spec: docs/spec/04 §sprites (MAME neogeo_spr.cpp l.462-509)
pub fn parse_sprites(vram: &mut [u16], scanline: u16) {
    // Select active list based on scanline parity
    let base = if scanline & 1 != 0 { 0x8680 } else { 0x8600 };

    let mut y: i32 = 0;
    let mut rows: i32 = 0;
    let mut count: usize = 0;
    let mut p = base;

    for n in 0..MAX_SPRITES_PER_SCREEN {
        let yc = vram[0x8200 | n];

        // If not chained, get Y position and height
        if yc & 0x40 == 0 {
            y = 0x200 - (yc >> 7) as i32;
            rows = (yc & 0x3F) as i32;
        }

        // Skip sprites with 0 rows
        if rows == 0 {
            continue;
        }

        // Check if sprite is on this scanline
        if !sprite_on_scanline(scanline as i32, y, rows) {
            continue;
        }

        // Add to active list
        vram[p] = n as u16;
        p += 1;
        count += 1;

        // Max sprites per line reached
        if count == MAX_SPRITES_PER_LINE {
            break;
        }
    }

    // Fill rest of sprite list with zeros (including one extra entry)
    let remaining = MAX_SPRITES_PER_LINE - count + 1;
    for i in 0..remaining {
        vram[p + i] = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LSPC_VRAM_SIZE: usize = 0x8800;

    #[test]
    fn sprite_on_scanline_basic() {
        // Sprite at y=100, rows=1 (height 16 lines: 100..115)
        assert!(sprite_on_scanline(100, 100, 1));
        assert!(sprite_on_scanline(115, 100, 1));
        assert!(!sprite_on_scanline(116, 100, 1));
    }

    #[test]
    fn sprite_on_scanline_full_height() {
        // rows=0x20 means full height (always on scanline)
        assert!(sprite_on_scanline(0, 100, 0x20));
        assert!(sprite_on_scanline(500, 100, 0x20));
    }

    #[test]
    fn sprite_on_scanline_zero_rows() {
        // rows=0 is always true (used for chained sprites)
        assert!(sprite_on_scanline(100, 100, 0));
    }

    #[test]
    fn parse_sprites_basic() {
        let mut vram = [0u16; LSPC_VRAM_SIZE];

        // Sprite 0: y=100, rows=1 (not chained)
        // yc = ((0x200 - 100) << 7) | 1 = (300 << 7) | 1
        vram[0x8200] = ((0x200 - 100) << 7) as u16 | 1;

        parse_sprites(&mut vram, 100);

        // Should be in the even list (scanline 100 is even)
        assert_eq!(vram[0x8600], 0); // sprite 0
        assert_eq!(vram[0x8601], 0); // terminator
    }

    #[test]
    fn parse_sprites_odd_scanline() {
        let mut vram = [0u16; LSPC_VRAM_SIZE];

        // Sprite 0: y=100, rows=1
        vram[0x8200] = ((0x200 - 100) << 7) as u16 | 1;

        parse_sprites(&mut vram, 101);

        // Should be in the odd list (scanline 101 is odd)
        assert_eq!(vram[0x8680], 0); // sprite 0
        assert_eq!(vram[0x8681], 0); // terminator
    }

    #[test]
    fn parse_sprites_chained() {
        let mut vram = [0u16; LSPC_VRAM_SIZE];

        // Sprite 0: y=100, rows=1 (not chained)
        vram[0x8200] = ((0x200 - 100) << 7) as u16 | 1;

        // Sprite 1: chained (bit 6 set), inherits y/rows from sprite 0
        vram[0x8201] = 0x40; // only bit 6 set

        parse_sprites(&mut vram, 100);

        assert_eq!(vram[0x8600], 0); // sprite 0
        assert_eq!(vram[0x8601], 1); // sprite 1 (chained)
        assert_eq!(vram[0x8602], 0); // terminator
    }

    #[test]
    fn parse_sprites_zero_rows_ignored() {
        let mut vram = [0u16; LSPC_VRAM_SIZE];

        // Sprite 0: y=100, rows=0 (should be ignored)
        vram[0x8200] = ((0x200 - 100) << 7) as u16;

        parse_sprites(&mut vram, 100);

        // No sprites in list
        assert_eq!(vram[0x8600], 0); // terminator only
    }

    #[test]
    fn parse_sprites_max_per_line() {
        let mut vram = [0u16; LSPC_VRAM_SIZE];

        // Create 97 sprites all active on scanline 100
        for n in 0..97 {
            vram[0x8200 | n] = ((0x200 - 100) << 7) as u16 | 1;
        }

        parse_sprites(&mut vram, 100);

        // Should have exactly 96 sprites in list
        for i in 0..96 {
            assert_eq!(vram[0x8600 + i], i as u16);
        }
        // Entry 97 should not be there (terminator at position 96)
        assert_eq!(vram[0x8600 + 96], 0);
    }

    #[test]
    fn parse_sprites_not_on_scanline() {
        let mut vram = [0u16; LSPC_VRAM_SIZE];

        // Sprite 0: y=200, rows=1 (not on scanline 100)
        vram[0x8200] = ((0x200 - 200) << 7) as u16 | 1;

        parse_sprites(&mut vram, 100);

        // No sprites in list
        assert_eq!(vram[0x8600], 0); // terminator only
    }
}
