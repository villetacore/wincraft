#!/usr/bin/env bash
# Runs after all packages are installed into TARGET_DIR ($1), before the squashfs image.
set -euo pipefail
TARGET=$1
BOARD=$(dirname "$0")
PROJECT=$(cd "$BOARD/../../.." && pwd)

cat > "$TARGET/etc/os-release" <<EOF
NAME="WinCraft OS"
ID=wincraft
PRETTY_NAME="WinCraft OS ${WINCRAFT_VERSION:-0.4.0} (developer preview)"
VERSION_ID=${WINCRAFT_VERSION:-0.4.0}
HOME_URL="https://github.com/villetacore/wincraft"
EOF

# Windows test fixtures (built on the Windows host by build/fixtures.ps1).
mkdir -p "$TARGET/apps"
for exe in win32-demo.exe win32-demo-x86.exe; do
    if test -f "$PROJECT/dist/$exe"; then install -m 0644 "$PROJECT/dist/$exe" "$TARGET/apps/$exe"; fi
done

# Known folders, as Wine links them into C:\users\user.
for d in Desktop Documents Downloads Music Pictures Videos; do
    mkdir -p "$TARGET/home/user/$d"
done
install -m 0644 "$PROJECT/os/README-guest.txt" "$TARGET/home/user/Desktop/Welcome.txt"

# Nothing in the image may start a login shell on the console by itself.
rm -f "$TARGET/etc/init.d/S"*getty* 2>/dev/null || true
