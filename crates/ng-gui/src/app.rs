//! Interface egui : affiche la dernière trame reçue du thread d'émulation.

use eframe::egui::{self, Key};

use std::path::PathBuf;

use ng_cart::Config;

use crate::bios_menu::{self, BiosEntry};
use crate::controls_menu::ControlsWindow;
use crate::debugger::{DebugAction, Debugger, Region, Tab};
use crate::display::{self, DisplayRect};
use crate::emu_thread::{self, Cmd, EmuHandle, FrameMsg};
use crate::input::{Bindings, InputState};
use crate::menu::{self, MenuAction, MenuState};
use crate::prefs::{self, Prefs};
use crate::settings::ConfigForm;
use crate::statusbar::{format_status, StatusInfo};
use ng_core::DebugLevel;

pub const FB_W: usize = display::FB_W as usize;
pub const FB_H: usize = display::FB_H as usize;

/// En plein écran, la barre de menus apparaît quand la souris est à moins de 8 points du bord
/// haut et reste tant qu'elle est sur la barre.
const MENU_REVEAL: f32 = 8.0;
const MENU_KEEP: f32 = 32.0;
/// Délai d'inactivité (s) avant de masquer le curseur en plein écran.
const CURSOR_HIDE_AFTER: f32 = 2.0;

/// Convertit un framebuffer 0x00RRGGBB en image egui.
pub fn to_color_image(pixels: &[u32]) -> egui::ColorImage {
    let rgba: Vec<egui::Color32> = pixels
        .iter()
        .map(|p| egui::Color32::from_rgb((p >> 16) as u8, (p >> 8) as u8, *p as u8))
        .collect();
    egui::ColorImage {
        size: [FB_W, FB_H],
        source_size: egui::vec2(FB_W as f32, FB_H as f32),
        pixels: rgba,
    }
}

/// Message affiché quand le BIOS configuré n'est pas utilisable.
pub fn missing_bios_message(cfg: &Config, bios: &[BiosEntry]) -> Option<String> {
    let known = bios.iter().find(|b| b.file == cfg.bios);
    match known {
        Some(b) if b.present => None,
        // BIOS hors liste : on ne sait pas, le chargement du jeu le dira
        None => None,
        Some(_) => Some(format!(
            "BIOS « {} » introuvable dans {} : choisissez-en un dans Système > BIOS",
            cfg.bios,
            cfg.bios_dir.display()
        )),
    }
}

/// La barre de menus est-elle visible en plein écran ? `y` = ordonnée de la souris.
pub fn menu_revealed(was_shown: bool, y: Option<f32>, popup_open: bool) -> bool {
    popup_open || y.is_some_and(|y| y < if was_shown { MENU_KEEP } else { MENU_REVEAL })
}

pub struct App {
    cfg: Option<Config>,
    cfg_path: PathBuf,
    form: ConfigForm,
    emu: Option<EmuHandle>,
    texture: Option<egui::TextureHandle>,
    last: Option<FrameMsg>,
    error: Option<String>,
    paused: bool,
    prefs: Prefs,
    /// Dernières préférences écrites sur disque (évite d'écrire à chaque trame).
    saved: Prefs,
    prefs_path: PathBuf,
    /// BIOS connus et leur présence (recalculé quand la configuration change).
    bios: Vec<BiosEntry>,
    game: Option<String>,
    input: InputState,
    controls: ControlsWindow,
    debugger: Debugger,
    /// Dernier niveau de débogage envoyé au thread d'émulation.
    debug_level: DebugLevel,
    /// Dernier état plein écran demandé à la fenêtre.
    fullscreen_applied: Option<bool>,
    menu_shown: bool,
    /// Placement de l'image à la dernière trame dessinée.
    shown: Option<DisplayRect>,
}

