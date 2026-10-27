#!/usr/bin/env bash
# Developer direct-kernel boot: the whole rootfs as one initramfs (dist/initramfs.cpio.gz).
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT=$(cat .build/rootfs-path)
[[ "$ROOT" == /var/lib/wincraft-build.*/root ]] && test -d "$ROOT" || { echo 'Invalid build root'; exit 1; }
if mountpoint -q "$ROOT/proc" || mountpoint -q "$ROOT/dev"; then echo 'Unmount build chroot before packaging'; exit 1; fi
bash scripts/install-os-files.sh "$ROOT"
test -e "$ROOT/dev/console" || mknod -m 600 "$ROOT/dev/console" c 5 1
test -e "$ROOT/dev/null" || mknod -m 666 "$ROOT/dev/null" c 1 3
(cd "$ROOT" && find . -path ./build -prune -o -path ./boot -prune -o -path ./root/.cargo -prune -o -path ./lib/firmware -prune -o -path ./usr/lib/firmware -prune -o -path ./var/cache/apk -prune -o -print0 | "$ROOT/lib/ld-musl-x86_64.so.1" "$ROOT/bin/busybox" cpio -0 -o -H newc) | gzip -1 > dist/initramfs.cpio.gz
sha256sum dist/vmlinuz dist/initramfs.cpio.gz > dist/SHA256SUMS
echo 'WINCRAFT_IMAGE_READY'
