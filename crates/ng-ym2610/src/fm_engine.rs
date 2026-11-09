// license:BSD-3-Clause
// copyright-holders:Aaron Giles
//! Moteur FM : un ensemble d'opérateurs et de canaux formant un coeur FM Yamaha.
//! Porté de ref/ymfm/src/ymfm_fm.h (l.345-463) et ymfm_fm.ipp (l.1180-1592).
//!
//! PORT-NOTE : `fm_engine_base<opn_registers_base<true>>` devient un type concret. L'interface hôte
//! est passée en argument (`intf: &mut dyn YmfmInterface`) aux méthodes qui l'appellent. Dans ymfm,
//! `ymfm_sync_mode_write` et `ymfm_sync_check_interrupts` appellent par défaut directement
//! `engine_mode_write` / `engine_check_interrupts` : ce comportement par défaut est reproduit ici
//! (appel direct), car un trait Rust ne peut pas rappeler le moteur qui le borrow. `save_restore`,
//! le log des écritures, les `output_rhythm_*` et la branche `rhythm_enable` (OPL) ne sont pas portés.
//! `DYNAMIC_OPS` est faux pour l'OPN : `assign_operators` n'est appelé qu'à la construction.

use crate::common::{bitfield, Output, YmfmInterface};
use crate::fm_channel::FmChannel;
use crate::fm_operator::{FmOperator, KEYON_CSM, KEYON_NORMAL};
use crate::opn_regs::OpnRegisters;

const CHANNELS: usize = OpnRegisters::CHANNELS as usize;
const OPERATORS: usize = OpnRegisters::OPERATORS as usize;
const ALL_CHANNELS: u32 = OpnRegisters::ALL_CHANNELS;

#[derive(Clone, Debug)]
pub struct FmEngine {
    /// Compteur d'enveloppe ; les 2 bits bas sont un sous-compteur.
    env_counter: u32,
    /// Registre de statut courant.
    status: u8,
    /// Facteur de prescale (2/3/6).
    clock_prescale: u8,
    /// Masque des bits qui signalent des IRQ.
    irq_mask: u8,
    /// État IRQ courant.
    irq_state: u8,
    /// État de marche des deux timers.
    timer_running: [u8; 2],
    /// 8 bits bas du nombre total d'horloges traitées.
    total_clocks: u8,
    /// Masque des canaux actifs (calculé par prepare).
    active_channels: u32,
    /// Masque des canaux modifiés.
    modified_channels: u32,
    /// Compteur pour les balayages périodiques de prepare.
    prepare_count: u32,
    /// Accès aux registres.
    regs: OpnRegisters,
    channel: [FmChannel; CHANNELS],
    operator: [FmOperator; OPERATORS],
}

impl Default for FmEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl FmEngine {
    pub const OUTPUTS: u32 = OpnRegisters::OUTPUTS;
    pub const CHANNELS: u32 = OpnRegisters::CHANNELS;
    pub const ALL_CHANNELS: u32 = OpnRegisters::ALL_CHANNELS;
    pub const OPERATORS: u32 = OpnRegisters::OPERATORS;
    pub const STATUS_TIMERA: u8 = OpnRegisters::STATUS_TIMERA;
    pub const STATUS_TIMERB: u8 = OpnRegisters::STATUS_TIMERB;
    pub const STATUS_BUSY: u8 = OpnRegisters::STATUS_BUSY;
    pub const STATUS_IRQ: u8 = OpnRegisters::STATUS_IRQ;

    /// Constructeur.
    pub fn new() -> Self {
        let mut engine = FmEngine {
            env_counter: 0,
            status: 0,
            clock_prescale: OpnRegisters::DEFAULT_PRESCALE as u8,
            irq_mask: Self::STATUS_TIMERA | Self::STATUS_TIMERB,
            irq_state: 0,
            timer_running: [0, 0],
            total_clocks: 0,
            active_channels: ALL_CHANNELS,
            modified_channels: ALL_CHANNELS,
            prepare_count: 0,
            regs: OpnRegisters::new(),
            // crée les canaux
            channel: std::array::from_fn(|chnum| {
                FmChannel::new(OpnRegisters::channel_offset(chnum as u32))
            }),
            // crée les opérateurs
            operator: std::array::from_fn(|opnum| {
                FmOperator::new(OpnRegisters::operator_offset(opnum as u32))
            }),
        };

        // affectation initiale des opérateurs
        engine.assign_operators();
        engine
    }

