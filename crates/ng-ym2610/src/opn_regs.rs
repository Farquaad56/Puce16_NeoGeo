// license:BSD-3-Clause
// copyright-holders:Aaron Giles
//! Registres FM OPN : `opn_registers_base<true>` (OPNA, utilisé par le YM2610) de ymfm.
//! Sources : ref/ymfm/src/ymfm_fm.h l.63-160, ymfm_opn.h l.48-330, ymfm_opn.cpp l.42-440.
//!
//! PORT-NOTE : un seul type concret (IsOpnA = true). `log_keyon` et `save_restore` ne sont pas portés
//! (les savestates sont gérés par T604). `opdata_cache::waveform` (pointeur vers la table de sinus) est
//! remplacé par l'accesseur `OpnRegisters::waveform()`.

use crate::common::{bit, bitfield, EG_ATTACK, EG_DECAY, EG_RELEASE, EG_STATES, EG_SUSTAIN};
use crate::fm_tables::{abs_sin_attenuation, detune_adjustment, opn_lfo_pm_phase_adjustment};

/// Données calculées une fois au début de l'horloge et constantes pendant la génération.
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OpdataCache {
    /// Pas de phase, ou `PHASE_STEP_DYNAMIC` si la PM est active.
    pub phase_step: u32,
    /// Total level * 8 + KSL.
    pub total_level: u32,
    /// Valeur brute block/fréquence (sert à calculer phase_step).
    pub block_freq: u32,
    /// Détune (sert à calculer phase_step).
    pub detune: i32,
    /// Multiple (x.1, sert à calculer phase_step).
    pub multiple: u32,
    /// Niveau de sustain, décalé vers les valeurs d'enveloppe.
    pub eg_sustain: u32,
    /// Taux d'enveloppe, KSR inclus.
    pub eg_rate: [u8; EG_STATES],
    /// Décalage d'enveloppe.
    pub eg_shift: u8,
}

impl OpdataCache {
    /// Valeur de `phase_step` demandant un recalcul à chaque échantillon (changements de PM LFO).
    pub const PHASE_STEP_DYNAMIC: u32 = 1;
}

/// Taille d'une forme d'onde sinus complète.
pub const WAVEFORM_LENGTH: usize = 0x400;
/// Valeur retournée par `write` pour les canaux rythme (OPL seulement).
pub const RHYTHM_CHANNEL: u32 = 0xff;

const LFO_MAX_COUNT: [u8; 8] = [109, 78, 72, 68, 63, 45, 9, 6];

const fn operator_list(o1: u8, o2: u8, o3: u8, o4: u8) -> u32 {
    (o1 as u32) | ((o2 as u32) << 8) | ((o3 as u32) << 16) | ((o4 as u32) << 24)
}

/// Opérateurs de chaque canal (ordre : carrier 1, carrier 2, modulator 1, modulator 2).
const FIXED_MAP: [u32; 6] = [
    operator_list(0, 6, 3, 9),
    operator_list(1, 7, 4, 10),
    operator_list(2, 8, 5, 11),
    operator_list(12, 18, 15, 21),
    operator_list(13, 19, 16, 22),
    operator_list(14, 20, 17, 23),
];

/// KSR appliqué au taux brut : ignoré si le taux brut est 0, plafonné à 63.
fn effective_rate(rawrate: u32, ksr: u32) -> u8 {
    if rawrate == 0 {
        0
    } else {
        (rawrate + ksr).min(63) as u8
    }
}

/// Registres OPNA.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OpnRegisters {
    lfo_counter: u32,
    lfo_am: u8,
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_arr"))]
    regdata: [u8; OpnRegisters::REGISTERS],
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_arr"))]
    waveform: [u16; WAVEFORM_LENGTH],
}

impl Default for OpnRegisters {
    fn default() -> Self {
        Self::new()
    }
}

