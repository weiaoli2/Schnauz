#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
APP="Schnauz.app"
if [ ! -d "$APP" ]; then
    echo 'Run ./build.sh first.'
    exit 1
fi
STAGING=$(mktemp -d "${TMPDIR:-/tmp}/schnauz-dmg.XXXXXX")
trap 'rm -rf "$STAGING"' EXIT
cp -R "$APP" "$STAGING/"
ln -s /Applications "$STAGING/Applications"
cp LICENSE "$STAGING/Shuffle-LICENSE.txt"
hdiutil create -volname "Schnauz" -srcfolder "$STAGING" -ov -format UDZO "Schnauz.dmg"
