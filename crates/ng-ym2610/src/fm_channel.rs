// license:BSD-3-Clause
// copyright-holders:Aaron Giles
//! Canal FM : combine la sortie de 2 ou 4 opérateurs en un résultat final.
//! Porté de ref/ymfm/src/ymfm_fm.h (l.250-345) et ymfm_fm.ipp (l.793-1077).
//!
//! PORT-NOTE : les opérateurs appartiennent à l'engine (T503e) : le canal garde des indices
//! (`op: [Option<usize>; 4]`) dans le tableau `ops: &[FmOperator]` passé aux méthodes, ainsi que
//! `regs: &OpnRegisters`. `save_restore`, `log_keyon` et les `output_rhythm_*` (OPL) ne sont pas portés.
//! `output_*` prennent `&mut self` car `m_feedback_in` est `mutable` en C++.

use crate::common::{bitfield, clamp, Output};
use crate::fm_operator::FmOperator;
use crate::opn_regs::OpnRegisters;

// Table des algorithmes à 4 opérateurs (8 OPN/OPM + 4 OPL3).
//
//      ---------x opout[x] en entrée de l'opérateur 2
//      ------xxx- opout[x] en entrée de l'opérateur 3
//      ---xxx---- opout[x] en entrée de l'opérateur 4
//      --x------- opout[1] dans la somme finale
//      -x-------- opout[2] dans la somme finale
//      x--------- opout[3] dans la somme finale
const fn algorithm(
    op2in: u16,
    op3in: u16,
    op4in: u16,
    op1out: u16,
    op2out: u16,
    op3out: u16,
) -> u16 {
    op2in | (op3in << 1) | (op4in << 4) | (op1out << 7) | (op2out << 8) | (op3out << 9)
}

const ALGORITHM_OPS: [u16; 12] = [
    algorithm(1, 2, 3, 0, 0, 0), //  0: O1 -> O2 -> O3 -> O4 -> out (O4)
    algorithm(0, 5, 3, 0, 0, 0), //  1: (O1 + O2) -> O3 -> O4 -> out (O4)
    algorithm(0, 2, 6, 0, 0, 0), //  2: (O1 + (O2 -> O3)) -> O4 -> out (O4)
    algorithm(1, 0, 7, 0, 0, 0), //  3: ((O1 -> O2) + O3) -> O4 -> out (O4)
    algorithm(1, 0, 3, 0, 1, 0), //  4: ((O1 -> O2) + (O3 -> O4)) -> out (O2+O4)
    algorithm(1, 1, 1, 0, 1, 1), //  5: ((O1 -> O2) + (O1 -> O3) + (O1 -> O4)) -> out (O2+O3+O4)
    algorithm(1, 0, 0, 0, 1, 1), //  6: ((O1 -> O2) + O3 + O4) -> out (O2+O3+O4)
    algorithm(0, 0, 0, 1, 1, 1), //  7: (O1 + O2 + O3 + O4) -> out (O1+O2+O3+O4)
    algorithm(1, 2, 3, 0, 0, 0), //  8: comme 0
    algorithm(0, 2, 3, 1, 0, 0), //  9: (O1 + (O2 -> O3 -> O4)) -> out (O1+O4)
    algorithm(1, 0, 3, 0, 1, 0), // 10: comme 4
    algorithm(0, 2, 0, 1, 0, 1), // 11: (O1 + (O2 -> O3) + O4) -> out (O1+O3+O4)
];

#[derive(Clone, Debug)]
pub struct FmChannel {
    /// Décalage de canal dans les registres.
    choffs: u32,
    /// Mémoire de feedback de l'opérateur 1.
    feedback: [i16; 2],
    /// Prochaine entrée de feedback de l'opérateur 1 (fixée dans output).
    feedback_in: i16,
    /// Indices (dans le tableau d'opérateurs de l'engine) de jusqu'à 4 opérateurs.
    op: [Option<usize>; 4],
}

