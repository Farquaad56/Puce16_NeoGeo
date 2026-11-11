mod app;
mod audio;
mod bios_menu;
mod controls_menu;
mod debugger;
mod dips_menu;
mod display;
mod emu_thread;
mod fx_menu;
mod input;
mod menu;
mod overlay;
mod postfx;
mod prefs;
mod settings;
mod statusbar;
mod video_debug;

fn main() -> eframe::Result {
    let path = settings::config_path();
    // argument 2 (facultatif) : jeu à lancer au démarrage, ex. `ng-gui neogeo.toml mslug`
    let game = std::env::args().nth(2);
    eframe::run_native(
        "Puce16 NeoGeo",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(app::App::new(path, game)))),
    )
}
