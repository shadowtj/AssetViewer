mod app;
mod config;
mod fs_model;
mod preview;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Asset Viewer")
            .with_inner_size([1280.0, 720.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Asset Viewer",
        native_options,
        Box::new(|cc| Box::new(app::AssetViewerApp::new(cc))),
    )
}
