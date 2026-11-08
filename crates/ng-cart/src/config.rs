use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub enum Model {
    Mvs,
    Aes,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub bios_dir: PathBuf,
    pub roms_dir: PathBuf,
    pub saves_dir: PathBuf,
    pub model: Model,
    pub bios: String,
}

fn get_leaf_str(table: &toml::Table, path: &[&str]) -> Result<String, String> {
    let mut current = table;
    for (i, key) in path.iter().enumerate() {
        if i == path.len() - 1 {
            match current.get(*key) {
                Some(toml::Value::String(s)) => return Ok(s.clone()),
                _ => return Err(format!("missing or invalid key: {}", path.join("."))),
            }
        } else {
            match current.get(*key) {
                Some(toml::Value::Table(sub)) => current = sub,
                _ => return Err(format!("missing or invalid key: {}", path[..=i].join("."))),
            }
        }
    }
    unreachable!()
}

pub fn parse_config(text: &str) -> Result<Config, String> {
    let table: toml::Table = text
        .parse()
        .map_err(|e| format!("TOML parse error: {}", e))?;

    let bios_dir = get_leaf_str(&table, &["paths", "bios_dir"])?;
    let roms_dir = get_leaf_str(&table, &["paths", "roms_dir"])?;
    let saves_dir = get_leaf_str(&table, &["paths", "saves_dir"])?;

    let model_str = get_leaf_str(&table, &["system", "model"])?.to_lowercase();
    let model = match model_str.as_str() {
        "mvs" => Model::Mvs,
        "aes" => Model::Aes,
        other => return Err(format!("invalid system.model: {}", other)),
    };

    let bios = get_leaf_str(&table, &["system", "bios"])?;

    Ok(Config {
        bios_dir: PathBuf::from(bios_dir),
        roms_dir: PathBuf::from(roms_dir),
        saves_dir: PathBuf::from(saves_dir),
        model,
        bios,
    })
}

pub fn load_config(path: &Path) -> Result<Config, String> {
    let text =
        fs::read_to_string(path).map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
    parse_config(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = r#"
[paths]
bios_dir  = "H:/__Emulator__/Puce16_NeoGeo/roms"
roms_dir  = "H:/__Emulator__/Puce16_NeoGeo/roms"
saves_dir = "saves"

[system]
model  = "mvs"
bios   = "sp-s2.sp1"
region = "euro"
"#;

    #[test]
    fn test_parse_example() {
        let cfg = parse_config(EXAMPLE).unwrap();
        assert_eq!(cfg.model, Model::Mvs);
        assert_eq!(cfg.bios, "sp-s2.sp1");
        assert_eq!(
            cfg.bios_dir.to_string_lossy(),
            "H:/__Emulator__/Puce16_NeoGeo/roms"
        );
    }

    #[test]
    fn test_parse_aes() {
        let text = r#"
[paths]
bios_dir = "/bios"
roms_dir = "/roms"
saves_dir = "/saves"

[system]
model = "AES"
bios = "sp-s2.sp1"
"#;
        let cfg = parse_config(text).unwrap();
        assert_eq!(cfg.model, Model::Aes);
    }

    #[test]
    fn test_parse_missing_key() {
        let text = r#"
[paths]
bios_dir = "/bios"
roms_dir = "/roms"

[system]
model = "mvs"
bios = "sp-s2.sp1"
"#;
        let result = parse_config(text);
        assert!(result.is_err());
    }
}
