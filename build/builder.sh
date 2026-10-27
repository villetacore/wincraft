#!/usr/bin/env bash
# Isolated build host for the WinCraft distribution.
# Creates /var/lib/wincraft/builder (Ubuntu base chroot) with the host tools Buildroot
# needs, GRUB/xorriso for ISO mastering and rustup for WinCraft's Rust components.
# Nothing is installed into the WSL distribution itself.
#
#   builder.sh setup              create/update the chroot
#   builder.sh run <command...>   run a command inside it (project at /work/src)
set -euo pipefail
cd "$(dirname "$0")/.."
PROJECT=$PWD
test "$(id -u)" = 0 || { echo 'Run inside WSL as root'; exit 1; }
BASE=/var/lib/wincraft
mkdir -p "$BASE"
ROOT=$BASE/builder
UBUNTU=ubuntu-base-24.04.5-base-amd64.tar.gz
UBUNTU_URL=https://cdimage.ubuntu.com/ubuntu-base/releases/24.04/release
mkdir -p "$BASE/cache" "$BASE/work"

mounts() {
    mkdir -p "$ROOT/work/src" "$ROOT/work/state"
    mountpoint -q "$ROOT/proc" || mount -t proc proc "$ROOT/proc"
    mountpoint -q "$ROOT/sys" || mount -t sysfs sysfs "$ROOT/sys"
    # WSL mounts are shared: without --make-rslave/--make-private an unmount inside the
    # chroot propagates back and tears down the host's /dev/pts.
    if ! mountpoint -q "$ROOT/dev"; then
        mount --rbind /dev "$ROOT/dev"
        mount --make-rslave "$ROOT/dev"
    fi
    if ! mountpoint -q "$ROOT/work/src"; then
        mount --bind "$PROJECT" "$ROOT/work/src"
        mount --make-private "$ROOT/work/src"
    fi
    # Build trees stay on the Linux filesystem: fast and case-sensitive.
    if ! mountpoint -q "$ROOT/work/state"; then
        mount --bind "$BASE/work" "$ROOT/work/state"
        mount --make-private "$ROOT/work/state"
    fi
    cp /etc/resolv.conf "$ROOT/etc/resolv.conf"
}
unmounts() {
    for m in work/state work/src sys proc; do
        if mountpoint -q "$ROOT/$m"; then umount "$ROOT/$m" || umount -l "$ROOT/$m"; fi
    done
    if mountpoint -q "$ROOT/dev"; then umount -R "$ROOT/dev" || umount -lR "$ROOT/dev"; fi
}

setup() {
    if ! test -f "$ROOT/etc/os-release"; then
        local tarball=$BASE/cache/$UBUNTU
        if ! test -f "$tarball"; then
            curl --fail --location --retry 3 -o "$tarball.part" "$UBUNTU_URL/$UBUNTU"
            mv "$tarball.part" "$tarball"
        fi
        curl --fail --silent --location "$UBUNTU_URL/SHA256SUMS" | grep " \*\?$UBUNTU\$" |
            sed "s#[ *]*$UBUNTU\$#  $tarball#" | sha256sum -c -
        mkdir -p "$ROOT"
        tar xzf "$tarball" -C "$ROOT"
    fi
    mounts
    trap unmounts EXIT
    chroot "$ROOT" /usr/bin/env -i PATH=/usr/sbin:/usr/bin:/sbin:/bin DEBIAN_FRONTEND=noninteractive HOME=/root bash -euc '
        apt-get update
        apt-get install -y --no-install-recommends \
            build-essential gcc g++ make patch perl python3 python3-setuptools rsync bc cpio unzip file \
            wget curl ca-certificates git xz-utils bzip2 gzip zstd lzip which findutils diffutils sed gawk \
            libncurses-dev pkg-config flex bison gettext texinfo \
            grub-efi-amd64-bin grub-pc-bin grub-common xorriso mtools dosfstools squashfs-tools e2fsprogs \
            gcc-mingw-w64-i686 gcc-mingw-w64-x86-64
        if ! test -x /root/.cargo/bin/cargo; then
            curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable
        fi
        /root/.cargo/bin/rustup target add x86_64-unknown-linux-gnu x86_64-unknown-linux-musl
    '
    echo 'WINCRAFT_BUILDER_READY'
}

run() {
    test -f "$ROOT/etc/os-release" || { echo 'Builder missing: run builder.sh setup'; exit 1; }
    mounts
    trap unmounts EXIT
    chroot "$ROOT" /usr/bin/env -i HOME=/root TERM=dumb LANG=C.UTF-8 \
        PATH=/root/.cargo/bin:/usr/sbin:/usr/bin:/sbin:/bin \
        WINCRAFT_JOBS="${WINCRAFT_JOBS:-$(nproc)}" \
        WINCRAFT_RELEASE_TAG="${WINCRAFT_RELEASE_TAG:-}" \
        bash -c 'cd /work/src && exec "$@"' wincraft "$@"
}

# Only one user of the builder at a time; parallel builds share mounts and trees.
exec 9>"$BASE/builder.lock"
flock -n 9 || { echo 'Another WinCraft build is running'; exit 1; }
case "${1:-}" in
    setup) setup ;;
    run) shift; run "$@" ;;
    *) echo "usage: $0 setup | run <command...>"; exit 2 ;;
esac
