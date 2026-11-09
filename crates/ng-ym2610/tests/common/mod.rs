//! Interface factice et pilote de rejeu pour les tests contre les vecteurs ymfm (tests-data/ym).

use std::cell::RefCell;
use std::rc::Rc;

use ng_ym2610::common::{AccessClass, YmfmInterface};
use ng_ym2610::ym2610::{SharedIntf, Ym2610};

/// Horloges YM par échantillon de sortie (fidélité MED de MAME).
pub const CLOCKS_PER_SAMPLE: u64 = 144;
pub const SAMPLES: u32 = 1000;

/// Écriture planifiée : `before` = numéro d'échantillon (>= 1), `off` = offset 0..3, `val` = valeur.
#[derive(Clone, Copy)]
pub struct W {
    pub before: u32,
    pub off: u8,
    pub val: u8,
}

#[derive(Default)]
pub struct TestIntf {
    pub ym_time: u64,
    pub deadline: [Option<u64>; 2],
    pub busy_end: u64,
    pub irq: bool,
}

/// Contenu de la ROM ADPCM factice (A et B).
fn rom(a: u32) -> u8 {
    let h = a.wrapping_mul(0x9E37_79B1);
    ((h ^ (h >> 15)) & 0xFF) as u8
}

impl YmfmInterface for TestIntf {
    fn sync_mode_write(&mut self, _data: u8) {}
    fn sync_check_interrupts(&mut self) {}
    fn set_timer(&mut self, tnum: u32, duration: i32) {
        self.deadline[tnum as usize] = if duration < 0 {
            None
        } else {
            Some(self.ym_time + duration as u64)
        };
    }
    fn set_busy_end(&mut self, clocks: u32) {
        self.busy_end = self.ym_time + clocks as u64;
    }
    fn is_busy(&self) -> bool {
        self.ym_time < self.busy_end
    }
    fn update_irq(&mut self, asserted: bool) {
        self.irq = asserted;
    }
    fn external_read(&mut self, _access: AccessClass, address: u32) -> u8 {
        rom(address)
    }
}

/// Rejoue `writes` et retourne les échantillons ([FM gauche, FM droite, SSG], clampés en i16)
/// et la valeur de `read(0)` après chaque échantillon.
pub fn run(writes: &[W]) -> (Vec<[i16; 3]>, Vec<u8>) {
    let intf = Rc::new(RefCell::new(TestIntf::default()));
    let shared: SharedIntf = intf.clone();
    let mut chip = Ym2610::new(shared, 0x36);
    chip.reset();

    let mut samples = Vec::new();
    let mut status = Vec::new();
    let mut wi = 0;
    for n in 1..=SAMPLES {
        // (1) écritures planifiées avant l'échantillon n
        intf.borrow_mut().ym_time = (n as u64 - 1) * CLOCKS_PER_SAMPLE;
        while wi < writes.len() && writes[wi].before <= n {
            chip.write(writes[wi].off as u32, writes[wi].val);
            wi += 1;
        }

        // (2) timers dont l'échéance <= n * 144, dans l'ordre chronologique (timer 0 si égalité)
        let target = n as u64 * CLOCKS_PER_SAMPLE;
        loop {
            let mut best: Option<usize> = None;
            for k in 0..2 {
                let d = intf.borrow().deadline[k];
                if let Some(d) = d {
                    if d <= target && best.is_none_or(|b| d < intf.borrow().deadline[b].unwrap()) {
                        best = Some(k);
                    }
                }
            }
            let Some(b) = best else { break };
            let d = intf.borrow().deadline[b].unwrap();
            intf.borrow_mut().ym_time = d;
            intf.borrow_mut().deadline[b] = None;
            chip.engine_timer_expired(b as u32);
        }
        intf.borrow_mut().ym_time = target;

        // (3) génération
        let mut o = [[0i32; 3]; 1];
        chip.generate(&mut o, 1);
        samples.push([
            o[0][0].clamp(-32768, 32767) as i16,
            o[0][1].clamp(-32768, 32767) as i16,
            o[0][2].clamp(-32768, 32767) as i16,
        ]);

        // (4) statut
        status.push(chip.read(0));
    }
    (samples, status)
}
