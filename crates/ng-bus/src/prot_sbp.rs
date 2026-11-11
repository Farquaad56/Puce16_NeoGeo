//! Super Bubble Pop (sbp, bootleg) : la « protection » est une zone de ROM basse brouillée.
//! Port exact de MAME `sbp.cpp` (`protection_r` / `protection_w`).
//!
//! MAME installe le gestionnaire sur `$000200-$001FFF` : une lecture renvoie le mot de P à la
//! même adresse avec les quartets permutés `bitswap<16>(orig, 11,10,9,8,15,14,13,12,3,2,1,0,7,6,5,4)`,
//! sauf à `$000D5E` (mot brut). Les écritures sont ignorées (MAME ne journalise que les inconnues).

use crate::device::CartPort;

/// Début / fin (exclue) de la fenêtre gérée par la puce.
pub const SBP_START: u32 = 0x200;
pub const SBP_END: u32 = 0x2000;

/// Permutation des quartets : (b11-8, b15-12, b3-0, b7-4).
pub fn sbp_scramble(o: u16) -> u16 {
    ((o >> 8) & 0xF) << 12 | ((o >> 12) & 0xF) << 8 | (o & 0xF) << 4 | ((o >> 4) & 0xF)
}

#[derive(Default)]
pub struct Sbp {
    accesses: u64,
    /// Écritures inconnues (hors `$1080` = 0x4E75/0xFFFF) ; sert aux tests.
    pub unknown_writes: u64,
}

impl Sbp {
    pub fn new() -> Self {
        Self::default()
    }
}

impl CartPort for Sbp {
    fn read(&mut self, a: u32, p: &[u8], _wram: &[u8]) -> Option<u16> {
        let a = a & !1;
        if !(SBP_START..SBP_END).contains(&a) {
            return None;
        }
        let i = a as usize;
        if i + 1 >= p.len() {
            return None;
        }
        self.accesses += 1;
        let orig = ((p[i] as u16) << 8) | p[i + 1] as u16;
        Some(if a == 0xD5E { orig } else { sbp_scramble(orig) })
    }

    fn write(&mut self, a: u32, d: u16, _u: bool, _l: bool, _p: &[u8], _w: &[u8]) -> bool {
        let a = a & !1;
        if !(SBP_START..SBP_END).contains(&a) {
            return false;
        }
        self.accesses += 1;
        if !(a == 0x1080 && (d == 0x4E75 || d == 0xFFFF)) {
            self.unknown_writes += 1;
        }
        true
    }

    fn access_count(&self) -> u64 {
        self.accesses
    }
}

#[cfg(test)]
mod prot_sbp_tests {
    use super::*;

    #[test]
    fn scramble_example_from_card() {
        assert_eq!(sbp_scramble(0xABCD), 0xBADC);
        assert_eq!(sbp_scramble(0x0000), 0x0000);
        assert_eq!(sbp_scramble(0xFFFF), 0xFFFF);
        assert_eq!(sbp_scramble(0x1234), 0x2143);
    }

    #[test]
    fn read_uses_word_at_same_address() {
        let mut p = vec![0u8; 0x4000];
        // mot[0x200/2 + 3] = adresse 0x206
        p[0x206] = 0xAB;
        p[0x207] = 0xCD;
        let mut s = Sbp::new();
        // offset 3 de la fenêtre = adresse 0x200 + 3*2
        assert_eq!(s.read(0x206, &p, &[]), Some(0xBADC));
    }

    #[test]
    fn d5e_is_raw() {
        let mut p = vec![0u8; 0x4000];
        p[0xD5E] = 0xAB;
        p[0xD5F] = 0xCD;
        let mut s = Sbp::new();
        assert_eq!(s.read(0xD5E, &p, &[]), Some(0xABCD));
    }

    #[test]
    fn outside_window_is_none() {
        let p = vec![0u8; 0x4000];
        let mut s = Sbp::new();
        assert_eq!(s.read(0x1FE, &p, &[]), None);
        assert_eq!(s.read(0x2000, &p, &[]), None);
        assert_eq!(s.read(0x200000, &p, &[]), None);
        assert!(!s.write(0x2000, 0, true, true, &p, &[]));
    }

    #[test]
    fn writes_are_consumed_and_counted() {
        let p = vec![0u8; 0x4000];
        let mut s = Sbp::new();
        assert!(s.write(0x1080, 0x4E75, true, true, &p, &[]));
        assert!(s.write(0x1080, 0xFFFF, true, true, &p, &[]));
        assert_eq!(s.unknown_writes, 0);
        assert!(s.write(0x1080, 0x1234, true, true, &p, &[]));
        assert!(s.write(0x300, 0x4E75, true, true, &p, &[]));
        assert_eq!(s.unknown_writes, 2);
    }
}
