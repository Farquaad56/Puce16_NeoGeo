//! Adaptateur YM2610 de la carte son Neo Geo.
//! Implémente `ng_bus::Ym` (côté carte son) et fournit l'interface hôte `YmfmInterface` de la puce.
//!
//! PORT-NOTE : la carte prévoit un seul type pour les deux rôles. Comme la puce garde un `Rc` vers son
//! interface (`SharedIntf`), l'interface (temps, échéances de timers, IRQ, busy, ROM ADPCM) est un objet
//! séparé, `YmIntf`, partagé avec `YmAdapter` qui possède la puce et les échantillons.
//! Unités : `ym_time` et les échéances sont en mclk ; une horloge YM = 3 mclk ; un échantillon = 432 mclk.

use std::cell::RefCell;
use std::rc::Rc;

use ng_bus::Ym;
use ng_ym2610::common::{AccessClass, YmfmInterface};
use ng_ym2610::ym2610::{SharedIntf, Ym2610};

use crate::audio_mix::mix_ym;

/// mclk par échantillon de sortie : 144 horloges YM x 3 (fidélité MED de MAME).
pub const MCLK_PER_SAMPLE: u64 = 432;
/// mclk par horloge YM.
const MCLK_PER_YM_CLOCK: u64 = 3;

/// État de l'interface hôte de la puce.
pub struct YmIntf {
    /// Temps courant de la puce en mclk.
    pub ym_time: u64,
    /// Échéances des timers A et B en mclk (None = arrêté).
    pub timer_deadline: [Option<u64>; 2],
    /// Niveau de la ligne IRQ.
    pub irq: bool,
    /// Fin de l'état occupé en mclk.
    pub busy_end: u64,
    /// ROM ADPCM (région V de la cartouche ; la même pour A et B).
    pub adpcm_rom: Vec<u8>,
}

impl YmIntf {
    fn new(adpcm_rom: Vec<u8>) -> Self {
        YmIntf {
            ym_time: 0,
            timer_deadline: [None, None],
            irq: false,
            busy_end: 0,
            adpcm_rom,
        }
    }
}

impl YmfmInterface for YmIntf {
    fn sync_mode_write(&mut self, _data: u8) {}

    fn sync_check_interrupts(&mut self) {}

    fn set_timer(&mut self, tnum: u32, duration_in_clocks: i32) {
        self.timer_deadline[tnum as usize] = if duration_in_clocks < 0 {
            None
        } else {
            Some(self.ym_time + duration_in_clocks as u64 * MCLK_PER_YM_CLOCK)
        };
    }

    fn set_busy_end(&mut self, clocks: u32) {
        self.busy_end = self.ym_time + clocks as u64 * MCLK_PER_YM_CLOCK;
    }

    fn is_busy(&self) -> bool {
        self.ym_time < self.busy_end
    }

    fn update_irq(&mut self, asserted: bool) {
        self.irq = asserted;
    }

    fn external_read(&mut self, access: AccessClass, address: u32) -> u8 {
        match access {
            AccessClass::AdpcmA | AccessClass::AdpcmB => {
                self.adpcm_rom.get(address as usize).copied().unwrap_or(0)
            }
            _ => 0,
        }
    }
}

/// YM2610 vu de la carte son : possède la puce et les échantillons produits.
pub struct YmAdapter {
    ym: Ym2610,
    intf: Rc<RefCell<YmIntf>>,
    /// Échantillons produits pendant la trame, déjà mélangés par `mix_ym`.
    pub samples: Vec<[f32; 2]>,
    /// Date (mclk) du prochain échantillon.
    next_sample: u64,
}

impl YmAdapter {
    /// Crée l'adaptateur avec la ROM ADPCM donnée.
    pub fn new(adpcm_rom: Vec<u8>) -> Self {
        let intf = Rc::new(RefCell::new(YmIntf::new(adpcm_rom)));
        let shared: SharedIntf = intf.clone();
        let mut adapter = YmAdapter {
            ym: Ym2610::new(shared, 0x36),
            intf,
            samples: Vec::new(),
            next_sample: MCLK_PER_SAMPLE,
        };
        adapter.ym.reset();
        adapter
    }

