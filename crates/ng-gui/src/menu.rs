//! Barre de menus : Fichier, Émulation, Système, Vidéo, Débogage.

use eframe::egui;
use ng_cart::defs::ALL_CARTS;
use ng_cart::{Config, Model};

use crate::bios_menu::{bios_label, BiosEntry};

use crate::debugger::Tab;
pub use crate::prefs::ScaleMode as Scale;
use crate::prefs::{Aspect, DisplayPrefs};

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
    SetBios(String),
    SetModel(Model),
    Controls,
    Fullscreen(bool),
    Aspect(Aspect),
    Smooth(bool),
    Step,
    DebugTab(Tab),
    StatusBar(bool),
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

/// Ce que la barre de menus doit refléter.
pub struct MenuState<'a> {
    pub cfg: &'a Config,
    pub paused: bool,
    pub display: &'a DisplayPrefs,
    /// Échelle effective, ex. « x3 (960x672) ».
    pub scale_info: &'a str,
    pub bios: &'a [BiosEntry],
    pub debug_open: bool,
    pub debug_tab: Tab,
    pub status_bar: bool,
}

/// Dessine la barre de menus et renvoie l'action choisie.
pub fn show(ui: &mut egui::Ui, st: &MenuState) -> Option<MenuAction> {
    let mut action = None;
    egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button("Fichier", |ui| {
            ui.menu_button("Ouvrir", |ui| {
                for (name, ok) in available_games(st.cfg) {
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
            if ui.button("Capture d'écran (F9)").clicked() {
                action = Some(MenuAction::Screenshot);
                ui.close();
            }
            ui.separator();
            if ui.button("Quitter").clicked() {
                action = Some(MenuAction::Quit);
                ui.close();
            }
        });
        ui.menu_button("Émulation", |ui| {
            let mut paused = st.paused;
            if ui.checkbox(&mut paused, "Pause (F5)").changed() {
                action = Some(MenuAction::Pause(paused));
                ui.close();
            }
            if ui.button("Reset").clicked() {
                action = Some(MenuAction::Reset);
                ui.close();
            }
            ui.separator();
            if ui.button("Contrôles… (F1)").clicked() {
                action = Some(MenuAction::Controls);
                ui.close();
            }
        });
        ui.menu_button("Système", |ui| {
            ui.menu_button(format!("BIOS : {}", bios_label(&st.cfg.bios)), |ui| {
                for b in st.bios {
                    let text = format!("{}  ({})", b.label, b.file);
                    let r = ui
                        .add_enabled(
                            b.present,
                            egui::RadioButton::new(st.cfg.bios == b.file, text),
                        )
                        .on_disabled_hover_text(format!(
                            "fichier introuvable dans {}",
                            st.cfg.bios_dir.display()
                        ));
                    if r.clicked() && st.cfg.bios != b.file {
                        action = Some(MenuAction::SetBios(b.file.to_string()));
                        ui.close();
                    }
                }
            });
            ui.menu_button("Modèle", |ui| {
                for (label, m) in [("MVS (arcade)", Model::Mvs), ("AES (console)", Model::Aes)] {
                    if ui.radio(st.cfg.model == m, label).clicked() && st.cfg.model != m {
                        action = Some(MenuAction::SetModel(m));
                        ui.close();
                    }
                }
            });
        });
        ui.menu_button("Vidéo", |ui| {
            let d = st.display;
            ui.menu_button("Échelle", |ui| {
                if ui
                    .radio(d.scale == Scale::Auto, "Auto (plus grand entier)")
                    .clicked()
                {
                    action = Some(MenuAction::Scale(Scale::Auto));
                    ui.close();
                }
                for n in 1..=8 {
                    let s = Scale::Fixed(n);
                    if ui.radio(d.scale == s, format!("x{n}")).clicked() {
                        action = Some(MenuAction::Scale(s));
                        ui.close();
                    }
                }
            });
            let mut fs = d.fullscreen;
            if ui.checkbox(&mut fs, "Plein écran (F11)").changed() {
                action = Some(MenuAction::Fullscreen(fs));
                ui.close();
            }
            ui.menu_button("Ratio", |ui| {
                for (label, a) in [
                    ("Pixels carrés (pixel perfect)", Aspect::Square),
                    ("4:3 (non pixel perfect)", Aspect::Tv4x3),
                ] {
                    if ui.radio(d.aspect == a, label).clicked() {
                        action = Some(MenuAction::Aspect(a));
                        ui.close();
                    }
                }
            });
            let mut smooth = d.smooth;
            if ui.checkbox(&mut smooth, "Lissage").changed() {
                action = Some(MenuAction::Smooth(smooth));
                ui.close();
            }
            ui.separator();
            ui.weak(st.scale_info);
        });
        ui.menu_button("Débogage", |ui| {
            let mut open = st.debug_open;
            if ui.checkbox(&mut open, "Débogueur (F12)").changed() {
                action = Some(MenuAction::Debugger);
                ui.close();
            }
            ui.menu_button("Onglet", |ui| {
                for t in Tab::ALL {
                    if ui
                        .radio(st.debug_open && st.debug_tab == t, t.label())
                        .clicked()
                    {
                        action = Some(MenuAction::DebugTab(t));
                        ui.close();
                    }
                }
            });
            ui.separator();
            let mut paused = st.paused;
            if ui.checkbox(&mut paused, "Pause (F5)").changed() {
                action = Some(MenuAction::Pause(paused));
                ui.close();
            }
            if ui.button("Avancer d'une trame (F6)").clicked() {
                action = Some(MenuAction::Step);
                ui.close();
            }
            ui.separator();
            let mut bar = st.status_bar;
            if ui.checkbox(&mut bar, "Barre d'état").changed() {
                action = Some(MenuAction::StatusBar(bar));
                ui.close();
            }
        });
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
