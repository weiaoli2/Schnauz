# Schnauz

Last updated: 2026-10-30

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

## Website and video material

- [Website product summary, copy and assets](website/PROJECT.md)
- [Video project, recordings, exports and current direction](video/PROJECT.md)

Both live in this source project. Continue production in these existing folders;
keep the product claims aligned with the current app. No publishing is implied.

## Build and verify

Rust + GPUI with native macOS integrations and an embedded PTY terminal. Requires
Rust stable and Xcode; the current development Mac is Apple Silicon (M4 Pro).

```sh
./build.sh                              # Optimized app bundle, locally signed
SCHNAUZ_BUILD_PROFILE=debug ./build.sh    # Faster development build
./test.sh                               # Existing regression suite
./update.sh                             # Apply the latest locally staged build after quitting
```

`update.sh` applies the latest signed build already staged by `build.sh`, using
the existing safe installer. It never quits or launches Schnauz, downloads a
GitHub release or rebuilds source. While Schnauz is running, installation stays
deferred so terminal sessions remain intact. Run it again after deliberately
quitting the app, then open the normal Schnauz.app. If no build is staged, prepare
one with `./build.sh` first.

The scripts source `dev-env.sh` and reuse the ignored `.build-cache/` directory
inside the checkout for the local Cargo registry and build cache.
App packaging now uses a stable Apple Development / Developer ID signing
certificate from Keychain, preserving the existing signer. To choose explicitly,
set `SCHNAUZ_CODESIGN_IDENTITY` to a certificate name or SHA-1. Missing or ambiguous
certificates fail before replacing the app; `-` explicitly requests a temporary
ad-hoc build whose permissions may not survive changes. After the one-time move
from ad-hoc signing, the user needs to re-add the current app in Full Disk Access.
Builds stage and verify the app. If the destination app is running, installation
is deferred; its bundle is never moved or deleted. After a deliberate quit, apply
the pending build with the command printed by packaging, then launch normally.
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

Website material index: [website/PROJECT.md](website/PROJECT.md). Keep the product
summary and copy aligned with new features using that handoff.

The website kit is at
/Users/oliw/CurrentProjects/Schnauz/website/Schnauz-Website-Kit.
Oliver explicitly authorized a product-info and screenshot refresh on 2026-10-18,
prioritizing projects, terminal, path paste and Todoist. That supersedes only the
website-kit freeze; release publication was later resumed for 0.5.5. The kit includes
four verified sample-data screenshots, the existing logo, complete English copy
and a website-Codex brief. Older images are archived separately, not presented as
the current captures.

The reported oversized picture preview is addressed in local 0.5.7: explicit
width and aspect-ratio height replace percentage width with intrinsic pixel
height. Preserve the user's live app while verifying in isolated fixtures.

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
policy uses a direct checked provider bridge (see the local 0.9.0 notes). Native offline protocol
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

The refreshed website handoff is website/Schnauz-Website-Kit, with START-HERE.md,
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
for Oliver's live process. Separate architecture artifacts passed the existing release workflow.

Both 0.5.5 architecture apps built successfully. Apple Silicon ran natively;
Intel --version and the native text-preview smoke check passed under Rosetta.
The release workflow was corrected to accept pre-1980 timestamps in unchanged
MPL source files (ZIP metadata only). No source contents are altered. Both DMGs
are built separately from the normal app; their final checksums/source archive
are verified before the authorized GitHub draft is published.

## Published release — 0.5.5

Public early prerelease published 2026-10-18:
https://github.com/weiaoli2/Schnauz/releases/tag/v0.5.5

Source tag v0.5.5 points to 7c48f1432377c08899f114c70c831903c197d694.
Both architecture apps/helpers have verified signatures and matching architectures;
Apple Silicon ran natively and Intel's text-preview smoke test passed under
Rosetta. Both DMGs, exact MPL sources and SHA256SUMS.txt were uploaded, and every
GitHub asset digest matched its local SHA-256. Release overview links TechGuyZone;
no new images. The release is Apple Development signed and not notarized.

Artifacts/receipt: dist/0.5.5/RELEASE-VERIFICATION.json. Existing release.sh and
docs/RELEASING.md are the reusable flow. This publication request is complete;
future versions require a new user request. Never replace published assets or
move v0.5.5. The user's live application/terminal sessions were not restarted.

## Local 0.5.6 — Schnauz-only app choices

Open With now shows native app icons and the current app. A different app opens
a confirmation with OK (remember for this extension in Schnauz), Only Now and
Cancel. Central file-apps.json choices apply across projects/windows and ordinary
open paths, including downloaded SSH files; macOS defaults stay untouched. The
menu can remove an override with Use macOS Default Again. No publication requested.

Validation: 84 Rust tests passed with --test-threads=1, plus all native
Nextcloud/Todoist/composer tests and hidden GPUI layout/drag checks. The new hidden
fixture checks Only Now without creating/changing preferences, Always persistence,
case-insensitive lookup, reset, actual NSMenuItem images/checkmarks, modal geometry
and preview blocking, using disposable settings and without launching file apps.
A parallel test run intermittently failed the pre-existing legacy-favourites
migration fixture; the serialized full suite passed. Live windows were preserved.

## Local folder reload control

A compact vector reload button sits beside All / Folders / Files in each pane.
It targets that pane and shares the existing ⌘R action for local/cloud and SSH
listings, preserving the current terminal session. No publication requested.

Reload-control validation: all 84 Rust tests and native Nextcloud/Todoist/composer
checks passed, plus hidden GPUI layout/drag/SSH-terminal roundtrip checks. The
normal optimized 0.5.6 app built and passed deep/strict signature verification
and --version. No live windows, terminals or SSH sessions were restarted.

## Local 0.5.7 — preview geometry, zoom and installed Quick Actions

Image previews now have explicit aspect-ratio dimensions, removing the oversized
white area. Wheel zoom scales images/documents and native text; a different file
resets zoom. Drag enlarged images to pan. Native text selection/copy remains intact.

Quick Actions previously contained only built-ins. File-input Automator workflows
from ~/Library/Services and /Library/Services now join that menu, filtered by native
UTType conformance, including Finder-scoped workflows such as Measure Render for
PNG. Clicking runs the captured workflow/file via Automator on a worker thread;
opening a menu never executes it. No workflows or system associations are modified.
No-input actions, Shortcuts-only actions and third-party Finder extensions are not
part of this integration. The current context menu exposes actions for one item.

Command-right-click opens Quick Actions directly for local files/folders in List,
Icons, search results and Common/Favourites. Settings → Keybinds → Quick Actions
records shortcuts for built-ins and installed workflows, with modifier requirements,
conflict checks, cancellation and clearing. Bindings persist in the existing central
keymap.txt using escaped stable action IDs. Dispatch respects editor/modal focus,
file type and a single local selection; normal app bindings take precedence.

Image/video Information uses two label/value pairs per row (four columns), with
explicit 12 pt left/right padding, a matching heading inset and full-value tooltips.
Native video presentation dimensions populate the same grid when loaded.

Validation: 87 Rust tests passed, including native UTType matching, shortcut
conflicts and a real disposable Automator input/argument fixture. Native
Nextcloud/Todoist/composer checks passed. Hidden GPUI checks passed for image
aspect/zoom/reset, four-column information bounds and 12 pt side insets,
Command menu routing, shortcut record/reload/cancel/clear, plus existing drag/PTY
regressions. Native text smoke checks preserve selection/copy through zoom.
The optimized normal Schnauz.app 0.5.7 is built; --version and deep/strict stable
Apple Development signature verification passed. It was not launched into the
user's live profile and no windows/terminals were restarted. Published release
remains 0.5.5; no publication requested. Actual user-workflow execution and mouse
interaction remain Oliver's manual check.

## Local 0.5.8 — compact Information and permanent sidebar shortcut visibility

Fixed the shared section-heading renderer to keep Information horizontal at a
full definite width. Added heading-height/overlap regressions, since the earlier
four-column tests did not catch its vertical wrapping. Created/Modified/Last
opened use native locale-aware date and time formatters on two separate lines,
with one-line labels, 13 pt line heights and 2 pt row gaps. Four columns and 12 pt
side insets remain. Preview minimum width rises 15%, from 220 to 253 pt, consistently
in drag bounds, allocation, central views and preference loading.

Common/Favourites Folders/Files are permanently expanded, including when old
collapse flags were saved. Headings no longer toggle; sidebar width measures all
visible shortcuts. The sidebar folder menu explicitly offers Apply Current View
& Sorting to This Folder & Subfolders, using existing central inherited rules.
It copies the main pane's view/sorting, preserves unrelated folders and sidebar
appearance, and never scans descendants.

Validation: 87 Rust and all native fixture checks passed; hidden GPUI checks
passed for heading bounds, tight date/time geometry at 253/360/420 pt, sidebar
migration and the actual sidebar menu callback's inherited view/sorting scope.
The normal optimized Schnauz.app 0.5.8 is built and passes --version plus
deep/strict certificate signature verification. User sessions remain untouched.
The separate visual QA capture was unusably small, so visual appearance is not
claimed verified; the complete real GPUI layout bounds checks above passed.

## Local 0.5.9 — visible Information, native drag recovery and Nextcloud aliases

The 0.5.8 container checks missed zero-width shaped text. The new regression
reproduced the missing heading/values. Information now derives explicit text
widths from the actual pane allocation and avoids preliminary zero-width
truncation. Checks assert complete heading/value strings and nonzero glyph
bounds at 253/360/420 pt, alongside compact date/time rows and padding.

Native drag dwell now runs in AppKit's tracking mode and presents the destination
frame while the drag is active. Native handoff and completion retire GPUI pointer
tracking and clear stale drag/click suppression state. A real tracking-mode test
checks tab/folder activation, repaint, same-folder cancellation and immediate
selection, plus disposable copy/move and the existing isolated terminal fixture.

Home-level symlink aliases such as ~/Nextcloud are identified from cached startup
routing to CloudStorage. Provider menus and linked-account routing handle alias
paths; downloaded files and folders receive cloud badges in list, icons and
search. Direct sharing still requires Schnauz's own explicit connection. No
desktop credentials, live server or Keychain were accessed for testing.

Validation: all 89 Rust tests, native Nextcloud/Todoist/composer fixtures and
hidden real GPUI text/layout, alias/menu and native drag regressions passed.
Live appearance and live cloud login remain user QA; no screenshot verification
is claimed. Existing windows, settings, terminals and SSH sessions are preserved.
The optimized Schnauz.app 0.5.9 is built with the stable Apple Development
certificate and passes --version and deep/strict signature verification.

Oliver subsequently requested public release with both DMGs and descriptions of
the bug fixes. Published https://github.com/weiaoli2/Schnauz/releases/tag/v0.5.9
from ba395fa98b044c7b6dc4506614b080df7851ac30. Both architectures and all helpers
passed signature/architecture checks; ARM native and Intel Rosetta text-preview
smoke tests passed. Both DMGs verified; exact MPL sources checked byte-for-byte.
All four GitHub asset digests match local SHA-256. Receipt is saved under
dist/0.5.9/RELEASE-VERIFICATION.json. English notes describe fixes and features,
link TechGuyZone, and state Apple Development signing/no notarization. Published
as an early prerelease. Live apps/terminals were untouched. This request is complete.

## Local 0.5.10 — clipboard, terminal scrolling, project moves and video preview

Search now handles Copy/Cut/Paste as text editing before file shortcuts and has
a native editing context menu. Unicode ranges, replacement and meaningful spaces
are preserved; multiline paste becomes a single-line query. Terminal selections
copy automatically on release, with correct forward/reverse half-cell boundaries,
word selection and release outside the viewport. Shift permits selection in
mouse-reporting terminal apps. The visible right scrollbar tracks actual history
and supports dragging/track clicks without writing to the PTY. Native text preview
uses ordinary wheel scrolling and Shift+wheel zoom, preserving selection.

Tabs offer Move to Project… with a destination list and default-on Activate
Project below. Both saved layouts are committed together before moving the live
Tab object into the destination's parked session. Activating restores that same
tab/session; nothing spawns, restarts or reconnects a terminal. Save failure
leaves all ownership intact. Add to Favourites… is the new tab-menu name for the
former View in… action, with destination feedback; it adds a shortcut only.

The MP4 failure was reproduced in a real hidden GPUI window: the native canvas
had 0 px width despite a correctly sized inspector. Explicit pixel widths and
flex-none sizing restore attachment before metadata is ready. AVPlayerItem avoids
optional duration preloading, polls early readiness at 50 ms, and starts paused
and muted. A generated local H.264 fixture tests viewport attachment and playback;
no personal media or live account is used. Public release remains 0.5.9.

Validation passed: all 89 Rust tests, native Nextcloud/Todoist/composer fixtures,
generated MP4 playback/unmute/cleanup and immediate real NSView attachment,
native ordinary-wheel scrolling/Shift-wheel zoom, focused search clipboard
editing, terminal selection/scrollbar positions, and the project-move dialog.
The owned shell keeps the same PID, session and view across background-tab moves,
activation on/off, Personal and split transitions; a failed save leaves it intact.
Clipboard fixtures restore original item data formats, including on failure.
All checks use disposable settings/files and owned fixture shells, not live
sessions or servers. User mouse/trackpad feel and real-world codec interoperability
remain manual QA; no new screenshot verification is claimed.
The optimized Schnauz.app 0.5.10 is built with the stable Apple Development
certificate. Packaged --version, native text scroll/Shift-zoom smoke checks and
deep/strict signature verification passed. No live app, terminal or SSH session
was restarted. No new GitHub publication was requested for this local test build.

