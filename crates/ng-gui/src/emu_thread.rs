//! Thread d'émulation : possède la `Machine` (non `Send`), reçoit des commandes, publie des trames.

use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam_channel::{bounded, unbounded, Receiver, Sender, TryRecvError};
use ng_cart::Config;
use ng_common::{PadState, SystemButtons};
use ng_core::{load_machine, Machine};

/// Horloge maître MVS en Hz (une trame = 405 504 mclk).
const MCLK_HZ: f64 = 24_000_000.0;
const MCLK_PER_FRAME: f64 = 405_504.0;

/// Commande envoyée au thread d'émulation.
pub enum Cmd {
    Load {
        game: String,
    },
    Reset,
    Pause(bool),
    Inputs {
        p1: PadState,
        p2: PadState,
        sys: SystemButtons,
    },
    Quit,
}

/// Trame publiée par le thread d'émulation.
pub struct FrameMsg {
    /// 320 x 224, 0x00RRGGBB.
    pub pixels: Vec<u32>,
    pub frame: u64,
    pub fps: f32,
}

/// Poignées côté interface.
pub struct EmuHandle {
    pub tx: Sender<Cmd>,
    pub rx: Receiver<FrameMsg>,
    pub err_rx: Receiver<String>,
    pub join: JoinHandle<()>,
}

/// Démarre le thread d'émulation.
pub fn spawn(cfg: Config) -> EmuHandle {
    let (tx, cmd_rx) = unbounded::<Cmd>();
    let (frame_tx, rx) = bounded::<FrameMsg>(2);
    let (err_tx, err_rx) = unbounded::<String>();
    let join = thread::spawn(move || run(cfg, cmd_rx, frame_tx, err_tx));
    EmuHandle {
        tx,
        rx,
        err_rx,
        join,
    }
}

fn run(cfg: Config, cmd_rx: Receiver<Cmd>, frame_tx: Sender<FrameMsg>, err_tx: Sender<String>) {
    let mut machine: Option<Machine> = None;
    let mut paused = false;
    let mut inputs = (
        PadState::default(),
        PadState::default(),
        SystemButtons::default(),
    );
    let mut t0 = Instant::now();
    let mut frames_since_t0: u64 = 0;
    let mut fps_mark = (Instant::now(), 0u64);
    let mut fps = 0.0f32;

    loop {
        loop {
            match cmd_rx.try_recv() {
                Ok(Cmd::Quit) | Err(TryRecvError::Disconnected) => return,
                Ok(Cmd::Load { game }) => match load_machine(&cfg, &game) {
                    Ok(m) => {
                        machine = Some(m);
                        t0 = Instant::now();
                        frames_since_t0 = 0;
                    }
                    Err(e) => {
                        let _ = err_tx.send(format!("{game} : {e}"));
                    }
                },
                Ok(Cmd::Reset) => {
                    if let Some(m) = machine.as_mut() {
                        m.reset();
                        t0 = Instant::now();
                        frames_since_t0 = 0;
                    }
                }
                Ok(Cmd::Pause(p)) => {
                    paused = p;
                    t0 = Instant::now();
                    frames_since_t0 = 0;
                }
                Ok(Cmd::Inputs { p1, p2, sys }) => inputs = (p1, p2, sys),
                Err(TryRecvError::Empty) => break,
            }
        }

        let Some(m) = machine.as_mut().filter(|_| !paused) else {
            thread::sleep(Duration::from_millis(16));
            continue;
        };
        m.set_inputs(inputs.0, inputs.1, inputs.2);
        m.run_frame();
        frames_since_t0 += 1;
        let frame = m.frame_count();
        if fps_mark.0.elapsed() >= Duration::from_millis(500) {
            fps = (frame - fps_mark.1) as f32 / fps_mark.0.elapsed().as_secs_f32();
            fps_mark = (Instant::now(), frame);
        }
        let _ = frame_tx.try_send(FrameMsg {
            pixels: m.framebuffer().pixels.clone(),
            frame,
            fps,
        });
        let target =
            t0 + Duration::from_secs_f64(frames_since_t0 as f64 * MCLK_PER_FRAME / MCLK_HZ);
        let now = Instant::now();
        if target > now {
            thread::sleep(target - now);
        } else if now - target > Duration::from_millis(200) {
            // trop en retard : on repart de maintenant plutôt que de rattraper en rafale
            t0 = now;
            frames_since_t0 = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn bad_cfg() -> Config {
        fallback_cfg()
    }

    fn fallback_cfg() -> Config {
        Config {
            bios_dir: PathBuf::from("/inexistant"),
            roms_dir: PathBuf::from("/inexistant"),
            saves_dir: PathBuf::from("/inexistant"),
            model: ng_cart::Model::Mvs,
            bios: "euro".into(),
        }
    }

    #[test]
    fn unknown_game_reports_an_error_then_quit_joins() {
        let h = spawn(bad_cfg());
        h.tx.send(Cmd::Load {
            game: "inconnu".into(),
        })
        .unwrap();
        let msg = h
            .err_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("message d'erreur");
        assert!(msg.contains("inconnu"));
        h.tx.send(Cmd::Quit).unwrap();
        h.join.join().expect("le thread se termine");
    }
}
