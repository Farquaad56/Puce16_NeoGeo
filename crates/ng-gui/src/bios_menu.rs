//! BIOS connus et présence dans `bios_dir` (neogeo.zip ou fichiers à plat) ; mise à jour de
//! `neogeo.toml` quand l'utilisateur change de BIOS ou de modèle.

use ng_cart::{Config, Model, Source};

use crate::settings::config_to_toml;

/// Taille d'un BIOS pris en charge (les BIOS de 512 Kio sont exclus : QUESTIONS Q-BIOS512).
const BIOS_SIZE: usize = 0x20000;

/// (fichier, libellé). MVS : macro NEOGEO_BIOS de MAME ; puis Universe BIOS et BIOS AES.
pub const KNOWN_BIOS: &[(&str, &str)] = &[
    ("sp-s2.sp1", "Europe MVS (Ver. 2)"),
    ("sp-s.sp1", "Europe MVS (Ver. 1)"),
    ("sp-s3.sp1", "Asia MV1B"),
    ("sp-u2.sp1", "US MVS (Ver. 2?)"),
    ("sp-e.sp1", "US MVS (Ver. 1)"),
    ("sp1-u2", "US MVS (4 slot, Ver 2)"),
    ("sp1-u4.bin", "US MVS (U4)"),
    ("sp1-u3.bin", "US MVS (U3)"),
    ("vs-bios.rom", "Japan MVS (Ver. 3)"),
    ("sp-j2.sp1", "Japan MVS (Ver. 2)"),
    ("sp1.jipan.1024", "Japan MVS (Ver. 1)"),
    ("japan-j3.bin", "Japan MV1B"),
    ("sp1-j3.bin", "Japan MVS (J3, alt)"),
    ("sp-1v1_3db8c.bin", "Custom Japanese Hotel"),
    ("uni-bios_4_0.rom", "Universe BIOS 4.0"),
    ("neo-epo.bin", "AES Export"),
    ("neo-po.bin", "AES Japan"),
];

/// Un BIOS de la liste et sa disponibilité.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BiosEntry {
    pub file: &'static str,
    pub label: &'static str,
    pub present: bool,
}

/// Libellé d'un fichier BIOS (le nom du fichier s'il est inconnu).
pub fn bios_label(file: &str) -> &str {
    KNOWN_BIOS
        .iter()
        .find(|(f, _)| *f == file)
        .map_or(file, |(_, l)| l)
}

/// Liste des BIOS connus ; présent = fichier de 128 Kio lisible, avec les ROM communes
/// (`000-lo.lo`, `sfix.sfix`, `sm1.sm1`) sans lesquelles aucun BIOS ne se charge.
pub fn available_bios(cfg: &Config) -> Vec<BiosEntry> {
    let zip = cfg.bios_dir.join("neogeo.zip");
    let mut src = if zip.exists() {
        Source::open(&zip).ok()
    } else if cfg.bios_dir.is_dir() {
        Some(Source::Dir(cfg.bios_dir.clone()))
    } else {
        None
    };
    let mut has = |name: &str| {
        src.as_mut()
            .and_then(|s| s.read(name).ok())
            .is_some_and(|d| d.len() == BIOS_SIZE)
    };
    let common = has("000-lo.lo") && has("sfix.sfix") && has("sm1.sm1");
    KNOWN_BIOS
        .iter()
        .map(|&(file, label)| BiosEntry {
            file,
            label,
            present: common && has(file),
        })
        .collect()
}

fn model_str(m: &Model) -> &'static str {
    match m {
        Model::Mvs => "mvs",
        Model::Aes => "aes",
    }
}