impl App {
    pub fn new(cfg_path: PathBuf) -> Self {
        let cfg = ng_cart::load_config(&cfg_path).ok();
        let emu = cfg.clone().map(emu_thread::spawn);
        let prefs_path = prefs::prefs_path(&cfg_path);
        let prefs = Prefs::load(&prefs_path);
        let bindings = Bindings::from_prefs(&prefs.input);
        let debugger = Debugger::new(
            prefs.debug.open,
            Tab::from_name(&prefs.debug.tab).unwrap_or_default(),
            Region::from_name(&prefs.debug.region).unwrap_or_default(),
        );
        let bios = cfg
            .as_ref()
            .map(bios_menu::available_bios)
            .unwrap_or_default();
        let error = cfg.as_ref().and_then(|c| missing_bios_message(c, &bios));
        Self {
            cfg,
            cfg_path,
            form: ConfigForm::default(),
            emu,
            texture: None,
            last: None,
            error,
            bios,
            paused: false,
            saved: prefs.clone(),
            prefs,
            prefs_path,
            game: None,
            input: InputState::with_bindings(bindings),
            controls: ControlsWindow::default(),
            debugger,
            debug_level: DebugLevel::Off,
            fullscreen_applied: None,
            menu_shown: false,
            shown: None,
        }
    }

    pub fn send(&self, cmd: Cmd) {
        if let Some(e) = &self.emu {
            let _ = e.tx.send(cmd);
        }
    }

    pub fn load_game(&mut self, name: &str) {
        self.game = Some(name.to_string());
        self.error = None;
        self.send(Cmd::Load {
            game: name.to_string(),
        });
    }

    /// Modifie BIOS ou modèle : écrit `neogeo.toml`, informe le thread d'émulation et relance le
    /// jeu courant avec la nouvelle configuration.
    fn change_system(&mut self, edit: impl FnOnce(&mut Config)) {
        let Some(cfg) = self.cfg.as_mut() else { return };
        edit(cfg);
        let cfg = cfg.clone();
        let old = std::fs::read_to_string(&self.cfg_path).unwrap_or_default();
        let text = bios_menu::updated_toml(&old, &cfg);
        self.error = prefs::write_atomic(&self.cfg_path, &text)
            .err()
            .map(|e| format!("configuration non sauvegardée : {e}"));
        self.bios = bios_menu::available_bios(&cfg);
        self.send(Cmd::Config(cfg));
        if let Some(game) = self.game.clone() {
            self.send(Cmd::Load { game });
        }
    }

    fn texture_options(&self) -> egui::TextureOptions {
        if self.prefs.display.smooth {
            egui::TextureOptions::LINEAR
        } else {
            egui::TextureOptions::NEAREST
        }
    }

    fn poll(&mut self, ctx: &egui::Context) {
        let Some(emu) = &self.emu else { return };
        let mut fresh = false;
        while let Ok(f) = emu.rx.try_recv() {
            self.last = Some(f);
            fresh = true;
        }
        while let Ok(e) = emu.err_rx.try_recv() {
            self.error = Some(e);
        }
        while let Ok(s) = emu.dbg_rx.try_recv() {
            self.debugger.set_snapshot(s);
        }
        let opts = self.texture_options();
        if let Some(f) = self
            .last
            .as_ref()
            .filter(|_| fresh || self.texture.is_none())
        {
            let img = to_color_image(&f.pixels);
            match &mut self.texture {
                Some(t) => t.set(img, opts),
                None => self.texture = Some(ctx.load_texture("fb", img, opts)),
            }
        }
    }