impl OpnRegisters {
    // constantes
    pub const OUTPUTS: u32 = 2;
    pub const CHANNELS: u32 = 6;
    pub const ALL_CHANNELS: u32 = (1 << Self::CHANNELS) - 1;
    pub const OPERATORS: u32 = Self::CHANNELS * 4;
    pub const WAVEFORMS: u32 = 1;
    pub const REGISTERS: usize = 0x200;
    pub const REG_MODE: u32 = 0x27;
    pub const DEFAULT_PRESCALE: u32 = 6;
    pub const EG_CLOCK_DIVIDER: u32 = 3;
    pub const CSM_TRIGGER_MASK: u32 = 1 << 2;
    pub const STATUS_TIMERA: u8 = 0x01;
    pub const STATUS_TIMERB: u8 = 0x02;
    pub const STATUS_BUSY: u8 = 0x80;
    pub const STATUS_IRQ: u8 = 0;
    pub const DYNAMIC_OPS: bool = false;
    pub const EG_HAS_DEPRESS: bool = false;
    pub const EG_HAS_REVERB: bool = false;
    pub const EG_HAS_SSG: bool = true;
    pub const MODULATOR_DELAY: bool = false;

    /// Constructeur : crée la forme d'onde.
    pub fn new() -> Self {
        let mut waveform = [0u16; WAVEFORM_LENGTH];
        for (index, w) in waveform.iter_mut().enumerate() {
            let index = index as u32;
            *w = (abs_sin_attenuation(index) | (bitfield(index, 9, 1) << 15)) as u16;
        }
        OpnRegisters {
            lfo_counter: 0,
            lfo_am: 0,
            regdata: [0; Self::REGISTERS],
            waveform,
        }
    }

    /// Retour à l'état initial.
    pub fn reset(&mut self) {
        self.regdata.fill(0);
        // sortie activée sur les deux canaux par défaut
        for i in [0xb4, 0xb5, 0xb6, 0x1b4, 0x1b5, 0x1b6] {
            self.regdata[i] = 0xc0;
        }
    }

    /// Table de sinus (WAVEFORMS = 1).
    pub fn waveform(&self) -> &[u16; WAVEFORM_LENGTH] {
        &self.waveform
    }

    /// Numéro de canal -> décalage de registre.
    pub const fn channel_offset(chnum: u32) -> u32 {
        (chnum % 3) + 0x100 * (chnum / 3)
    }

    /// Numéro d'opérateur -> décalage de registre.
    pub const fn operator_offset(opnum: u32) -> u32 {
        (opnum % 12) + ((opnum % 12) / 3) + 0x100 * (opnum / 12)
    }

    /// Opérateurs de chaque canal (fixe pour OPN).
    pub fn operator_map(&self) -> [u32; 6] {
        FIXED_MAP
    }

    /// Lecture d'un registre.
    pub fn read(&self, index: u16) -> u8 {
        self.regdata[index as usize]
    }

    /// Écriture dans le tableau de registres ; retourne true pour un key on/off
    /// (`channel` et `opmask` sont alors renseignés).
    pub fn write(&mut self, index: u16, data: u8, channel: &mut u32, opmask: &mut u32) -> bool {
        debug_assert!((index as usize) < Self::REGISTERS);
        let idx = index as usize;

        // 0xa0-af / 0x1a0-af : paires verrouillées ; les registres 0xb8-bf servent de verrou temporaire
        if (index & 0xf0) == 0xa0 {
            if bitfield(index as u32, 0, 2) == 3 {
                return false;
            }
            let latchindex = 0xb8 | bitfield(index as u32, 3, 1) as usize;
            if bitfield(index as u32, 2, 1) != 0 {
                // l'écriture de la moitié haute ne fait que verrouiller (6 bits utiles)
                self.regdata[latchindex] = data & 0x3f;
            } else {
                // l'écriture de la moitié basse applique aussi le verrou
                self.regdata[idx] = data;
                self.regdata[idx | 4] = self.regdata[latchindex];
            }
            return false;
        } else if (index & 0xf8) == 0xb8 {
            // registres 0xb8-0xbf : usage interne
            return false;
        }

        // tout le reste est normal
        self.regdata[idx] = data;

        // écriture du registre key on
        if index == 0x28 {
            *channel = bitfield(data as u32, 0, 2);
            if *channel == 3 {
                return false;
            }
            *channel += bitfield(data as u32, 2, 1) * 3;
            *opmask = bitfield(data as u32, 4, 4);
            return true;
        }
        false
    }

