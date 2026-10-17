#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p assets/Getter.iconset
for points in 16 32 128 256 512; do
    sips -z "$points" "$points" assets/Getter-icon.png --out "assets/Getter.iconset/icon_${points}x${points}.png" >/dev/null
    pixels=$((points * 2))
    sips -z "$pixels" "$pixels" assets/Getter-icon.png --out "assets/Getter.iconset/icon_${points}x${points}@2x.png" >/dev/null
done
iconutil --convert icns assets/Getter.iconset --output AppIcon.icns
