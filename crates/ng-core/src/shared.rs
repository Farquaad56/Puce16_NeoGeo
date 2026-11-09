//! Adaptateurs de partage entre le Bus et la Machine : `Rc<RefCell<T>>` implémentant `Device`.
//!
//! PORT-NOTE : `Lspc` et `Palette` (ng-lspc) n'implémentent pas `ng_bus::Device` et la règle des
//! orphelins interdit de le faire ici : `SharedLspc` et `SharedPalette` jouent ce rôle.

use std::cell::RefCell;
use std::rc::Rc;

use ng_bus::Device;
use ng_lspc::palette::Palette;
use ng_lspc::Lspc;

/// Adaptateur générique : délègue à un `Device` partagé.
pub struct Shared<T: Device>(pub Rc<RefCell<T>>);

impl<T: Device> Device for Shared<T> {
    fn read(&mut self, addr: u32) -> u16 {
        self.0.borrow_mut().read(addr)
    }

    fn write(&mut self, addr: u32, data: u16, upper: bool, lower: bool) {
        self.0.borrow_mut().write(addr, data, upper, lower);
    }
}

/// Registres vidéo LSPC ($3C0000-$3C000F), partagés avec la Machine.
pub struct SharedLspc(pub Rc<RefCell<Lspc>>);

impl Device for SharedLspc {
    fn read(&mut self, addr: u32) -> u16 {
        // décalage en mots dans la fenêtre de registres
        let offset = (addr >> 1) & 7;
        self.0
            .borrow_mut()
            .read_reg(offset, true, true)
            .unwrap_or(0xFFFF)
    }

    fn write(&mut self, addr: u32, data: u16, upper: bool, lower: bool) {
        let offset = (addr >> 1) & 7;
        self.0.borrow_mut().write_reg(offset, data, upper, lower);
    }
}

/// RAM de palette ($400000-$7FFFFF, miroir tous les 0x2000 octets), partagée avec la Machine.
pub struct SharedPalette(pub Rc<RefCell<Palette>>);

impl Device for SharedPalette {
    fn read(&mut self, addr: u32) -> u16 {
        self.0.borrow().read16(((addr & 0x1FFE) >> 1) as usize)
    }

    fn write(&mut self, addr: u32, data: u16, upper: bool, lower: bool) {
        let off = ((addr & 0x1FFE) >> 1) as usize;
        let mut pal = self.0.borrow_mut();
        // écriture partielle : fusionne avec la valeur courante
        let merged = match (upper, lower) {
            (true, true) => data,
            (true, false) => (data & 0xFF00) | (pal.read16(off) & 0x00FF),
            (false, true) => (pal.read16(off) & 0xFF00) | (data & 0x00FF),
            (false, false) => return,
        };
        pal.write16(off, merged);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ng_bus::backup::BackupRam;
    use ng_bus::Io;

    #[test]
    fn shared_io_delegates() {
        let io = Rc::new(RefCell::new(Io::default()));
        let mut dev = Shared(io.clone());
        io.borrow_mut().sound_reply = 0xAB;
        assert_eq!(dev.read(0x320000) >> 8, 0xAB);
    }

    #[test]
    fn shared_backup_delegates() {
        let b = Rc::new(RefCell::new(BackupRam::new()));
        b.borrow_mut().unlocked = true;
        let mut dev = Shared(b.clone());
        dev.write(0xD00010, 0x1234, true, true);
        assert_eq!(dev.read(0xD00010), 0x1234);
    }

    #[test]
    fn palette_mirrors_and_merges_bytes() {
        let p = Rc::new(RefCell::new(Palette::new()));
        let mut dev = SharedPalette(p.clone());
        dev.write(0x400004, 0x1234, true, true);
        assert_eq!(dev.read(0x400004), 0x1234);
        assert_eq!(dev.read(0x402004), 0x1234); // miroir 0x2000
        dev.write(0x400004, 0xAB00, true, false);
        assert_eq!(dev.read(0x400004), 0xAB34);
        dev.write(0x400004, 0x00CD, false, true);
        assert_eq!(dev.read(0x400004), 0xABCD);
    }

    #[test]
    fn lspc_registers_reach_the_chip() {
        let l = Rc::new(RefCell::new(Lspc::new()));
        let mut dev = SharedLspc(l.clone());
        dev.write(0x3C0004, 0x0020, true, true); // modulo
        assert_eq!(dev.read(0x3C0004), 0x0020);
        dev.write(0x3C0006, 0x0108, true, true); // contrôle vidéo
        assert!(l.borrow().anim_disabled);
    }
}
