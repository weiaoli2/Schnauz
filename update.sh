#!/bin/bash
# Install the latest locally staged Schnauz build without stopping live sessions.
set -euo pipefail
cd "$(dirname "$0")"

if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
    echo "Usage: ./update.sh"
    echo "Installs the latest locally prepared, signed build of Schnauz."
    echo "Quit Schnauz first. Running apps and terminal sessions are never stopped."
    echo "No GitHub download, source build or automatic app launch is performed."
    exit 0
fi
if [ "$#" -ne 0 ]; then
    echo "Usage: ./update.sh" >&2
    exit 2
fi

exec python3 scripts/install_bundle.py "$PWD/Schnauz.app"
