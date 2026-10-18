# Building a Schnauz release

Official repository: https://github.com/weiaoli2/Schnauz (public).
Planned first candidate: **v0.5.0-rc.1**, with public source and public downloads.

## Current status — publication on hold

On 2026-10-18 Oliver asked to save and reuse this release workflow, fix the main
bugs first, and publish the release only afterwards. Do not run the publishing
steps until he explicitly resumes the release. Oliver separately authorized a
website product-info and screenshot refresh on 2026-10-18; that refresh does not
authorize release publication, tags, installers or uploads.

The public repository and source already exist at the URL above. No GitHub
release/tag was published during preparation. Both architectures were built into
the chat's outputs/Schnauz-0.5.0-rc.1 directory, and tests passed. Those are
pre-fix artifacts, not the final release; rebuild after the approved bug fixes.
Keep the existing compiler caches, target installations, certificates, notices,
README and packaging scripts. Do not recreate the repository or release tooling.

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

## Reuse this workflow after the bug fixes

1. Finish Oliver's reported bugs and let him test the main app. Preserve live
   settings, terminals and SSH sessions. The screenshot demo has a separate
   profile under /private/tmp/Schnauz-Website-Demo; never substitute it for the
   real app's profile. The web kit stays frozen unless Oliver requests changes.
2. Review and commit the fixes locally. Confirm the intended Cargo version
   (currently 0.5.0-rc.1) in both Cargo.toml and Cargo.lock. Since no candidate
   tag/release exists yet, the same candidate version can be kept. Do not move
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
7. Only after Oliver resumes publication, use the existing origin, push the
   reviewed source, create the version tag and prepare a draft GitHub release.
   Upload all artifacts together, verify the draft and then publish it as a
   prerelease. No new repository, updater, account setup or screenshots needed.

The publication commands below are a saved procedure, not permission to execute
them while the release is on hold. Run from the repository after committing fixes:

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
