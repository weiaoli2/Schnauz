# Building a Schnauz release

Official repository: https://github.com/weiaoli2/Schnauz (public).
Current release: **v0.8.9**, public Apple Silicon + Intel early preview.
Previous published release: **v0.8.1**.

## Authorized release — 0.9.21 (2026-10-30)

Oliver confirmed Schnauz is working well and requested a new public GitHub
release. This supersedes the earlier candidate holds. Publish the current
0.9.21 source, including the requested faster Devices refresh and Info/Eject controls, both signed architecture DMGs, exact MPL sources and checksums.
Keep all previous tags/releases, including unpublished 0.9.2 preparation.
Exclude website/, video/ and test.csv from the source commit/archive.

Retain compile/sign/package-only validation while the live Schnauz session is
in use: do not execute application, native helper or test binaries. Static
architecture/signature, bundle allowlist, DMG, source archive and checksum
verification are permitted. Do not claim a fresh runtime/UI or Intel hardware
test, confirmed Nextcloud provider actions, resolved issues #1/#2, or a confirmed
fix for the reported Codex-over-SSH redraw problem. Publish as an early prerelease
with Apple Development signing and no Apple notarization. Preserve the running
bundle, windows, PTYs, SSH sessions and settings. Build the normal local app only
through safe deferred installation; applying it remains Oliver's deliberate step.

## Authorized release — 0.9.2

Before publication Oliver requested a different, progressive Disk Usage scanner
and wants to test its approach. The 0.9.2 architecture builds remain unpublished;
local 0.9.3 is the new test stand. Do not publish the superseded candidate while
that feedback is pending. Continue release preparation after the scanner is accepted.

On 2026-10-25 Oliver requested publishing the current 0.9.2 test build to GitHub.
This supersedes the paused 0.9.0 preparation. Publish new public source and both
signed architecture DMGs as an early prerelease; preserve all previous tags/assets.
Include exact MPL sources, checksums, English changes since 0.8.9 and the homepage.
Exclude website/video production and test.csv. Do not claim issues #1/#2 resolved.

Oliver is using the live Schnauz terminal for this session. Preserve its executable
and bundle in place, all windows/PTYs/SSH sessions and privacy attribution. This
release's checks use command-line Rust tests and static architecture/signature,
DMG/source/checksum checks. Do not rerun native/UI suites or launch either release
binary/helper for smoke tests while he is working. Report that validation limit
explicitly; earlier native/UI results are historical, not a fresh 0.9.2 smoke test.
Direct Nextcloud pin-action QA and the cause of reported 0.9.0 SIGSEGVs remain
unresolved. Apple Development signed, not notarized; no physical Intel validation.

## Authorized release — 0.9.0

On 2026-10-24 Oliver requested reviewing all changes since 0.8.9 and publishing
0.9.0 with both architectures, plus triage of the two open GitHub issues.
Preserve the existing early-prerelease policy, Apple Development / unnotarized
disclosure, exact MPL sources, dependency notices and safe live-app staging.
The release includes project-specific exclusion filters and blue project chrome,
seamless active tabs, finer native icons, startup-disk inspection fixes, audio
preview and compact document margins, Nextcloud menu/status cleanup and measured
mouse editing for Rename. Exclude local website/video production and test.csv.
Issues #1 and #2 remain open: direct persistent offline policy has an implemented
runtime-checked bridge but awaits real provider QA; returning to preserved search
results after Show in Folder is not implemented. Oliver paused
release preparation to correct Nextcloud behavior; its rejected Finder handoff
has been removed. Do not resume publication until he requests it.

## Published release — 0.8.9

Published 2026-10-23 at https://github.com/weiaoli2/Schnauz/releases/tag/v0.8.9,
source 0ec6e3cd539cf16edee0db3ce79f4fb3889f755d. Both signed DMGs, exact MPL
sources and SHA256SUMS match all four GitHub asset digests. Public unauthenticated
visibility, exact notes/URLs and all 1746 source ZIP files verified. Every helper
architecture/signature, mounted installer and native text-preview smoke check
passed. ARM native / Intel Rosetta on Apple Silicon, no physical Intel hardware.
Apple Development signed, not notarized, early prerelease. Receipts: dist/0.8.9.
The normal local update is staged because the old app is running; PID, installed
inodes and hash remain unchanged. No restart or privacy changes. Request complete;
preserve v0.8.9 and every earlier release. Future publication requires a new request.

## Authorized release — 0.8.9

Oliver requested an updated GitHub release on 2026-10-23 following the confirmed
live-bundle deletion bug. Publish the accumulated verified fixes since 0.8.1 as
0.8.9, with Apple Silicon and Intel DMGs, exact MPL source ZIP and SHA256SUMS.
Use a new tag and early prerelease; preserve all existing releases/tags. Stable
Apple Development signing, no notarization. Explicitly distinguish safe source
build/install staging from manual Finder replacement, which requires quitting
Schnauz first. No live session restart, automatic update or crash-report upload.

## Published release — 0.8.1

