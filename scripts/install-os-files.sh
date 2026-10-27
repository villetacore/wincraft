#!/usr/bin/env bash
# TRANSITIONAL (Alpine-based 0.3 pipeline, removed once distro/ passes smoke-iso):
# copies WinCraft configuration into a built Alpine rootfs.
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT=$1
[[ "$ROOT" == /var/lib/wincraft-build.*/root ]] && test -d "$ROOT" || { echo 'Invalid build root'; exit 1; }
# Same overlay as the real distribution: labwc configuration, theme and keys.
cp -a distro/board/qemu-x86_64/rootfs-overlay/. "$ROOT/"
rm -f "$ROOT/usr/bin/wincraft-session" "$ROOT/usr/bin/wincraft-user-session" "$ROOT/etc/X11/xorg.conf.d/10-wincraft.conf.orig"
if ! chroot "$ROOT" /bin/busybox id user >/dev/null 2>&1; then
    chroot "$ROOT" /bin/busybox adduser -D -h /home/user -s /bin/sh -u 1000 user
fi
# As in distro users.table: wlroots reopens the DRM device itself for its buffers
# (seatd only hands over the master fd), so the user needs the video group.
for group in video input audio; do
    chroot "$ROOT" /bin/busybox addgroup user "$group" 2>/dev/null || true
done
for d in Desktop Documents Downloads Music Pictures Videos; do
    install -d -o 1000 -g 1000 "$ROOT/home/user/$d"
done
rm -f "$ROOT/home/user/Welcome.txt"
install -m644 -o 1000 -g 1000 os/README-guest.txt "$ROOT/home/user/Desktop/Welcome.txt"
mkdir -p "$ROOT/apps"
for exe in dist/win32-demo.exe dist/win32-demo-x86.exe; do
    if test -f "$exe"; then install -m644 "$exe" "$ROOT/apps/"; fi
done
