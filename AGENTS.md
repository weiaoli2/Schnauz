# Schnauz

Read [PROJECT.md](PROJECT.md) first for the working location, build commands,
current stage and the Todoist integration. This file records detailed working rules.

Independent personal macOS app based on WizenPainter/shuffle, starting at
50a94466dadef2c483913bc2cd5c76999cda4dda. The local branch is `schnauz`;
`schnauz-starting-point` preserves the unmodified source. `upstream` is reference only.

- Rust + GPUI. Keep the native renderer and Mac integrations.
- `src/theme.rs`: themes; `src/schnauz.rs`: custom chrome and project UI;
  `src/projects.rs`: typed persistence; `src/main.rs`: existing file manager.
- Retain the original MIT LICENSE and attribution.
- Schnauz credits: developer, design and product direction Oliver Weber;
  implementation assistance Codex; original Shuffle code Jaime Guzman.
  Keep `CREDITS.txt`, the source header and README consistent, and ship the
  credits alongside the unchanged upstream and third-party license notices.
- About Schnauz (app menu and idle mascot click) credits Oliver and offers optional “Coffee for Cookie”
  support for his schnauzer: US$2.50 via PayPal to o@freedom-net.de. Open PayPal
  only on the user's click; keep the original Shuffle and third-party credits.
- App identity: Schnauz, `local.schnauz.app`. Never enable the
  original Shuffle self-updater or publish to its GitHub repository.
- A possible €5 paid edition was discussed; the MIT base does not require opening
  our adaptations. Preserve third-party obligations, including MPL-2.0 source
  access for `option-ext`; distribution notes are saved in the README.
- Current debugging workflow: Oliver tests the live app and reports individual
  bugs. Wait for his reports; do not start unsolicited checks or change his live
  browsing session. Run relevant verification when implementing a reported fix.
- For a normal build run `./build.sh`; for a faster development build use
  `SCHNAUZ_BUILD_PROFILE=debug ./build.sh`. Rust stable and Xcode are required.
- Run `./test.sh` after project or file-operation changes. Use disposable
  folders for copy, move, rename and delete verification.
- Native UI QA can set `SCHNAUZ_CONFIG_DIR` and `SCHNAUZ_START_DIR`
  to disposable paths. Do not change HOME.
- `dev-env.sh` reuses the prepared cache under `.build-cache/` and builds offline.
  Use `CARGO_NET_OFFLINE=false` when a new dependency must be downloaded.
- Show a working first version, gather feedback, then iterate. Default visual
  direction: quiet Mac-style light/dark palettes, indigo accent, clear spacing.
- The first preview was launched with
  `/Users/oliw/Documents/Codex/2026-10-17/what/work/qa-profile`. The user began
  browsing in that window. Preserve its settings/workspaces if switching the
  running app to its normal profile; it may now contain user-created state.
- The live app now uses `~/Library/Application Support/Schnauz/`. Its projects
  and tabs were preserved from both preview profiles, including the user's two
  Desktop tabs and earlier Applications/Home tabs. Backups are in
  `/Users/oliw/Documents/Codex/2026-10-17/what/work/profile-backups/`.
  Treat preview profiles as user data too; never
  replace live state with QA fixtures or leave a testing view active for the user.
- User feedback: no automatic recursive folder-size calculations while browsing.
  Main text is 12 logical px/pt, small labels 11, file rows 26 px. Keep it compact.
- Tabs have visible outlines and rounded top corners. Folder names use the same
  primary text color as files; their icons distinguish them.
- In file panes, one click selects files/folders and double-click opens them.
  Clicking the name of the sole selected item again starts inline rename after
  the macOS double-click interval. Cancel pending rename at the next pointer/key
  interaction, scroll, navigation, or drag; never let it intercept double-click
  opening or modified selection. List, Icons and search results share the editor.
  Keep an active edit intact when its name is clicked. The special parent row
  remains a navigation shortcut and cannot be renamed.
- The collapsed sidebar contains only its expand button. Do not render section
  actions, plus buttons, shortcuts or placeholders until it is expanded.
  The former compact icon-rail layouts and helper options are removed; keep
  section and shortcut renderers dedicated to the expanded sidebar.
- Only List and Icons are supported. Migrate legacy Gallery/Columns tabs to List.
  Lists have configurable, reorderable, independently resizable columns and a
  horizontal scrollbar when needed. Keep header, row and folder-drop coordinates
  aligned even when Name is moved. Use native vector toolbar icons, not emoji.
