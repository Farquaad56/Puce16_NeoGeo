use std::path::Path;

use crate::romset::Source;

#[derive(Debug)]
pub struct Bios {
    pub main: Vec<u8>,
    pub lo: Vec<u8>,
    pub sfix: Vec<u8>,
    pub sm1: Vec<u8>,
}

#[derive(Debug)]
pub enum BiosError {
    Io(std::io::Error),
    MissingFile(String),
    UnsupportedSize(u64),
}

impl From<std::io::Error> for BiosError {
    fn from(e: std::io::Error) -> Self {
        BiosError::Io(e)
    }
}

const BIOS_SIZE: usize = 0x20000;

fn word_swap(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; data.len()];
    for k in 0..data.len() / 2 {
        out[2 * k] = data[2 * k + 1];
        out[2 * k + 1] = data[2 * k];
    }
    out
}

fn open_bios_source(dir: &Path) -> Result<Source, BiosError> {
    // Try neogeo.zip first, fall back to directory
    let zip_path = dir.join("neogeo.zip");
    if zip_path.exists() {
        Source::open(&zip_path)
            .map_err(|e| BiosError::Io(std::io::Error::other(format!("{:?}", e))))
    } else {
        Ok(Source::Dir(dir.to_path_buf()))
    }
}

pub fn load_bios(dir: &Path, main_file: &str) -> Result<Bios, BiosError> {
    let mut src = open_bios_source(dir)?;
    let raw_main = src
        .read(main_file)
        .map_err(|e| BiosError::MissingFile(format!("{}: {:?}", main_file, e)))?;
    build_bios(src, &raw_main)
}

/// Comme `load_bios`, avec un BIOS principal fourni (non permuté), par exemple le `236-bios.sp1`
/// contenu dans le zip du jeu (irrmaze). Les ROM auxiliaires viennent du dossier des BIOS.
pub fn load_bios_with_main(dir: &Path, raw_main: &[u8]) -> Result<Bios, BiosError> {
    build_bios(open_bios_source(dir)?, raw_main)
}

/// BIOS principal propre à un jeu, lu dans son zip : `236-bios.sp1` d'irrmaze (« Asia MV1B 263 »,
/// seul BIOS qui gère le trackball). `None` si le jeu n'en a pas besoin ou si le fichier manque.
pub fn game_bios_main(zip: &Path, def: &crate::types::CartDef) -> Option<Vec<u8>> {
    if def.controller != crate::types::Controller::Trackball {
        return None;
    }
    let mut src = Source::open(zip).ok()?;
    src.read("236-bios.sp1")
        .ok()
        .filter(|d| d.len() == BIOS_SIZE)
}

