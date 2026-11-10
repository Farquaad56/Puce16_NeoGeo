//! Barre de menus : Fichier, Émulation, Vidéo, Capture.

use eframe::egui;
use ng_cart::defs::ALL_CARTS;
use ng_cart::Config;

/// Mode d'échelle de l'image.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scale {
    Auto,
    Fixed(u32),
}

/// Action demandée par l'utilisateur via le menu.
#[derive(Debug, PartialEq)]
pub enum MenuAction {
    Load(String),
    Quit,
    Pause(bool),
    Reset,
    Scale(Scale),
    Screenshot,
    Debugger,
}

/// Jeux connus et présence de leur zip dans `roms_dir`, dans l'ordre de `ALL_CARTS`.
pub fn available_games(cfg: &Config) -> Vec<(&'static str, bool)> {
    ALL_CARTS
        .iter()
        .map(|d| {
            (
                d.name,
                cfg.roms_dir.join(format!("{}.zip", d.name)).exists(),
            )
        })
        .collect()
}

/// Nom du fichier de capture.
pub fn shot_name(game: &str, frame: u64) -> String {
    format!("{game}_{frame:06}.ppm")
}

/// Dessine la barre de menus et renvoie l'action choisie.
pub fn show(
    ui: &mut egui::Ui,
    cfg: &Config,
    paused: &mut bool,
    scale: Scale,
) -> Option<MenuAction> {
    let mut action = None;
    egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button("Fichier", |ui| {
            ui.menu_button("Ouvrir", |ui| {
                for (name, ok) in available_games(cfg) {
                    let title = ALL_CARTS
                        .iter()
                        .find(|d| d.name == name)
                        .map_or(name, |d| d.title);
                    if ui.add_enabled(ok, egui::Button::new(title)).clicked() {
                        action = Some(MenuAction::Load(name.to_string()));
                        ui.close();
                    }
                }
            });
            if ui.button("Quitter").clicked() {
                action = Some(MenuAction::Quit);
                ui.close();
            }
        });
        ui.menu_button("Émulation", |ui| {
            if ui.checkbox(paused, "Pause").changed() {
                action = Some(MenuAction::Pause(*paused));
            }
            if ui.button("Reset").clicked() {
                action = Some(MenuAction::Reset);
                ui.close();
            }
        });
        ui.menu_button("Vidéo", |ui| {
            for (label, s) in [
                ("Auto", Scale::Auto),
                ("1x", Scale::Fixed(1)),
                ("2x", Scale::Fixed(2)),
                ("3x", Scale::Fixed(3)),
                ("4x", Scale::Fixed(4)),
            ] {
                if ui.radio(scale == s, label).clicked() {
                    action = Some(MenuAction::Scale(s));
                    ui.close();
                }
            }
        });
        if ui.button("Capture").clicked() {
            action = Some(MenuAction::Screenshot);
        }
        if ui.button("Débogueur").clicked() {
            action = Some(MenuAction::Debugger);
        }
    });
    action
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn games_list_flags_present_zip() {
        let dir = std::env::temp_dir().join(format!("ng_gui_games_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("mslug.zip"), b"").unwrap();
        let cfg = Config {
            bios_dir: dir.clone(),
            roms_dir: dir.clone(),
            saves_dir: PathBuf::from("saves"),
            model: ng_cart::Model::Mvs,
            bios: "sp-s2.sp1".into(),
        };
        let games = available_games(&cfg);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(games[0], ("mslug", true));
        assert_eq!(games[1], ("mslug2", false));
        assert_eq!(games.len(), 6);
    }

    #[test]
    fn shot_name_is_zero_padded() {
        assert_eq!(shot_name("mslug", 120), "mslug_000120.ppm");
    }
}
