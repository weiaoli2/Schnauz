#!/bin/bash
# Real native layout with synthetic tasks, hidden windows and disposable settings.
set -euo pipefail
cd "$(dirname "$0")"
source ./dev-env.sh
cargo build --locked --features ui-checks
"${CARGO_TARGET_DIR:-target}/debug/schnauz" --check-layout
