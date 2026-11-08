#!/bin/bash
# Merge existing signed architecture bundles without touching a running app.
set -euo pipefail
cd "$(dirname "$0")"
source ./dev-env.sh
source ./build-signing.sh
if [ "$#" -ne 3 ]; then
    echo "Usage: ./make_universal.sh ARM_APP INTEL_APP OUTPUT_APP" >&2
    exit 2
fi
PREVIOUS_SIGNER=$(codesign -dvv "$1" 2>&1 | sed -n 's/^Authority=//p' | head -1)
SIGNING_IDENTITIES=$(security find-identity -v -p codesigning | sed -nE 's/^[[:space:]]*[0-9]+\) ([[:xdigit:]]{40}) "([^"]+)".*/\1|\2/p')
SIGNING_IDENTITY=$(schnauz_signing_identity "${SCHNAUZ_CODESIGN_IDENTITY:-}" "$PREVIOUS_SIGNER" "$SIGNING_IDENTITIES")
python3 scripts/make_universal_app.py "$1" "$2" "$3" --identity "$SIGNING_IDENTITY"
