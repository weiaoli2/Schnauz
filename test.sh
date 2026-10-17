#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
source ./dev-env.sh
cargo test "$@"
NATIVE_TEST_DIR="${CARGO_TARGET_DIR:-target}/nextcloud-tests"
mkdir -p "$NATIVE_TEST_DIR"
xcrun swiftc -parse-as-library -module-cache-path "$NATIVE_TEST_DIR/modules" \
    native/nextcloud/Core.swift native/nextcloud/Tests.swift -o "$NATIVE_TEST_DIR/run"
"$NATIVE_TEST_DIR/run"
