//! Palette RAM with DAC color conversion (2 banks × 0x1000 entries).
//! Bus zone: $400000-$7FFFFF, mirrored every 0x2000.

use crate::dac_table::DAC;

/// Convert 8-bit RGB components to u32 in 0x00RRGGBB format.
fn rgb(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

pub struct Palette {
    pub ram: Box<[u16; 0x2000]>,
    /// Pens for both banks + shadow variants.
    /// Layout: [bank0_normal(0x1000)][bank1_normal(0x1000)][bank0_shadow(0x1000)][bank1_shadow(0x1000)]
    pub pens: Box<[u32; 0x4000]>,
    pub bank: u8,
    pub shadow: bool,
}

impl Palette {
    pub fn new() -> Self {
        let ram = Box::new([0u16; 0x2000]);
        let pens = Box::new([0u32; 0x4000]);
        // MAME: pen 0 is black at reset, no recalculation needed
        Self {
            ram,
            pens,
            bank: 0,
            shadow: false,
        }
    }

    pub fn read16(&self, off: usize) -> u16 {
        let i = (self.bank as usize) * 0x1000 + off;
        self.ram[i]
    }

    pub fn write16(&mut self, off: usize, data: u16) {
        let i = (self.bank as usize) * 0x1000 + off;
        self.ram[i] = data;

        // Extract color components per MAME neogeo_v.cpp paletteram_w()
        let dark = ((data >> 15) & 1) as usize;
        let r = (((data >> 14) & 1) | ((data >> 7) & 0x1E)) as usize;
        let g = (((data >> 13) & 1) | ((data >> 3) & 0x1E)) as usize;
        let b = (((data >> 12) & 1) | ((data << 1) & 0x1E)) as usize;

        // Normal pen: DAC column `dark` (0=normal, 1=dark)
        self.pens[i] = rgb(DAC[r][dark], DAC[g][dark], DAC[b][dark]);

        // Shadow pen at offset + 0x2000: DAC column `dark+2` (2=shadow, 3=dark+shadow)
        let shadow_col = dark + 2;
        self.pens[i + 0x2000] = rgb(DAC[r][shadow_col], DAC[g][shadow_col], DAC[b][shadow_col]);
    }

    /// Base of the pen array for current bank/shadow selection.
    pub fn pens_base(&self) -> &[u32] {
        let base = (self.bank as usize) * 0x1000 + if self.shadow { 0x2000 } else { 0 };
        &self.pens[base..base + 0x1000]
    }

    /// Backdrop color is pen 0xFFF of the current bank/shadow.
    pub fn backdrop(&self) -> u32 {
        self.pens_base()[0xFFF]
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_black_gives_black() {
        let mut p = Palette::new();
        p.write16(0, 0x0000);
        assert_eq!(p.pens[0], 0);
    }

    #[test]
    fn write_white_gives_max_dac() {
        let mut p = Palette::new();
        // 0x7FFF: dark=0, r=g=b=31 (max)
        p.write16(0, 0x7FFF);
        let expected = rgb(DAC[31][0], DAC[31][0], DAC[31][0]);
        assert_eq!(p.pens[0], expected);
    }

    #[test]
    fn dark_bit_sets_column() {
        let mut p = Palette::new();
        // 0x8000: dark=1, r=g=b=0 -> DAC[0][1]=0 for all channels
        p.write16(1, 0x8000);
        assert_eq!(p.pens[1], rgb(DAC[0][1], DAC[0][1], DAC[0][1]));

        // 0x8FFF: dark=1, r=g=b=30 -> all channels use DAC[30][1]
        p.write16(2, 0x8FFF);
        let expected = rgb(DAC[30][1], DAC[30][1], DAC[30][1]);
        assert_eq!(p.pens[2], expected);
    }

    #[test]
    fn shadow_pen_uses_column_plus_two() {
        let mut p = Palette::new();
        // Write to entry 3, check normal and shadow pens
        p.write16(3, 0x7FFF);
        // Normal: DAC[31][0]
        assert_eq!(p.pens[3], rgb(DAC[31][0], DAC[31][0], DAC[31][0]));
        // Shadow at +0x2000: DAC[31][2]
        assert_eq!(p.pens[3 + 0x2000], rgb(DAC[31][2], DAC[31][2], DAC[31][2]));

        // Dark+shadow: dark=1 -> column 3
        p.write16(4, 0xFFFF);
        assert_eq!(p.pens[4 + 0x2000], rgb(DAC[31][3], DAC[31][3], DAC[31][3]));
    }

    #[test]
    fn bank_isolation() {
        let mut p = Palette::new();
        // Write to bank 1
        p.bank = 1;
        p.write16(5, 0x7FFF);
        assert_eq!(p.ram[0x1005], 0x7FFF);

        // Bank 0 doesn't see it
        p.bank = 0;
        assert_eq!(p.read16(5), 0);

        // Bank 1 can read it back
        p.bank = 1;
        assert_eq!(p.read16(5), 0x7FFF);
    }

    #[test]
    fn backdrop_color() {
        let mut p = Palette::new();
        // Write to pen 0xFFF in bank 0
        p.write16(0xFFF, 0x7FFF);
        assert_eq!(p.backdrop(), p.pens[0xFFF]);

        // Shadow mode uses shadow pens
        p.shadow = true;
        assert_eq!(p.backdrop(), p.pens[0x2FFF]);
    }

    #[test]
    fn reset_all_black() {
        let p = Palette::new();
        for i in 0..0x4000 {
            assert_eq!(p.pens[i], 0);
        }
    }
}
