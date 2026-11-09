// license:BSD-3-Clause
// copyright-holders:Aaron Giles
//! Rééchantillonneur SSG (`ssg_resampler`) : ramène la sortie du SSG à la cadence de sortie de la puce.
//! Porté de ref/ymfm/src/ymfm_opn.h (l.337-410) et ymfm_opn.cpp (l.448-640).
//!
//! PORT-NOTE : le YM2610 instancie `ssg_resampler<output_data, 2, true>` (3 sorties, premier index 2,
//! mélange sur une seule sortie) : ces paramètres sont fixés ici. Seuls les rééchantillonneurs
//! utilisés en fidélité MED (2:9, et le nop pour un SSG surchargé) sont portés ; les autres rapports
//! de `configure` sont refusés. Le C++ garde une référence au SSG ; ici le SSG est passé en argument.

use crate::common::{bitfield, Output};
use crate::ssg::SsgEngine;

/// Index du premier échantillon écrit dans la sortie (FM gauche/droite occupent 0 et 1).
const FIRST_OUTPUT: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Resampler {
    Nop,
    TwoNine,
}

#[derive(Clone, Debug)]
pub struct SsgResampler {
    sampindex: u32,
    resampler: Resampler,
    last: Output<3>,
}

impl Default for SsgResampler {
    fn default() -> Self {
        Self::new()
    }
}

impl SsgResampler {
    /// Constructeur.
    pub fn new() -> Self {
        let mut last = Output::<3> { data: [0; 3] };
        last.clear();
        SsgResampler {
            sampindex: 0,
            resampler: Resampler::Nop,
            last,
        }
    }

    /// Index d'échantillon courant.
    pub fn sampindex(&self) -> u32 {
        self.sampindex
    }

    /// Configure un nouveau rapport (échantillons de sortie / échantillons SSG).
    pub fn configure(&mut self, outsamples: u8, srcsamples: u8) {
        self.resampler = match (outsamples, srcsamples) {
            (2, 9) => Resampler::TwoNine, // 2:9
            (0, 0) => Resampler::Nop,     // 0:0
            _ => panic!("rapport SSG non porte : {outsamples}:{srcsamples}"),
        };
    }

    /// Rééchantillonne `numsamples` échantillons dans `output` (3 sorties par échantillon).
    pub fn resample(&mut self, ssg: &mut SsgEngine, output: &mut [[i32; 3]], numsamples: u32) {
        match self.resampler {
            Resampler::TwoNine => self.resample_2_9(ssg, output, numsamples),
            Resampler::Nop => self.resample_nop(output, numsamples),
        }
    }

    /// Ajoute la dernière valeur calculée aux sommes, avec le facteur donné.
    fn add_last(&self, sum0: &mut i32, sum1: &mut i32, sum2: &mut i32, scale: i32) {
        *sum0 = sum0.wrapping_add(self.last.data[0].wrapping_mul(scale));
        *sum1 = sum1.wrapping_add(self.last.data[1].wrapping_mul(scale));
        *sum2 = sum2.wrapping_add(self.last.data[2].wrapping_mul(scale));
    }

    /// Cadence une nouvelle valeur puis l'ajoute aux sommes, avec le facteur donné.
    fn clock_and_add(
        &mut self,
        ssg: &mut SsgEngine,
        sum0: &mut i32,
        sum1: &mut i32,
        sum2: &mut i32,
        scale: i32,
    ) {
        ssg.clock();
        ssg.output(&mut self.last);
        self.add_last(sum0, sum1, sum2, scale);
    }

    /// Écrit les sommes dans la sortie, avec le diviseur donné (MixTo1 = vrai).
    fn write_to_output(
        &mut self,
        output: &mut [i32; 3],
        sum0: i32,
        sum1: i32,
        sum2: i32,
        divisor: i32,
    ) {
        // mélange sur une seule sortie : facteur 2/3 pour éviter le débordement
        output[FIRST_OUTPUT] =
            sum0.wrapping_add(sum1).wrapping_add(sum2).wrapping_mul(2) / (3 * divisor);

        // suit l'index d'échantillon ici
        self.sampindex = self.sampindex.wrapping_add(1);
    }

