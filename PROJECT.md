# Schnauz

Last updated: 2026-10-18

Schnauz is Oliver Weber's independent macOS file manager, based on Shuffle by
Jaime Guzman. Oliver leads design and product direction; Codex assists with
implementation. Preserve upstream attribution and bundled third-party notices.

## Find and continue this project

- Working repository: `/Users/oliw/CurrentProjects/Schnauz`
- Built app: `Schnauz.app` inside that repository; bundle ID `local.schnauz.app`.
- Local branch: `schnauz`. Check Git for current changes before editing.
- Project navigation: `/Users/oliw/CodexProjects/INDEX.md`, entry **Schnauz**.
- Read [AGENTS.md](AGENTS.md) for the product decisions and working constraints,
  and [README.md](README.md) for the feature guide and source map.

Start Codex CLI directly in the existing repository:

```sh
codex --cd /Users/oliw/CurrentProjects/Schnauz
```

Work in this checkout rather than creating a fresh Shuffle clone. The original
chat output path is a compatibility symlink for existing sessions; the source,
Git history, app bundle and build cache now live in CurrentProjects. Keep this
document current when the architecture, working location or release stage changes.

Codex reads repository `AGENTS.md`; that file points here for the handoff.
See the [official instruction-discovery guide](https://learn.chatgpt.com/docs/agent-configuration/agents-md).

## Build and verify

Rust + GPUI with native macOS integrations and an embedded PTY terminal. Requires
Rust stable and Xcode; the current development Mac is Apple Silicon (M4 Pro).

```sh
./build.sh                              # Optimized app bundle, locally signed
SCHNAUZ_BUILD_PROFILE=debug ./build.sh    # Faster development build
./test.sh                               # Existing regression suite
```

The scripts source `dev-env.sh` and reuse the ignored `.build-cache/` directory
inside the checkout for the local Cargo registry and build cache.
That prepared cache defaults to offline mode. Use `CARGO_NET_OFFLINE=false` only
when dependencies need fetching. Read `Cargo.toml` for the current package version.
Use the normal build or tests for verification; avoid a separate expensive
`cargo check` rebuild of the GPUI/Metal dependency stack. Format touched code only.

All current implementation names use Schnauz. `src/legacy.rs` retains only
compatibility identifiers for earlier settings, user scripts and saved SFTP
access. Original credits and license notices remain intact; the old updater
has been removed. App-icon customization and icon packs are also removed;
Schnauz keeps its fixed app icon and uses native macOS file/application icons.

Schnauz Light uses black text and controls; saved stock light themes upgrade
on load while native folder icons keep macOS blue. File preview is independent
of the framed Todoist/Disk Usage tools pane, below the full-width path/filter
rows. The two tools replace each other; preview remains visible for previewable
selections. Narrow split panes stack tools and preview. Preview width stays in
folder views; tools width is saved separately in central preferences. Terminal
and Preview have visible toolbar labels. Text preview is read-only native text
at the file-name size (12 pt), with mouse selection, Cmd+A and Cmd+C.

## Current stage and user data

Oliver is manually testing the current app and reporting individual fixes. Wait
for those reports; do not start an unsolicited UI audit or alter his live browsing
session. Do not quit/restart his app, terminals, Codex sessions or SSH connections.

Normal settings are in `/Users/oliw/Library/Application Support/Schnauz/`.
Projects and folder views are stored centrally, including `projects.json` and
`folder-views.json`. Earlier profiles under
`/Users/oliw/Documents/Codex/2026-10-17/what/work/` also contain user data;
never replace them with test fixtures. For necessary isolated UI verification,
use fresh `SCHNAUZ_CONFIG_DIR` and `SCHNAUZ_START_DIR` paths without changing HOME.

The next release intention is a **0.5 candidate**, publishing to Oliver's Gitea
at **192.168.1.8**, then installing the release app in Applications after his
manual review. This has not been published or installed as that release. Verify
the server account/repository and current review status before proceeding.

## Where changes belong

| Area | Source |
|---|---|
| Browser, navigation and file operations | `src/main.rs` |
| Project persistence, shared windows and switching | `src/projects.rs`, `src/schnauz.rs`, `src/browser_windows.rs` |
| Project folders, tags and connections | `src/project_sidebar.rs` |
| Global drive capacity and cloud paths/menus | `src/volumes.rs`, `volumectl.swift`, `src/cloud_locations.rs`, `src/cloud_actions.rs` |
| Direct Nextcloud accounts, links, sharing and locks | `src/nextcloud.rs`, `native/nextcloud/Core.swift`, `native/nextcloud/main.swift` |
| Saved folder views and list columns | `src/folder_views.rs`, `src/list_ui.rs` |
| Todoist API bridge, cache and Tasks pane | `src/todoist.rs`, `src/todoist_ui.rs`, `native/todoist/` |
| Inline name editor and click-to-rename timing | `src/rename_ui.rs`, click dispatch in `src/main.rs` and `src/search_ui.rs` |
| Preview layout, selectable text and media | `src/preview_panel.rs`, `src/previews.rs`, `src/text_view.rs`, `src/native_preview.rs`, `src/video_preview.rs` |
| Terminal and SSH following | `src/schnauz_terminal.rs`, `src/ssh_sync.rs`, `src/terminal-shell/` |
| Remote directory names and listings | `src/remote_listing.rs` |
| Search and storage map | `src/file_search.rs`, `src/search_ui.rs`, `src/space_map.rs`, `src/space_ui.rs` |

Keep browsing fast: no automatic recursive size scans or home indexing. The
collapsed sidebar contains only its reopen button; obsolete compact-rail controls
are removed from the source. Devices stays above Projects with background capacity
updates and adjustable 80/90%-used colour thresholds. The compact sidebar has
Projects without count badges, then Favourites with a pinned local Home entry
(blue Home badge) in every project, followed by Folders / Files. Drive capacity
badges toggle saved GB / percent free without navigation. Folder menus open in
place, a new tab or a new Schnauz window. Windows share the active project and
view cache; project switches propagate while live tabs/PTYs stay per window. Empty
subsections and their placeholder messages are hidden; Favourites has no +
button or dedicated picker. Its width fits
visible labels, capped for long names. Dropped local/SSH items and the Tag button
use the same typed shortcuts. Legacy Locations, Cloud, network folders and Tagged
Items merge without loss; their old sections are removed. Cloud/network badges
identify providers without rendering-time I/O. SSH favourite navigation uses a
browser-only tab and never sends terminal commands. Connections remain in Settings
and Go → Connect to Server… (Cmd+K). Cloud paths have a persistent blue
provider badge and relative breadcrumbs. The approved direct Nextcloud connection
is implemented: Settings → General → Connections links a local folder through
browser login, with its own credential in Keychain. Nextcloud menus offer sharing,
internal links and server-supported file locks. Nonsecret mappings are centralized
in `nextcloud.json`; no browsing-time network requests. Local automatic-space
policy still uses an explicitly labeled Finder handoff. Native offline protocol
tests run with `./test.sh`; actual account login/server interoperability awaits
Oliver's manual test. Do not contact his live server for automated verification.
Local terminal directory changes are allowed only
at an idle prompt. SSH follows **terminal → browser only**; never send directory
commands into a remote session. Unsync returns to local browsing without ending SSH.

## Todoist integration

Oliver approved implementation on 2026-10-17 and added a specific rule: Schnauz
and Todoist projects share the same name. Creating a Schnauz project checks all
Todoist projects, links an existing name match and immediately displays its tasks,
or creates a same-named Todoist project if absent. Ambiguous duplicate names get
an explicit chooser. A failed lookup cannot cause project creation. If disconnected,
the creation intent waits in the local project until Todoist is connected.

Connect in **Settings → General → Connections → Todoist**. The toolbar's Todoist
button appears only after an account is configured; connection opens Tasks and
Settings → Show tasks can reopen it. For this personal build, enter the API token in the native
secure dialog. Only macOS Keychain stores it. `todoist.json` holds the local account
reference and display identity; project links and stable creation-request IDs live
in `projects.json`. Existing projects migrate without links. Disconnect removes
Schnauz access without revoking a personal token used by other apps.

Oliver confirmed that Todoist was connected and working on 2026-10-18. Panel
layout and text-preview refinements are in the current build; visual verification
remains his manual workflow.

The **Todoist** button toggles the framed tools pane, below the full-width navigation
and filter bars, independently of file preview. It follows Schnauz project switching
and has its own saved width; opening Disk Usage replaces Tasks in that tools pane.
Existing unlinked projects look for matching names; **Find or create matching
project** can explicitly create one if absent. Open tasks show sections, subtasks,
due dates and priority colours. Click a circle to complete, a task to open Todoist,
or **Add task…** for a native entry dialog with an optional due date. Failed forms
keep their content for retry. Writes retain the project captured when opened.

Requests run through `native/todoist/` / `todoistctl` to Todoist API v1, with tokens
outside Rust/settings/argv/logs. The Rust bridge caches by account/project, keeps
old results on failure, follows pagination, checks response identity and excludes
stale reads after a write. The virtual list renders only visible rows. Refresh is
30 seconds while Tasks is open and Schnauz active, plus reopening/focus when stale;
rate-limit backoff applies to manual refresh too. The native helper refuses redirects.

`./test.sh` includes offline URLProtocol fixtures for naming/creation, pagination,
captured project destinations, completion, idempotency, errors and settings, plus
Rust tests for project persistence, hierarchy and cache races. Do not contact the
live Todoist service or read/write Keychain during automated QA. Actual login and
UI verification remain Oliver's manual checks. The app has not been restarted.

Future distribution work: OAuth browser connection, without an embedded client
secret. File-to-task actions and advanced task editing remain separate ideas.
API reference: [Todoist API v1](https://developer.todoist.com/api/v1/).
