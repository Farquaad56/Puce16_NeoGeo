//! T600c : démarrage de la machine et API publique (GUI, tests).
//!
//! `load_machine` assemble BIOS + cartouche ; `set_inputs`, `run_frame`,
//! `framebuffer`, `take_audio`, `frame_count`, `save_backup`/`load_backup`
//! sont les méthodes de [`Machine`]. `save_machine` écrit la backup RAM.

use std::path::{Path, PathBuf};

use ng_bus::prot_cartram::CartRam;
use ng_bus::prot_fatfury2::FatFury2;
use ng_bus::prot_kof98::Kof98;
use ng_bus::prot_mslugx::MslugX;
use ng_bus::prot_pvc::Pvc;
use ng_bus::prot_sbp::Sbp;
use ng_bus::prot_sma::{Sma3, SmaChip, SmaKind};
use ng_bus::CartPort;
use ng_cart::types::RuntimeProt;
use ng_cart::{find_def, game_bios_main, load_bios, load_bios_with_main, load_cart, Cart, Config};

use crate::machine::Machine;

/// Fichier de sauvegarde de la backup RAM d'un jeu.
fn backup_path(cfg: &Config, game: &str) -> PathBuf {
    cfg.saves_dir.join(format!("{game}.sram"))
}

/// Fichier de la RAM de cartouche (8192 octets) de `game` (V-Liner, Jockey GP).
pub fn cartram_path(cfg: &Config, game: &str) -> PathBuf {
    cfg.saves_dir.join(format!("{game}.cartram"))
}

/// Fichier de la carte mémoire du profil (nom du BIOS choisi), partagée par tous les jeux.
pub fn memcard_path(cfg: &Config) -> PathBuf {
    let profile: String = cfg
        .bios
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    cfg.saves_dir.join(format!("{profile}.memcard"))
}

/// Crée une carte mémoire vierge (2048 octets à 0xFF) si elle n'existe pas ; renvoie son chemin.
/// La carte est insérée au prochain `load_machine` (présente = le fichier existe).
pub fn insert_memcard(cfg: &Config) -> Result<PathBuf, String> {
    std::fs::create_dir_all(&cfg.saves_dir).map_err(|e| e.to_string())?;
    let path = memcard_path(cfg);
    if !path.exists() {
        std::fs::write(&path, vec![0xFFu8; ng_bus::memcard::MEMCARD_SIZE])
            .map_err(|e| e.to_string())?;
    }
    Ok(path)
}

/// Port de protection runtime de la cartouche (`$200000-$2FFFFF`), s'il y en a un.
pub fn make_port(cart: &Cart) -> Option<Box<dyn CartPort>> {
    match cart.def.prot.runtime {
        RuntimeProt::None => None,
        RuntimeProt::Msx => Some(Box::new(MslugX::new())),
        RuntimeProt::Sma3 => Some(Box::new(Sma3::new())),
        RuntimeProt::Pvc => Some(Box::new(Pvc::new())),
        RuntimeProt::Kof98 => Some(Box::new(Kof98::new())),
        RuntimeProt::CartRam => Some(Box::new(CartRam::new(false))),
        RuntimeProt::Vliner => Some(Box::new(CartRam::new(true))),
        RuntimeProt::Sbp => Some(Box::new(Sbp::new())),
        RuntimeProt::FatFury2 => Some(Box::new(FatFury2::new())),
        RuntimeProt::Sma99 => Some(Box::new(SmaChip::new(SmaKind::Kof99))),
        RuntimeProt::SmaGarou => Some(Box::new(SmaChip::new(SmaKind::Garou))),
        RuntimeProt::Sma2000 => Some(Box::new(SmaChip::new(SmaKind::Kof2000))),
    }
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
    let mut cart = load_cart(&zip, def, true).map_err(|e| format!("{game}.zip : {e:?}"))?;
    // irrmaze : BIOS spécial à trackball fourni dans le zip du jeu (sinon BIOS configuré).
    let bios = match game_bios_main(&zip, def) {
        Some(main) => load_bios_with_main(&cfg.bios_dir, &main),
        None => load_bios(&cfg.bios_dir, &cfg.bios),
    }
    .map_err(|e| format!("BIOS : {e:?}"))?;
    // Protections de cartouche : déchiffrement (MAME decrypt_all) puis port runtime.
    cart.decrypt_all();
    let port = make_port(&cart);
    let mut machine = Machine::new(cart, bios, cfg.model.clone());
    machine.bus.port = port;
    let sram = backup_path(cfg, game);
    if sram.exists() {
        machine.load_backup(&sram);
    }
    machine.load_memcard(&memcard_path(cfg));
    machine.load_cart_ram(&cartram_path(cfg, game));
    Ok(machine)
}

