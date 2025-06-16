mod calibration;
mod commands;
mod gui;
mod image;

fn main() {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default().with_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Eventide",
        options,
        Box::new(|cc| Ok(Box::new(gui::EventideApp::new(cc)))),
    )
    .unwrap_or_else(|e| {
        eprintln!("Failed to start application: {}", e);
    });
}
