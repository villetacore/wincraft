# Changelog

## 0.4.0 — developer preview

The first source publication prepares the existing WinCraft OS prototype for collaboration and automated releases.

### System

- Buildroot-based x86-64 live system with Linux, a read-only squashfs, and a RAM overlay.
- Rust early boot, PID 1, user session manager, desktop, and system applications.
- Wayland desktop using labwc, seatd, and XWayland; Wine WoW64 package for Windows applications.
- Explorer, Start menu, Notepad, Task Manager, and WebKitGTK browser.
- Optional persistent data disk and QEMU launch/acceptance tools.

### Release engineering

- Windows/Linux library CI, Linux guest checks, formatting and repository hygiene checks.
- Tag-driven ISO builds, BIOS/UEFI boot smoke checks, prerelease publication, and SHA-256 checksums.
- Source/license packaging, MIT metadata alignment, contributor documentation, and project-only public identity.

### Known limitations

- No installer, automatic updater, or login screen.
- QEMU x86-64 is the current target; physical hardware is not qualified.
- Compatibility with arbitrary Windows applications is not guaranteed.
- Full clean ISO builds and guest acceptance must be validated on the release runner; host library tests cover only part of the system.
