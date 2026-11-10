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
    let data = bitswap::<6>(sel as u32, [9, 3, 6, 15, 12, 14]);
    0x100000 + BANKOFFSET[data as usize]
}

// ---- T1007 : kof99 / garou / kof2000 (port exact de prot_sma.cpp) ----
const KOF99_BANKOFFSET: [u32; 64] = [
    0x000000, 0x100000, 0x200000, 0x300000, 0x3CC000, 0x4CC000, 0x3F2000, 0x4F2000, 0x407800,
    0x507800, 0x40D000, 0x50D000, 0x417800, 0x517800, 0x420800, 0x520800, 0x424800, 0x524800,
    0x429000, 0x529000, 0x42E800, 0x52E800, 0x431800, 0x531800, 0x54D000, 0x551000, 0x567000,
    0x592800, 0x588800, 0x581800, 0x599800, 0x594800, 0x598000, 0x000000, 0x000000, 0x000000,
    0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000,
    0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000,
    0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000,
    0x000000,
];

/// MAME prot_sma.cpp kof99_bank_base : bitswap<6>(sel, 5, 12, 10, 8, 6, 14).
pub fn kof99_bank_base(sel: u16) -> u32 {
    let data = bitswap::<6>(sel as u32, [5, 12, 10, 8, 6, 14]);
    0x100000 + KOF99_BANKOFFSET[data as usize]
}

const GAROU_BANKOFFSET: [u32; 64] = [
    0x000000, 0x100000, 0x200000, 0x300000, 0x280000, 0x380000, 0x2D0000, 0x3D0000, 0x2F0000,
    0x3F0000, 0x400000, 0x500000, 0x420000, 0x520000, 0x440000, 0x540000, 0x498000, 0x598000,
    0x4A0000, 0x5A0000, 0x4A8000, 0x5A8000, 0x4B0000, 0x5B0000, 0x4B8000, 0x5B8000, 0x4C0000,
    0x5C0000, 0x4C8000, 0x5C8000, 0x4D0000, 0x5D0000, 0x458000, 0x558000, 0x460000, 0x560000,
    0x468000, 0x568000, 0x470000, 0x570000, 0x478000, 0x578000, 0x480000, 0x580000, 0x488000,
    0x588000, 0x490000, 0x590000, 0x5D0000, 0x5D8000, 0x5E0000, 0x5E8000, 0x5F0000, 0x5F8000,
    0x600000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000,
    0x000000,
];

/// MAME prot_sma.cpp garou_bank_base : bitswap<6>(sel, 12, 14, 6, 7, 9, 5).
pub fn garou_bank_base(sel: u16) -> u32 {
    let data = bitswap::<6>(sel as u32, [12, 14, 6, 7, 9, 5]);
    0x100000 + GAROU_BANKOFFSET[data as usize]
}

const KOF2000_BANKOFFSET: [u32; 64] = [
    0x000000, 0x100000, 0x200000, 0x300000, 0x3F7800, 0x4F7800, 0x3FF800, 0x4FF800, 0x407800,
    0x507800, 0x40F800, 0x50F800, 0x416800, 0x516800, 0x41D800, 0x51D800, 0x424000, 0x524000,
    0x523800, 0x623800, 0x526000, 0x626000, 0x528000, 0x628000, 0x52A000, 0x62A000, 0x52B800,
    0x62B800, 0x52D000, 0x62D000, 0x52E800, 0x62E800, 0x618000, 0x619000, 0x61A000, 0x61A800,
    0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000,
    0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000,
    0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000,
    0x000000,
];

/// MAME prot_sma.cpp kof2000_bank_base : bitswap<6>(sel, 5, 10, 3, 7, 14, 15).
pub fn kof2000_bank_base(sel: u16) -> u32 {
    let data = bitswap::<6>(sel as u32, [5, 10, 3, 7, 14, 15]);
    0x100000 + KOF2000_BANKOFFSET[data as usize]
}

/// Metal Slug 3 protection chip state.
#[derive(Default)]
pub struct Sma3 {
    bank: Option<u32>,
    bank_changes: u64,
}