    /// Remet l'état global à zéro.
    pub fn reset(&mut self, intf: &mut dyn YmfmInterface) {
        // remet à zéro tous les bits de statut
        self.set_reset_status(0, 0xff, intf);

        // initialisation propre au type de registres
        self.regs.reset();

        // écrit explicitement le registre de mode car il a des effets de bord
        self.write(OpnRegisters::REG_MODE as u16, 0, intf);

        // remet à zéro les canaux
        for chan in self.channel.iter_mut() {
            chan.reset();
        }

        // remet à zéro les opérateurs
        for op in self.operator.iter_mut() {
            op.reset();
        }
    }

    /// Fonction d'horloge principale : fait avancer tous les canaux d'un pas.
    pub fn clock(&mut self, chanmask: u32) -> u32 {
        // met à jour le compteur d'horloge
        self.total_clocks = self.total_clocks.wrapping_add(1);

        // si quelque chose a été modifié, prépare ; prépare aussi tous les 4k échantillons
        // pour détecter les notes qui se terminent
        // PORT-NOTE : `m_prepare_count++` n'est évalué que si `m_modified_channels == 0` (court-circuit).
        let need_prepare = if self.modified_channels != 0 {
            true
        } else {
            let c = self.prepare_count >= 4096;
            self.prepare_count = self.prepare_count.wrapping_add(1);
            c
        };
        if need_prepare {
            // appelle chaque canal pour qu'il se prépare
            self.active_channels = 0;
            for chnum in 0..CHANNELS {
                if bitfield(chanmask, chnum as i32, 1) != 0
                    && self.channel[chnum].prepare(&mut self.operator, &self.regs)
                {
                    self.active_channels |= 1 << chnum;
                }
            }

            // remet à zéro les canaux modifiés et le compteur de prepare
            self.modified_channels = 0;
            self.prepare_count = 0;
        }

        // diviseur d'horloge d'enveloppe (3 pour l'OPN) : incrémente de 1 et déborde manuellement
        // quand on atteint le diviseur
        self.env_counter = self.env_counter.wrapping_add(1);
        if bitfield(self.env_counter, 0, 2) == OpnRegisters::EG_CLOCK_DIVIDER {
            self.env_counter = self
                .env_counter
                .wrapping_add(4 - OpnRegisters::EG_CLOCK_DIVIDER);
        }

        // cadence le générateur de bruit et le LFO
        let lfo_raw_pm = self.regs.clock_noise_and_lfo();

        // met à jour l'état de tous les canaux et opérateurs
        for chnum in 0..CHANNELS {
            if bitfield(chanmask, chnum as i32, 1) != 0 {
                self.channel[chnum].clock(
                    self.env_counter,
                    lfo_raw_pm,
                    &mut self.operator,
                    &self.regs,
                );
            }
        }

        // retourne le compteur d'enveloppe (sert à cadencer l'ADPCM-A)
        self.env_counter
    }

    /// Calcule la somme des sorties des canaux concernés.
    pub fn output(&mut self, output: &mut Output<2>, rshift: u32, clipmax: i32, mut chanmask: u32) {
        // exclut les canaux inactifs
        chanmask &= self.active_channels;

        // PORT-NOTE : la branche `rhythm_enable` (OPL) est omise : toujours 0 sur OPN.
        // somme sur tous les canaux voulus
        for chnum in 0..CHANNELS {
            if bitfield(chanmask, chnum as i32, 1) != 0 {
                if self.channel[chnum].is4op() {
                    self.channel[chnum].output_4op(
                        output,
                        rshift,
                        clipmax,
                        &self.operator,
                        &self.regs,
                    );
                } else {
                    self.channel[chnum].output_2op(
                        output,
                        rshift,
                        clipmax,
                        &self.operator,
                        &self.regs,
                    );
                }
            }
        }
    }

