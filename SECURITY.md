# Security and release verification

Report security issues privately to Oliver Weber at **o@freedom-net.de**.
Please avoid posting credentials, personal paths or private file contents in issues.

## Release provenance

Official source and downloads are at [weiaoli2/Schnauz](https://github.com/weiaoli2/Schnauz).
Releases identify their source tag and commit and include SHA-256 checksums.
The 0.5.0-rc.1 Mac builds are signed locally with Oliver's **Apple Development**
certificate. They are **not Apple-notarized** and do not yet use Developer ID
distribution signing. A valid bundle signature checks integrity; it does not
mean that Gatekeeper will automatically allow the app or that Apple reviewed it.

Check a downloaded disk image against the checksum from the same release:

```sh
shasum -a 256 -c SHA256SUMS.txt
codesign --verify --deep --strict /Applications/Schnauz.app
codesign -dvv /Applications/Schnauz.app
```

Keep the checksum file with both disk images and the MPL source archive when
checking all entries. Checksums establish correspondence to release assets;
they are not independent publisher authentication. On a Mac blocking first
launch, use the per-app Open Anyway option in Privacy & Security only if you
trust the download. Do not disable Gatekeeper globally.

## Local builds and updates

`./build.sh` selects a stable Apple Development / Developer ID signing identity,
or `SCHNAUZ_CODESIGN_IDENTITY` can choose one. Signing keys stay in Keychain and
are never committed or uploaded. Temporary ad-hoc signing is explicit opt-in
with `SCHNAUZ_CODESIGN_IDENTITY=-`; rebuilding these changes their privacy identity.
All helpers are built for the app's architecture and signed with its identity.
Packaging verifies a staged bundle before installation and never restarts it.

There is no automatic updater. Download new versions from Releases or build from
source. Schnauz does not use the original Shuffle update channel.

## Credentials and filesystem access

Nextcloud and Todoist credentials stay in macOS Keychain; settings contain
nonsecret routing metadata. Existing SFTP credentials retain a compatibility
read path. Tests use temporary data and mocked HTTP responses, never live accounts.
Schnauz cannot grant itself Full Disk Access; macOS and the user control access.

Local terminal folder sync pauses while another CLI runs. SSH folder following
uses the authenticated terminal transport and never sends navigation commands
into a remote session. See the [feature guide](docs/FEATURES.md) for the scope
of supported SSH shells and connection types.

Upstream attribution is retained in LICENSE, CREDITS.txt and THIRD_PARTY_NOTICES.txt.
