//! Puce Altera de The King of Fighters '98 : recouvrement de ROM ($100-$103).
//! Port exact de MAME prot_kof98.cpp (`protection_r` / `protection_w`).
//!
//! Écrire 0x0090 à $20AAAA fait lire 0x00C2 / 0x00FD en $100 / $102 (l'en-tête Neo Geo) ; écrire
//! 0x00F0 fait lire 0x4E45 / 0x4F2D ; sinon on lit la ROM décryptée. L'état survit au reset
//! (MAME ne le remet pas à zéro dans `device_reset`).

use crate::device::CartPort;

#[derive(Default)]
pub struct Kof98 {
    state: u8,
}

impl Kof98 {
    pub fn new() -> Self {
        Self { state: 0 }
    }
}

impl CartPort for Kof98 {
    fn read(&mut self, a: u32, p: &[u8], _wram: &[u8]) -> Option<u16> {
        let a = a & !1;
        if a != 0x100 && a != 0x102 {
            return None;
        }
        let offset = (a - 0x100) >> 1;
        Some(match (self.state, offset) {
            (1, 0) => 0x00C2,
            (1, _) => 0x00FD,
            (2, 0) => 0x4E45,
            (2, _) => 0x4F2D,
            // ROM par défaut : mots décryptés en $100 et $102 (MAME m_default_rom).
            _ => {
                let i = a as usize;
                ((p[i] as u16) << 8) | p[i + 1] as u16
            }
        })
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
        if a & !1 != 0x20AAAA {
            return false;
        }
        match d {
            0x0090 => self.state = 1,
            0x00F0 => self.state = 2,
            _ => {} // 0x00AA est écrit mais sans effet
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rom() -> Vec<u8> {
        let mut p = vec![0u8; 0x200];
        p[0x100..0x104].copy_from_slice(&[0xAB, 0xCD, 0x12, 0x34]);
        p
    }

    #[test]
    fn initial_state_reads_the_default_rom() {
        let mut c = Kof98::new();
        let p = rom();
        assert_eq!(c.read(0x100, &p, &[]), Some(0xABCD));
        assert_eq!(c.read(0x102, &p, &[]), Some(0x1234));
    }

    #[test]
    fn write_0x0090_gives_the_c2_fd_header() {
        let mut c = Kof98::new();
        let p = rom();
        assert!(c.write(0x20AAAA, 0x0090, true, true, &p, &[]));
        assert_eq!(c.read(0x100, &p, &[]), Some(0x00C2));
        assert_eq!(c.read(0x102, &p, &[]), Some(0x00FD));
    }

    #[test]
    fn write_0x00f0_gives_the_name_header() {
        let mut c = Kof98::new();
        let p = rom();
        c.write(0x20AAAA, 0x00F0, true, true, &p, &[]);
        assert_eq!(c.read(0x100, &p, &[]), Some(0x4E45));
        assert_eq!(c.read(0x102, &p, &[]), Some(0x4F2D));
    }

    #[test]
    fn other_writes_keep_the_state_and_other_addresses_are_ignored() {
        let mut c = Kof98::new();
        let p = rom();
        c.write(0x20AAAA, 0x0090, true, true, &p, &[]);
        assert!(c.write(0x20AAAA, 0x00AA, true, true, &p, &[]));
        assert_eq!(c.read(0x100, &p, &[]), Some(0x00C2));
        assert!(!c.write(0x20AAAC, 0x00F0, true, true, &p, &[]));
        assert_eq!(c.read(0x104, &p, &[]), None);
        assert_eq!(c.read(0x2FFFE0, &p, &[]), None);
        // l'état survit au reset
        c.reset();
        assert_eq!(c.read(0x100, &p, &[]), Some(0x00C2));
    }
}