impl FmChannel {
    /// Constructeur.
    pub fn new(choffs: u32) -> Self {
        FmChannel {
            choffs,
            feedback: [0, 0],
            feedback_in: 0,
            op: [None; 4],
        }
    }

    /// Remet l'état du canal à zéro.
    pub fn reset(&mut self) {
        self.feedback = [0, 0];
        self.feedback_in = 0;
    }

    pub fn choffs(&self) -> u32 {
        self.choffs
    }

    /// Assigne un opérateur (par indice dans `ops`) ; le canal courant lui est communiqué.
    pub fn assign(&mut self, index: usize, op: Option<usize>, ops: &mut [FmOperator]) {
        self.op[index] = op;
        if let Some(i) = op {
            ops[i].set_choffs(self.choffs);
        }
    }

    /// Canal à 4 opérateurs ? (OPNA : OPERATORS / CHANNELS == 4, DYNAMIC_OPS faux)
    pub fn is4op(&self) -> bool {
        OpnRegisters::OPERATORS / OpnRegisters::CHANNELS == 4
    }

    pub fn debug_operator(&self, index: usize) -> Option<usize> {
        self.op[index]
    }

    /// Signale un key on/off aux opérateurs du canal.
    pub fn keyonoff(&mut self, states: u32, keyon_type: u32, ops: &mut [FmOperator]) {
        for opnum in 0..self.op.len() {
            if let Some(i) = self.op[opnum] {
                ops[i].keyonoff(bitfield(states, opnum as i32, 1), keyon_type);
            }
        }
    }

    /// Prépare avant l'horloge ; true si au moins un opérateur est actif.
    pub fn prepare(&mut self, ops: &mut [FmOperator], regs: &OpnRegisters) -> bool {
        let mut active_mask = 0u32;

        // prépare tous les opérateurs et détermine s'ils sont actifs
        for opnum in 0..self.op.len() {
            if let Some(i) = self.op[opnum] {
                if ops[i].prepare(regs) {
                    active_mask |= 1 << opnum;
                }
            }
        }

        active_mask != 0
    }

    /// Horloge principale de tous les opérateurs.
    pub fn clock(
        &mut self,
        env_counter: u32,
        lfo_raw_pm: i32,
        ops: &mut [FmOperator],
        regs: &OpnRegisters,
    ) {
        // fait avancer le feedback
        self.feedback[0] = self.feedback[1];
        self.feedback[1] = self.feedback_in;

        for opnum in 0..self.op.len() {
            if let Some(i) = self.op[opnum] {
                ops[i].clock(env_counter, lfo_raw_pm, regs);
            }
        }
    }

    /// Combine 2 opérateurs selon l'algorithme ; somme selon `rshift` et `clipmax`.
    pub fn output_2op(
        &mut self,
        output: &mut Output<2>,
        rshift: u32,
        clipmax: i32,
        ops: &[FmOperator],
        regs: &OpnRegisters,
    ) {
        // les 2 premiers opérateurs doivent exister
        let o1 = &ops[self.op[0].expect("operateur 1")];
        let o2 = &ops[self.op[1].expect("operateur 2")];

        // l'AM est identique pour tous les opérateurs : calculée une fois
        let am_offset = regs.lfo_am_offset(self.choffs);

        // l'opérateur 1 a un auto-feedback optionnel
        let mut opmod: i32 = 0;
        let feedback = regs.ch_feedback(self.choffs);
        if feedback != 0 {
            opmod = (self.feedback[0] as i32 + self.feedback[1] as i32) >> (10 - feedback);
        }

        // volume 14 bits de l'opérateur 1 et mise à jour du feedback
        // PORT-NOTE : `op1value = m_feedback_in = ...` : la valeur est tronquée en int16.
        self.feedback_in =
            o1.compute_volume(o1.phase().wrapping_add(opmod as u32), am_offset, regs) as i16;
        let op1value = self.feedback_in as i32;

        // inutile de continuer si toutes les sorties sont coupées
        if regs.ch_output_any(self.choffs) == 0 {
            return;
        }

        // algorithmes à deux opérateurs :
        //    0: O1 -> O2 -> out
        //    1: (O1 + O2) -> out
        let result = if bitfield(regs.ch_algorithm(self.choffs), 0, 1) == 0 {
            // MODULATOR_DELAY est faux sur OPN : on utilise l'échantillon courant
            opmod = op1value >> 1;
            o2.compute_volume(o2.phase().wrapping_add(opmod as u32), am_offset, regs) >> rshift
        } else {
            let mut r = op1value >> rshift;
            r += o2.compute_volume(o2.phase(), am_offset, regs) >> rshift;
            let clipmin = -clipmax - 1;
            clamp(r, clipmin, clipmax)
        };

        // ajoute à la sortie
        self.add_to_output(output, result, regs);
    }

