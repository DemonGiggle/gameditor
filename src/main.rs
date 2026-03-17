#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod memory;
mod process;
mod scanner;
mod types;
mod worker;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1000.0, 700.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Memory Scanner",
        options,
        Box::new(|_cc| Ok(Box::new(app::App::new()))),
    )
}
