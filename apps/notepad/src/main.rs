//! Notepad for WinCraft: plain-text editing with Windows Notepad's menus and shortcuts.
//! Keeps the file's line endings (CRLF or LF) and asks before discarding changes.
#[cfg(target_os = "linux")]
mod app;

#[cfg(target_os = "linux")]
fn main() -> eframe::Result {
    let path = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    eframe::run_native(
        "notepad",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("Notepad")
                .with_app_id("notepad")
                .with_inner_size([820.0, 560.0])
                .with_min_inner_size([360.0, 240.0]),
            renderer: eframe::Renderer::Glow,
            ..Default::default()
        },
        Box::new(move |cc| {
            wc_ui::theme::install_fonts(&cc.egui_ctx);
            wc_ui::Theme::load().apply(&cc.egui_ctx);
            Ok(Box::new(app::Notepad::new(path)))
        }),
    )
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("notepad runs inside the WinCraft guest");
}