    /// Cadence le LFO (pas de bruit sur OPN) ; retourne la valeur PM du LFO.
    pub fn clock_noise_and_lfo(&mut self) -> i32 {
        // LFO désactivé : sortie rapide à 0 ; la position 0 donne une valeur AM de 0x3f
        if self.lfo_enable() == 0 {
            self.lfo_counter = 0;
            self.lfo_am = 0x3f;
            return 0;
        }

        let subcount = (self.lfo_counter as u8) as u32;
        self.lfo_counter = self.lfo_counter.wrapping_add(1);

        // au franchissement du diviseur on ajoute de quoi le remettre à zéro (décalage d'un cran voulu)
        if subcount >= LFO_MAX_COUNT[self.lfo_rate() as usize] as u32 {
            self.lfo_counter = self
                .lfo_counter
                .wrapping_add(0x101u32.wrapping_sub(subcount));
        }

        // valeur AM sur 7 bits à partir du bit 8 : on prend les 6 bas
        self.lfo_am = bitfield(self.lfo_counter, 8, 6) as u8;

        // la première moitié de la période AM (bit 6 == 0) est inversée
        if bitfield(self.lfo_counter, 8 + 6, 1) == 0 {
            self.lfo_am ^= 0x3f;
        }

        // valeur PM sur 5 bits à partir du bit 10 : on prend les 3 bas
        let mut pm = bitfield(self.lfo_counter, 10, 3) as i32;

        // PM réfléchie selon le bit 3
        if bitfield(self.lfo_counter, 10 + 3, 1) != 0 {
            pm ^= 7;
        }

        // PM niée selon le bit 4
        if bitfield(self.lfo_counter, 10 + 4, 1) != 0 {
            -pm
        } else {
            pm
        }
    }

    /// Remet le LFO à zéro.
    pub fn reset_lfo(&mut self) {
        self.lfo_counter = 0;
    }

    /// Décalage AM du LFO pour le canal donné.
    pub fn lfo_am_offset(&self, choffs: u32) -> u32 {
        // décalage de sensibilité AM : [7, 3, 1, 0]
        let am_shift = (1u32 << (self.ch_lfo_am_sens(choffs) ^ 3)) - 1;
        // la valeur AM brute va de 0 à 3F : on la double (7 bits) avant le décalage final
        ((self.lfo_am as u32) << 1) >> am_shift
    }

    /// État du bruit (absent sur OPN).
    pub fn noise_state(&self) -> u32 {
        0
    }

