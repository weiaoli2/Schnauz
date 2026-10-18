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
  and [README.md](README.md) for the product overview and [docs/FEATURES.md](docs/FEATURES.md) for the detailed feature guide and source map.

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
App packaging now uses a stable Apple Development / Developer ID signing
certificate from Keychain, preserving the existing signer. To choose explicitly,
set `SCHNAUZ_CODESIGN_IDENTITY` to a certificate name or SHA-1. Missing or ambiguous
certificates fail before replacing the app; `-` explicitly requests a temporary
ad-hoc build whose permissions may not survive changes. After the one-time move
from ad-hoc signing, the user needs to re-add the current app in Full Disk Access.
Builds stage and verify the app before replacing it, without restarting sessions.
`./test-signing.sh` checks selection using fixtures without signing or TCC access.

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
selections. Preview stays directly right of the files and uses their existing
width; Todoist/Disk Usage extend the window to the right. They share one extension
without stacking or repeated growth when switching tools. Width measurements are
per window. Preview width stays in
folder views; tools width is saved separately in central preferences. Terminal
and Preview have visible toolbar labels. Text preview is read-only native text
at the file-name size (12 pt), with mouse selection, Cmd+A and Cmd+C.

## Release workflow — resumed for 0.5.5

Oliver initially paused release work on 2026-10-18, then explicitly resumed it
for 0.5.5 after the transfer-footer/tab-hover fixes. Reuse the saved workflow. Public source is already at
https://github.com/weiaoli2/Schnauz (origin/main, preparation commit ad01fb5).
The original 0.5.0-rc.1 preparation published no tag or release. Its outputs
predate subsequent fixes; 0.5.5 is rebuilt separately before publication. Reuse release.sh and the complete
procedure in docs/RELEASING.md; do not redo account/repository/tooling setup.

The separate website kit is at
/Users/oliw/Documents/Codex/2026-10-17/what/outputs/Schnauz-Website-Kit.
Oliver explicitly authorized a product-info and screenshot refresh on 2026-10-18,
prioritizing projects, terminal, path paste and Todoist. That supersedes only the
website-kit freeze; release publication was later resumed for 0.5.5. The kit includes
four verified sample-data screenshots, the existing logo, complete English copy
and a website-Codex brief. Older images are archived separately, not presented as
the current captures.

An isolated screenshot session exposed a blank/oversized picture preview; this
has not been fixed. Suspected cause: percentage image width combined with GPUI's
intrinsic pixel height. Reproduce and fix in the normal bug workflow when taken
up; do not assume the root cause is confirmed. Oliver will provide further bugs.
Never restart or substitute the demo for his live app.

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

On 2026-10-18 Oliver explicitly requested public source and public downloads on
GitHub, superseding the earlier Gitea/private-repository idea. The release destination
is **weiaoli2/Schnauz** for both Apple Silicon and Intel. The original 0.5.0-rc.1
preparation is superseded by the authorized 0.5.5 preview.
Release packaging uses separate output paths and must preserve the running app.
See docs/RELEASING.md for signing, source notices and publication checks.
Apple Development signing is available; Developer ID/notarization is not yet set up.

## Latest reported fixes — 2026-10-18

- Ancestor navigation starts at the top of the new folder and rebuilds filters.
- Failed local reads show Folder access denied/unavailable, Retry and an explicit
  File Access Settings button for denied reads; they no longer look like empty
  folders. Actual Documents access is still controlled by macOS and tested by Oliver.
- Current breadcrumb / Cmd+L opens an editable path with mouse selection,
  Cut/Copy/Paste, select-all, delete and scrolling. Invalid paths remain editable.
- Oliver confirmed Home and Documents browsing work. Home now moves to Common
  → Folders, a shared list above project Favourites. Common starts with Desktop
  and Downloads; both can be removed permanently, while Home stays pinned.
- Todoist status/empty text and task rows keep their width. Native Todoist links
  fall back to the browser if the app is absent. Oliver confirmed task creation
  reaches the correct Todoist project.
- The compact native task composer uses Schnauz colours and the small toolbar
  dog. Project stays locked; description, date, deadline, priority, labels,
  absolute-time reminders and location reminders are implemented. Location search
  uses Apple Maps on the user's search action. Optional reminders depend on the
  Todoist account/plan; failed optional writes cannot create a second task on retry.

Validation passed: 72 Rust tests, six Nextcloud and six Todoist offline fixture
groups, native light/dark composer and partial-save retry checks, and the complete
real GPUI window layout with synthetic empty/populated Todoist data. The normal
local bundle is rebuilt after the current fixes with its deep/strict signature
verified before replacement. No live account or Keychain reads were used for testing. A disposable
separate app successfully received a native local-folder drop, kept Home first
and displayed the blue notice in the fixed footer. Held-drag flicker and the SSH
drag preview were not visually verified; live Todoist task listing remains
Oliver's manual check. Release publication remains on hold; website imagery was
subsequently refreshed at his explicit request.
Never restart his live app or sessions.

