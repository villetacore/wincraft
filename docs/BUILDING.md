# Building and testing WinCraft

## Fast checks

Install a current stable Rust toolchain with rustfmt and clippy. The workspace declares Rust 1.85 as a minimum for original code; the locked dependency graph and complete OS are validated on current stable Rust, not an MSRV CI lane.

```sh
cargo fmt --all -- --check
cargo test --locked
cargo clippy --locked --all-targets
python3 scripts/check-repository.py
```

The default workspace members are the platform-independent libraries. Windows and macOS checks do not build the complete guest.

On Ubuntu 24.04 x86-64, install guest development libraries:

```sh
sudo apt-get update
sudo apt-get install -y libwayland-dev libxkbcommon-dev libegl1-mesa-dev libgtk-4-dev libwebkitgtk-6.0-dev
cargo test --workspace --locked
cargo check --workspace --all-targets --locked
```

## Full current distribution (Buildroot)

Use an x86-64 Linux machine with root access, `curl`, `xz-utils`, mount/chroot support, and internet access. Ubuntu 24.04 is the CI host. Plan for at least 100 GB free storage and 16 GB RAM for a comfortable clean WebKitGTK/LLVM build; actual usage depends on parallelism and package versions. Two jobs reduce peak memory usage. First builds can exceed the GitHub-hosted runner time or storage limit.

```sh
sudo bash build/builder.sh setup
sudo env WINCRAFT_JOBS=2 bash build/builder.sh run bash build/release.sh
```

The builder lives at `/var/lib/wincraft/builder`; build state and downloaded sources live at `/var/lib/wincraft/work`. The repository is bind-mounted at `/work/src`. Setup installs build dependencies **inside the chroot** and temporarily mounts `/proc`, `/sys`, and `/dev`. Only run these scripts on a trusted dedicated build host. Do not run concurrent builds using the same state directory.

Outputs: `dist/wincraft-os.iso` and `dist/release/`. The release script additionally builds x64/x86 PE fixtures, collects third-party sources and notices, vendors Cargo dependencies, and generates checksums. `WINCRAFT_RELEASE_TAG` may select a tag matching the Cargo workspace version, such as `v0.4.0-preview.1`.

For a faster local iteration without release packaging:

```sh
sudo bash build/builder.sh run bash build/build.sh
sudo bash build/builder.sh run bash build/build.sh linux-rebuild
```

On Windows, install Rust MSVC, Ubuntu under WSL, and QEMU. `Build-WinCraft.cmd` builds the PE fixtures and calls the same isolated Buildroot builder. Run `WinCraft.cmd` to boot the result. Do not build the complete Linux guest with a Windows target.

## VM verification

On Ubuntu, install `qemu-system-x86` and `ovmf`:

```sh
python3 scripts/smoke-iso.py dist/wincraft-os.iso
python3 scripts/smoke-iso.py dist/wincraft-os.iso --uefi
```

These boot the actual ISO in disposable, network-isolated VMs and require the default release-mode PID 1 and desktop readiness markers. They do not validate arbitrary applications, graphical correctness, persistence, or shutdown. Serial logs are written to `dist/smoke-*.log`.

The extended Windows acceptance harness is `Test-WinCraft.cmd` / `scripts/smoke-iso.ps1`; it checks more session and Wine behavior. Its debug path was originally written for the transitional Alpine image; verify it against the current Buildroot boot path before relying on its results. The default CI boot test uses the normal ISO entry and does not rely on debug injection.

## Persistent data disk

Without a data disk the live system stores changes in RAM. `scripts/create-data.sh` creates `dist/data.ext4` labelled `WINCRAFT_DATA`, without overwriting an existing disk. It requires Linux, `e2fsprogs`, and sufficient space. The Windows QEMU launcher attaches it automatically unless `-NoData` is passed. Back it up separately; never commit it.

## Legacy pipeline

`scripts/build-os.sh`, `scripts/build-os.ps1`, and the old initramfs scripts implement the transitional Alpine 0.3 pipeline. They are retained for reference and development. GitHub releases use `build/release.sh` and the Buildroot configuration in `distro/`, not the transitional image.
