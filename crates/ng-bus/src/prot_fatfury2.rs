//! Puce PRO-CT0 (Alpha denshi ALPHA-8921) de Fatal Fury 2 et Super Sidekicks.
//! Port exact de MAME `alpha_8921.cpp` + `prot_fatfury2.cpp`.
//!
//! Tout le fenêtre `$200000-$2FFFFF` est routée vers la puce : une écriture charge/décale un
//! registre à décalage 32 bits, une lecture renvoie 8 bits issus des sorties GAD/GBD.

use crate::device::CartPort;

/// Équivalent de `bitswap<16>(v, 15,11,14,10,13,9,12,8,7,3,6,2,5,1,4,0)` de MAME :
/// bit de sortie 15 = v15, 14 = v11, 13 = v14, 12 = v10, 11 = v13, 10 = v9, 9 = v12, 8 = v8,
/// 7 = v7, 6 = v3, 5 = v6, 4 = v2, 3 = v5, 2 = v1, 1 = v4, 0 = v0.
fn swap16(v: u16) -> u16 {
    // MAP[rang de sortie] = bit source.
    const MAP: [u8; 16] = [0, 4, 1, 5, 2, 6, 3, 7, 8, 12, 9, 13, 10, 14, 11, 15];
    let mut r = 0u16;
    for (o, &s) in MAP.iter().enumerate() {
        r |= ((v >> s) & 1) << o;
    }
    r
}

/// Registre à décalage + entrées de la PRO-CT0.
#[derive(Default)]
pub struct Alpha8921 {
    clk: bool,
    load: bool,
    even: bool,
    h: bool,
    c: u32,
    sr: u32,
}

impl Alpha8921 {
    fn clk_w(&mut self, state: bool) {
        if self.clk != state {
            self.clk = state;
            if !self.clk {
                // front descendant
                let sr = self.sr;
                self.sr = if self.load {
                    self.c
                } else if self.h {
                    (((sr >> 24) & 0x3F) << 26)
                        | (((sr >> 16) & 0x3F) << 18)
                        | (((sr >> 8) & 0x3F) << 10)
                        | ((sr & 0x3F) << 2)
                } else {
                    (((sr >> 26) & 0x3F) << 24)
                        | (((sr >> 18) & 0x3F) << 16)
                        | (((sr >> 10) & 0x3F) << 8)
                        | ((sr >> 2) & 0x3F)
                };
            }
        }
    }

    /// (gad, gbd) : fonction pure de (sr, even, h) — équivalent du cache `update_output` de MAME,
    /// qui n'est recalculé que lorsqu'une de ces trois valeurs change.
    fn outputs(&self) -> (u8, u8) {
        let b = |n: u32| ((self.sr >> n) & 1) as u8;
        let (mut gad, mut gbd) = if self.h {
            (
                (b(31) << 3) | (b(23) << 2) | (b(15) << 1) | b(7),
                (b(30) << 3) | (b(22) << 2) | (b(14) << 1) | b(6),
            )
        } else {
            (
                (b(24) << 3) | (b(16) << 2) | (b(8) << 1) | b(0),
                (b(25) << 3) | (b(17) << 2) | (b(9) << 1) | b(1),
            )
        };
        if self.even {
            std::mem::swap(&mut gad, &mut gbd);
        }
        (gad, gbd)
    }
}

/// Port de cartouche Fatal Fury 2.
#[derive(Default)]
pub struct FatFury2 {
    chip: Alpha8921,
    accesses: u64,
}

impl FatFury2 {
    pub fn new() -> Self {
        Self::default()
    }

    /// `protection_r(offset)` ; `offset` = index de mot dans la fenêtre.
    pub fn protection_r(&mut self, offset: u32) -> u16 {
        self.chip.even = (offset >> 1) & 1 != 0;
        self.chip.h = (offset >> 2) & 1 != 0;
        let (gad, gbd) = self.chip.outputs();
        let (gad, gbd) = (gad as u16, gbd as u16);
        ((gbd & 3) << 6) | (((gbd >> 2) & 3) << 4) | ((gad & 3) << 2) | ((gad >> 2) & 3)
    }

    /// `protection_w(offset, data)`.
    pub fn protection_w(&mut self, offset: u32, data: u16) {
        let c = &mut self.chip;
        c.clk_w(true);
        c.load = offset & 1 != 0;
        c.even = (offset >> 1) & 1 != 0;
        c.h = (offset >> 2) & 1 != 0;
        let addr = ((offset >> 3) & 0xFFFF) as u16;
        c.c = ((swap16(addr) as u32) << 16) | swap16(data) as u32;
        c.clk_w(false);
    }
}

