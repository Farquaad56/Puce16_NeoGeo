//! SMA Metal Slug 3 protection chip emulation ($2FFFE4 bank select, $2FE446 read).
//! Port of MAME prot_sma.cpp (lines 250-275: mslug3_bank_base; lines 326-330: prot_9a37_r).

use crate::device::CartPort;
use ng_common::bitswap::bitswap;

/// Bank offset table for Metal Slug 3 (from MAME prot_sma.cpp lines 252-267).
/// Entries beyond index 44 are not used by the game.
const BANKOFFSET: [u32; 64] = [
    0x000000, 0x020000, 0x040000, 0x060000, // 00
    0x070000, 0x090000, 0x0b0000, 0x0d0000, // 04
    0x0e0000, 0x0f0000, 0x120000, 0x130000, // 08
    0x140000, 0x150000, 0x180000, 0x190000, // 12
    0x1a0000, 0x1b0000, 0x1e0000, 0x1f0000, // 16
    0x200000, 0x210000, 0x240000, 0x250000, // 20
    0x260000, 0x270000, 0x2a0000, 0x2b0000, // 24
    0x2c0000, 0x2d0000, 0x300000, 0x310000, // 28
    0x320000, 0x330000, 0x360000, 0x370000, // 32
    0x380000, 0x390000, 0x3c0000, 0x3d0000, // 36
    0x400000, 0x410000, 0x440000, 0x450000, // 40
    0x460000, 0x470000, 0x4a0000, 0x4b0000, // 44
    0x4c0000, // rest not used?
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

/// Compute the bank base address for Metal Slug 3.
/// Port of MAME prot_sma.cpp mslug3_bank_base (lines 250-274).
pub fn mslug3_bank_base(sel: u16) -> u32 {
    // Unscramble bank number using bitswap with the specified bit permutation
    let data = bitswap::<6>(sel as u32, [11, 12, 6, 1, 3, 15]);
    0x100000 + BANKOFFSET[data as usize]
}

/// Metal Slug 3 protection chip state.
#[derive(Default)]
pub struct Sma3 {
    bank: Option<u32>,
}

impl Sma3 {
    pub fn new() -> Self {
        Self { bank: None }
    }
}

impl CartPort for Sma3 {
    fn read(&mut self, a: u32, _p: &[u8], _wram: &[u8]) -> Option<u16> {
        // Protection read at $2FE446 returns constant 0x9A37 (MAME prot_sma.cpp line 328)
        if a == 0x2FE446 {
            Some(0x9A37)
        } else {
            None
        }
    }

    fn write(
        &mut self,
        a: u32,
        d: u16,
        _upper: bool,
        _lower: bool,
        _p: &[u8],
        _wram: &[u8],
    ) -> bool {
        // Bank select at $2FFFE4 (MAME neogeo.cpp line 1465)
        if a == 0x2FFFE4 {
            self.bank = Some(mslug3_bank_base(d));
            true
        } else {
            false
        }
    }

    fn take_bank(&mut self) -> Option<u32> {
        self.bank.take()
    }

    fn reset(&mut self) {
        self.bank = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_protection_returns_9a37() {
        let mut prot = Sma3::new();
        assert_eq!(prot.read(0x2FE446, &[], &[]), Some(0x9A37));
    }

    #[test]
    fn read_other_address_returns_none() {
        let mut prot = Sma3::new();
        assert_eq!(prot.read(0x2FE448, &[], &[]), None);
    }

    #[test]
    fn bitswap_sel_0x8000_gives_bank_1() {
        // sel=0x8000 (bit 15 set) -> last in list [11,12,6,1,3,15] -> bit 0 of data
        // data = 1 -> base = 0x100000 + BANKOFFSET[1] = 0x100000 + 0x020000 = 0x120000
        assert_eq!(mslug3_bank_base(0x8000), 0x120000);
    }

    #[test]
    fn bitswap_sel_0x0800_gives_bank_32() {
        // sel=0x0800 (bit 11 set) -> first in list [11,12,6,1,3,15] -> bit 5 of data
        // data = 32 -> base = 0x100000 + BANKOFFSET[32] = 0x100000 + 0x320000 = 0x420000
        assert_eq!(mslug3_bank_base(0x0800), 0x420000);
    }

    #[test]
    fn write_bank_select_and_take() {
        let mut prot = Sma3::new();

        // Write bank select at $2FFFE4 with sel=0x8000
        assert!(prot.write(0x2FFFE4, 0x8000, true, true, &[], &[]));

        // take_bank should return the computed base address
        assert_eq!(prot.take_bank(), Some(0x120000));

        // Second take returns None (already taken)
        assert_eq!(prot.take_bank(), None);
    }

    #[test]
    fn write_other_address_not_consumed() {
        let mut prot = Sma3::new();
        assert!(!prot.write(0x2FFFE0, 0x1234, true, true, &[], &[]));
    }

    #[test]
    fn reset_clears_bank() {
        let mut prot = Sma3::new();
        prot.write(0x2FFFE4, 0x8000, true, true, &[], &[]);
        assert!(prot.bank.is_some());

        prot.reset();
        assert_eq!(prot.take_bank(), None);
    }
}
