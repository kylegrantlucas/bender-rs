//! `bender-gui`: live preview and render app for bender-rs.

mod app;
mod preview;

use std::path::PathBuf;

use app::BenderApp;

fn main() -> eframe::Result {
    env_logger::init();
    if let Err(e) = bender_gst::init() {
        eprintln!("bender-gui: {e}");
        std::process::exit(1);
    }
    let open = std::env::args_os().nth(1).map(PathBuf::from);
    eframe::run_native(
        "bender-rs",
        eframe::NativeOptions::default(),
        Box::new(|cc| Ok(Box::new(BenderApp::new(cc, open)))),
    )
}
