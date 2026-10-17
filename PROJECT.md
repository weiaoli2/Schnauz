# Schnauz

Last updated: 2026-10-17

Schnauz is Oliver Weber's independent macOS file manager, based on Shuffle by
Jaime Guzman. Oliver leads design and product direction; Codex assists with
implementation. Preserve upstream attribution and bundled third-party notices.

## Find and continue this project

- Working repository: `/Users/oliw/Documents/Codex/2026-10-17/what/outputs/shuffle-studio`
- Built app: `Schnauz.app` inside that repository; bundle ID `local.schnauz.app`.
- Local branch: `studio`. Check Git for current changes before editing.
- Project navigation: `/Users/oliw/CodexProjects/INDEX.md`, entry **Schnauz**.
- Read [AGENTS.md](AGENTS.md) for the product decisions and working constraints,
  and [README.md](README.md) for the feature guide and source map.

Start Codex CLI directly in the existing repository:

```sh
codex --cd /Users/oliw/Documents/Codex/2026-10-17/what/outputs/shuffle-studio
```

The folder's legacy name is retained; the app is called Schnauz. Work in this
checkout rather than creating a fresh Shuffle clone. Keep this document current
when the architecture, working location or release stage changes.

Codex reads repository `AGENTS.md`; that file points here for the handoff.
See the [official instruction-discovery guide](https://learn.chatgpt.com/docs/agent-configuration/agents-md).

## Build and verify

Rust + GPUI with native macOS integrations and an embedded PTY terminal. Requires
Rust stable and Xcode; the current development Mac is Apple Silicon (M4 Pro).

```sh
./build.sh                              # Optimized app bundle, locally signed
STUDIO_BUILD_PROFILE=debug ./build.sh    # Faster development build
./test.sh                               # Existing regression suite
```

The scripts source `dev-env.sh` and reuse `../../work` for the local build cache.
That prepared cache defaults to offline mode. Use `CARGO_NET_OFFLINE=false` only
when dependencies need fetching. Read `Cargo.toml` for the current package version.
Use the normal build or tests for verification; avoid a separate expensive
`cargo check` rebuild of the GPUI/Metal dependency stack. Format touched code only.

## Current stage and user data

Oliver is manually testing the current app and reporting individual fixes. Wait
for those reports; do not start an unsolicited UI audit or alter his live browsing
session. Do not quit/restart his app, terminals, Codex sessions or SSH connections.

Normal settings are in `/Users/oliw/Library/Application Support/Schnauz/`.
Projects and folder views are stored centrally, including `projects.json` and
`folder-views.json`. Earlier profiles under `../../work` also contain user data;
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
| Project persistence and switching | `src/projects.rs`, `src/studio.rs` |
| Project folders, tags and connections | `src/project_sidebar.rs` |
| Global drive capacity and cloud paths/menus | `src/volumes.rs`, `volumectl.swift`, `src/cloud_locations.rs`, `src/cloud_actions.rs` |
| Direct Nextcloud accounts, links, sharing and locks | `src/nextcloud.rs`, `native/nextcloud/Core.swift`, `native/nextcloud/main.swift` |
| Saved folder views and list columns | `src/folder_views.rs`, `src/list_ui.rs` |
| Preview layout and media | `src/preview_panel.rs`, `src/previews.rs`, `src/video_preview.rs` |
| Terminal and SSH following | `src/studio_terminal.rs`, `src/ssh_sync.rs`, `src/terminal-shell/` |
| Remote directory names and listings | `src/remote_listing.rs` |
| Search and storage map | `src/file_search.rs`, `src/search_ui.rs`, `src/space_map.rs`, `src/space_ui.rs` |

Keep browsing fast: no automatic recursive size scans or home indexing. The
collapsed sidebar contains only its reopen button; obsolete compact-rail controls
are removed from the source. Devices stays above Projects with background capacity
updates and adjustable 80/90%-used colour thresholds. Locations replaces Favorites;
legacy shortcuts merge without duplicates. Cloud paths have a persistent blue
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

## Todoist proposal — discussion only

This integration is not implemented or approved for implementation yet. The
following is a suggested first version, saved for the next design discussion.

**Interaction:** Add a **Tasks** choice beside Preview in the right-hand pane,
below the path and filter bars. Once connected, a project can explicitly select
an existing Todoist project. Switching Schnauz projects switches the task list.
Show open tasks, section/subtask structure, due dates and priorities, with a
checkbox to complete and a small **Add task…** field. Task details can open in
Todoist. Tasks stay visible independently of the file preview's auto-hide rule.

**Connection:** For the first personal build, let Oliver enter his Todoist API
token in the app and store it through the macOS Keychain API. Keep credentials
out of project files, logs, command arguments and Git. For distribution, plan a
**Connect Todoist** browser login; verify the production callback flow before
shipping it. Todoist documents OAuth public clients with PKCE, so never embed a
confidential client secret in the app.

**Implementation shape:**

- Add an optional, backwards-compatible project link with a local account
  reference and Todoist project ID. Store IDs as strings in existing local
  project settings; names are display labels. Existing projects load unlinked.
  Personal can have its own optional link. No automatic project creation.
- Keep a small native Rust API layer in a proposed `src/todoist.rs` and UI in
  `src/todoist_ui.rs`. Use direct HTTPS; there is no need to run an AI agent or
  CLI for routine task operations.
- Fetch projects for the picker and active tasks filtered by the linked project;
  follow pagination to avoid showing only the first page. Creating a task uses
  the captured destination project, even if the user switches projects while
  the request runs. Completion updates Todoist explicitly.
- Run requests in background workers, cache by account/project, and discard
  stale responses after a project/account switch. Show cached results immediately
  and refresh when the pane opens or the app regains focus; refresh periodically
  only while visible. Network work must never block file browsing.
- Show loading, offline and failure states. Retain a failed new-task draft and
  mark cached results as stale. Start with online writes rather than building
  an offline mutation queue. Keep refreshes bounded and back off on rate limits.
- Verify persistence/migration, pagination, project-switch races and failed
  writes using fixtures and a mock service before any live account testing.

Later possibilities: create a task from a selected file, attach a useful file
reference, or add task editing. Local file references would work only on machines
that can access those paths. These are separate follow-up choices.

API basis: [Todoist API v1](https://developer.todoist.com/api/v1/) supports
project-scoped task reads, task creation/completion, pagination and authentication.
Recheck the current schemas when implementing. The existing
`/Users/oliw/OliPomodoro` app is a possible local reference for Todoist behavior;
review relevant source only and do not reuse its credentials or running state.
