# Building a Schnauz release

Official repository: https://github.com/weiaoli2/Schnauz (public).
First public candidate: **v0.5.0-rc.1**. Source and downloads are public.

## Requirements

- macOS with full Xcode, Metal tools and Rust stable.
- `rustup target add aarch64-apple-darwin x86_64-apple-darwin`.
- An Apple Development or Developer ID certificate for stable local signing.
- Python 3 for generating third-party notices and source archives.

`./release.sh` builds both architectures into the ignored `dist/` directory,
creates verified DMGs and a checksum manifest, and never publishes or replaces
the developer's running Schnauz.app. It checks dependency notices before packaging.
The source version in Cargo.toml and Cargo.lock must match.

For a single build:

```sh
SCHNAUZ_TARGET=x86_64-apple-darwin \
SCHNAUZ_APP_OUTPUT="$PWD/dist/intel/Schnauz.app" ./build.sh

SCHNAUZ_APP_OUTPUT="$PWD/dist/intel/Schnauz.app" \
SCHNAUZ_DMG_OUTPUT="$PWD/dist/Schnauz-intel.dmg" ./make_dmg.sh
```

`SCHNAUZ_TARGET` selects Rust's target and every Swift helper's architecture.
Without it, development builds target the current Mac. `SCHNAUZ_APP_OUTPUT`
defaults to Schnauz.app in the checkout. Pre-release Cargo versions are shown
in About; the bundle's numeric version omits the pre-release suffix.

## Dependency licenses

`scripts/generate_notices.py` reads Cargo's locked dependency metadata and
collects supplied license/notice files, plus supplemental upstream texts for
crates that omitted them from their archive. Provenance for those exact upstream
texts is recorded alongside them under vendor/licenses/supplemental.
The result is THIRD_PARTY_NOTICES.txt, copied into each app bundle.

The unmodified MPL-2.0 components option-ext 0.2.0 (runtime) and cbindgen 0.28.0
(build tool) are preserved under vendor/mpl-sources with their original licenses.
The release script attaches their exact source archive. This supplies component
source access without including compiler caches or personal app settings.
All other components retain their own terms. Preserve the original Shuffle MIT
LICENSE and all vendored terminal licenses.

## Verify before publishing

Run `./test.sh`, verify each app and helper's architecture and code signature,
and verify the disk images and checksums. Record which architectures were
actually executed; cross-compilation alone is not an Intel hardware smoke test.
Keep the source commit/tag with the artifacts and publish both architectures
together. Never upload signing keys, tokens, live settings or compiler caches.

The first candidate uses Apple Development signing, not Developer ID or Apple
notarization. Keep this limitation explicit on the download page and in the
installer. Moving to Developer ID/notarization is a separate future improvement.
Do not disable system security checks or modify user privacy databases.

Publishing is a separate explicit action through GitHub Releases. Only push to
the Schnauz origin; the Shuffle upstream remote is read-only reference material.