    /// Gère les écritures dans les registres OPN.
    pub fn write(&mut self, regnum: u16, data: u8, intf: &mut dyn YmfmInterface) {
        // cas spécial : les écritures du registre de mode peuvent affecter les IRQ ;
        // ymfm les planifie via l'interface pour garantir l'ordre avec les timers
        if regnum as u32 == OpnRegisters::REG_MODE {
            self.engine_mode_write(data, intf);
            return;
        }

        // pour l'instant on marque tous les canaux comme modifiés
        self.modified_channels = ALL_CHANNELS;

        // la plupart des écritures sont passives, consommées seulement au besoin
        let mut keyon_channel = 0u32;
        let mut keyon_opmask = 0u32;
        if self
            .regs
            .write(regnum, data, &mut keyon_channel, &mut keyon_opmask)
        {
            // écritures du ou des registres key on
            if (keyon_channel as usize) < CHANNELS {
                // key on/off d'un canal normal
                self.channel[keyon_channel as usize].keyonoff(
                    keyon_opmask,
                    KEYON_NORMAL,
                    &mut self.operator,
                );
            }
            // PORT-NOTE : cas spécial des canaux rythme OPL (CHANNELS >= 9) omis.
        }
    }

    /// État courant des drapeaux de statut.
    pub fn status(&self) -> u8 {
        self.status & !Self::STATUS_BUSY & !(self.regs.status_mask() as u8)
    }

    /// Met/remet des bits du registre de statut en mettant à jour l'IRQ.
    pub fn set_reset_status(&mut self, set: u8, reset: u8, intf: &mut dyn YmfmInterface) -> u8 {
        self.status = (self.status | set) & !(reset | Self::STATUS_BUSY);
        self.engine_check_interrupts(intf);
        self.status & !(self.regs.status_mask() as u8)
    }

    /// Fixe le masque d'IRQ.
    pub fn set_irq_mask(&mut self, mask: u8, intf: &mut dyn YmfmInterface) {
        self.irq_mask = mask;
        self.engine_check_interrupts(intf);
    }

    /// Prescale d'horloge courant.
    pub fn clock_prescale(&self) -> u32 {
        self.clock_prescale as u32
    }

    /// Fixe le facteur de prescale (2/3/6).
    pub fn set_clock_prescale(&mut self, prescale: u32) {
        self.clock_prescale = prescale as u8;
    }

    /// Calcule la fréquence d'échantillonnage.
    pub fn sample_rate(&self, baseclock: u32) -> u32 {
        baseclock / (self.clock_prescale as u32 * OpnRegisters::OPERATORS)
    }

    /// Accès aux registres.
    pub fn regs(&self) -> &OpnRegisters {
        &self.regs
    }

    pub fn regs_mut(&mut self) -> &mut OpnRegisters {
        &mut self.regs
    }

    /// Invalide les caches.
    pub fn invalidate_caches(&mut self) {
        self.modified_channels = ALL_CHANNELS;
    }

    pub fn debug_channel(&self, index: usize) -> &FmChannel {
        &self.channel[index]
    }

    pub fn debug_operator(&self, index: usize) -> &FmOperator {
        &self.operator[index]
    }

    /// Callback de timer, appelé par l'interface quand un timer expire.
    pub fn engine_timer_expired(&mut self, tnum: u32, intf: &mut dyn YmfmInterface) {
        debug_assert!(tnum == 0 || tnum == 1);

        // met à jour le statut
        if tnum == 0 && self.regs.enable_timer_a() != 0 {
            self.set_reset_status(Self::STATUS_TIMERA, 0, intf);
        } else if tnum == 1 && self.regs.enable_timer_b() != 0 {
            self.set_reset_status(Self::STATUS_TIMERB, 0, intf);
        }

        // si le timer A expire en mode CSM, déclenche CSM sur tous les canaux concernés
        if tnum == 0 && self.regs.csm() != 0 {
            for chnum in 0..CHANNELS {
                if bitfield(OpnRegisters::CSM_TRIGGER_MASK, chnum as i32, 1) != 0 {
                    self.channel[chnum].keyonoff(0xf, KEYON_CSM, &mut self.operator);
                    self.modified_channels |= 1 << chnum;
                }
            }
        }

        // relance
        self.timer_running[tnum as usize] = 0;
        self.update_timer(tnum, 1, 0, intf);
    }

    /// Vérifie les sources d'interruption ; appelé par l'interface après synchronisation.
    pub fn engine_check_interrupts(&mut self, intf: &mut dyn YmfmInterface) {
        // met à jour l'état
        let old_state = self.irq_state;
        self.irq_state =
            u8::from((self.status & self.irq_mask & !(self.regs.status_mask() as u8)) != 0);

        // positionne le bit de statut IRQ
        // PORT-NOTE : STATUS_IRQ vaut 0 pour l'OPN, ces opérations sont sans effet (conservées pour fidélité).
        #[allow(clippy::identity_op)]
        if self.irq_state != 0 {
            self.status |= Self::STATUS_IRQ;
        } else {
            self.status &= !Self::STATUS_IRQ;
        }

        // si l'état a changé, signale le nouvel état
        if old_state != self.irq_state {
            intf.update_irq(self.irq_state != 0);
        }
    }

