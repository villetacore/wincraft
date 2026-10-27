# Contributing to WinCraft

Bug reports, documentation improvements, and focused pull requests are welcome. Public project identity: **villetacore**. Use GitHub issues and pull requests for project communication; no personal contact details are needed.

## Before making changes

Read the [architecture](docs/12-wincraft-os.md) and [decisions](docs/10-decisions.md). Keep changes small enough to review and explain the user-visible behavior. For large architectural changes, open an issue first.

## Local checks

```sh
cargo fmt --all -- --check
cargo test --locked
cargo clippy --locked --all-targets
python3 scripts/check-repository.py
```

On Linux, install the dependencies listed in [BUILDING.md](docs/BUILDING.md) and also run `cargo test --workspace --locked` and `cargo check --workspace --all-targets --locked`. Kernel, compositor, boot, Wine, and image changes require a fresh ISO and VM verification. A library-only host test cannot validate the guest desktop.

Add tests for behavioral fixes where practical. Explain what you tested and any untested guest behavior in your pull request. Do not submit build outputs, data disks, downloaded applications, credentials, private host paths, or personal machine configuration.

Use a GitHub-provided noreply email if you want to keep your email private in commits. Do not copy code from Windows, ReactOS, or other projects without verifying licensing and preserving required attribution.

Unless explicitly stated otherwise, contributions to original WinCraft code are provided under the project's MIT license. Third-party code keeps its original license and attribution.