/// Nouveau texte de `neogeo.toml` : remplace les lignes `bios =` et `model =` en gardant le reste
/// (commentaires, clés inconnues). Si le résultat ne se relit pas à l'identique, réécrit tout.
pub fn updated_toml(old: &str, cfg: &Config) -> String {
    let is_key = |line: &str, key: &str| {
        line.trim_start()
            .strip_prefix(key)
            .is_some_and(|r| r.trim_start().starts_with('='))
    };
    let mut out = String::new();
    for line in old.lines() {
        if is_key(line, "bios") {
            out.push_str(&format!("bios = {:?}", cfg.bios));
        } else if is_key(line, "model") {
            out.push_str(&format!("model = {:?}", model_str(&cfg.model)));
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    match ng_cart::parse_config(&out) {
        Ok(c)
            if c.bios == cfg.bios
                && c.model == cfg.model
                && c.bios_dir == cfg.bios_dir
                && c.roms_dir == cfg.roms_dir
                && c.saves_dir == cfg.saves_dir =>
        {
            out
        }
        _ => config_to_toml(cfg),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn cfg(dir: &std::path::Path) -> Config {
        Config {
            bios_dir: dir.to_path_buf(),
            roms_dir: dir.to_path_buf(),
            saves_dir: PathBuf::from("saves"),
            model: Model::Mvs,
            bios: "sp-s2.sp1".into(),
        }
    }

    #[test]
    fn availability_follows_the_files() {
        let dir = std::env::temp_dir().join(format!("ng_gui_bios_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let c = cfg(&dir);
        assert!(available_bios(&c).iter().all(|b| !b.present));

        let rom = vec![0u8; BIOS_SIZE];
        std::fs::write(dir.join("sp-s2.sp1"), &rom).unwrap();
        // sans les ROM communes, aucun BIOS n'est utilisable
        assert!(available_bios(&c).iter().all(|b| !b.present));
        for f in ["000-lo.lo", "sfix.sfix", "sm1.sm1"] {
            std::fs::write(dir.join(f), &rom).unwrap();
        }
        // un fichier de mauvaise taille est refusé
        std::fs::write(dir.join("sp-s.sp1"), vec![0u8; 100]).unwrap();
        let present: Vec<_> = available_bios(&c)
            .into_iter()
            .filter(|b| b.present)
            .map(|b| b.file)
            .collect();
        assert_eq!(present, ["sp-s2.sp1"]);
        assert!(ng_cart::load_bios(&dir, "sp-s2.sp1").is_ok());
        let _ = std::fs::remove_dir_all(&dir);

        assert!(available_bios(&cfg(std::path::Path::new("/inexistant")))
            .iter()
            .all(|b| !b.present));
    }

    #[test]
    fn no_512k_bios_in_the_list() {
        for f in ["sp-45.sp1", "sp-j3.sp1"] {
            assert!(KNOWN_BIOS.iter().all(|(file, _)| *file != f));
        }
        assert_eq!(bios_label("sp-s2.sp1"), "Europe MVS (Ver. 2)");
        assert_eq!(bios_label("perso.bin"), "perso.bin");
    }

    #[test]
    fn toml_update_keeps_other_lines() {
        let old = "# mon fichier\n[paths]\nbios_dir  = \"B\"\nroms_dir  = \"R\"\nsaves_dir = \"saves\"\n\n[system]\nmodel  = \"mvs\"\nbios   = \"sp-s2.sp1\"\nregion = \"euro\"\n";
        let mut c = Config {
            bios_dir: PathBuf::from("B"),
            roms_dir: PathBuf::from("R"),
            saves_dir: PathBuf::from("saves"),
            model: Model::Aes,
            bios: "neo-epo.bin".into(),
        };
        let new = updated_toml(old, &c);
        assert!(new.contains("# mon fichier") && new.contains("region = \"euro\""));
        let back = ng_cart::parse_config(&new).unwrap();
        assert_eq!(
            (back.bios.as_str(), back.model),
            ("neo-epo.bin", Model::Aes)
        );

        // fichier inutilisable : réécriture complète
        c.model = Model::Mvs;
        let back = ng_cart::parse_config(&updated_toml("n'importe quoi", &c)).unwrap();
        assert_eq!(
            (back.bios.as_str(), back.model),
            ("neo-epo.bin", Model::Mvs)
        );
    }
}
