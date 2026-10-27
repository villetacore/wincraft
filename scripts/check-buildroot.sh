#!/usr/bin/env bash
# Validate the complete defconfig, without building an OS or requiring root.
set -euo pipefail
cd "$(dirname "$0")/.."
project=$PWD
source build/versions.env
check_root=$(mktemp -d)
trap 'rm -rf "$check_root"' EXIT
curl --fail --location --retry 3 "https://buildroot.org/downloads/buildroot-$BR_VERSION.tar.xz" -o "$check_root/buildroot.tar.xz"
echo "$BR_SHA256  $check_root/buildroot.tar.xz" | sha256sum -c -
tar -xJf "$check_root/buildroot.tar.xz" -C "$check_root"
make -C "$check_root/buildroot-$BR_VERSION" O="$check_root/output" \
    BR2_EXTERNAL="$project/distro" wincraft_qemu_x86_64_defconfig
# Kconfig can silently discard requested packages when dependencies are missing.
python3 - "$check_root/output/.config" <<'PY'
import pathlib
import sys
wanted = pathlib.Path("distro/configs/wincraft_qemu_x86_64_defconfig").read_text().splitlines()
actual = set(pathlib.Path(sys.argv[1]).read_text().splitlines())
missing = [line for line in wanted if line.startswith("BR2_") and line.endswith("=y") and line not in actual]
if missing:
    raise SystemExit("Buildroot dropped requested options:\n" + "\n".join(missing))
print("All requested Buildroot options are enabled.")
PY
