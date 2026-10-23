#!/bin/bash
# Assemble our own app from an existing Cargo build.
set -euo pipefail
cd "$(dirname "$0")"
source ./dev-env.sh
DEST_APP="${SCHNAUZ_APP_OUTPUT:-$PWD/Schnauz.app}"
PROFILE="${SCHNAUZ_BUILD_PROFILE:-release}"
TARGET="${CARGO_TARGET_DIR:-target}"
RUST_TARGET="${SCHNAUZ_TARGET:-$(rustc -vV | sed -n 's/^host: //p')}"
case "$RUST_TARGET" in
    aarch64-apple-darwin) APP_ARCH=arm64 ;;
    x86_64-apple-darwin) APP_ARCH=x86_64 ;;
    *) echo "Unsupported Mac target: $RUST_TARGET" >&2; exit 1 ;;
esac
BINARY_DIR="$TARGET"
if [ -n "${SCHNAUZ_TARGET:-}" ]; then
    BINARY_DIR="$TARGET/$SCHNAUZ_TARGET"
fi
BINARY="$BINARY_DIR/$PROFILE/schnauz"
if [ ! -f "$BINARY" ]; then
    echo "Build the $PROFILE binary first with ./build.sh."
    exit 1
fi
VERSION=$(sed -nE 's/^version = "([^"]+)"/\1/p' Cargo.toml | head -1)
BUNDLE_VERSION="${SCHNAUZ_BUNDLE_VERSION:-${VERSION%%-*}}"
if [[ ! "$BUNDLE_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Bundle version must contain three numeric components." >&2
    exit 1
fi
if [ "$(lipo -archs "$BINARY")" != "$APP_ARCH" ]; then
    echo "Built binary does not match $APP_ARCH." >&2
    exit 1
fi
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
    # Installed, pending and retained bundles live outside this disposable root.
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
cp THIRD_PARTY_NOTICES.txt "$APP/Contents/Resources/Licenses/THIRD_PARTY_NOTICES.txt"
SWIFT_CACHE="$TARGET/swift-module-cache/$APP_ARCH"
mkdir -p "$SWIFT_CACHE"
SWIFT_ARGS=(-module-cache-path "$SWIFT_CACHE" -target "$APP_ARCH-apple-macosx14.0" -O)
for helper in removebg cloudctl volumectl; do
    xcrun swiftc "${SWIFT_ARGS[@]}" "$helper.swift" -o "$APP/Contents/MacOS/$helper"
    codesign --force --timestamp=none --sign "$SIGNING_IDENTITY" "$APP/Contents/MacOS/$helper"
done
xcrun swiftc "${SWIFT_ARGS[@]}" native/nextcloud/Core.swift native/nextcloud/main.swift -o "$APP/Contents/MacOS/nextcloudctl"
codesign --force --timestamp=none --sign "$SIGNING_IDENTITY" "$APP/Contents/MacOS/nextcloudctl"
xcrun swiftc "${SWIFT_ARGS[@]}" native/todoist/Core.swift native/todoist/Composer.swift native/todoist/main.swift -o "$APP/Contents/MacOS/todoistctl"
codesign --force --timestamp=none --sign "$SIGNING_IDENTITY" "$APP/Contents/MacOS/todoistctl"
xcrun swiftc -parse-as-library "${SWIFT_ARGS[@]}" native/weblinks/Form.swift native/weblinks/main.swift -o "$APP/Contents/MacOS/weblinkctl"
codesign --force --timestamp=none --sign "$SIGNING_IDENTITY" "$APP/Contents/MacOS/weblinkctl"
xcrun swiftc -parse-as-library "${SWIFT_ARGS[@]}" native/network/Core.swift native/network/System.swift native/network/Form.swift native/network/main.swift -o "$APP/Contents/MacOS/networkctl"
codesign --force --timestamp=none --sign "$SIGNING_IDENTITY" "$APP/Contents/MacOS/networkctl"
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
    <key>CFBundleShortVersionString</key><string>$BUNDLE_VERSION</string>
    <key>CFBundleGetInfoString</key><string>Schnauz $VERSION</string>
    <key>CFBundleVersion</key><string>$BUNDLE_VERSION</string>
    <key>LSMinimumSystemVersion</key><string>14.0</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>LSApplicationCategoryType</key><string>public.app-category.productivity</string>
    <key>NSHumanReadableCopyright</key><string>Schnauz © Oliver Weber. Based on Shuffle © Jaime Guzman. MIT license.</string>
</dict></plist>
PLIST
python3 scripts/check_bundle_contents.py "$APP"
codesign --force --timestamp=none --sign "$SIGNING_IDENTITY" --identifier local.schnauz.app "$APP"
codesign --verify --deep --strict "$APP"
for executable in "$APP"/Contents/MacOS/*; do
    if [ "$(lipo -archs "$executable")" != "$APP_ARCH" ]; then
        echo "Helper architecture mismatch: $executable" >&2
        exit 1
    fi
done
# Refuse live replacement: deleting a moved running executable breaks macOS
# privacy attribution even when old and new builds have the same signer.
python3 scripts/install_bundle.py "$DEST_APP" --source "$APP"
