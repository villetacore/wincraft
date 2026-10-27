//! explorer: the WinCraft shell (the explorer.exe role).
//!
//! One window per process, all from this binary:
//!
//! | invocation                      | window                                        |
//! |---------------------------------|-----------------------------------------------|
//! | `explorer`                      | desktop + shell host (or ask the running one) |
//! | `explorer --taskbar`            | taskbar (started and supervised by the host)  |
//! | `explorer --start-menu`         | Start menu                                    |
//! | `explorer --folder <target>`    | File Explorer window                          |
//! | `explorer --run-dialog`         | Run                                           |
//! | `explorer --settings-window`    | Settings                                      |
//! | `explorer --message <text>`     | message box                                   |
//! | `explorer --extract <file.zip>` | extract a zip archive, open the new folder    |
//! | `explorer --list-windows`       | (no window) print taskbar windows, for tests  |
//!
//! Public forms used by key bindings and other programs (`explorer --start`, `--run`,
//! `--settings`, `explorer C:\path`) are forwarded to the running shell host over a Unix
//! socket, like a second explorer.exe hands its request to the first one.
//!
//! Runs only inside the WinCraft guest (ADR-016).
#[cfg(target_os = "linux")]
mod actions;
#[cfg(target_os = "linux")]
mod catalog;
#[cfg(target_os = "linux")]
mod desktop;
#[cfg(target_os = "linux")]
mod dialogs;
#[cfg(target_os = "linux")]
mod files;
#[cfg(target_os = "linux")]
mod folder;
#[cfg(target_os = "linux")]
mod ipc;
#[cfg(target_os = "linux")]
mod settings;
#[cfg(target_os = "linux")]
mod start_menu;
#[cfg(target_os = "linux")]
mod system;
#[cfg(target_os = "linux")]
mod taskbar;

#[cfg(target_os = "linux")]
fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("--taskbar") => taskbar::run(),
        Some("--start-menu") => start_menu::run(),
        Some("--folder") => folder::run(
            args.get(1)
                .cloned()
                .unwrap_or_else(|| "shell:ThisPC".into()),
        ),
        Some("--run-dialog") => dialogs::run_dialog(),
        Some("--settings-window") => settings::run(),
        Some("--message") => dialogs::message(args.get(1).cloned().unwrap_or_default()),
        // "Extract All" for .zip archives, then the folder with the files opens.
        Some("--extract") => match args
            .get(1)
            .map(std::path::Path::new)
            .map(files::extract_zip)
        {
            Some(Ok(dir)) => actions::spawn_self(&["--folder", &dir.display().to_string()]),
            Some(Err(e)) => dialogs::message(e),
            None => Err("--extract needs an archive".into()),
        },
        // Diagnostics for tests and support: the taskbar's view of the windows, one per line.
        Some("--list-windows") => wc_wayland::WindowList::connect()
            .map_err(|e| e.to_string())
            .map(|list| {
                for w in list.windows() {
                    println!(
                        "{}\t{}\t{}\t{}",
                        w.id,
                        w.app_id,
                        if w.minimized { "minimized" } else { "normal" },
                        w.title
                    );
                }
            }),
        _ => {
            let request = ipc::Request::from_args(args.into_iter());
            if ipc::forward(&request).is_ok() {
                return std::process::ExitCode::SUCCESS;
            }
            desktop::run(request)
        }
    };
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("explorer: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("explorer is the WinCraft guest shell; start the OS in QEMU (WinCraft.cmd)");
}
