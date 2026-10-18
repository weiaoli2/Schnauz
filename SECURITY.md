# Schnauz security and build provenance

Schnauz is a local macOS file manager with filesystem access and an interactive
terminal. The current personal build is compiled locally and ad-hoc signed.
Public CI attestations, Developer ID signing and Apple notarization are not yet
configured for Schnauz.

## Verify the local app

```sh
codesign --verify --deep --strict Schnauz.app
```

This checks the bundle signature and integrity. Ad-hoc signing does not establish
a publisher identity or notarization. Keep the source commit and artifact digest
with a future release so the reviewed source can be identified.

## Updates and distribution

There is no automatic updater. Build with `./build.sh`; `./make_dmg.sh` creates a
local Schnauz installer without publishing it. Release hosting, signing and
notarization must be configured for this independent project before distribution.

## Credentials and terminal access

Nextcloud and Todoist credentials belong to Schnauz and stay in macOS Keychain;
settings contain nonsecret routing metadata. Existing SFTP credentials retain a
compatibility read path. Do not put credentials into repository files or logs.
Automated integration tests use temporary data and mocked HTTP responses.

The local terminal follows browser paths only at an idle shell prompt. Remote
SSH folder following reuses the authenticated terminal connection and never
injects navigation commands into the remote session.

Report Schnauz security issues privately to Oliver Weber at o@freedom-net.de.
Upstream and third-party attribution is recorded in LICENSE and CREDITS.txt.
