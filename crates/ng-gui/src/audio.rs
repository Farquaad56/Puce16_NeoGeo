//! Sortie audio (cpal) : tampon circulaire stéréo entrelacé alimenté par le thread d'émulation.

use std::collections::VecDeque;
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

pub struct AudioOut {
    _stream: cpal::Stream,
    shared: SharedBuf,
    pub rate: u32,
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
        let stream = device
            .build_output_stream(
                config.into(),
                move |data: &mut [f32], _| {
                    let mut q = cb_buf.lock().unwrap_or_else(|e| e.into_inner());
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
        })
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
