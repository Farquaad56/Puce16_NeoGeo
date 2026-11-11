// Banc de vitesse du noyau d'émulation (hors interface) : ms par trame sur quelques jeux.
// Test réel #[ignore] (zips + BIOS locaux) :
//   cargo test --release -p ng-core --test speed -- --ignored --nocapture
// NG_GAMES=a,b filtre les jeux ; NG_FRAMES = trames mesurées (défaut 1200).

use std::path::PathBuf;
use std::time::Instant;

use ng_cart::load_config;
use ng_core::load_machine;

/// Durée d'une trame à vitesse réelle : 405 504 mclk à 24 MHz.
const FRAME_MS: f64 = 405_504.0 / 24_000_000.0 * 1000.0;

#[test]
#[ignore]
fn core_speed() {
    let mut dir = std::env::current_dir().unwrap();
    while !dir.join("neogeo.toml").exists() {
        assert!(dir.pop(), "neogeo.toml introuvable");
    }
    let cfg = load_config(&dir.join("neogeo.toml")).expect("neogeo.toml");
    let games = std::env::var("NG_GAMES")
        .unwrap_or_else(|_| "mslug3,kof2003,kof98,samsho5,garou,mslugx".into());
    let frames: u32 = std::env::var("NG_FRAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1200);
    println!("budget : {FRAME_MS:.2} ms/trame (59,19 trames/s)");
    for game in games.split(',') {
        let zip: PathBuf = cfg.roms_dir.join(format!("{game}.zip"));
        if !zip.exists() {
            println!("{game:10} absent");
            continue;
        }
        let mut m = match load_machine(&cfg, game) {
            Ok(m) => m,
            Err(e) => {
                println!("{game:10} échec : {e:?}");
                continue;
            }
        };
        for _ in 0..300 {
            m.run_frame(); // démarrage hors mesure
        }
        let mut t: Vec<f64> = Vec::with_capacity(frames as usize);
        for _ in 0..frames {
            let t0 = Instant::now();
            m.run_frame();
            t.push(t0.elapsed().as_secs_f64() * 1000.0);
        }
        let avg = t.iter().sum::<f64>() / t.len() as f64;
        t.sort_by(|a, b| a.total_cmp(b));
        let p99 = t[(t.len() * 99 / 100).min(t.len() - 1)];
        println!(
            "{game:10} moy {avg:6.2} ms  p99 {p99:6.2} ms  max {:6.2} ms  => x{:.1} le temps réel",
            t[t.len() - 1],
            FRAME_MS / avg
        );
    }
}
