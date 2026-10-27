# Releasing WinCraft

## Workflows

`CI` runs on branch pushes and pull requests: Rust formatting, Windows/Linux library tests and clippy, Linux guest workspace tests/checks, shell syntax, and repository hygiene.

`Release` runs on code/build pushes to `main`, `v*` tag pushes, and manual dispatch. Documentation-only branch pushes skip the expensive ISO build; tags always run it. It first runs CI, then builds a fresh ISO from source in the isolated Linux builder, collects sources/notices, and boots the ISO in BIOS and UEFI QEMU VMs. After successful checks, tag runs upload the verified files to a draft release and publish it as a **prerelease**. Branch and manual runs build downloadable Actions artifacts without creating a GitHub release. New branch pushes cancel superseded branch ISO builds; tagged releases are not cancelled.

Only the publication job has `contents: write`; build jobs use read-only permissions. Pull requests do not execute privileged ISO builds. No personal access token is needed: release publication uses GitHub's scoped workflow token. See [GitHub workflow permissions](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#permissions).

## Download an ISO from CI

Open **Actions → Release**, select a successful run, and download `wincraft-iso-<run number>` under **Artifacts**. Extract the archive to get `wincraft-os.iso`, its SHA-256 file, and the Windows launcher ZIP. Artifacts are kept for seven days; tagged Releases are the durable distribution channel. A failed build or boot check does not upload this ISO artifact.

To start a build without a code change, use **Actions → Release → Run workflow → main**. A normal code push to `main` starts the same pipeline automatically. For a published preview, push a new `v*` tag matching the Cargo version.

## Runner capacity

The default is `ubuntu-24.04`, two build jobs, and a 360-minute timeout. A clean LLVM/WebKitGTK/Wine/toolchain build can exceed hosted runner limits. For reliable repeated releases, set repository Actions variable `WINCRAFT_BUILD_RUNNER` to a JSON array of labels for a trusted x86-64 Linux runner, for example `["self-hosted","linux","x64","wincraft-builder"]`. Provision at least 100 GB free disk and adequate RAM; measure a clean build before setting limits. Increase the workflow timeout for a self-hosted runner if required. Never expose a privileged self-hosted builder to untrusted pull-request code.

There is no deployment server: CD here means publishing versioned ISO/source artifacts in GitHub Releases.

## Publish a preview

1. Update `workspace.package.version` in `Cargo.toml`, the Buildroot package version in `distro/package/wincraft/wincraft.mk`, guest fallback version strings, and `CHANGELOG.md` together. Update the lockfile if dependencies change.
2. Require a green CI run on the intended commit. Review `THIRD_PARTY.md`, package changes, and known issues. Confirm the extended guest behaviors relevant to the change.
3. Push a lightweight tag matching the Cargo version; a preview suffix is supported:

   ```sh
   git tag v0.4.0-preview.1
   git push origin v0.4.0-preview.1
   ```

4. Watch the Release workflow. Failed builds or boot checks must not be presented as working releases. Fix the problem, create a new preview tag, and preserve published tags.
5. Download and verify the uploaded assets; review the source bundle's `legal-info/README` warnings. The [Buildroot collector](https://buildroot.org/downloads/manual/manual.html#legal-info) is an aid and has documented limitations. A stable release additionally needs broader guest acceptance and review of all required source/notices, including Cargo packages.

The current workflow intentionally marks all releases as prereleases. A stable release requires an explicit policy change after the OS is ready.

## Assets

- `wincraft-os-<tag>-x86_64.iso`: bootable system image.
- `wincraft-launcher.zip`: Windows launch scripts and instructions; place the ISO at `dist/wincraft-os.iso` after extracting.
- `wincraft-sources-<tag>.tar.xz` or numbered `.part-*`: WinCraft/build recipes, Buildroot source archive, upstream legal information and source packages, vendored Cargo dependencies.
- `packages.csv`, `build-info.txt`, `LICENSE.txt`, `SHA256SUMS`.

The source bundle includes `vendor-config.toml`; merge its source replacement configuration into `.cargo/config.toml` in an extracted build tree to build the Rust workspace offline using `cargo build --offline --locked`. The complete Buildroot build has additional host and download requirements documented in its own source/manual. The ISO is not currently claimed to be bit-for-bit reproducible: timestamps and the generated media ID vary between builds.

If an upload fails, rerun the failed job to finish the draft. Existing tags must point to the same commit. A failed build must not be worked around by uploading a stale local ISO.

## Public identity and privacy

Public project identity is `villetacore`. New commits are GPG-signed with the selected project identity. Build outputs, VM data, IDE settings, environment files, keys, and logs are ignored. `scripts/check-repository.py` checks publishable files for personal email addresses, host paths, obvious tokens, binary artifacts (except validated, metadata-free PNGs in `docs/screenshots/`), and oversized files. It is a focused guard, not a complete secret scanner. Review new files before publication.

Existing commits retain their original metadata; changing local Git identity affects future commits. GitHub repository settings, account profiles, and any previously published history are separate from source file cleanup.
