# Third-party software

The root MIT license applies to original WinCraft code. It does not relicense Linux, Wine, Buildroot, Rust dependencies, fonts, firmware, or other software included in an OS image. Buildroot-derived recipes and patches retain their upstream terms, including the GPL-2.0 Wine recipe attribution in `distro/package/wincraft-wine/wincraft-wine.mk`.

For each release, `packages.csv` describes Buildroot target packages. The `wincraft-sources-*.tar.xz` archive (or its numbered parts) contains:

- Buildroot's `legal-info` directory: package sources, patches, license texts, configuration, target/host manifests, and collection warnings;
- the verified Buildroot source archive;
- WinCraft sources and build recipes;
- vendored Cargo dependencies, with their original license files, plus a vendor configuration and `Cargo.lock`.

The upstream collector has limitations; its `README` warnings must be reviewed when changing packages or preparing a stable release. Rust dependency metadata is available with `cargo metadata --locked`. The Cargo vendor tree complements the Buildroot manifest; it is not represented by that manifest.

To reconstruct a split archive, concatenate `.part-000`, `.part-001`, and subsequent parts in numeric order, then extract the resulting `.tar.xz`. Verify the individual uploaded parts against `SHA256SUMS` before concatenating. The source bundle is tied to the binary release and should remain available alongside it.

References: [Buildroot legal information](https://buildroot.org/downloads/manual/manual.html#legal-info), [Linux COPYING](https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/COPYING), [Wine license](https://gitlab.winehq.org/wine/wine/-/blob/master/LICENSE).
