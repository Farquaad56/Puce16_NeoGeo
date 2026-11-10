//! Configuration : lecture de `neogeo.toml` ou formulaire de première configuration.

use std::path::{Path, PathBuf};

use eframe::egui;
use ng_cart::{Config, Model};

/// Chemin du fichier de configuration : argument 1 ou `./neogeo.toml`.
pub fn config_path() -> PathBuf {
    std::env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from("neogeo.toml"), PathBuf::from)
}

/// Texte TOML d'une configuration (relu par `parse_config`).
pub fn config_to_toml(cfg: &Config) -> String {
    let model = match cfg.model {
        Model::Mvs => "mvs",
        Model::Aes => "aes",
    };
    format!(
        "[paths]\nbios_dir = {:?}\nroms_dir = {:?}\nsaves_dir = {:?}\n\n[system]\nmodel = {:?}\nbios = {:?}\n",
        cfg.bios_dir.to_string_lossy(),
        cfg.roms_dir.to_string_lossy(),
        cfg.saves_dir.to_string_lossy(),
        model,
        cfg.bios
    )
}

/// Formulaire de configuration (trois dossiers).
#[derive(Default)]
pub struct ConfigForm {
    pub bios_dir: String,
    pub roms_dir: String,
    pub saves_dir: String,
    pub message: Option<String>,
}

impl ConfigForm {
    pub fn to_config(&self) -> Config {
        Config {
            bios_dir: PathBuf::from(&self.bios_dir),
            roms_dir: PathBuf::from(&self.roms_dir),
            saves_dir: PathBuf::from(&self.saves_dir),
            model: Model::Mvs,
            bios: "sp-s2.sp1".into(),
        }
    }

    /// Affiche le formulaire ; renvoie la configuration écrite quand l'utilisateur valide.
    pub fn ui(&mut self, ui: &mut egui::Ui, path: &Path) -> Option<Config> {
        ui.heading("Configuration");
        ui.label(format!("Fichier introuvable : {}", path.display()));
        for (label, value) in [
            ("Dossier BIOS (neogeo.zip)", &mut self.bios_dir),
            ("Dossier ROMs", &mut self.roms_dir),
            ("Dossier sauvegardes", &mut self.saves_dir),
        ] {
            ui.horizontal(|ui| {
                ui.label(label);
                ui.text_edit_singleline(value);
                if ui.button("Parcourir…").clicked() {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        *value = dir.to_string_lossy().into_owned();
                    }
                }
            });
        }
        let mut done = None;
        if ui.button("Enregistrer").clicked() {
            let cfg = self.to_config();
            match std::fs::write(path, config_to_toml(&cfg)) {
                Ok(()) => done = Some(cfg),
                Err(e) => self.message = Some(format!("écriture impossible : {e}")),
            }
        }
        if let Some(m) = &self.message {
            ui.colored_label(egui::Color32::RED, m);
        }
        done
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_round_trip() {
        let cfg = Config {
            bios_dir: PathBuf::from("C:/bios dir"),
            roms_dir: PathBuf::from("D:/roms"),
            saves_dir: PathBuf::from("saves"),
            model: Model::Aes,
            bios: "sp-s2.sp1".into(),
        };
        let back = ng_cart::parse_config(&config_to_toml(&cfg)).expect("parse");
        assert_eq!(back.bios_dir, cfg.bios_dir);
        assert_eq!(back.roms_dir, cfg.roms_dir);
        assert_eq!(back.saves_dir, cfg.saves_dir);
        assert_eq!(back.model, cfg.model);
        assert_eq!(back.bios, cfg.bios);
    }
}
