#!/usr/bin/env bash
# Run inside the isolated builder: builder.sh run bash build/release.sh
set -euo pipefail
cd "$(dirname "$0")/.."
export FORCE_UNSAFE_CONFIGURE=1
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n1)
ref=${WINCRAFT_RELEASE_TAG:-v$version}
[[ "$ref" =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-[A-Za-z0-9.-]+)?$ ]] || { echo 'Invalid release tag'; exit 1; }
[[ "${ref#v}" == "$version" || "${ref#v}" == "$version"-* ]] || { echo 'Tag does not match Cargo.toml'; exit 1; }
mkdir -p dist
# Build both PE test programs on Linux, with reproducible remapped Rust paths.
rustup target add x86_64-pc-windows-gnu i686-pc-windows-gnu
rustc --edition 2021 -O --remap-path-prefix "$PWD=/wincraft" --target x86_64-pc-windows-gnu tests/fixtures/win32-demo.rs -o dist/win32-demo.exe
rustc --edition 2021 -O --remap-path-prefix "$PWD=/wincraft" --target i686-pc-windows-gnu tests/fixtures/win32-demo.rs -o dist/win32-demo-x86.exe
bash build/build.sh
# Collect exact upstream sources, patches, configs and notices before remastering.
bash build/build.sh legal-info
bash build/build.sh target-post-image
cp /work/state/output/images/wincraft-os.iso dist/wincraft-os.iso
(cd dist && sha256sum wincraft-os.iso > wincraft-os.iso.sha256)
release="$PWD/dist/release"
rm -rf "$release"
mkdir -p "$release"
cp dist/wincraft-os.iso "$release/wincraft-os-$ref-x86_64.iso"
cp LICENSE "$release/LICENSE.txt"
python3 - "$release/wincraft-launcher.zip" <<'PYZIP'
import pathlib
import sys
import zipfile
with zipfile.ZipFile(sys.argv[1], "w", zipfile.ZIP_DEFLATED) as z:
    for name in ["WinCraft.cmd", "scripts/run-iso.ps1", "README.md", "LICENSE"]:
        z.write(name, name)
    z.writestr("dist/PLACE-ISO-HERE.txt", "Place the release ISO here as wincraft-os.iso.\n")
PYZIP
cp /work/state/output/legal-info/manifest.csv "$release/packages.csv"
# Cargo dependencies are not covered by Buildroot's package collector.
staging=$(mktemp -d /work/state/release.XXXXXXXX)
trap 'rm -rf "$staging"' EXIT
cargo vendor --locked "$staging/vendor" | sed "s|$staging/vendor|vendor|g" > "$staging/vendor-config.toml"
cp Cargo.toml Cargo.lock LICENSE "$staging/"
cp -r .cargo apps crates system build distro scripts tests os docs "$staging/"
cp README.md README.ru.md THIRD_PARTY.md "$staging/"
source build/versions.env
cp "/work/state/buildroot-$BR_VERSION.tar.xz" "$staging/"
cp -r /work/state/output/legal-info "$staging/"
# Record tool versions; no account names, runner environment or private paths.
revision=$(git -c safe.directory="$PWD" rev-parse HEAD 2>/dev/null || cat SOURCE_REVISION 2>/dev/null || printf unknown)
printf '%s\n' "$revision" > "$staging/SOURCE_REVISION"
{ printf 'WinCraft %s\ncommit: %s\n' "$ref" "$revision"; rustc --version; cargo --version; } > "$release/build-info.txt"
XZ_OPT="-T2 -1" tar -C "$staging" -cJf "$release/wincraft-sources-$ref.tar.xz" .
# GitHub Release has a per-file size limit. Split large source archives, with
# ordered names; checksums cover precisely the files that are uploaded.
archive="$release/wincraft-sources-$ref.tar.xz"
if (( $(stat -c %s "$archive") > 1900000000 )); then
    split -b 1900000000 -d -a 3 "$archive" "$archive.part-"
    rm "$archive"
fi
find "$release" -type f -size +2000000000c -print -quit | grep -q . && { echo 'Release asset exceeds size limit'; exit 1; }
(cd "$release" && sha256sum ./* > SHA256SUMS)
echo 'WINCRAFT_RELEASE_READY'
