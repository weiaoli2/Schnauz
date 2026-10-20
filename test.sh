#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
source ./dev-env.sh
./test-signing.sh
python3 scripts/check_bundle_contents.py --self-test
cargo test "$@"
cargo test --locked -p gpui-terminal --lib
NATIVE_TEST_DIR="${CARGO_TARGET_DIR:-target}/nextcloud-tests"
mkdir -p "$NATIVE_TEST_DIR"
xcrun swiftc -parse-as-library -module-cache-path "$NATIVE_TEST_DIR/modules" \
    native/nextcloud/Core.swift native/nextcloud/Tests.swift -o "$NATIVE_TEST_DIR/run"
"$NATIVE_TEST_DIR/run"
TODOIST_TEST_DIR="${CARGO_TARGET_DIR:-target}/todoist-tests"
mkdir -p "$TODOIST_TEST_DIR"
xcrun swiftc -parse-as-library -module-cache-path "$TODOIST_TEST_DIR/modules" \
    native/todoist/Core.swift native/todoist/Tests.swift -o "$TODOIST_TEST_DIR/run"
"$TODOIST_TEST_DIR/run"

COMPOSER_TEST_DIR="${CARGO_TARGET_DIR:-target}/todoist-composer-tests"
mkdir -p "$COMPOSER_TEST_DIR"
xcrun swiftc -parse-as-library -module-cache-path "$COMPOSER_TEST_DIR/modules" \
    native/todoist/Core.swift native/todoist/Composer.swift native/todoist/ComposerTests.swift -o "$COMPOSER_TEST_DIR/run"
"$COMPOSER_TEST_DIR/run"

WEBLINK_TEST_DIR="${CARGO_TARGET_DIR:-target}/weblink-tests"
mkdir -p "$WEBLINK_TEST_DIR"
xcrun swiftc -parse-as-library -module-cache-path "$WEBLINK_TEST_DIR/modules" native/weblinks/Form.swift native/weblinks/Tests.swift -o "$WEBLINK_TEST_DIR/run"
"$WEBLINK_TEST_DIR/run"

NETWORK_TEST_DIR="${CARGO_TARGET_DIR:-target}/network-tests"
mkdir -p "$NETWORK_TEST_DIR"
xcrun swiftc -parse-as-library -module-cache-path "$NETWORK_TEST_DIR/modules" native/network/Core.swift native/network/Form.swift native/network/Tests.swift -o "$NETWORK_TEST_DIR/run"
"$NETWORK_TEST_DIR/run"

./test-ui.sh