- Folder views (mode, columns, order, widths, preview width and sorting) autosave centrally in
  `folder-views.json`. Load once, resolve ancestor rules in memory, and write
  atomically in the background after changes, never per resize frame. Applying a
  view to a folder tree replaces child overrides without scanning subfolders or
  creating files in them; subsequent individual-folder overrides remain possible.
- Hidden toggles and view changes preserve selected files that remain visible,
  including refreshed search results. Keep selection visible when changing views.
- Context, sort and sidebar menus use AppKit so they can extend beyond the app
  window and scroll at display edges. Never hold a GPUI borrow during menu tracking.
  Finder tag menu items carry real coloured NSImage dots (cached, Retina, non-template);
  preserve the dot field when adapting MenuNode to NSMenuItem.
- All/Folders/Files visibility is independent for each tab and persists in sessions
  and projects. Keep every view, selection and keyboard navigation consistent
  with the visible results; tint the top bar while a type filter is active.
- App bundles keep physical directory semantics for file operations, but display
  as applications with native icons cached per app path. Single-click selects,
  double-click launches, and Show Package Contents explicitly browses the bundle.
- Plain-text path paste opens a folder or reveals a file; text editors and the
  terminal retain ordinary paste behavior. Copied file objects still paste files.
- The breadcrumb ends at the current folder and clips overflowing ancestors on
  the left. Keep the current folder visible, including in narrow split panes.
- The terminal is a persistent native PTY with an Alacritty-backed component in
  `vendor/gpui-terminal`; old one-shot command mode is removed. Keep two-way cwd
  sync via OSC 7 and prompt hooks. Browser changes must be ignored while a foreground
  program runs, including Codex; never queue them to apply later. Preserve unfinished
  shell input and never execute paths as code.
- Unsynced is an explicit recovery button. Restart only at a ready shell prompt
  with the shell owning the PTY and no child jobs. Keep the panel open and file
  browser focused. Never restart automatically after a foreground app exits.

- Projects replace the old workspace UI. Each owns its tagged items,
  Locations, Cloud, saved servers/network folders and autosaved local layout.
  New project sidebars start empty; Personal has its own profile. Recents and
  automatic cloud-folder discovery are removed. Locations is the single shortcut
  list; Favorites and its controls/model are removed. Merge legacy project and
  Personal favorites into Locations on load, deduplicating without dropping files
  or unavailable paths. Preserve legacy files; new saves contain Locations only.
  Folders can be dropped onto Locations to add shortcuts to the active project.
  Show “Drop to add to favourites” over that section while hovering a file drag.
  Add folder references only, deduplicate, and never move/copy the dropped files.
  A collapsed sidebar still has only its expand button.
  The Tag icon adds/removes local
  selection from Tagged Items; clicking a tagged file reveals it. Sidebar folders
  offer View in… with All projects first, then checkable individual projects;
  update destinations without switching. All affects existing projects only.
  Central projects.json stores profiles; migrate legacy servers once to Personal,
  preserve original legacy files and shared Keychain credentials. Migrate legacy workspaces
  and bookmarks without modifying their original files. Keep live PTYs when switching.
- Devices is a global fixed section above the scrolling Projects/Locations area,
  independent of the selected project. Show mounted local startup/external drives
  with compact free-space labels. Use filesystem capacity, refreshed off-thread
  every 30 seconds; never calculate folder sizes. Default colours are green below
  80% used, yellow at 80–90%, red above 90%; thresholds are saved in Settings →
  General → Sidebar. A collapsed sidebar still contains only its reopen button.
- Cloud paths under macOS CloudStorage and iCloud Drive use a persistent white-on-blue
  rounded provider badge followed by the path relative to that cloud root. The
  badge stays visible when ancestor crumbs are clipped. Preserve the full actual
  path for editing, tooltips and file operations; never apply local cloud identity
  to remote SSH paths. Clicking the badge goes to that provider root.
