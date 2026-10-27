//! `/init` of the WinCraft initramfs (the role of NTLDR/winload hand-off in ReactOS terms).
//!
//! 1. mount proc, sysfs, devtmpfs
//! 2. find the live medium: an ISO 9660 volume whose `/wincraft/id` equals `wincraft.media=`
//!    (only ISO 9660 is probed, read-only, so local disks are never written)
//! 3. optionally verify `rootfs.squashfs` against `/wincraft/SHA256SUMS` (`wincraft.verify`)
//! 4. attach the image to a loop device, mount squashfs read-only, tmpfs upper, overlay
//! 5. move the mounts into the new root and switch_root to `/usr/bin/wincraft-init`
//!
//! The kernel has every needed driver built in; this binary is static and needs nothing else.
#[cfg(target_os = "linux")]
mod boot;

#[cfg(target_os = "linux")]
fn main() {
    boot::main();
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("wincraft-boot runs only as /init of the WinCraft initramfs");
}
