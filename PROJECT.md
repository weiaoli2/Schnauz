# Schnauz

Last updated: 2026-10-20

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
