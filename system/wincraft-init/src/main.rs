//! PID 1 of WinCraft OS: the smss.exe + services.exe role of the NT/ReactOS design.
//!
//! Reached from `wincraft-boot` (live ISO: squashfs + RAM overlay) or directly from a
//! developer initramfs. Responsibilities:
//! - system mounts (critical ones fail into a visible error state)
//! - the optional data disk, found only by its `WINCRAFT_DATA` ext4 label
//! - registering Windows executables with binfmt_misc
//! - supervising services, today `winlogon` (session), with restart backoff
//! - power actions requested by the unprivileged session, and orderly shutdown
#[cfg(target_os = "linux")]
mod init;
#[cfg(target_os = "linux")]
mod storage;

#[cfg(target_os = "linux")]
fn main() {
    init::main();
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("wincraft-init is PID 1 of the WinCraft guest and does not run on this host");
}
