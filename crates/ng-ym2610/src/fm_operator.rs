// license:BSD-3-Clause
// copyright-holders:Aaron Giles
//! Opérateur FM ("slot") : onde sinus modulée par une enveloppe.
//! Porté de ref/ymfm/src/ymfm_fm.h (l.158-250) et ymfm_fm.ipp (l.369-792).
//!
//! PORT-NOTE : `m_regs` / `m_owner` deviennent un argument `regs: &OpnRegisters` passé aux méthodes
//! qui en ont besoin. `save_restore` n'est pas porté (savestates : T604). `compute_noise_volume`
//! est porté mais ne sert qu'aux puces à bruit (l'OPN renvoie `noise_state() == 0`).
//! `EG_HAS_DEPRESS` et `EG_HAS_REVERB` valent false pour l'OPN : ces branches sont omises.

use crate::common::{bitfield, EG_ATTACK, EG_DECAY, EG_RELEASE, EG_SUSTAIN};
use crate::fm_tables::{attenuation_increment, attenuation_to_volume};
use crate::opn_regs::{OpdataCache, OpnRegisters};

/// Types de key on (ymfm `keyon_type`).
pub const KEYON_NORMAL: u32 = 0;
pub const KEYON_RHYTHM: u32 = 1;
pub const KEYON_CSM: u32 = 2;

/// Valeur "silencieuse" : permet d'éviter du travail inutile.
pub const EG_QUIET: u32 = 0x380;

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FmOperator {
    choffs: u32,
    opoffs: u32,
    /// Phase courante (format 10.10).
    phase: u32,
    /// Atténuation d'enveloppe calculée (format 4.6).
    env_attenuation: u16,
    env_state: usize,
    /// Non nul si la sortie doit être inversée (bit 0).
    ssg_inverted: u8,
    /// État de la touche (bit 0).
    key_state: u8,
    /// Key on live (bit 0 = direct, bit 1 = rythme, bit 2 = CSM).
    keyon_live: u8,
    cache: OpdataCache,
}

impl FmOperator {
    /// Constructeur.
    pub fn new(opoffs: u32) -> Self {
        FmOperator {
            choffs: 0,
            opoffs,
            phase: 0,
            env_attenuation: 0x3ff,
            env_state: EG_RELEASE,
            ssg_inverted: 0,
            key_state: 0,
            keyon_live: 0,
            cache: OpdataCache::default(),
        }
    }

    /// Remet l'état de l'opérateur à zéro.
    pub fn reset(&mut self) {
        self.phase = 0;
        self.env_attenuation = 0x3ff;
        self.env_state = EG_RELEASE;
        self.ssg_inverted = 0;
        self.key_state = 0;
        self.keyon_live = 0;
    }

    pub fn opoffs(&self) -> u32 {
        self.opoffs
    }

    pub fn choffs(&self) -> u32 {
        self.choffs
    }

    pub fn set_choffs(&mut self, choffs: u32) {
        self.choffs = choffs;
    }

    // accesseurs de débogage
    pub fn debug_eg_state(&self) -> usize {
        self.env_state
    }
    pub fn debug_eg_attenuation(&self) -> u16 {
        self.env_attenuation
    }
    pub fn debug_ssg_inverted(&self) -> u8 {
        self.ssg_inverted
    }
    pub fn debug_cache(&mut self) -> &mut OpdataCache {
        &mut self.cache
    }

    /// Force l'état de l'enveloppe (tests des canaux).
    #[cfg(test)]
    pub(crate) fn test_set_envelope(&mut self, attenuation: u16, state: usize) {
        self.env_attenuation = attenuation;
        self.env_state = state;
    }

    /// Prépare avant l'horloge ; true tant que l'opérateur est actif.
    pub fn prepare(&mut self, regs: &OpnRegisters) -> bool {
        // met les données en cache
        regs.cache_operator_data(self.choffs, self.opoffs, &mut self.cache);

        // cadence l'état de la touche
        self.clock_keystate(u32::from(self.keyon_live != 0), regs);
        self.keyon_live &= !(1u8 << KEYON_CSM);

        // actif jusqu'à ce qu'on soit silencieux après le release
        self.env_state != EG_RELEASE || (self.env_attenuation as u32) < EG_QUIET
    }

