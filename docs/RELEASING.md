# Building a Schnauz release

Official repository: https://github.com/weiaoli2/Schnauz (public).
Current release: **v0.5.5**, public Apple Silicon + Intel early preview.

## Authorized next release — 0.5.9

Oliver explicitly requested publication of 0.5.9 with both DMGs on 2026-10-18,
including descriptions of the bug fixes. Use the same public repository and
prerelease/signing policy. Preserve v0.5.5 and all live sessions. Release notes
cover preview geometry/Information, drag activation and cancellation, Nextcloud
aliases/badges, installed Quick Actions, permanent sidebar shortcuts, and the
new preview zoom, action shortcuts and Schnauz-only Open With preferences.

## Current status — 0.5.5 published

On 2026-10-18 Oliver resumed GitHub publication after the transfer footer and
stable tab-drag feedback fixes. He requested a concise, attractive overview,
no new screenshots, and a link to https://techguyzone.com/apps/.
The earlier publication hold and unpublished v0.5.0-rc.1 candidate are superseded.

0.5.5 was published on 2026-10-18 at
https://github.com/weiaoli2/Schnauz/releases/tag/v0.5.5. Both DMGs, MPL sources
and the checksum file passed local verification and matched GitHub asset digests.
Source: v0.5.5 / 7c48f14. Receipt: dist/0.5.5/RELEASE-VERIFICATION.json.
This request is complete; future releases need new authorization and a new version.
Never rerun publication over the existing tag/assets.
Reuse this workflow; rebuild both targets from the final tested source. Publish
as a prerelease because this is an early preview with Apple Development signing
and no Apple notarization. Never replace/restart the user's live app or terminals.

The repository already exists. Keep existing caches, targets, certificates and
notices. Do not recreate release tooling or upload app profiles, tokens or keys.

Homepage: https://techguyzone.com/apps/
Local branch: schnauz. Public destination: origin/main. Shuffle upstream is reference only.

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
The release script attaches their exact source archive. ZIP-incompatible dates
before 1980 are clamped to the ZIP minimum without changing file contents. This supplies component
source access without including compiler caches or personal app settings.
All other components retain their own terms. Preserve the original Shuffle MIT
LICENSE and all vendored terminal licenses.

## Verify before publishing

Run `./test.sh`, verify each app and helper's architecture and code signature,
and verify the disk images and checksums. Record which architectures were
actually executed; cross-compilation alone is not an Intel hardware smoke test.
Keep the source commit/tag with the artifacts and publish both architectures
together. Never upload signing keys, tokens, live settings or compiler caches.

This preview uses Apple Development signing, not Developer ID or Apple
notarization. Keep this limitation explicit on the download page and in the
installer. Moving to Developer ID/notarization is a separate future improvement.
Do not disable system security checks or modify user privacy databases.

Publishing is a separate explicit action through GitHub Releases. Only push to
the Schnauz origin; the Shuffle upstream remote is read-only reference material.

## Reuse this workflow after the bug fixes

1. Finish and verify Oliver's reported fixes before publication. Preserve live
   settings, terminals and SSH sessions. The screenshot demo has a separate
   profile under /private/tmp/Schnauz-Website-Demo; never substitute it for the
   real app's profile. The web kit stays frozen unless Oliver requests changes.
2. Review and commit the fixes locally. Confirm the intended Cargo version
   (currently 0.5.5) in both Cargo.toml and Cargo.lock. Never reuse an old published version for changed artifacts. Do not move
   any existing tag or overwrite already published assets in later releases.
3. Run ./test.sh. Reuse the existing Apple Silicon and Intel targets and caches.
   Run the existing native text-preview smoke check on both release binaries;
   this Mac has Rosetta, so Intel can execute under translation. Record that as
   Rosetta validation, not a physical Intel hardware test.
4. Run the existing ./release.sh to build both architectures, signed app bundles,
   verified DMGs, the exact MPL component-source ZIP and SHA256SUMS.txt. Output
   defaults to dist/<version>; SCHNAUZ_RELEASE_OUTPUT can select another absolute
   destination. Never point release output at the running Schnauz.app.
5. Check each app/helper architecture and signature, installer checksums and
   source correspondence. Review prospective committed files and Git history
   for credentials without reading Keychain tokens. Live settings, tokens,
   compiler caches, signing keys and the website kit do not belong in Git.
6. Write release notes in the output directory with the features/fixes, source
   commit, macOS 14+ requirement, Apple Silicon/Intel choices and validation
   scope. Clearly state Apple Development signing and no Apple notarization.
7. With user-authorized publication, use the existing origin, push the
   reviewed source, create the version tag and prepare a draft GitHub release.
   Upload all artifacts together, verify the draft and then publish it as a
   prerelease. No new repository, updater, account setup or screenshots needed.

The publication commands below require a user-authorized release. 0.5.5 is
already published. For a newly authorized future version, run from the repository
after committing and verifying its fixes:

```sh
VERSION=$(sed -nE 's/^version = "([^"]+)"/\1/p' Cargo.toml | head -1)
TAG="v$VERSION"
OUT="${SCHNAUZ_RELEASE_OUTPUT:-$PWD/dist/$VERSION}"

git status --short
git push origin HEAD:main
git tag -a "$TAG" -m "Schnauz $VERSION release candidate"
git push origin "$TAG"

gh release create "$TAG" \
  "$OUT/Schnauz-$VERSION-macos-arm64.dmg" \
  "$OUT/Schnauz-$VERSION-macos-x86_64.dmg" \
  "$OUT/Schnauz-$VERSION-MPL-sources.zip" \
  "$OUT/SHA256SUMS.txt" \
  --repo weiaoli2/Schnauz --verify-tag --draft --prerelease \
  --title "Schnauz $VERSION — Release Candidate" \
  --notes-file "$OUT/RELEASE-NOTES.md"

gh release view "$TAG" --repo weiaoli2/Schnauz \
  --json tagName,isDraft,isPrerelease,url,assets

# Final step, once the draft and its assets are verified:
gh release edit "$TAG" --repo weiaoli2/Schnauz --draft=false --prerelease
```

For subsequent releases, reuse these scripts and update the version, release
notes and changed validation details. Regenerate notices only when dependency
changes require it. Keep the GitHub homepage set to https://techguyzone.com/apps/.