impl Sma3 {
    pub fn new() -> Self {
        Self {
            bank: None,
            bank_changes: 0,
        }
    }
}

impl CartPort for Sma3 {
    fn bank_change_count(&self) -> u64 {
        self.bank_changes
    }

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
            self.bank_changes += 1;
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

/// Variante de puce SMA pour les jeux à générateur pseudo-aléatoire (kof99, garou, kof2000).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SmaKind {
    Kof99,
    Garou,
    Kof2000,
}

impl SmaKind {
    /// Adresse du registre de banque (neogeo.cpp, install_handlers).
    fn bank_addr(self) -> u32 {
        match self {
            SmaKind::Kof99 => 0x2FFFF0,
            SmaKind::Garou => 0x2FFFC0,
            SmaKind::Kof2000 => 0x2FFFEC,
        }
    }

    /// Adresses de lecture du générateur pseudo-aléatoire (`addon_r`).
    fn rng_addrs(self) -> [u32; 2] {
        match self {
            SmaKind::Kof99 => [0x2FFFF8, 0x2FFFFA],
            SmaKind::Garou => [0x2FFFCC, 0x2FFFF0],
            SmaKind::Kof2000 => [0x2FFFD8, 0x2FFFDA],
        }
    }

    fn bank_base(self, sel: u16) -> u32 {
        match self {
            SmaKind::Kof99 => kof99_bank_base(sel),
            SmaKind::Garou => garou_bank_base(sel),
            SmaKind::Kof2000 => kof2000_bank_base(sel),
        }
    }
}

const SMA_RNG_SEED: u16 = 0x2345;

/// Puce SMA de kof99 / garou / kof2000 : banque, lecture 0x9A37 en $2FE446 et générateur
/// pseudo-aléatoire (MAME `random_r` : valeur courante renvoyée, puis registre à décalage).
pub struct SmaChip {
    kind: SmaKind,
    rng: u16,
    bank: Option<u32>,
    bank_changes: u64,
}

impl SmaChip {
    pub fn new(kind: SmaKind) -> Self {
        Self {
            kind,
            rng: SMA_RNG_SEED,
            bank: None,
            bank_changes: 0,
        }
    }

    /// MAME sma_prot_device::random_r.
    fn random(&mut self) -> u16 {
        let r = self.rng;
        let newbit = ((r >> 2)
            ^ (r >> 3)
            ^ (r >> 5)
            ^ (r >> 6)
            ^ (r >> 7)
            ^ (r >> 11)
            ^ (r >> 12)
            ^ (r >> 15))
            & 1;
        self.rng = (r << 1) | newbit;
        r
    }
}

impl CartPort for SmaChip {
    fn bank_change_count(&self) -> u64 {
        self.bank_changes
    }

