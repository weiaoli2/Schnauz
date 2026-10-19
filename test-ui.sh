#!/bin/bash
# Real native layout with synthetic tasks, hidden windows and disposable settings.
set -euo pipefail
cd "$(dirname "$0")"
source ./dev-env.sh
cargo build --locked --features ui-checks
VIDEO_TEST_DIR="${CARGO_TARGET_DIR:-target}/video-preview-tests"
mkdir -p "$VIDEO_TEST_DIR"
xcrun swiftc -module-cache-path "$VIDEO_TEST_DIR/modules" native/VideoPreviewFixture.swift -o "$VIDEO_TEST_DIR/make-fixture"
VIDEO_FIXTURE=$(mktemp -d "${TMPDIR:-/tmp}/schnauz-video-check.XXXXXX")
trap 'rm -rf "$VIDEO_FIXTURE"' EXIT
"$VIDEO_TEST_DIR/make-fixture" "$VIDEO_FIXTURE/sample.mp4"
export SCHNAUZ_UI_VIDEO_FIXTURE="$VIDEO_FIXTURE/sample.mp4"
"${CARGO_TARGET_DIR:-target}/debug/schnauz" --video-preview-test "$SCHNAUZ_UI_VIDEO_FIXTURE"
"${CARGO_TARGET_DIR:-target}/debug/schnauz" --text-preview-test
"${CARGO_TARGET_DIR:-target}/debug/schnauz" --check-layout
