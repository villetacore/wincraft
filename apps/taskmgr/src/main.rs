//! Task Manager: processes with CPU and memory use, End task, performance graphs.
#[cfg(target_os = "linux")]
mod app;
#[cfg(target_os = "linux")]
mod procfs;

#[cfg(target_os = "linux")]
fn main() -> eframe::Result {
    eframe::run_native(
        "taskmgr",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("Task Manager")
                .with_app_id("taskmgr")
                .with_inner_size([760.0, 560.0])
                .with_min_inner_size([480.0, 320.0]),
            renderer: eframe::Renderer::Glow,
            ..Default::default()
        },
        Box::new(|cc| {
            wc_ui::theme::install_fonts(&cc.egui_ctx);
            wc_ui::Theme::load().apply(&cc.egui_ctx);
            Ok(Box::new(app::TaskManager::default()))
        }),
    )
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("taskmgr runs inside the WinCraft guest");
}
