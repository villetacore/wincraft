#!/usr/bin/env bash
# Live ISO (ISO-01/BOOT-02): GRUB for UEFI and BIOS, Linux + small early initramfs,
# read-only rootfs.squashfs with a RAM overlay. Output: dist/wincraft-live.iso
# Packaging tools live in a separate Alpine chroot so they never enter the OS image.
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT=$(cat .build/rootfs-path)
[[ "$ROOT" == /var/lib/wincraft-build.*/root ]] && test -d "$ROOT" || { echo 'Invalid build root'; exit 1; }
if mountpoint -q "$ROOT/proc" || mountpoint -q "$ROOT/dev"; then echo 'Unmount build chroot before packaging'; exit 1; fi
BUILDER=$(dirname "$ROOT")
TOOLS=$BUILDER/tools
WORK=$BUILDER/iso-work
VERSION=0.3.0
ARCHIVE=.build/alpine-minirootfs-3.24.2-x86_64.tar.gz

bash scripts/install-os-files.sh "$ROOT"
if ! test -f "$TOOLS/etc/alpine-release"; then
    mkdir -p "$TOOLS"
    echo "c5ca053cfe1d85c5b96dff8b9bc57045f7f184a30ffb6b65776409ca90388677  $ARCHIVE" | sha256sum -c -
    tar xzf "$ARCHIVE" -C "$TOOLS"
fi
cleanup() {
    for m in work/root work/out proc dev; do
        if mountpoint -q "$TOOLS/$m"; then umount "$TOOLS/$m" || true; fi
    done
}
cleanup
trap cleanup EXIT
rm -rf --one-file-system "$WORK"
mkdir -p "$WORK"/{early,iso/boot/grub,iso/wincraft,iso/licenses,out} "$TOOLS/work/root" "$TOOLS/work/out"
cp /etc/resolv.conf "$TOOLS/etc/resolv.conf"
mount -t proc proc "$TOOLS/proc"
mount --bind /dev "$TOOLS/dev"
mount --bind "$ROOT" "$TOOLS/work/root"
mount -o remount,bind,ro "$TOOLS/work/root"
mount --bind "$WORK" "$TOOLS/work/out"
chroot "$TOOLS" /sbin/apk add squashfs-tools grub grub-efi grub-bios xorriso mtools busybox-static

# Early initramfs: static busybox, the needed storage/filesystem modules, os/early-init.
KVER=$(ls "$ROOT/lib/modules")
test "$(echo "$KVER" | wc -l)" = 1 || { echo "Expected one kernel in rootfs: $KVER"; exit 1; }
EARLY=$WORK/early
mkdir -p "$EARLY"/{bin,dev,proc,sys,media,newroot,lib/modules}
install -m755 "$TOOLS/bin/busybox.static" "$EARLY/bin/busybox"
install -m755 os/early-init "$EARLY/init"
mknod -m 600 "$EARLY/dev/console" c 5 1
mknod -m 666 "$EARLY/dev/null" c 1 3
MODULES="loop squashfs overlay isofs nls_utf8 sr_mod ahci ata_piix virtio_pci virtio_blk virtio_scsi sd_mod xhci_pci ehci_pci usb_storage uas nvme"
for module in $MODULES; do
    chroot "$ROOT" /sbin/modprobe -S "$KVER" --show-depends "$module"
done | awk '$1 == "insmod" && !seen[$2]++ { print $2 }' | while read -r path; do
    name=$(basename "$path" .gz)
    gzip -dc "$ROOT$path" > "$EARLY/lib/modules/$name"
    echo "$name" >> "$EARLY/lib/modules/load.list"
done
(cd "$EARLY" && find . | sort | "$TOOLS/bin/busybox.static" cpio -o -H newc) | gzip -9 > "$WORK/iso/boot/initramfs.img"

cp "$ROOT/boot/vmlinuz-lts" "$WORK/iso/boot/vmlinuz"
chroot "$TOOLS" /usr/bin/mksquashfs /work/root /work/out/iso/wincraft/rootfs.squashfs \
    -noappend -comp zstd -quiet \
    -e build boot root/.cargo var/cache/apk lib/firmware usr/lib/firmware
BUILD_ID=$(date -u +%Y%m%dT%H%M%SZ)-$(head -c4 /dev/urandom | od -An -tx1 | tr -d ' \n')
echo "$BUILD_ID" > "$WORK/iso/wincraft/id"
(cd "$WORK/iso/wincraft" && sha256sum rootfs.squashfs > SHA256SUMS)
SQUASH_SHA=$(cut -d' ' -f1 "$WORK/iso/wincraft/SHA256SUMS")
cat > "$WORK/iso/wincraft/manifest.json" <<EOF
{
  "name": "WinCraft Unix",
  "version": "$VERSION",
  "build_id": "$BUILD_ID",
  "kernel": "$KVER",
  "rootfs": {"file": "rootfs.squashfs", "sha256": "$SQUASH_SHA", "size": $(stat -c %s "$WORK/iso/wincraft/rootfs.squashfs")},
  "debug_image": true
}
EOF
cp dist/packages.txt "$WORK/iso/licenses/packages.txt"
cat > "$WORK/iso/licenses/README.txt" <<'EOF'
WinCraft Unix live image. Linux, Alpine packages, Wine, Xorg, GRUB and the other
listed packages keep their own licenses; see packages.txt and the upstream projects.
This is a developer preview image; corresponding sources are available from Alpine.
EOF

entry() { # title, extra kernel arguments
    cat <<EOF
menuentry "$1" {
    linux /boot/vmlinuz wincraft.media=$BUILD_ID console=tty0 console=ttyS0,115200 fbcon=nodefer $2
    initrd /boot/initramfs.img
}
EOF
}
{
    echo 'set timeout=5'
    echo 'set default=0'
    echo 'serial --unit=0 --speed=115200'
    echo 'terminal_input console serial'
    echo 'terminal_output console serial'
    echo 'insmod all_video'
    entry "WinCraft Unix $VERSION Live" 'quiet loglevel=3'
    entry "WinCraft Unix $VERSION Live - verify media first" 'wincraft.verify loglevel=4'
    entry "WinCraft Unix $VERSION Debug - serial root shell" 'wincraft.debug loglevel=4'
    entry "WinCraft Unix $VERSION Debug - 2 GB RAM limit" 'wincraft.debug loglevel=4 mem=2G'
} > "$WORK/iso/boot/grub/grub.cfg"

chroot "$TOOLS" /usr/bin/grub-mkrescue -o /work/out/out/wincraft-live.iso /work/out/iso -- -volid WINCRAFT_LIVE
cleanup
trap - EXIT
rm -f "$TOOLS/etc/resolv.conf"
cp "$WORK/out/wincraft-live.iso" dist/wincraft-live.iso
(cd dist && sha256sum wincraft-live.iso > wincraft-live.iso.sha256)
ls -l dist/wincraft-live.iso
echo "WINCRAFT_ISO_READY build=$BUILD_ID"
