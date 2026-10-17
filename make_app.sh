#!/bin/bash
# Assemble our own app from an existing Cargo build.
set -euo pipefail
cd "$(dirname "$0")"
if [ -d /Applications/Xcode.app/Contents/Developer ]; then
    export DEVELOPER_DIR="${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}"
fi
APP="Getter.app"
PROFILE="${STUDIO_BUILD_PROFILE:-release}"
TARGET="${CARGO_TARGET_DIR:-target}"
BINARY="$TARGET/$PROFILE/getter"
if [ ! -f "$BINARY" ]; then
    echo "Build the $PROFILE binary first with ./build.sh."
    exit 1
fi
VERSION=$(sed -nE 's/^version = "([^"]+)"/\1/p' Cargo.toml | head -1)
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BINARY" "$APP/Contents/MacOS/getter"
cp AppIcon.icns "$APP/Contents/Resources/AppIcon.icns"
mkdir -p "$APP/Contents/Resources/Licenses"
cp LICENSE "$APP/Contents/Resources/Licenses/Shuffle-MIT.txt"
cp vendor/gpui-terminal/LICENSE-MIT "$APP/Contents/Resources/Licenses/gpui-terminal-MIT.txt"
cp vendor/gpui-terminal/LICENSE-APACHE "$APP/Contents/Resources/Licenses/gpui-terminal-Apache-2.0.txt"
cp vendor/licenses/*.txt "$APP/Contents/Resources/Licenses/"
mkdir -p "$TARGET/swift-module-cache"
for helper in removebg cloudctl; do
    xcrun swiftc -module-cache-path "$TARGET/swift-module-cache" -O "$helper.swift" -o "$APP/Contents/MacOS/$helper"
    codesign --force --sign - "$APP/Contents/MacOS/$helper"
done
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
    <key>CFBundleName</key><string>Getter</string>
    <key>CFBundleDisplayName</key><string>Getter</string>
    <key>CFBundleExecutable</key><string>getter</string>
    <key>CFBundleIdentifier</key><string>local.getter.app</string>
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
codesign --force --sign - --identifier local.getter.app "$APP"
codesign --verify --deep --strict "$APP"
echo "Built $PWD/$APP"