    fn read(&mut self, a: u32, _p: &[u8], _wram: &[u8]) -> Option<u16> {
        let a = a & !1;
        if a == 0x2FE446 {
            Some(0x9A37)
        } else if self.kind.rng_addrs().contains(&a) {
            Some(self.random())
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
        if a & !1 == self.kind.bank_addr() {
            self.bank = Some(self.kind.bank_base(d));
            self.bank_changes += 1;
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
        self.rng = SMA_RNG_SEED;
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
    fn bitswap_sel_0x4000_gives_bank_1() {
        // MAME : bitswap<6>(sel, 9, 3, 6, 15, 12, 14) ; bit 14 = poids faible de data
        // data = 1 -> base = 0x100000 + BANKOFFSET[1] = 0x120000
        assert_eq!(mslug3_bank_base(0x4000), 0x120000);
        // bit 15 -> data = 4 -> 0x100000 + 0x070000
        assert_eq!(mslug3_bank_base(0x8000), 0x170000);
    }

    #[test]
    fn bitswap_sel_0x0200_gives_bank_32() {
        // bit 9 = poids fort de data (32) -> 0x100000 + BANKOFFSET[32] = 0x420000
        assert_eq!(mslug3_bank_base(0x0200), 0x420000);
    }

    #[test]
    fn write_bank_select_and_take() {
        let mut prot = Sma3::new();

        // Write bank select at $2FFFE4 with sel=0x4000
        assert!(prot.write(0x2FFFE4, 0x4000, true, true, &[], &[]));

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

    // ---- T1007 : valeurs recalculées à la main depuis prot_sma.cpp ----
    #[test]
    fn kof99_bank_values() {
        // bitswap<6>(sel, 5,12,10,8,6,14) : bit 14 -> data bit 0, bit 12 -> bit 4, bit 5 -> bit 5.
        assert_eq!(kof99_bank_base(0x4000), 0x200000); // data 1  -> 0x100000 + 0x100000
        assert_eq!(kof99_bank_base(0x1000), 0x524800); // data 16 -> 0x100000 + 0x424800
        assert_eq!(kof99_bank_base(0x0020), 0x698000); // data 32 -> 0x100000 + 0x598000
    }

    #[test]
    fn garou_bank_values() {
        // bitswap<6>(sel, 12,14,6,7,9,5) : bit 5 -> data bit 0, bit 14 -> bit 4, bit 12 -> bit 5.
        assert_eq!(garou_bank_base(0x0020), 0x200000); // data 1  -> 0x100000 + 0x100000
        assert_eq!(garou_bank_base(0x4000), 0x598000); // data 16 -> 0x100000 + 0x498000
        assert_eq!(garou_bank_base(0x1000), 0x558000); // data 32 -> 0x100000 + 0x458000
    }

    #[test]
    fn kof2000_bank_values() {
        // bitswap<6>(sel, 5,10,3,7,14,15) : bit 15 -> data bit 0, bit 14 -> bit 1, bit 5 -> bit 5.
        assert_eq!(kof2000_bank_base(0x8000), 0x200000); // data 1  -> 0x100000 + 0x100000
        assert_eq!(kof2000_bank_base(0x4000), 0x300000); // data 2  -> 0x100000 + 0x200000
        assert_eq!(kof2000_bank_base(0x0020), 0x718000); // data 32 -> 0x100000 + 0x618000
    }

    #[test]
    fn bank_bases_stay_inside_the_p_rom() {
        for sel in 0..=0xFFFFu16 {
            for base in [
                kof99_bank_base(sel),
                garou_bank_base(sel),
                kof2000_bank_base(sel),
            ] {
                assert!(base < 0x900000, "sel {sel:#06X} -> {base:#X}");
            }
        }
    }

    #[test]
    fn sma_rng_sequence_matches_mame() {
        // Graine 0x2345 ; bits 2,3,5,6,7,11,12,15 : 0x2345 -> 0x468A -> 0x8D14.
        let mut c = SmaChip::new(SmaKind::Kof99);
        assert_eq!(c.read(0x2FFFF8, &[], &[]), Some(0x2345));
        assert_eq!(c.read(0x2FFFFA, &[], &[]), Some(0x468A));
        assert_eq!(c.read(0x2FFFF8, &[], &[]), Some(0x8D14));
        c.reset();
        assert_eq!(c.read(0x2FFFFA, &[], &[]), Some(0x2345));
    }

    #[test]
    fn sma_chip_addresses_per_game() {
        for (kind, bank, rng) in [
            (SmaKind::Kof99, 0x2FFFF0, [0x2FFFF8, 0x2FFFFA]),
            (SmaKind::Garou, 0x2FFFC0, [0x2FFFCC, 0x2FFFF0]),
            (SmaKind::Kof2000, 0x2FFFEC, [0x2FFFD8, 0x2FFFDA]),
        ] {
            let mut c = SmaChip::new(kind);
            assert_eq!(c.read(0x2FE446, &[], &[]), Some(0x9A37));
            for a in rng {
                assert!(c.read(a, &[], &[]).is_some(), "{kind:?} {a:#X}");
            }
            assert_eq!(c.read(0x2FFFE0, &[], &[]), None);
            assert!(c.write(bank, 0x4000, true, true, &[], &[]));
            assert!(c.take_bank().is_some());
            assert!(!c.write(0x2FFFE0, 0, true, true, &[], &[]));
        }
    }
}
