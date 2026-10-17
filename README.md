# Schnauz

A personal Mac file manager based on [Shuffle](https://github.com/WizenPainter/shuffle).
This is an independent local version, with its own application identity and settings.

Design and product direction: **Oliver Weber**. Implementation of the Schnauz
adaptations: **Codex**. Original codebase: **Shuffle by Jaime Guzman (MIT)**.
See [CREDITS.txt](CREDITS.txt) for the full attribution.

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
- The existing native file browser, file operations and Mac integrations.

The top search bar defaults to **Current Folder + File**. Current Folder includes
subfolders; Drive searches the containing volume. Names match literal parts without
fuzzy guesses, ignoring case. Spaces mean AND: `test .txt` requires `test` in the
name and `.txt` at the end. **Content** checks only text content, supports UTF-8 and
BOM-marked UTF-16, and never matches by name. Search runs only after typing, reports
results progressively and cancels when changed or closed. Hidden-item visibility
follows the active tab. Results are capped at 500 with a notice to refine the query.
Results use the main file pane, with Name and Location columns. Click to select,
preview or copy; double-click to open. **Show in Folder** or Enter reveals the
selected item in the browser. Type/hidden filters also apply to search results.
The small Schnauz beside the app name sits when idle and runs while searching.
Click the running Schnauz to stop and keep results. The status bar then shows
**Search stopped · Results may be incomplete** until the next search.
Escape or **Done ×** closes search and
restores Current Folder + File. No home-folder index is built at launch.

Right-click a folder, empty folder area or sidebar folder and choose **Inspect…**
for an on-demand size map on the right. The window widens when the display has room.
Rectangle area represents file sizes, with colors grouped by file type. Click a
folder heading to open it on the left and zoom; click a file tile to reveal and
select it on the left. Use ↑ to zoom out or Refresh to scan again. The map includes
hidden items and uses metadata without reading file contents. Directory symlinks
and other volumes are not traversed. Permission failures or the 500,000-item memory
limit show an explicit partial-map notice. Closing the panel cancels the scan.

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
preview. Drag its left edge to change its width; the setting is remembered.
Double-click the edge to reset to 420 pt. Text files use readable 14 pt text.
Large text previews are bounded, and show a notice when shortened.

Lists, search results and columns render visible rows and request their icons
as needed. Column listings share entries and cache visibility masks. Icon,
preview and listing caches have limits; obsolete local preview jobs are debounced
and cancelled. Visible local folders use macOS change notifications, with an
adaptive fallback when notifications are unavailable. Ordinary file selections
do not launch code-signature checks. The old home index and its direct scanning
dependencies have been removed.

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
| ⌘P | Focus file search |
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
open "Schnauz.app"
```

For development: `STUDIO_BUILD_PROFILE=debug ./build.sh`. The app is locally
ad-hoc signed; public distribution and Apple notarization are not configured.
`./make_dmg.sh` packages an existing build without publishing it.

```sh
./test.sh
```

Normal settings live in `~/Library/Application Support/Schnauz/`. Existing
Getter or Shuffle Studio settings are reused automatically if present.
This prepared workspace reuses its local compiler cache in offline mode.
If adding a new dependency, use `CARGO_NET_OFFLINE=false ./build.sh` to fetch it.
For native UI checks, `SCHNAUZ_CONFIG_DIR` and `SCHNAUZ_START_DIR` can point at
disposable test folders.

## Source

- `src/theme.rs`: palette and live theming
- `src/studio.rs`: custom toolbar, footer and project controls
- `src/projects.rs`: project layouts, favorites, migration and validation
- `src/entry_filter.rs`: per-tab folder/file visibility
- `src/clipboard_path.rs`: pasted-path interpretation
- `src/new_text_file.rs`: named text-file creation without overwriting
- `src/text_preview.rs`: bounded native text previews
- `src/previews.rs`: cancellable preview jobs, metadata and bounded preview caches
- `src/preview_panel.rs`: preview divider dragging, limits and remembered width
- `src/cache.rs`: weighted LRU storage for icons, previews and shared listings
- `src/folder_watch.rs`: macOS filesystem notifications and adaptive fallback
- `src/columns.rs`: shared column entries and cached visibility masks
- `src/settings.rs`: settings window and controls
- `src/mascot.rs`: native sitting/running Schnauz animation and stop control
- `src/file_search.rs`, `src/search_ui.rs`: scoped name/content search and its toolbar
- `src/space_map.rs`, `src/space_ui.rs`: explicit folder scans and interactive size map
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
