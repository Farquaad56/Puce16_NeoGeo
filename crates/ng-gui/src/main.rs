// Les commandes Reset/Pause/Inputs sont branchées par T800b/T801b : dead_code toléré d'ici là.
#![allow(dead_code)]

mod app;
mod emu_thread;

use std::path::Path;

fn main() -> eframe::Result {
    let cfg = ng_cart::load_config(Path::new("neogeo.toml")).unwrap_or_else(|e| {
        eprintln!("configuration : {e}");
        std::process::exit(1);
    });
    let emu = emu_thread::spawn(cfg);
    if let Some(game) = std::env::args().nth(1) {
        let _ = emu.tx.send(emu_thread::Cmd::Load { game });
    }
    eframe::run_native(
        "Puce16 NeoGeo",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(app::App::new(emu)))),
    )
}