    /// Gère une écriture du registre de mode (via le callback de timer).
    pub fn engine_mode_write(&mut self, data: u8, intf: &mut dyn YmfmInterface) {
        // marque tous les canaux comme modifiés
        self.modified_channels = ALL_CHANNELS;

        // écrit réellement le registre de mode maintenant
        let (mut dummy1, mut dummy2) = (0u32, 0u32);
        self.regs.write(
            OpnRegisters::REG_MODE as u16,
            data,
            &mut dummy1,
            &mut dummy2,
        );

        // remet à zéro le statut d'IRQ : à l'écriture tous les autres bits sont ignorés
        if self.regs.irq_reset() != 0 {
            self.set_reset_status(0, 0x78, intf);
        } else {
            // remet à zéro le statut des timers
            let mut reset_mask = 0u8;
            if self.regs.reset_timer_b() != 0 {
                reset_mask |= OpnRegisters::STATUS_TIMERB;
            }
            if self.regs.reset_timer_a() != 0 {
                reset_mask |= OpnRegisters::STATUS_TIMERA;
            }
            self.set_reset_status(0, reset_mask, intf);

            // charge les timers ; le timer B reçoit un petit ajustement négatif car le multiplicateur
            // *16 tourne librement : le premier tick d'horloge est un peu plus court
            let load_b = self.regs.load_timer_b();
            let load_a = self.regs.load_timer_a();
            self.update_timer(1, load_b, -((self.total_clocks & 15) as i32), intf);
            self.update_timer(0, load_a, 0, intf);
        }
    }

    /// Récupère l'affectation courante opérateurs -> canaux et l'applique.
    fn assign_operators(&mut self) {
        let map = self.regs.operator_map();

        for (chnum, chan_map) in map.iter().enumerate() {
            for index in 0..4usize {
                let opnum = bitfield(*chan_map, 8 * index as i32, 8);
                let op = if opnum == 0xff {
                    None
                } else {
                    Some(opnum as usize)
                };
                self.channel[chnum].assign(index, op, &mut self.operator);
            }
        }
    }

