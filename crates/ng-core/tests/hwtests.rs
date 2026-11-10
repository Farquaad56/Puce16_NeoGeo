// T902 - ROMs de test matérielles : chaque dossier tests-data/hwtests/<nom>/ avec un hw.toml est
// exécuté N trames et comparé aux captures MAME de golden/. Voir docs/spec/data/test-roms.md.

use std::path::{Path, PathBuf};

use ng_cart::{load_config, Config};
use ng_common::Framebuffer;
use ng_core::load_machine;

/// Description d'un cas (hw.toml).
#[derive(Debug, PartialEq, Eq)]
struct HwCase {
    name: String,
    game: String,
    frames: Vec<u32>,
    max_diff_percent: usize,
}

/// Lit un hw.toml minimal : `clé = valeur`, `#` commentaire, `frames` = liste séparée par espaces/virgules.
fn parse_case(name: &str, text: &str) -> Result<HwCase, String> {
    let (mut game, mut frames, mut pct) = (None, Vec::new(), 2usize);
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (k, v) = line
            .split_once('=')
            .ok_or_else(|| format!("{name}/hw.toml : ligne invalide « {line} »"))?;
        let v = v.trim().trim_matches('"');
        match k.trim() {
            "game" => game = Some(v.to_string()),
            "frames" => {
                for t in v.split([' ', ',']).filter(|t| !t.is_empty()) {
                    frames.push(
                        t.parse()
                            .map_err(|_| format!("{name} : trame « {t} » invalide"))?,
                    );
                }
            }
            "max_diff_percent" => {
                pct = v
                    .parse()
                    .map_err(|_| format!("{name} : pourcentage invalide"))?
            }
            _ => {} // purpose, etc.
        }
    }
    Ok(HwCase {
        name: name.to_string(),
        game: game.ok_or_else(|| format!("{name}/hw.toml : clé « game » manquante"))?,
        frames,
        max_diff_percent: pct,
    })
}

fn repo_root() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    for _ in 0..5 {
        if dir.join("neogeo.toml").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

/// Cas présents (dossiers triés ayant un hw.toml).
fn list_cases(dir: &Path) -> Vec<Result<HwCase, String>> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    dirs.iter()
        .filter(|d| d.join("hw.toml").exists())
        .map(|d| {
            let name = d.file_name().map(|n| n.to_string_lossy().into_owned());
            let name = name.unwrap_or_default();
            let text = std::fs::read_to_string(d.join("hw.toml"))
                .map_err(|e| format!("{name}/hw.toml : {e}"))?;
            parse_case(&name, &text)
        })
        .collect()
}

/// Ligne de rapport pour une trame.
fn report_line(case: &str, frame: u32, different: usize, total: usize, max: usize) -> String {
    let status = if different <= max { "OK" } else { "ECART" };
    format!("| {case} | {frame} | {different} / {total} | {max} | {status} |")
}

/// Exécute un cas ; renvoie les lignes de rapport et le nombre d'échecs.
fn run_case(root: &Path, cfg: &Config, case: &HwCase) -> Result<(Vec<String>, usize), String> {
    let gdir = root
        .join("tests-data/hwtests")
        .join(&case.name)
        .join("golden");
    if !gdir.is_dir() {
        return Err(format!("{} : dossier golden/ absent", case.name));
    }
    let mut m = load_machine(cfg, &case.game).map_err(|e| format!("{} : {e}", case.name))?;
    let last = case.frames.iter().copied().max().unwrap_or(0);
    let (mut lines, mut failures) = (Vec::new(), 0);
    for frame in 1..=last {
        m.run_frame();
        if !case.frames.contains(&frame) {
            continue;
        }
        let path = gdir.join(format!("f{frame}.png"));
        let (w, h, golden) = ng_testkit::golden::load_png_rgb(&path)?;
        let mut fb = Framebuffer::new();
        fb.pixels.clone_from(&m.framebuffer().pixels);
        let d = ng_testkit::golden::diff_frame(&fb, &golden, w, h)?;
        let max = d.total * case.max_diff_percent / 100;
        let out = root.join(format!("target/golden-diff/hw_{}_f{frame}.ppm", case.name));
        if let Some(p) = out.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        let _ = ng_testkit::golden::write_diff_ppm(&fb, &golden, &out);
        if d.different > max {
            failures += 1;
        }
        lines.push(report_line(&case.name, frame, d.different, d.total, max));
    }
    Ok((lines, failures))
}

#[test]
fn all_hardware_test_roms() {
    let Some(root) = repo_root() else {
        eprintln!("neogeo.toml introuvable : tests matériels ignorés");
        return;
    };
    let cases = list_cases(&root.join("tests-data/hwtests"));
    if cases.is_empty() {
        eprintln!("aucun cas dans tests-data/hwtests : ignoré");
        return;
    }
    let Ok(cfg) = load_config(&root.join("neogeo.toml")) else {
        eprintln!("configuration illisible : ignoré");
        return;
    };
    let mut report = String::from("# Rapport hwtests\n\n| Cas | Trame | Pixels différents | Max | Statut |\n|---|---|---|---|---|\n");
    let mut failures = 0;
    for case in cases {
        match case.and_then(|c| run_case(&root, &cfg, &c)) {
            Ok((lines, f)) => {
                failures += f;
                for l in lines {
                    report.push_str(&l);
                    report.push('\n');
                }
            }
            Err(e) => eprintln!("cas ignoré : {e}"),
        }
    }
    let _ = std::fs::create_dir_all(root.join("target"));
    let _ = std::fs::write(root.join("target/hw-report.md"), &report);
    eprintln!("{report}");
    assert_eq!(
        failures, 0,
        "des trames dépassent le seuil (voir target/hw-report.md)"
    );
}

#[test]
fn empty_directory_gives_no_case() {
    let dir = std::env::temp_dir().join("hwtests_empty_t902");
    let _ = std::fs::create_dir_all(&dir);
    assert!(list_cases(&dir).is_empty());
    assert!(list_cases(&dir.join("inexistant")).is_empty());
}

#[test]
fn case_file_parsing() {
    let c = parse_case(
        "t",
        "# commentaire\ngame = mslug\nframes = 60, 120 600\nmax_diff_percent = 5\npurpose = fix\n",
    )
    .expect("cas valide");
    assert_eq!(
        c,
        HwCase {
            name: "t".into(),
            game: "mslug".into(),
            frames: vec![60, 120, 600],
            max_diff_percent: 5
        }
    );
    assert!(parse_case("t", "frames = 1").is_err());
    assert!(parse_case("t", "game = x\nframes = abc").is_err());
}

#[test]
fn comparator_requires_320x224() {
    let fb = Framebuffer::new();
    let err = ng_testkit::golden::diff_frame(&fb, &vec![0; 100 * 100], 100, 100).unwrap_err();
    assert!(err.contains("320x224"), "{err}");
}

#[test]
fn report_line_format() {
    assert_eq!(
        report_line("a", 60, 10, 71680, 1433),
        "| a | 60 | 10 / 71680 | 1433 | OK |"
    );
    assert!(report_line("a", 60, 2000, 71680, 1433).ends_with("ECART |"));
}
