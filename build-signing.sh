#!/bin/bash
# Select one stable signing identity. Inputs are public certificate metadata only.
schnauz_signing_identity() {
    local requested="${1-}" previous="${2-}" entries="${3-}"
    local hash label selected="" matches=0 eligible
    if [ "$requested" = "-" ]; then
        printf '%s\n' '-'
        return 0
    fi
    while IFS='|' read -r hash label; do
        [ -n "$hash" ] || continue
        eligible=false
        if [ -n "$requested" ]; then
            if [ "$requested" = "$hash" ] || [ "$requested" = "$label" ]; then eligible=true; fi
        elif [ -n "$previous" ]; then
            if [ "$previous" = "$label" ]; then eligible=true; fi
        else
            case "$label" in
                'Apple Development: '*|'Mac Developer: '*|'Developer ID Application: '*) eligible=true ;;
            esac
        fi
        if [ "$eligible" = true ]; then
            selected="$hash"
            matches=$((matches + 1))
        fi
    done <<< "$entries"
    if [ "$matches" -eq 1 ]; then
        printf '%s\n' "$selected"
        return 0
    fi
    if [ "$matches" -gt 1 ]; then
        printf '%s\n' 'Several signing identities match. Set SCHNAUZ_CODESIGN_IDENTITY to the exact certificate SHA-1.' >&2
    elif [ -n "$requested" ]; then
        printf '%s\n' 'SCHNAUZ_CODESIGN_IDENTITY does not match an available signing identity; the existing app was preserved.' >&2
    elif [ -n "$previous" ]; then
        printf '%s\n' 'The previous app signing certificate is unavailable. Select a replacement explicitly with SCHNAUZ_CODESIGN_IDENTITY.' >&2
    else
        printf '%s\n' 'No Apple Development or Developer ID signing identity is available. Configure one to preserve macOS permissions across builds.' >&2
    fi
    printf '%s\n' 'For a deliberately temporary build only, use SCHNAUZ_CODESIGN_IDENTITY=- (macOS may ask for permissions again).' >&2
    return 1
}