impl CartPort for FatFury2 {
    fn read(&mut self, a: u32, _p: &[u8], _wram: &[u8]) -> Option<u16> {
        if !(0x200000..0x300000).contains(&a) {
            return None;
        }
        self.accesses += 1;
        Some(self.protection_r((a.wrapping_sub(0x200000) & 0xFFFFF) >> 1))
    }

    fn write(&mut self, a: u32, d: u16, _u: bool, _l: bool, _p: &[u8], _w: &[u8]) -> bool {
        if !(0x200000..0x300000).contains(&a) {
            return false;
        }
        self.accesses += 1;
        self.protection_w((a.wrapping_sub(0x200000) & 0xFFFFF) >> 1, d);
        true
    }

    /// MAME ne remet pas le registre à zéro (`device_reset` ne fait que recalculer les sorties).
    fn reset(&mut self) {}

    fn access_count(&self) -> u64 {
        self.accesses
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swap16_matches_mame_bitswap() {
        // bit 11 de l'entrée -> bit 14 de la sortie ; bit 3 -> bit 6 ; bit 15 reste en 15.
        assert_eq!(swap16(1 << 11), 1 << 14);
        assert_eq!(swap16(1 << 14), 1 << 13);
        assert_eq!(swap16(1 << 3), 1 << 6);
        assert_eq!(swap16(1 << 15), 1 << 15);
        assert_eq!(swap16(1 << 4), 1 << 1);
        assert_eq!(swap16(1 << 1), 1 << 2);
        assert_eq!(swap16(0xFFFF), 0xFFFF);
    }

    #[test]
    fn reset_state_reads_zero() {
        let mut f = FatFury2::new();
        f.reset();
        for off in 0..8 {
            assert_eq!(f.protection_r(off), 0);
        }
    }

    #[test]
    fn load_then_read_hand_computed() {
        let mut f = FatFury2::new();
        // offset=1 : LOAD=1, even=0, h=0, A4..A19=0 ; data 0x0001 -> swap16 = 0x0001 -> C = 1.
        f.protection_w(1, 0x0001);
        // sr = 1 : (h=0) gad = bits 24,16,8,0 -> 0b0001, gbd = bits 25,17,9,1 -> 0.
        // retour : (gbd&3)<<6 | (gbd>>2)<<4 | (gad&3)<<2 | gad>>2 = 1<<2 = 4.
        assert_eq!(f.protection_r(0), 4);
        // even=1 (offset bit1) échange gad/gbd : gbd=1 -> (1&3)<<6 = 0x40.
        assert_eq!(f.protection_r(2), 0x40);
        // h=1 : gad = bits 31,23,15,7 = 0, gbd = bits 30,22,14,6 = 0.
        assert_eq!(f.protection_r(4), 0);
    }

    #[test]
    fn shift_sequence_hand_computed() {
        let mut f = FatFury2::new();
        // Charge sr = 0x0000_0004 : data 0x0004 (bit2 -> bit1 de la sortie swap16 = 0x0002? voir ci-dessous).
        // swap16(0x0004): bit2 -> sortie rang 4 (MAP[4]=2) => 0x0010.
        f.protection_w(1, 0x0004);
        assert_eq!(f.chip.sr, 0x0010);
        // Écriture sans LOAD, h=0 (offset 0) : décalage vers la droite de 2 bits par octet.
        f.protection_w(0, 0);
        assert_eq!(f.chip.sr, 0x0004);
        // h=1 (offset 4) : décalage vers la gauche de 2 bits par octet.
        f.protection_w(4, 0);
        assert_eq!(f.chip.sr, 0x0010);
        // Les 6 bits hauts d'un octet ne débordent pas sur l'octet voisin.
        f.protection_w(1, 0x8000); // swap16(0x8000)=0x8000
        assert_eq!(f.chip.sr, 0x8000);
        f.protection_w(4, 0); // octet1 = 0x80 : (0x80&0x3F)=0 -> 0
        assert_eq!(f.chip.sr, 0);
    }

    #[test]
    fn port_ignores_addresses_outside_window() {
        let mut f = FatFury2::new();
        assert_eq!(f.read(0x100, &[], &[]), None);
        assert_eq!(f.read(0x1FFFFE, &[], &[]), None);
        assert!(f.read(0x200000, &[], &[]).is_some());
        assert!(f.read(0x2FFFFE, &[], &[]).is_some());
        assert!(!f.write(0x1FFFFE, 0, true, true, &[], &[]));
        assert!(f.write(0x200000, 0, true, true, &[], &[]));
        assert_eq!(f.access_count(), 3);
    }

    #[test]
    fn read_is_at_most_8_bits() {
        let mut f = FatFury2::new();
        f.protection_w(1, 0xFFFF);
        f.protection_w(0x7FFF1, 0xFFFF);
        for off in 0..8 {
            assert!(f.protection_r(off) <= 0xFF);
        }
    }
}
