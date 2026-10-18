#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
source ./dev-env.sh
export SCHNAUZ_BUILD_PROFILE="${SCHNAUZ_BUILD_PROFILE:-release}"
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-14.0}"
BUILD_ARGS=(--locked)
if [ "$SCHNAUZ_BUILD_PROFILE" != debug ]; then
    BUILD_ARGS+=(--release)
fi
if [ -n "${SCHNAUZ_TARGET:-}" ]; then
    BUILD_ARGS+=(--target "$SCHNAUZ_TARGET")
fi
cargo build "${BUILD_ARGS[@]}"
./make_app.sh
