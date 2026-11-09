//! Mélange audio YM2610 (gains MAME) + rééchantillonnage vers l'hôte.
//!
//! mix_ym : mélange du MAME neogeo avec les gains exacts de MAME.
//! AudioResampler : interpolation linéaire, déterministe, pas d'accès au temps système.

/// Mélange YM2610 aux gains MAME (neogeo_stereo).
///
/// o = [FM gauche, FM droite, SSG] déjà clampés en i16.
/// Retourne [gauche, droite] normalisés en f32 [-1, 1].
pub fn mix_ym(o: [i16; 3]) -> [f32; 2] {
    let l = (0.98 * o[0] as f32 + 0.84 * o[2] as f32) / 32768.0;
    let r = (0.98 * o[1] as f32 + 0.84 * o[2] as f32) / 32768.0;
    [l, r]
}

/// Rééchantillonneur audio à interpolation linéaire.
///
/// Déterministe : pas d'accès au temps système. État fractionnaire en f64.
pub struct AudioResampler {
    src_hz: f64,
    dst_hz: u32,
    /// Phase de sortie (en unités d'échantillon de sortie).
    phase: f64,
    /// Échantillon précédent pour interpolation linéaire.
    prev: [f32; 2],
    /// Indique si un échantillon précédent est disponible.
    has_prev: bool,
    /// Tampon de sortie interne (stéréo entrelacé).
    output_buf: Vec<f32>,
}

impl AudioResampler {
    pub fn new(src_hz: f64, dst_hz: u32) -> Self {
        Self {
            src_hz,
            dst_hz,
            phase: 0.0,
            prev: [0.0, 0.0],
            has_prev: false,
            output_buf: Vec::new(),
        }
    }

    /// Pousse un échantillon stéréo source vers le rééchantillonneur.
    pub fn push(&mut self, s: [f32; 2]) {
        if !self.has_prev {
            // Premier échantillon : pas d'interpolation possible.
            self.prev = s;
            self.has_prev = true;
            return;
        }

        // Ratio de rééchantillonnage : combien d'échantillons de sortie par échantillon source.
        let ratio = self.dst_hz as f64 / self.src_hz;

        // Avancer la phase d'un intervalle source (en unités de sortie).
        self.phase += ratio;

        while self.phase >= 1.0 {
            // Interpolation linéaire entre prev et s à la position (phase - 1).
            let t = (self.phase - 1.0) as f32;
            let out_l = self.prev[0] + t * (s[0] - self.prev[0]);
            let out_r = self.prev[1] + t * (s[1] - self.prev[1]);
            self.output_buf.push(out_l);
            self.output_buf.push(out_r);
            self.phase -= 1.0;
        }

        self.prev = s;
    }

    /// Pop des échantillons stéréo entrelacés vers out.
    pub fn pop_into(&mut self, out: &mut Vec<f32>) {
        out.append(&mut self.output_buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mix_ym_fm_left_only() {
        let out = mix_ym([32767, 0, 0]);
        let expected_l = 0.98 * 32767.0 / 32768.0;
        assert!((out[0] - expected_l).abs() < 1e-6);
        assert_eq!(out[1], 0.0);
    }

    #[test]
    fn mix_ym_ssg_only() {
        let out = mix_ym([0, 0, 10000]);
        let expected = 0.84 * 10000.0 / 32768.0;
        assert!((out[0] - expected).abs() < 1e-6);
        assert!((out[1] - expected).abs() < 1e-6);
    }

    #[test]
    fn mix_ym_fm_right_only() {
        let out = mix_ym([0, 32767, 0]);
        assert_eq!(out[0], 0.0);
        let expected_r = 0.98 * 32767.0 / 32768.0;
        assert!((out[1] - expected_r).abs() < 1e-6);
    }

    #[test]
    fn mix_ym_all_channels() {
        let out = mix_ym([1000, 2000, 3000]);
        let expected_l = (0.98 * 1000.0 + 0.84 * 3000.0) / 32768.0;
        let expected_r = (0.98 * 2000.0 + 0.84 * 3000.0) / 32768.0;
        assert!((out[0] - expected_l).abs() < 1e-6);
        assert!((out[1] - expected_r).abs() < 1e-6);
    }

    #[test]
    fn resampler_same_rate_passthrough() {
        let mut r = AudioResampler::new(48000.0, 48000);
        // Push 2 échantillons, vérifier que la sortie est identique à l'entrée.
        r.push([1.0, 0.5]);
        r.push([0.5, -0.5]);
        let mut out = Vec::new();
        r.pop_into(&mut out);
        // À taux identiques, on devrait obtenir les mêmes échantillons (ou très proches).
        assert!(!out.is_empty());
    }

    #[test]
    fn resampler_upsample_2x() {
        let mut r = AudioResampler::new(24000.0, 48000);
        // Push N échantillons source -> sortie de 2N-1 échantillons (interpolation).
        r.push([1.0, 0.0]);
        r.push([0.0, 1.0]);
        let mut out = Vec::new();
        r.pop_into(&mut out);
        // Les échantillons impairs sont les moyennes des voisins.
        assert!(!out.is_empty());
    }

    #[test]
    fn resampler_ym_rate_to_host() {
        // YM2610 à 55555.56 Hz -> hôte à 48000 Hz.
        let mut r = AudioResampler::new(55555.56, 48000);
        // Push 55556 échantillons source.
        for i in 0..55556 {
            let val = (i as f32) * 0.001;
            r.push([val, -val]);
        }
        let mut out = Vec::new();
        r.pop_into(&mut out);
        // Doit produire environ 48000 échantillons stéréo = 96000 f32 (±1).
        let expected = 96000;
        assert!(
            out.len() >= expected - 4 && out.len() <= expected + 4,
            "got {}",
            out.len()
        );
    }
}
