#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
source ./dev-env.sh
export STUDIO_BUILD_PROFILE="${STUDIO_BUILD_PROFILE:-release}"
if [ "$STUDIO_BUILD_PROFILE" = debug ]; then
    cargo build
else
    cargo build --release
fi
./make_app.sh
