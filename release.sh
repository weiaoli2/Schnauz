#!/bin/bash
# Local packaging only; publishing is configured separately for our own repo.
set -euo pipefail
cd "$(dirname "$0")"
./build.sh
./make_dmg.sh
