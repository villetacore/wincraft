#!/usr/bin/env bash
# TRANSITIONAL Alpine pipeline (0.3); the real distribution is built by build/build.sh.
# Builds the guest rootfs in a fresh chroot (BUILD-01), then packs the developer
# initramfs and the live ISO. WINCRAFT_REUSE_ROOT=1 reuses the last chroot for
# quick iterations; release-like builds must not set it.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p .build dist
test "$(id -u)" = 0 || { echo 'Run inside WSL as root'; exit 1; }
# One build at a time: two builds sharing chroot mounts corrupt each other.
exec 9>/var/lib/wincraft-build.lock
flock -n 9 || { echo 'Another WinCraft build is running'; exit 1; }
if test "${WINCRAFT_REUSE_ROOT:-0}" = 1 && test -f .build/rootfs-path && test -d "$(cat .build/rootfs-path)"; then
    ROOT=$(cat .build/rootfs-path)
    [[ "$ROOT" == /var/lib/wincraft-build.*/root ]] && test -d "$ROOT" || { echo 'Invalid build root'; exit 1; }
    echo "Reusing build root $ROOT"
else
    ROOT=$(mktemp -d /var/lib/wincraft-build.XXXXXXXX)/root
    mkdir -p "$ROOT"
    printf '%s\n' "$ROOT" > .build/rootfs-path
    echo "Fresh build root $ROOT"
fi
ARCHIVE=.build/alpine-minirootfs-3.24.2-x86_64.tar.gz
if ! test -f "$ROOT/etc/alpine-release"; then
    test -f "$ARCHIVE" || curl --fail --location --retry 3 -o "$ARCHIVE" https://dl-cdn.alpinelinux.org/alpine/v3.24/releases/x86_64/alpine-minirootfs-3.24.2-x86_64.tar.gz
    echo "c5ca053cfe1d85c5b96dff8b9bc57045f7f184a30ffb6b65776409ca90388677  $ARCHIVE" | sha256sum -c -
    tar xzf "$ARCHIVE" -C "$ROOT"
fi
cp /etc/resolv.conf "$ROOT/etc/resolv.conf"
mountpoint -q "$ROOT/proc" || mount -t proc proc "$ROOT/proc"
mountpoint -q "$ROOT/dev" || mount --bind /dev "$ROOT/dev"
cleanup() { umount "$ROOT/proc" || true; umount "$ROOT/dev" || true; }
trap cleanup EXIT
# Wayland session as in the real distribution (ADR-018): seatd + labwc + Xwayland, foot.
chroot "$ROOT" /sbin/apk add linux-firmware-none linux-lts busybox eudev seatd labwc xwayland mesa-dri-gallium mesa-egl mesa-gbm mesa-gl libxkbcommon wayland-libs-client wayland-libs-egl font-dejavu foot wlr-randr wine \
    webkit2gtk-6.0 glib-networking ca-certificates gst-plugins-good gst-libav
chroot "$ROOT" /sbin/apk add --virtual .build-deps cargo rust build-base pkgconf libxkbcommon-dev wayland-dev mesa-dev gtk4.0-dev webkit2gtk-6.0-dev
mkdir -p "$ROOT/build"
rm -rf "$ROOT/build/src" "$ROOT/build/crates" "$ROOT/build/system" "$ROOT/build/apps"
cp -r Cargo.toml Cargo.lock crates system apps "$ROOT/build/"
BINS="wincraft-init winlogon explorer notepad taskmgr start browser"
chroot "$ROOT" /bin/sh -c "cd /build && cargo test --locked && cargo build --release --locked --workspace $(printf -- '--bin %s ' $BINS)"
for b in $BINS; do install -m755 "$ROOT/build/target/release/$b" "$ROOT/usr/bin/$b"; done
rm -f "$ROOT/usr/bin/wincraft"
install -m755 "$ROOT/build/target/release/wincraft-init" "$ROOT/init"
cp "$ROOT/boot/vmlinuz-lts" dist/vmlinuz
chroot "$ROOT" /sbin/apk del .build-deps
chroot "$ROOT" /sbin/apk info -v | sort > dist/packages.txt
rm -f "$ROOT/etc/resolv.conf"
cleanup
trap - EXIT
bash scripts/install-os-files.sh "$ROOT"
if test "${WINCRAFT_SKIP_INITRAMFS:-0}" != 1; then bash scripts/pack-os.sh; fi
bash scripts/make-iso.sh
