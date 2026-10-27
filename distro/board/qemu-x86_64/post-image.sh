#!/usr/bin/env bash
# Masters the live ISO from BINARIES_DIR ($1): GRUB (UEFI + BIOS), kernel,
# initramfs with wincraft-boot, rootfs.squashfs, manifest.
set -euo pipefail
IMAGES=$1
BOARD=$(dirname "$0")
VERSION=${WINCRAFT_VERSION:-0.4.0}
WORK=$IMAGES/iso-work
rm -rf --one-file-system "$WORK"
mkdir -p "$WORK"/{initramfs/dev,initramfs/proc,initramfs/sys,initramfs/media,initramfs/newroot,iso/boot/grub,iso/wincraft,iso/licenses}

# Early stage: a single static Rust binary; every driver it needs is built into the kernel.
install -m 0755 "$IMAGES/wincraft-boot" "$WORK/initramfs/init"
mknod -m 600 "$WORK/initramfs/dev/console" c 5 1
mknod -m 666 "$WORK/initramfs/dev/null" c 1 3
(cd "$WORK/initramfs" && find . | LC_ALL=C sort | cpio --quiet -o -H newc --reproducible) |
    gzip -9n > "$WORK/iso/boot/initramfs.img"

cp "$IMAGES/bzImage" "$WORK/iso/boot/vmlinuz"
cp "$IMAGES/rootfs.squashfs" "$WORK/iso/wincraft/rootfs.squashfs"
BUILD_ID=$(date -u +%Y%m%dT%H%M%SZ)-$(head -c4 /dev/urandom | od -An -tx1 | tr -d ' \n')
echo "$BUILD_ID" > "$WORK/iso/wincraft/id"
(cd "$WORK/iso/wincraft" && sha256sum rootfs.squashfs > SHA256SUMS)
cat > "$WORK/iso/wincraft/manifest.json" <<EOF
{
  "name": "WinCraft OS",
  "version": "$VERSION",
  "build_id": "$BUILD_ID",
  "kernel": "$(file -b "$IMAGES/bzImage" | grep -o 'version [^ ]*' | cut -d' ' -f2)",
  "rootfs": {"file": "rootfs.squashfs", "sha256": "$(cut -d' ' -f1 "$WORK/iso/wincraft/SHA256SUMS")"},
  "debug_image": true
}
EOF
if test -f "$BASE_DIR/legal-info/manifest.csv"; then
    cp "$BASE_DIR/legal-info/manifest.csv" "$WORK/iso/licenses/"
fi

sed -e "s/@BUILD_ID@/$BUILD_ID/g" -e "s/@VERSION@/$VERSION/g" "$BOARD/grub.cfg" > "$WORK/iso/boot/grub/grub.cfg"
grub-mkrescue -o "$IMAGES/wincraft-os.iso" "$WORK/iso" -- -volid WINCRAFT_OS
echo "WINCRAFT_ISO_READY build=$BUILD_ID"
