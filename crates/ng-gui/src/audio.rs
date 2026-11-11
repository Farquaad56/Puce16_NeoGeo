//! Sortie audio (cpal) : tampon circulaire stéréo entrelacé alimenté par le thread d'émulation.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

pub type SharedBuf = Arc<Mutex<VecDeque<f32>>>;

/// Remplit `out` depuis `deque` ; complète par du silence si le tampon est vide.
pub fn fill_buffer(deque: &mut VecDeque<f32>, out: &mut [f32]) {
    for o in out.iter_mut() {
        *o = deque.pop_front().unwrap_or(0.0);
    }
}

/// Ajoute des échantillons en plafonnant le tampon à `cap` valeurs (les plus anciennes sont jetées).
pub fn push_capped(deque: &mut VecDeque<f32>, samples: &[f32], cap: usize) {
    deque.extend(samples.iter().copied());
    while deque.len() > cap {
        deque.pop_front();
    }
}

/// Latence visée du tampon audio, en ms : de quoi absorber les irrégularités d'ordonnancement.
pub const TARGET_MS: usize = 60;

/// Ajustement relatif de la fréquence source pour ramener le tampon vers sa cible : `fill` est le
/// remplissage en fraction de la cible (1 = cible). Tampon trop vide -> source déclarée plus lente
/// (plus d'échantillons produits), trop plein -> plus rapide ; borné à ±0,5 % (inaudible).
pub fn drc_adjust(fill: f32) -> f64 {
    (f64::from(fill - 1.0) * 0.01).clamp(-0.005, 0.005)
}

/// Gain linéaire appliqué au signal pour un volume de 0 à 100 % (courbe quadratique, plus proche
/// de la perception) ; 0 si le son est coupé.
pub fn volume_gain(volume: u8, mute: bool) -> f32 {
    if mute {
        return 0.0;
    }
    let v = f32::from(volume.min(100)) / 100.0;
    v * v
}

pub struct AudioOut {
    _stream: cpal::Stream,
    shared: SharedBuf,
    pub rate: u32,
    /// Sous-alimentations du périphérique tant que l'émulation tourne.
    underruns: Arc<AtomicU64>,
    /// Faux à l'arrêt et en pause : le silence n'est alors pas une sous-alimentation.
    active: Arc<AtomicBool>,
    /// Vrai une fois la latence visée accumulée ; repasse à faux après un manque ou en pause.
    primed: Arc<AtomicBool>,
    /// Latence visée, en valeurs entrelacées.
    target: usize,
}

