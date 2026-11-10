// Les commandes Inputs sont branchées par T801b : dead_code toléré d'ici là.
#![allow(dead_code)]

mod app;
mod audio;
mod emu_thread;
mod menu;
mod settings;

fn main() -> eframe::Result {
    let path = settings::config_path();
    eframe::run_native(
        "Puce16 NeoGeo",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(app::App::new(path)))),
    )
}
