// T710 - Metal Slug 5 : démarrage et attract mode.
// Tests #[ignore] : nécessitent mslug5.zip et le BIOS locaux (neogeo.toml).

use std::path::{Path, PathBuf};

use ng_cart::load_config;
use ng_core::load_machine;

/// Écrit la trame en PPM (P6) pour inspection visuelle.
fn dump_ppm(root: &Path, name: &str, pixels: &[u32]) {
    let dir = root.join("target/golden-diff");
    let _ = std::fs::create_dir_all(&dir);
    let mut out = b"P6\n320 224\n255\n".to_vec();
    for p in pixels {
        out.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, *p as u8]);
    }
    let _ = std::fs::write(dir.join(name), out);
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

/// Cherche la golden d'une trame (`f<N>.png` ou `f<N sur 6 chiffres>.png`).
fn golden_path(root: &Path, frame: u32) -> Option<PathBuf> {
    let dir = root.join("tests-data/golden/mslug5");
    [format!("f{frame}.png"), format!("f{frame:06}.png")]
        .into_iter()
        .map(|n| dir.join(n))
        .find(|p| p.exists())
}

#[test]
#[ignore = "requires local mslug5.zip and BIOS"]
fn mslug5_600_frames() {
    let root = repo_root();
    let cfg = load_config(&root.join("neogeo.toml")).expect("neogeo.toml");
    let mut m = load_machine(&cfg, "mslug5").expect("load mslug5");

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
        if frame == 120 || frame == 600 {
            dump_ppm(
                &root,
                &format!("mslug5_ours_f{frame}.ppm"),
                &m.framebuffer().pixels,
            );
        }
    }
    let fb600 = m.framebuffer().pixels.clone();
    assert!(fb120 != fb600, "trame 600 identique à la trame 120");

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
        let out = root.join(format!("target/golden-diff/mslug5_f{frame}.ppm"));
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
#[ignore = "requires local mslug5.zip and BIOS"]
fn mslug5_1500_frames() {
    let root = repo_root();
    let cfg = load_config(&root.join("neogeo.toml")).expect("neogeo.toml");
    let mut m = load_machine(&cfg, "mslug5").expect("load mslug5");

    assert!(m.bus.port.is_some(), "port PVC non installé");
    let mut fb600 = Vec::new();
    for frame in 1..=1500u32 {
        m.run_frame();
        let s = m.cpu.state();
        assert!(!m.cpu.is_halted(), "double faute à la trame {frame}");
        assert_ne!(s.pc, 0, "PC nul à la trame {frame}");
        if frame == 600 {
            fb600 = m.framebuffer().pixels.clone();
        }
        if frame == 1500 {
            dump_ppm(&root, "mslug5_ours_f1500.ppm", &m.framebuffer().pixels);
        }
    }
    // Le jeu dépasse l'écran de titre : la trame 1500 diffère de la 600.
    assert!(
        fb600 != m.framebuffer().pixels,
        "trame 1500 identique à la 600"
    );
    // Les écritures dans la RAM PVC ($2FFFF0...) provoquent des changements de banque.
    let banks = m.bus.port.as_ref().map_or(0, |p| p.bank_change_count());
    assert!(banks > 0, "aucun changement de banque PVC");
    eprintln!("changements de banque PVC : {banks}");

    // Golden MAME frame 1500 (si présente)
    let Some(path) = golden_path(&root, 1500) else {
        eprintln!("golden absente pour la trame 1500 : comparaison ignorée");
        return;
    };
    let fb = m.framebuffer().pixels.clone();
    let (w, h, golden) = ng_testkit::golden::load_png_rgb(&path).expect("PNG golden");
    let mut fbf = ng_common::Framebuffer::new();
    fbf.pixels.clone_from(&fb);
    let d = ng_testkit::golden::diff_frame(&fbf, &golden, w, h).expect("diff");
    let out = root.join("target/golden-diff/mslug5_f1500.ppm");
    if let Some(parent) = out.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = ng_testkit::golden::write_diff_ppm(&fbf, &golden, &out);
    let max = d.total * 2 / 100;
    assert!(
        d.different <= max,
        "trame 1500 : {} pixels différents sur {} (max {max})",
        d.different,
        d.total
    );
}
