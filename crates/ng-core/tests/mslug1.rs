// T603 - Metal Slug 1 : démarrage et attract mode.
// Tests #[ignore] : nécessitent mslug.zip et le BIOS locaux (neogeo.toml).

use std::path::{Path, PathBuf};

use ng_cart::load_config;
use ng_core::load_machine;

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

/// Cherche la golden d'une trame (`f<N>.png` ou `f<N sur 6 chiffres>.png`).
fn golden_path(root: &Path, frame: u32) -> Option<PathBuf> {
    let dir = root.join("tests-data/golden/mslug");
    [format!("f{frame}.png"), format!("f{frame:06}.png")]
        .into_iter()
        .map(|n| dir.join(n))
        .find(|p| p.exists())
}

#[test]
#[ignore = "requires local mslug.zip and BIOS"]
fn mslug_600_frames() {
    let root = repo_root();
    let cfg = load_config(&root.join("neogeo.toml")).expect("neogeo.toml");
    let mut m = load_machine(&cfg, "mslug").expect("load mslug");

    let mut fb120 = Vec::new();
    for frame in 1..=600u32 {
        m.run_frame();
        let s = m.cpu.state();
        assert!(!m.cpu.is_halted(), "double faute à la trame {frame}");
        assert_ne!(s.pc, 0, "PC nul à la trame {frame}");
        let sp = if s.sr & 0x2000 != 0 { s.ssp } else { s.usp };
        assert!(
            (0x100000..=0x110000).contains(&sp),
            "SP 0x{sp:06X} hors RAM à la trame {frame}"
        );
        if frame == 120 {
            fb120 = m.framebuffer().pixels.clone();
        }
    }
    let fb600 = m.framebuffer().pixels.clone();
    assert_ne!(fb120, fb600, "trame 600 identique à la trame 120");

    // Goldens MAME (si présentes) : seuil initial 2 % des pixels.
    for (frame, px) in [(120u32, &fb120), (600u32, &fb600)] {
        let Some(path) = golden_path(&root, frame) else {
            eprintln!("golden absente pour la trame {frame} : comparaison ignorée");
            continue;
        };
        let (w, h, golden) = ng_testkit::golden::load_png_rgb(&path).expect("PNG golden");
        let mut fb = ng_common::Framebuffer::new();
        fb.pixels.clone_from(px);
        let d = ng_testkit::golden::diff_frame(&fb, &golden, w, h).expect("diff");
        let out = root.join(format!("target/golden-diff/mslug_f{frame}.ppm"));
        if let Some(parent) = out.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = ng_testkit::golden::write_diff_ppm(&fb, &golden, &out);
        let max = d.total * 2 / 100;
        assert!(
            d.different <= max,
            "trame {frame} : {} pixels différents sur {} (max {max})",
            d.different,
            d.total
        );
    }
}

#[test]
#[ignore = "requires local mslug.zip and BIOS"]
fn mslug_savestate_resumes_identically() {
    let root = repo_root();
    let cfg = load_config(&root.join("neogeo.toml")).expect("neogeo.toml");
    let mut m = load_machine(&cfg, "mslug").expect("load mslug");
    for _ in 0..300 {
        m.run_frame();
    }
    let saved = m.save_state();
    for _ in 0..120 {
        m.run_frame();
    }
    let fb_a = m.framebuffer().pixels.clone();
    let end_a = m.save_state();

    m.load_state(&saved).expect("load_state");
    for _ in 0..120 {
        m.run_frame();
    }
    assert!(
        m.framebuffer().pixels == fb_a,
        "image différente après reprise"
    );
    assert!(m.save_state() == end_a, "état différent après reprise");
}