    /// Remplit le cache de l'opérateur avec des données précalculées.
    pub fn cache_operator_data(&self, choffs: u32, opoffs: u32, cache: &mut OpdataCache) {
        // fréquence du canal
        cache.block_freq = self.ch_block_freq(choffs);
        let mut block_freq = cache.block_freq;

        // mode multi-fréquence et canal 2 : on prend l'une des fréquences spéciales
        if self.multi_freq() != 0 && choffs == 2 {
            if opoffs == 2 {
                cache.block_freq = self.multi_block_freq(1);
                block_freq = cache.block_freq;
            } else if opoffs == 10 {
                cache.block_freq = self.multi_block_freq(2);
                block_freq = cache.block_freq;
            } else if opoffs == 6 {
                cache.block_freq = self.multi_block_freq(0);
                block_freq = cache.block_freq;
            }
        }

        // key code de 5 bits : les 4 bits hauts plus une formule pour le dernier bit
        let mut keycode = bitfield(block_freq, 10, 4) << 1;

        // (F11 & (F10 | F9 | F8)) | (!F11 & F10 & F9 & F8), lu dans une constante 16 bits
        keycode |= bit(0xfe80, bitfield(block_freq, 7, 4) as i32);

        // ajustement de détune
        cache.detune = detune_adjustment(self.op_detune(opoffs), keycode);

        // multiple en x.1 (0 signifie 0,5)
        cache.multiple = self.op_multiple(opoffs) * 2;
        if cache.multiple == 0 {
            cache.multiple = 1;
        }

        // pas de phase, ou PHASE_STEP_DYNAMIC si la PM est active (dépend de block_freq, detune, multiple)
        if self.lfo_enable() == 0 || self.ch_lfo_pm_sens(choffs) == 0 {
            cache.phase_step = self.compute_phase_step(choffs, opoffs, cache, 0);
        } else {
            cache.phase_step = OpdataCache::PHASE_STEP_DYNAMIC;
        }

        // total level multiplié par 8
        cache.total_level = self.op_total_level(opoffs) << 3;

        // niveau de sustain sur 4 bits, mais 15 signifie 31 : 5 bits effectifs
        cache.eg_sustain = self.op_sustain_level(opoffs);
        cache.eg_sustain |= (cache.eg_sustain + 1) & 0x10;
        cache.eg_sustain <<= 5;

        // ajustement KSR des taux d'enveloppe
        let ksrval = keycode >> (self.op_ksr(opoffs) ^ 3);
        cache.eg_rate[EG_ATTACK] = effective_rate(self.op_attack_rate(opoffs) * 2, ksrval);
        cache.eg_rate[EG_DECAY] = effective_rate(self.op_decay_rate(opoffs) * 2, ksrval);
        cache.eg_rate[EG_SUSTAIN] = effective_rate(self.op_sustain_rate(opoffs) * 2, ksrval);
        cache.eg_rate[EG_RELEASE] = effective_rate(self.op_release_rate(opoffs) * 4 + 2, ksrval);
    }

    /// Calcule le pas de phase pour une valeur PM donnée.
    pub fn compute_phase_step(
        &self,
        choffs: u32,
        _opoffs: u32,
        cache: &OpdataCache,
        lfo_raw_pm: i32,
    ) -> u32 {
        // l'OPN n'a qu'un paramètre de détune et utilise des FNUM au lieu de key codes
        // numéro de fréquence (11 bits bas de block_freq)
        let mut fnum = bitfield(cache.block_freq, 0, 11) << 1;

        // sensibilité PM non nulle : calcul de l'ajustement
        let pm_sensitivity = self.ch_lfo_pm_sens(choffs);
        if pm_sensitivity != 0 {
            // ajustement de phase d'après les 7 bits hauts de FNUM et la profondeur PM
            fnum = fnum.wrapping_add(opn_lfo_pm_phase_adjustment(
                bitfield(cache.block_freq, 4, 7),
                pm_sensitivity,
                lfo_raw_pm,
            ) as u32);
            // fnum reste sur 12 bits
            fnum &= 0xfff;
        }

        // décalage de bloc pour obtenir le pas de phase
        let block = bitfield(cache.block_freq, 11, 3);
        let mut phase_step = (fnum << block) >> 2;

        // détune d'après le key code
        phase_step = phase_step.wrapping_add(cache.detune as u32);

        // limité à 17 bits au cas où le détune déborde
        phase_step &= 0x1ffff;

        // multiplicateur de fréquence (en cache comme valeur x.1)
        phase_step.wrapping_mul(cache.multiple) >> 1
    }

    // ---- accesseurs communs (valeurs par défaut de fm_registers_base) ----
    pub fn status_mask(&self) -> u32 {
        0
    }
    pub fn irq_reset(&self) -> u32 {
        0
    }
    pub fn noise_enable(&self) -> u32 {
        0
    }
    pub fn rhythm_enable(&self) -> u32 {
        0
    }

