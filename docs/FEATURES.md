# Schnauz feature guide

A personal Mac file manager based on [Shuffle](https://github.com/WizenPainter/shuffle).
This is an independent local version, with its own application identity and settings.

For continuing development with Codex CLI, start with [PROJECT.md](../PROJECT.md)
and the repository's [AGENTS.md](../AGENTS.md).

Developer, design and product direction: **Oliver Weber**. Implementation
assistance: **Codex**. Original codebase: **Shuffle by Jaime Guzman (MIT)**.
See [CREDITS.txt](../CREDITS.txt) for the full attribution.

## About and Cookie

**Schnauz → About Schnauz**, or clicking the sitting Schnauzer beside the app name,
shows the developer credit, original code attribution,
credits and bundled licenses. **Schnauz website** opens
[the app page](https://techguyzone.com/apps/schnauz/), and **GitHub** opens
[the public source repository](https://github.com/weiaoli2/Schnauz). Links open
only when clicked. Optional **Coffee for Cookie · US$2.50** support opens
PayPal for **o@freedom-net.de**; the address can also be copied. This is voluntary
support for Oliver and his schnauzer Cookie. The button uses PayPal's documented
[Payments Standard variables](https://developer.paypal.com/api/nvp-soap/paypal-payments-standard/integration-guide/Appx-websitestandard-htmlvariables/)
with a suggested USD 2.50 amount; the user completes any payment on PayPal.

## First version

- Schnauz Light and Schnauz Dark appearances, an indigo accent, and compact 12 pt lists.
- A fixed Schnauzer app icon and native macOS file/application icons.
- Folder totals are not calculated automatically; the size column shows file sizes only.
- One click selects a file or folder; double-click opens it. Click the sole selected
  item's name again to rename it inline in List, Icons or search results. The
  short delay follows your Mac's double-click setting. Enter saves, Escape cancels.
- Applications display their native icons and are labeled **Application**. Click to
  select, double-click to launch, or right-click **Show Package Contents** to browse inside.
  App bundles appear under **Files**, while **Folders** shows ordinary directories.
- **All · Folders · Files** controls for every tab. Active filters tint the top bar,
  show the visible item count, and are remembered in sessions and projects.
- **Reload folder** beside those controls re-reads the current local or SSH folder
  (⌘R), using the existing refresh action without restarting terminals.
- Search, split/join panes, preview and project controls in the toolbar.
- The **globe button immediately right of Search** connects a Windows/NAS **SMB**
  drive (or AFP). Enter the server name/IP or a share URL, username and password,
  or choose Guest; macOS lets you select the share. Optional read-only access and
  a folder picker are available. Each connected share/folder opens in a new tab.
  Add that tab to Favourites to retain its server/share, user and relative folder.
  **Remember password in Keychain** is enabled by default; passwords stay in
  Schnauz's macOS Keychain entry, outside project/session files. Reopening a
  favourite reconnects even if the mount name changes. Restored disconnected tabs
  offer Retry; login dialogs do not open automatically at startup. Shares remain
  mounted when a tab closes; eject them using Finder when finished.
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
- Projects with separate favourites, tabs, view modes, sorting and split layouts. Changes are remembered automatically.
- A preview panel and footer with item counts, centered status and appearance control.
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

**Start a terminal in an SSH folder:** click Terminal while browsing a remote
folder to open a new interactive SSH PTY in that directory. It uses the saved SSH
profile and can open an additional channel over an already authenticated master.
The original transport/session is not replaced; if a live-only mux is gone, the
new channel fails closed instead of silently authenticating again. Fresh SSH
connections can show their normal password/passphrase prompts inside the new
terminal. Initial `cd` belongs only to this new session's startup; later browser
navigation never sends input into it. A terminal already present is preserved.

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

Click **Disk Usage** in the filter row, or right-click a folder, empty folder
area or sidebar folder and choose **Disk Usage…**
for an on-demand size map on the right. The window widens when the display has room.
Rectangle area represents file sizes, with colors grouped by file type. Click a
folder heading to open it on the left and zoom; click a file tile to reveal and
select it on the left. Use ↑ to zoom out or Refresh to scan again. The map includes
hidden items and uses metadata without reading file contents. Directory symlinks
and other volumes are not traversed. Permission failures or the 500,000-item memory
limit show an explicit partial-map notice. Closing the panel cancels the scan.
The map starts below the path and filter bars. Selecting outside it closes it;
navigation from inside its graphic keeps it open.

**File access:** On first launch, Schnauz shows a skippable guide. **Open System
Settings** opens macOS **Privacy & Security → Full Disk Access**. Enable Schnauz
there, or use **+** to add this app, then restart it when convenient. You can
continue without it, with some folders restricted. The guide remains under
**Settings → General → File Access**. Acknowledging it does not grant access.

Click **+ beside Projects** or press **⌘⇧S** to create a project from the current tabs.
Each project keeps its own **Favourites**, connection settings, Todoist link and
last tabs/layout. **Personal** has its own profile. The shared **Common** list
appears above Favourites in every project. It initially includes **Home**, **Desktop**
and **Downloads**. Desktop and Downloads can be removed; Home stays pinned first
under Common → Folders with a blue badge and cannot be removed. Removed defaults
stay removed after restarting. Common folders/files are stored once centrally.
New projects otherwise start empty. All Schnauz windows share the active project;
changing projects in one changes it everywhere. Each window can browse its own
folder within that project. Folder context menus offer **Open**, **Open in New
Tab** and **Open in New Window**, including SSH folders and sidebar favourites.
New windows open the chosen folder without duplicating terminal sessions.

Right-click a saved folder or file in Favourites or Common and choose **Edit alias and
add-on…**. **Alias** replaces its name in the sidebar; leave it empty to
keep the original name. **Add-on** is optional text shown in brackets after the
name, for example `Reports (Archive)` or `Work (Server)`. These labels belong to
every matching reference across Common and all projects; the actual path stays
the same. Hover over any folder tab to see its full path (and server for SSH tabs).

**Weblinks** stores named bookmarks in both Common and each project's Favourites.
Right-click **COMMON** or **FAVOURITES**, choose **Add Weblink…**, enter a name and complete http(s) link, and choose either
**macOS Default** or a specific installed browser for that bookmark. Click a link
to open it; right-click for **Edit weblink…**, **Copy link**, or **Remove weblink**.
Editing also changes its browser choice. Drag bookmarks to reorder them. Browser
choices and bookmark order persist centrally; system defaults stay unchanged.
A missing chosen browser asks you to edit the link rather than switching browsers.

The search field aligns with the Schnauz title. Below it, a horizontal row lists
projects with open tabs or terminals across your windows. The current project
has a blue background; click another project to switch. Sessions stay open when
you switch, and the row scrolls horizontally when needed.

When you quit Schnauz, it checks for active processes in project terminals,
including background project sessions and jobs. A warning identifies the projects
and processes, with **Cancel** and **Quit Anyway**. Idle shells alone do not prompt.
This also applies to Quit from the Dock; checking never sends terminal commands.

The reload button beside **All / Folders / Files** appears only for SSH/SFTP.
Local and locally synced cloud folders keep automatic updates; **⌘R** remains
available for a manual refresh in every folder.

Click the **PROJECTS** or **COMMON** heading to collapse/expand that group. The
heading stays visible, with no extra icon, and its state is remembered. Dropping
a reference on collapsed Common expands it to show the added item.

Drag a folder tab onto Common to share its folder across projects, or elsewhere
in the expanded sidebar to add it to the captured project's Favourites. The tab
and its terminal remain open. SSH tabs keep their server identity; these drops
add references without transferring files or connecting to servers.

Drag projects to change their order. Drag references within a Common or
Favourites Folders/Files group to reorder them; a thin insertion line marks the
position. Orders are saved across restarts. Personal and Home stay pinned, and
the section order stays fixed. These gestures rearrange references without moving
files. Single-click a sidebar file to reveal it; double-click to open it with its
configured app (remote files download before opening).

A small green dot before a project icon means it has live tabs or terminals in
any Schnauz window, including background project sessions. A saved layout alone
does not light the dot.

Right-click an open project in the sidebar or top project bar and choose **Close**
to close its tabs and terminals in every window. Its green dot and top-bar chip
disappear; the saved project, favourites and settings remain. Closing the selected
project switches to the next open project in sidebar order, wrapping at the end.
Closing the last tab in a window also switches projects and discards that window's
outgoing tabs; other windows retain their own tabs. With no other open project,
browsing returns to a fresh Personal/Home tab.

The compact sidebar shows **Devices**, **Projects**, **Common**, then **Favourites**.
Common and Favourites group references under **Folders / Files / Weblinks**,
hiding empty subsections and keeping **Add Weblink…** in the group heading’s context menu, with no placeholder
messages or + button beside Favourites. Its width follows the visible names,
with a cap for very long names and full paths in tooltips. Project rows no longer show tab counts. Cloud, Servers,
Locations and Tagged Items are replaced by this single favourites list.

Drop local or SSH folders/files on Common to share them across all projects, or
elsewhere in the expanded sidebar for project Favourites: Schnauz sorts
them into **Folders** or **Files**, without moving, copying or downloading them.
**Drop to add to Favourites** identifies the target. Duplicates are kept once,
and identical paths on different servers stay separate. Escape or dropping
outside cancels an SSH reference drag. Use the **Tag** button or
**Add to Favourites** in the context menu as alternatives to dragging. Existing
shortcuts migrate without dropping unavailable paths.

The **Tag** button toggles project favourites for local and remote selections.
Click a file favourite to reveal it; a folder opens in the browser. A small cloud
badge identifies Nextcloud, iCloud, OneDrive and other File Provider folders;
network/SSH items carry a globe. SSH favourites retain host/user/port and their
path. Clicking one reuses the authenticated SSH connection when available, in a
separate browser tab from its terminal; terminal sessions and commands stay intact.
After restart a saved favourite can reconnect through the normal SSH settings.
**Go → Connect to Server…** / **⌘K** remains available; saved connections stay in
Settings. Cloud and network icons never initiate a connection.

Right-click any favourite and choose **View in… → All projects** or an individual
project. Checkmarks show membership. This changes references only; All includes
current projects and Personal, while Common also applies to future projects automatically. The file
context menu **Tags** separately applies Finder colour labels, shown as coloured
dots in the native menu.
Existing saved workspaces are imported automatically;
their original settings file is preserved. Legacy saved servers migrate once to
**Personal**; passwords remain in the macOS Keychain. Removing a project connection
keeps shared host credentials available for other projects.

**Devices** stays above the scrolling project sections and is shared by every
project. Click a drive's compact free-space badge to switch all drives between
**GB free** and **percent free**; the choice is saved. Clicking the badge keeps
your current folder open. Its colour reflects used capacity: green below
80% used, yellow at 80–90%, red above 90%. Change those thresholds in **Settings →
General → Sidebar**. Capacity refreshes in the background every 30 seconds and
never scans folders. Collapsing the sidebar leaves only its reopen button.

Inside Nextcloud, iCloud Drive and other macOS CloudStorage folders, the path bar
shows a blue provider badge followed by the cloud-relative folder path. The badge
stays visible even when the path is long; click it to open the cloud root. Hover
or edit the path to see the full filesystem location.

Cloud files, folders and shortcuts have a provider submenu. Downloads and iCloud
space controls run directly. Connect Nextcloud in **Settings → General →
Connections → Connect folder…**, or right-click an item in a Nextcloud CloudStorage
folder and choose **Nextcloud → Connect this folder to Nextcloud…**. Enter the
server address and the remote folder corresponding to the selected local root
(usually `/`), then approve Schnauz in your browser. This also supports classic
Nextcloud sync folders outside CloudStorage. Each linked root gets the blue badge.

Schnauz obtains its own app credential through
[Nextcloud's browser login flow](https://docs.nextcloud.com/server/stable/developer_manual/client_apis/LoginFlow/index.html)
and stores it in macOS Keychain; your desktop client's account remains independent.
Only folder/server/account metadata goes into the central `nextcloud.json` file.
Disconnect removes Schnauz's mapping and tries to revoke its own credential. If
the server is unreachable, revoke it later in Nextcloud's personal Security settings.

The connected **Nextcloud** submenu offers **Share options…**, **Open in browser**
and **Copy internal link**. Share options lists existing shares; create a read-only
public link with optional password/expiry, share with a Nextcloud user/group ID,
change collaborator access, copy a public link or remove a selected share. Merely
opening the window creates nothing. Advanced sharing options remain in the browser.
**Lock file / Unlock file** requires the server's
[Files Locking app](https://github.com/nextcloud/files_lock#capabilities-api).
Schnauz releases only user-owned locks confirmed as belonging to your account.
Use **Refresh server features** after enabling server capabilities.

**Local storage policy in Finder…** still selects the item in Finder for
Nextcloud's automatic freeing-up-space setting. macOS does not expose another
provider's Finder extension menus as a reusable menu for other file managers.
See [Apple's Finder Sync guide](https://developer.apple.com/library/archive/documentation/General/Conceptual/ExtensibilityPG/Finder.html)
and [Nextcloud's File Provider guide](https://docs.nextcloud.com/server/stable/user_manual/en/desktop/macosfileprovider.html).
Opening a cloud menu never recursively scans a folder.
Account and file requests run outside the browser's render path. `./test.sh` tests
login, paths, DAV/OCS sharing, locking and atomic settings entirely with fixtures;
actual login and server compatibility require a manual test with your account.

Project changes save automatically. Live terminal sessions survive project
switches while the app stays open; terminals start fresh after quitting the app.
Projects persist local tabs; remote tabs remain available during the running
session. Unavailable local folders are skipped when restoring.

The **Todoist** button appears after connection and opens Tasks beside the file pane. Connect under
**Settings → General → Connections → Todoist**, using an API token entered in a
secure native field and stored in Keychain. A Schnauz project uses the same name
as its Todoist project: a unique existing match is linked, while a newly created
Schnauz project creates the Todoist counterpart only if no match exists. Duplicate
Todoist names get a chooser. Offline creation intents wait until connected.

Todoist opens a project-specific Tasks pane on the right after connection.
Use the **Todoist** toolbar button or **Settings → General → Connections →
Todoist → Show tasks** to reopen it. Todoist and Disk Usage share a framed tools
pane with its own saved width. File preview remains visible independently. In
all views the order is files → preview → Todoist / Disk Usage. Preview takes
space from the file list without enlarging the window. Tools instead extend the
window to the right when display space permits, and share one extension when
switching between them. Closing tools removes that extension while preserving
manual window resizing. Preview and tools each resize from their left edge;
changing the tools divider redistributes existing space. The outer right border
only resizes the native app window. Panels remain side by side at screen edges or in full screen.

Selecting another Schnauz project switches Tasks with it. Existing unlinked
projects first search by name; **Find or create matching project** can create a
missing counterpart. Tasks show completion circles, fine separators and due dates
below their titles, plus named sections/subtasks. The indigo **+ Add task** row
stays at the end of the list even when no tasks exist. Click the circle
to complete, the task text to open Todoist, or **Add task…** for a compact native
composer with Schnauz colours and the little toolbar dog. The fixed project cannot
be changed. Add a title, description, date, deadline, priority and labels; optional
time/location reminders depend on the Todoist account. Location search uses Apple
Maps only after an explicit address search. A task is sent to the project where
its dialog was opened, even if you switch projects. Failed forms keep their text;
retrying an optional reminder after a task was saved cannot create a second task.
**Open in Todoist** and task links prefer the native app, falling back to the browser.
Cached tasks remain visible during connection errors. Only visible rows render,
and refresh runs in the background while Tasks is open. **Preview** toggles only
file previews. Todoist is connected and working in Oliver's personal build;
automated tests use fixtures without live service or Keychain access.

The preview pane appears only for a single selected file with an available
preview. It starts below the full-width path and filter bars. Drag its left edge
to change its width; the setting is part of that folder's saved view and inherited
folder-tree rules. Double-click the edge to reset to 420 pt. Photos and videos scale
to the full preview width, including smaller originals; tall media scrolls.
Local clips use native AVKit playback with seeking, Play/Pause and Unmute/Mute.
Each new clip starts paused and muted, and leaving its preview stops playback.
Supported codecs follow macOS; unsupported videos show an explanation.
Text uses the same 12 pt size as file names. Select text with the mouse, use
**⌘A** to select all and **⌘C** to copy. The preview is read-only, and its selection
and scroll position survive resizing. Large text previews are bounded, with a
notice when shortened. Terminal and Preview retain labeled toolbar buttons.

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

For development: `SCHNAUZ_BUILD_PROFILE=debug ./build.sh`. Packaging uses a stable
Apple Development or Developer ID signing identity from the local Keychain,
preserving the previous signer when available. With several certificates, choose
one using `SCHNAUZ_CODESIGN_IDENTITY="certificate name or SHA-1" ./build.sh`.
Missing/ambiguous identities fail before replacing the existing app. Temporary
ad-hoc signing requires explicit `SCHNAUZ_CODESIGN_IDENTITY=- ./build.sh`; such
builds can lose macOS permission grants when their contents change.

A change from earlier ad-hoc builds requires granting access to the newly signed
app once: quit Schnauz when ready, remove its old Full Disk Access entry, add the
current Schnauz.app and reopen. Subsequent builds keep the signing identity.
Packaging compiles and verifies a staged bundle before replacing the existing
app; it never restarts a live session. Public release candidates are hosted on [GitHub](https://github.com/weiaoli2/Schnauz/releases).
Apple notarization is not yet configured. `./make_dmg.sh` packages an existing build without publishing it.

```sh
./test.sh
```

Normal settings live in `~/Library/Application Support/Schnauz/`. Existing
settings from earlier builds are reused automatically if present.
This prepared workspace reuses its local compiler cache in offline mode.
If adding a new dependency, use `CARGO_NET_OFFLINE=false ./build.sh` to fetch it.
For native UI checks, `SCHNAUZ_CONFIG_DIR` and `SCHNAUZ_START_DIR` can point at
disposable test folders.

## Source

- `src/theme.rs`: palette and live theming
- `src/schnauz.rs`: custom toolbar, footer and project controls
- `src/projects.rs`: project layouts, local/remote favourites, migration and validation
- `src/volumes.rs`, `volumectl.swift`: global drive capacity and colour badges
- `src/cloud_locations.rs`, `src/cloud_actions.rs`: cloud identity, paths and provider menus
- `src/entry_filter.rs`: per-tab folder/file visibility
- `src/clipboard_path.rs`: pasted-path interpretation
- `src/new_text_file.rs`: named text-file creation without overwriting
- `src/text_preview.rs`: bounded text decoding; `src/text_view.rs`: native read-only selection/copying
- `src/native_preview.rs`: shared clipping geometry for native text and video
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
- `src/schnauz_terminal.rs`, `src/terminal-shell/`: PTY sessions and shell integration
- `src/ssh_sync.rs`: connection-bound SSH folder following and return to local browsing
- `src/main.rs`: the existing GPUI file browser and file operations
- `scripts/package_icon.sh`: package the Schnauzer artwork as a Mac app icon

The original snapshot is tagged `schnauz-starting-point`; the working branch is
`schnauz`. The `upstream` remote is retained for provenance. This app does not
check for or install original Shuffle updates.

Based on Shuffle by Jaime Guzman, under the [MIT license](../LICENSE).

The original MIT license and credits are retained. Full dependency notices are
in [THIRD_PARTY_NOTICES.txt](../THIRD_PARTY_NOTICES.txt). Unmodified MPL-2.0
component sources accompany release downloads; see [distribution details](RELEASING.md).

The embedded terminal uses [gpui-terminal](https://github.com/zortax/gpui-terminal)
by Leonard Seibold (MIT or Apache 2.0), [Alacritty's terminal engine](https://github.com/alacritty/alacritty),
and [portable-pty](https://github.com/wezterm/wezterm/tree/main/pty). Its local component
adds scrollback, selection, clipboard handling, terminal query responses and shell
integration; original license files are retained under `vendor/gpui-terminal`.

### Folder access and path editing

Folder changes reset scroll positions and refresh the visible entries. A denied
or failed read shows an explicit error with **Retry**, rather than an empty folder;
**File Access Settings** is available for denied reads. macOS still controls access.
Click the current breadcrumb or press **⌘L** to edit the full path. Mouse/Shift
selection, double-clicking components, select-all, cut/copy/paste and deletion are
supported. **Enter** opens the path; invalid text stays editable. **Escape** or an
outside click dismisses editing. Ancestor breadcrumbs continue to navigate.

Status messages appear centered in the existing bottom status bar, in a rounded
blue badge. They do not change the file area's height. Long messages have a full
tooltip; connection errors can still be dismissed. Todoist refresh runs silently
in the background, with its processing separate from file work. The task composer
opens using the fixed project name before online validation; validation occurs on
Save. Confirmed new tasks appear even before the first complete list has loaded.

The toolbar displays only the running app's version beside the Schnauz title;
`schnauz --version` shows the same version. The app filename stays Schnauz.app. Reopening an already-running macOS app does not replace its process.

Local breadcrumbs inside Home use **~/**. Click **~** for Home and its parent
folders; the current folder or **⌘L** still edits the complete path. The footer
shows item/selection counts, centered status and mode/settings, without repeating
the path. File drags do not recolour the browser/sidebar background.

## Website product handoff

The website kit (separate from GitHub) prioritizes project-specific work,
seamless terminal use, pasted-path navigation and Todoist. Its description,
brief and new sample-data captures are in the existing website/Schnauz-Website-Kit.
Release status is recorded in docs/RELEASING.md. The opt-in ui-checks build supports
--website-preview; its separately named preview bundle always uses an isolated
profile even when LaunchServices drops arguments. --website-preview-scene=N
selects reproducible start scenes (0 light, 1 another project, 2 pasted path,
3 dark); this fixture is excluded from the normal app. The current four captures
are 3200x1640 JPEGs and are documented in the handoff ASSETS.md.

## Transfer feedback — 0.5.5

Local copies/moves and SSH uploads/downloads show progress in the fixed footer,
with a compact bar, percentage and Cancel. Network progress refers to the current
file (also named), so a folder's next file starts its own percentage. Preparing
shows an indeterminate state; completion is confirmed after the process succeeds.
Cancelling a network copy can leave partial destination files; source files stay.
The transfer owns a separate SFTP process/PTY and never uses a live shell.
The old floating progress panel is removed. Hovered destination tabs remain blue
without moving the mouse and stay marked after their 650 ms dwell activates them.

### Schnauz app choices

Open With shows native application icons and marks the current app. Other
Application… opens a native chooser for apps not suggested by macOS. Choosing a
different app asks “Always open .ext with App?”: OK remembers that extension in
Schnauz, Only Now opens once, Cancel/Escape does neither. The choice applies to
normal file opening, multiple selected files, palette results and downloaded SSH
files, across projects and windows. macOS/Finder defaults remain unchanged.
Use macOS Default Again removes the Schnauz override. Choices are stored centrally
in file-apps.json; folders, app bundles and extensionless files retain normal behavior.

Image and document previews support wheel zoom and dragging to pan an enlarged
image. Selecting another file resets preview zoom. In native text previews,
ordinary wheel scrolls the text and Shift+wheel changes its size. Image layout
retains the source aspect ratio at pane width.
Quick Actions also lists matching installed Automator file actions from the user
and system Services folders, including Finder-scoped image workflows. Actions run
only when chosen; no-input workflows and Finder extensions are separate facilities.

Hold Command while right-clicking a local file or folder to open Quick Actions
directly. Assign action keys under Settings → Keybinds → Quick Actions; click a
binding, press a combination with Command/Control/Option, or Backspace to clear.
Conflicting keys are rejected. Shortcuts run on one selected local item.
Preview Information places two label/value pairs on each row, with 12 pt side
padding and tooltips for long values, shared by image and video previews.

Information headings stay horizontal. Created, Modified and Last opened show the
system-local date on one line and time below it, with compact spacing. The preview
has a 253 pt minimum width when space permits. Sidebar Folders/Files subsections
always stay expanded. Right-click a sidebar folder and choose Apply Current View
& Sorting to This Folder & Subfolders to copy the main file view into that tree;
this leaves the sidebar appearance unchanged.

Dragging over another tab or folder activates it after a short dwell. Canceling
back into the original folder restores immediate file selection.
Home-folder aliases pointing into CloudStorage (such as ~/Nextcloud) retain cloud
identity, provider menus and file/folder badges. Nextcloud sharing requires an
explicit Schnauz connection; the desktop client's credentials are not reused.

Search supports Command+C/X/V and a native Cut/Copy/Paste/Select All context menu.
Terminal mouse selections copy automatically on release. Use Shift while selecting
in terminal applications that capture the mouse. A visible right-hand scrollbar
shows the scrollback position and can be dragged.

Right-click a tab and choose Move to Project… to move it with its existing terminal,
output and running jobs. Choose a project, leave Activate Project checked to switch
there immediately, and click Move. Clear that option to stay in the source project.
Add to Favourites… in the tab menu adds its folder as a project shortcut instead.

Click the idle Schnauz dog to open About Schnauz / Coffee for Cookie. Its Updates
section offers an optional startup check (off by default) and Check for Updates….
Checks use public GitHub releases, including previews, and offer only the DMG for
the installed app's architecture. A small info badge above the dog indicates a
newer available version; hover for Update available and click for details.
A locally newer developer build shows an amber info badge instead, with an About
warning naming the installed and latest published versions. No downgrade is offered.
Checks do not install updates or restart the app or terminals. The same manual
check is available in the Schnauz app menu and Settings → General → Version.

### Floating Todoist window (local test stand)

Use the small pin in the task pane's upper-right corner to detach it. There is
one shared floating window across all Schnauz browser windows, following the
active project. It stays above normal windows and follows you across Spaces.
Compact shows the first open task; Expand restores the full task list and size.
Use its pin to dock it again, or choose Dock Here in another browser window.
Show Floating Tasks brings the existing window forward. Tasks keep working when
the original browser closes, without retaining that browser's terminals.

The task pane uses a neutral outline pin while docked and a crossed pin while
floating. Hovering identifies the current state and the Detach/Dock action.


SSH connections and remote folder loads show immediate progress in the
bottom-center status bar, including saved SSH favourites. The blue badge names
the server and reports SSH settings checks, connecting and folder loading. It
clears when the operation finishes; connection errors retain their dismiss button.


The terminal's Select text button lets you mark and copy a still view while a
program such as Codex continues running. Release the mouse to copy; ⌘C also
copies the selection. Click Live terminal, type, paste with ⌘V or press Escape
to return to live output. Shift-drag in mouse-reporting/full-screen programs
also holds a still view, preventing redraws from erasing the selection.


Tabs with a live terminal session show the same terminal icon as the toolbar,
scaled down on the tab's right side before Close. It remains visible when the
terminal panel is hidden and disappears when the session ends.

SSH tabs also show the globe used on SSH favourite-folder badges. The globe and
terminal tab indicators are black in light mode and white in dark mode.


On quitting Schnauz or closing its last browser window, choose Keep Setup & Quit,
Start Fresh Next Time or Cancel. Keeping restores browser windows, open projects,
local/SSH tabs, layouts and terminal panels at the next launch. Terminals start
fresh in their saved folders; running commands and programs are not resumed.
Starting fresh opens a clean Personal/Home window while preserving saved projects,
favourites and preferences. The warning about active terminal work still appears
before quitting when needed.


Active-project chips below Search are centered across the full window. A subtle
one-point divider and compact spacing separate the rows. When projects exceed
the available width, horizontal scrolling and selected-project reveal remain.


Weblinks in each project's Favourites and shared Common store a name, HTTP(S) URL
and optional browser. Add weblink… / Edit offers macOS Default or an installed
browser; the choice belongs to that link, independently of the system default.
Single-click opens; drag reorders without opening. Menus offer Edit, Copy link and
Remove. A missing chosen browser reports an error so you can choose another.
The website kit's FEATURE-STATUS.md lists the additions since its original 0.5.4
copy, with local-preview/public-release distinctions and unchanged capture dates.


Drag the file action icons beside the folder path to change their order. The
order is shared across windows and saved for next time; List/Icons moves as a
single group. The document-plus button beside New Folder opens New Text File…:
choose a name to create an empty .txt file in the current folder. Existing files
are never overwritten.


Terminal header: Hide keeps the tab's shell and terminal badge alive; the Close
cross immediately beside it ends that owned session and removes the badge.
Terminal resize updates the grid and PTY before drawing, and full-screen editors
receive modified navigation keys and Shift-page keys.


### Reliability and diagnostics (0.8.9)

The terminal divider can expand the terminal without crashing a very short file
list. The project row has 8 pt clearance above and below its chips to distinguish
projects from file tabs. Local session diagnostics retain up to 12 private logs
in ~/Library/Logs/Schnauz: Rust panics include a forced stack trace, uncaught
native exceptions include their stack, and fatal signals receive a marker.
Raw memory-fault signals have no custom stack trace. Fixed action labels do not
record terminal output, commands, file contents or credentials. No reports are
uploaded automatically.

Source builds stage a verified update while the installed app is running;
applying it requires a deliberate quit. For manual DMG/Finder updates, quit the
old app before replacing Schnauz.app.
