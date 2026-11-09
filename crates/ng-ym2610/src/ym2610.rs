// license:BSD-3-Clause
// copyright-holders:Aaron Giles
//! YM2610 : FM 4 voies + SSG + ADPCM-A (6 voies) + ADPCM-B, vu du bus (registres, statut, lecture/écriture).
//! Porté de ref/ymfm/src/ymfm_opn.h (l.645-720) et ymfm_opn.cpp (l.1838-2109).
//!
//! PORT-NOTE : ymfm partage une seule `ymfm_interface` entre tous les moteurs. En Rust, chaque moteur
//! (SSG, ADPCM-A, ADPCM-B) possède un `Box<dyn YmfmInterface>` : l'interface hôte est donc un
//! `Rc<RefCell<dyn YmfmInterface>>` (`SharedIntf`), enveloppé dans un `IntfHandle` par moteur.
//! `ssg_override`, `set_fidelity`, `update_prescale`, `save_restore` et `generate` sont hors de cette
//! carte (generate : T504b ; savestates : T604). `ym2610b` n'existe pas ici (channel_mask par défaut 0x36).

use std::cell::RefCell;
use std::rc::Rc;

use crate::adpcm_a::AdpcMAEngine;
use crate::adpcm_b::AdpcMBEngine;
use crate::common::{bitfield, AccessClass, YmfmInterface};
use crate::fm_engine::FmEngine;
use crate::ssg::SsgEngine;

/// Interface hôte partagée entre les moteurs de la puce.
pub type SharedIntf = Rc<RefCell<dyn YmfmInterface>>;

/// Poignée clonable qui relaie vers l'interface partagée.
pub struct IntfHandle(pub SharedIntf);

impl YmfmInterface for IntfHandle {
    fn sync_mode_write(&mut self, data: u8) {
        self.0.borrow_mut().sync_mode_write(data);
    }
    fn sync_check_interrupts(&mut self) {
        self.0.borrow_mut().sync_check_interrupts();
    }
    fn set_timer(&mut self, tnum: u32, duration_in_clocks: i32) {
        self.0.borrow_mut().set_timer(tnum, duration_in_clocks);
    }
    fn set_busy_end(&mut self, clocks: u32) {
        self.0.borrow_mut().set_busy_end(clocks);
    }
    fn is_busy(&self) -> bool {
        self.0.borrow().is_busy()
    }
    fn update_irq(&mut self, asserted: bool) {
        self.0.borrow_mut().update_irq(asserted);
    }
    fn external_read(&mut self, access: AccessClass, address: u32) -> u8 {
        self.0.borrow_mut().external_read(access, address)
    }
    fn external_write(&mut self, access: AccessClass, address: u32, data: u8) {
        self.0.borrow_mut().external_write(access, address, data);
    }
}

pub struct Ym2610 {
    /// Registre d'adresse.
    address: u16,
    /// Masque des canaux FM.
    fm_mask: u8,
    /// Signaux de fin d'échantillon.
    eos_status: u8,
    /// Contrôle du masque de drapeaux.
    flag_mask: u8,
    /// Interface propre au moteur FM (les autres moteurs ont la leur).
    fm_intf: IntfHandle,
    /// Moteur FM central.
    fm: FmEngine,
    ssg: SsgEngine,
    adpcm_a: AdpcMAEngine,
    adpcm_b: AdpcMBEngine,
}

impl Ym2610 {
    pub const EOS_FLAGS_MASK: u8 = 0xbf;
    pub const FM_OUTPUTS: u32 = FmEngine::OUTPUTS;
    pub const SSG_OUTPUTS: u32 = 1;
    pub const OUTPUTS: u32 = Self::FM_OUTPUTS + Self::SSG_OUTPUTS;