## Local 0.5.11 — GitHub update checks and pre-Todoist checkpoint

About Schnauz now contains Automatically check for updates at startup, default
OFF, and “Updates are checked via GitHub.” Manual checks also exist in Settings
and the app menu. The public release list includes published prereleases; plain
tags/drafts are ignored. SemVer comparison prevents false numeric ordering and
downgrades. Only an uploaded DMG matching the installed binary's architecture
is offered, including Intel under Rosetta. Release and download URLs are restricted
to weiaoli2/Schnauz. Discovery is a bounded, timed, credential-free HTTPS read on a
dedicated worker, not an installer. The original upstream updater stays removed.

A small info badge above the toolbar mascot appears for a discovered newer
version, with Update available tooltip and an About link. It does not intercept
the dog's existing stop-search action or open a browser automatically. A locally
newer developer build shows an amber badge and the installed/latest versions in
About; this makes the check testable before publication without offering downgrades. The user
chooses Release Notes or the architecture-specific download. Existing terminals
and SSH sessions are never restarted. GitHub already supports this; no duplicate
homepage DMGs or homepage edits are needed.

Oliver requested committing/tagging this working checkpoint before the next
Todoist change, without a release. Next: one shared floating Todoist window,
following the globally active project; full and first-task compact modes;
always-on-top/all-Spaces; Show Floating Tasks and Dock Here across main windows.
Test before a later release and retain this checkpoint for rollback.

0.5.11 validation: 93 Rust tests and the isolated real-GPUI suite pass, including
both blue available-update and amber developer-build indicators, readable About
status and opt-in preference persistence. Offline native fixtures from the full
suite also passed before the final warning addition. Dependency notices verified
(497 dependencies, 361 texts). Optimized Schnauz.app 0.5.11 is built and signed
with the existing Apple Development certificate; packaged version and deep/strict
signature checks pass. No live windows, terminals or SSH sessions were restarted.
The pre-Todoist checkpoint is an annotated v0.5.11 source tag, not a GitHub Release.
The next Todoist change uses a small pin at the panel's upper-right to detach it.

## Local 0.5.12 — detachable Todoist test stand

The pre-change source is committed as 93e1c7d and tagged v0.5.11 on GitHub main;
no GitHub Release or DMGs were published. Keep that checkpoint for rollback.

Todoist's task cache, helper requests and linking now live in one shared controller
which owns no browser or terminal. The upper-right pin detaches a single floating
window. All browser task panes close; Show Floating Tasks reuses that same window,
and Dock Here returns it to the chosen browser. Its own pin returns to the origin
or another surviving browser. Closing a browser does not retain its PTYs and does
not close the detached tasks. Closing the floater leaves tasks closed. Account
disconnect closes it too. The window follows the global active project and keeps
Add/complete operations bound to their captured account/project, including after
project switches. Existing pagination, backoff and stale-read guards remain.

Compact shows the first open task; Expand restores the previous full size. Normal
full mode keeps sections, subtasks, Add task, Refresh, Relink and Open in Todoist.
Short transient results and cached-data errors also appear in the detached pane;
compact messages stay one line with a tooltip. Each window has its own scroll
handle. AppKit floating level/all Spaces/no hide on deactivation and macOS 13+
CanJoinAllApplications keep it over eligible other applications/full-screen spaces.

Native window destruction during QA exposed an existing browser drop subclass
bug: AppKit's _windowLayerContext KVO forwarding threw an unregistered-observer
exception in NSWindow dealloc. LLDB on the disposable test process identified it.
Restore the original GPUI window class before deallocation; no exception masking
or global GPUI-class changes. Existing drag-copy/move/hover tests still pass.

Final validation passed: 93 Rust tests, offline Nextcloud/Todoist/composer tests,
and the entire hidden real-GPUI suite, including a real native pin click, actual
AppKit floating level/all-Spaces flags, compact first/next task and actual detail
text bounds with an error, global project switches, singleton behavior across two
browsers, Dock Here, and destroying the originating browser without retaining it.
Existing terminal identity/tab-move/drag-copy/move/hover and text/video preview
checks remain green. Dependency notices verified for 497 dependencies (361 texts).
The optimized Schnauz.app 0.5.12 is built and signed with the existing Apple
Development certificate; packaged --version and deep/strict signature verification
pass. No live app/window, terminal or SSH session was restarted; no live account
or Keychain credentials were used for QA. Real account workflow, Spaces/full-screen
behavior and subjective UI feel remain Oliver's manual test.
A public release stays deferred until Oliver tests this behavior in his workflow.

## Local 0.5.13 — distinct Todoist pin states

Oliver confirmed the detached window works and requested clearer, quieter pins.
Use native macOS pin.fill for the docked pane and pin.slash for the floating
window, in the theme's neutral secondary colour with matching subtle hover
backgrounds. Tooltips identify Docked/Detach Todoist and Floating/Dock Todoist.
No change to window/terminal/project ownership or the shared singleton behavior.

0.5.13 validation passed: five Todoist cache/routing tests and the complete
isolated native/UI suite, including the actual pin click, detach/dock, compact
view, project switching, browser destruction and unchanged drag/terminal flows.
The optimized Schnauz.app 0.5.13 is built with the stable Apple Development
signer. Packaged version and deep/strict signature checks pass. No live app,
terminal or SSH session was restarted; no new GitHub Release was published.

## Local 0.5.14 — lighter docked pin

Oliver requested an outline instead of the heavy filled docked pin. Use macOS
pin rather than pin.fill, keeping the neutral colour, size, tooltip and existing
pin.slash floating state. No behavior changes.

Optimized 0.5.14 build and stable Apple Development signing passed. Packaged
--version and deep/strict signature verification passed. Live windows and
terminal/SSH sessions were not restarted.

## 0.5.14 publication requested — 2026-10-19

Oliver approved the working Todoist window and outline pin, then requested a
commit and release. This supersedes the temporary post-checkpoint release hold.
Prepare both signed architecture-specific DMGs via release.sh and publish an
early GitHub prerelease after verification. English release notes include all
changes since public 0.5.9: clipboard/terminal scrolling, project-tab moves with
PTY continuity, text/video preview fixes, opt-in GitHub updates and floating
Todoist. Keep v0.5.11 and all existing public releases unchanged. Website and
video production are unrelated working files and are excluded from publication.

## Published 0.5.14 — 2026-10-19

https://github.com/weiaoli2/Schnauz/releases/tag/v0.5.14

Source commit 5b26dd7fcc296acd9e950a7fbadf5331f9bd31cb, annotated tag v0.5.14.
Published as an early prerelease with signed Apple Silicon and Intel DMGs,
exact MPL component sources, SHA256SUMS.txt and English notes covering changes
since public 0.5.9. All four uploaded asset digests and sizes match local files.
GitHub's source ZIP matches the tagged local archive exactly (1725 files).
Every app/helper architecture and certificate signature passed, and both DMGs
were mounted readonly and compared against the verified app bundles. ARM ran
natively; Intel native text-preview checks passed under Rosetta, not physical
Intel hardware. The complete 93-test Rust suite and native/GPUI fixtures passed.
The unauthenticated update-check endpoint exposes both exact architecture DMGs.
Apple Development signed, not notarized. Receipts in dist/0.5.14 include local,
source and public-release verification. README now points to the public downloads.
Website/video work, credentials and local settings were excluded. No live app,
terminal or SSH session was restarted. Earlier releases and v0.5.11 are preserved.
This request is complete; later releases require fresh authorization.

## Local 0.5.15 — sidebar ordering, file opening and project activity

Projects and the references within each Common/Favourites Folders/Files group
can be manually reordered by dragging. Upper/lower row halves insert before/after,
with a thin accent line and fixed row geometry. Personal, Home and section order
remain fixed. Central projects.json keeps the order across restarts and shares it
between windows. Typed drag data captures the owning project/group; mismatched
sections, stale items and Home are rejected. Sorting changes references only,
without browsing, file transfers or terminal input.

Single-clicking a sidebar file still reveals/selects it. Double-click opens through
the existing Schnauz app choices; remote files use the existing download/open flow.
A 5 pt green dot before the project icon indicates live tabs/terminals in any
browser, including parked sessions. Saved layouts alone do not count. Runtime
activity holds no browser/PTY references and clears when its owner is released.

Validation: all 95 Rust tests and offline native fixtures passed. Hidden AppKit
pointer tests verify actual sidebar drag/drop, persisted order, no drag navigation,
and single/double-click file routing with external launch intercepted. Existing
real-window checks also verify parked activity across two browsers and its cleanup
when the owning browser is destroyed, alongside unchanged Todoist, native file
drag, preview and terminal identity regressions. No live settings, accounts,
Keychain credentials, user windows or terminal/SSH sessions were used for QA.
No commit or public release requested; published version remains 0.5.14.

The optimized normal Schnauz.app 0.5.15 is built with the existing stable Apple
Development certificate. Packaged --version and deep/strict certificate signature
verification passed. The running app, windows, terminals and SSH sessions were
not restarted. Visual feel and live remote-file interoperability remain manual QA.

## Local 0.5.16 — tab shortcuts and compact collapsible sidebar groups

Dragging a folder tab onto Common or elsewhere in the expanded sidebar now adds
its folder reference to Common or the captured project's Favourites. The payload
freezes folder/server/project, plus the owning window and stable tab identity for
ordinary tab rearrangement. Sidebar drops preserve all live tabs and terminals,
keep remote Host aliases without live mux ownership, and never scan remote paths,
transfer files or reconnect. Duplicate references stay deduplicated. Existing
small drop hints identify Common/Favourites without painting destination rows.

Clicking PROJECTS or COMMON toggles that group with no additional icon and saves
its state. PROJECTS already had this behavior; it now has a discoverable tooltip
and actual AppKit click coverage. Collapsed Common remains a reference-drop
target and expands after a drop. Common/Favourites headings use 8 pt above and
no bottom padding; Folders/Files use 4 pt above and 2 pt below. Rows remain 24 pt.
This supersedes the earlier always-expanded whole-Common guidance; individual
Folders/Files labels and Favourites retain their existing behavior.

Final validation: all 95 Rust tests and the offline native Nextcloud/Todoist/
composer fixtures passed, followed by the full hidden GPUI/AppKit suite. The new
fixture clicks PROJECTS/COMMON labels and verifies saved collapse state, drags
local/SSH tabs to Favourites/Common (including collapsed Common), checks hints,
deduplication, outside cancellation, saved Host alias/no mux, and unchanged tab,
terminal view, session and real owned shell PID. Ordinary tab reorder and stale
payload rejection also pass. No live servers, accounts or user sessions used.
No commit/publication requested; public release remains 0.5.14.

## Local 0.7.0 — Weblinks, active projects and safe quit

Common and each project's Favourites now include Weblinks, with Add weblink…
always available. Each bookmark stores a stable ID, display name, HTTP(S) URL and
optional browser application path in projects.json. Old project files default to
empty bookmark lists. Single-click opens once; dragging reorders without opening.
Context menus offer Open, Edit, Copy link and Remove. Edit has native name/link
fields with normal clipboard shortcuts and a browser picker (macOS Default plus
registered installed handlers). Browser enumeration occurs only on explicit
editor opening. Each bookmark's browser choice is independent of system defaults;
missing chosen browsers produce an actionable footer error without fallback.

The signed native/weblinks/ helper receives/returns form data over stdin/stdout
and waits on a dedicated worker, preserving responsive file/terminal work. Rust
validates returned fields, captures the original project/Common destination,
rejects removed/stale edits and retains forms after save/helper failure for retry.
Native previews hide while the editor is open. Tests intercept URL launches and
use synthetic bookmarks, temporary settings and browser-name fixtures.

0.7.0 also guards Quit against active terminal work. A weak browser registry
captures read-only probes from current and parked project sessions across windows.
The explicit quit scan reads owned process trees/names off-thread, including
background/suspended jobs; idle shells do not prompt. A native confirmation groups
active processes by project with Cancel / Quit Anyway. An owned application
delegate subclass intercepts Dock termination too, with one explicit authorization
for normal GPUI shutdown. No terminal input, remote probing, session restart or
process signal is used by the check. Actual test quitting is intercepted.

The existing 56 pt toolbar now aligns the search field with the Schnauz title.
A compact horizontal active-project strip replaces the lone project subtitle.
Chips use the same all-window live tab/session definition as sidebar dots; the
current project is blue. Clicking uses the existing globally synchronized switch
and retains all live terminals. Per-window scrolling reveals the current chip
on project/order changes; no extra toolbar height or browser/header displacement.

Oliver selected version 0.7 for this feature stand; Cargo/app/CLI use 0.7.0.
The reload icon beside All/Folders/Files now renders only for SSH/SFTP tabs;
local/cloud browsing keeps automatic watching and Cmd+R still refreshes manually.

Final verification before packaging: all 96 Rust tests passed, plus offline
Nextcloud/Todoist/composer and native Weblinks form/browser-choice checks. The
complete hidden GPUI/AppKit suite passed for bookmark click/browser routing,
reorder persistence, Projects/Common collapse, tab-to-sidebar reference drops,
remote-only reload visibility, title/search alignment, native active-project
chip switching and unchanged owned terminal/session/PID. Quit checks cover idle
shell bypass, foreground/background work, exec replacing a shell, parked project
labels, the AppKit delegate path, Cancel and explicit Quit Anyway. Actual quitting
and URL launches are intercepted; no live user process, server, account, credential
or settings profile is used. Publication is not requested; public remains 0.5.14.

The optimized local Schnauz.app is now packaged as 0.7.0, including weblinkctl,
signed with the existing Apple Development identity. CLI version and deep/strict
bundle signature verification passed. Live Schnauz was not restarted and no
commit, tag or publication was made. Oliver will test this build first; wait for
his explicit release go-ahead.


