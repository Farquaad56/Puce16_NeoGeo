//! Interface egui : affiche la dernière trame reçue du thread d'émulation.

use eframe::egui;

use std::path::PathBuf;

use ng_cart::Config;

use crate::bios_menu::{self, BiosEntry};
use crate::controls_menu::ControlsWindow;
use crate::debugger::Debugger;
use crate::emu_thread::{self, Cmd, EmuHandle, FrameMsg};
use crate::input::{Bindings, InputState};
use crate::menu::{self, MenuAction, MenuState, Scale};
use crate::prefs::{self, Prefs};
use crate::settings::ConfigForm;

pub const FB_W: usize = 320;
pub const FB_H: usize = 224;

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

/// Échelle entière maximale (au moins 1) tenant dans la zone.
pub fn integer_scale(avail_w: f32, avail_h: f32) -> u32 {
    let s = (avail_w / FB_W as f32).min(avail_h / FB_H as f32).floor();
    (s as u32).max(1)
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
}

impl App {
    pub fn new(cfg_path: PathBuf) -> Self {
        let cfg = ng_cart::load_config(&cfg_path).ok();
        let emu = cfg.clone().map(emu_thread::spawn);
        let prefs_path = prefs::prefs_path(&cfg_path);
        let prefs = Prefs::load(&prefs_path);
        let bindings = Bindings::from_prefs(&prefs.input);
        let mut debugger = Debugger::default();
        debugger.open = prefs.debug.open;
        if let (Some(e), true) = (&emu, debugger.open) {
            let _ = e.tx.send(Cmd::Debug(true));
        }
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

    fn poll(&mut self, ctx: &egui::Context) {
        let Some(emu) = &self.emu else { return };
        while let Ok(f) = emu.rx.try_recv() {
            self.last = Some(f);
        }
        while let Ok(e) = emu.err_rx.try_recv() {
            self.error = Some(e);
        }
        while let Ok(s) = emu.dbg_rx.try_recv() {
            self.debugger.snap = Some(s);
        }
        if let Some(f) = &self.last {
            let img = to_color_image(&f.pixels);
            match &mut self.texture {
                Some(t) => t.set(img, egui::TextureOptions::NEAREST),
                None => {
                    self.texture = Some(ctx.load_texture("fb", img, egui::TextureOptions::NEAREST))
                }
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

    fn handle(&mut self, ctx: &egui::Context, action: MenuAction) {
        match action {
            MenuAction::Load(g) => self.load_game(&g),
            MenuAction::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            MenuAction::Pause(p) => {
                self.paused = p;
                self.send(Cmd::Pause(p));
            }
            MenuAction::Reset => self.send(Cmd::Reset),
            MenuAction::Scale(s) => self.prefs.display.scale = s,
            MenuAction::Screenshot => self.screenshot(),
            MenuAction::SetBios(file) => self.change_system(|c| c.bios = file),
            MenuAction::SetModel(m) => self.change_system(|c| c.model = m),
            MenuAction::Controls => self.controls.open = true,
            MenuAction::Debugger => {
                self.debugger.open = !self.debugger.open;
                self.send(Cmd::Debug(self.debugger.open));
            }
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let Some(cfg) = self.cfg.clone() else {
            if let Some(cfg) = self.form.ui(ui, &self.cfg_path) {
                self.emu = Some(emu_thread::spawn(cfg.clone()));
                self.bios = bios_menu::available_bios(&cfg);
                self.error = missing_bios_message(&cfg, &self.bios);
                if self.debugger.open {
                    self.send(Cmd::Debug(true));
                }
                self.cfg = Some(cfg);
            }
            return;
        };
        self.poll(ctx);
        self.input.update_keys(ctx);
        self.input.poll_gamepads();
        if self.controls.show(ctx, &mut self.input) {
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
        let action = menu::show(
            ui,
            &MenuState {
                cfg: &cfg,
                paused: self.paused,
                scale: self.prefs.display.scale,
                bios: &self.bios,
            },
        );
        if let Some(a) = action {
            self.handle(ctx, a);
        }
        if let Some(f) = &self.last {
            ui.label(format!("{:.1} fps", f.fps));
        }
        if let Some(e) = &self.error {
            ui.colored_label(egui::Color32::RED, e);
        }
        if let Some(t) = &self.texture {
            let avail = ui.available_size();
            let s = match self.prefs.display.scale {
                Scale::Auto => integer_scale(avail.x, avail.y),
                Scale::Fixed(n) => n,
            } as f32;
            ui.centered_and_justified(|ui| {
                ui.image((t.id(), egui::vec2(FB_W as f32 * s, FB_H as f32 * s)));
            });
        } else {
            ui.label("Fichier > Ouvrir pour lancer un jeu");
        }
        if self.debugger.open {
            self.debugger.show(ctx);
        }
        self.prefs.debug.open = self.debugger.open;
        self.save_prefs_if_changed();
        ctx.request_repaint();
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
        egui::CentralPanel::default().show(ui, |ui| self.show(&ctx, ui));
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
    fn scale_is_integer() {
        assert_eq!(integer_scale(1000.0, 700.0), 3);
        assert_eq!(integer_scale(300.0, 200.0), 1);
    }
}
