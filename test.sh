#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
source ./dev-env.sh
./test-signing.sh
cargo test "$@"
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

./test-ui.sh
