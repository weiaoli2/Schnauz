#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
source ./dev-env.sh
export SCHNAUZ_BUILD_PROFILE="${SCHNAUZ_BUILD_PROFILE:-release}"
if [ "$SCHNAUZ_BUILD_PROFILE" = debug ]; then
    cargo build
else
    cargo build --release
fi
./make_app.sh