    /// Combine 4 opérateurs selon l'algorithme ; somme selon `rshift` et `clipmax`.
    pub fn output_4op(
        &mut self,
        output: &mut Output<2>,
        rshift: u32,
        clipmax: i32,
        ops: &[FmOperator],
        regs: &OpnRegisters,
    ) {
        // les 4 opérateurs doivent exister
        let o1 = &ops[self.op[0].expect("operateur 1")];
        let o2 = &ops[self.op[1].expect("operateur 2")];
        let o3 = &ops[self.op[2].expect("operateur 3")];
        let o4 = &ops[self.op[3].expect("operateur 4")];

        // l'AM est identique pour tous les opérateurs : calculée une fois
        let am_offset = regs.lfo_am_offset(self.choffs);

        // l'opérateur 1 a un auto-feedback optionnel
        let mut opmod: i32 = 0;
        let feedback = regs.ch_feedback(self.choffs);
        if feedback != 0 {
            opmod = (self.feedback[0] as i32 + self.feedback[1] as i32) >> (10 - feedback);
        }

        // volume 14 bits de l'opérateur 1 et mise à jour du feedback
        self.feedback_in =
            o1.compute_volume(o1.phase().wrapping_add(opmod as u32), am_offset, regs) as i16;
        let op1value = self.feedback_in as i32;

        // inutile de continuer si toutes les sorties sont coupées
        if regs.ch_output_any(self.choffs) == 0 {
            return;
        }

        // OPM/OPN : 8 algorithmes de connexion pour 4 opérateurs (OPL3 en ajoute 4, numérotés 8-11).
        // Les opérateurs sont calculés dans l'ordre, les entrées lues dans le tableau opout :
        //    0 = 0, 1 = O1, 2 = O2, 3 = O3, 4 = (O4), 5 = O1+O2, 6 = O1+O3, 7 = O2+O3
        let algorithm_ops = ALGORITHM_OPS[regs.ch_algorithm(self.choffs) as usize] as u32;

        // PORT-NOTE : opout est un int16_t[8] en C++ : chaque valeur est tronquée en 16 bits.
        let mut opout = [0i16; 8];
        opout[0] = 0;
        opout[1] = op1value as i16;

        // volume 14 bits de l'opérateur 2
        opmod = (opout[bitfield(algorithm_ops, 0, 1) as usize] as i32) >> 1;
        opout[2] = o2.compute_volume(o2.phase().wrapping_add(opmod as u32), am_offset, regs) as i16;
        opout[5] = (opout[1] as i32 + opout[2] as i32) as i16;

        // volume 14 bits de l'opérateur 3
        opmod = (opout[bitfield(algorithm_ops, 1, 3) as usize] as i32) >> 1;
        opout[3] = o3.compute_volume(o3.phase().wrapping_add(opmod as u32), am_offset, regs) as i16;
        opout[6] = (opout[1] as i32 + opout[3] as i32) as i16;
        opout[7] = (opout[2] as i32 + opout[3] as i32) as i16;

        // volume 14 bits de l'opérateur 4 (valeur de bruit possible sur OPM) ;
        // tous les algorithmes consomment au minimum la sortie de l'OP4
        let mut result: i32;
        if regs.noise_enable() != 0 && self.choffs == 7 {
            result = o4.compute_noise_volume(am_offset, regs);
        } else {
            opmod = (opout[bitfield(algorithm_ops, 4, 3) as usize] as i32) >> 1;
            result = o4.compute_volume(o4.phase().wrapping_add(opmod as u32), am_offset, regs);
        }
        result >>= rshift;

        // ajoute éventuellement OP1, OP2, OP3
        let clipmin = -clipmax - 1;
        if bitfield(algorithm_ops, 7, 1) != 0 {
            result = clamp(result + ((opout[1] as i32) >> rshift), clipmin, clipmax);
        }
        if bitfield(algorithm_ops, 8, 1) != 0 {
            result = clamp(result + ((opout[2] as i32) >> rshift), clipmin, clipmax);
        }
        if bitfield(algorithm_ops, 9, 1) != 0 {
            result = clamp(result + ((opout[3] as i32) >> rshift), clipmin, clipmax);
        }

        // ajoute à la sortie
        self.add_to_output(output, result, regs);
    }