    /// Remet la puce et tout l'état à zéro (la ROM est conservée).
    pub fn reset(&mut self) {
        {
            let mut i = self.intf.borrow_mut();
            i.ym_time = 0;
            i.timer_deadline = [None, None];
            i.irq = false;
            i.busy_end = 0;
        }
        self.ym.reset();
        self.samples.clear();
        self.next_sample = MCLK_PER_SAMPLE;
    }

    /// Accès à la puce (tests, débogueur).
    pub fn chip(&self) -> &Ym2610 {
        &self.ym
    }

    /// Accès à l'interface hôte (tests, débogueur).
    pub fn intf(&self) -> &Rc<RefCell<YmIntf>> {
        &self.intf
    }
}

impl Ym for YmAdapter {
    /// Fait avancer la puce jusqu'à `mclk` : timers et échantillons dans l'ordre chronologique
    /// (timers avant l'échantillon à date égale, timer A avant timer B).
    fn advance(&mut self, mclk: u64) {
        loop {
            // prochaine échéance
            let (d0, d1) = {
                let i = self.intf.borrow();
                (i.timer_deadline[0], i.timer_deadline[1])
            };
            let mut next = self.next_sample;
            for d in [d0, d1].into_iter().flatten() {
                next = next.min(d);
            }
            if next > mclk {
                break;
            }

            // le temps de la puce ne recule jamais
            {
                let mut i = self.intf.borrow_mut();
                i.ym_time = i.ym_time.max(next);
            }

            // timers échus
            for tnum in 0..2usize {
                let due = self.intf.borrow().timer_deadline[tnum].is_some_and(|d| d <= next);
                if due {
                    self.intf.borrow_mut().timer_deadline[tnum] = None;
                    self.ym.engine_timer_expired(tnum as u32);
                }
            }

            // échantillon
            if self.next_sample <= next {
                let mut out = [[0i32; 3]; 1];
                self.ym.generate(&mut out, 1);
                let clamped = [
                    out[0][0].clamp(-32768, 32767) as i16,
                    out[0][1].clamp(-32768, 32767) as i16,
                    out[0][2].clamp(-32768, 32767) as i16,
                ];
                self.samples.push(mix_ym(clamped));
                self.next_sample += MCLK_PER_SAMPLE;
            }
        }

        let mut i = self.intf.borrow_mut();
        i.ym_time = i.ym_time.max(mclk);
    }

    fn read(&mut self, port: u8) -> u8 {
        self.ym.read(port as u32)
    }

    fn write(&mut self, port: u8, v: u8) {
        self.ym.write(port as u32, v);
    }

    fn irq(&self) -> bool {
        self.intf.borrow().irq
    }

    fn timer_due(&self, mclk: u64) -> bool {
        let i = self.intf.borrow();
        i.timer_deadline.iter().flatten().any(|&d| d <= mclk)
    }
}

/// État hors puce : temps, échéances, IRQ, busy, échantillons, prochain échantillon.
type YmHostState = (u64, [Option<u64>; 2], bool, u64, Vec<[f32; 2]>, u64);

impl YmAdapter {
    /// Écrit l'état (puce, interface hôte, échantillons) à la suite de `buf` (savestates).
    pub(crate) fn save_into(&self, buf: &mut Vec<u8>) {
        let i = self.intf.borrow();
        let host: YmHostState = (
            i.ym_time,
            i.timer_deadline,
            i.irq,
            i.busy_end,
            self.samples.clone(),
            self.next_sample,
        );
        bincode::serialize_into(&mut *buf, &self.ym).expect("savestate YM en mémoire");
        bincode::serialize_into(&mut *buf, &host).expect("savestate YM en mémoire");
    }

