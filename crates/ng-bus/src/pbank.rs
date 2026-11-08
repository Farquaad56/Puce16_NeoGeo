//! P-ROM bank switching for Neo Geo MVS/AES.
//!
//! Implements the write_banksel logic from MAME (neogeo.cpp) that selects
//! which 1 MiB bank of the cart P-ROM is mapped into the $200000-$2FFFFF window.

/// Compute the initial bank base offset for a given P-ROM size.
/// If the ROM is larger than 1 MiB, start at bank 1 (offset 0x100000).
pub fn initial_pbank_base(p_len: usize) -> u32 {
    if p_len > 0x100000 {
        0x100000
    } else {
        0
    }
}

/// Handle a bank select write to $2FFFF0-$2FFFFF.
///
/// If the P-ROM is <= 1 MiB, no banks exist and nothing changes.
/// Otherwise, select bank (data & 7), clamping to valid range.
pub fn banksel(base: &mut u32, p_len: usize, data: u16) {
    if p_len <= 0x100000 {
        return; // No banks available
    }

    let bank = (data & 7) as usize;
    if (bank + 1) * 0x100000 >= p_len {
        // Bank would be empty or out of range, fall back to bank 0
        *base = 0x100000;
    } else {
        *base = ((bank + 1) * 0x100000) as u32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_base_small_rom() {
        // p_len <= 0x100000: base should be 0
        assert_eq!(initial_pbank_base(0x80000), 0);
        assert_eq!(initial_pbank_base(0x100000), 0);
    }

    #[test]
    fn initial_base_large_rom() {
        // p_len > 0x100000: base should be 0x100000 (bank 1)
        assert_eq!(initial_pbank_base(0x200000), 0x100000);
        assert_eq!(initial_pbank_base(0x500000), 0x100000);
    }

    #[test]
    fn banksel_no_banks() {
        // p_len <= 0x100000: no change regardless of data
        let mut base = 0;
        banksel(&mut base, 0x100000, 3);
        assert_eq!(base, 0);

        let mut base2 = 0x100000;
        banksel(&mut base2, 0x80000, 5);
        assert_eq!(base2, 0x100000); // Unchanged
    }

    #[test]
    fn banksel_mslugx() {
        // p_len = 0x500000 (Metal Slug X / Metal Slug 4)
        let mut base = initial_pbank_base(0x500000);
        assert_eq!(base, 0x100000);

        // data=2 -> bank 2 -> offset 0x300000
        banksel(&mut base, 0x500000, 2);
        assert_eq!(base, 0x300000);

        // data=3 -> bank 3 -> offset 0x400000 (valid: < 0x500000)
        banksel(&mut base, 0x500000, 3);
        assert_eq!(base, 0x400000);

        // data=4 -> bank 4 -> offset would be 0x500000 >= len -> clamp to bank 0 (0x100000)
        banksel(&mut base, 0x500000, 4);
        assert_eq!(base, 0x100000);
    }

    #[test]
    fn banksel_mslug() {
        // p_len = 0x200000 (Metal Slug 1)
        let mut base = initial_pbank_base(0x200000);
        assert_eq!(base, 0x100000);

        // data=1 -> bank 1 -> offset would be 0x200000 >= len -> clamp to bank 0 (0x100000)
        banksel(&mut base, 0x200000, 1);
        assert_eq!(base, 0x100000);

        // data=0 -> bank 0 -> offset 0x100000 (valid)
        banksel(&mut base, 0x200000, 0);
        assert_eq!(base, 0x100000);
    }

    #[test]
    fn banksel_data_masking() {
        // Only lower 3 bits of data are used
        let mut base = initial_pbank_base(0x500000);
        banksel(&mut base, 0x500000, 0xFF); // 0xFF & 7 = 7 -> bank 7 (invalid for 0x500000)
        assert_eq!(base, 0x100000); // Clamped to bank 0

        banksel(&mut base, 0x500000, 0x82); // 0x82 & 7 = 2 -> bank 2
        assert_eq!(base, 0x300000);
    }
}