- Cloud context menus identify the provider on files, folders, backgrounds and
  sidebar shortcuts. Downloads and iCloud free-space operations are native.
  Oliver approved a direct Nextcloud connection: Settings → General → Connections,
  or a Nextcloud item's Connect action, links a local folder to a server and remote
  folder. Browser Login Flow v2 obtains Schnauz's own revocable app credential;
  keep it exclusively in macOS Keychain, bound to its server/login. Never read
  Nextcloud desktop credentials or put credentials in settings, argv or logs.
  `src/nextcloud.rs` reads nonsecret account routing once; `native/nextcloud/`
  handles networking, Keychain and native share/account UI through `nextcloudctl`.
  Share options, read-only public links, user/group sharing, permissions and
  internal links use OCS/WebDAV. Opening a menu/window never creates a share.
  File locking requires a fresh server capability check; unlock only confirmed
  user-owned locks belonging to the current user, never app/token/foreign locks.
  Disconnect revokes/removes only Schnauz's credential, even if Keychain access is
  missing or the server is offline. Preserve invalid account config files.
  Local automatic-space policy stays explicitly labeled as a Finder handoff;
  macOS does not expose another provider's Finder extension menu to Schnauz.
  Never recursively scan a folder while constructing a cloud menu. Test Nextcloud
  with URLProtocol fixtures and temporary settings, not Oliver's live server or
  Keychain. `./test.sh` includes those native tests. Live login remains manual QA.
- Hidden items (dot names and macOS UF_HIDDEN) are hidden by default; the per-tab eye
  toggle applies in every view and combines with type/text filters.
- Hide the inspector without an actual previewable single selection. Show text as
  readable native text in a scrollable panel, with bounded reads for large files.
- The preview's left edge resizes both text and visual previews. Remember width
  after the drag as part of the folder view, retain room for browsing, and double-click to reset to 420 pt.
  Start the preview below the tab, path/icon and type-filter rows so those headers
  retain their full width. Measure file viewports separately from header widths.
- New Text File asks for a name before creating anything, appends .txt, and uses
  create_new so existing data cannot be overwritten. Copy feedback names items in-pane.

- Top search is filenames/content only, never app commands. Default every fresh
  search to Current Folder + File; spaces mean AND and .ext is a filename suffix.
  Search recursively on demand, cancel stale work and show progressive results.
  Content mode must validate text rather than matching binary bytes or filenames.
  Do not restore startup home indexing.
- Show search results in the main file pane, with single-click selection/preview,
  double-click opening and explicit Show in Folder. Keep type/hidden filters,
  keyboard selection and copying consistent with the displayed search results.
- The small Schnauz mascot sits idle and runs only while search is active. Clicking
  it while idle opens About Schnauz with Coffee for Cookie. Clicking while running
  stops the scan, keeps the results, and shows a short status-bar notice that
  results may be incomplete. Decide using current search state at click time.
  Clear that notice when a new search starts.
- Inspect… is an explicit folder-size scan, available on folder, background and
  sidebar menus. Keep automatic browsing calculations off. Preserve the right-hand
  map while navigating from its file tiles or folder headings; never open a file
  merely by clicking its tile. Cancel replaced/closed scans and flag partial maps.
  It is labeled “Disk Usage…” and also has a button in the filter row.
  Like the preview, its panel starts below the path and filter bars. Selecting
  outside it closes the map; navigation from within its graphic keeps it open.

- The Schnauz app icon is fixed. Icon packs and app-icon customization are
  removed, including their loading, persistence, background recoloring and
  Dock overrides. Settings → Customization contains Theme and Colors only.
  Use native macOS file/application icons and keep their bounded cache;
  do not restore the legacy icon generator or customization controls.
- Use Apple's actual square.and.arrow.up SF Symbol for Share. Cache rasterized
  symbols per color and preserve their aspect ratio.
- Photos and videos fill the preview width. Native AVKit clips start paused and
  muted, offer playback/seek/unmute, and stop when their preview closes. Clip native
  views below GPUI headers and inside the scroll viewport; hide during app modals.
- Dates use NSDateFormatter and current macOS locale, time zone and hour cycle.
  Never hard-code English or AM/PM; reuse a cached formatter rather than per-row I/O.

- Normal interactive SSH from a new integrated local zsh terminal can follow
  remote Bash/Zsh folders in the same browser tab. This is terminal → browser only;
  never inject commands or directory-change keys into SSH. Clicking Synced
  detaches the view and restores the original local folder/layout while SSH and
  remote CLIs stay running. Click Unsynced to follow SSH again. SSH exit restores
  local browsing unless the user already detached and chose another local folder.
  Only our private local launch record can authorize a remote host/control socket.
  Use the live authenticated SSH mux only, with no reconnect/auth fallback. Remote
  prompt hooks are session-local temporary files; never modify remote dotfiles.
  Commands, tunnels, custom RemoteCommand and user-defined ssh functions retain
  normal behavior. Remote SFTP paths must quote glob characters and reject line
  breaks/NUL before generating a batch command. Preserve original local tabs in
  saved projects while the browser temporarily follows SSH.
