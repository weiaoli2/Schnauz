#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
source ./build-signing.sh
DEV_HASH='1111111111111111111111111111111111111111'
OTHER_HASH='2222222222222222222222222222222222222222'
DEV="$DEV_HASH|Apple Development: Test Developer (TESTTEAM)"
OTHER="$OTHER_HASH|Developer ID Application: Test Developer (TESTTEAM)"
expect() {
    local wanted="$1"; shift
    local actual
    actual=$(schnauz_signing_identity "$@")
    if [ "$actual" != "$wanted" ]; then
        printf 'FAIL signing identity: expected %s, got %s\n' "$wanted" "$actual" >&2
        exit 1
    fi
}
reject() {
    if schnauz_signing_identity "$@" >/dev/null 2>&1; then
        printf '%s\n' 'FAIL signing identity: an ambiguous or missing certificate was accepted' >&2
        exit 1
    fi
}
expect "$DEV_HASH" '' '' "$DEV"
expect "$DEV_HASH" '' 'Apple Development: Test Developer (TESTTEAM)' "$DEV
$OTHER"
expect "$OTHER_HASH" "$OTHER_HASH" '' "$DEV
$OTHER"
expect "$DEV_HASH" 'Apple Development: Test Developer (TESTTEAM)' '' "$DEV"
expect '-' '-' 'An unavailable previous certificate' ''
reject '' '' ''
reject '' '' "$DEV
$OTHER"
reject 'missing identity' '' "$DEV"
reject '' 'An unavailable previous certificate' "$DEV"
reject 'Apple Development: Test Developer (TESTTEAM)' '' "$DEV
$OTHER_HASH|Apple Development: Test Developer (TESTTEAM)"
printf '%s\n' 'PASS signing identity selection, signer preservation, ambiguity and explicit ad-hoc opt-in'
