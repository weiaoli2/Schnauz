#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
source ./dev-env.sh
cargo test "$@"
