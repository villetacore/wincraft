<div align="center">

# WinCraft OS

**A familiar desktop. An open foundation.**

An experimental Linux system with a Rust desktop, Windows-style paths, and Windows application support through Wine.

[![CI](https://github.com/villetacore/wincraft/actions/workflows/ci.yml/badge.svg)](https://github.com/villetacore/wincraft/actions/workflows/ci.yml)
[![Release](https://github.com/villetacore/wincraft/actions/workflows/release.yml/badge.svg)](https://github.com/villetacore/wincraft/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Status: Developer preview](https://img.shields.io/badge/status-developer%20preview-orange.svg)](#project-status)

[Download previews](https://github.com/villetacore/wincraft/releases) · [Documentation](docs/README.md) · [Report a bug](https://github.com/villetacore/wincraft/issues/new/choose) · [Contribute](CONTRIBUTING.md)

</div>

---

WinCraft explores a desktop that feels familiar to Windows users while remaining an open Unix system underneath. It combines a vanilla Linux kernel, a custom Rust shell, a Wayland session, and Wine. The current target is **x86-64 QEMU virtual machines**.

## Project status

**0.4.0 · Developer preview.** This is an experimental operating system. There is no installer, automatic updater, or login screen yet. Windows application compatibility varies; Windows drivers and anti-cheat support are outside the current scope. Use a disposable VM and keep backups of anything you import.

Release builds are published as **prereleases** after CI and BIOS/UEFI boot checks pass. If the Releases page is empty, the first verified ISO has not been published yet. The presence of a workflow badge does not certify compatibility with arbitrary Windows applications.

## Explore the desktop

- **Desktop and Explorer** — taskbar, Start menu, file browsing, Run dialog, Settings, and Recycle Bin.
- **Familiar paths** — `C:\`, `D:\`, known user folders, and case-insensitive path resolution.
- **Windows applications** — PE detection and launching through Wine; a WoW64 build targets x64 and x86 programs.
- **Built-in tools** — Notepad, Task Manager, and a WebKitGTK browser.
- **Open system** — Linux, a Rust PID 1 and session manager, and a Wayland desktop running as an ordinary user.
- **Live environment** — a read-only system image with a RAM overlay; an optional data disk provides persistence.

| Shortcut | Action |
| --- | --- |
| Win | Start menu |
| Win+E / Win+R | Explorer / Run |
| Win+D / Alt+Tab | Desktop / switch windows |
| Alt+F4 / Win+← / Win+→ | Close / snap window |
| Ctrl+Shift+Esc | Task Manager |

## Try a release

Download the ISO, `wincraft-launcher.zip`, and `SHA256SUMS` from the **same** [release](https://github.com/villetacore/wincraft/releases). QEMU is required on the host; it is not bundled.

### Windows

1. Install QEMU for Windows, including its UEFI firmware.
2. Extract `wincraft-launcher.zip`.
3. Put the downloaded ISO in its `dist` directory as `wincraft-os.iso`.
4. Run `WinCraft.cmd`. The launcher uses WHPX when available and falls back to software emulation.

Optional settings:

```powershell
.\scripts\run-iso.ps1 -Memory 4096 -Cpus 2
.\scripts\run-iso.ps1 -Bios -NoData -Accel tcg
```

Check the downloaded ISO against its entry in `SHA256SUMS`:

```powershell
Get-FileHash .\dist\wincraft-os.iso -Algorithm SHA256
```

### Linux

With QEMU installed, boot the downloaded ISO using its actual filename:

```sh
sha256sum -c SHA256SUMS --ignore-missing
qemu-system-x86_64 -machine q35 -accel tcg -cpu Nehalem \
  -m 4096 -smp 2 -vga std -boot d -cdrom wincraft-os-v0.4.0-x86_64.iso
```

The filename above is an example; prerelease tags add a suffix. The ISO supports both BIOS and UEFI. A host with at least 8 GB RAM is recommended when assigning 4 GB to the VM.

**Persistence:** without a `WINCRAFT_DATA` disk, changes disappear at shutdown. See [the build and data guide](docs/BUILDING.md) before creating a persistent disk. The debug boot menu entry exposes a root shell on the VM serial console; the default boot does not.

## Build from source

Fast library checks work on Windows, Linux, and macOS:

```sh
git clone https://github.com/villetacore/wincraft.git
cd wincraft
cargo test --locked
```

The full OS build requires an **x86-64 Linux host** or Ubuntu under WSL, root privileges for an isolated builder, internet access, and substantial disk space. It compiles Linux, Mesa, WebKitGTK, Wine, and the Rust workspace from source; the first build can take many hours.

```sh
sudo bash build/builder.sh setup
sudo bash build/builder.sh run bash build/release.sh
```

Windows entry point: `Build-WinCraft.cmd` (Ubuntu WSL and Rust MSVC required).

See [BUILDING.md](docs/BUILDING.md) for prerequisites, guest checks, data disks, and the distinction between the current Buildroot pipeline and the legacy Alpine pipeline.

## Architecture

```text
GRUB (BIOS / UEFI)
  └─ Linux + wincraft-boot       live media, squashfs, RAM overlay
      └─ wincraft-init          PID 1, mounts, services, shutdown
          └─ winlogon           user session and Wine prefix
              └─ labwc         Wayland compositor + XWayland
                  ├─ explorer  desktop, taskbar, Start, files
                  ├─ notepad · taskmgr · browser
                  └─ Wine      Windows applications
```

| Directory | Contents |
| --- | --- |
| [`apps/`](apps/) | Desktop and applications |
| [`crates/`](crates/) | Formats, paths, launch/runtime logic, UI, Wayland helpers |
| [`system/`](system/) | Early boot, PID 1, session manager |
| [`distro/`](distro/) | Buildroot configuration, package recipes, root filesystem overlay |
| [`build/`](build/) | Isolated builder and release packaging |
| [`scripts/`](scripts/) | VM launchers, smoke checks, maintenance tools |
| [`docs/`](docs/README.md) | Architecture, decisions, roadmap, engineering notes (mostly Russian) |

The [architecture guide](docs/12-wincraft-os.md) explains the system in detail. The [roadmap](docs/06-roadmap.md) distinguishes implemented features from future work.

## Development and releases

Pull requests and branch pushes run formatting, library tests on Windows/Linux, Linux guest checks, and repository hygiene checks. A `v*` tag additionally builds the ISO, collects sources and notices, boots the ISO in BIOS and UEFI QEMU VMs, and publishes a prerelease with SHA-256 checksums.

[Contributing](CONTRIBUTING.md) · [Release process](docs/RELEASING.md) · [Security policy](SECURITY.md) · [Changelog](CHANGELOG.md)

## License and credits

Original WinCraft code is released under the [MIT License](LICENSE), maintained by **villetacore**. Third-party components retain their respective licenses; the complete OS image is not covered solely by MIT. Release source bundles include Buildroot legal information, upstream sources, patches, and vendored Cargo dependencies; see [THIRD_PARTY.md](THIRD_PARTY.md).

Built on the work of the Linux, Rust, Buildroot, Wine, labwc, wlroots, Wayland, Mesa, GTK, WebKitGTK, and DejaVu communities. WinCraft is an independent project and is not affiliated with Microsoft or the ReactOS project. Windows is a trademark of Microsoft.
