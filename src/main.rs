#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod memory;
mod process;
mod scanner;
mod theme;
mod types;
mod worker;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1050.0, 740.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Game Editor",
        options,
        Box::new(|cc| {
            theme::apply(&cc.egui_ctx);
            Ok(Box::new(app::App::new()))
        }),
    )
}
