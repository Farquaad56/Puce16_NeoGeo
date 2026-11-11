//! Thread d'émulation : possède la `Machine` (non `Send`), reçoit des commandes, publie des trames.

use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam_channel::{bounded, unbounded, Receiver, Sender, TryRecvError};
use ng_cart::Config;
use ng_common::{PadState, SystemButtons};
use ng_core::audio_mix::AudioResampler;
use ng_core::{load_machine, DebugLevel, DebugSnapshot, Machine};

use crate::audio::AudioOut;

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
    /// Remplace la configuration (BIOS, modèle) utilisée par les prochains `Load`.
    Config(Config),
    /// Niveau des instantanés de débogage (`Off` = aucun).
    Debug(DebugLevel),
    /// En pause : exécute une seule trame.
    StepFrame,
    Inputs {
        p1: PadState,
        p2: PadState,
        sys: SystemButtons,
        /// Panneau mahjong (3 rangées, bit à 0 = pressée).
        mj: [u8; 3],
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
    pub dbg_rx: Receiver<DebugSnapshot>,
    #[allow(dead_code)] // conservé pour joindre le thread dans les tests et un futur arrêt propre
    pub join: JoinHandle<()>,
}

/// BIOS Japon (par ordre de préférence) : le panneau mahjong n'y fonctionne dans le menu service
/// que sur ces BIOS (MAME : `ROM_DEFAULT_BIOS( "japan" )` pour les machines `neogeo_mj`).
const JAPAN_BIOS: [&str; 3] = ["vs-bios.rom", "japan-j3.bin", "sp1.jipan.1024"];

/// Configuration à utiliser pour `game` : pour un jeu mahjong, le BIOS Japon est choisi
/// automatiquement s'il est disponible ; sinon un message est renvoyé pour la barre d'état.
fn config_for_game(cfg: &Config, game: &str) -> (Config, Option<String>) {
    let mahjong = ng_cart::find_def(game)
        .is_some_and(|d| d.controller == ng_cart::types::Controller::Mahjong);
    if !mahjong || JAPAN_BIOS.contains(&cfg.bios.as_str()) {
        return (cfg.clone(), None);
    }
    match JAPAN_BIOS
        .iter()
        .find(|b| cfg.bios_dir.join(b).exists())
    {
        Some(b) => {
            let mut c = cfg.clone();
            c.bios = (*b).to_string();
            (c, Some(format!("{game} : BIOS Japon {b} sélectionné pour le panneau mahjong")))
        }
        None => (
            cfg.clone(),
            Some(format!(
                "{game} : BIOS Japon introuvable, le panneau mahjong peut ne pas fonctionner dans le menu service"
            )),
        ),
    }
}

/// Démarre le thread d'émulation.
pub fn spawn(cfg: Config) -> EmuHandle {
    let (tx, cmd_rx) = unbounded::<Cmd>();
    let (frame_tx, rx) = bounded::<FrameMsg>(2);
    let (err_tx, err_rx) = unbounded::<String>();
    let (dbg_tx, dbg_rx) = bounded::<DebugSnapshot>(2);
    let join = thread::spawn(move || run(cfg, cmd_rx, frame_tx, err_tx, dbg_tx));
    EmuHandle {
        tx,
        rx,
        err_rx,
        dbg_rx,
        join,
    }
}

