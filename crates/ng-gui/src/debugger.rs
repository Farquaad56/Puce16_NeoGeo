//! Fenêtre de débogage : registres 68000 / Z80 et visionneuse de la RAM de travail.

use eframe::egui;
use ng_core::DebugSnapshot;

use crate::video_debug::VideoDebug;

/// Adresse de base de la RAM de travail.
pub const WORK_RAM_BASE: u32 = 0x100000;

/// « 100000: 00 01 ... 0F » (adresse sur 6 chiffres, octets en hexa majuscule).
pub fn hex_line(addr: u32, bytes: &[u8]) -> String {
    let mut s = format!("{addr:06X}:");
    for b in bytes {
        s.push_str(&format!(" {b:02X}"));
    }
    s
}

#[derive(Default)]
pub struct Debugger {
    pub open: bool,
    pub snap: Option<DebugSnapshot>,
    video: VideoDebug,
}

impl Debugger {
    pub fn show(&mut self, ctx: &egui::Context) {
        let mut open = self.open;
        egui::Window::new("Débogueur")
            .open(&mut open)
            .show(ctx, |ui| {
                let Some(s) = self.snap.as_ref() else {
                    ui.label("En attente d'une trame…");
                    return;
                };
                ui.monospace(format!("now={}  vpos={}", s.now, s.vpos));
                ui.separator();
                ui.monospace(format!(
                    "PC={:08X}  SR={:04X}  USP={:08X}  SSP={:08X}",
                    s.m68k.pc, s.m68k.sr, s.m68k.usp, s.m68k.ssp
                ));
                egui::Grid::new("regs68k").show(ui, |ui| {
                    for i in 0..8 {
                        ui.monospace(format!("D{i}={:08X}", s.m68k.d[i]));
                        let a = if i < 7 { s.m68k.a[i] } else { 0 };
                        if i < 7 {
                            ui.monospace(format!("A{i}={a:08X}"));
                        }
                        ui.end_row();
                    }
                });
                ui.separator();
                ui.monospace(format!("Z80 {:?}", s.z80));
                ui.separator();
                egui::ScrollArea::vertical().max_height(260.0).show_rows(
                    ui,
                    ui.text_style_height(&egui::TextStyle::Monospace),
                    s.work_ram.len() / 16,
                    |ui, rows| {
                        for r in rows {
                            let off = r * 16;
                            ui.monospace(hex_line(
                                WORK_RAM_BASE + off as u32,
                                &s.work_ram[off..off + 16],
                            ));
                        }
                    },
                );
            });
        self.open = open;
        if let Some(s) = self.snap.as_ref().filter(|_| self.open) {
            self.video.show(ctx, s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_line_format() {
        let b: Vec<u8> = (0..16).collect();
        assert_eq!(
            hex_line(0x100000, &b),
            "100000: 00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F"
        );
    }
}