fn build_bios(mut src: Source, raw_main: &[u8]) -> Result<Bios, BiosError> {
    // Main BIOS with WordSwap (ROM_LOAD16_WORD_SWAP_BIOS)
    if raw_main.len() != BIOS_SIZE {
        return Err(BiosError::UnsupportedSize(raw_main.len() as u64));
    }
    let main = word_swap(raw_main);

    // Load auxiliary ROMs (no swap)
    let lo = src
        .read("000-lo.lo")
        .map_err(|e| BiosError::MissingFile(format!("000-lo.lo: {:?}", e)))?;
    if lo.len() != BIOS_SIZE {
        return Err(BiosError::UnsupportedSize(lo.len() as u64));
    }

    let sfix = src
        .read("sfix.sfix")
        .map_err(|e| BiosError::MissingFile(format!("sfix.sfix: {:?}", e)))?;
    if sfix.len() != BIOS_SIZE {
        return Err(BiosError::UnsupportedSize(sfix.len() as u64));
    }

    let sm1 = src
        .read("sm1.sm1")
        .map_err(|e| BiosError::MissingFile(format!("sm1.sm1: {:?}", e)))?;
    if sm1.len() != BIOS_SIZE {
        return Err(BiosError::UnsupportedSize(sm1.len() as u64));
    }

    Ok(Bios {
        main,
        lo,
        sfix,
        sm1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_bios_wordswap() {
        // Main BIOS file with [0x12, 0x34] at start -> after WordSwap: [0x34, 0x12]
        let dir = tempdir().unwrap();

        // Create main BIOS file (0x20000 bytes)
        let mut main_data = vec![0u8; BIOS_SIZE];
        main_data[0] = 0x12;
        main_data[1] = 0x34;
        fs::write(dir.path().join("sp-s2.sp1"), &main_data).unwrap();

        // Create auxiliary ROMs (0x20000 bytes each)
        let aux_data = vec![0u8; BIOS_SIZE];
        fs::write(dir.path().join("000-lo.lo"), &aux_data).unwrap();
        fs::write(dir.path().join("sfix.sfix"), &aux_data).unwrap();
        fs::write(dir.path().join("sm1.sm1"), &aux_data).unwrap();

        let bios = load_bios(dir.path(), "sp-s2.sp1").unwrap();
        assert_eq!(bios.main[0], 0x34);
        assert_eq!(bios.main[1], 0x12);
    }

    #[test]
    fn test_bios_missing_file() {
        let dir = tempdir().unwrap();

        // Only create main BIOS, missing aux files
        let main_data = vec![0u8; BIOS_SIZE];
        fs::write(dir.path().join("sp-s2.sp1"), &main_data).unwrap();

        let result = load_bios(dir.path(), "sp-s2.sp1");
        match result {
            Err(BiosError::MissingFile(name)) => assert!(name.contains("000-lo.lo")),
            other => panic!("Expected MissingFile error, got {:?}", other),
        }
    }

    #[test]
    fn game_bios_is_read_from_the_game_zip() {
        use std::io::Write;
        let dir = tempdir().unwrap();
        let zip_path = dir.path().join("irrmaze.zip");
        let mut zw = zip::ZipWriter::new(fs::File::create(&zip_path).unwrap());
        zw.start_file("236-bios.sp1", zip::write::SimpleFileOptions::default())
            .unwrap();
        zw.write_all(&vec![0x5Au8; BIOS_SIZE]).unwrap();
        zw.finish().unwrap();
        let irr = crate::romset::find_def("irrmaze").unwrap();
        let data = game_bios_main(&zip_path, irr).expect("BIOS du zip");
        assert_eq!(data.len(), BIOS_SIZE);
        // autre contrôleur : jamais de BIOS spécial
        let kof = crate::romset::find_def("kof94").unwrap();
        assert!(game_bios_main(&zip_path, kof).is_none());
        // zip sans le fichier : repli sur le BIOS configuré
        let other = dir.path().join("other.zip");
        let mut zw = zip::ZipWriter::new(fs::File::create(&other).unwrap());
        zw.start_file("x.bin", zip::write::SimpleFileOptions::default())
            .unwrap();
        zw.write_all(b"x").unwrap();
        zw.finish().unwrap();
        assert!(game_bios_main(&other, irr).is_none());
    }

    #[test]
    fn test_bios_wrong_size() {
        let dir = tempdir().unwrap();

        // Create main BIOS with wrong size (512KB instead of 128KB)
        let main_data = vec![0u8; 0x80000];
        fs::write(dir.path().join("sp-45.sp1"), &main_data).unwrap();

        // Create aux files with correct size
        let aux_data = vec![0u8; BIOS_SIZE];
        fs::write(dir.path().join("000-lo.lo"), &aux_data).unwrap();
        fs::write(dir.path().join("sfix.sfix"), &aux_data).unwrap();
        fs::write(dir.path().join("sm1.sm1"), &aux_data).unwrap();

        let result = load_bios(dir.path(), "sp-45.sp1");
        match result {
            Err(BiosError::UnsupportedSize(size)) => assert_eq!(size, 0x80000),
            other => panic!("Expected UnsupportedSize error, got {:?}", other),
        }
    }
}
