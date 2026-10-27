//! browser: WinCraft Browser (the msedge.exe role), on the WebKitGTK engine.
//!
//! ```text
//! browser [address-or-search ...]   one window, one tab per argument
//! ```
//!
//! Tabs, an address bar that also searches, persistent cookies and history
//! (`~/.local/share/wincraft/browser`), and downloads to the Downloads folder with "Open",
//! which hands installers to the Win32 subsystem through the `start` launcher.
mod address;
#[cfg(target_os = "linux")]
mod app;

#[cfg(target_os = "linux")]
fn main() -> webkit6::glib::ExitCode {
    app::run()
}

#[cfg(not(target_os = "linux"))]
fn main() {
    // The pure address logic is still built and tested on the host.
    let _ = (address::resolve, address::safe_file_name);
    eprintln!("browser runs only inside the WinCraft guest");
}