    /// Met à jour l'état du timer donné.
    fn update_timer(
        &mut self,
        tnum: u32,
        enable: u32,
        delta_clocks: i32,
        intf: &mut dyn YmfmInterface,
    ) {
        // si le timer est actif mais pas encore armé, on l'arme
        if enable != 0 && self.timer_running[tnum as usize] == 0 {
            // la période vient des registres et diffère pour chaque timer
            let mut period = if tnum == 0 {
                1024u32.wrapping_sub(self.regs.timer_a_value())
            } else {
                16u32.wrapping_mul(256u32.wrapping_sub(self.regs.timer_b_value()))
            };

            // l'appelant peut aussi fournir un delta pour tenir compte d'autres effets
            period = period.wrapping_add(delta_clocks as u32);

            // le réarme
            intf.set_timer(
                tnum,
                period
                    .wrapping_mul(OpnRegisters::OPERATORS)
                    .wrapping_mul(self.clock_prescale as u32) as i32,
            );
            self.timer_running[tnum as usize] = 1;
        }
        // si le timer n'est pas actif, s'assure qu'il n'est pas armé
        else if enable == 0 {
            intf.set_timer(tnum, -1);
            self.timer_running[tnum as usize] = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fm_tables::attenuation_to_volume;

    #[derive(Default)]
    struct Mock {
        timers: Vec<(u32, i32)>,
        irqs: Vec<bool>,
    }

    impl YmfmInterface for Mock {
        fn sync_mode_write(&mut self, _data: u8) {}
        fn sync_check_interrupts(&mut self) {}
        fn set_timer(&mut self, tnum: u32, duration: i32) {
            self.timers.push((tnum, duration));
        }
        fn update_irq(&mut self, asserted: bool) {
            self.irqs.push(asserted);
        }
    }

    fn engine() -> (FmEngine, Mock) {
        let mut e = FmEngine::new();
        let mut m = Mock::default();
        e.reset(&mut m);
        m.timers.clear();
        m.irqs.clear();
        (e, m)
    }

    #[test]
    fn operators_are_assigned_to_channels() {
        let e = FmEngine::new();
        // canal 0 : opérateurs 0, 6, 3, 9 ; canal 5 : 14, 20, 17, 23
        assert_eq!(e.debug_channel(0).debug_operator(0), Some(0));
        assert_eq!(e.debug_channel(0).debug_operator(1), Some(6));
        assert_eq!(e.debug_channel(0).debug_operator(2), Some(3));
        assert_eq!(e.debug_channel(0).debug_operator(3), Some(9));
        assert_eq!(e.debug_channel(5).debug_operator(0), Some(14));
        assert_eq!(e.debug_channel(5).debug_operator(3), Some(23));
        assert_eq!(
            e.debug_operator(13).choffs(),
            OpnRegisters::channel_offset(4)
        );
        assert_eq!(
            e.debug_operator(13).opoffs(),
            OpnRegisters::operator_offset(13)
        );
    }

    #[test]
    fn timer_a_load_sets_timer_with_source_duration() {
        let (mut e, mut m) = engine();
        e.write(0x24, 0x80, &mut m); // N = 0x80 << 2 | 0 = 512
        e.write(0x25, 0x00, &mut m);
        e.write(0x27, 0x05, &mut m); // charge A + active A
                                     // période = 1024 - 512 = 512 ; durée = période * 24 opérateurs * prescale 6
        assert_eq!(m.timers, vec![(1, -1), (0, 512 * 24 * 6)]);
    }

    #[test]
    fn timer_expiry_sets_status_and_irq() {
        let (mut e, mut m) = engine();
        e.write(0x24, 0x80, &mut m);
        e.write(0x27, 0x05, &mut m);
        m.timers.clear();
        e.engine_timer_expired(0, &mut m);
        assert_eq!(e.status(), 1);
        assert_eq!(m.irqs, vec![true]);
        // le timer est réarmé
        assert_eq!(m.timers, vec![(0, 512 * 24 * 6)]);
        // reset du timer A : statut effacé, IRQ retombe
        e.write(0x27, 0x15, &mut m);
        assert_eq!(e.status(), 0);
        assert_eq!(m.irqs, vec![true, false]);
    }

    #[test]
    fn timer_b_uses_negative_adjustment() {
        let (mut e, mut m) = engine();
        for _ in 0..5 {
            e.clock(0x3F);
        }
        e.write(0x26, 0x00, &mut m); // N = 0 -> période 16 * 256
        e.write(0x27, 0x0A, &mut m); // charge B + active B
        assert_eq!(m.timers[0], (1, (16 * 256 - 5) * 24 * 6));
    }

    #[test]
    fn env_counter_follows_divider_of_three() {
        let (mut e, _m) = engine();
        let got: Vec<u32> = (0..4).map(|_| e.clock(0x3F)).collect();
        // 1, 2, 3 -> 3 a ses 2 bits bas == 3 : +1 => 4, puis 5
        assert_eq!(got, vec![1, 2, 4, 5]);
    }

    #[test]
    fn key_on_produces_output() {
        let (mut e, mut m) = engine();
        // canal 0 : opérateurs de registres 0, 8, 4, 12 ; AR = 31 ; algorithme 7
        for off in [0x00u16, 0x04, 0x08, 0x0C] {
            e.write(0x50 + off, 0x1F, &mut m);
        }
        e.write(0xB0, 0x07, &mut m);
        e.write(0x28, 0xF0, &mut m); // key on, 4 opérateurs, canal 0
        e.clock(0x3F);
        let mut out = Output::<2> { data: [0; 2] };
        e.output(&mut out, 0, 32767, 0x3F);
        let sin = e.regs().waveform()[0] as u32;
        let v0 = attenuation_to_volume(sin & 0x7fff) as i32;
        assert_ne!(v0, 0);
        assert_eq!(out.data, [4 * v0, 4 * v0]);
    }

    #[test]
    fn sample_rate_and_prescale() {
        let mut e = FmEngine::new();
        assert_eq!(e.clock_prescale(), 6);
        assert_eq!(e.sample_rate(8_000_000), 8_000_000 / (6 * 24));
        e.set_clock_prescale(3);
        assert_eq!(e.clock_prescale(), 3);
    }
}
