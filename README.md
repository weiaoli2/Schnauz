# Shuffle Studio

A personal Mac file manager based on [Shuffle](https://github.com/WizenPainter/shuffle).
This is an independent local version, with its own application identity and settings.

## First version

- Studio Light and Studio Dark appearances, an indigo accent, and roomier lists.
- Search, split/join panes, preview and workspace controls in the toolbar.
- Named workspaces for local folders: tabs, view modes, sorting and split layout.
- A preview panel and footer with item counts, current path and appearance control.
- The existing native file browser, search, file operations and Mac integrations.

Click **Save workspace** or press **⌘⇧S** to name the current layout. Click a
workspace in the sidebar to restore it. Right-click a saved workspace to update
it from the current tabs or remove it. Workspaces save local folders; remote
connections continue to work as ordinary tabs but aren't included in snapshots.
Unavailable folders are skipped when restoring; no folders are created or removed.

| Shortcut | Action |
|---|---|
| ⌘P | Search files and commands |
| ⌘⇧S | Save workspace |
| ⌘⇧D | Split or join panes |
| ⌘⇧I | Show or hide preview |
| ⌘T | New tab |
| ⌘, | Settings |

## Build

Requires Rust stable and Xcode with its Metal compiler. The packaged app targets
macOS 14 or later and is built for the current Mac's processor.

```sh
./build.sh
open "Shuffle Studio.app"
```

For development: `STUDIO_BUILD_PROFILE=debug ./build.sh`. The app is locally
ad-hoc signed; public distribution and Apple notarization are not configured.
`./make_dmg.sh` packages an existing build without publishing it.

```sh
./test.sh
```

Normal settings live in `~/Library/Application Support/Shuffle Studio/`.
This prepared workspace reuses its local compiler cache in offline mode.
If adding a new dependency, use `CARGO_NET_OFFLINE=false ./build.sh` to fetch it.
For native UI checks, `SHUFFLE_STUDIO_CONFIG_DIR`, `SHUFFLE_STUDIO_START_DIR`
and `SHUFFLE_STUDIO_INDEX_ROOT` can point at disposable test folders.

## Source

- `src/theme.rs`: palette and live theming
- `src/studio.rs`: custom toolbar, footer and workspace controls
- `src/workspaces.rs`: JSON workspace storage and validation
- `src/main.rs`: the existing GPUI file browser and file operations
- `scripts/generate_icon.swift`: editable native icon artwork

The original snapshot is tagged `studio-starting-point`; the working branch is
`studio`. The `upstream` remote is retained for provenance. This app does not
check for or install original Shuffle updates.

Based on Shuffle by Jaime Guzman, under the [MIT license](LICENSE).
The original README is preserved in [README.upstream.md](README.upstream.md).
