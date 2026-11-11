//! Fenêtre « Filtres vidéo » : préréglages d'écran, interpolation, scanlines, gamma, masque.

use eframe::egui;

use crate::postfx::{Fx, Mask, Preset, ScanMode, MEDIUM, SHARP, SOFT};

#[derive(Default)]
pub struct FxWindow {
    pub open: bool,
}

fn pct(ui: &mut egui::Ui, v: &mut u8, range: std::ops::RangeInclusive<u8>, step: f64) -> bool {
    ui.add(egui::Slider::new(v, range).suffix(" %").step_by(step))
        .changed()
}

impl FxWindow {
    /// Dessine la fenêtre ; renvoie `true` si un réglage a changé.
    pub fn show(&mut self, ctx: &egui::Context, fx: &mut Fx) -> bool {
        if !self.open {
            return false;
        }
        let mut changed = false;
        let mut open = self.open;
        egui::Window::new("Filtres vidéo")
            .open(&mut open)
            .resizable(false)
            .show(ctx, |ui| {
                let current = fx.detect();
                egui::ComboBox::from_label("Préréglage d'écran")
                    .selected_text(current.map_or("Personnalisé", Preset::label))
                    .show_ui(ui, |ui| {
                        for p in Preset::ALL {
                            if ui.selectable_label(current == Some(p), p.label()).clicked() {
                                *fx = Fx::preset(p);
                                changed = true;
                            }
                        }
                    });
                ui.separator();

                ui.strong("Interpolation");
                ui.horizontal(|ui| {
                    for (label, v) in [
                        ("Aucune", 0),
                        ("Douce", SOFT),
                        ("Moyenne", MEDIUM),
                        ("Nette", SHARP),
                    ] {
                        if ui.selectable_label(fx.interp == v, label).clicked() {
                            fx.interp = v;
                            changed = true;
                        }
                    }
                });
                if fx.interp > 0 {
                    ui.horizontal(|ui| {
                        ui.label("Netteté");
                        changed |= pct(ui, &mut fx.interp, 1..=100, 1.0);
                    });
                }
                ui.separator();

                ui.strong("Scanlines");
                ui.horizontal(|ui| {
                    for (label, m) in [
                        ("Désactivées", ScanMode::Off),
                        ("Standard", ScanMode::Standard),
                        ("Adaptatives", ScanMode::Adaptive),
                    ] {
                        if ui.radio(fx.scan_mode == m, label).clicked() {
                            fx.scan_mode = m;
                            changed = true;
                        }
                    }
                });
                match fx.scan_mode {
                    ScanMode::Off => {}
                    ScanMode::Standard => {
                        ui.horizontal(|ui| {
                            ui.label("Profondeur");
                            changed |= pct(ui, &mut fx.scan, 0..=95, 5.0);
                        });
                    }
                    ScanMode::Adaptive => {
                        ui.horizontal(|ui| {
                            ui.label("Pixels sombres");
                            changed |= pct(ui, &mut fx.scan_dk, 0..=95, 5.0);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Pixels clairs");
                            changed |= pct(ui, &mut fx.scan_br, 0..=95, 5.0);
                        });
                    }
                }
                if fx.scan_mode != ScanMode::Off {
                    ui.weak("Visibles à partir d'une échelle x2.");
                }
                ui.separator();

                ui.strong("Gamma");
                ui.horizontal(|ui| {
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut fx.gamma, 100..=200)
                                .custom_formatter(|v, _| {
                                    if v <= 100.0 {
                                        "désactivé".to_string()
                                    } else {
                                        format!("{:.2}", v / 100.0)
                                    }
                                })
                                .custom_parser(|s| {
                                    s.replace(',', ".").parse::<f64>().ok().map(|v| v * 100.0)
                                }),
                        )
                        .changed();
                });
                ui.separator();

                ui.strong("Masque");
                ui.horizontal(|ui| {
                    for (label, m) in [
                        ("Aucun", Mask::None),
                        ("Sony Trinitron", Mask::Trinitron),
                        ("Tube fané (jaune)", Mask::FadedYellow),
                    ] {
                        if ui.selectable_label(fx.mask == m, label).clicked() {
                            fx.mask = m;
                            changed = true;
                        }
                    }
                });
                if fx.mask != Mask::None {
                    ui.horizontal(|ui| {
                        ui.label("Force");
                        changed |= pct(ui, &mut fx.mask_pct, 0..=100, 5.0);
                    });
                }
                ui.separator();
                if ui.button("Tout désactiver").clicked() {
                    *fx = Fx::default();
                    changed = true;
                }
            });
        self.open = open;
        changed
    }
}
