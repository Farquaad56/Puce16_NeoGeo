mod app;
mod audio;
mod debugger;
mod emu_thread;
mod input;
mod menu;
mod settings;
mod video_debug;

fn main() -> eframe::Result {
    let path = settings::config_path();
    eframe::run_native(
        "Puce16 NeoGeo",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(app::App::new(path)))),
    )
}