Local 0.7.1 adds optional Alias and Add-on for saved folder references in project
Favourites and Common. Right-click → Edit alias and add-on… opens native fields.
Empty alias uses the folder basename; add-on displays as `Name (Add-on)`.
Labels persist centrally per reference without renaming folders, changing routing
or disturbing live tabs/terminals. Re-adding the same destination retains its
label; stale editors reject changed/removed references. Sidebar sizing and drag
previews use the displayed label. Hovering a tab shows its full folder path and,
for SSH tabs, server identity. No publication or live app restart is implied.

0.7.1 verification completed: all 97 Rust tests, offline native services/composer
checks, the native alias/add-on form checks and complete hidden GPUI/AppKit
suite passed via ./test.sh. The optimized Schnauz.app was built with the existing
Apple Development signer; CLI/bundle version 0.7.1 and deep/strict signature
verification passed. Live app windows, terminals and SSH sessions were not
restarted. No commit, tag or publication was made.

Local 0.7.2 fixes cancellation of the active-terminal quit warning. It now owns
one standalone native NSAlert and explicitly ends its application modal session
and orders the dialog out before clearing the pending guard and refreshing the
browser windows. Cancel keeps the app and terminal programs running. AppKit owns
the button response tags; Escape cancels. Modal-session polling happens outside
GPUI borrows. Native sheets are avoided because attaching one to the dynamically
adapted browser window exposed an AppKit KVO crash in the real-dialog regression.
No dependency cache or global AppKit classes were modified.

The complete hidden UI suite passed, including two actual native Cancel button
clicks, no remaining visible warning/modal session, duplicate-quit prevention,
repeat quit after Cancel, and identical owned terminal/job/PID. Existing checks
for idle shells, parked projects, AppKit delegate routing and Quit Anyway remain
covered. Tests use disposable profiles and fixture terminals; no live user
windows, terminals, SSH sessions, accounts or settings were changed.

The optimized 0.7.2 Schnauz.app is packaged with the existing Apple Development
signer. CLI version and deep/strict bundle signature verification passed. The
live app was not restarted; no commit, tag or release was made.


Local 0.7.3 adds immediate SSH connection feedback in the existing bottom-center
blue footer badge, including saved remote favourites and manual remote reloads.
It reports Checking SSH settings…, Connecting… and connection/folder loading,
with the server identity. Progress belongs to the captured tab and load generation,
clears on completion/failure, and takes precedence over older general notices.
Tab switches/reordering cannot apply a connection result to a different tab;
selecting a pending connection does not start another listing. Live terminals
and SSH authentication/transport behavior are preserved.

Verification: 98 Rust tests and the offline native fixtures passed via ./test.sh.
The updated hidden-window suite checks synthetic SSH progress stages, footer
bounds, unchanged file-body geometry, notice priority and completion cleanup,
without contacting a server or accessing credentials. No publication requested.

The optimized 0.7.3 Schnauz.app is packaged with the existing Apple Development
signer. Packaged CLI/bundle versions and deep/strict signature verification
passed. The complete updated hidden UI suite passed. No live app/session was
restarted, and no commit, tag or public release was made.


Local 0.7.4 adds a terminal Select text button for marking/copying text while
Codex or another foreground TUI owns the mouse or continually redraws. It holds
a bounded local copy of the current terminal grid and scrollback. The live PTY,
parser and protocol replies continue normally. The button becomes Live terminal;
typing, pasting or Escape returns to the live output. Shift-drag in alternate
screen/mouse-reporting mode uses the same stable selection. Cmd+C copies the
snapshot; Cmd+V resumes and respects the live bracketed-paste mode.

The new native regression clicks the actual Schnauz header button and sends
AppKit plain/Shift mouse drags across a screen clear/redraw, checking selected
text, automatic copy, routed Cmd+C and exact bracketed Cmd+V bytes. Input is
captured in a test buffer and the original owned fixture session/PID is preserved.
The complete hidden UI suite passed. No live Codex/SSH session was touched.

0.7.4 final validation: all 98 Rust tests, offline native fixtures and the full
hidden GPUI/AppKit suite passed via ./test.sh. The optimized Schnauz.app is
packaged with the existing Apple Development signer; packaged CLI/bundle 0.7.4
and deep/strict signature verification passed. The currently running app was
not restarted. No commit, tag or public release was made.


Local 0.7.5 reuses the top Terminal button’s vector icon at 11 pt on the right of each tab,
before Close, while its terminal session is alive. It stays visible with the
terminal panel hidden and has a Terminal session active tooltip. Tabs without a
terminal or with an ended session have no indicator. No terminal/session behavior
changes, live app restart or publication is implied.

0.7.5 validation: the existing complete hidden UI suite passed, including tab
selection/reordering and terminal identity regressions. The optimized app is
packaged with the existing Apple Development signer; packaged --version 0.7.5
and deep/strict bundle signature verification passed. Live sessions were not
restarted. No commit, tag or public release was made.


Local 0.7.6 adds the favourite-folder Globe vector icon to SSH browser tabs,
beside the terminal indicator before Close. Both indicators are black in light
mode and white in dark mode, for active and inactive tabs. Globe visibility
follows the tab's remote browsing state independently of its terminal session.
No remote reads or connection changes are involved.

0.7.6 packaging verification: optimized normal app build passed, packaged CLI
reports 0.7.6, and deep/strict stable-certificate signature verification passed.
This small rendering change reuses existing icons without altering sessions.
No live app restart, commit, tag or public release was performed.


## Local 0.7.7 — keep setup for next start

Quit and closing the final browser window now ask Keep Setup & Quit, Start Fresh
Next Time or Cancel. The existing warning for active terminal work still comes
first. The setup dialog is a standalone native modal; every outcome ends its
session and hides it. Cancel leaves the app and startup choice unchanged. A save
failure keeps the app open with a footer error. Start Fresh retains all saved
projects/favourites and preferences and opens Personal/Home next time.

An explicit Keep writes startup-setup.json atomically, separately from the
ordinary local project-layout autosaves. It remembers browser windows/frames,
active and parked projects, local/SSH tabs and their order, active panes/tabs,
splits, views/columns/sorting, type/hidden filters and selections. Terminal panels
remember presence, hidden/open state, height and cwd. Current-project terminals
start fresh on launch; parked-project terminals start when that project is
visited. The original local layout rules during temporary SSH following stay
unchanged. Saved server routing preserves Host aliases and excludes live sockets
and credentials. SSH restarts use the normal new-session transport. Commands,
output, jobs and environment are not resumed. Missing local folders stay visible
with their normal read error; remote paths are never checked against local files.
Docked Tasks can restore without adding its window extension a second time;
Disk Usage scans remain explicit.

Validation: 100 Rust tests and offline native service/composer fixtures passed.
The full hidden GPUI/AppKit suite passes, including real Keep/Start Fresh/Cancel
buttons, no residual modal session, unchanged saved projects/favourites and owned
fixture PID. Restore QA starts separate disposable shells and checks hidden/open
panels, height, selected tab and distinct shell/browser folders. Remote roundtrip
fixtures prove no local namesake reads, credential storage or live-socket reuse.
No live app/window/terminal/SSH session was restarted and no publication requested.

0.7.7 final packaging: optimized Schnauz.app built with the existing Apple
Development signer; packaged --version 0.7.7 and deep/strict signature verification
passed. The cold-library startup regression also passes for Keep and Start Fresh,
retaining all saved project records. The currently running app was left open;
the new quit prompt becomes available when the user launches 0.7.7. No commit,
tag or public release was made.


## Local 0.7.8 — terminal drawing and network drives

- Terminal text now uses GPUI's own vertical centering once, with explicit
  content/cell clipping that keeps the right-hand scrollbar clear. Hidden/wide
  spacer cells cannot overpaint the last line. Wheel input obeys negotiated
  legacy/UTF-8/SGR mouse reporting, wheel counts and modifiers; alternate-screen
  arrow scrolling requires ALTERNATE_SCROLL. Shift-wheel browses local history.
- `src/network.rs` and `native/network/` add a globe toolbar control immediately
  after Search. The native form supports SMB/AFP, name/IP/share URL, username,
  secure password, Guest, read-only, optional folder selection and default-on
  Keychain remembering. Public NetFS presents share selection and mounts the drive.
  Connected folders open in new tabs; file operations use ordinary mounted paths.
- Typed network references in Favourites, saved projects and startup setups retain
  server/share and share-relative paths. Credentials live only in Schnauz's own
  Keychain service. No password URLs, argv, Rust IPC or saved JSON. A different
  mount name is remapped on reconnect. Dragging a network tab or its files to
  Common/Favourites preserves that reference. Network tabs carry the globe badge.
- Mount/dialog waits and network listings use dedicated workers. Disconnected
  restored tabs show Retry without automatic login. Errors stay in the pane or
  existing footer; no automatic unmount and no live session replacement.
- Validation uses Rust persistence/identity regressions, native offline address,
  path/credential binding and form geometry fixtures, and actual hidden GPUI
  toolbar/footer, tab drag and disconnected-session checks. No live SMB server or
  Keychain is used in tests. A real server login/share selection remains manual QA.

- Release privacy: `scripts/check_bundle_contents.py` enforces an exact bundle
  file/directory allowlist before signing and DMG creation. Unknown files and
  symlinks fail packaging, including accidental profiles/settings/session files.
  Disposable fixtures exercise these rejection paths in `test.sh`. Packaging
  never reads Keychain or the user's Application Support folder.

- About Schnauz now has explicit Schnauz website and GitHub links below its
  version heading. Website: https://techguyzone.com/apps/schnauz/ (HTTP 200 after
  canonical trailing-slash redirect); source: https://github.com/weiaoli2/Schnauz.
  Only clicking opens a browser; About layout checks cover both link bounds.

0.7.8 final validation: 101 application tests and 60 terminal tests passed, with
native offline service/form fixtures and the complete hidden-window UI suite.
About-link changes passed the UI suite again. Bundle privacy rejection fixtures
passed; final optimized Schnauz.app was signed with the existing Apple Development
identity, validated against the file allowlist, deep/strict signature checked,
and reports Schnauz 0.7.8. Live windows/PTYs were not restarted. No commit,
publication, live SMB connection or credential/Keychain inspection was performed.


## Local 0.7.9 — centered project selection and website feature refresh

The active-project row is centered across the whole browser window. Its auto
margins collapse for horizontal overflow; selection reveal and native chip
switching retain the existing per-window scroll handle and live terminals. A
1 pt themed divider, 1 pt space above and 2 pt below separates it from the title /
Search row. Browser header grows only 4 pt (60 total); Settings keeps its 56 pt
header. Native traffic-light clearance stays on the first row only. Existing
hidden-window checks verify centering, separator geometry and owned PTY/PID.

The main website kit is refreshed for the current 0.7 series, including named
web links / per-link browsers and Schnauz-only file-extension app choices separate
from Finder defaults. It also includes the other additions since the 0.5.4 kit;
FEATURE-STATUS.md maps the feature claims and public/local distinction. Original
0.5.4 screenshots and branding remain unchanged and labeled. Previous kit/ZIP
preserved under website/Schnauz-Website-Kit-Archive/2026-10-20-before-feature-refresh.
Refresh the kit checksum manifest and ZIP together; website deployment is separate.

Terminal, Preview and Todoist toolbar buttons (including floating-task Dock Here)
now use a 26 pt background instead of 30 pt, keeping their text/icon sizes and
horizontal spacing. Other shared dialog/button styles are unchanged.

Toolbar toggle backgrounds share TOOLBAR_CONTROL_H (26 pt) with the Search
scope and File/Content groups, matching their overall height while keeping the
existing text sizes. First-row vertical centering stays consistent.

