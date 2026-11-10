// T1004 - Banc générique du catalogue : démarre chaque jeu (600 trames), vérifie la santé,
// compare aux goldens MAME locales si elles existent, écrit target/catalogue-report.md.
// Tests réels #[ignore] : zips + BIOS locaux. NG_GAMES=a,b,c filtre ; NG_STRICT=1 durcit.

use std::collections::HashSet;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::time::Instant;

use ng_cart::defs::ALL_CARTS;
use ng_cart::load_config;
use ng_core::load_machine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Etat {
    Ok,
    Noir,
    Diff,
    Echec,
}

/// Classe un jeu : moins de 16 couleurs distinctes => Noir ; diff > 2 % => Diff.
fn classify(distinct_colors: usize, diff600: Option<f64>) -> Etat {
    // Une image conforme à la golden MAME est « ok » même si elle est pauvre en couleurs
    // (démarrage à froid : le BIOS peut encore être à l'écran de test).
    if let Some(d) = diff600 {
        return if d > 2.0 { Etat::Diff } else { Etat::Ok };
    }
    if distinct_colors < 16 {
        return Etat::Noir;
    }
    match diff600 {
        Some(d) if d > 2.0 => Etat::Diff,
        _ => Etat::Ok,
    }
}

fn repo_root() -> PathBuf {
    let mut dir = std::env::current_dir().expect("current dir");
    for _ in 0..5 {
        if dir.join("neogeo.toml").exists() {
            return dir;
        }
        if !dir.pop() {
            break;
        }
    }
    panic!("neogeo.toml introuvable");
}

fn golden_path(root: &Path, game: &str, frame: u32) -> Option<PathBuf> {
    let dir = root.join("tests-data/golden").join(game);
    [format!("f{frame}.png"), format!("f{frame:06}.png")]
        .into_iter()
        .map(|n| dir.join(n))
        .find(|p| p.exists())
}

fn diff_percent(root: &Path, game: &str, frame: u32, px: &[u32]) -> Option<f64> {
    let path = golden_path(root, game, frame)?;
    let (w, h, golden) = ng_testkit::golden::load_png_rgb(&path).ok()?;
    let mut fb = ng_common::Framebuffer::new();
    fb.pixels.clear();
    fb.pixels.extend_from_slice(px);
    let d = ng_testkit::golden::diff_frame(&fb, &golden, w, h).ok()?;
    let out = root.join(format!("target/golden-diff/{game}_f{frame}.ppm"));
    if let Some(parent) = out.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = ng_testkit::golden::write_diff_ppm(&fb, &golden, &out);
    Some(d.different as f64 * 100.0 / d.total as f64)
}

struct Row {
    game: String,
    etat: Etat,
    note: String,
    diff120: Option<f64>,
    diff600: Option<f64>,
    colors: usize,
    secs: f64,
}

fn run_game(
    frames: u32,
    root: &Path,
    cfg: &ng_cart::Config,
    game: &str,
) -> Result<(usize, Option<f64>, Option<f64>), String> {
    let mut m = load_machine(cfg, game).map_err(|e| format!("{e:?}"))?;
    let mut fb120: Vec<u32> = Vec::new();
    for frame in 1..=frames {
        m.run_frame();
        let s = m.cpu.state();
        if m.cpu.is_halted() {
            return Err(format!("double faute à la trame {frame}"));
        }
        let sp = if s.sr & 0x2000 != 0 { s.ssp } else { s.usp };
        if !(0x100000..=0x110000).contains(&sp) {
            return Err(format!("SP 0x{sp:06X} hors RAM à la trame {frame}"));
        }
        if frame == 120 {
            fb120 = m.framebuffer().pixels.clone();
        }
    }
    let fb600 = m.framebuffer().pixels.clone();
    let colors = fb600.iter().collect::<HashSet<_>>().len();
    let d120 = diff_percent(root, game, 120, &fb120);
    let d600 = diff_percent(root, game, frames, &fb600);
    Ok((colors, d120, d600))
}