Further reported fixes: notices/SSH errors use the fixed footer's centered blue
badge. Todoist has definite pane/row/text widths, confirmed additions populate
before the first snapshot, and the composer opens before online validation.
Helper waits/sorting stay separate from file work; silent refresh cannot open
modal windows. Tasks use round completion controls, separators, secondary due
labels and an always-present + Add task row at the list end. Both preview/tools
resize from the LEFT; tools divider movement never grows the window. Only opening
tools adds space; the normal right border remains a native window resize handle.
The complete hidden GPUI layout check covers empty/populated/error states. A
separate visible fixture confirmed horizontal titles/dates, the Add task row,
Common defaults, independent preview, left divider drag and native right-border
resize. Its data is synthetic; live Todoist remains Oliver's verification.

Only the compiled version (currently 0.5.5) appears beside the Schnauz title
and in --version. Oliver removed date/build/source IDs; the app stays Schnauz.app. At inspection the user's process had
started at 13:04, before the prior 13:06 build finished, so old observations did
not verify the new binary. Do not restart his live sessions automatically.

Full Disk Access identity was inspected read-only in System Settings. Its enabled
lowercase `schnauz` entry resolves through Show in Finder to the current
`/Users/oliw/CurrentProjects/Schnauz/Schnauz.app/Contents/MacOS/schnauz`, not an old
development build. The generic executable icon is consistent with that individual
binary entry; the complete app bundle has the correct Schnauz name and AppIcon.
This observation does not prove that every launch uses that grant. No permission
was changed and no protected files were probed. Apple documents no supported API
for prepopulating only the Full Disk Access list:
https://developer.apple.com/forums/thread/670923 . The user can manually add the
whole current Schnauz.app with + for an unambiguous app-bundle entry.

## Where changes belong

| Area | Source |
|---|---|
| Browser, navigation and file operations | `src/main.rs` |
| Fallible local listings and access errors | `src/directory_listing.rs` |
| Editable path and pointer/keyboard selection | `src/path_editor.rs` |
| Optional macOS file-access guide | `src/file_access.rs`, General → File Access settings |
| Stable signing and staged app packaging | `build-signing.sh`, `make_app.sh`, `test-signing.sh` |
| Project persistence, shared windows and switching | `src/projects.rs`, `src/schnauz.rs`, `src/browser_windows.rs` |
| Project folders, tags and connections | `src/project_sidebar.rs` |
| Global drive capacity and cloud paths/menus | `src/volumes.rs`, `volumectl.swift`, `src/cloud_locations.rs`, `src/cloud_actions.rs` |
| Direct Nextcloud accounts, links, sharing and locks | `src/nextcloud.rs`, `native/nextcloud/Core.swift`, `native/nextcloud/main.swift` |
| Saved folder views and list columns | `src/folder_views.rs`, `src/list_ui.rs` |
| Todoist API bridge, cache and Tasks pane | `src/todoist.rs`, `src/todoist_ui.rs`, `native/todoist/` |
| Compact native task composer and layout checks | `native/todoist/Composer.swift`, `native/todoist/ComposerTests.swift` |
| Running app version | `Cargo.toml`, `src/schnauz.rs`, `--version` |
| Complete GPUI layout regression checks | `src/ui_checks.rs`, `test-ui.sh` (optional `ui-checks` feature) |
| Inline name editor and click-to-rename timing | `src/rename_ui.rs`, click dispatch in `src/main.rs` and `src/search_ui.rs` |
| Preview layout, selectable text and media | `src/preview_panel.rs`, `src/previews.rs`, `src/text_view.rs`, `src/native_preview.rs`, `src/video_preview.rs` |
| Terminal and SSH following | `src/schnauz_terminal.rs`, `src/ssh_sync.rs`, `src/terminal-shell/` |
| Footer transfer progress and owned SFTP meter | `src/transfer_status.rs`, `JobProgress` / upload / download in `src/main.rs` |
| Remote directory names and listings | `src/remote_listing.rs` |
| Search and storage map | `src/file_search.rs`, `src/search_ui.rs`, `src/space_map.rs`, `src/space_ui.rs` |

