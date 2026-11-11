//! Fenêtre « Contrôles » : assignation des touches et des boutons de manette.

use eframe::egui::{self, Key};

use crate::input::{button_name, Action, Bindings, InputState};

/// Ce que l'on attend de l'utilisateur.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Capture {
    Key(Action),
    Button(Action),
}

#[derive(Default)]
pub struct ControlsWindow {
    pub open: bool,
    capture: Option<Capture>,
}

const WAIT_KEY: &str = "Appuyez sur une touche…";
const WAIT_BUTTON: &str = "Appuyez sur un bouton…";

/// Nom affiché d'une touche (les flèches n'ont pas de glyphe dans la police par défaut).
pub fn key_label(k: Key) -> &'static str {
    match k {
        Key::ArrowUp => "Flèche haut",
        Key::ArrowDown => "Flèche bas",
        Key::ArrowLeft => "Flèche gauche",
        Key::ArrowRight => "Flèche droite",
        Key::Enter => "Entrée",
        Key::Space => "Espace",
        other => other.name(),
    }
}

/// Texte de la case « clavier » d'une action.
pub fn key_cell(b: &Bindings, a: Action, capturing: bool) -> String {
    if capturing {
        return WAIT_KEY.into();
    }
    match b.key(a) {
        Some(k) => key_label(k).to_string(),
        None if a == Action::Select1 => "Maj".into(),
        None => "—".into(),
    }
}

/// Texte de la case « manette » d'une action.
pub fn button_cell(b: &Bindings, a: Action, capturing: bool) -> String {
    if capturing {
        return WAIT_BUTTON.into();
    }
    b.button(a)
        .map_or("—".into(), |x| button_name(x).to_string())
}

impl ControlsWindow {
    /// Une capture est en cours : les touches ne doivent pas aller au jeu.
    pub fn capturing(&self) -> bool {
        self.open && self.capture.is_some()
    }

    /// Affiche la fenêtre ; renvoie vrai si les assignations ont changé.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        input: &mut InputState,
        mahjong: bool,
        pointer: bool,
        sens: &mut u32,
    ) -> bool {
        if !self.open {
            self.capture = None;
            return false;
        }
        let mut changed = self.poll_capture(ctx, input);
        let mut open = self.open;
        egui::Window::new("Contrôles")
            .open(&mut open)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(
                    "Cliquez sur une case puis appuyez sur la touche ou le bouton. Échap annule.",
                );
                ui.label("Une touche déjà utilisée est échangée avec l'autre action.");
                ui.separator();
                ui.columns(3, |cols| {
                    for (g, title) in ["Joueur 1", "Joueur 2", "Système"].into_iter().enumerate() {
                        let ui = &mut cols[g];
                        ui.strong(title);
                        egui::Grid::new(("controls", g))
                            .num_columns(3)
                            .striped(true)
                            .show(ui, |ui| {
                                ui.weak("Action");
                                ui.weak("Clavier");
                                ui.weak("Manette");
                                ui.end_row();
                                for a in Action::ALL.into_iter().filter(|a| a.group() == g) {
                                    ui.label(a.label());
                                    let cap = self.capture == Some(Capture::Key(a));
                                    let text = key_cell(&input.bindings, a, cap);
                                    if ui.selectable_label(cap, text).clicked() {
                                        self.capture = Some(Capture::Key(a));
                                    }
                                    let cap = self.capture == Some(Capture::Button(a));
                                    let text = button_cell(&input.bindings, a, cap);
                                    if ui.selectable_label(cap, text).clicked() {
                                        input.take_pressed_button();
                                        self.capture = Some(Capture::Button(a));
                                    }
                                    ui.end_row();
                                }
                            });
                        if g < 2 && ui.button(format!("Défaut {title}")).clicked() {
                            input.bindings.reset(|a| a.group() == g);
                            changed = true;
                        }
                    }
                });
                if pointer {
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("Sensibilité souris (trackball / molette)");
                        if ui
                            .add(egui::Slider::new(sens, 10..=500).suffix(" %"))
                            .changed()
                        {
                            changed = true;
                        }
                    });
                    ui.weak("Sans souris, les flèches de J1 simulent un mouvement constant.");
                }
                if mahjong {
                    ui.separator();
                    ui.strong("Panneau mahjong (jeu courant)");
                    let acts: Vec<Action> =
                        Action::ALL.into_iter().filter(|a| a.group() == 3).collect();
                    ui.columns(2, |cols| {
                        for (c, chunk) in acts.chunks(acts.len().div_ceil(2)).enumerate() {
                            egui::Grid::new(("controls_mj", c))
                                .num_columns(3)
                                .striped(true)
                                .show(&mut cols[c], |ui| {
                                    for &a in chunk {
                                        ui.label(a.label());
                                        let cap = self.capture == Some(Capture::Key(a));
                                        let text = key_cell(&input.bindings, a, cap);
                                        if ui.selectable_label(cap, text).clicked() {
                                            self.capture = Some(Capture::Key(a));
                                        }
                                        let cap = self.capture == Some(Capture::Button(a));
                                        let text = button_cell(&input.bindings, a, cap);
                                        if ui.selectable_label(cap, text).clicked() {
                                            input.take_pressed_button();
                                            self.capture = Some(Capture::Button(a));
                                        }
                                        ui.end_row();
                                    }
                                });
                        }
                    });
                    if ui.button("Défaut panneau mahjong").clicked() {
                        input.bindings.reset(|a| a.group() == 3);
                        changed = true;
                    }
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Tout par défaut").clicked() {
                        input.bindings = Bindings::default();
                        self.capture = None;
                        changed = true;
                    }
                    ui.weak(
                        "Le stick gauche donne toujours les directions. Sauvegarde automatique.",
                    );
                });
            });
        self.open = open;
        changed
    }

    /// Termine la capture en cours si une touche ou un bouton vient d'être pressé.
    fn poll_capture(&mut self, ctx: &egui::Context, input: &mut InputState) -> bool {
        let Some(cap) = self.capture else {
            return false;
        };
        let key = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    ..
                } => Some(*key),
                _ => None,
            })
        });
        if key == Some(Key::Escape) {
            self.capture = None;
            return false;
        }
        match cap {
            Capture::Key(a) => {
                let Some(k) = key else { return false };
                input.bindings.set_key(a, k);
            }
            Capture::Button(a) => {
                let Some((pad, b)) = input.take_pressed_button() else {
                    return false;
                };
                if pad != a.gamepad() {
                    return false;
                }
                input.bindings.set_button(a, b);
            }
        }
        self.capture = None;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_texts() {
        let mut b = Bindings::default();
        assert_eq!(key_cell(&b, Action::P1A, false), "Z");
        assert_eq!(key_cell(&b, Action::P1A, true), WAIT_KEY);
        assert_eq!(key_cell(&b, Action::P1Up, false), "Flèche haut");
        assert_eq!(key_cell(&b, Action::Coin1, false), "5");
        assert_eq!(key_cell(&b, Action::Select1, false), "Maj");
        assert_eq!(button_cell(&b, Action::P1A, false), "South");
        assert_eq!(button_cell(&b, Action::Coin1, false), "—");
        assert_eq!(button_cell(&b, Action::Coin1, true), WAIT_BUTTON);
        b.set_key(Action::Select1, Key::Enter);
        assert_eq!(key_cell(&b, Action::Start1, false), "—");
    }

    #[test]
    fn closed_window_never_captures() {
        let mut w = ControlsWindow {
            open: false,
            capture: Some(Capture::Key(Action::P1A)),
        };
        assert!(!w.capturing());
        w.open = true;
        assert!(w.capturing());
    }
}
