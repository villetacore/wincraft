//! winlogon: the session manager (winlogon.exe + userinit.exe roles).
//!
//! Runs as root under `wincraft-init`. Starts seatd (the only privileged part of the
//! graphics stack), then as the unprivileged account: the labwc Wayland compositor, Wine
//! prefix preparation and the shell (`explorer`), which is restarted if it crashes — as
//! Windows restarts explorer.exe. Exits when the compositor dies or the user signs out;
//! init then starts a new session.
//!
//! `winlogon --report-session` is the compositor's startup command: it reports the
//! session's socket names back to the session manager.
#[cfg(target_os = "linux")]
mod session;
#[cfg(target_os = "linux")]
mod user;

#[cfg(target_os = "linux")]
fn main() {
    if std::env::args().nth(1).as_deref() == Some("--report-session") {
        std::process::exit(session::report());
    }
    std::process::exit(session::run());
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("winlogon runs only inside the WinCraft guest");
}
