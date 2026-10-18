#!/bin/bash
# Shared build environment. Keep the prepared cache with this checkout.
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
export RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-stable}"
if [ -d /Applications/Xcode.app/Contents/Developer ]; then
    export DEVELOPER_DIR="${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}"
fi
SCHNAUZ_CACHE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/.build-cache"
if [ -d "$SCHNAUZ_CACHE_ROOT/cargo-home/registry" ]; then
    export CARGO_HOME="${CARGO_HOME:-$SCHNAUZ_CACHE_ROOT/cargo-home}"
    export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$SCHNAUZ_CACHE_ROOT/cargo-target}"
    export CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}"
fi