    /// Fonction d'horloge principale.
    pub fn clock(&mut self, env_counter: u32, lfo_raw_pm: i32, regs: &OpnRegisters) {
        // cadence l'état SSG-EG (OPN/OPNA)
        if regs.op_ssg_eg_enable(self.opoffs) != 0 {
            self.clock_ssg_eg_state(regs);
        } else {
            self.ssg_inverted = 0;
        }

        // cadence l'enveloppe sur un cycle d'enveloppe ; env_counter est une valeur x.2
        if bitfield(env_counter, 0, 2) == 0 {
            self.clock_envelope(env_counter >> 2, regs);
        }

        // cadence la phase
        self.clock_phase(lfo_raw_pm, regs);
    }

    /// Phase courante (10 bits de poids faible = période complète du sinus).
    pub fn phase(&self) -> u32 {
        self.phase >> 10
    }

    /// Volume signé 14 bits pour une phase (modulation incluse) et un offset AM.
    pub fn compute_volume(&self, phase: u32, am_offset: u32, regs: &OpnRegisters) -> i32 {
        // sortie rapide si l'enveloppe est effectivement coupée
        if self.env_attenuation as u32 > EG_QUIET {
            return 0;
        }

        // valeur absolue du sinus, en atténuation 4.8
        let sin_attenuation =
            regs.waveform()[(phase as usize) & (crate::opn_regs::WAVEFORM_LENGTH - 1)] as u32;

        // atténuation de l'enveloppe 4.6 décalée en 4.8
        let env_attenuation = self.envelope_attenuation(am_offset, regs) << 2;

        // valeur 5.8 -> volume linéaire 13 bits
        let result =
            attenuation_to_volume((sin_attenuation & 0x7fff).wrapping_add(env_attenuation)) as i32;

        // négatif dans la partie négative du sinus
        if bitfield(sin_attenuation, 15, 1) != 0 {
            -result
        } else {
            result
        }
    }

    /// Volume bruit signé 14 bits (canal bruit OPM ; sans effet sur OPN).
    pub fn compute_noise_volume(&self, am_offset: u32, regs: &OpnRegisters) -> i32 {
        let result = ((self.envelope_attenuation(am_offset, regs) ^ 0x3ff) << 1) as i32;
        if bitfield(regs.noise_state(), 0, 1) != 0 {
            -result
        } else {
            result
        }
    }

    /// Signale un événement key on/off.
    pub fn keyonoff(&mut self, on: u32, keyon_type: u32) {
        self.keyon_live = ((self.keyon_live as u32 & !(1u32 << keyon_type))
            | (bitfield(on, 0, 1) << keyon_type)) as u8;
    }

    /// Démarre l'attaque (key on, ou fin de cycle SSG-EG).
    fn start_attack(&mut self, is_restart: bool, regs: &OpnRegisters) {
        // rien à faire si déjà en attaque
        if self.env_state == EG_ATTACK {
            return;
        }
        self.env_state = EG_ATTACK;

        // en général non inversé au départ, sauf SSG-EG avec un mode inversé ;
        // sur un restart on n'y touche pas (géré par clock_ssg_eg_state)
        if !is_restart {
            self.ssg_inverted = (regs.op_ssg_eg_enable(self.opoffs)
                & bitfield(regs.op_ssg_eg_mode(self.opoffs), 2, 1))
                as u8;
        }

        // phase remise à 0 sur un key on (pas sur un restart SSG-EG)
        if !is_restart {
            self.phase = 0;
        }

        // taux d'attaque >= 62 : atténuation immédiatement au maximum
        if self.cache.eg_rate[EG_ATTACK] >= 62 {
            self.env_attenuation = 0;
        }
    }

    /// Démarre le release (key off).
    fn start_release(&mut self) {
        // rien à faire si déjà en release
        if self.env_state >= EG_RELEASE {
            return;
        }
        self.env_state = EG_RELEASE;

        // atténuation inversée par SSG-EG : on fige l'atténuation inversée comme point de départ
        if self.ssg_inverted != 0 {
            self.env_attenuation =
                ((0x200u32.wrapping_sub(self.env_attenuation as u32)) & 0x3ff) as u16;
            self.ssg_inverted = 0;
        }
    }

