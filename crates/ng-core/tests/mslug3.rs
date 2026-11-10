// T708 - Metal Slug 3 : démarrage et attract mode.
// Tests #[ignore] : nécessitent mslug3.zip et le BIOS locaux (neogeo.toml).

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
    let dir = root.join("tests-data/golden/mslug3");
    [format!("f{frame}.png"), format!("f{frame:06}.png")]
        .into_iter()
        .map(|n| dir.join(n))
        .find(|p| p.exists())
}

#[test]
#[ignore = "requires local mslug3.zip and BIOS"]
fn mslug3_600_frames() {
    let root = repo_root();
    let cfg = load_config(&root.join("neogeo.toml")).expect("neogeo.toml");
    let mut m = load_machine(&cfg, "mslug3").expect("load mslug3");

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
                &format!("mslug3_ours_f{frame}.ppm"),
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
        let out = root.join(format!("target/golden-diff/mslug3_f{frame}.ppm"));
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
#[ignore = "requires local mslug3.zip and BIOS"]
fn mslug3_1500_frames() {
    let root = repo_root();
    let cfg = load_config(&root.join("neogeo.toml")).expect("neogeo.toml");
    let mut m = load_machine(&cfg, "mslug3").expect("load mslug3");

    assert!(m.bus.port.is_some(), "port SMA non installé");
    // Le premier vecteur (SP initial) lu dans la P ROM déchiffrée vaut 0x0010F300.
    let sp = u32::from_be_bytes(m.bus.cart_p[0..4].try_into().unwrap());
    assert_eq!(sp, 0x0010_F300, "P ROM mal déchiffrée");
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
            dump_ppm(&root, "mslug3_ours_f1500.ppm", &m.framebuffer().pixels);
        }
    }
    // Le jeu dépasse l'écran de titre : la trame 1500 diffère de la 600.
    assert!(
        fb600 != m.framebuffer().pixels,
        "trame 1500 identique à la 600"
    );
    // La banque $2FFFE4 est sollicitée pendant l'attract.
    let banks = m.bus.port.as_ref().map_or(0, |p| p.bank_change_count());
    assert!(banks > 0, "aucun changement de banque SMA");
    eprintln!("changements de banque SMA : {banks}");

    // L'image doit être celle d'une scène de jeu (pas un écran noir ni un écran de garbage uni).
    // Pas de comparaison pixel à pixel avec MAME ici : la scène de démo affichée à la trame 1500
    // dépend du déroulé de MAME, qui varie d'une exécution à l'autre (voir docs/STATE.md, T708).
    let fb = m.framebuffer().pixels.clone();
    let mut counts = std::collections::HashMap::new();
    for p in &fb {
        *counts.entry(*p).or_insert(0usize) += 1;
    }
    let dominant = counts.values().copied().max().unwrap_or(0);
    assert!(
        counts.len() >= 64,
        "trame 1500 : {} couleurs seulement",
        counts.len()
    );
    assert!(
        dominant * 100 / fb.len() < 50,
        "trame 1500 dominée par une seule couleur ({dominant} pixels)"
    );
    if let Some(path) = golden_path(&root, 1500) {
        let (w, h, golden) = ng_testkit::golden::load_png_rgb(&path).expect("PNG golden");
        let mut fbf = ng_common::Framebuffer::new();
        fbf.pixels.clone_from(&fb);
        let d = ng_testkit::golden::diff_frame(&fbf, &golden, w, h).expect("diff");
        eprintln!(
            "trame 1500 : {} pixels différents sur {} (information)",
            d.different, d.total
        );
    }
}
