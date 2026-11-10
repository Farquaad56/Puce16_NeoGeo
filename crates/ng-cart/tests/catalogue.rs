// T1003 - Contrôle d'intégrité du catalogue : chaque définition doit
// correspondre aux ROM réelles (CRC compris). Test #[ignore] : les zips
// sont locaux (jamais commités). Jeu inconnu dans NG_GAMES => compté ABSENT.

use std::path::PathBuf;

use ng_cart::defs::ALL_CARTS;
use ng_cart::load_cart;

fn roms_dir() -> PathBuf {
    match std::env::var_os("NG_ROMS") {
        Some(p) => PathBuf::from(p),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../roms"),
    }
}

#[test]
#[ignore = "requires local ROM zips"]
fn catalogue_integrity() {
    let dir = roms_dir();
    let filter: Option<Vec<String>> = std::env::var("NG_GAMES").ok().map(|s| {
        s.split(',')
            .map(|g| g.trim().to_ascii_lowercase())
            .filter(|g| !g.is_empty())
            .collect()
    });

    let (mut ok, mut absent) = (0usize, 0usize);
    let mut fails: Vec<(String, String)> = Vec::new();

    // Jeux demandés mais absents du catalogue : comptés « absent ».
    if let Some(f) = &filter {
        absent += f
            .iter()
            .filter(|g| !ALL_CARTS.iter().any(|d| d.name.eq_ignore_ascii_case(g)))
            .count();
    }

    for def in ALL_CARTS.iter() {
        if let Some(f) = &filter {
            if !f.iter().any(|g| g.eq_ignore_ascii_case(def.name)) {
                continue;
            }
        }
        let zip = dir.join(format!("{}.zip", def.name));
        if !zip.exists() {
            absent += 1;
            continue;
        }
        match load_cart(&zip, def, true) {
            Ok(_) => ok += 1,
            Err(e) => fails.push((def.name.to_string(), format!("{e:?}"))),
        }
    }

    println!("OK {ok} / ABSENT {absent} / ECHEC {}", fails.len());
    for (n, e) in &fails {
        println!("ÉCHEC {n}: {e}");
    }
    assert!(fails.is_empty(), "{} jeux en échec", fails.len());
}
