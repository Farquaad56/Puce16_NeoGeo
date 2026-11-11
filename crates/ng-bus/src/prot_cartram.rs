//! RAM de cartouche de 8 Kio non volatile ($200000-$201FFF) : V-Liner et Jockey GP.
//! MAME : `neogeo_vliner_cart_device` / `neogeo_cmc_jockeygp_cart_device` (`ram_r` / `ram_w`) ;
//! V-Liner ajoute des ports d'entrée en $280000 (IN5) et $2C0000 (IN6).

use crate::device::CartPort;

pub const CART_RAM_SIZE: usize = 0x2000;
const BASE: u32 = 0x200000;

pub struct CartRam {
    ram: Vec<u8>,
    vliner: bool,
    accesses: u64,
}

impl CartRam {
    pub fn new(vliner: bool) -> Self {
        Self {
            ram: vec![0; CART_RAM_SIZE],
            vliner,
            accesses: 0,
        }
    }
}

impl CartPort for CartRam {
    fn read(&mut self, a: u32, _p: &[u8], _wram: &[u8]) -> Option<u16> {
        let a = a & !1;
        if (BASE..BASE + CART_RAM_SIZE as u32).contains(&a) {
            self.accesses += 1;
            let i = (a - BASE) as usize;
            return Some(((self.ram[i] as u16) << 8) | self.ram[i + 1] as u16);
        }
        if self.vliner {
            // IN5 : tous les bits actifs bas au repos ; IN6 : bits 0-1 actifs bas, 6-15 actifs hauts.
            match a {
                0x280000 => return Some(0xFFFF),
                0x2C0000 => return Some(0x0003),
                _ => {}
            }
        }
        None
    }

    fn write(&mut self, a: u32, d: u16, upper: bool, lower: bool, _p: &[u8], _w: &[u8]) -> bool {
        let a = a & !1;
        if !(BASE..BASE + CART_RAM_SIZE as u32).contains(&a) {
            return false;
        }
        self.accesses += 1;
        let i = (a - BASE) as usize;
        if upper {
            self.ram[i] = (d >> 8) as u8;
        }
        if lower {
            self.ram[i + 1] = d as u8;
        }
        true
    }

    fn access_count(&self) -> u64 {
        self.accesses
    }

    fn nv_ram(&self) -> Option<&[u8]> {
        Some(&self.ram)
    }

    fn nv_ram_mut(&mut self) -> Option<&mut [u8]> {
        Some(&mut self.ram)
    }
}

#[cfg(test)]
mod prot_cartram_tests {
    use super::*;

    #[test]
    fn write_then_read_word() {
        let mut c = CartRam::new(false);
        assert!(c.write(0x200010, 0x1234, true, true, &[], &[]));
        assert_eq!(c.read(0x200010, &[], &[]), Some(0x1234));
    }

    #[test]
    fn byte_writes_respect_lanes() {
        let mut c = CartRam::new(false);
        c.write(0x200000, 0xABCD, true, true, &[], &[]);
        c.write(0x200000, 0x1100, true, false, &[], &[]);
        assert_eq!(c.read(0x200000, &[], &[]), Some(0x11CD));
        c.write(0x200000, 0x0022, false, true, &[], &[]);
        assert_eq!(c.read(0x200000, &[], &[]), Some(0x1122));
    }

    #[test]
    fn window_is_8k_only() {
        let mut c = CartRam::new(false);
        assert!(!c.write(0x202000, 1, true, true, &[], &[]));
        assert_eq!(c.read(0x202000, &[], &[]), None);
        assert!(c.write(0x201FFE, 0xBEEF, true, true, &[], &[]));
        assert_eq!(c.read(0x201FFE, &[], &[]), Some(0xBEEF));
        assert_eq!(c.nv_ram().unwrap().len(), CART_RAM_SIZE);
    }

    #[test]
    fn only_vliner_has_extra_inputs() {
        let mut j = CartRam::new(false);
        let mut v = CartRam::new(true);
        assert_eq!(j.read(0x280000, &[], &[]), None);
        assert_eq!(v.read(0x280000, &[], &[]), Some(0xFFFF));
        assert_eq!(v.read(0x2C0000, &[], &[]), Some(0x0003));
    }
}
