#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
APP="${SCHNAUZ_APP_OUTPUT:-$PWD/Schnauz.app}"
OUTPUT="${SCHNAUZ_DMG_OUTPUT:-$PWD/Schnauz.dmg}"
if [ ! -d "$APP" ]; then
    echo 'Run ./build.sh first.' >&2
    exit 1
fi
python3 scripts/check_bundle_contents.py "$APP"
codesign --verify --deep --strict "$APP"
STAGING=$(mktemp -d "${TMPDIR:-/tmp}/schnauz-dmg.XXXXXX")
trap 'rm -rf "$STAGING"' EXIT
ditto "$APP" "$STAGING/Schnauz.app"
ln -s /Applications "$STAGING/Applications"
cp LICENSE "$STAGING/Shuffle-LICENSE.txt"
cp CREDITS.txt "$STAGING/Credits.txt"
cp docs/INSTALL.txt "$STAGING/Read Me First.txt"
mkdir -p "$(dirname "$OUTPUT")"
hdiutil create -volname "Schnauz" -srcfolder "$STAGING" -ov -format UDZO "$OUTPUT"
hdiutil verify "$OUTPUT"
