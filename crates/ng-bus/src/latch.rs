//! 74HC259 system latch (Neo Geo MVS).
//!
//! Writes to $3A0000-$3BFFFF are decoded by the address bits, not the data.
//! Bit A1-A3 select output Q0-Q7; bit A4 is the value written.
//! The actual data byte written is ignored.

/// System latch with 8 outputs (Q0-Q7).
#[derive(Default)]
pub struct SystemLatch {
    pub q: [bool; 8],
}

impl SystemLatch {
    /// Write to the latch via address decoding.
    /// `addr` is the full 24-bit address in the $3A0000-$3BFFFF range.
    pub fn write(&mut self, addr: u32) {
        let n = ((addr >> 1) & 7) as usize; // A1-A3 select output
        let v = (addr >> 4) & 1 == 1; // A4 is the value
        self.q[n] = v;
    }

    /// Q0: Shadow mode (darken video).
    pub fn shadow(&self) -> bool {
        self.q[0]
    }

    /// Q1: Use cartridge vectors (REG_SWPROM=1) vs BIOS vectors.
    pub fn use_cart_vectors(&self) -> bool {
        self.q[1]
    }

    /// Q5: CRTFIX - use cartridge S ROM + M1 (1) vs BIOS sfix/sm1 (0).
    pub fn use_cart_fix_audio(&self) -> bool {
        self.q[5]
    }

    /// Q6: SRAM unlocked.
    pub fn sram_unlocked(&self) -> bool {
        self.q[6]
    }

    /// Q7: Palette bank (0 or 1).
    pub fn palette_bank(&self) -> u8 {
        if self.q[7] {
            1
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latch_reset_all_low() {
        let latch = SystemLatch::default();
        for i in 0..8 {
            assert!(!latch.q[i], "Q{} should be low on reset", i);
        }
    }

    #[test]
    fn latch_q0_shadow() {
        let mut latch = SystemLatch::default();
        // bit4=0 -> false
        latch.write(0x3A0001);
        assert!(!latch.shadow());
        // bit4=1 -> true
        latch.write(0x3A0011);
        assert!(latch.shadow());
    }

    #[test]
    fn latch_q1_vectors() {
        let mut latch = SystemLatch::default();
        latch.write(0x3A0003);
        assert!(!latch.use_cart_vectors());
        latch.write(0x3A0013);
        assert!(latch.use_cart_vectors());
    }

    #[test]
    fn latch_q5_crtfix() {
        let mut latch = SystemLatch::default();
        latch.write(0x3A000B);
        assert!(!latch.use_cart_fix_audio());
        latch.write(0x3A001B);
        assert!(latch.use_cart_fix_audio());
    }

    #[test]
    fn latch_q6_sram() {
        let mut latch = SystemLatch::default();
        latch.write(0x3A000D);
        assert!(!latch.sram_unlocked());
        latch.write(0x3A001D);
        assert!(latch.sram_unlocked());
    }

    #[test]
    fn latch_q7_palette() {
        let mut latch = SystemLatch::default();
        latch.write(0x3A000F);
        assert_eq!(latch.palette_bank(), 0);
        latch.write(0x3A001F);
        assert_eq!(latch.palette_bank(), 1);
    }

    #[test]
    fn latch_all_outputs() {
        let mut latch = SystemLatch::default();
        // Set all outputs high via bit4=1 addresses
        for i in 0..8u32 {
            latch.write(0x3A0000 | (i << 1) | 0x10);
        }
        for i in 0..8 {
            assert!(latch.q[i], "Q{} should be high", i);
        }

        // Set all outputs low via bit4=0 addresses
        let mut latch2 = SystemLatch::default();
        for i in 0..8u32 {
            latch2.write(0x3A0000 | (i << 1) | 0x01);
        }
        for i in 0..8 {
            assert!(!latch2.q[i], "Q{} should be low", i);
        }
    }

    #[test]
    fn latch_mirror_range() {
        // Writes to $3BFFFF (top of mirror range) should work the same way
        let mut latch = SystemLatch::default();
        latch.write(0x3BFFFF); // Q7, bit4=1 -> high
        assert!(latch.q[7]);
    }
}