    fn screenshot(&mut self) {
        let (Some(cfg), Some(game), Some(f)) = (&self.cfg, &self.game, &self.last) else {
            return;
        };
        let dir = cfg.saves_dir.join("shots");
        let mut out = format!("P6\n{FB_W} {FB_H}\n255\n").into_bytes();
        for p in &f.pixels {
            out.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, *p as u8]);
        }
        let res = std::fs::create_dir_all(&dir)
            .and_then(|()| std::fs::write(dir.join(menu::shot_name(game, f.frame)), out));
        if let Err(e) = res {
            self.error = Some(format!("capture impossible : {e}"));
        }
    }

    fn set_paused(&mut self, p: bool) {
        self.paused = p;
        self.send(Cmd::Pause(p));
    }

    /// Avance d'une trame (met en pause si besoin).
    fn step_frame(&mut self) {
        if !self.paused {
            self.set_paused(true);
        }
        self.send(Cmd::StepFrame);
    }

    /// Informe le thread d'émulation quand l'onglet visible demande d'autres données.
    fn sync_debug_level(&mut self) {
        let level = self.debugger.level();
        if level != self.debug_level && self.emu.is_some() {
            self.send(Cmd::Debug(level));
            self.debug_level = level;
        }
    }

    fn status_text(&self, cfg: &Config) -> String {
        let scale = self.scale_info();
        format_status(&StatusInfo {
            game: self.game.as_deref(),
            bios: Some(bios_menu::bios_label(&cfg.bios)),
            model: Some(match cfg.model {
                ng_cart::Model::Mvs => "MVS",
                ng_cart::Model::Aes => "AES",
            }),
            frame: self.last.as_ref().map(|f| f.frame),
            fps: self.last.as_ref().map(|f| f.fps),
            scale: self.shown.is_some().then_some(scale.as_str()),
            paused: self.paused,
        })
    }

    fn handle(&mut self, ctx: &egui::Context, action: MenuAction) {
        match action {
            MenuAction::Load(g) => self.load_game(&g),
            MenuAction::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            MenuAction::Pause(p) => self.set_paused(p),
            MenuAction::Reset => self.send(Cmd::Reset),
            MenuAction::Scale(s) => self.prefs.display.scale = s,
            MenuAction::Fullscreen(f) => self.prefs.display.fullscreen = f,
            MenuAction::Aspect(a) => self.prefs.display.aspect = a,
            MenuAction::Smooth(s) => {
                self.prefs.display.smooth = s;
                // la texture reprend ses options au prochain rafraîchissement
                self.texture = None;
            }
            MenuAction::Screenshot => self.screenshot(),
            MenuAction::SetBios(file) => self.change_system(|c| c.bios = file),
            MenuAction::SetModel(m) => self.change_system(|c| c.model = m),
            MenuAction::Controls => self.controls.open = true,
            MenuAction::Debugger => self.debugger.open = !self.debugger.open,
            MenuAction::DebugTab(t) => {
                self.debugger.open = true;
                self.debugger.tab = t;
            }
            MenuAction::Step => self.step_frame(),
            MenuAction::StatusBar(b) => self.prefs.debug.show_status_bar = b,
        }
    }

    /// Raccourcis de l'interface (inactifs pendant une capture de touche).
    fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.controls.capturing() {
            return;
        }
        let pressed = |k: Key| ctx.input(|i| i.key_pressed(k));
        if pressed(Key::F11) {
            self.prefs.display.fullscreen = !self.prefs.display.fullscreen;
        }
        if pressed(Key::Escape) && self.prefs.display.fullscreen {
            self.prefs.display.fullscreen = false;
        }
        if pressed(Key::F5) {
            self.set_paused(!self.paused);
        }
        if pressed(Key::F6) {
            self.step_frame();
        }
        if pressed(Key::F9) {
            self.screenshot();
        }
        if pressed(Key::F12) {
            self.debugger.open = !self.debugger.open;
        }
    }

    /// Applique le plein écran à la fenêtre quand le réglage change (et au démarrage).
    fn apply_fullscreen(&mut self, ctx: &egui::Context) {
        let want = self.prefs.display.fullscreen;
        if self.fullscreen_applied != Some(want) {
            // au démarrage, ne rien envoyer si la fenêtre doit rester fenêtrée
            if want || self.fullscreen_applied.is_some() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(want));
            }
            self.fullscreen_applied = Some(want);
        }
        if want {
            let idle = ctx.input(|i| i.pointer.time_since_last_movement());
            if idle > CURSOR_HIDE_AFTER && !self.menu_shown {
                ctx.set_cursor_icon(egui::CursorIcon::None);
            }
        }
    }

    /// Dessine la trame dans la zone centrale : fond noir, facteur entier, pixels nets.
    fn draw_frame(&mut self, ui: &mut egui::Ui) {
        let avail = ui.max_rect();
        let Some(t) = &self.texture else {
            self.shown = None;
            ui.centered_and_justified(|ui| {
                ui.label("Fichier > Ouvrir pour lancer un jeu");
            });
            return;
        };
        let d = &self.prefs.display;
        let l = display::layout(avail, ui.pixels_per_point(), d.scale, d.aspect);
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        ui.painter_at(avail)
            .image(t.id(), l.rect, uv, egui::Color32::WHITE);
        self.shown = Some(l);
    }

    fn scale_info(&self) -> String {
        self.shown
            .as_ref()
            .map_or_else(|| "–".to_string(), display::scale_label)
    }

    /// Sauvegarde `gui.toml` quand les préférences ont changé depuis la dernière écriture.
    fn save_prefs_if_changed(&mut self) {
        if self.prefs == self.saved {
            return;
        }
        match self.prefs.save(&self.prefs_path) {
            Ok(()) => {}
            Err(e) => self.error = Some(format!("préférences non sauvegardées : {e}")),
        }
        // même en cas d'échec : ne pas réessayer à chaque trame
        self.saved = self.prefs.clone();
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let Some(cfg) = self.cfg.clone() else {
            egui::CentralPanel::default().show(ui, |ui| {
                if let Some(cfg) = self.form.ui(ui, &self.cfg_path) {
                    self.emu = Some(emu_thread::spawn(cfg.clone()));
                    self.bios = bios_menu::available_bios(&cfg);
                    self.error = missing_bios_message(&cfg, &self.bios);
                    self.cfg = Some(cfg);
                }
            });
            return;
        };
        self.poll(&ctx);
        self.shortcuts(&ctx);
        self.apply_fullscreen(&ctx);

        self.input.update_keys(&ctx);
        self.input.poll_gamepads();
        if self.controls.show(&ctx, &mut self.input) {
            self.prefs.input = self.input.bindings.to_prefs();
        }
        // pendant une capture de touche, rien n'est envoyé au jeu
        if !self.controls.capturing() {
            let (p1, p2) = self.input.pads();
            self.send(Cmd::Inputs {
                p1,
                p2,
                sys: self.input.system(),
            });
        }

        let fullscreen = self.prefs.display.fullscreen;
        self.menu_shown = !fullscreen
            || menu_revealed(
                self.menu_shown,
                ctx.input(|i| i.pointer.latest_pos()).map(|p| p.y),
                egui::Popup::is_any_open(&ctx),
            );
        if self.menu_shown {
            let scale_info = self.scale_info();
            let action = egui::Panel::top("menu")
                .show(ui, |ui| {
                    menu::show(
                        ui,
                        &MenuState {
                            cfg: &cfg,
                            paused: self.paused,
                            display: &self.prefs.display,
                            scale_info: &scale_info,
                            bios: &self.bios,
                            debug_open: self.debugger.open,
                            debug_tab: self.debugger.tab,
                            status_bar: self.prefs.debug.show_status_bar,
                        },
                    )
                })
                .inner;
            if let Some(a) = action {
                self.handle(&ctx, a);
            }
        }
        if let Some(e) = self.error.clone() {
            egui::Panel::top("error").show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.small_button("×").on_hover_text("Fermer").clicked() {
                        self.error = None;
                    }
                    ui.colored_label(egui::Color32::LIGHT_RED, e);
                });
            });
        }
        if !fullscreen && self.prefs.debug.show_status_bar {
            let text = self.status_text(&cfg);
            egui::Panel::bottom("status").show(ui, |ui| {
                ui.monospace(text);
            });
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::BLACK))
            .show(ui, |ui| self.draw_frame(ui));

        if self.debugger.open {
            match self.debugger.show(&ctx, self.paused) {
                Some(DebugAction::Pause(p)) => self.set_paused(p),
                Some(DebugAction::Step) => self.step_frame(),
                None => {}
            }
        }
        self.sync_debug_level();
        self.prefs.debug.open = self.debugger.open;
        self.prefs.debug.tab = self.debugger.tab.name().to_string();
        self.prefs.debug.region = self.debugger.region.name().to_string();
        self.save_prefs_if_changed();
        ctx.request_repaint();
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.save_prefs_if_changed();
        self.send(Cmd::Quit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_conversion() {
        let mut px = vec![0u32; FB_W * FB_H];
        px[0] = 0x00FF_8040;
        let img = to_color_image(&px);
        assert_eq!(img.pixels[0], egui::Color32::from_rgb(255, 128, 64));
    }

    #[test]
    fn fullscreen_menu_reveal() {
        assert!(!menu_revealed(false, Some(100.0), false));
        assert!(menu_revealed(false, Some(3.0), false));
        // une fois visible, elle reste tant que la souris est sur la barre ou qu'un menu est ouvert
        assert!(menu_revealed(true, Some(20.0), false));
        assert!(!menu_revealed(true, Some(40.0), false));
        assert!(menu_revealed(false, Some(400.0), true));
        assert!(!menu_revealed(true, None, false));
    }
}