Keep browsing fast: no automatic recursive size scans or home indexing. The
collapsed sidebar contains only its reopen button; obsolete compact-rail controls
are removed from the source. Devices stays above Projects with background capacity
updates and adjustable 80/90%-used colour thresholds. The compact sidebar has
Projects without count badges, then shared Common and project Favourites. Home
(blue badge) stays first in Common → Folders. Desktop/Downloads are shared defaults,
removable without being restored on restart. Drive capacity
badges toggle saved GB / percent free without navigation. Folder menus open in
place, a new tab or a new Schnauz window. Windows share the active project and
view cache; project switches propagate while live tabs/PTYs stay per window. Empty
subsections and their placeholder messages are hidden; Favourites has no +
button or dedicated picker. Its width fits
visible labels, capped for long names. Dropped local/SSH items and the Tag button
use the same typed shortcuts. Legacy Locations, Cloud, network folders and Tagged
Items merge without loss; their old sections are removed. Cloud/network badges
identify providers without rendering-time I/O. SSH favourite navigation uses a
current tab if it already has a terminal, retaining its visible PTY and never
sending terminal commands. Connections remain in Settings
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

Oliver confirmed the account is connected and new tasks reach the correct
Todoist project. He subsequently reported missing tasks and one-letter status
wrapping in the pane. The latest local build addresses width allocation and cache
updates; live account and visual verification remain his manual workflow.

The **Todoist** button toggles the framed tools pane, below the full-width navigation
and filter bars, independently of file preview. It follows Schnauz project switching
and has its own saved width; opening Disk Usage replaces Tasks in that tools pane.
Existing unlinked projects look for matching names; **Find or create matching
project** can explicitly create one if absent. Open tasks show sections, subtasks,
due dates and priority colours. Click a circle to complete, a task to open Todoist,
or **Add task…** for the compact, project-locked native task composer. Failed forms
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

Latest pointer refinements: file drags retain selection but no longer tint the
whole file pane/sidebar or folder targets; the obsolete tint-tracking code was
removed. Outer-window resizing stays native AppKit. Cursor-only one-point edge
regions keep GPUI from replacing the resize cursor, with no painted border or
mouse-event interception. Before this cursor-only refinement, a normal 0.5.1
copy in a disposable profile successfully resized from all four edges and all
four corners; a point farther inside correctly remained ordinary content.

The footer's duplicate path is removed. Local paths beneath Home display ~/ plus
relative crumbs; clicking ~ opens native choices for Home, Users and the disk
root. Full-path editing remains available. Cloud badges and SSH roots are unchanged.

The earlier complete local testing version was 0.5.2. Earlier 0.5.1 QA windows do
not contain the last pointer/drag/breadcrumb refinements. The separate normal-app
QA copy at /private/tmp/schnauz-window-qa-gstoq1oa/Schnauz Window QA.app has its
own profile and was seen browsing Downloads with its sidebar collapsed. Preserve
that profile if it has become user state; do not terminate user sessions blindly.

## Previous test stand — 0.5.3

Normal drag moves; holding Command copies with AppKit's green + and a fixed-footer
Copy N items / Move N items label. The payload count survives tab switching. A
blue hovered tab activates after 650 ms, then folders (List/Icons, including the
parent row) spring-open after the same dwell. Browser and sidebar stay untinted.
Copy/move use the existing background jobs, undo and no-clobber behavior. Uploads
remain copies; sidebar drops remain references. Drag-out stays copy-only.

SSH favourites retain the original OpenSSH alias, preserving keys, custom ports
and jump-host settings on reconnect. Older resolved-address favourites recover
only a unique matching Host profile in the background; ambiguous matches are
never guessed. Offline configuration discovery supports literal aliases/includes
and caches nonsecret routing for 30s. Live terminal mux connections never reconnect
silently, and orphaned browser mux references are no longer reused.

Opening local/remote favourites while a terminal is present reuses that tab and
keeps the same PTY/view visible. Returning to its exact reported SSH folder resumes
following; browsing another folder or server stays Unsynced. No shell commands,
PTY replacement or live-session restart are involved. The hidden UI regression
checks this roundtrip plus native drag forwarding, real dwell, modifier feedback
and actual file copy/move with disposable data. Oliver's real server reconnect
and hand-drag appearance remain manual checks. Release publication stays on hold.

Validation for 0.5.3 passed: 76 Rust tests; six Nextcloud fixture groups; six
Todoist fixture groups; compact native composer checks; hidden native layout and
interaction checks, including modifier changes, tab/folder dwell, real copy/move
through the drop handlers and a same-session SSH/local/remote favourite roundtrip.
All use disposable data. No live app restart, real-server login or release upload.

The optimized normal Schnauz.app 0.5.3 is built and stable-certificate signed.
Deep/strict verification and --version passed. It was not launched or substituted
for Oliver's running app. Logs: /private/tmp/schnauz-053-tests.log and
/private/tmp/schnauz-053-build.log. No push, release, installer or website-kit update.