    /// Aligne l'état de la touche sur l'état entrant.
    fn clock_keystate(&mut self, keystate: u32, regs: &OpnRegisters) {
        debug_assert!(keystate == 0 || keystate == 1);

        // la touche a-t-elle changé ?
        if (keystate ^ self.key_state as u32) != 0 {
            self.key_state = keystate as u8;

            if keystate != 0 {
                // key on : démarre l'attaque
                self.start_attack(false, regs);
            } else {
                // key off : démarre le release
                self.start_release();
            }
        }
    }

    /// Cadence l'état SSG-EG ; à n'appeler que si SSG-EG est activé.
    fn clock_ssg_eg_state(&mut self, regs: &OpnRegisters) {
        // le travail n'a lieu qu'une fois l'atténuation passée au-dessus de 0x200
        if bitfield(self.env_attenuation as u32, 9, 1) == 0 {
            return;
        }

        // 8 modes SSG-EG :
        //    000: répète normalement
        //    001: une fois, reste bas
        //    010: répète en alternant inversé/non inversé
        //    011: une fois, reste haut
        //    100: répète normalement, inversé
        //    101: une fois inversé, reste bas
        //    110: répète inversé en alternant
        //    111: une fois inversé, reste haut
        let mode = regs.op_ssg_eg_mode(self.opoffs);

        if bitfield(mode, 0, 1) != 0 {
            // modes "hold" (1/3/5/7) : drapeau inversé = état final (0 pour 1/7, 1 pour 3/5)
            self.ssg_inverted = (bitfield(mode, 2, 1) ^ bitfield(mode, 1, 1)) as u8;

            // en hold, force l'atténuation attendue une fois passée la phase d'attaque
            if self.env_state != EG_ATTACK {
                self.env_attenuation = if self.ssg_inverted != 0 { 0x200 } else { 0x3ff };
            }
        } else {
            // modes continus (0/2/4/6) : bascule l'inversion en mode alterné (même en attaque)
            self.ssg_inverted ^= bitfield(mode, 1, 1) as u8;

            // relance l'attaque en decay/sustain
            if self.env_state == EG_DECAY || self.env_state == EG_SUSTAIN {
                self.start_attack(true, regs);
            }

            // la phase est remise à 0 dans les modes 0/4
            if bitfield(mode, 1, 1) == 0 {
                self.phase = 0;
            }
        }

        // dans tous les modes, une fois en release l'atténuation est forcée au maximum
        if self.env_state == EG_RELEASE {
            self.env_attenuation = 0x3ff;
        }
    }

    /// Cadence l'enveloppe selon le compteur donné.
    fn clock_envelope(&mut self, env_counter: u32, regs: &OpnRegisters) {
        // transitions attaque -> decay
        if self.env_state == EG_ATTACK && self.env_attenuation == 0 {
            self.env_state = EG_DECAY;
        }

        // transitions decay -> sustain, immédiatement après la précédente (niveau de sustain à 0)
        if self.env_state == EG_DECAY && self.env_attenuation as u32 >= self.cache.eg_sustain {
            self.env_state = EG_SUSTAIN;
        }

        // taux 6 bits depuis le cache
        let rate = self.cache.eg_rate[self.env_state] as u32;

        // décalage de taux : rend env_counter en virgule fixe 5.11
        let rate_shift = rate >> 2;
        let env_counter = env_counter.wrapping_shl(rate_shift);

        // partie fractionnaire non nulle : ce n'est pas le moment de cadencer
        if bitfield(env_counter, 0, 11) != 0 {
            return;
        }

        // incrément selon la partie entière de env_counter
        let relevant_bits = bitfield(
            env_counter,
            if rate_shift <= 11 {
                11
            } else {
                rate_shift as i32
            },
            3,
        );
        let increment = attenuation_increment(rate, relevant_bits);

        // l'attaque est la seule qui augmente (l'atténuation décroît)
        if self.env_state == EG_ATTACK {
            // taux 62/63 : pas d'incrément s'ils sont changés après le key on initial
            if rate < 62 {
                let att = self.env_attenuation as u32;
                // PORT-NOTE : ~m_env_attenuation est un int 32 bits ; produit modulo 2^32 puis >> 4 non signé,
                // le résultat est ajouté à un uint16 (troncature).
                let delta = (!att).wrapping_mul(increment) >> 4;
                self.env_attenuation = self.env_attenuation.wrapping_add(delta as u16);
            }
        } else {
            // sans SSG-EG : applique simplement l'incrément
            if regs.op_ssg_eg_enable(self.opoffs) == 0 {
                self.env_attenuation = self.env_attenuation.wrapping_add(increment as u16);
            } else if self.env_attenuation < 0x200 {
                // SSG-EG : seulement sous le point milieu, et à 4x
                self.env_attenuation = self.env_attenuation.wrapping_add((4 * increment) as u16);
            }

            // borne l'atténuation finale
            if self.env_attenuation >= 0x400 {
                self.env_attenuation = 0x3ff;
            }
        }
    }

