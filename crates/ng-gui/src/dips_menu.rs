//! Fenêtre « DIP switches MVS » : réglages matériels lus par le BIOS à `$300001`.

use eframe::egui;

/// Un réglage : masque, titre, libellé quand le bit vaut 1 puis quand il vaut 0.
const SWITCHES: [(u8, &str, &str, &str); 6] = [
    (0x01, "Mode test (Settings)", "Non", "Oui"),
    (0x02, "Monnayeurs", "2", "1"),
    (0x04, "Contrôleur", "Manette", "Panneau mahjong"),
    (0x20, "Liaison (COMM)", "Désactivée", "Activée"),
    (0x40, "Free Play", "Non", "Oui"),
    (0x80, "Freeze (gel de l'écran)", "Non", "Oui"),
];

/// Valeur par défaut : tous les interrupteurs « off ».
pub const DEFAULT: u8 = 0xFF;

#[derive(Default)]
pub struct DipsWindow {
    pub open: bool,
}

/// Inverse l'interrupteur `mask` de `v`.
pub fn set_bit(v: u8, mask: u8, on: bool) -> u8 {
    if on {
        v | mask
    } else {
        v & !mask
    }
}

impl DipsWindow {
    /// Dessine la fenêtre ; renvoie `true` si `dipsw` a changé.
    pub fn show(&mut self, ctx: &egui::Context, dipsw: &mut u8) -> bool {
        if !self.open {
            return false;
        }
        let mut changed = false;
        let mut open = self.open;
        egui::Window::new("DIP switches MVS")
            .open(&mut open)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Pris en compte tout de suite ; le « mode test » s'ouvre après un Reset.");
                ui.separator();
                egui::Grid::new("dips").num_columns(2).show(ui, |ui| {
                    for (mask, title, off, on) in SWITCHES {
                        ui.label(title);
                        let mut high = *dipsw & mask != 0;
                        ui.horizontal(|ui| {
                            if ui.radio_value(&mut high, true, off).changed()
                                | ui.radio_value(&mut high, false, on).changed()
                            {
                                *dipsw = set_bit(*dipsw, mask, high);
                                changed = true;
                            }
                        });
                        ui.end_row();
                    }
                });
                ui.separator();
                ui.label(format!("Valeur : 0x{:02X}", *dipsw));
                if ui.button("Valeurs par défaut").clicked() {
                    *dipsw = DEFAULT;
                    changed = true;
                }
            });
        self.open = open;
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bits() {
        assert_eq!(set_bit(0xFF, 0x40, false), 0xBF);
        assert_eq!(set_bit(0xBF, 0x40, true), 0xFF);
        assert_eq!(set_bit(0xFF, 0x01, false), 0xFE);
    }
}