    /// Constructeur ; `channel_mask` vaut 0x36 pour le YM2610.
    pub fn new(intf: SharedIntf, channel_mask: u8) -> Self {
        Ym2610 {
            address: 0,
            fm_mask: channel_mask,
            eos_status: 0x00,
            flag_mask: Self::EOS_FLAGS_MASK,
            fm_intf: IntfHandle(intf.clone()),
            fm: FmEngine::new(),
            ssg: SsgEngine::new(Box::new(IntfHandle(intf.clone()))),
            adpcm_a: AdpcMAEngine::new(Box::new(IntfHandle(intf.clone())), 8),
            adpcm_b: AdpcMBEngine::new(Box::new(IntfHandle(intf)), 8),
        }
    }

    /// Remet le système à zéro.
    pub fn reset(&mut self) {
        // remet à zéro les moteurs
        self.fm.reset(&mut self.fm_intf);
        self.ssg.reset();
        self.adpcm_a.reset();
        self.adpcm_b.reset();

        // initialise nos états d'interruption spéciaux
        self.eos_status = 0x00;
        self.flag_mask = Self::EOS_FLAGS_MASK;
    }

    pub fn fm_mask(&self) -> u8 {
        self.fm_mask
    }

    pub fn fm(&self) -> &FmEngine {
        &self.fm
    }

    pub fn fm_mut(&mut self) -> &mut FmEngine {
        &mut self.fm
    }

    pub fn ssg(&self) -> &SsgEngine {
        &self.ssg
    }

    pub fn adpcm_a(&self) -> &AdpcMAEngine {
        &self.adpcm_a
    }

    pub fn adpcm_b(&self) -> &AdpcMBEngine {
        &self.adpcm_b
    }

    /// Appelé par l'hôte quand un timer armé via `set_timer` expire.
    pub fn engine_timer_expired(&mut self, tnum: u32) {
        self.fm.engine_timer_expired(tnum, &mut self.fm_intf);
    }

    /// Invalide les caches du moteur FM.
    pub fn invalidate_caches(&mut self) {
        self.fm.invalidate_caches();
    }

    /// Lit le registre de statut.
    pub fn read_status(&mut self) -> u8 {
        let mut result = self.fm.status() & (FmEngine::STATUS_TIMERA | FmEngine::STATUS_TIMERB);
        if self.fm_intf.is_busy() {
            result |= FmEngine::STATUS_BUSY;
        }
        result
    }

    /// Lit le registre de données.
    pub fn read_data(&mut self) -> u8 {
        let mut result = 0;
        if self.address < 0x0e {
            // 00-0D : lecture du SSG
            result = self.ssg.read((self.address & 0x0f) as u32);
        } else if self.address < 0x10 {
            // 0E-0F : ports d'E/S non gérés
            result = 0xff;
        } else if self.address == 0xff {
            // FF : code d'identification
            result = 1;
        }
        result
    }

    /// Lit le registre de statut étendu.
    pub fn read_status_hi(&mut self) -> u8 {
        self.eos_status & self.flag_mask
    }

    /// Lit le registre de données haut.
    pub fn read_data_hi(&mut self) -> u8 {
        0
    }

    /// Gère une lecture depuis le composant.
    pub fn read(&mut self, offset: u32) -> u8 {
        match offset & 3 {
            // port de statut, compatible YM2203
            0 => self.read_status(),
            // port de données (SSG seulement)
            1 => self.read_data(),
            // port de statut étendu
            2 => self.read_status_hi(),
            // données ADPCM-B
            _ => self.read_data_hi(),
        }
    }

    /// Gère une écriture dans le registre d'adresse.
    pub fn write_address(&mut self, data: u8) {
        // fixe simplement l'adresse
        self.address = data as u16;
    }

