# Getter

A personal Mac file manager based on [Shuffle](https://github.com/WizenPainter/shuffle).
This is an independent local version, with its own application identity and settings.

## First version

- Studio Light and Studio Dark appearances, an indigo accent, and compact 12 pt lists.
- Folder totals are not calculated automatically; the size column shows file sizes only.
- Applications display their native icons and are labeled **Application**. Click to
  select, double-click to launch, or right-click **Show Package Contents** to browse inside.
  App bundles appear under **Files**, while **Folders** shows ordinary directories.
- **All · Folders · Files** controls for every tab. Active filters tint the top bar,
  show the visible item count, and are remembered in sessions and projects.
- Search, split/join panes, preview and project controls in the toolbar.
- Projects with separate favorites, tabs, view modes, sorting and split layouts. Changes are remembered automatically.
- A preview panel and footer with item counts, current path and appearance control.
- The clickable path ends at the current folder. Long paths clip from the left,
  and hovering shows the complete path.
- Paste a local folder path with **⌘V** to open it. Paste a file path to open its
  enclosing folder and select that file. Absolute paths, `~/` paths, quoted paths
  and local `file://` URLs work; copied file objects retain ordinary copy/paste.
- A **Terminal** button opens a persistent interactive zsh session below the active
  file view. The folder view and shell directory follow each other. Folder sync
  pauses while another CLI app runs; browsing changes during that time are discarded.
  At the shell prompt, sync preserves unfinished input. The header shows **Synced**
  or **Unsynced**. Click **Unsynced** to restart in the browser's folder once idle,
  without moving focus into the terminal. Running, background and suspended jobs
  block recovery. A restart clears scrollback, shell variables and unfinished input.
- The existing native file browser, search, file operations and Mac integrations.

Click **New project** or press **⌘⇧S** to create a project from the current tabs.
Click a project in the sidebar to switch to its own favorites and last layout.
Use **+** beside **Project favorites** to pin the current folder; right-click a
favorite to remove it. **Personal** keeps a separate set of general browsing tabs
and favorites. Standard **Locations** remain shared. Existing saved workspaces
are imported automatically; their original settings file is preserved.

Project changes save automatically. Live terminal sessions survive project
switches while the app stays open; terminals start fresh after quitting the app.
Projects persist local tabs; remote tabs remain available during the running
session. Unavailable local folders are skipped when restoring.

The preview pane appears only for a single selected file with an available
preview. Text files use a readable 14 pt text view in a 420 pt scrolling pane.
Large text previews are bounded, and show a notice when shortened.

Right-click the file area and choose **New Text File…**. Enter a name; `.txt` is
added automatically. Cancel creates nothing, and existing files are never replaced.
The eye beside the trash icon toggles hidden files and folders for that tab;
hidden items are hidden by default. The setting is included in project layouts.
**Copy** or **⌘C** shows the copied item’s name at the bottom of its pane.

The terminal supports interactive tools, ANSI colors, scrollback, text selection,
**⌘C/⌘V**, and a draggable divider. **Hide** keeps the session running; closing its
file tab ends the session. It uses your normal zsh startup files. The old one-shot
command panel and its settings have been removed.

| Shortcut | Action |
|---|---|
| ⌘P | Search files and commands |
| ⌘⇧S | New project |
| ⌘⇧D | Split or join panes |
| ⌘⇧I | Show or hide preview |
| ⌘T | New tab |
| ⌘V | Open a pasted path or paste copied files |
| ⌘C | Copy selected files or folders |
| ⌘, | Settings |

## Build

Requires Rust stable and Xcode with its Metal compiler. The packaged app targets
macOS 14 or later and is built for the current Mac's processor.

```sh
./build.sh
open "Getter.app"
```

For development: `STUDIO_BUILD_PROFILE=debug ./build.sh`. The app is locally
ad-hoc signed; public distribution and Apple notarization are not configured.
`./make_dmg.sh` packages an existing build without publishing it.

```sh
./test.sh
```

Normal settings live in `~/Library/Application Support/Getter/`. Existing
Shuffle Studio settings are reused automatically if present.
This prepared workspace reuses its local compiler cache in offline mode.
If adding a new dependency, use `CARGO_NET_OFFLINE=false ./build.sh` to fetch it.
For native UI checks, `SHUFFLE_STUDIO_CONFIG_DIR`, `SHUFFLE_STUDIO_START_DIR`
and `SHUFFLE_STUDIO_INDEX_ROOT` can point at disposable test folders.

## Source

- `src/theme.rs`: palette and live theming
- `src/studio.rs`: custom toolbar, footer and project controls
- `src/projects.rs`: project layouts, favorites, migration and validation
- `src/entry_filter.rs`: per-tab folder/file visibility
- `src/clipboard_path.rs`: pasted-path interpretation
- `src/new_text_file.rs`: named text-file creation without overwriting
- `src/text_preview.rs`: bounded native text previews
- `src/studio_terminal.rs`, `src/terminal-shell/`: PTY sessions and folder synchronization
- `src/main.rs`: the existing GPUI file browser and file operations
- `scripts/package_icon.sh`: package the Schnauzer artwork as a Mac app icon

The original snapshot is tagged `studio-starting-point`; the working branch is
`studio`. The `upstream` remote is retained for provenance. This app does not
check for or install original Shuffle updates.

Based on Shuffle by Jaime Guzman, under the [MIT license](LICENSE).
The original README is preserved in [README.upstream.md](README.upstream.md).

The embedded terminal uses [gpui-terminal](https://github.com/zortax/gpui-terminal)
by Leonard Seibold (MIT or Apache 2.0), [Alacritty's terminal engine](https://github.com/alacritty/alacritty),
and [portable-pty](https://github.com/wezterm/wezterm/tree/main/pty). Its local component
adds scrollback, selection, clipboard handling, terminal query responses and shell
integration; original license files are retained under `vendor/gpui-terminal`.
