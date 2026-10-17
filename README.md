# Schnauz

A personal Mac file manager based on [Shuffle](https://github.com/WizenPainter/shuffle).
This is an independent local version, with its own application identity and settings.

For continuing development with Codex CLI, start with [PROJECT.md](PROJECT.md)
and the repository's [AGENTS.md](AGENTS.md).

Developer, design and product direction: **Oliver Weber**. Implementation
assistance: **Codex**. Original codebase: **Shuffle by Jaime Guzman (MIT)**.
See [CREDITS.txt](CREDITS.txt) for the full attribution.

## About and Cookie

**Schnauz → About Schnauz**, or clicking the sitting Schnauzer beside the app name,
shows the developer credit, original code attribution,
credits and bundled licenses. Optional **Coffee for Cookie · US$2.50** support opens
PayPal for **o@freedom-net.de**; the address can also be copied. This is voluntary
support for Oliver and his schnauzer Cookie. The button uses PayPal's documented
[Payments Standard variables](https://developer.paypal.com/api/nvp-soap/paypal-payments-standard/integration-guide/Appx-websitestandard-htmlvariables/)
with a suggested USD 2.50 amount; the user completes any payment on PayPal.

## First version

- Studio Light and Studio Dark appearances, an indigo accent, and compact 12 pt lists.
- Folder totals are not calculated automatically; the size column shows file sizes only.
- Applications display their native icons and are labeled **Application**. Click to
  select, double-click to launch, or right-click **Show Package Contents** to browse inside.
  App bundles appear under **Files**, while **Folders** shows ordinary directories.
- **All · Folders · Files** controls for every tab. Active filters tint the top bar,
  show the visible item count, and are remembered in sessions and projects.
- Search, split/join panes, preview and project controls in the toolbar.
- **List** and **Icons** with matching vector toolbar controls. Right-click a list
  header or use **View options → List columns** to choose columns. Drag headers to
  reorder and their edges to resize. Wide lists scroll horizontally.
- View, columns, order, widths, preview width and sorting are remembered for each local folder.
  **Apply view to all subfolders** sets one inherited rule; later folder-specific
  changes override it. The folder context menu applies the current view to that
  folder and its descendants. One central `folder-views.json` is loaded once, with
  in-memory ancestor lookups and background atomic writes. No directory scan or
  per-folder settings files. **Use inherited / default view** removes that folder's
  rule. Applying a tree rule replaces existing child overrides.
- Projects with separate locations, tabs, view modes, sorting and split layouts. Changes are remembered automatically.
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

**SSH folder following:** start a new integrated terminal and use normal
`ssh host` (including SSH aliases, keys, ports and jump hosts). On remote Bash or
Zsh, the browser above follows the remote shell's folder, using SFTP over the same
authenticated connection. The breadcrumb and terminal header identify the server.
This is **terminal → browser only**; browsing never sends commands or keypresses
into SSH. Click **Synced** to detach and return to the original Mac folder without
ending SSH or a remote CLI; click **Unsynced** to follow again. SSH exit returns
the browser to local browsing; an already detached local folder stays as chosen.
Remote prompt integration uses a private temporary folder for that session and
leaves remote dotfiles unchanged. The server must allow SFTP. Existing terminal
sessions need a fresh shell to load the new integration. Commands (`ssh host cmd`),
tunnels, custom RemoteCommand sessions and user-defined `ssh` functions use their
normal behavior. `command ssh …` explicitly bypasses integration. Nested SSH,
existing tmux sessions and other remote shells do not automatically report folders.
The authenticated transport is not reused after its SSH session ends. SFTP
listings reject paths containing line breaks instead of interpreting extra commands.

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
Click the sitting Schnauz to open **About Schnauz / Coffee for Cookie**. During a
search the running Schnauz stops the scan and keeps results instead. The status bar then shows
**Search stopped · Results may be incomplete** until the next search.
Escape or **Done ×** closes search and
restores Current Folder + File. No home-folder index is built at launch.

Click **Speicherbelegung** in the filter row, or right-click a folder, empty folder
area or sidebar folder and choose **Speicherbelegung…**
for an on-demand size map on the right. The window widens when the display has room.
Rectangle area represents file sizes, with colors grouped by file type. Click a
folder heading to open it on the left and zoom; click a file tile to reveal and
select it on the left. Use ↑ to zoom out or Refresh to scan again. The map includes
hidden items and uses metadata without reading file contents. Directory symlinks
and other volumes are not traversed. Permission failures or the 500,000-item memory
limit show an explicit partial-map notice. Closing the panel cancels the scan.
The map starts below the path and filter bars. Selecting outside it closes it;
navigation from inside its graphic keeps it open.

Click **New project** or press **⌘⇧S** to create a project from the current tabs.
Click a project in the sidebar to switch to its own tagged items, locations, cloud folders, servers and last layout.
**Locations** is the single place for project shortcuts. Use its **+** to add a
folder, or right-click a file/folder and choose **Add to Locations**. The separate
Favorites section and its code are removed; existing favorites move into Locations
without duplicates, including files and currently unavailable paths.
**Personal** keeps its own browsing tabs and locations.
New projects start with empty **Locations**, **Cloud**, **Servers**
and **Tagged Items**. Add folders with **+**; servers can also include explicit
network-folder shortcuts. **Recents** has been removed, including history tracking.
The little **Tag** toolbar button adds/removes selected local files and folders
from the current project's **Tagged Items**. Click a tagged file to reveal it in
its folder. Right-click a sidebar folder and choose **View in… → All projects** or
an individual project; checkmarks indicate membership, and clicking again removes
it there. This changes shortcuts only; source files stay in place. All projects
means the projects that currently exist (including Personal); future projects
still start empty. Tags are project membership, stored centrally rather than in
Finder tags or file metadata. Existing saved workspaces are imported automatically;
their original settings file is preserved. Legacy saved servers migrate once to
**Personal**; passwords remain in the macOS Keychain. Removing a project connection
keeps shared host credentials available for other projects.

**Devices** stays above the scrolling project sections and is shared by every
project. Each mounted local drive shows a compact free-space label: green below
80% used, yellow at 80–90%, red above 90%. Change those thresholds in **Settings →
General → Sidebar**. Capacity refreshes in the background every 30 seconds and
never scans folders. Collapsing the sidebar leaves only its reopen button.

Inside Nextcloud, iCloud Drive and other macOS CloudStorage folders, the path bar
shows a blue provider badge followed by the cloud-relative folder path. The badge
stays visible even when the path is long; click it to open the cloud root. Hover
or edit the path to see the full filesystem location.

Cloud files, folders and shortcuts have a provider submenu. Downloads and iCloud
space controls run directly. **Nextcloud actions in Finder…** selects the item
in Finder for Nextcloud's own sharing, locking and automatic-space menus.
Those Finder extension actions are not exposed as a reusable menu for other file
managers; a separate direct Nextcloud connection is still under discussion.
See [Apple's Finder Sync guide](https://developer.apple.com/library/archive/documentation/General/Conceptual/ExtensibilityPG/Finder.html)
and [Nextcloud's File Provider guide](https://docs.nextcloud.com/server/stable/user_manual/en/desktop/macosfileprovider.html).
Opening a cloud menu never recursively scans a folder.

Project changes save automatically. Live terminal sessions survive project
switches while the app stays open; terminals start fresh after quitting the app.
Projects persist local tabs; remote tabs remain available during the running
session. Unavailable local folders are skipped when restoring.

The preview pane appears only for a single selected file with an available
preview. It starts below the full-width path and filter bars. Drag its left edge
to change its width; the setting is part of that folder's saved view and inherited
folder-tree rules. Double-click the edge to reset to 420 pt. Photos and videos scale
to the full preview width, including smaller originals; tall media scrolls.
Local clips use native AVKit playback with seeking, Play/Pause and Unmute/Mute.
Each new clip starts paused and muted, and leaving its preview stops playback.
Supported codecs follow macOS; unsupported videos show an explanation.
Text files use readable 14 pt text.
Large text previews are bounded, and show a notice when shortened.

File timestamps follow the macOS locale, time zone and 12/24-hour setting using
the native date formatter. The Share control uses Apple's `square.and.arrow.up`
SF Symbol. The About panel includes a painted portrait of the real Cookie.


Lists and search results render visible rows and request their icons
as needed. Icon and preview caches have limits; obsolete local preview jobs are debounced
and cancelled. Visible local folders use macOS change notifications, with an
adaptive fallback when notifications are unavailable. Ordinary file selections
do not launch code-signature checks. The old home index and its direct scanning
dependencies have been removed.

Right-click the file area and choose **New Text File…**. Enter a name; `.txt` is
added automatically. Cancel creates nothing, and existing files are never replaced.
The eye beside the trash icon toggles hidden files and folders for that tab;
hidden items are hidden by default. The setting is included in project layouts.
Visibility/view changes retain selections that remain visible. Native macOS context,
sort and sidebar menus can extend beyond the app window.
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
- `src/projects.rs`: project layouts, locations, migration and validation
- `src/volumes.rs`, `volumectl.swift`: global drive capacity and colour badges
- `src/cloud_locations.rs`, `src/cloud_actions.rs`: cloud identity, paths and provider menus
- `src/entry_filter.rs`: per-tab folder/file visibility
- `src/clipboard_path.rs`: pasted-path interpretation
- `src/new_text_file.rs`: named text-file creation without overwriting
- `src/text_preview.rs`: bounded native text previews
- `src/previews.rs`: cancellable preview jobs, metadata and bounded preview caches
- `src/preview_panel.rs`: preview divider dragging, limits and remembered width
- `src/cache.rs`: weighted LRU storage for icons, previews and shared listings
- `src/folder_watch.rs`: macOS filesystem notifications and adaptive fallback
- `src/folder_views.rs`: central folder rules, inheritance and column layouts
- `src/list_ui.rs`: configurable list headers and folder view controls
- `src/toolbar_icons.rs`: native vector icons
- `src/native_menus.rs`: screen-level AppKit menus
- `src/settings.rs`: settings window and controls
- `src/about.rs`: developer credits, licenses and optional Cookie support
- `src/mascot.rs`: native sitting/running Schnauz animation and stop control
- `src/file_search.rs`, `src/search_ui.rs`: scoped name/content search and its toolbar
- `src/space_map.rs`, `src/space_ui.rs`: explicit folder scans and interactive size map
- `src/studio_terminal.rs`, `src/terminal-shell/`: PTY sessions and shell integration
- `src/ssh_sync.rs`: connection-bound SSH folder following and return to local browsing
- `src/main.rs`: the existing GPUI file browser and file operations
- `scripts/package_icon.sh`: package the Schnauzer artwork as a Mac app icon

The original snapshot is tagged `studio-starting-point`; the working branch is
`studio`. The `upstream` remote is retained for provenance. This app does not
check for or install original Shuffle updates.

Based on Shuffle by Jaime Guzman, under the [MIT license](LICENSE).
The original README is preserved in [README.upstream.md](README.upstream.md).

Commercial distribution, including a possible €5 edition, was discussed. The
base [MIT license](https://opensource.org/license/mit) permits commercial sale
without requiring publication of our Schnauz adaptations. Preserve the original
copyright and license notices. The price is an option, not a release commitment.

Other components retain their own license conditions. In particular,
`option-ext` 0.2.0 uses MPL-2.0: distribution requires access to that component's
source and a notice explaining how to obtain it; this does not require opening
our separate Schnauz implementation. See the [Mozilla MPL FAQ](https://www.mozilla.org/en-US/MPL/2.0/FAQ/).
Complete the dependency notices and source-access information before external
distribution; the current bundle contains selected third-party license texts.

The embedded terminal uses [gpui-terminal](https://github.com/zortax/gpui-terminal)
by Leonard Seibold (MIT or Apache 2.0), [Alacritty's terminal engine](https://github.com/alacritty/alacritty),
and [portable-pty](https://github.com/wezterm/wezterm/tree/main/pty). Its local component
adds scrollback, selection, clipboard handling, terminal query responses and shell
integration; original license files are retained under `vendor/gpui-terminal`.
