#!/bin/bash
# Shared build environment. Reuse this workspace's prepared cache when present.
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
export RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-stable}"
if [ -d /Applications/Xcode.app/Contents/Developer ]; then
    export DEVELOPER_DIR="${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}"
fi
STUDIO_CACHE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/work"
if [ -d "$STUDIO_CACHE_ROOT/cargo-home/registry" ]; then
    export CARGO_HOME="${CARGO_HOME:-$STUDIO_CACHE_ROOT/cargo-home}"
    export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$STUDIO_CACHE_ROOT/cargo-target}"
    export CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}"
fi