    // ---- registres système ----
    pub fn test(&self) -> u32 {
        self.byte(0x21, 0, 8, 0)
    }
    pub fn lfo_enable(&self) -> u32 {
        self.byte(0x22, 3, 1, 0)
    }
    pub fn lfo_rate(&self) -> u32 {
        self.byte(0x22, 0, 3, 0)
    }
    pub fn timer_a_value(&self) -> u32 {
        self.word(0x24, 0, 8, 0x25, 0, 2, 0)
    }
    pub fn timer_b_value(&self) -> u32 {
        self.byte(0x26, 0, 8, 0)
    }
    pub fn csm(&self) -> u32 {
        (self.byte(0x27, 6, 2, 0) == 2) as u32
    }
    pub fn multi_freq(&self) -> u32 {
        (self.byte(0x27, 6, 2, 0) != 0) as u32
    }
    pub fn reset_timer_b(&self) -> u32 {
        self.byte(0x27, 5, 1, 0)
    }
    pub fn reset_timer_a(&self) -> u32 {
        self.byte(0x27, 4, 1, 0)
    }
    pub fn enable_timer_b(&self) -> u32 {
        self.byte(0x27, 3, 1, 0)
    }
    pub fn enable_timer_a(&self) -> u32 {
        self.byte(0x27, 2, 1, 0)
    }
    pub fn load_timer_b(&self) -> u32 {
        self.byte(0x27, 1, 1, 0)
    }
    pub fn load_timer_a(&self) -> u32 {
        self.byte(0x27, 0, 1, 0)
    }
    pub fn multi_block_freq(&self, num: u32) -> u32 {
        self.word(0xac, 0, 6, 0xa8, 0, 8, num)
    }

    // ---- registres par canal ----
    pub fn ch_block_freq(&self, choffs: u32) -> u32 {
        self.word(0xa4, 0, 6, 0xa0, 0, 8, choffs)
    }
    pub fn ch_feedback(&self, choffs: u32) -> u32 {
        self.byte(0xb0, 3, 3, choffs)
    }
    pub fn ch_algorithm(&self, choffs: u32) -> u32 {
        self.byte(0xb0, 0, 3, choffs)
    }
    pub fn ch_output_any(&self, choffs: u32) -> u32 {
        self.byte(0xb4, 6, 2, choffs)
    }
    pub fn ch_output_0(&self, choffs: u32) -> u32 {
        self.byte(0xb4, 7, 1, choffs)
    }
    pub fn ch_output_1(&self, choffs: u32) -> u32 {
        self.byte(0xb4, 6, 1, choffs)
    }
    pub fn ch_output_2(&self, _choffs: u32) -> u32 {
        0
    }
    pub fn ch_output_3(&self, _choffs: u32) -> u32 {
        0
    }
    pub fn ch_lfo_am_sens(&self, choffs: u32) -> u32 {
        self.byte(0xb4, 4, 2, choffs)
    }
    pub fn ch_lfo_pm_sens(&self, choffs: u32) -> u32 {
        self.byte(0xb4, 0, 3, choffs)
    }

    // ---- registres par opérateur ----
    pub fn op_detune(&self, opoffs: u32) -> u32 {
        self.byte(0x30, 4, 3, opoffs)
    }
    pub fn op_multiple(&self, opoffs: u32) -> u32 {
        self.byte(0x30, 0, 4, opoffs)
    }
    pub fn op_total_level(&self, opoffs: u32) -> u32 {
        self.byte(0x40, 0, 7, opoffs)
    }
    pub fn op_ksr(&self, opoffs: u32) -> u32 {
        self.byte(0x50, 6, 2, opoffs)
    }
    pub fn op_attack_rate(&self, opoffs: u32) -> u32 {
        self.byte(0x50, 0, 5, opoffs)
    }
    pub fn op_decay_rate(&self, opoffs: u32) -> u32 {
        self.byte(0x60, 0, 5, opoffs)
    }
    pub fn op_lfo_am_enable(&self, opoffs: u32) -> u32 {
        self.byte(0x60, 7, 1, opoffs)
    }
    pub fn op_sustain_rate(&self, opoffs: u32) -> u32 {
        self.byte(0x70, 0, 5, opoffs)
    }
    pub fn op_sustain_level(&self, opoffs: u32) -> u32 {
        self.byte(0x80, 4, 4, opoffs)
    }
    pub fn op_release_rate(&self, opoffs: u32) -> u32 {
        self.byte(0x80, 0, 4, opoffs)
    }
    pub fn op_ssg_eg_enable(&self, opoffs: u32) -> u32 {
        self.byte(0x90, 3, 1, opoffs)
    }
    pub fn op_ssg_eg_mode(&self, opoffs: u32) -> u32 {
        self.byte(0x90, 0, 3, opoffs)
    }