impl AudioOut {
    /// Ouvre le périphérique de sortie par défaut en f32 stéréo.
    pub fn new() -> Result<AudioOut, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("aucun périphérique audio de sortie")?;
        let config = device
            .default_output_config()
            .map_err(|e| format!("configuration audio : {e}"))?;
        if config.sample_format() != cpal::SampleFormat::F32 {
            return Err(format!(
                "format audio {:?} non pris en charge (f32 requis)",
                config.sample_format()
            ));
        }
        let channels = config.channels() as usize;
        if channels < 2 {
            return Err("sortie audio mono non prise en charge (stéréo requise)".into());
        }
        let rate = config.sample_rate();
        let shared: SharedBuf = Arc::new(Mutex::new(VecDeque::new()));
        let cb_buf = Arc::clone(&shared);
        let underruns = Arc::new(AtomicU64::new(0));
        let active = Arc::new(AtomicBool::new(false));
        let (cb_under, cb_active) = (Arc::clone(&underruns), Arc::clone(&active));
        let target = rate as usize * 2 * TARGET_MS / 1000;
        // la lecture ne démarre qu'une fois la latence visée accumulée (et redémarre après un manque)
        let primed = Arc::new(AtomicBool::new(false));
        let cb_primed = Arc::clone(&primed);
        let stream = device
            .build_output_stream(
                config.into(),
                move |data: &mut [f32], _| {
                    let mut q = cb_buf.lock().unwrap_or_else(|e| e.into_inner());
                    let active = cb_active.load(Ordering::Relaxed);
                    if !cb_primed.load(Ordering::Relaxed) {
                        if active && q.len() >= target {
                            cb_primed.store(true, Ordering::Relaxed);
                        } else {
                            data.fill(0.0); // accumulation de la latence visée, ou pause
                            return;
                        }
                    }
                    if q.len() < data.len() / channels * 2 && active {
                        cb_under.fetch_add(1, Ordering::Relaxed);
                        // on rejoue ce qui reste puis on reconstitue le coussin
                        cb_primed.store(false, Ordering::Relaxed);
                    }
                    if channels == 2 {
                        fill_buffer(&mut q, data);
                    } else {
                        for frame in data.chunks_mut(channels) {
                            frame[0] = q.pop_front().unwrap_or(0.0);
                            frame[1] = q.pop_front().unwrap_or(0.0);
                            for c in frame.iter_mut().skip(2) {
                                *c = 0.0;
                            }
                        }
                    }
                },
                |e| eprintln!("erreur audio : {e}"),
                None,
            )
            .map_err(|e| format!("ouverture du flux audio : {e}"))?;
        stream
            .play()
            .map_err(|e| format!("démarrage du flux audio : {e}"))?;
        Ok(AudioOut {
            _stream: stream,
            shared,
            rate,
            underruns,
            active,
            primed,
            target,
        })
    }

    /// Remplissage du tampon en fraction de la latence visée (1 = cible).
    pub fn fill(&self) -> f32 {
        let q = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        q.len() as f32 / self.target as f32
    }

    pub fn underruns(&self) -> u64 {
        self.underruns.load(Ordering::Relaxed)
    }

    /// Indique si l'émulation produit du son (sinon le silence n'est pas compté).
    pub fn set_active(&self, on: bool) {
        self.active.store(on, Ordering::Relaxed);
        if !on {
            self.primed.store(false, Ordering::Relaxed);
        }
    }

    /// Ajoute des échantillons stéréo entrelacés (plafond : 0,25 s).
    pub fn push(&self, samples: &[f32]) {
        let cap = self.rate as usize / 2;
        let mut q = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        push_capped(&mut q, samples, cap);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_control_is_bounded_and_signed() {
        assert_eq!(drc_adjust(1.0), 0.0);
        // tampon vide : source plus lente (négatif) ; plein : plus rapide ; toujours dans ±0,5 %
        assert_eq!(drc_adjust(0.0), -0.005);
        assert_eq!(drc_adjust(5.0), 0.005);
        assert!(drc_adjust(0.8) < 0.0 && drc_adjust(1.2) > 0.0);
        assert!(drc_adjust(0.8).abs() < 0.005);
    }

    #[test]
    fn volume_curve() {
        assert_eq!(volume_gain(100, false), 1.0);
        assert_eq!(volume_gain(0, false), 0.0);
        assert_eq!(volume_gain(100, true), 0.0);
        assert!((volume_gain(50, false) - 0.25).abs() < 1e-6);
        assert_eq!(volume_gain(200, false), 1.0);
    }

    #[test]
    fn underrun_is_filled_with_silence() {
        let mut d: VecDeque<f32> = VecDeque::from(vec![0.1, 0.2]);
        let mut buf = [9.0f32; 4];
        fill_buffer(&mut d, &mut buf);
        assert_eq!(buf, [0.1, 0.2, 0.0, 0.0]);
    }

    #[test]
    fn buffer_is_capped_to_the_latest_samples() {
        let rate = 48_000usize;
        let mut d = VecDeque::new();
        let all: Vec<f32> = (0..rate).map(|i| i as f32).collect();
        push_capped(&mut d, &all, rate / 2);
        assert_eq!(d.len(), rate / 2);
        assert_eq!(d.front().copied(), Some((rate / 2) as f32));
        assert_eq!(d.back().copied(), Some((rate - 1) as f32));
    }

    #[test]
    #[ignore = "dépend du périphérique audio de la machine"]
    fn open_default_device() {
        let a = AudioOut::new();
        eprintln!("audio : {:?}", a.as_ref().err());
    }
}
