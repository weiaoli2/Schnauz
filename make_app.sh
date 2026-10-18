#!/bin/bash
# Assemble our own app from an existing Cargo build.
set -euo pipefail
cd "$(dirname "$0")"
if [ -d /Applications/Xcode.app/Contents/Developer ]; then
    export DEVELOPER_DIR="${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}"
fi
DEST_APP="$PWD/Schnauz.app"
PROFILE="${SCHNAUZ_BUILD_PROFILE:-release}"
TARGET="${CARGO_TARGET_DIR:-target}"
BINARY="$TARGET/$PROFILE/schnauz"
if [ ! -f "$BINARY" ]; then
    echo "Build the $PROFILE binary first with ./build.sh."
    exit 1
fi
VERSION=$(sed -nE 's/^version = "([^"]+)"/\1/p' Cargo.toml | head -1)
# Resolve the certificate before touching the existing app. Ad-hoc signing is
# opt-in because its changing cdhash invalidates macOS privacy permissions.
source ./build-signing.sh
PREVIOUS_SIGNER=""
if [ -d "$DEST_APP" ]; then
    PREVIOUS_SIGNER=$(codesign -dvv "$DEST_APP" 2>&1 | sed -n 's/^Authority=//p' | head -1) || true
fi
SIGNING_IDENTITIES=""
if [ "${SCHNAUZ_CODESIGN_IDENTITY:-}" != "-" ]; then
    SIGNING_IDENTITIES=$(security find-identity -v -p codesigning | sed -nE 's/^[[:space:]]*[0-9]+\) ([[:xdigit:]]{40}) "([^"]+)".*/\1|\2/p')
fi
SIGNING_IDENTITY=$(schnauz_signing_identity "${SCHNAUZ_CODESIGN_IDENTITY:-}" "$PREVIOUS_SIGNER" "$SIGNING_IDENTITIES")
if [ "$SIGNING_IDENTITY" = "-" ]; then
    echo "Temporary ad-hoc build: macOS permissions may need to be granted again."
else
    echo "Signing Schnauz with a stable code-signing certificate."
fi
# Build and verify in staging. A failed compile/sign must leave the existing app
# intact, including while the user still has a terminal running inside it.
mkdir -p .build-cache
STAGING_ROOT=$(mktemp -d "$PWD/.build-cache/app-stage.XXXXXX")
APP="$STAGING_ROOT/Schnauz.app"
cleanup_package() {
    # Preserve the previous app if final installation and rollback both failed.
    if [ -d "$STAGING_ROOT/previous/Schnauz.app" ] && [ ! -d "$DEST_APP" ]; then
        echo "Previous Schnauz app preserved at $STAGING_ROOT/previous/Schnauz.app" >&2
        return
    fi
    rm -rf "$STAGING_ROOT"
}
trap cleanup_package EXIT
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BINARY" "$APP/Contents/MacOS/schnauz"
cp AppIcon.icns "$APP/Contents/Resources/AppIcon.icns"
cp CREDITS.txt "$APP/Contents/Resources/Credits.txt"
mkdir -p "$APP/Contents/Resources/Licenses"
cp LICENSE "$APP/Contents/Resources/Licenses/Shuffle-MIT.txt"
cp vendor/gpui-terminal/LICENSE-MIT "$APP/Contents/Resources/Licenses/gpui-terminal-MIT.txt"
cp vendor/gpui-terminal/LICENSE-APACHE "$APP/Contents/Resources/Licenses/gpui-terminal-Apache-2.0.txt"
cp vendor/licenses/*.txt "$APP/Contents/Resources/Licenses/"
mkdir -p "$TARGET/swift-module-cache"
for helper in removebg cloudctl volumectl; do
    xcrun swiftc -module-cache-path "$TARGET/swift-module-cache" -O "$helper.swift" -o "$APP/Contents/MacOS/$helper"
    codesign --force --timestamp=none --sign "$SIGNING_IDENTITY" "$APP/Contents/MacOS/$helper"
done
xcrun swiftc -module-cache-path "$TARGET/swift-module-cache" -O native/nextcloud/Core.swift native/nextcloud/main.swift -o "$APP/Contents/MacOS/nextcloudctl"
codesign --force --timestamp=none --sign "$SIGNING_IDENTITY" "$APP/Contents/MacOS/nextcloudctl"
xcrun swiftc -module-cache-path "$TARGET/swift-module-cache" -O native/todoist/Core.swift native/todoist/main.swift -o "$APP/Contents/MacOS/todoistctl"
codesign --force --timestamp=none --sign "$SIGNING_IDENTITY" "$APP/Contents/MacOS/todoistctl"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
    <key>CFBundleName</key><string>Schnauz</string>
    <key>CFBundleDisplayName</key><string>Schnauz</string>
    <key>CFBundleExecutable</key><string>schnauz</string>
    <key>CFBundleIdentifier</key><string>local.schnauz.app</string>
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
codesign --force --timestamp=none --sign "$SIGNING_IDENTITY" --identifier local.schnauz.app "$APP"
codesign --verify --deep --strict "$APP"
if [ -d "$DEST_APP" ]; then
    mkdir -p "$STAGING_ROOT/previous"
    mv "$DEST_APP" "$STAGING_ROOT/previous/Schnauz.app"
fi
if ! mv "$APP" "$DEST_APP"; then
    if [ -d "$STAGING_ROOT/previous/Schnauz.app" ]; then
        mv "$STAGING_ROOT/previous/Schnauz.app" "$DEST_APP"
    fi
    exit 1
fi
echo "Built $DEST_APP"
