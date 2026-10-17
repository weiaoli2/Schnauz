#!/bin/bash
# Assemble our own app from an existing Cargo build.
set -euo pipefail
cd "$(dirname "$0")"
if [ -d /Applications/Xcode.app/Contents/Developer ]; then
    export DEVELOPER_DIR="${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}"
fi
APP="Shuffle Studio.app"
PROFILE="${STUDIO_BUILD_PROFILE:-release}"
TARGET="${CARGO_TARGET_DIR:-target}"
BINARY="$TARGET/$PROFILE/shuffle-studio"
if [ ! -f "$BINARY" ]; then
    echo "Build the $PROFILE binary first with ./build.sh."
    exit 1
fi
VERSION=$(sed -nE 's/^version = "([^"]+)"/\1/p' Cargo.toml | head -1)
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BINARY" "$APP/Contents/MacOS/shuffle-studio"
cp AppIcon.icns "$APP/Contents/Resources/AppIcon.icns"
for helper in removebg cloudctl; do
    xcrun swiftc -O "$helper.swift" -o "$APP/Contents/MacOS/$helper"
    codesign --force --sign - "$APP/Contents/MacOS/$helper"
done
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
    <key>CFBundleName</key><string>Shuffle Studio</string>
    <key>CFBundleDisplayName</key><string>Shuffle Studio</string>
    <key>CFBundleExecutable</key><string>shuffle-studio</string>
    <key>CFBundleIdentifier</key><string>local.shuffle-studio.app</string>
    <key>CFBundleIconFile</key><string>AppIcon</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$VERSION</string>
    <key>CFBundleVersion</key><string>$VERSION</string>
    <key>LSMinimumSystemVersion</key><string>14.0</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>LSApplicationCategoryType</key><string>public.app-category.productivity</string>
    <key>NSHumanReadableCopyright</key><string>Based on Shuffle. Original code © Jaime Guzman, MIT license.</string>
</dict></plist>
PLIST
codesign --force --sign - --identifier local.shuffle-studio.app "$APP"
codesign --verify --deep --strict "$APP"
echo "Built $PWD/$APP"