fn fmt_pct(v: Option<f64>) -> String {
    v.map_or("-".to_string(), |d| format!("{d:.2}%"))
}

#[test]
#[ignore = "requires local ROM zips and BIOS"]
fn catalogue_boot_bench() {
    let root = repo_root();
    let mut cfg = load_config(&root.join("neogeo.toml")).expect("neogeo.toml");
    // Les goldens MAME sont faites avec le BIOS Europe MVS v2 (défaut de MAME) : on force
    // ce BIOS pour rester comparable (NG_BIOS=<fichier> pour en choisir un autre).
    cfg.bios = std::env::var("NG_BIOS").unwrap_or_else(|_| "sp-s2.sp1".to_string());
    let roms = cfg.roms_dir.clone();
    let filter: Option<Vec<String>> = std::env::var("NG_GAMES").ok().map(|s| {
        s.split(',')
            .map(|g| g.trim().to_ascii_lowercase())
            .filter(|g| !g.is_empty())
            .collect()
    });
    let games: Vec<&str> = ALL_CARTS
        .iter()
        .map(|d| d.name)
        .filter(|n| match &filter {
            Some(f) => f.iter().any(|g| g == n),
            None => roms.join(format!("{n}.zip")).exists(),
        })
        .collect();

    // NG_FRAMES : trame finale (600 par défaut ; 1500 laisse le temps à l'attract mode).
    let frames: u32 = std::env::var("NG_FRAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(600);
    let mut rows = Vec::new();
    for game in games {
        let t0 = Instant::now();
        let res = catch_unwind(AssertUnwindSafe(|| run_game(frames, &root, &cfg, game)));
        let secs = t0.elapsed().as_secs_f64();
        let row = match res {
            Ok(Ok((colors, d120, d600))) => Row {
                game: game.to_string(),
                etat: classify(colors, d600),
                note: String::new(),
                diff120: d120,
                diff600: d600,
                colors,
                secs,
            },
            Ok(Err(e)) => Row {
                game: game.to_string(),
                etat: Etat::Echec,
                note: e,
                diff120: None,
                diff600: None,
                colors: 0,
                secs,
            },
            Err(_) => Row {
                game: game.to_string(),
                etat: Etat::Echec,
                note: "panique".to_string(),
                diff120: None,
                diff600: None,
                colors: 0,
                secs,
            },
        };
        eprintln!("{} -> {:?} {}", row.game, row.etat, row.note);
        rows.push(row);
    }

    let mut md = String::from("# Rapport catalogue\n\n| jeu | état | diff120 | diff finale | couleurs | secondes |\n|---|---|---|---|---|---|\n");
    for r in &rows {
        let etat = match r.etat {
            Etat::Ok => "ok".to_string(),
            Etat::Noir => "noir".to_string(),
            Etat::Diff => format!("diff {}", fmt_pct(r.diff600)),
            Etat::Echec => format!("echec {}", r.note),
        };
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {:.1} |\n",
            r.game,
            etat,
            fmt_pct(r.diff120),
            fmt_pct(r.diff600),
            r.colors,
            r.secs
        ));
    }
    println!("{md}");
    let out = root.join("target/catalogue-report.md");
    let _ = std::fs::write(&out, &md);

    let strict = std::env::var("NG_STRICT").is_ok_and(|v| v == "1");
    let bad = rows
        .iter()
        .filter(|r| r.etat == Etat::Echec || (strict && matches!(r.etat, Etat::Noir | Etat::Diff)))
        .count();
    assert!(
        bad == 0,
        "{bad} jeux en échec (voir target/catalogue-report.md)"
    );
}

#[test]
fn classify_rules() {
    assert_eq!(classify(3, None), Etat::Noir);
    assert_eq!(classify(40, None), Etat::Ok);
    assert_eq!(classify(40, Some(1.9)), Etat::Ok);
    assert_eq!(classify(40, Some(2.1)), Etat::Diff);
}
