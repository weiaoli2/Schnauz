#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
APP="Getter.app"
if [ ! -d "$APP" ]; then
    echo 'Run ./build.sh first.'
    exit 1
fi
STAGING=$(mktemp -d "${TMPDIR:-/tmp}/getter-dmg.XXXXXX")
trap 'rm -rf "$STAGING"' EXIT
cp -R "$APP" "$STAGING/"
ln -s /Applications "$STAGING/Applications"
cp LICENSE "$STAGING/Shuffle-LICENSE.txt"
hdiutil create -volname "Getter" -srcfolder "$STAGING" -ov -format UDZO "Getter.dmg"
