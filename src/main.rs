//! KbWrite — a local-first, Obsidian-style knowledge base written in native Rust.
mod ai;
mod app;
mod blocks;
mod editor;
mod settings;
mod vault;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1240.0, 820.0])
            .with_min_inner_size([720.0, 480.0])
            .with_title("KbWrite"),
        ..Default::default()
    };

    eframe::run_native(
        "KbWrite",
        options,
        Box::new(|cc| {
            // Fonts: Phosphor icons + platform default UI/code fonts.
            cc.egui_ctx.set_fonts(app::font_definitions());
            Ok(app::KbWriteApp::new(cc) as Box<dyn eframe::App>)
        }),
    )
}