    /// Champ de bits extrait d'un octet.
    fn byte(&self, offset: u32, start: u32, count: u32, extra_offset: u32) -> u32 {
        bitfield(
            self.regdata[(offset + extra_offset) as usize] as u32,
            start as i32,
            count as i32,
        )
    }

    /// Champ de bits extrait d'une paire d'octets, bits de poids fort en premier.
    #[allow(clippy::too_many_arguments)]
    fn word(
        &self,
        offset1: u32,
        start1: u32,
        count1: u32,
        offset2: u32,
        start2: u32,
        count2: u32,
        extra_offset: u32,
    ) -> u32 {
        (self.byte(offset1, start1, count1, extra_offset) << count2)
            | self.byte(offset2, start2, count2, extra_offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn regs() -> OpnRegisters {
        let mut r = OpnRegisters::new();
        r.reset();
        r
    }

    #[test]
    fn reset_clears_and_enables_outputs() {
        let mut r = OpnRegisters::new();
        let (mut ch, mut mask) = (0, 0);
        r.write(0x30, 0x71, &mut ch, &mut mask);
        r.reset();
        assert_eq!(r.read(0x30), 0);
        for i in [0xb4u16, 0xb5, 0xb6, 0x1b4, 0x1b5, 0x1b6] {
            assert_eq!(r.read(i), 0xc0);
        }
        assert_eq!(r.read(0xb0), 0);
        assert_eq!(r.ch_output_0(0), 1);
        assert_eq!(r.ch_output_1(0), 1);
    }

    #[test]
    fn waveform_has_sign_bit_in_second_half() {
        let r = regs();
        assert_eq!(r.waveform()[0], 0x859);
        assert_eq!(r.waveform()[0x200], 0x859 | 0x8000);
        assert_eq!(r.waveform()[0x100], 0);
    }

    #[test]
    fn key_on_write() {
        let mut r = regs();
        let (mut ch, mut mask) = (99, 99);
        assert!(r.write(0x28, 0xF1, &mut ch, &mut mask));
        assert_eq!((ch, mask), (1, 0xF));
        assert!(r.write(0x28, 0x25, &mut ch, &mut mask)); // bit 2 : canal 3 + (5 & 3 = 1) => 4
        assert_eq!((ch, mask), (4, 2));
        assert!(!r.write(0x28, 0xF3, &mut ch, &mut mask)); // canal 3 invalide
    }

    #[test]
    fn frequency_latch_pair() {
        let mut r = regs();
        let (mut ch, mut mask) = (0, 0);
        assert!(!r.write(0xA4, 0x22, &mut ch, &mut mask)); // haut : verrou seulement
        assert_eq!(r.ch_block_freq(0), 0);
        assert!(!r.write(0xA0, 0x34, &mut ch, &mut mask)); // bas : applique le verrou
        assert_eq!(r.ch_block_freq(0), (0x22 << 8) | 0x34);
        assert!(!r.write(0xA7, 0xFF, &mut ch, &mut mask)); // index & 3 == 3 : ignoré
                                                           // seconde moitié du jeu de registres (0x1A4/0x1A0) : canal 3
        r.write(0x1A4, 0x0B, &mut ch, &mut mask);
        r.write(0x1A0, 0x56, &mut ch, &mut mask);
        assert_eq!(
            r.ch_block_freq(OpnRegisters::channel_offset(3)),
            (0x0B << 8) | 0x56
        );
    }

    #[test]
    fn operator_cache_fields() {
        let mut r = regs();
        let (mut ch, mut mask) = (0, 0);
        r.write(0x30, 0x71, &mut ch, &mut mask); // detune 7, multiple 1
        r.write(0x40, 0x2A, &mut ch, &mut mask); // TL 0x2A
        r.write(0x50, 0x5F, &mut ch, &mut mask); // KSR 1, AR 31
        r.write(0x60, 0x0A, &mut ch, &mut mask); // DR 10
        r.write(0x70, 0x03, &mut ch, &mut mask); // SR 3
        r.write(0x80, 0xFB, &mut ch, &mut mask); // SL 15, RR 11
        r.write(0xA4, 0x22, &mut ch, &mut mask);
        r.write(0xA0, 0x34, &mut ch, &mut mask);
        let mut c = OpdataCache::default();
        r.cache_operator_data(0, 0, &mut c);
        assert_eq!(c.block_freq, 0x2234);
        assert_eq!(c.multiple, 2);
        assert_eq!(c.total_level, 0x2A << 3);
        assert_eq!(c.eg_sustain, 31 << 5);
        // keycode : bits 13-10 de block_freq = 0x8 -> 0x10 ; bit bas : bit (0xfe80 >> 0x4) = (0xfe80>>4)&1 = 0
        let keycode = 0x10;
        assert_eq!(c.detune, -detune_adjustment(3, keycode));
        let ksr = keycode >> (1 ^ 3);
        assert_eq!(c.eg_rate[EG_ATTACK] as u32, (62 + ksr).min(63));
        assert_eq!(c.eg_rate[EG_DECAY] as u32, 20 + ksr);
        assert_eq!(c.eg_rate[EG_SUSTAIN] as u32, 6 + ksr);
        assert_eq!(c.eg_rate[EG_RELEASE] as u32, 46 + ksr);
        // LFO désactivé : phase_step calculé
        assert_ne!(c.phase_step, OpdataCache::PHASE_STEP_DYNAMIC);
        let fnum = (0x234u32 << 1) & 0xfff;
        let step = ((((fnum << 4) >> 2).wrapping_add(c.detune as u32) & 0x1ffff) * c.multiple) >> 1;
        assert_eq!(c.phase_step, step);
    }

    #[test]
    fn pm_active_makes_phase_step_dynamic() {
        let mut r = regs();
        let (mut ch, mut mask) = (0, 0);
        r.write(0x22, 0x08, &mut ch, &mut mask); // LFO enable
        r.write(0xB4, 0xC3, &mut ch, &mut mask); // PM sens 3
        let mut c = OpdataCache::default();
        r.cache_operator_data(0, 0, &mut c);
        assert_eq!(c.phase_step, OpdataCache::PHASE_STEP_DYNAMIC);
    }

    #[test]
    fn lfo_disabled_gives_am_3f_and_zero_pm() {
        let mut r = regs();
        assert_eq!(r.clock_noise_and_lfo(), 0);
        assert_eq!(r.lfo_am_offset(0), (0x3fu32 << 1) >> ((1 << 3) - 1));
    }

    #[test]
    fn lfo_counts_and_wraps() {
        let mut r = regs();
        let (mut ch, mut mask) = (0, 0);
        r.write(0x22, 0x0F, &mut ch, &mut mask); // enable, rate 7 (max count 6)
        let mut pm = Vec::new();
        for _ in 0..40000 {
            pm.push(r.clock_noise_and_lfo());
        }
        assert!(pm.iter().all(|v| (-7..=7).contains(v)));
        assert!(pm.iter().any(|&v| v > 0) && pm.iter().any(|&v| v < 0));
    }

    #[test]
    fn operator_maps_and_offsets() {
        let r = regs();
        assert_eq!(r.operator_map()[0], operator_list(0, 6, 3, 9));
        assert_eq!(r.operator_map()[5], operator_list(14, 20, 17, 23));
        assert_eq!(OpnRegisters::channel_offset(4), 1 + 0x100);
        assert_eq!(OpnRegisters::operator_offset(13), 1 + 0x100);
        assert_eq!(OpnRegisters::operator_offset(11), 11 + 3);
    }
}