## Previous development preview — 0.5.4

Terminal can now start directly in the current SSH folder. Preparation and
saved-profile resolution run in the background; a new SSH PTY starts in the
captured folder. Existing authenticated transports are reused through an owned
private socket alias that never removes the original master. Initial folder
quoting handles spaces, quotes, shell metacharacters, percent signs and newlines.
Bash/Zsh initial prompts follow the requested folder even if startup files change
cwd. Existing terminal sessions are never replaced for navigation. Plain local
path paste in a remote tab also retains the visible terminal in that tab.

Oliver additionally authorized a refresh of the website product kit and new
sample screenshots, emphasizing projects, seamless terminal, path paste and
Todoist. Publication remains on hold. The screenshot fixture is an opt-in
ui-checks feature, launched in a separately named preview app. That bundle always
uses isolated mode, including argument-free LaunchServices relaunches; never load
live profiles. Its --website-preview-scene=N start option chooses real project
views, path navigation, theme and the real Tasks view with synthetic tasks. The app logo,
copy and handoff stay together in the existing website-kit output directory.

Validation for 0.5.4 passed: 79 Rust tests, all Nextcloud/Todoist offline fixture
groups, native composer checks and the hidden real GPUI layout/drag checks. A
new actual-PTY fixture runs the remote startup bootstrap through Bash and Zsh,
checks a literal difficult path, an interactive Vim session and clean exit. It
contacts no real SSH server. The native UI test also verifies local-file path
paste preserves an existing SSH terminal/tab and selects the intended file.
The optimized normal Schnauz.app prints 0.5.4 and passes deep/strict certificate
signature verification. It was not launched or substituted for Oliver's live app.
Real-server interoperability remains his manual test. Logs are
/private/tmp/schnauz-054-tests.log and /private/tmp/schnauz-054-build.log.

The refreshed website handoff is outputs/Schnauz-Website-Kit, with START-HERE.md,
WEBSITE-BRIEF.md, ASSETS.md, English description/SEO copy, original brand artwork
and four 3200x1640 JPEG captures: light project/terminal/Todoist overview, Brand
Refresh workspace, pasted README.md path/selection and dark overview. Todoist
tasks/projects are synthetic; the real local terminal lists only fixture files.
No live tokens, tasks, private file contents or user terminal sessions were used.
The separate preview app was closed after capture. The 13-file ZIP and SHA-256
manifest were verified; older screenshots are retained in a sibling archive.
No pushes, tags, releases, installers or website deployment were performed.

## Current work — 0.5.5 release

Oliver confirmed file copies work locally/SSH and SSH/SSH; missing visible feedback
was the issue. Existing SFTP transfer paths are retained. transfer_status.rs adds
real per-file network meter output via a separate owned PTY, private mode-0600
batch file and cancellation. Local copy/move jobs share the footer bar. No floating
progress overlay remains. Completion requires success; partial network destinations
are disclosed on cancellation. Existing user PTYs and SSH masters are untouched.

Tab hover uses stable app state/bounds rather than transient GPUI drag-over styles;
it stays blue without pointer motion and through dwell activation. The hidden
native drag test covers stationary hover, dwell and actual copy/move. The real
footer layout probe verifies progress stays in the 30 pt footer without shifting
files. Local SFTP-server tests cover quoted paths, recursion, real percentage
output and owned-process cancellation; no live server or account is accessed.

Oliver authorized public GitHub source + both Mac installers for 0.5.5, with an
English overview linking https://techguyzone.com/apps/, and no new images. The
prior hold is superseded. Reuse release.sh; never overwrite an existing release
or restart live sessions. Apple Development signing, not Apple notarization;
Intel execution here is Rosetta, not physical Intel hardware. Record actual
artifact/build/publication results after verification.

0.5.5 validation passed: 82 Rust tests, six Nextcloud fixture groups, six Todoist
fixture groups, native composer checks and complete hidden layout/drag checks.
After removing an unnecessary newer SFTP flag, the three real transfer/meter/
cancellation tests passed again. The normal 0.5.5 optimized app is built and
passes deep/strict certificate verification; it was not launched or substituted
for Oliver's live process. Separate architecture artifacts are being verified
through the existing release workflow.

Both 0.5.5 architecture apps built successfully. Apple Silicon ran natively;
Intel --version and the native text-preview smoke check passed under Rosetta.
The release workflow was corrected to accept pre-1980 timestamps in unchanged
MPL source files (ZIP metadata only). No source contents are altered. Both DMGs
are built separately from the normal app; their final checksums/source archive
are verified before the authorized GitHub draft is published.