- About is 520×600 pt, with content following naturally and no flexible blank
  spacer above Close. Preserve scrolling for small windows.
- Next release workflow requested: Oliver checks the current SSH/About build
  first. After that review, publish this project to his Gitea at 192.168.1.8,
  prepare a 0.5 release candidate, and install the release app in Applications.
  Do not publish before his review or restart live terminals to install a build.

- Todoist was approved for implementation, including automatic project creation.
  A Tasks pane below the shared path/filter bars follows the selected Schnauz
  project and stays visible without file selection. Match Todoist projects by
  normalized, case-insensitive name; reuse a unique match, ask for a choice if
  several have that name. New Schnauz projects persist a creation intent and stable
  idempotency key. Once connected, create the same-named Todoist project only after
  successfully checking all project pages and finding no match. A failed lookup
  must never trigger creation. Existing unlinked projects first look for a match;
  the explicit Find or create button authorizes creation if absent. Personal can
  have its own optional link. Keep actual IDs as opaque strings in projects.json.
  Todoist names are reflected in the Schnauz project name with uniqueness checks.
- Connect through Settings → General → Connections or the Tasks pane. The personal
  build uses an API token entered in a native secure field, stored only under
  local.schnauz.todoist in Keychain. todoist.json holds nonsecret account metadata.
  Never use another app's credentials or the user's live account for tests.
  Disconnect only removes Schnauz access, not the personal token other apps may use.
  OAuth is a future distribution improvement; do not embed a client secret.
- src/todoist.rs manages typed caches and background work; src/todoist_ui.rs renders
  only visible task rows, with sections, subtasks, due dates, priority colours,
  completion and Add task. native/todoist/ implements fixed-host HTTPS API v1,
  Keychain and native dialogs via todoistctl. Follow all pagination cursors,
  scope every page to the captured project, keep cached tasks on failure, honour
  rate-limit backoff, refresh every 30 seconds only with Tasks open and the app
  active. Discard responses after account changes and stale reads after writes.
  Add/complete actions retain their original project across project switches.
  Failed add forms retain text and the write request ID for explicit retry.
  Opening a pane or listing projects never writes tasks. Only new-project creation
  intents or the explicit Find/create button may create a project.
  ./test.sh includes native URLProtocol fixtures plus project/cache regressions;
  no live Todoist calls or Keychain access in tests. Manual account/UI QA remains
  Oliver's workflow. Preview videos stop while Tasks occupies the right pane.

- Remote listings enter the requested directory with SFTP `cd` before `ls -la`.
  Store immediate child basenames only; drop dot/parent entries even when a client
  prints full paths, and preserve whitespace inside filenames. Path breadcrumbs
  follow terminal cwd reports; listing prefixes must never become child paths.

- All current branding, implementation types, modules, shell hooks and build flags
  use Schnauz. Original attribution/licenses stay unchanged. `src/legacy.rs`
  isolates read compatibility for earlier settings, script headers/environment,
  SFTP Keychain items and active control sockets. Do not drop saved access when
  changing internal names. The former upstream updater is removed.

- The interface language is English; call the storage map Disk Usage. Disk Usage,
  file previews and Todoist share the right-panel wrapper, height, saved width
  and resize boundary below the navigation/filter rows. Always show scan progress
  or errors instead of an empty panel. Inspect closes Tasks; opening Tasks
  dismisses Disk Usage through the existing deferred close path.
  Close the map on actual browser selection/navigation, never on a root-level
  mouse press; moving/resizing the window and dragging its panel divider must
  preserve the map. Map-originated navigation explicitly preserves it too.
- Schnauz Light uses black normal/secondary text and controls. Upgrade saved
  stock light colours on load; preserve custom and dark palettes. Native macOS
  folder icons stay blue. Keep semantic status/tag colours and disabled states.
- The toolbar button is labeled Todoist with a checklist icon. Successful
  connection requests the Tasks pane; Settings → Connections also offers Show
  tasks. A pane-open request must not invalidate account/project caches.
- Settings uses the main app's transparent native titlebar and 56 pt themed
  header, with traffic-light clearance and independently scrolling content.