    /// Cadence la phase 10.10 (logique OPN vérifiée contre le générateur de phase Nuked).
    fn clock_phase(&mut self, lfo_raw_pm: i32, regs: &OpnRegisters) {
        // lit le cache, ou recalcule si la PM est active
        let mut phase_step = self.cache.phase_step;
        if phase_step == OpdataCache::PHASE_STEP_DYNAMIC {
            phase_step = regs.compute_phase_step(self.choffs, self.opoffs, &self.cache, lfo_raw_pm);
        }

        // applique le pas à la phase courante
        self.phase = self.phase.wrapping_add(phase_step);
    }

    /// Atténuation effective de l'enveloppe.
    fn envelope_attenuation(&self, am_offset: u32, regs: &OpnRegisters) -> u32 {
        let mut result = (self.env_attenuation as u32) >> self.cache.eg_shift;

        // inverse si nécessaire à cause de SSG-EG
        if self.ssg_inverted != 0 {
            result = (0x200u32.wrapping_sub(result)) & 0x3ff;
        }

        // ajoute la modulation LFO AM
        if regs.op_lfo_am_enable(self.opoffs) != 0 {
            result = result.wrapping_add(am_offset);
        }

        // ajoute total level et KSL du cache
        result = result.wrapping_add(self.cache.total_level);

        // borne au maximum
        result.min(0x3ff)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fm_tables::abs_sin_attenuation;

    fn setup() -> (OpnRegisters, FmOperator) {
        let mut regs = OpnRegisters::new();
        regs.reset();
        (regs, FmOperator::new(0))
    }

    fn w(regs: &mut OpnRegisters, index: u16, data: u8) {
        let (mut ch, mut mask) = (0, 0);
        regs.write(index, data, &mut ch, &mut mask);
    }

    #[test]
    fn reset_state() {
        let (_, mut op) = setup();
        op.phase = 5;
        op.env_attenuation = 1;
        op.reset();
        assert_eq!(op.debug_eg_state(), EG_RELEASE);
        assert_eq!(op.debug_eg_attenuation(), 0x3ff);
        assert_eq!(op.phase(), 0);
    }

    #[test]
    fn instant_attack_with_ar_31() {
        let (mut regs, mut op) = setup();
        w(&mut regs, 0x50, 0x1F); // AR = 31, KSR 0
        op.keyonoff(1, KEYON_NORMAL);
        assert!(op.prepare(&regs));
        assert_eq!(op.debug_eg_state(), EG_ATTACK);
        assert_eq!(op.debug_eg_attenuation(), 0);
        op.clock(0, 0, &regs);
        assert_eq!(op.debug_eg_attenuation(), 0);
        // attaque -> decay -> (SL = 0) sustain dès le premier cycle d'enveloppe
        assert_ne!(op.debug_eg_state(), EG_ATTACK);
    }

    #[test]
    fn key_off_goes_to_release_and_attenuation_grows() {
        let (mut regs, mut op) = setup();
        w(&mut regs, 0x50, 0x1F);
        w(&mut regs, 0x80, 0x08); // RR = 8
        op.keyonoff(1, KEYON_NORMAL);
        op.prepare(&regs);
        op.clock(0, 0, &regs);
        op.keyonoff(0, KEYON_NORMAL);
        op.prepare(&regs);
        assert_eq!(op.debug_eg_state(), EG_RELEASE);
        let start = op.debug_eg_attenuation();
        for k in 1..=4096u32 {
            op.clock(k << 2, 0, &regs);
        }
        assert!(op.debug_eg_attenuation() > start);
        assert!(op.debug_eg_attenuation() <= 0x3ff);
    }

    #[test]
    fn compute_volume_matches_tables() {
        let (regs, mut op) = setup();
        op.env_attenuation = 0;
        op.env_state = EG_SUSTAIN;
        let expected = attenuation_to_volume(abs_sin_attenuation(0x100)) as i32;
        assert_eq!(op.compute_volume(0x100, 0, &regs), expected);
        // seconde moitié de la période : signe négatif
        let expected = attenuation_to_volume(abs_sin_attenuation(0x300)) as i32;
        assert_eq!(op.compute_volume(0x300, 0, &regs), -expected);
        // enveloppe coupée : 0
        op.env_attenuation = 0x381;
        assert_eq!(op.compute_volume(0x100, 0, &regs), 0);
    }

    #[test]
    fn ssg_eg_mode_8_repeats_with_attack_restart() {
        let (mut regs, mut op) = setup();
        w(&mut regs, 0x90, 0x08); // SSG-EG activé, mode 0 (répète normalement)
        w(&mut regs, 0x50, 0x10);
        op.keyonoff(1, KEYON_NORMAL);
        op.prepare(&regs);
        assert_eq!(op.debug_eg_state(), EG_ATTACK);
        // atténuation sous 0x200 : rien ne se passe
        op.env_state = EG_DECAY;
        op.env_attenuation = 0x1ff;
        op.phase = 0x1234 << 10;
        op.clock_ssg_eg_state(&regs);
        assert_eq!(op.debug_eg_state(), EG_DECAY);
        // au-dessus de 0x200 : relance l'attaque, phase remise à 0
        op.env_attenuation = 0x200;
        op.clock_ssg_eg_state(&regs);
        assert_eq!(op.debug_eg_state(), EG_ATTACK);
        assert_eq!(op.phase, 0);
        assert_eq!(op.debug_ssg_inverted(), 0);
    }

    #[test]
    fn ssg_eg_alternating_and_hold_modes() {
        let (mut regs, mut op) = setup();
        // mode 2 (alterné) : inversion basculée, phase conservée
        w(&mut regs, 0x90, 0x0A);
        op.env_state = EG_SUSTAIN;
        op.env_attenuation = 0x300;
        op.phase = 7 << 10;
        op.clock_ssg_eg_state(&regs);
        assert_eq!(op.debug_ssg_inverted(), 1);
        assert_eq!(op.debug_eg_state(), EG_ATTACK);
        assert_eq!(op.phase, 7 << 10);
        // mode 3 (une fois, reste haut) : inversé, atténuation 0x200
        w(&mut regs, 0x90, 0x0B);
        op.env_state = EG_SUSTAIN;
        op.env_attenuation = 0x300;
        op.clock_ssg_eg_state(&regs);
        assert_eq!(op.debug_ssg_inverted(), 1);
        assert_eq!(op.debug_eg_attenuation(), 0x200);
        // mode 1 (une fois, reste bas) : non inversé, atténuation 0x3ff
        w(&mut regs, 0x90, 0x09);
        op.env_state = EG_SUSTAIN;
        op.env_attenuation = 0x300;
        op.clock_ssg_eg_state(&regs);
        assert_eq!(op.debug_ssg_inverted(), 0);
        assert_eq!(op.debug_eg_attenuation(), 0x3ff);
    }

    #[test]
    fn start_release_snaps_inverted_attenuation() {
        let (_, mut op) = setup();
        op.env_state = EG_SUSTAIN;
        op.ssg_inverted = 1;
        op.env_attenuation = 0x100;
        op.start_release();
        assert_eq!(op.debug_eg_state(), EG_RELEASE);
        assert_eq!(op.debug_eg_attenuation(), 0x100);
        assert_eq!(op.debug_ssg_inverted(), 0);
    }

    #[test]
    fn phase_advances_by_cached_step() {
        let (mut regs, mut op) = setup();
        w(&mut regs, 0xA4, 0x22);
        w(&mut regs, 0xA0, 0x34);
        w(&mut regs, 0x30, 0x01);
        op.prepare(&regs);
        let step = op.debug_cache().phase_step;
        op.clock(0, 0, &regs);
        assert_eq!(op.phase, step);
    }
}
