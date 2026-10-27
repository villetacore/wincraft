#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
# Only a new dedicated image; never formats a physical device or an existing image.
test ! -e dist/data.ext4 || { echo 'dist/data.ext4 already exists; keeping it unchanged'; exit 0; }
rm -rf .build/data-seed
mkdir -p dist .build/data-seed/home/user .build/data-seed/apps
cp os/README-guest.txt .build/data-seed/home/user/Welcome.txt
for exe in dist/win32-demo.exe dist/win32-demo-x86.exe; do
    if test -f "$exe"; then cp "$exe" .build/data-seed/apps/; fi
done
if test -d apps; then cp -a apps/. .build/data-seed/apps/; fi
truncate -s 2G dist/data.ext4
# The guest finds this disk only by its label; other disks are never mounted automatically.
mkfs.ext4 -F -L WINCRAFT_DATA -d .build/data-seed dist/data.ext4
# The guest desktop account is UID/GID 1000; seed files must belong to it.
for path in /home/user /home/user/Welcome.txt; do
    debugfs -w -R "set_inode_field $path uid 1000" dist/data.ext4
    debugfs -w -R "set_inode_field $path gid 1000" dist/data.ext4
done
echo 'Created dist/data.ext4 (2 GiB sparse image)'