fn run(
    mut cfg: Config,
    cmd_rx: Receiver<Cmd>,
    frame_tx: Sender<FrameMsg>,
    err_tx: Sender<String>,
    dbg_tx: Sender<DebugSnapshot>,
) {
    let mut machine: Option<Machine> = None;
    let audio = match AudioOut::new() {
        Ok(a) => Some(a),
        Err(e) => {
            let _ = err_tx.send(format!("audio désactivé : {e}"));
            None
        }
    };
    let mut resampler = audio
        .as_ref()
        .map(|a| AudioResampler::new(55_555.56, a.rate));
    let mut pcm: Vec<f32> = Vec::new();
    let mut paused = false;
    let mut debug = DebugLevel::Off;
    // un instantané est dû (niveau changé, pause, jeu chargé) même sans nouvelle trame
    let mut dbg_dirty = false;
    let mut step = false;
    let mut inputs = (
        PadState::default(),
        PadState::default(),
        SystemButtons::default(),
        [0xFFu8; 3],
    );
    let mut t0 = Instant::now();
    let mut frames_since_t0: u64 = 0;
    let mut fps_mark = (Instant::now(), 0u64);
    let mut fps = 0.0f32;

    loop {
        loop {
            match cmd_rx.try_recv() {
                Ok(Cmd::Quit) | Err(TryRecvError::Disconnected) => return,
                Ok(Cmd::Load { game }) => {
                    let (game_cfg, note) = config_for_game(&cfg, &game);
                    if let Some(n) = note {
                        let _ = err_tx.send(n);
                    }
                    match load_machine(&game_cfg, &game) {
                        Ok(m) => {
                            machine = Some(m);
                            t0 = Instant::now();
                            frames_since_t0 = 0;
                        }
                        Err(e) => {
                            let _ = err_tx.send(format!("{game} : {e}"));
                        }
                    }
                }
                Ok(Cmd::Reset) => {
                    if let Some(m) = machine.as_mut() {
                        m.reset();
                        t0 = Instant::now();
                        frames_since_t0 = 0;
                    }
                }
                Ok(Cmd::Pause(p)) => {
                    paused = p;
                    dbg_dirty = true;
                    t0 = Instant::now();
                    frames_since_t0 = 0;
                }
                Ok(Cmd::Config(c)) => cfg = c,
                Ok(Cmd::Debug(d)) => {
                    debug = d;
                    dbg_dirty = true;
                }
                Ok(Cmd::StepFrame) => step = true,
                Ok(Cmd::Inputs { p1, p2, sys, mj }) => inputs = (p1, p2, sys, mj),
                Err(TryRecvError::Empty) => break,
            }
        }

        let stepping = paused && std::mem::take(&mut step);
        let Some(m) = machine.as_mut().filter(|_| !paused || stepping) else {
            // à l'arrêt : rafraîchir le débogueur si son contenu demandé a changé
            if let (true, Some(m)) = (dbg_dirty && debug != DebugLevel::Off, machine.as_mut()) {
                let _ = dbg_tx.try_send(m.debug_snapshot_level(debug));
            }
            dbg_dirty = false;
            step = false;
            thread::sleep(Duration::from_millis(8));
            continue;
        };
        m.set_inputs(inputs.0, inputs.1, inputs.2);
        m.set_mahjong(inputs.3);
        m.run_frame();
        frames_since_t0 += 1;
        let samples = m.take_audio();
        if let (Some(a), Some(r)) = (audio.as_ref(), resampler.as_mut()) {
            for s in samples {
                r.push(s);
            }
            pcm.clear();
            r.pop_into(&mut pcm);
            a.push(&pcm);
        }
        let frame = m.frame_count();
        // une trame sur deux suffit à l'affichage ; toujours après un pas à pas
        if debug != DebugLevel::Off && (stepping || dbg_dirty || frame % 2 == 0) {
            let _ = dbg_tx.try_send(m.debug_snapshot_level(debug));
            dbg_dirty = false;
        }
        if fps_mark.0.elapsed() >= Duration::from_millis(500) {
            fps = (frame - fps_mark.1) as f32 / fps_mark.0.elapsed().as_secs_f32();
            fps_mark = (Instant::now(), frame);
        }
        let _ = frame_tx.try_send(FrameMsg {
            pixels: m.framebuffer().pixels.clone(),
            frame,
            fps,
        });
        if stepping {
            // pas à pas : pas de cadencement, l'horloge repart à la reprise
            t0 = Instant::now();
            frames_since_t0 = 0;
            continue;
        }
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
    fn mahjong_games_pick_the_japan_bios() {
        let dir = std::env::temp_dir().join(format!("ng_mj_bios_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut c = fallback_cfg();
        c.bios_dir = dir.clone();
        // joystick : inchangé
        let (r, n) = config_for_game(&c, "kof94");
        assert_eq!((r.bios.as_str(), n), ("euro", None));
        // mahjong sans BIOS Japon : message, BIOS inchangé
        let (r, n) = config_for_game(&c, "janshin");
        assert_eq!(r.bios, "euro");
        assert!(n.unwrap().contains("introuvable"));
        // avec vs-bios.rom : sélectionné
        std::fs::write(dir.join("vs-bios.rom"), b"x").unwrap();
        let (r, n) = config_for_game(&c, "bakatono");
        assert_eq!(r.bios, "vs-bios.rom");
        assert!(n.is_some());
        // déjà un BIOS Japon : inchangé
        c.bios = "japan-j3.bin".into();
        assert_eq!(config_for_game(&c, "minasan").0.bios, "japan-j3.bin");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_game_reports_an_error_then_quit_joins() {
        let h = spawn(bad_cfg());
        h.tx.send(Cmd::Load {
            game: "inconnu".into(),
        })
        .unwrap();
        // un éventuel message « audio désactivé » peut précéder l'erreur de chargement
        let msg = loop {
            let m = h
                .err_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("message d'erreur");
            if m.contains("inconnu") {
                break m;
            }
        };
        assert!(msg.contains("inconnu"));
        h.tx.send(Cmd::Quit).unwrap();
        h.join.join().expect("le thread se termine");
    }
}
