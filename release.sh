#!/bin/bash
# Build one universal Mac installer; legacy names contain identical bytes.
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
    ./build.sh
done
export SCHNAUZ_APP_OUTPUT="$OUTPUT/universal/Schnauz.app"
export SCHNAUZ_DMG_OUTPUT="$OUTPUT/Schnauz-$VERSION-macos-universal.dmg"
./make_universal.sh "$OUTPUT/arm64/Schnauz.app" "$OUTPUT/x86_64/Schnauz.app" "$SCHNAUZ_APP_OUTPUT"
./make_dmg.sh
# Pre-V1 update checkers require these exact names. Both are the universal DMG,
# not architecture-specific packages; retain them while those clients are supported.
cp "$SCHNAUZ_DMG_OUTPUT" "$OUTPUT/Schnauz-$VERSION-macos-arm64.dmg"
cp "$SCHNAUZ_DMG_OUTPUT" "$OUTPUT/Schnauz-$VERSION-macos-x86_64.dmg"
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
    shasum -a 256 "Schnauz-$VERSION-macos-universal.dmg" "Schnauz-$VERSION-macos-arm64.dmg" "Schnauz-$VERSION-macos-x86_64.dmg" "Schnauz-$VERSION-MPL-sources.zip" > SHA256SUMS.txt
    shasum -a 256 -c SHA256SUMS.txt
)
echo "Release artifacts ready in $OUTPUT. Nothing has been published."