    /// Relit l'état écrit par `save_into` ; la ROM ADPCM est conservée.
    pub(crate) fn load_from(&mut self, rd: &mut &[u8]) -> Result<(), String> {
        let err = |e: bincode::Error| format!("savestate YM illisible : {e}");
        let ym: Ym2610 = bincode::deserialize_from(&mut *rd).map_err(err)?;
        let host: YmHostState = bincode::deserialize_from(&mut *rd).map_err(err)?;
        self.ym.restore(ym);
        let mut i = self.intf.borrow_mut();
        (i.ym_time, i.timer_deadline, i.irq, i.busy_end) = (host.0, host.1, host.2, host.3);
        self.samples = host.4;
        self.next_sample = host.5;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_timer_converts_ym_clocks_to_mclk() {
        let a = YmAdapter::new(Vec::new());
        a.intf.borrow_mut().set_timer(0, 100);
        assert_eq!(a.intf.borrow().timer_deadline[0], Some(300));
        a.intf.borrow_mut().set_timer(0, -1);
        assert_eq!(a.intf.borrow().timer_deadline[0], None);
    }

    #[test]
    fn timer_expires_exactly_at_its_deadline() {
        let mut a = YmAdapter::new(Vec::new());
        a.intf.borrow_mut().set_timer(0, 100); // échéance 300
        a.advance(299);
        assert_eq!(a.intf.borrow().timer_deadline[0], Some(300));
        a.advance(300);
        // engine_timer_expired(0) a été appelé : il réarme le timer A avec sa période par défaut
        // (1024 x 24 opérateurs x prescale 6 = 147456 horloges YM)
        assert_eq!(a.intf.borrow().timer_deadline[0], Some(300 + 147_456 * 3));
    }

    #[test]
    fn timer_a_sets_status_and_irq() {
        let mut a = YmAdapter::new(Vec::new());
        a.advance(0);
        for (port, v) in [
            (0u8, 0x24u8),
            (1, 0xFF),
            (0, 0x25),
            (1, 0x03),
            (0, 0x27),
            (1, 0x05),
        ] {
            a.write(port, v);
        }
        // N = 1023 : période 1 -> 144 horloges YM = 432 mclk
        assert_eq!(a.intf.borrow().timer_deadline[0], Some(432));
        a.advance(431);
        assert_eq!(a.read(0) & 1, 0);
        assert!(!a.irq());
        a.advance(432);
        assert_eq!(a.read(0) & 1, 1);
        assert!(a.irq());
    }

    #[test]
    fn three_samples_in_three_sample_periods() {
        let mut a = YmAdapter::new(Vec::new());
        a.advance(432 * 3);
        assert_eq!(a.samples.len(), 3);
        a.advance(432 * 3 + 431);
        assert_eq!(a.samples.len(), 3);
        a.advance(432 * 4);
        assert_eq!(a.samples.len(), 4);
        assert!(a.samples.iter().all(|s| *s == [0.0, 0.0]));
    }

    #[test]
    fn external_read_outside_rom_is_zero() {
        let a = YmAdapter::new(vec![0x11, 0x22]);
        let mut i = a.intf.borrow_mut();
        assert_eq!(i.external_read(AccessClass::AdpcmA, 1), 0x22);
        assert_eq!(i.external_read(AccessClass::AdpcmB, 0), 0x11);
        assert_eq!(i.external_read(AccessClass::AdpcmA, 2), 0);
        assert_eq!(i.external_read(AccessClass::IoPort, 0), 0);
    }

    #[test]
    fn busy_follows_writes() {
        let mut a = YmAdapter::new(Vec::new());
        assert_eq!(a.read(0) & 0x80, 0);
        a.write(0, 0x30);
        a.write(1, 0x00); // écriture FM : occupé 32 x prescale 6 horloges YM
        assert_eq!(a.read(0) & 0x80, 0x80);
        a.advance(32 * 6 * 3);
        assert_eq!(a.read(0) & 0x80, 0);
    }

    #[test]
    fn reset_clears_state_but_keeps_rom() {
        let mut a = YmAdapter::new(vec![7]);
        a.advance(432 * 2);
        a.reset();
        assert!(a.samples.is_empty());
        assert_eq!(a.intf.borrow().ym_time, 0);
        assert_eq!(a.intf.borrow().adpcm_rom, vec![7]);
    }
}
