#!/usr/bin/env bash
# Builds WinCraft OS inside the builder chroot (see build/builder.sh):
#   build/builder.sh run build/build.sh            full build -> dist/wincraft-os.iso
#   build/builder.sh run build/build.sh <target>   any Buildroot target, e.g. linux-menuconfig
set -euo pipefail
cd "$(dirname "$0")/.."
SRC=$PWD
STATE=/work/state
BR_VERSION=2026.02.3
BR_SHA256=5a59e7501b0b4ec52c41f4bfa79412320e0b37eae5f719605a258e8d0c6fc7fb
BR=$STATE/buildroot-$BR_VERSION
OUT=$STATE/output
DEFCONFIG=wincraft_qemu_x86_64_defconfig
export WINCRAFT_VERSION
WINCRAFT_VERSION=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n1)
# The builder is an isolated root chroot; autotools otherwise refuses to configure.
export FORCE_UNSAFE_CONFIGURE=1
mkdir -p "$SRC/dist" "$STATE"
export WINCRAFT_SOURCE_DIR=$STATE/wincraft-src
export WINCRAFT_CARGO_TARGET_DIR=$STATE/cargo-target

if ! test -d "$BR"; then
    tarball=$STATE/buildroot-$BR_VERSION.tar.xz
    test -f "$tarball" || curl -fL --retry 3 -o "$tarball" "https://buildroot.org/downloads/buildroot-$BR_VERSION.tar.xz"
    echo "$BR_SHA256  $tarball" | sha256sum -c -
    tar xJf "$tarball" -C "$STATE"
fi

# Only the Rust workspace goes into the package source (no dist/, target/ or images).
mkdir -p "$WINCRAFT_SOURCE_DIR"
rsync -a --delete --include='/.cargo/***' --include='/LICENSE' --include='/Cargo.toml' --include='/Cargo.lock' \
    --include='/crates/***' --include='/system/***' --include='/apps/***' --exclude='*' \
    "$SRC/" "$WINCRAFT_SOURCE_DIR/"

make_br() { make -C "$BR" O="$OUT" BR2_EXTERNAL="$SRC/distro" BR2_JLEVEL="${WINCRAFT_JOBS:-0}" "$@"; }
stamp=$OUT/.wincraft-defconfig.sha256
current=$(sha256sum "$SRC/distro/configs/$DEFCONFIG" | cut -d' ' -f1)
if ! test -f "$OUT/.config" || test "$(cat "$stamp" 2>/dev/null)" != "$current"; then
    make_br "$DEFCONFIG"
    # Buildroot never removes files of packages dropped from the configuration (Xorg,
    # Openbox after ADR-018). Rebuild the target tree from the already compiled packages:
    # every package installs itself again, nothing is recompiled.
    if test -d "$OUT/target"; then
        rm -rf --one-file-system "$OUT/target"
        find "$OUT/build" -maxdepth 2 -name .stamp_target_installed -delete
    fi
    # Packages whose build depends on options of other packages must be rebuilt by hand
    # once: build/build.sh <package>-dirclean (e.g. mesa3d for the Wayland platform).
    echo "$current" > "$stamp"
fi

if test $# -gt 0; then
    make_br "$@"
    exit
fi
# Local package: re-sync our sources on every build (cargo itself is incremental).
make_br wincraft-dirclean
make_br
cp "$OUT/images/wincraft-os.iso" "$SRC/dist/wincraft-os.iso"
(cd "$SRC/dist" && sha256sum wincraft-os.iso > wincraft-os.iso.sha256)
echo "WINCRAFT_OS_BUILD_DONE"
