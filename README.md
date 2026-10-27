<div align="center">

<img src="docs/assets/banner.svg" alt="WinCraft OS — A familiar desktop. An open foundation." width="100%">

An experimental Linux system with a Rust desktop, Windows-style paths, and Windows application support through Wine.

[![CI](https://github.com/villetacore/wincraft/actions/workflows/ci.yml/badge.svg)](https://github.com/villetacore/wincraft/actions/workflows/ci.yml)
[![Release](https://github.com/villetacore/wincraft/actions/workflows/release.yml/badge.svg)](https://github.com/villetacore/wincraft/actions/workflows/release.yml)
[![Preview](https://img.shields.io/github/v/release/villetacore/wincraft?include_prereleases&color=168fd3&label=preview)](https://github.com/villetacore/wincraft/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Status: Developer preview](https://img.shields.io/badge/status-developer%20preview-orange.svg)](#project-status)

[Download previews](https://github.com/villetacore/wincraft/releases) · [Documentation](docs/README.md) · [Report a bug](https://github.com/villetacore/wincraft/issues/new/choose) · [Contribute](CONTRIBUTING.md) · [Русский](README.ru.md)

</div>

---

WinCraft explores a desktop that feels familiar to Windows users while remaining an open Unix system underneath. It combines a vanilla Linux kernel, a custom Rust shell, a Wayland session, and Wine. The current target is **x86-64 QEMU virtual machines**.

<p align="center">
  <a href="docs/screenshots/explorer.png"><img src="docs/screenshots/explorer.png" alt="WinCraft Explorer showing familiar folders and C:, D:, and Z: drives" width="100%"></a>
  <br><sub><b>Your files, in familiar places.</b> Explorer on the WinCraft desktop.</sub>
</p>

<table>
<tr>
<td width="50%"><a href="docs/screenshots/desktop.png"><img src="docs/screenshots/desktop.png" alt="WinCraft desktop with its blue wallpaper, desktop icons, and taskbar" width="100%"></a></td>
<td width="50%"><a href="docs/screenshots/win32-demo.png"><img src="docs/screenshots/win32-demo.png" alt="A real Windows PE test program displaying a Win32 message box through Wine" width="100%"></a></td>
</tr>
<tr>
<td align="center"><b>The desktop</b><br>Rust shell, taskbar, and desktop icons</td>
<td align="center"><b>Windows meets Unix</b><br>A Win32 test application running through Wine</td>
</tr>
</table>

<sub>Actual QEMU screenshots from a local live-image development build, captured on 14 October 2026. They illustrate the prototype; they are not screenshots of a verified GitHub release.</sub>

## At a glance

<table>
<tr>
<td width="50%" valign="top">

### A desktop you already know

Explorer, Start, the taskbar, familiar keyboard shortcuts, and a Run dialog. Windows-style drive letters and known folders, backed by an open Unix filesystem.

</td>
<td width="50%" valign="top">

### Windows applications, through Wine

PE detection and application launching, with a WoW64 package targeting x64 and x86 programs. Compatibility depends on the application.

</td>
</tr>
<tr>
<td valign="top">

### Rust at the core of the experience

Custom early boot, PID 1, session manager, desktop, and system applications. Wayland graphics through labwc, XWayland, and Mesa.

</td>
<td valign="top">

### Boot, explore, start again

A live ISO for QEMU with BIOS and UEFI support. The system image stays read-only; changes live in RAM unless an optional data disk is attached.

</td>
</tr>
</table>

## Project status

**0.4.0 · Developer preview.** This is an experimental operating system. There is no installer, automatic updater, or login screen yet. Windows application compatibility varies; Windows drivers and anti-cheat support are outside the current scope. Use a disposable VM and keep backups of anything you import.

Release builds are published as **prereleases** after CI and BIOS/UEFI boot checks pass. If the Releases page is empty, the first verified ISO has not been published yet. The presence of a workflow badge does not certify compatibility with arbitrary Windows applications.

## Explore the desktop

Built-in tools include **Notepad**, **Task Manager**, and a **WebKitGTK browser**. The shell includes Settings and a Recycle Bin.

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

<img src="docs/assets/architecture.svg" alt="WinCraft architecture: applications, Rust desktop and Wine, Wayland graphics, Linux and system services" width="100%">


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

Pull requests and branch pushes run formatting, library tests on Windows/Linux, Linux guest checks, and repository hygiene checks. Code/build pushes to `main` also build an ISO, collect sources and notices, and boot the ISO in BIOS and UEFI QEMU VMs. Download successful builds from **Actions → Release → Artifacts → wincraft-iso-…** (retained for seven days). A `v*` tag publishes a prerelease with SHA-256 checksums. You can also start an ISO build with **Actions → Release → Run workflow**.

[Contributing](CONTRIBUTING.md) · [Release process](docs/RELEASING.md) · [Security policy](SECURITY.md) · [Changelog](CHANGELOG.md)

## License and credits

Original WinCraft code is released under the [MIT License](LICENSE), maintained by **villetacore**. Third-party components retain their respective licenses; the complete OS image is not covered solely by MIT. Release source bundles include Buildroot legal information, upstream sources, patches, and vendored Cargo dependencies; see [THIRD_PARTY.md](THIRD_PARTY.md).

Built on the work of the Linux, Rust, Buildroot, Wine, labwc, wlroots, Wayland, Mesa, GTK, WebKitGTK, and DejaVu communities. WinCraft is an independent project and is not affiliated with Microsoft or the ReactOS project. Windows is a trademark of Microsoft.