    /// Ajoute une valeur aux sorties selon les validations de canal (OUTPUTS = 2).
    fn add_to_output(&self, output: &mut Output<2>, value: i32, regs: &OpnRegisters) {
        if regs.ch_output_0(self.choffs) != 0 {
            output.data[0] += value;
        }
        if regs.ch_output_1(self.choffs) != 0 {
            output.data[1] += value;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::EG_SUSTAIN;
    use crate::fm_operator::KEYON_NORMAL;
    use crate::fm_tables::attenuation_to_volume;

    fn setup() -> (OpnRegisters, Vec<FmOperator>, FmChannel) {
        let mut regs = OpnRegisters::new();
        regs.reset();
        let mut ops: Vec<FmOperator> = (0..4).map(FmOperator::new).collect();
        let mut ch = FmChannel::new(0);
        for i in 0..4 {
            ch.assign(i, Some(i), &mut ops);
            ops[i].test_set_envelope(0, EG_SUSTAIN);
        }
        (regs, ops, ch)
    }

    fn set_alg(regs: &mut OpnRegisters, alg: u8) {
        let (mut c, mut m) = (0, 0);
        regs.write(0xB0, alg, &mut c, &mut m);
    }

    /// Volume d'un opérateur à atténuation 0 et phase 0.
    fn v0(regs: &OpnRegisters) -> i32 {
        let sin = regs.waveform()[0] as u32;
        let v = attenuation_to_volume(sin & 0x7fff) as i32;
        if sin & 0x8000 != 0 {
            -v
        } else {
            v
        }
    }

    #[test]
    fn algorithm_7_sums_four_operators() {
        let (mut regs, ops, mut ch) = setup();
        set_alg(&mut regs, 7);
        let mut out = Output::<2> { data: [0; 2] };
        ch.output_4op(&mut out, 0, 32767, &ops, &regs);
        assert_ne!(v0(&regs), 0);
        assert_eq!(out.data[0], 4 * v0(&regs));
        assert_eq!(out.data[1], 4 * v0(&regs));
    }

    #[test]
    fn algorithm_0_depends_on_op4_only_without_modulation() {
        let (mut regs, mut ops, mut ch) = setup();
        set_alg(&mut regs, 0);
        // op4 coupé : sortie nulle quels que soient op1..3
        ops[3].test_set_envelope(0x3ff, EG_SUSTAIN);
        let mut out = Output::<2> { data: [0; 2] };
        ch.output_4op(&mut out, 0, 32767, &ops, &regs);
        assert_eq!(out.data, [0, 0]);
        // seul op4 actif : sortie = volume(0)
        ops[3].test_set_envelope(0, EG_SUSTAIN);
        for op in ops.iter_mut().take(3) {
            op.test_set_envelope(0x3ff, EG_SUSTAIN);
        }
        let mut out = Output::<2> { data: [0; 2] };
        ch.output_4op(&mut out, 0, 32767, &ops, &regs);
        assert_eq!(out.data[0], v0(&regs));
    }

    #[test]
    fn output_is_clamped_to_clipmax() {
        let (mut regs, ops, mut ch) = setup();
        set_alg(&mut regs, 7);
        let mut out = Output::<2> { data: [0; 2] };
        ch.output_4op(&mut out, 0, 100, &ops, &regs);
        assert_eq!(out.data, [100, 100]);
    }

    #[test]
    fn output_respects_channel_enables_and_rshift() {
        let (mut regs, ops, mut ch) = setup();
        set_alg(&mut regs, 7);
        let (mut c, mut m) = (0, 0);
        regs.write(0xB4, 0x80, &mut c, &mut m); // gauche seulement
        let mut out = Output::<2> { data: [0; 2] };
        ch.output_4op(&mut out, 1, 32767, &ops, &regs);
        assert_eq!(out.data[0], (v0(&regs) >> 1) * 4);
        assert_eq!(out.data[1], 0);
        regs.write(0xB4, 0x00, &mut c, &mut m); // tout coupé : feedback mis à jour, pas de sortie
        let mut out = Output::<2> { data: [0; 2] };
        ch.output_4op(&mut out, 0, 32767, &ops, &regs);
        assert_eq!(out.data, [0, 0]);
    }

    #[test]
    fn two_op_algorithms() {
        let (mut regs, ops, mut ch) = setup();
        set_alg(&mut regs, 1); // (O1 + O2)
        let mut out = Output::<2> { data: [0; 2] };
        ch.output_2op(&mut out, 0, 32767, &ops, &regs);
        assert_eq!(out.data[0], 2 * v0(&regs));
        set_alg(&mut regs, 0); // O1 -> O2 : phase de O2 décalée de v0 >> 1
        let mut out = Output::<2> { data: [0; 2] };
        ch.output_2op(&mut out, 0, 32767, &ops, &regs);
        let expected = ops[1].compute_volume((v0(&regs) >> 1) as u32, 0, &regs);
        assert_eq!(out.data[0], expected);
    }

    #[test]
    fn feedback_shifts_through_clock() {
        let (mut regs, mut ops, mut ch) = setup();
        set_alg(&mut regs, 7);
        let mut out = Output::<2> { data: [0; 2] };
        ch.output_4op(&mut out, 0, 32767, &ops, &regs);
        assert_eq!(ch.feedback_in as i32, v0(&regs));
        ch.clock(0, 0, &mut ops, &regs);
        assert_eq!(ch.feedback, [0, v0(&regs) as i16]);
        ch.clock(0, 0, &mut ops, &regs);
        assert_eq!(ch.feedback, [v0(&regs) as i16, v0(&regs) as i16]);
        ch.reset();
        assert_eq!(ch.feedback, [0, 0]);
    }

    #[test]
    fn keyonoff_and_prepare() {
        let (regs, mut ops, mut ch) = setup();
        for op in ops.iter_mut() {
            op.reset();
        }
        assert!(!ch.prepare(&mut ops, &regs)); // tous en release silencieux
        ch.keyonoff(0b0101, KEYON_NORMAL, &mut ops);
        assert!(ch.prepare(&mut ops, &regs));
        assert_ne!(ops[0].debug_eg_state(), crate::common::EG_RELEASE);
        assert_eq!(ops[1].debug_eg_state(), crate::common::EG_RELEASE);
        assert_ne!(ops[2].debug_eg_state(), crate::common::EG_RELEASE);
        assert!(ch.is4op());
    }
}
