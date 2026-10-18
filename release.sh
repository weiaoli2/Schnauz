#!/bin/bash
# Build both Mac release installers locally; publishing is a separate action.
set -euo pipefail
cd "$(dirname "$0")"
source ./dev-env.sh
VERSION=$(sed -nE 's/^version = "([^"]+)"/\1/p' Cargo.toml | head -1)
OUTPUT="${SCHNAUZ_RELEASE_OUTPUT:-$PWD/dist/$VERSION}"
mkdir -p "$OUTPUT"
python3 scripts/generate_notices.py --check
for target in aarch64-apple-darwin x86_64-apple-darwin; do
    if ! rustup target list --installed | grep -qx "$target"; then
        echo "Install the missing Rust target: rustup target add $target" >&2
        exit 1
    fi
done
for entry in aarch64-apple-darwin:arm64 x86_64-apple-darwin:x86_64; do
    target="${entry%%:*}"
    arch="${entry##*:}"
    export SCHNAUZ_TARGET="$target"
    export SCHNAUZ_APP_OUTPUT="$OUTPUT/$arch/Schnauz.app"
    export SCHNAUZ_DMG_OUTPUT="$OUTPUT/Schnauz-$VERSION-macos-$arch.dmg"
    ./build.sh
    ./make_dmg.sh
done
python3 - "$OUTPUT/Schnauz-$VERSION-MPL-sources.zip" <<'PY'
from pathlib import Path
import sys,zipfile
base=Path('vendor/mpl-sources')
with zipfile.ZipFile(sys.argv[1],'w',compression=zipfile.ZIP_DEFLATED,strict_timestamps=False) as archive:
    for path in sorted(base.rglob('*')):
        if path.is_file():archive.write(path,Path('MPL-sources')/path.relative_to(base))
PY
(
    cd "$OUTPUT"
    shasum -a 256 "Schnauz-$VERSION-macos-arm64.dmg" "Schnauz-$VERSION-macos-x86_64.dmg" "Schnauz-$VERSION-MPL-sources.zip" > SHA256SUMS.txt
    shasum -a 256 -c SHA256SUMS.txt
)
echo "Release artifacts ready in $OUTPUT. Nothing has been published."
