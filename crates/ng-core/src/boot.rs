//! T600c : démarrage de la machine et API publique (GUI, tests).
//!
//! `load_machine` assemble BIOS + cartouche ; `set_inputs`, `run_frame`,
//! `framebuffer`, `take_audio`, `frame_count`, `save_backup`/`load_backup`
//! sont les méthodes de [`Machine`]. `save_machine` écrit la backup RAM.

use std::path::{Path, PathBuf};

use ng_cart::{find_def, load_bios, load_cart, Config};

use crate::machine::Machine;

/// Fichier de sauvegarde de la backup RAM d'un jeu.
fn backup_path(cfg: &Config, game: &str) -> PathBuf {
    cfg.saves_dir.join(format!("{game}.sram"))
}

/// Construit une machine pour `game` (zip dans `cfg.roms_dir`).
///
/// Le Z80 démarre au reset ; la backup RAM est rechargée si elle existe.
pub fn load_machine(cfg: &Config, game: &str) -> Result<Machine, String> {
    let def = find_def(game).ok_or_else(|| format!("jeu inconnu : {game}"))?;
    let zip = cfg.roms_dir.join(format!("{game}.zip"));
    if !zip.exists() {
        return Err(format!(
            "archive introuvable : {game}.zip ({})",
            zip.display()
        ));
    }
    let cart = load_cart(&zip, def, true).map_err(|e| format!("{game}.zip : {e:?}"))?;
    let bios = load_bios(&cfg.bios_dir, &cfg.bios).map_err(|e| format!("BIOS : {e:?}"))?;
    // Protection de cartouche : rien à installer pour l'instant.
    let mut machine = Machine::new(cart, bios, cfg.model.clone());
    let sram = backup_path(cfg, game);
    if sram.exists() {
        machine.load_backup(&sram);
    }
    Ok(machine)
}

/// Sauvegarde la backup RAM de `game` dans `cfg.saves_dir`.
pub fn save_machine(machine: &Machine, cfg: &Config, game: &str) -> Result<(), String> {
    std::fs::create_dir_all(&cfg.saves_dir).map_err(|e| e.to_string())?;
    let path: &Path = &backup_path(cfg, game);
    machine.save_backup(path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::tests_support::{machine_with, program_bios};
    use ng_cart::Model;
    use ng_common::{PadState, SystemButtons};

    fn cfg(dir: &Path) -> Config {
        Config {
            bios_dir: dir.to_path_buf(),
            roms_dir: dir.to_path_buf(),
            saves_dir: dir.to_path_buf(),
            model: Model::Mvs,
            bios: "sp-s2.sp1".to_string(),
        }
    }

    fn running() -> Machine {
        // BRA.s * : boucle infinie.
        machine_with(program_bios(&[0x60, 0xFE]), 0x00)
    }

    #[test]
    fn missing_game_reports_zip_name() {
        let dir = std::env::temp_dir();
        match load_machine(&cfg(&dir), "mslug") {
            Err(e) => assert!(e.contains("mslug.zip"), "{e}"),
            Ok(_) => panic!("erreur attendue"),
        }
    }

    #[test]
    fn set_inputs_visible_on_bus() {
        let mut m = running();
        m.set_inputs(
            PadState { bits: 0xEF },
            PadState::default(),
            SystemButtons::default(),
        );
        assert_eq!(m.bus.read16(0x300000) >> 8, 0xEF);
    }

    #[test]
    fn one_frame_yields_about_one_frame_of_samples() {
        let mut m = running();
        m.run_frame();
        // 55555,56 Hz sur une trame de MCLK_PER_FRAME (24 MHz) : ~938 (925 serait à 60 Hz).
        let expected = (ng_common::clock::MCLK_PER_FRAME as f64 * 55555.56 / 24_000_000.0) as i64;
        let n = m.take_audio().len() as i64;
        assert!((n - expected).abs() <= 2, "échantillons : {n}");
        assert!(m.take_audio().is_empty());
        assert_eq!(m.frame_count(), 1);
    }

    #[test]
    fn backup_roundtrip() {
        let dir = std::env::temp_dir().join(format!("ng_core_boot_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.sram");
        let mut m = running();
        m.backup.borrow_mut().data[0x1234] = 0x5A;
        m.save_backup(&path);
        assert_eq!(std::fs::metadata(&path).unwrap().len(), 65536);
        m.backup.borrow_mut().data[0x1234] = 0;
        m.load_backup(&path);
        assert_eq!(m.backup.borrow().data[0x1234], 0x5A);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