The current project chip and active Terminal/Preview/Todoist buttons now share
CHROME_HIGHLIGHT (#3478c5), with the same darker hover blue. This avoids mixing
project blue and toolbar indigo; other dialog accents follow the existing theme.

0.7.9 final verification: ./test.sh passed all 101 application tests, 60 terminal
tests and offline native/hidden-window fixtures. The final shared-highlight change
passed ./test-ui.sh again. The optimized normal Schnauz.app includes the final
project blue on active toolbar toggles, was signed with the existing Apple
Development identity, passes deep/strict signature and bundle-privacy checks,
and reports Schnauz 0.7.9. The 13 website handoff files, relative links, manifest
and refreshed ZIP were checked; all seven original image/branding files are
byte-identical to the archived snapshot. No deployment, commit or public release;
live Schnauz windows, terminals and SSH sessions were not restarted.

## Local 0.7.10 — symmetric project-row spacing

Adds 2 pt below the active-project chips, matching the 2 pt gap above them
between the row divider and the bottom border. The browser header is now 62 pt;
Settings retains its independent 56 pt header. Hidden-window layout checks assert
equal clearance above and below the chips. No live windows or terminals restart.

Validation: ./test-ui.sh passed, including symmetric chip clearance and native
project switching with preserved terminal PID. Optimized Schnauz.app built with
the existing certificate, passed deep/strict signature and bundle-content checks,
and reports 0.7.10. No commit or public release.

## Local 0.7.11 — Add Weblink from section menus

Right-click COMMON or FAVOURITES for Add Weblink…. Removes the inline plus/add
rows and hides empty Weblinks subsections. Collapsed Common keeps its context
menu; creation captures Common/current project when the menu opens. Failed-save
drafts remain available by choosing Add Weblink again. Existing link menus and
drag ordering remain in place. The network-drive globe moves from the top toolbar into each pane’s file action
icon bar, between Trash / Add to Favourites and the hidden-items toggle. It
captures that pane before opening the connection dialog.

Validation: ./test-ui.sh passed with the network control measured in the file
action row (107 pt below the top), including existing bookmark interaction,
sidebar ordering and preserved terminal-session checks. Live sessions untouched.
Optimized Schnauz.app 0.7.11 is packaged with the existing Apple Development
certificate; deep/strict signature and bundle-content verification passed.
No commit or publication.


## Release 0.8.0 — authorized 2026-10-20

Oliver requested public GitHub release 0.8 with notes and both builds after the
shared-label change. Cargo/app version is 0.8.0. Alias and add-on edits now update
all references to the same destination across Common, Personal and every project,
including files. No scope option or confirmation. Provider identity stays part
of matching; stale/removed editors cannot mutate other copies. Clearing labels
also applies everywhere. No paths, order, credentials or live sessions change.

Release scope includes the accumulated 0.6/0.7 work since public 0.5.14: sidebar
ordering and activity dots, tab reference drops, text-only group collapse,
Weblinks/native browser choices, active-project header chips, safe Quit and
optional next-start setup restoration, SSH loading feedback, folder-tab tooltips,
terminal selection snapshots/drawing/scrolling, tab terminal/SSH indicators,
network-drive connections and typed references, About links and compact chrome.
The final network globe is in the file action icon bar; Add Weblink is in the
Common/Favourites context menus. Source publication excludes website/video work,
test.csv, profiles and build caches. Use dist/0.8.0 and preserve prior tags/assets.

0.8.0 validation: ./test.sh passed 102 application tests, 60 terminal tests,
offline native service/form fixtures and the complete hidden GPUI/AppKit suite.
Shared-label cases cover files/folders, all projects, clearing, stale-edit
rejection, provider separation and JSON persistence. No live accounts used.


## Corrected release 0.8.1 — selected preview follows external saves

Before the 0.8.0 draft was published, Oliver reported stale text after opening a
selected file in an editor and saving it. Folder vnode events do not reliably
report in-place writes, and listing refresh did not invalidate preview caches.
The watcher now includes only explicitly selected local preview paths, alongside
visible folders. File events invalidate cached text/image/info and cancel pending
preview/info work, then reload asynchronously. Atomic replacement reattaches the
file vnode; unrelated folder activity does not reset a native text selection.
No recursive scanning or protected-folder probes; SSH previews stay on their
existing explicit remote path. A hidden native UI regression verifies actual
NSTextView content after same-length overwrite, atomic save and rapid edits.

Full ./test.sh passed (102 application tests, 60 terminal tests, offline native
fixtures and the extended hidden-window suite). Tag v0.8.0 / 8ae32d2 and its draft
are preserved as unpublished preparation. Continue the authorized release with
0.8.1 builds/notes in dist/0.8.1, without restarting live sessions.

Final 0.8.1 hidden UI checks passed on the final watcher update. The setup-dialog
regression now compares project identities/settings and all favourite references
separately from ordinary background tab-layout autosaves, avoiding a timing-only
assertion while preserving the data-safety checks. External-save QA launches no
editor and uses only disposable files.


Published 0.8.1 on 2026-10-20:
https://github.com/weiaoli2/Schnauz/releases/tag/v0.8.1
Source: c8a427520bf53ddbf32f8421cdd59075f0f877da. Both signed DMGs, exact MPL sources and SHA256SUMS
match all four GitHub asset digests. The public unauthenticated release endpoint
exposes both builds, and the GitHub source archive matches all 1741 tagged files.
All helpers and both app bundles passed architecture/signature checks; mounted
installers match the verified bundles/notes. ARM ran natively, Intel under Rosetta
on Apple Silicon, not physical Intel hardware. Apple Development signed,
unnotarized early prerelease. Receipts: dist/0.8.1/{LOCAL-,SOURCE-,DRAFT-,RELEASE-}VERIFICATION.json.
v0.8.0 / 8ae32d2 and its clearly superseded draft remain unpublished and intact.
No running app, terminal or SSH session was restarted. This publication request
is complete; future releases require a new request. Preserve both tags/assets.


## Local 0.8.2 — close open projects

Sidebar and top-bar project context menus offer Close while a project is open,
including Personal. Explicit Close drops that project's tabs/terminals across
browser windows and clears its activity indicators, retaining saved metadata.
Closing the selected project's final tab switches to the next open project in
sidebar order with wraparound; other windows retain their own tabs on individual
tab closure. With no other open project, a fresh Personal/Home tab remains.
Persistence failure leaves tabs intact. The new hidden-window checks cover the
menu callback, inactive close, next-tab identity, split/final-tab closure, fallback,
save failure and multi-window parked-session cleanup. ./test.sh passed 102 app
and 60 terminal tests plus offline fixtures; the updated ./test-ui.sh passed.

The reported 0.7.11 on relaunch was the ordinary repository-root Schnauz.app:
release.sh built published 0.8.1 under dist/0.8.1 without updating that launch
path. The release guide now requires verifying/updating the ordinary local bundle
as well. Published 0.8.1 artifacts stay unchanged; 0.8.2 is local only.

0.8.2 final packaging: the ordinary repository-root Schnauz.app now reports
0.8.2 in both Info.plist and packaged --version. Optimized build, bundle allowlist
and deep/strict Apple Development signature verification passed. The original
running process (PID 22631) remains alive; Oliver must quit/reopen to load the
updated executable. No commit, tag or public release was made.


## Local 0.8.3 — smaller mascot status dot

The update/developer indicator above the toolbar Schnauz is a plain 5 × 5 pt
coloured dot, matching the sidebar project activity dot. The former 13 pt circle
and white i are removed. Existing blue/amber status colours, tooltip and About
click action remain. ./test-ui.sh passed, including both status-dot size checks.


## Local 0.8.4 — lower mascot status dot

Moves the 5 pt toolbar status dot down by 4 pt, from top -4 pt to 0 pt.
Size, colours, tooltip and About action stay unchanged. Live sessions are preserved.

0.8.4 validation: the isolated native UI suite passed and the optimized local
bundle was built with the existing certificate and bundle-content verification.
Superseded locally by the following combined test stand; no live restart.

## Local 0.8.5 — reorder file action icons and create text files

Drag file-action controls left/right to reorder them; an inset insertion marker
keeps layout stable. The shared prefs order updates all windows and survives
restart. List/Icons remains one movable group; narrow/remote panes retain their
existing visibility rules without discarding hidden controls from saved order.
The new document-plus icon defaults immediately after New Folder and opens the
existing named .txt creation dialog. Menu actions trigger on click release so
Share, Sort and View Options remain draggable without opening during a drag.
The mascot status dot also includes the requested 4 pt downward adjustment.

Validation: ./test.sh passed 103 application tests and 60 terminal tests, offline
native fixtures and the hidden GPUI/AppKit suite. New actual mouse-drag checks
cover both directions, inactive and menu controls, List/Icons, network, outside
cancellation, disk reload and unchanged tab/path with no accidental actions.
The actual new-file button opens its naming dialog and creates an empty .txt in
a disposable folder. No live window, terminal, SSH, account or server is used.


## Local 0.8.6 — terminal sizing, editor input and Close

Terminal dimensions now update during layout before drawing, with the emulator
resized and its lock released before notifying the owned PTY. Rendering and
mouse coordinates share the same measured font metrics. New PTYs discard inherited
COLUMNS/LINES overrides. Modified navigation/function keys retain xterm modifiers;
Home/End respect application mode, Alt+Backspace/Enter retain Meta, and Shift-page
keys reach full-screen apps. Wheel-to-arrow counts match mouse-report counts;
fractional trackpad movement cannot cross between local/app scrolling modes.
Frozen text selection explicitly blocks wheel input to the foreground program.

A vector Close cross sits immediately right of Hide. Hide preserves the live
session/badge; Close drops only that tab's owned PTY, removes restore intent and
its terminal badge, and returns focus to browsing. SSH closure uses the existing
end-of-session browser cleanup so no dead owned socket remains attached.

QA uses owned fixture PTYs and actual native pointer events, with vi reporting
rows/columns after narrow/wide resizes and multiline command output scrolling
above its bottom prompt. Existing user sessions remain untouched. Oliver's
reported Codex-over-SSH overlap still needs confirmation in his workflow; the
screenshot alone does not prove the narrower transcript is a terminal-size fault.
No commit, tag or public release is requested.

Synchronized redraws now enforce VTE's 150 ms deadline even when no further
output arrives. Resizing flushes a pending redraw against its original geometry
before the grid changes, preventing an old buffered frame from later overwriting
the resized command/input area. Regression covers deadline expiry and late end
markers after resizing.

0.8.6 final validation: ./test.sh passed all 104 application and 62 terminal
tests plus offline native and hidden-window fixtures. Final optimized normal
Schnauz.app reports 0.8.6; bundle allowlist and deep/strict Apple Development
signature verification passed outside the restricted sandbox. No live app,
terminal or SSH session was restarted; no commit or publication was made.


## Local 0.8.7 — terminal-divider crash and local diagnostics

The reported 2026-10-22 16:45:37 SIGABRT followed an upward terminal-divider drag.
An actual hidden native mouse-drag regression reproduced a Rust panic: the file
list scrollbar clamped between a 28 pt minimum and a 19 pt viewport. Bound the
minimum to available space in list painting/dragging, horizontal bars and static
bars. Preserve live terminals and their PTY identity during resizing.

Normal launches now retain 12 private, bounded local session logs under
~/Library/Logs/Schnauz (isolated profiles use their own diagnostics subfolder).
Rust panics force a stack trace regardless of RUST_BACKTRACE; uncaught native
exceptions include their native call stack. Fatal signals append a small safe
marker and retain normal macOS crash processing. Raw memory-fault signals do not
produce a custom stack trace. Fixed action labels include terminal-divider starts;
no terminal transcript, commands, credentials or file contents are recorded.
No upload is configured: report destination and automatic/manual sending await
Oliver’s choice. macOS refused the original report with “Log limit exceeded”.


0.8.7 validation: ./test.sh -- --test-threads=1 passed 106 application and
62 terminal tests, offline native fixtures and the hidden-window suite, including
the previously crashing native upward-divider drag with a live vi PTY. Panic and
native-exception reports were verified in a disposable child process. An initial
parallel run had an existing fixture-shell early-exit failure; serialized rerun
passed. The signed optimized 0.8.7 bundle passed packaging checks without restart.

## Local 0.8.8 — clearer project-row separation

Oliver requested four times the vertical clearance around the top project row:
8 pt above and below instead of 2 pt. Browser header becomes 74 pt; the Search
row, project chip dimensions and Settings header stay as before. The actual
hidden-window layout check asserts symmetric 8 pt clearance. Includes the 0.8.7
terminal-divider crash fix and local crash diagnostics; no live session restart.

0.8.8 validation: the final hidden-window suite passed symmetric 8 pt project-row
clearance and the actual terminal-divider drag regression. Optimized normal
Schnauz.app reports 0.8.8; bundle allowlist and deep/strict certificate signature
verification passed outside the sandbox. No live app/PTY/SSH restart or app
release publication was performed.


## 2026-10-23 — protected folders fail after replacing the running app

Confirmed from the reported Downloads denial at 12:22:46: tccd could not resolve
PID 32977 (proc_pidpath_audittoken: ENOENT), failed to construct its attribution
chain, then the kernel denied Downloads reads. lsof still identified its original
executable under .build-cache/app-stage.2z2co4/previous/Schnauz.app, but that path
no longer existed. make_app.sh moved the running app into that temporary backup
and its EXIT cleanup deleted it after installing 0.8.7. The process stayed alive;
its executable did not. Stable signing cannot fix missing process attribution.
No evidence supports resetting TCC or treating this as a new user denial.

make_app.sh now uses scripts/install_bundle.py. It inspects executable paths
(no command arguments), fails closed if inspection or signature checks fail,
and stages a pending candidate while the destination or its helpers are running.
No restart or automatic launch. After Oliver deliberately quits, apply with:

    python3 scripts/install_bundle.py /Users/oliw/CurrentProjects/Schnauz/Schnauz.app

Replaced bundles and superseded candidates are retained outside disposable
packaging roots, protecting a process that races the inventory check. Do not
remove retained bundles while a process could still use them. Pending candidates
are keyed by destination, and each apply verifies their signature again.

Five disposable lifecycle regressions passed, including a compiled owned process
with an unchanged PID/path/inode, deferred apply, rollback, inspection/signature
failure and path aliases. Actual signed packaging against the live destination
also deferred successfully: PID 32977 and the installed bundle/executable inodes
and SHA-256 were unchanged. Receipt: .build-cache/install-preservation-proof.json.
No protected-folder probes, privacy changes, live restarts or publication.
The currently affected process still has a deleted executable; prevention does
not repair that already-running session. Relaunch recovery remains Oliver’s
manual check when his terminal/SSH work can safely stop.

Final safe-install verification: ./test.sh -- --test-threads=1 passed all five
installer lifecycle tests, 106 application tests, 62 terminal tests, native offline
fixtures and the complete hidden-window suite. Signed packaging deferred against
the live destination with unchanged PID/inodes/hash. No commit or publication.


## Release 0.8.9 — authorized 2026-10-23

Oliver requested an updated GitHub release after the confirmed running-bundle
replacement bug. Includes the accumulated verified changes since public 0.8.1:
project Close, draggable file-action order/New Text File, terminal geometry/input
and synchronized redraws, terminal Close, divider-crash fix, local diagnostics,
8 pt project-strip spacing and safer source build installation. Package both
architectures with the stable certificate; retain early-prerelease/not-notarized
status. Preserve all existing releases/tags, local production work and live
sessions. No automatic crash sending, auto-updater or website deployment.
Manual DMG/Finder upgrades must quit the old app before replacing it. Exclude
website/, video/, test.csv and generated bytecode/cache/profile files from source.

0.8.9 final-source validation: ./test.sh -- --test-threads=1 passed 106
application tests, 62 terminal tests, five installer lifecycle tests, offline
native helpers and the complete hidden-window suite. Prospective source files
were reviewed for secrets and production/profile/cache exclusions. Both release
architectures and their artifacts must pass verification before publication.


Published 0.8.9 on 2026-10-23:
https://github.com/weiaoli2/Schnauz/releases/tag/v0.8.9
Source tag: 0ec6e3cd539cf16edee0db3ce79f4fb3889f755d. Both signed DMGs, exact MPL
component sources and checksum manifest match all four GitHub asset digests.
The public unauthenticated listing, exact notes and download URLs passed; the
GitHub source ZIP matches all 1746 tagged files. Every app/helper passed its
architecture/signature checks; readonly-mounted installers match the verified
bundles and install notes. ARM ran natively, Intel under Rosetta on Apple Silicon,
not physical Intel hardware. Apple Development signed, unnotarized prerelease.
Receipts: dist/0.8.9/{LOCAL-,LOCAL-INSTALL-,SOURCE-,DRAFT-,RELEASE-}VERIFICATION.json.

The normal local 0.8.9 build is verified and staged as a pending update because
PID 32977 is still running. Its installed bundle/executable inodes and SHA-256
are unchanged; no restart, privacy change or terminal/SSH input occurred. Applying
that update remains a deliberate after-quit action. This publication request is
complete; preserve every existing tag/asset. Future publication needs a new request.


## Local 0.8.10 — blue project bar

Oliver requested a clearer separation of projects from Search/file tabs. The
project row now has a full-width CHROME_HIGHLIGHT (#3478c5) background including
its existing 8 pt top/bottom clearance. The selected chip retains its blue fill
and white text, with a white 1 pt border. Inactive chips retain their themed
neutral appearance. Header height, chip geometry, scrolling and switching remain
as before. Local test stand only; no commit, release or live session restart.

0.8.10 validation: ./test-ui.sh passed outside the sandbox, including unchanged
8 pt symmetric spacing, native project switching and preserved owned terminal
PID. The optimized candidate reports 0.8.10 and passed bundle allowlist and
deep/strict certificate signature verification. Packaging deferred it while
PID 32977 remains active; the installed bundle/executable inodes and SHA-256
are unchanged. Receipt: .build-cache/blue-project-bar-verification.json.
Apply only after a deliberate quit. No commit or public release was made.


## Local 0.8.11 — project-specific file type filters

Oliver requested Filter: On / Off and Modify… beside Show All/Folders/Files.
Each project and Personal now persists an independent literal file-ending list
and enabled flag in projects.json. Example: .blend1,.bak. Matching ignores case,
accepts optional dots, *.bak shorthand and compound endings, normalizes duplicates,
and validates limits/unsupported patterns. Off preserves the list. Existing
profiles default to Off; directories stay visible and files are never changed.
Rules apply in List/Icons, current-folder filtering, recursive filename/content
search and loaded remote listings. Synchronize tabs, parked projects and other
browser windows in memory without network requests or terminal replacement.
Narrow panes scroll the control row horizontally. Native Modify supports normal
Mac text-edit shortcuts, Save/Cancel, visible validation/save failures and a
captured project target even if another window switches projects. Persistence
must succeed before any visibility change. Includes the local 0.8.10 blue bar.

Validation: ./test.sh -- --test-threads=1 passed 110 application tests,
62 terminal tests, five installer lifecycle tests, native offline fixtures and
the complete hidden-window suite. New checks cover literal/case-insensitive
rules, legacy storage defaults and project isolation, list/icon selection and
reload, recursive name/content search, real On/Off clicks, native Modify
Save/Cancel and Cmd+A, captured-project editing, persistence failure and
cross-window propagation. Owned tab/PTY/PID and disposable fixture files remain
unchanged. No commit, publication or live session restart is authorized by this
feature request.

The optimized 0.8.11 candidate passed the bundle allowlist, version check and
codesign --verify --deep --strict. It is staged as the pending update while
PID 32977 stays active. Installed bundle/executable inodes and SHA-256 are
identical before/after; no live session was restarted. Receipt:
.build-cache/project-file-filter-verification.json. After a deliberate quit:

    python3 scripts/install_bundle.py /Users/oliw/CurrentProjects/Schnauz/Schnauz.app

Then launch the normal Schnauz.app. No Git commit or publication was made.


## Local 0.8.12 — seamless active file tab

Oliver requested removal of the thin grey line beneath the active file tab.
Tabs now have only top/side outlines. The selected tab extends over the strip's
1 pt bottom divider using the content background; inactive tabs use the strip's
divider instead of drawing a separate bottom edge. Preserve the strip height,
tab top alignment, switching and existing project filter/blue-bar changes.
./test-ui.sh passed outside the sandbox, including native tab dragging,
switching, project filters and preserved owned terminal/PID. The optimized
0.8.12 bundle passed the packaging allowlist, version and deep/strict certificate
signature checks. It is staged as the pending update; PID 32977 and the installed
bundle/executable inodes and SHA-256 are unchanged. Receipt:
.build-cache/tab-seam-verification.json. Apply with the same deliberate after-quit
installer command as 0.8.11. No commit/publication or live app restart occurred.


## Local 0.8.13 — lighter native toolbar icons

Oliver requested modern action icons in the path toolbar while retaining the
view icons, with finer strokes throughout. Folder/file creation, Share,
Favourites (bookmark), Trash, network globe, Eye/Eye Off and Search now use
native macOS SF Symbols in Light weight. Existing List/Grid/Columns and other
vector silhouettes stay intact, with stroke width reduced from 1.5 to 1.2 on
the 20-unit grid. Native Share/pin symbols use the same lighter weight. Keep
button geometry, ordering, tooltips, selected/disabled colouring and all actions.
The symbol cache now holds 128 variants for the broader icon set; vector
fallbacks remain available. QA preview: .build-cache/toolbar-icons-preview.png.
./test-ui.sh passed with isolated native windows, including toolbar drag/order,
New Text File, Share symbol decoding and existing project-filter regressions.
All nine requested SF Symbols resolved successfully; the light/dark contact
sheet was visually checked. Optimized 0.8.13 passed bundle allowlist, version and
deep/strict certificate signature checks. It is staged pending a deliberate quit;
PID 32977 and installed bundle/executable inodes and SHA-256 remain unchanged.
Receipt: .build-cache/toolbar-icons-verification.json. Apply with the same
after-quit installer command as 0.8.11. No release, commit or live-session restart.

## Local 0.8.14 — startup-disk Disk Usage scope and totals

Oliver reported Macintosh HD stopping around 50 GB despite a roughly 500 GB
drive. The scanner previously rejected every device other than the root device,
which excludes the paired APFS Data volume from a boot-root scan. Explicit `/`
scans now allow that Data device through the merged macOS namespace, skip its
duplicate backing mount and deduplicate directories by device/inode. Unrelated
mounts and internal symlinks remain excluded. Explicitly selected root aliases
are resolved before scanning; normal browsing still never triggers a scan.

The panel separates logical scanned file bytes from filesystem-reported capacity
and available space. Read errors, excluded mounts and the existing 500,000-item
safety limit are visible above the map with explicit partial/scope details.
Snapshots, filesystem overhead and inaccessible files are not invented as tiles;
the mapped sum is not a promise to match physical disk usage or drive capacity.
Verification uses disposable folders, not a scan of Oliver's startup disk.

Validation: `./test.sh` passed 112 application tests, 62 terminal tests, five
installer tests, offline native fixtures and the isolated native UI suite.
Optimized 0.8.14 passed the bundle allowlist, `--version` and deep/strict signing
verification. It is staged for `./update.sh` after a deliberate quit; the running
app and all live terminals remain untouched. Logs: `.build-cache/disk-usage-tests.log`
and `.build-cache/disk-usage-build.log`. No commit or publication requested.

## Local 0.8.15 — audio playback in Preview

Audio selections now route through the existing embedded AVKit player alongside
video. The native inline controls occupy a compact 88 pt viewport with seeking
and sound controls; the preview header retains Play/Pause and Mute. Audio starts
paused and unmuted, video remains paused/muted. Selection changes, Preview close
and native-modal blocking stop/pause playback using the existing session lifecycle.
Recognized music extensions include MP3, M4A/AAC, WAV, FLAC, AIFF and CAF; actual
decoding follows macOS and rejected/corrupt files show an audio-specific error.
Remote SSH files retain the existing local-media restriction.

QA generates silent disposable WAV and AAC fixtures; player integration checks
silently exercise initial state, playback, mute, modal pause and cleanup. The real
isolated GPUI preview check also selects AAC and verifies native attachment,
compact height, paused/unmuted state and removal after changing selection.

Validation: `./test.sh` passed 113 application tests, 62 terminal tests, native
offline fixtures and isolated native UI checks, including WAV/AAC playback and
actual audio preview attachment/cleanup. Optimized 0.8.15 passed the bundle
allowlist, version and deep/strict certificate checks; it is staged for
`./update.sh` after a deliberate quit. No live app/session was restarted.
Logs: `.build-cache/audio-preview-tests.log`, `.build-cache/audio-preview-build.log`.

## Local 0.8.16 — compact document-preview margins

Oliver approved reducing PDF/document preview padding from 16 pt outer + 8 pt
inner to 6 pt outer + 2 pt inner on every side. Available image width now deducts
17 pt (both insets plus the divider), and Information width deducts 13 pt. Images
and native media retain their existing edge-to-edge/compact layouts. The isolated
native preview check measures a synthetic document's outer/inner offsets and
available width alongside the existing picture/zoom/audio/video checks.

Validation: `./test-ui.sh` passed with measured 6 pt outer/2 pt inner document
padding, existing picture zoom, audio/video and native session regressions.
Optimized 0.8.16 passed bundle allowlist, version and deep/strict signing checks;
staged for `./update.sh` after a deliberate quit. No live sessions were restarted.
Logs: `.build-cache/preview-margins-tests.log`, `.build-cache/preview-margins-build.log`.

## Local 0.9.0 — Nextcloud menus, availability and mouse-editable Rename

Oliver chose ordinary version 0.9.0 for this build; no publication requested.
The Nextcloud sync-folder identity is separate from Schnauz's own optional
server credential. Detected folders no longer offer “Connect this folder”.
“Enable sharing in Schnauz…” explicitly enables Schnauz's server actions when
no mapping exists. Connected menus retain Share options, browser/internal links
and supported file locks; connection maintenance moves to Settings. Nextcloud's
generic Sync Now entry is removed. Oliver rejected the Finder handoff; it has
been removed entirely from Nextcloud menus. Persistent-offline policy now has
a direct installed-provider bridge; real provider-action QA remains pending.
Never import desktop credentials or fake provider actions.

Cloud file badges now distinguish local data (green check) and online placeholders
(blue cloud), with status tooltips and a provider-menu status row. Failed metadata
is unknown, not downloaded; an available directory does not assert recursive
offline availability. The bounded metadata cache is invalidated after operations.
No folder scan is started merely to construct a menu or paint a badge.

Selected-item context menus, local and SFTP, omit New Folder/Text File/File;
creation remains on empty background and toolbar/menu actions. The separate
Quick Look viewer, context entries and Space handlers are deleted. Embedded
preview thumbnail generation is retained for PDF/document/image previews.

Rename uses a clipped, measured text line with a visible caret, Unicode pointer
positioning, drag/Shift/double-click selection and caret scrolling. The same
editor serves List, Icons and search. Starting/dragging a rename clears file-drag
candidates so text selection cannot start an OS file drag. Native QA exercises
real AppKit clicks, Unicode drag and Shift selection, copying, and menu scope in
all three views with disposable files; no live account/session is used.

Validation: the full `./test.sh` passed 113 application tests, 62 terminal tests,
offline native fixtures and isolated UI checks. The final `./test-ui.sh` also
passed lost-mouse-up/hover recovery for Rename. Oliver authorized publication of
0.9.0 on 2026-10-24 after reviewing changes since 0.8.9. GitHub issues #1 (direct
persistent offline policy) and #2 (return to preserved search results) are not
fully resolved and remain open; do not claim either fixed in the release notes.

Oliver paused release preparation on 2026-10-24 while discussing the rejected
Finder handoff. The release build/publication has not started. Continue the
Nextcloud corrections locally; wait for his instruction to resume publication.


### Direct Nextcloud provider bridge — 2026-10-24

The reference client is Nextcloud Desktop 34.0.1, inspected on macOS 15.7.4.
Older clients need not be supported. LaunchServices discovers the installed app;
provider metadata supplies item/domain routing. No personal account/path, SwiftBar
installation, ncstatus configuration or desktop credential is required.
`native/nextcloud/Local.m` invokes Nextcloud's declared KeepDownloaded action via
internal macOS File Provider interfaces with checked method signatures. The
NSError-first completion contract was checked using an owned, unscheduled runtime
operation on Apple Silicon and under Rosetta. This does not prove live pinning.
Only verified nonlocal Nextcloud files/partly available folders offer the action;
kept/downloading items are excluded. A fresh read precedes the write and a fresh
policy check must confirm success. Unknown or unsupported metadata fails closed.

`src/cloud_status.rs` and `src/nextcloud_local.rs` keep bounded metadata caches,
deduplicated queues and dedicated background workers; no metadata I/O, placeholder
reads, full provider dump or recursive scans occur in menu/render callbacks.
Sharing mutations now distinguish confirmed writes from failed refreshes, retain
old rows and require Refresh instead of replaying a successful write. Hidden
AppKit fixtures exercise actual minimum-size light/dark controls and retry flow.
The signed normal 0.9.0 build is staged; the running app remains untouched.
Real provider metadata/action QA needs a specifically approved disposable object.
Release and GitHub issue closure remain paused pending that check.

Final automated verification for the provider bridge: `./test.sh` passed with
123 application tests, 62 terminal tests, native protocol/local-policy fixtures,
the actual hidden sharing window and the complete isolated GPUI/AppKit suite.
The staged 0.9.0 bundle passed deep/strict signature verification and reports
`Schnauz 0.9.0`. These checks do not replace the pending live provider-action QA.

## Local 0.9.1 — Nextcloud metadata, trailing status and text-preview margins

Oliver reported “Nextcloud local metadata is unavailable” for every item.
A read-only helper request against the detected Nextcloud root reproduced the
protocol error: `recursivelyDownloaded` was emitted as numeric `0`, because a C
logical expression promotes BOOL operands to int. Rust correctly requires JSON
Booleans. The native public-response boundary now normalizes numeric flag fields
to JSON Booleans, retaining missing/malformed fields as invalid. An offline
regression round-trips actual JSON and checks every flag's Boolean type for both
0 and 1. No live pin/download action has been performed; issue #1 remains open.

List and search rows show status at the right edge of the Name column, separately
from the file icon. Online-only/partly downloaded folders use Schnauz blue
#3478c5 and Apple's filled cloud symbol; downloaded content uses a green check,
kept-offline content a green pin, in-progress work amber arrows and unavailable
metadata a grey question mark. Tooltips explain the state. Icons view retains
its corner badges. Hidden fixture checks passed right alignment and nonoverlap
with long Unicode names at narrow/wide, reordered Name columns.

Oliver's annotated screenshot showed text-preview spacing, which the earlier
0.8.16 PDF/document-only margin change had missed. The native text branch now
uses 6 pt outer padding, a 6 pt title gap, 2 pt native text insets and zero extra
line-fragment padding. Text size, selection, copying and session reuse remain.
The native text smoke check passed; the first full-window spacing assertion used
the content probe rather than the outer preview frame and was corrected. That
latest assertion has not been rerun.

The Nextcloud/status changes passed the full suite (123 application tests,
62 terminal tests, offline native fixtures and hidden GPUI checks) before the
text-margin change. During this work Oliver reported crashes in his live 0.9.0;
its local diagnostics contain SIGSEGV markers, without a stack or established
cause. A separate sandboxed nextcloudctl diagnostic invocation also aborted.
All test processes have ended. Oliver is actively using Schnauz as his terminal
for this chat: current work is compile/sign/stage only, with no more native/UI
tests, helper launches, live app interaction, restarts or bundle replacement.
Keep the running bundle and all terminal/SSH sessions intact. No release requested.

Optimized 0.9.1 compilation, bundle allowlist, helper architectures and deep/strict
Apple Development signing checks passed. The safe installer deferred the update
to `.build-cache/pending-apps/3ab6c3edb5d533a6/Schnauz.app`, preserving the live
bundle. Apply with `./update.sh` only after Oliver deliberately quits. No new app
or helper was launched during the rebuild. Logs:
`.build-cache/nextcloud-boolean-build.log`, `.build-cache/nextcloud-boolean-tests.log`
and `.build-cache/cloud-text-ui-tests.log` (the latter records the spacing-probe
failure described above, not a passing final full-window check).

## Local 0.9.2 — outline cloud and stable status refresh

Oliver requested an outline cloud matching the pin's light native styling,
retaining Schnauz blue #3478c5. Use Apple's `cloud` SF Symbol instead of
`cloud.fill`; right-aligned Name-column placement and other status symbols stay.

The reported pin-to-cloud flicker came from the five-second provider cache expiry:
`nextcloud_local::read` replaced the whole entry with an empty pending entry and
returned None until the helper replied, so rendering fell back to generic folder
availability. Refresh now preserves the confirmed snapshot in place. Display
retains it during background revalidation for at most 30 seconds; action/menu
eligibility still requires metadata newer than five seconds, and provider writes
still perform their own fresh validation. A completed failed refresh clears the
snapshot and shows Unknown rather than misrepresenting provider policy as a cloud.
Refreshing an existing entry no longer evicts another cached row at capacity.

Two pure cache regressions cover display- and menu-initiated refresh, retained
pin status, request deduplication, strict action freshness and bounded display
grace. They were added but not run: Oliver is using Schnauz for this conversation,
and current verification remains compile/sign/stage only. No native/UI test,
helper invocation or live app interaction is authorized during this rebuild.

Optimized 0.9.2 compiled successfully and passed the bundle allowlist,
architecture and deep/strict certificate signature checks. The update is staged
for `./update.sh` after a deliberate quit; the running bundle was preserved.
Build log: `.build-cache/cloud-status-refresh-build.log`. No app/helper or test
binary was launched as part of this change; live visual confirmation is pending.

## Authorized 0.9.2 publication — 2026-10-25

Oliver approved publishing the current build on GitHub, superseding the paused
0.9.0 release preparation. Use the existing public repository, both signed DMGs,
exact MPL source archive and checksums, as an unnotarized Apple Development early
prerelease. Release notes cover changes since 0.8.9 and outstanding limitations.
He is actively using Schnauz: no native/UI suite or release-binary/helper smoke
launches during publication. Verify package contents, architectures, signatures,
mounted installers, exact source and asset digests without launching apps. Preserve
the installed bundle in place and compare fingerprints before/after.

Current command-line Rust checks passed all 125 application tests, including
both cache-refresh regressions. The first sandboxed attempt failed two existing
Quick Action fixtures; the outside-sandbox rerun passed. Existing native/UI results
remain historical, and the final text-layout assertion has not been rerun. Issues
#1/#2 remain open; no claim of verified live Nextcloud pinning or resolved 0.9.0
SIGSEGV crashes is authorized. Website/video production and test.csv stay local.

## Local 0.9.3 — progressive Disk Usage, test before publication

Before publishing, Oliver challenged the arbitrary 500,000-entry scan stop and
proposed a fast top-level overview, larger-branch priority and click-to-deepen.
He wants to try this approach and refine it from feedback. Both 0.9.2 DMGs were
built but no tag/source push/GitHub release was published; keep that candidate
unpublished while he tests the new scanner. Its reviewed source commits are local.

Disk Usage now publishes an early tree and progressive updates at roughly 50,000
entries or timed intervals. Every first-level branch gets a short initial pass,
then a heap favours larger known branches, with periodic fair exploration so a
deep unknown branch is not starved. A clicked folder takes priority. Directory
sizes are counted metadata, never invented recursive values; the map labels
provisional totals and pending folders until completion. Work runs on a dedicated
thread with cancellable short slices and a bounded snapshot channel, leaving
GPUI's shared file-work pool available for browsing and previews.

There is no entry-count stop. An independent 50,000-node detail budget groups
extra files while continuing to count all readable bytes/files. Larger files
discovered later replace smaller file detail, and grouped tiles remain explicit
with truthful area/totals. Clicking a grouped area cancels/rebases the scan on its
containing folder for finer detail. Unreadable folders and excluded volumes are
reported explicitly. Symlinks are not followed; boot/Data-volume handling remains.
Oliver also removed the duplicate count beside Disk Usage in the Show/filter row;
the existing bottom status bar remains the sole item-count display.

Seven focused Rust tests passed using disposable folders, including early-map
delivery, click-priority over sibling order, complete totals after grouping,
retaining late large files, symlink safety, cancellation and treemap geometry.
No personal drive scan, UI test, app/helper launch or live-session restart ran.
Build/stage 0.9.3 for Oliver's manual feedback before resuming publication.

The same test stand aligns only top-row controls to Terminal/Preview/Todoist's
26 pt height: Search, scope/mode segment buttons and Split. Search no longer uses
30 pt and segments no longer derive their height from text plus vertical padding.
The Schnauz name, version and little dog remain unchanged, as Oliver requested.
Existing hidden layout checks now assert all these control bounds, but no native
UI checks were launched in his live session. Dialog button heights stay separate.

Final 0.9.3 build completed with the scanner, removed duplicate count and aligned
controls together. Optimized compilation, bundle allowlist, helper architectures
and deep/strict Apple Development signature verification passed. The safe
installer preserved the live app and staged the update for `./update.sh` after
Oliver deliberately quits. No UI/native tests or app/helper launches ran.
Final build log: `.build-cache/0.9.3-complete-test-build.log`; scanner regressions:
`.build-cache/0.9.3-disk-usage-tests.log`. Await Oliver's manual test before GitHub
publication; the local 0.9.3 work remains uncommitted, and all 0.9.2 release
artifacts/source commits remain unpublished.

## Local 0.9.4 — right-click row colours

Oliver reported lingering row colour after right-clicking files and unstable
folder colours. GPUI 0.2.2's generic active style latches on any mouse button;
native AppKit menu tracking can consume the release that clears it. List rows
and Icons cells now derive their background only from actual selection, the
current context-menu target and pointer hover, without the generic active style.
Hover no longer overrides a selected item's colour. Search results use the same
selection/hover priority and temporary context-target colour. Right-click still
targets its menu without changing the real selection or file operations.

Verification remains compile/sign/stage only under the existing live-session
restriction; no native/UI test or app/helper launch. Manual visual confirmation
of repeated file/folder right-clicks and menu dismissal remains pending.
Build log: `.build-cache/0.9.4-context-highlight-build.log`.

Optimized 0.9.4 compilation, bundle allowlist, architecture and deep/strict
certificate signature checks passed. The first packaging attempt was stopped by
the sandbox blocking `/bin/ps`; the approved packaging rerun completed its safe
process check and staged the update for `./update.sh` after a deliberate quit.
The running app bundle and all live sessions were preserved. No publication.

## Local 0.9.5 — clearer file tabs and softer project bar

Oliver's annotated screenshot requested an uninterrupted active tab, grayer
inactive tabs and half-strength blue behind the project chips. The tab baseline
is now a separate layer painted before the tabs, so the active background can
cover it; the parent border previously remained visible over that background.
Inactive tabs have a neutral shade (darker in light mode, lighter in dark mode),
with a lighter hover shade. The project-row background uses 50% Schnauz blue;
the selected project chip retains its solid blue and white outline.

Verification remains compile/sign/stage only under the live-session restriction.
No app/helper launch or native/UI test; visual confirmation awaits Oliver's
manual test. Build log: `.build-cache/0.9.5-tab-layout-build.log`.

Optimized compilation, bundle allowlist, architecture and deep/strict certificate
signature verification passed. Packaging initially stopped because the sandbox
blocked `/bin/ps`; the approved rerun completed the safe process check and staged
0.9.5 without replacing the running bundle. Apply with `./update.sh` after a
deliberate quit. Packaging log: `.build-cache/0.9.5-tab-layout-package.log`.
No publication or live-session restart.

## Local 0.9.6 — stable search-row background

The top Search row now always uses its normal theme background and border,
independent of All/Folders/Files and project file-ending filters. The file filter
row retains its active tint; the project row retains its constant soft blue.
Verification uses compile/sign/stage only, preserving all live app/terminal
sessions without native/UI tests or app/helper launches.
Build log: `.build-cache/0.9.6-search-row-build.log`.

Optimized compilation, bundle allowlist, architecture and deep/strict certificate
signature verification passed. Packaging required an approved rerun because the
sandbox blocked the safe installer's `/bin/ps` check. The signed 0.9.6 update is
staged; apply with `./update.sh` after a deliberate quit. The running app bundle
and all sessions were preserved. No publication.
Packaging log: `.build-cache/0.9.6-search-row-package.log`.

## Local 0.9.7 — footer exclusion counts

The footer explains omitted folder items, for example “38 of 52 items · 4 hidden
· 10 filtered”. Hidden counts cover dot names and macOS hidden flags while the
eye toggle is off. Other exclusions (type, file endings and in-folder text/content
filters) count as filtered; hidden items take precedence so nothing is counted
twice. Zero categories are omitted. Recursive search retains its result count,
since it has no complete folder-listing total. Selection counts remain visible.

Hidden totals are cached when listing/filter results are recomputed, with no
filesystem reads or listing scans during footer rendering. The single-line label
truncates in narrow windows and exposes its full text in a tooltip.
Verification remains compile/sign/stage only; live app/terminal sessions are
preserved, with no native/UI tests or app/helper launches.
Build log: `.build-cache/0.9.7-footer-counts-build.log`.

Optimized compilation, bundle allowlist, helper architectures and deep/strict
certificate signature verification passed. Packaging required an approved rerun
for the safe installer's sandbox-blocked `/bin/ps` check. Signed 0.9.7, including
the 0.9.6 Search-row fix, is staged for `./update.sh` after a deliberate quit.
The running bundle and live sessions remain intact. No publication.
Packaging log: `.build-cache/0.9.7-footer-counts-package.log`.

## Local 0.9.8 — hidden sidebar and compact path row

Hiding the sidebar now removes its former 52 pt strip entirely. A small 24 pt
sidebar-icon overlay at the left of the blue project row reopens it, below native
window controls and clear of the file tabs/path actions. Symmetric project-row
insets keep project chips centered and scrollable without covering the first
chip. The existing sidebar toggle shortcut and expanded collapse control remain.
Hidden sidebars report zero width for pane measurements and discard stale
sidebar drop geometry; no hidden section content is constructed.

The folder/path-and-actions row has 4 pt vertical padding instead of 8 pt, reducing
its height by 8 pt while preserving icons, text, action hit areas and path editing.
0.9.8 also includes the 0.9.6 header and 0.9.7 footer changes. Verification remains
compile/sign/stage only, preserving all live app/terminal sessions without
native/UI tests or app/helper launches.
Build log: `.build-cache/0.9.8-compact-sidebar-path-build.log`.

Oliver also requested a tighter Show/Filter row in the same change. Its height
is 30 pt (previously 36); All/Folders/Files and On/Off segments are 20 pt high
(previously 24), with 8 pt horizontal padding (previously 12) and 1 pt group
padding (previously 2). The row/group gaps and Modify, Disk Usage and SSH Reload
controls are compacted consistently. Text/icon sizes and action routing remain.

The final combined optimized 0.9.8 build, bundle allowlist, helper architecture
and deep/strict certificate signature checks passed. The first packaging attempt
hit the sandbox's `/bin/ps` restriction; the approved final build safely staged
the combined changes without replacing the running bundle. Apply with
`./update.sh` after a deliberate quit. No publication or live-session restart.

## Local 0.9.9 — reorder top project chips

Named projects in the top bar now drag before or after another project chip,
with a vertical insertion marker that does not change chip geometry. Personal
remains pinned first, matching the sidebar. Reordering uses the existing typed
project gesture and atomic projects.json save, so the top bar and sidebar share
one order across all windows and future launches, including Keep Setup & Quit
restoration. No separate volatile startup ordering can override it.

Drops validate current live project IDs and use metadata persistence only,
without switching projects or replacing live tabs/PTYs. Existing project-order
serialization regressions cover the reused persistence path. Under the current
live-session restriction, verification is compile/sign/stage only, with no
native/UI test or app/helper launch; actual drag feedback awaits manual QA.
Build log: `.build-cache/0.9.9-project-chip-order-build.log`.

Optimized compilation, bundle allowlist, helper architectures and deep/strict
certificate signature verification passed. After the sandbox blocked the safe
installer's `/bin/ps` check, an approved packaging rerun staged signed 0.9.9
without replacing the running bundle. Apply with `./update.sh` after a deliberate
quit. No publication or live-session restart.
Packaging log: `.build-cache/0.9.9-project-chip-order-package.log`.

## Local 0.9.10 — blue active-project outline

The active top project chip now uses CHROME_HIGHLIGHT blue for its outline,
matching its blue background instead of the former white border. Its white label
and existing darker blue hover remain. This supersedes the earlier white-outline
direction. Verification remains compile/sign/stage only; no native/UI tests,
app/helper launches or live-session changes.
Build log: `.build-cache/0.9.10-project-outline-build.log`.

Optimized compilation, bundle allowlist, helper architectures and deep/strict
certificate signature verification passed. The approved packaging rerun resolved
the sandbox-blocked `/bin/ps` process check and staged signed 0.9.10. Apply with
`./update.sh` after a deliberate quit; the live app bundle and sessions were
preserved. No publication.
Packaging log: `.build-cache/0.9.10-project-outline-package.log`.

## Local 0.9.11 — sidebar reference drops and project search history

Oliver reported that dragging files/folders onto Common or Favourites had stopped
working. A root capture-phase release handler now routes native file drops by
the sidebar/Common bounds before nested row handlers can consume them. It uses
only the current native drag's captured paths and retires that drag so a drop
cannot also perform a file transfer. Existing background metadata validation
classifies local files/folders; remote drags use their known typed references
and now finish in the capture path too. Hidden/outside sidebar drops are excluded.

File/folder context menus (including search results and SSH items) now offer
Add to Project (choose a named project or Personal) and Add to Common. They capture
typed paths and project IDs when opened, add references without switching project
or transferring files, and reuse existing deduplication and atomic persistence.

The search bar has a recent-search icon/menu with up to ten most-recent searches
per project, including Personal. History is stored in each project's central
sidebar metadata, defaults empty for existing profiles, deduplicates query plus
File/Content and scope, and reloads with those search options. Settled queries
save after 1.2 seconds without edits; closing/submitting/stopping a search saves
immediately. Captured project identity prevents leakage across project switches.
Clearing history affects only that project and suppresses re-adding the current
query until a new search starts. No legacy shared palette history is imported.

Verification remains compile/sign/stage only under the live-session restriction;
no native/UI tests or app/helper launches. Live drag behavior awaits Oliver's
manual confirmation. Build log: `.build-cache/0.9.11-sidebar-reference-build.log`.

The final combined optimized build, bundle allowlist, helper architectures and
deep/strict Apple Development signature checks passed. The initial drop-only
packaging attempt hit the sandbox's `/bin/ps` restriction; the approved combined
build safely staged signed 0.9.11, preserving the running bundle and all sessions.
Apply with `./update.sh` after a deliberate quit. No publication.

## Local 0.9.12 — direct SMB deletion and visible file-operation errors

Oliver reported a dismissed Trash dialog leaving an SMB file untouched with no
error. The old helper discarded NSFileManager's NSError and the caller silently
filtered out failures. Trash now returns the native error; failed and partial
operations report it in the fixed footer, including completed/failed counts.

Oliver explicitly chose direct permanent deletion on SMB/network drives: skip
Trash entirely and label the confirmation “Delete Permanently”, with text saying
that it cannot be undone. This supersedes an initially considered Trash/fallback
flow. The action checks the selected item's containing filesystem off-thread,
so Finder-mounted shares work too; it does not require a saved Schnauz connection.
Local drives retain actual macOS Trash and undo for returned Trash destinations.
Context-menu deletion goes through the same confirmation and routing.

Network targets capture file identity before confirmation and reject changed
items. Symlinks delete only the link; folder deletion preserves symlink targets.
Preparation and mutation use dedicated workers with progress/cancellation between
items, without blocking the UI/shared file-work pool. Project switching cancels
pending prompts/preparation. Successful operations refresh only relevant local
panes/searches, without refreshing unrelated SSH sessions. Native previews hide
while either delete confirmation is open.

Disposable-fixture Rust regressions cover permanent file/folder/link deletion,
replacement rejection and partial-error messages. Current verification remains
compile/sign/stage only: do not run native/UI tests, app/helpers or live SMB
operations. Actual server deletion and confirmation layout await manual QA.
Build log: `.build-cache/0.9.12-smb-trash-build.log`.

Optimized compilation, `cargo test --locked --no-run --bin schnauz`, bundle
allowlist, helper architectures and deep/strict Apple Development signatures
passed. Tests were compiled, not executed. The sandbox initially blocked the
safe installer's `/bin/ps` check; the approved packaging rerun staged signed
0.9.12 while preserving the running bundle and every live session. Apply with
`./update.sh` after a deliberate quit. No publication.
Logs: `.build-cache/0.9.12-delete-test-compile.log`,
`.build-cache/0.9.12-smb-trash-package.log`.

## Local 0.9.13 — right-click selects its target

Oliver reported menus visually targeting a newly right-clicked item while actions
still used the earlier left-click selection. File/folder right-click now selects
an item outside the current selection before building any menu. Right-clicking
inside a selected group retains that group while setting the clicked item as
focus/anchor. Background right-click leaves selection alone.

The shared selection path covers List, Icons, SSH, direct Quick Actions and search
results. It cancels pending rename/drag gestures and updates preview focus through
the existing selection code. Search-result Add to Project/Common also follows
the selected group. Hidden fixture assertions cover selecting a different file,
preserving a group, changing its anchor and selecting a different folder.

Verification remains compile/sign/stage only under the live-session restriction:
no native/UI tests or app/helper launches. Live menu behavior awaits manual QA.
Build log: `.build-cache/0.9.13-context-selection-build.log`.

Optimized compilation, `cargo test --locked --no-run --bin schnauz --features
ui-checks`, bundle allowlist, helper architectures and deep/strict Apple
Development signatures passed. The regression checks were compiled, not run.
The sandbox's `/bin/ps` restriction required an approved packaging rerun, which
safely staged signed 0.9.13 without replacing the running bundle or sessions.
Apply with `./update.sh` after a deliberate quit. No publication.
Logs: `.build-cache/0.9.13-context-selection-test-compile.log`,
`.build-cache/0.9.13-context-selection-package.log`.


## Local 0.9.14 — terminal protocol replies without painting

Oliver reports that typed lines still overwrite one another in Codex over SSH.
The earlier local grid/PTY sizing changes remain. A further concrete protocol
problem was found: cursor/device, colour and text-area-size replies were drained
only while rendering. Hidden tabs therefore withheld replies, and visible tabs
waited for a frame. Output processing now sends protocol replies immediately,
also after synchronized-redraw expiry or resize flushes. Window/UI callbacks
remain deferred to rendering, and text-selection snapshots do not affect replies
from the live grid. No shell input, SSH commands or reconnects are introduced.

Added unattached-terminal fixture assertions for fragmented cursor probes, long
wrapped input, size replies, resize and bottom-row scrolling at three widths,
including frozen displayed text. Verification is compile/sign/stage only under
the existing live-session restriction; fixture assertions are compiled, not run.
This addresses a protocol gap, but the specific Codex-over-SSH overwrite is not
yet reproduced or confirmed fixed. Oliver's fresh-session versus post-resize
clarification and live workflow confirmation remain pending. No publication.

Optimized compilation and `cargo test --locked --no-run --bin schnauz --features
ui-checks` passed. No test executable, app or native helper was launched.
Bundle allowlist and deep/strict Apple Development signature verification passed.
The initial safe-install check was sandbox-blocked at `/bin/ps`; approved
packaging safely staged signed 0.9.14 without replacing the running bundle.
Apply with `./update.sh` after a deliberate quit. Live SSH/Codex sessions remain
untouched. Logs: `.build-cache/0.9.14-terminal-protocol-test-compile.log`,
`.build-cache/0.9.14-terminal-protocol-build.log`,
`.build-cache/0.9.14-terminal-protocol-package.log`.


## Local 0.9.15 — Current Project shortcut in reference menus

File/folder Add to Project submenus now start with Current Project, followed by
a separator and the existing Personal/named-project destinations. Tab Add to
Favourites also starts with the same action, above its existing destinations.
It adds the captured file/folder references to the project active when the menu
opened, including Personal, using existing typed/deduplicated favourites storage.
Remote server/network identity and the selected tab's folder are preserved.
The destination cannot change if another window switches projects while the
menu is open. No project switch, file transfer or terminal change is involved.
Verification remains compile/sign/stage only under the live-session restriction.

Optimized build, bundle allowlist and deep/strict Apple Development signature
verification passed. The sandbox initially blocked `/bin/ps`; approved packaging
staged signed 0.9.15, preserving the running bundle and every live session.
No native/UI tests, app or helper launches. Apply with `./update.sh` after a
deliberate quit. Includes the 0.9.14 terminal protocol change. No publication.
Logs: `.build-cache/0.9.15-current-project-build.log`,
`.build-cache/0.9.15-current-project-package.log`.


## Local 0.9.16 — duplicate downloads retain file extensions

Oliver reported server files opening under names such as x.pdf 2 when x.pdf
already exists in Downloads, breaking the automatic application association.
The server-download path now reuses the existing copy destination naming helper:
x.pdf, x 2.pdf, x 3.pdf. Both download-and-open and explicit Download to Downloads
use it. The file extension and original download remain intact. Mounted SMB
files opened directly still use their existing native open route.
Verification remains compile/sign/stage only under the live-session restriction;
no live server/download, app/helper launch or native/UI test is run.

Optimized compilation, bundle allowlist and deep/strict Apple Development
signature verification passed. The initial installer check was blocked by the
sandbox at `/bin/ps`; approved packaging safely staged signed 0.9.16 without
replacing the running app or changing any live terminal/server session. Apply
with `./update.sh` after a deliberate quit. No tests/app/helpers were launched.
Logs: `.build-cache/0.9.16-download-extension-build.log`,
`.build-cache/0.9.16-download-extension-package.log`.

## Local 0.9.17 — project strip matches file selection

The top project-row background now uses the theme's selected-file colour
(`Theme.selected`) instead of translucent Schnauz blue, following light/dark
and custom palettes. Active project chips retain their existing blue treatment.
Verification remains compile/sign/stage only, preserving the running bundle
and all live sessions. No native/UI tests or app/helper launches.
Build log: `.build-cache/0.9.17-project-background-build.log`.

Optimized compilation, bundle allowlist and deep/strict Apple Development
signature verification passed. An approved packaging rerun resolved the
sandbox-blocked `/bin/ps` safety check and staged 0.9.17, preserving the live app.
Packaging log: `.build-cache/0.9.17-project-background-package.log`.

## Local 0.9.18 — simpler Customization settings

Customization now starts with two illustrated Light/Dark cards, the current
palette name and a live sample of sidebar, project-strip, normal, selected and
hovered file rows. The other 28 built-in palettes remain available behind
More themes. Existing saved themes load unchanged; no user settings are migrated
or overwritten simply by opening Settings.

The former three always-visible 85-swatch grids are replaced by optional compact
Background, Text, Selection and Hover rows, with only one picker expanded at a
time. Each has a small curated palette, six-digit custom hex entry with paste,
explicit Apply/Enter, Escape cancellation, inline validation and Reset color.
Background derives matching surfaces/dividers/hover/selection, restoring readable
foregrounds on a light/dark crossing; Text updates secondary labels too.
Selection controls both selected files and the top project strip (0.9.17).
Active-project and toolbar blue, native macOS icons and native menu styling
retain their existing behavior; no misleading global accent/menu controls added.
Reset to Schnauz Light resets only the theme, without the former unrelated
legacy menu-style write. The unused menu-style apply function and generated
255-swatch UI/palette code are removed; saved legacy menu metadata is preserved.

Verification remains compile/sign/stage only under the live-session restriction.
Pure regression assertions cover exact hex parsing, grouped field updates,
mode changes and per-color resets; compile them without execution. Visual
confirmation awaits Oliver's manual test; no UI/native test or app/helper launch.
Logs: `.build-cache/0.9.18-customization-build.log`,
`.build-cache/0.9.18-customization-test-compile.log`.

Optimized compilation and `cargo test --locked --no-run --bin schnauz --features
ui-checks` passed. Tests were compiled, not executed. Bundle allowlist, helper
architectures and deep/strict Apple Development signatures passed. Approved
packaging safely staged the combined 0.9.18 update without replacing the running
bundle or touching any live session. Apply with `./update.sh` after a deliberate
quit; no publication. Packaging log:
`.build-cache/0.9.18-customization-package.log`.

## Local 0.9.19 — approved Shuffle cleanup review

Oliver approved implementing the ten findings from the source review. This
stand includes the 0.9.18 Customization redesign and selected-file-colour project
strip, with the following safety, responsiveness and maintenance changes.

File copy/move uses `src/file_transfer.rs`. Same-volume moves and publication
of completed copies use macOS exclusive rename. Copies are assembled inside a
0700, uniquely reserved directory at the destination; failure/cancellation only
cleans that owned stage. A competing destination is preserved. Cross-volume
moves check the source's identity before removing it, and report a saved copy
whose original could not be removed. Preparation/size scans check cancellation,
and a failed source does not prevent other eligible items in the batch. Only
affected local panes/searches refresh on completion, preserving unrelated SSH
tabs and project switches. Exclusive-rename support on mounted filesystems
remains a manual QA item; unsupported filesystems fail safely.

Local folder constructors now allocate state only. Names and metadata listing
passes both run on dedicated workers, with loading/error UI, cancellation and
captured tab IDs/generations, including parked tabs. Terminal cwd updates on
parked/inactive local tabs defer their listings until visible. Ordinary folder
navigation no longer validates a directory synchronously before requesting its
listing. `src/workers.rs` also isolates SFTP, preview, archive, script, cloud and
Get Info waits from GPUI's shared pool. SFTP batch requests have cancellation
and a 45-second command deadline; SSH command/control checks have bounded waits.
Only the newly spawned child is killed, never an existing master or terminal.

Session and last-directory saves now queue atomic writes on workers.
`src/persistence.rs` orders/coalesces snapshots and explicit Library.save
transactions by generation, so stale autosaves cannot overwrite newer metadata.
Quit flushes pending writes. Existing projects/session formats are retained;
unchanged windows still skip saves. Explicit metadata transactions retain error
reporting and synchronous commit semantics.

Remote image/PDF keys contain the complete nonsecret host/user/port/alias/path
identity and cannot collide with real local paths. Downloads use unique 0700
temporary folders, a 40 MiB cap checked during transfer, bounded retained source
storage, captured servers and cancellation/request checks. Every SFTP path uses
the literal argument encoder, including create/rename. Local text/native preview
and Information content cannot leak through an identical remote pathname.
PDF reads are capped at 64 MiB and serialized; cancelled/unneeded page requests
are retired and discarded. Downloaded temporary files are removed off the UI
thread when their last retained owner is released. Get Info reads and recursive
totals also run on workers, stop on close/replacement and display scan errors.

Removed the unused floating palette/action panel and its mode flag, global
palette-history reader/writer, menu-style state/observers/persistence and opacity
preference. AppKit menus retain their actual native styling. The search menu,
keyboard recall, clear action and history switch use the same per-project list;
recall preserves File/Content and scope. Old shortcut IDs/preferences remain
readable; the old shared history file is untouched and not imported. Retired the
optional Waterfall tree, removing its unlimited prefetch/cache and idle polling.
User-created sidebar groups, favourites, tabs and compatibility files remain.

`native/sftp/` adds a signed `sftpctl` helper using Security/LocalAuthentication.
Credentials travel through private stdin/stdout; native writes report failure
and preserve the form. Native reads are noninteractive; the existing CLI client's
read-only path stays in legacy.rs to preserve access to older items with its
access policy. Neither old service is deleted or rewritten for migration.
The owned expect authentication helper now receives a UTF-8 hex line on stdin,
with no password in argv/environment or the script file. Stale/double form
submissions cannot apply credentials metadata to a different form/project.
Removed the live diagnostic flags that accepted passwords or executed scripts;
remaining diagnostics are excluded from normal builds, and unknown options exit
before app startup. Build/install allowlists include the signed new helper.

Removed unused direct terminal dependencies anyhow/ropey; ropey and str_indices
leave the locked graph. The image-codec dependency tree stays because GPUI also
requests it. Regenerated notices for 495 dependencies and replaced their stale
unpublished-candidate MPL download reference with the published releases index.
Original source attribution and licences remain intact. No commit/publication.

Verification remains compile/sign/stage only while Oliver uses Schnauz for this
chat. Rust regression fixtures cover competing destinations, owned stage cleanup,
cancelled moves, remote-source identities, ordered saves and owned-child timeout
scope. Swift credential tests use an injected fake store and exclude Security;
test.sh includes them for a later permitted run. Tests are compiled, not executed.
Manual filesystem/SMB, SSH/Keychain and UI workflow confirmation remains pending;
no measured runtime speedup is claimed. Logs:
`.build-cache/0.9.19-cleanup-test-compile.log`,
`.build-cache/0.9.19-cleanup-build.log`,
`.build-cache/0.9.19-sftp-helper-compile.log`,
`.build-cache/0.9.19-sftp-fixtures-compile.log`.

Final optimized compilation and the ui-checks-enabled Rust test compilation
passed. Native SFTP helper and fake-store fixture binaries compiled without
execution. Notices generation and whitespace checks passed. Approved packaging
passed the strict bundle allowlist, every executable's architecture check and
deep/strict Apple Development signature verification, then safely deferred the
update while Schnauz was running. The pending bundle was independently checked:
version 0.9.19, local.schnauz.app identity, new helper present, and its main build
UUID and all 16 Mach-O sections match the compiled release binary (code-signing
metadata excluded). The pending signature was verified again after staging.
Packaging log: `.build-cache/0.9.19-cleanup-package.log`.

The combined update is at the normal pending-app location. Apply with
`./update.sh` only after a deliberate quit. The installed/running bundle and
every live session remain untouched. No application, helper or test binary was
launched; no real account/Keychain credential, SMB share or server was used for
QA. No commit, tag, release or other publication.

## Local 0.9.20 — Eject in Devices

Oliver reported that external drives could not be ejected from the sidebar.
Devices rows now have an AppKit right-click menu with Eject “drive name” for
eligible external local volumes. Internal/startup/unknown/nonlocal volumes
cannot invoke it; duplicate in-flight requests are disabled across windows.
Opening or rendering the menu performs no filesystem query or eject operation.

The existing volumectl helper now reports eligibility and a captured mount
identity (persistent UUID when available plus kernel filesystem ID, including
filesystems without UUIDs). On the explicit click it obtains a fresh mounted
snapshot and validates path/identity/eligibility before calling the normal
NSWorkspace unmountAndEjectDevice API. There is no force, diskutil fallback,
process termination or network-share unmount. Native refusal/busy errors and
progress/completion use the fixed footer.

Metadata and eject operations run on owned workers. All windows refresh their
Devices cache with generation checks; the originating window may close without
stranding the shared busy state. Related native media previews are released and
suppressed while ejecting, with pending preview work cancelled. Successful eject
clears stale local listings/selections and shows a reconnect/Retry error while
retaining paths and every live terminal/PTY. Failed eject retains the mounted
drive and allows retry. The capacity badge's left-click toggle remains intact.

Native logic lives in native/volumes/Core.swift with injected fake eject calls
for startup/internal/network/identity changes, missing mounts, busy refusal and
success fixtures. test.sh compiles/runs those only when execution is permitted.
The current workflow compiles them without execution, along with the real helper
and Rust/ui-checks fixtures. No drive has been queried, unmounted or ejected for
QA, and no application/helper/test binary has been launched. Actual hardware/UI
confirmation remains Oliver's manual test. No commit or publication.

Logs: `.build-cache/0.9.20-eject-test-compile.log`,
`.build-cache/0.9.20-eject-build.log`,
`.build-cache/0.9.20-volume-helper-compile.log`,
`.build-cache/0.9.20-volume-fixtures-compile.log`.

Optimized 0.9.20 compilation, Rust/ui-checks test compilation and real/native
fixture helper compilation passed; no binaries were executed. Bundle allowlist,
all helper architectures and deep/strict Apple Development signatures passed.
Approved packaging safely staged the update while preserving the running app
bundle and every live session. Pending bundle version/identity/helper presence
and its deep/strict signature were checked after staging. Apply with
`./update.sh` after a deliberate quit. No real-drive eject or UI test was run.
Packaging log: `.build-cache/0.9.20-eject-package.log`.


## Authorized 0.9.21 publication — 2026-10-30

Oliver confirmed Schnauz is working well and requested a new GitHub release.
Publish the current version with changes since public 0.8.9, both architecture
DMGs, exact MPL sources and checksums using release.sh and docs/RELEASING.md.
Earlier candidate holds are superseded; preserve all existing tags/assets and
exclude local website/video work and test.csv from committed source.
Validation remains compile/sign/package only: no test binaries, applications or
native helpers are executed. Runtime/UI, real-drive eject, live provider actions
and physical Intel checks are not newly verified. Issues #1/#2 remain open and
the specific Codex-over-SSH redraw symptom is not claimed resolved. Preserve the
installed live bundle and all windows, terminals, SSH sessions and settings.


## Local 0.9.21 — immediate Devices refresh, Info and Eject controls

During the release Oliver reported delayed detection of newly attached drives,
requested Info and Eject on right-click, and an Eject symbol beside eligible
drives. He explicitly requested continuing publication with these changes.

The old 30-second poll was the only mount-detection trigger. Devices now also
subscribes once to NSWorkspace's own mount/unmount/rename/wake notifications.
Callbacks only wake window subscribers; owned workers refresh the cached list.
Requests are coalesced, with at most one in-flight listing and one follow-up,
while the 30-second timer remains for capacity updates. No filesystem work runs
inside notification callbacks or rendering.

Every drive menu offers Info. Eligible drives additionally show Eject and a small
native eject symbol beside the name; other drives show neither and have no
cannot-eject placeholder. Busy eligible drives disable duplicate ejection.
Info displays cached capacity, available/used space and the mount path through
the existing modal; it never recursively scans a drive or reads its folders.
The eject button consumes its click so it cannot navigate or change terminals.
Existing native mount-identity revalidation and normal, non-forced eject remain.

Compile-only fixtures cover delivery to multiple window subscribers, cleanup of
closed subscriptions and Info for a nonexistent path using cached capacity.
Physical-drive latency/UI/eject checks remain manual; no app, helper or test
binary is launched under the current live-session restriction.


## Local 0.9.22 — selected Illustrator preview flicker; release paused

Oliver paused publication before any push/tag/upload and reported a selected
.ai preview repeatedly appearing and disappearing. The 0.9.21 source commit
193ed56 and both installers remain local unpublished preparation. Do not publish
them while this reported preview problem is being investigated/tested.

Source tracing found that the shared vnode watcher treated every NOTE_ATTRIB
notification on a selected file as a content change. Quick Look can update
access metadata/extended attributes while generating its thumbnail; invalidating
that thumbnail then runs the generator again. The watcher now suppresses only
attribute-only events on an unchanged regular file, comparing device/inode,
length and modification time. Access/change time and xattrs do not cause preview
invalidation. Actual writes (even equal-length writes with restored timestamps),
removal/rename/revoke, changed file identity/mtime/length and all directory
attribute events still refresh normally. No personal .ai file was read and the
specific live event sequence has not been measured or claimed reproduced.

Rust/ui-checks regression fixtures compile without execution under the current
live-session restriction. They cover unchanged-file attributes versus real writes,
combined write/attribute events, replacement/removal and directory attributes.
Prepare the normal signed 0.9.22 local build for Oliver's manual preview check;
keep the running bundle and all terminal/SSH sessions intact. Publication remains
paused pending his feedback; no further commit/tag/push/release is implied.


0.9.22 verification: optimized compilation, Rust/ui-checks test compilation,
bundle allowlist and deep/strict Apple Development signatures passed. The normal
update is staged for `./update.sh` after a deliberate quit. All 19 recorded files
in the installed live bundle retain their original inode/device/hash. No app,
helper or test executable was launched; no live .ai file was opened for QA.
Logs: `.build-cache/0.9.22-preview-{test-compile,build}.log`; receipt:
`.build-cache/0.9.22-preview-verification.json`. Await Oliver's manual .ai check;
publication remains paused, with no push/tag/upload or additional source commit.


## Authorized 0.9.22 publication — resumed

Oliver explicitly requested releasing the new version after the preview fix.
This supersedes the hold above. Publish 0.9.22 with both architectures, current
source, exact MPL sources and checksums, retaining the early-prerelease and
Apple Development/not-notarized disclosures. Fresh validation remains compilation,
static signatures/architectures, package/source/checksums only; no app/helper/test
binary launches or live account/hardware operations. Preserve the running app,
all terminals and SSH, and all existing release tags/assets. Exclude website/,
video/ and test.csv. Do not claim issues #1/#2 or the specific SSH/Codex redraw
symptom resolved. The normal signed local 0.9.22 build is already safely staged.