    /// Gère une écriture dans le registre de données.
    pub fn write_data(&mut self, mut data: u8) {
        // ignoré s'il est associé à l'adresse haute
        if bitfield(self.address as u32, 8, 1) != 0 {
            return;
        }

        if self.address < 0x0e {
            // 00-0D : écriture dans le SSG
            self.ssg.write((self.address & 0x0f) as u32, data);
        } else if self.address < 0x10 {
            // 0E-0F : ports d'E/S non gérés
        } else if self.address < 0x1c {
            // 10-1B : écriture dans l'ADPCM-B ; le YM2610 force le mode externe et désactive l'enregistrement
            if self.address == 0x10 {
                data = (data | 0x20) & !0x40;
            }
            self.adpcm_b.write((self.address & 0x0f) as u32, data);
        } else if self.address == 0x1c {
            // 1C : remise à zéro des drapeaux EOS
            self.flag_mask = !data & Self::EOS_FLAGS_MASK;
            self.eos_status &= !(data & Self::EOS_FLAGS_MASK);
        } else {
            // 1D-FF : écriture dans le FM
            self.fm.write(self.address, data, &mut self.fm_intf);
        }

        // marque occupé pour un moment
        self.fm_intf.set_busy_end(32 * self.fm.clock_prescale());
    }

    /// Gère une écriture dans le registre d'adresse haut.
    pub fn write_address_hi(&mut self, data: u8) {
        // fixe simplement l'adresse
        self.address = 0x100 | data as u16;
    }

    /// Gère une écriture dans le registre de données haut.
    pub fn write_data_hi(&mut self, data: u8) {
        // ignoré s'il est associé à l'adresse basse
        if bitfield(self.address as u32, 8, 1) == 0 {
            return;
        }

        if self.address < 0x130 {
            // 100-12F : écriture dans l'ADPCM-A
            self.adpcm_a.write((self.address & 0x3f) as u32, data);
        } else {
            // 130-1FF : écriture dans le FM
            self.fm.write(self.address, data, &mut self.fm_intf);
        }

        // marque occupé pour un moment
        self.fm_intf.set_busy_end(32 * self.fm.clock_prescale());
    }