Published 2026-10-20 at https://github.com/weiaoli2/Schnauz/releases/tag/v0.8.1,
source c8a427520bf53ddbf32f8421cdd59075f0f877da. Both DMGs, exact MPL component sources and checksum
asset match GitHub SHA-256 digests, including the unauthenticated public listing.
GitHub source archive matches all 1741 tagged files. Every helper architecture
and stable certificate signature passed; readonly-mounted installers match the
verified apps and install notes. ARM smoke checks ran natively; Intel ran under
Rosetta on Apple Silicon, not physical Intel hardware. Apple Development signed,
not notarized, early prerelease. Receipts are in dist/0.8.1.
The 0.8.0 tag/draft is retained as superseded unpublished preparation. The ongoing
release request is complete. Future publication requires a new request; never
move existing tags, overwrite assets or restart live sessions.

## Corrected candidate — 0.8.1

Oliver reported stale previews during the 0.8.0 draft upload, before publication.
The ongoing release request now includes the selected-file preview refresh fix.
Preserve v0.8.0 and its draft; publish 0.8.1 with both verified architectures and
updated notes. Do not move the existing tag or publish the superseded draft.

## Authorized release — 0.8.0

On 2026-10-20 Oliver requested release 0.8 with GitHub notes and both builds,
after shared alias/add-on edits. Publish 0.8.0 as an early prerelease using the
existing Apple Development certificate, explicitly unnotarized, with notes
covering changes since 0.5.14. Include both DMGs, MPL sources and SHA256SUMS.
Preserve all prior tags/assets and live sessions. Exclude local website/video
production and test.csv from the source commit/archive.

## Published release — 0.5.14

Published on 2026-10-19 at
https://github.com/weiaoli2/Schnauz/releases/tag/v0.5.14 from source
5b26dd7fcc296acd9e950a7fbadf5331f9bd31cb. Both signed DMGs, the exact MPL
source ZIP and SHA256SUMS.txt match all four uploaded GitHub asset digests.
The downloaded GitHub source archive matches the local source tag exactly
(1725 files), excluding website/video work, local settings and build caches.
Apple Silicon ran natively; Intel's native text-preview smoke test ran under
Rosetta, not physical Intel hardware. Every helper's architecture and certificate
signature passed, and readonly-mounted DMGs match the verified app bundles.
93 Rust tests plus offline native and complete hidden GPUI checks passed.
The public unauthenticated update endpoint exposes both correctly named DMGs.
Apple Development signed, not notarized; published as an early prerelease.
Receipts: dist/0.5.14/{LOCAL-,SOURCE-,RELEASE-}VERIFICATION.json.
This request is complete; future versions need a new user request. Do not replace
these artifacts or move this tag; retain the earlier v0.5.11 checkpoint too.

## Authorized release preparation — 0.5.14

On 2026-10-19 Oliver approved the floating Todoist window and final outline pin,
then explicitly requested a commit and public release with the accumulated fixes.
Publish 0.5.14 as an early prerelease with Apple Silicon and Intel DMGs, English
notes covering every change since public 0.5.9, MPL component sources and checksums.
Use the existing Apple Development signer and state that it is not notarized.
Preserve previous releases/tags, the v0.5.11 checkpoint and all live sessions.
Website/video production folders remain outside the source commit and release.

## Published release — 0.5.9

Oliver explicitly requested publication of 0.5.9 with both DMGs on 2026-10-18,
including descriptions of the bug fixes. Use the same public repository and
prerelease/signing policy. Preserve v0.5.5 and all live sessions. Release notes
cover preview geometry/Information, drag activation and cancellation, Nextcloud
aliases/badges, installed Quick Actions, permanent sidebar shortcuts, and the
new preview zoom, action shortcuts and Schnauz-only Open With preferences.

Published at https://github.com/weiaoli2/Schnauz/releases/tag/v0.5.9 from source
ba395fa98b044c7b6dc4506614b080df7851ac30. Both signed DMGs, exact MPL sources and
SHA256SUMS.txt match GitHub's four asset digests. ARM ran natively; Intel's native
text-preview smoke test passed under Rosetta. Verification receipt:
dist/0.5.9/RELEASE-VERIFICATION.json. This publication request is complete; future
versions need a new request. Never overwrite these assets or move the tag.

## Earlier published release — 0.5.5

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
   in both Cargo.toml and Cargo.lock. Never reuse an old published version for changed artifacts. Do not move
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

The publication commands below require a user-authorized release. 0.5.14 is
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

The architecture-specific release builds do not update the ordinary
`Schnauz.app` at the repository root. After a release, run `./build.sh` with its
normal output. If that app is running, packaging stages a verified pending build
and leaves the installed bundle untouched. Keeping the process alive while
moving/deleting its bundle breaks macOS privacy attribution and is forbidden.
After Oliver deliberately quits, apply the pending build using the printed
`python3 scripts/install_bundle.py DESTINATION` command and verify version and
signature. Launching remains manual; never stop live windows/PTYs to apply it.
Replaced bundles remain in ignored .build-cache/retained-apps, outside staging
cleanup, to protect a launch racing the process inventory. Do not delete these
while any process could still be using them. No TCC resets or trust changes.
