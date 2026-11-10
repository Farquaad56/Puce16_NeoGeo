//! Interface egui : affiche la dernière trame reçue du thread d'émulation.

use eframe::egui;

use crate::emu_thread::{Cmd, EmuHandle, FrameMsg};

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

pub struct App {
    emu: EmuHandle,
    texture: Option<egui::TextureHandle>,
    last: Option<FrameMsg>,
    error: Option<String>,
    paused: bool,
}

impl App {
    pub fn new(emu: EmuHandle) -> Self {
        Self {
            emu,
            texture: None,
            last: None,
            error: None,
            paused: false,
        }
    }

    pub fn send(&self, cmd: Cmd) {
        let _ = self.emu.tx.send(cmd);
    }

    pub fn set_paused(&mut self, p: bool) {
        self.paused = p;
        self.send(Cmd::Pause(p));
    }

    fn poll(&mut self, ctx: &egui::Context) {
        while let Ok(f) = self.emu.rx.try_recv() {
            self.last = Some(f);
        }
        while let Ok(e) = self.emu.err_rx.try_recv() {
            self.error = Some(e);
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

    pub fn show(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        self.poll(ctx);
        if let Some(e) = &self.error {
            ui.colored_label(egui::Color32::RED, e);
        }
        if let Some(t) = &self.texture {
            let avail = ui.available_size();
            let s = integer_scale(avail.x, avail.y) as f32;
            ui.centered_and_justified(|ui| {
                ui.image((t.id(), egui::vec2(FB_W as f32 * s, FB_H as f32 * s)));
            });
        } else {
            ui.label("Aucun jeu chargé");
        }
        if let Some(f) = &self.last {
            ui.ctx().debug_text(format!("{:.1} fps", f.fps));
        }
        ctx.request_repaint();
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
        let _ = self.emu.tx.send(Cmd::Quit);
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