    /// Gère une écriture via l'interface de registres.
    pub fn write(&mut self, offset: u32, data: u8) {
        match offset & 3 {
            // port d'adresse
            0 => self.write_address(data),
            // port de données
            1 => self.write_data(data),
            // port d'adresse haut
            2 => self.write_address_hi(data),
            // port de données haut
            _ => self.write_data_hi(data),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Mock {
        busy: bool,
        timers: Vec<(u32, i32)>,
        busy_ends: Vec<u32>,
        irqs: Vec<bool>,
    }

    impl YmfmInterface for Mock {
        fn sync_mode_write(&mut self, _data: u8) {}
        fn sync_check_interrupts(&mut self) {}
        fn set_timer(&mut self, tnum: u32, duration: i32) {
            self.timers.push((tnum, duration));
        }
        fn set_busy_end(&mut self, clocks: u32) {
            self.busy_ends.push(clocks);
        }
        fn is_busy(&self) -> bool {
            self.busy
        }
        fn update_irq(&mut self, asserted: bool) {
            self.irqs.push(asserted);
        }
    }

    fn chip() -> (Ym2610, Rc<RefCell<Mock>>) {
        let mock = Rc::new(RefCell::new(Mock::default()));
        let intf: SharedIntf = mock.clone();
        let mut chip = Ym2610::new(intf, 0x36);
        chip.reset();
        mock.borrow_mut().timers.clear();
        mock.borrow_mut().irqs.clear();
        (chip, mock)
    }

    #[test]
    fn reset_status_is_zero() {
        let (mut chip, mock) = chip();
        assert_eq!(chip.read(0), 0);
        assert_eq!(chip.read(2), 0);
        assert_eq!(chip.read(3), 0);
        assert_eq!(chip.fm_mask(), 0x36);
        mock.borrow_mut().busy = true;
        assert_eq!(chip.read(0), 0x80);
    }

    #[test]
    fn mode_register_starts_timer_a() {
        let (mut chip, mock) = chip();
        chip.write(0, 0x27);
        chip.write(1, 0x15); // reset A + active A + charge A
                             // timer A : N = 0 -> période 1024 ; durée = 1024 * 24 opérateurs * prescale 6
        assert_eq!(mock.borrow().timers, vec![(1, -1), (0, 1024 * 24 * 6)]);
        assert_eq!(mock.borrow().busy_ends, vec![32 * 6]);
        assert_eq!(chip.read(0) & 3, 0);
        // expiration : statut A visible et IRQ levée
        chip.engine_timer_expired(0);
        assert_eq!(chip.read(0) & 3, 1);
        assert_eq!(mock.borrow().irqs, vec![true]);
    }

    #[test]
    fn address_halves_must_match() {
        let (mut chip, mock) = chip();
        chip.write(2, 0x01); // adresse haute 0x101
        chip.write(1, 0x7F); // écriture basse avec adresse haute : ignorée
        assert_eq!(chip.adpcm_a().regs().total_level(), 0);
        assert!(mock.borrow().busy_ends.is_empty());
        chip.write(3, 0x7F); // ADPCM-A registre 0x01 : total level
        assert_eq!(chip.adpcm_a().regs().total_level(), 0x3F);
        assert_eq!(mock.borrow().busy_ends, vec![192]);
        chip.write(0, 0x01);
        chip.write(3, 0x55); // adresse basse avec écriture haute : ignorée
        assert_eq!(chip.adpcm_a().regs().total_level(), 0x3F);
    }

    #[test]
    fn adpcm_b_control_forces_external_and_no_record() {
        let (mut chip, _mock) = chip();
        chip.write(0, 0x10);
        chip.write(1, 0x40); // enregistrement demandé
        assert!(chip.adpcm_b().regs().external());
        assert!(!chip.adpcm_b().regs().record());
    }

    #[test]
    fn eos_flag_reset_register() {
        let (mut chip, _mock) = chip();
        chip.eos_status = 0x3F;
        assert_eq!(chip.read(2), 0x3F);
        chip.write(0, 0x1C);
        chip.write(1, 0x01);
        assert_eq!(chip.flag_mask, 0xBE);
        assert_eq!(chip.eos_status, 0x3E);
        assert_eq!(chip.read(2), 0x3E);
        chip.write(1, 0xFF);
        assert_eq!(chip.flag_mask, 0x00);
        assert_eq!(chip.read(2), 0);
        // reset : tout revient à l'état initial
        chip.reset();
        assert_eq!(chip.flag_mask, Ym2610::EOS_FLAGS_MASK);
        assert_eq!(chip.eos_status, 0);
    }

    #[test]
    fn ssg_data_and_id_reads() {
        let (mut chip, _mock) = chip();
        chip.write(0, 0x00);
        chip.write(1, 0xAB); // SSG registre 0
        chip.write(0, 0x00);
        assert_eq!(chip.read(1), 0xAB);
        chip.write(0, 0x0E);
        assert_eq!(chip.read(1), 0xFF);
        chip.write(0, 0xFF);
        assert_eq!(chip.read(1), 1);
        chip.write(0, 0x20);
        assert_eq!(chip.read(1), 0);
    }

    #[test]
    fn fm_writes_reach_the_engine() {
        let (mut chip, _mock) = chip();
        chip.write(0, 0xA4);
        chip.write(1, 0x22);
        chip.write(0, 0xA0);
        chip.write(1, 0x34);
        assert_eq!(chip.fm().regs().ch_block_freq(0), 0x2234);
        // moitié haute : canal 3 (0x1A4/0x1A0)
        chip.write(2, 0xA4);
        chip.write(3, 0x0B);
        chip.write(2, 0xA0);
        chip.write(3, 0x56);
        assert_eq!(
            chip.fm()
                .regs()
                .ch_block_freq(crate::opn_regs::OpnRegisters::channel_offset(3)),
            0x0B56
        );
    }
}
