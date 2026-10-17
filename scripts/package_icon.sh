#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p assets/Schnauz.iconset
for points in 16 32 128 256 512; do
    sips -z "$points" "$points" assets/Schnauz-icon.png --out "assets/Schnauz.iconset/icon_${points}x${points}.png" >/dev/null
    pixels=$((points * 2))
    sips -z "$pixels" "$pixels" assets/Schnauz-icon.png --out "assets/Schnauz.iconset/icon_${points}x${points}@2x.png" >/dev/null
done
iconutil --convert icns assets/Schnauz.iconset --output AppIcon.icns