    /// 9 échantillons SSG pour 2 échantillons de sortie.
    fn resample_2_9(&mut self, ssg: &mut SsgEngine, output: &mut [[i32; 3]], numsamples: u32) {
        for out in output.iter_mut().take(numsamples as usize) {
            let (mut sum0, mut sum1, mut sum2) = (0i32, 0i32, 0i32);
            if bitfield(self.sampindex, 0, 1) != 0 {
                self.add_last(&mut sum0, &mut sum1, &mut sum2, 1);
            }
            self.clock_and_add(ssg, &mut sum0, &mut sum1, &mut sum2, 2);
            self.clock_and_add(ssg, &mut sum0, &mut sum1, &mut sum2, 2);
            self.clock_and_add(ssg, &mut sum0, &mut sum1, &mut sum2, 2);
            self.clock_and_add(ssg, &mut sum0, &mut sum1, &mut sum2, 2);
            if bitfield(self.sampindex, 0, 1) == 0 {
                self.clock_and_add(ssg, &mut sum0, &mut sum1, &mut sum2, 1);
            }
            self.write_to_output(out, sum0, sum1, sum2, 9);
        }
    }

    /// Rééchantillonneur nul : incrémente seulement l'index d'échantillon.
    fn resample_nop(&mut self, _output: &mut [[i32; 3]], numsamples: u32) {
        self.sampindex = self.sampindex.wrapping_add(numsamples);
    }

    #[cfg(test)]
    pub(crate) fn test_set_last(&mut self, last: [i32; 3]) {
        self.last.data = last;
    }

    #[cfg(test)]
    pub(crate) fn test_set_sampindex(&mut self, sampindex: u32) {
        self.sampindex = sampindex;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::YmfmInterface;

    struct Dummy;
    impl YmfmInterface for Dummy {
        fn sync_mode_write(&mut self, _data: u8) {}
        fn sync_check_interrupts(&mut self) {}
    }

    fn silent_ssg() -> SsgEngine {
        let mut ssg = SsgEngine::new(Box::new(Dummy));
        ssg.reset();
        ssg
    }

    #[test]
    fn nop_only_advances_the_sample_index() {
        let mut ssg = silent_ssg();
        let mut r = SsgResampler::new();
        let mut out = [[7, 8, 9]; 3];
        r.resample(&mut ssg, &mut out, 3);
        assert_eq!(r.sampindex(), 3);
        assert_eq!(out, [[7, 8, 9]; 3]);
    }

    #[test]
    fn two_nine_odd_index_reuses_last_value() {
        let mut ssg = silent_ssg();
        let mut r = SsgResampler::new();
        r.configure(2, 9);
        r.test_set_sampindex(1);
        r.test_set_last([9, 9, 9]);
        let mut out = [[1, 2, 0]];
        r.resample(&mut ssg, &mut out, 1);
        // sommes : last * 1 = 9 chacune ; les 4 clock_and_add d'un SSG muet ajoutent 0
        // sortie = (9+9+9) * 2 / (3 * 9) = 2 ; seul out[2] est écrit
        assert_eq!(out[0], [1, 2, (9 + 9 + 9) * 2 / (3 * 9)]);
        assert_eq!(r.sampindex(), 2);
    }

    #[test]
    fn two_nine_even_index_clocks_five_times() {
        let mut ssg = silent_ssg();
        let mut r = SsgResampler::new();
        r.configure(2, 9);
        r.test_set_last([9, 9, 9]); // sampindex 0 : last n'est pas réutilisé
        let mut out = [[0, 0, 5]];
        r.resample(&mut ssg, &mut out, 1);
        assert_eq!(out[0][2], 0);
        assert_eq!(r.sampindex(), 1);
    }

    #[test]
    #[should_panic]
    fn unsupported_ratio_is_rejected() {
        SsgResampler::new().configure(4, 1);
    }
}