/// Sauvegarde la backup RAM de `game` dans `cfg.saves_dir`.
pub fn save_machine(machine: &Machine, cfg: &Config, game: &str) -> Result<(), String> {
    std::fs::create_dir_all(&cfg.saves_dir).map_err(|e| e.to_string())?;
    let path: &Path = &backup_path(cfg, game);
    machine.save_backup(path);
    machine
        .save_memcard(&memcard_path(cfg))
        .map_err(|e| e.to_string())?;
    machine
        .save_cart_ram(&cartram_path(cfg, game))
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::tests_support::{machine_with, program_bios};
    use ng_bus::Device;
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
    fn make_port_follows_the_protection_table() {
        use crate::machine::tests_support::cart_named;
        assert!(make_port(&cart_named("mslugx")).is_some());
        assert!(make_port(&cart_named("mslug3")).is_some());
        assert!(make_port(&cart_named("mslug5")).is_some());
        assert!(make_port(&cart_named("mslug")).is_none());
        assert!(make_port(&cart_named("mslug2")).is_none());
        assert!(make_port(&cart_named("mslug4")).is_none());
        for g in ["kof99", "garou", "kof2000"] {
            assert!(make_port(&cart_named(g)).is_some(), "{g}");
        }
        for g in [
            "kof2003", "svc", "kof98", "fatfury2", "ssideki", "sbp", "vliner", "jockeygp",
        ] {
            assert!(make_port(&cart_named(g)).is_some(), "{g}");
        }
        assert!(make_port(&cart_named("kof94")).is_none());
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
    fn z80_reply_and_rtc_pins_reach_status_a() {
        use puce_m68k::bus::Bus;
        let mut m = running();
        m.sound.reply = 0xC3;
        let (_, mut host) = m.split();
        assert_eq!(host.read(0x320000, 5, true, true) >> 8, 0xC3);
        // REG_RTCCTRL : commande série 0x4 (TP 64 Hz) puis STB ; TP bascule ensuite.
        for bit in [0u16, 0, 1, 0] {
            host.write(0x380050, 5, false, true, bit);
            host.write(0x380050, 5, false, true, bit | 2);
        }
        host.write(0x380050, 5, false, true, 4);
        host.write(0x380050, 5, false, true, 0);
        let tp0 = host.read(0x320000, 5, true, true) & 0x40;
        *host.now += 24_000_000 / 128; // demi-période de 64 Hz
        let tp1 = host.read(0x320000, 5, true, true) & 0x40;
        assert_ne!(tp0, tp1);
    }

    #[test]
    fn memcard_is_inserted_from_its_file_and_saved() {
        let dir = std::env::temp_dir().join(format!("ng_core_card_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let c = cfg(&dir);
        assert!(memcard_path(&c).ends_with("sp_s2_sp1.memcard"));
        let mut m = running();
        assert!(
            !m.load_memcard(&memcard_path(&c)),
            "pas de fichier : pas de carte"
        );
        assert_eq!(m.io.borrow_mut().read(0x380000) & 0x7000, 0x7000);

        let path = insert_memcard(&c).expect("création");
        assert_eq!(std::fs::metadata(&path).unwrap().len(), 2048);
        assert!(m.load_memcard(&path));
        assert_eq!(m.io.borrow_mut().read(0x380000) & 0x7000, 0);
        m.bus.write16(0x800006, 0x00A5, false, true);
        assert_eq!(m.bus.read16(0x800006), 0xFFA5);
        m.save_memcard(&path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap()[3], 0xA5);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cart_ram_is_mapped_saved_and_reloaded() {
        let dir = std::env::temp_dir().join(format!("ng_core_cram_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let c = cfg(&dir);
        let path = cartram_path(&c, "vliner");
        assert!(path.ends_with("vliner.cartram"));
        let mut m = running();
        m.bus.port = Some(Box::new(CartRam::new(true)));
        assert!(!m.load_cart_ram(&path), "pas de fichier");
        m.bus.write16(0x200000, 0xBEEF, true, true);
        m.bus.write16(0x200010, 0x1234, true, true);
        assert_eq!(m.bus.read16(0x200010), 0x1234);
        m.save_cart_ram(&path).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().len(), 8192);

        let mut m2 = running();
        m2.bus.port = Some(Box::new(CartRam::new(true)));
        assert!(m2.load_cart_ram(&path));
        assert_eq!(m2.bus.read16(0x200000), 0xBEEF);
        // Sans RAM de cartouche : rien n'est écrit ni chargé.
        let mut m3 = running();
        assert!(!m3.load_cart_ram(&path));
        let other = dir.join("none.cartram");
        m3.save_cart_ram(&other).unwrap();
        assert!(!other.exists());
        let _ = std::fs::remove_dir_all(&dir);
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
