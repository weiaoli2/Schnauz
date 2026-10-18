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
- Package with a stable Apple Development / Developer ID certificate, reusing
  the existing app's signer. `SCHNAUZ_CODESIGN_IDENTITY` can select a certificate
  name/SHA-1; missing/ambiguous identities fail before replacing the app. Ad-hoc
  signing is explicit opt-in with `SCHNAUZ_CODESIGN_IDENTITY=-` only, since its
  cdhash-based identity can invalidate TCC grants after rebuilds. Sign every
  helper and the app with the same identity. Stage and verify before replacing
  the bundle; never reset TCC, change trust settings or restart live sessions.
  A one-time ad-hoc → certificate transition requires the user to re-add the
  current app in Full Disk Access. Signing tests use certificate-name fixtures,
  not real keys or TCC. The local Keychain already has Oliver's Apple Development
  identity; use it through codesign without exporting keys or altering Keychain.
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
- Show a skippable English File access for Schnauz introduction once, with a
  persistent Settings → General → File Access section. Open macOS Privacy &
  Security → Full Disk Access only on an explicit button click. The user grants
  access there; never change permissions/TCC or probe protected folders to infer
  a grant. The central file_access_intro_seen preference records acknowledgement
  only. Do not force a restart or block normal browsing without Full Disk Access.
  There is no supported API to prepopulate only the Full Disk Access list; do
  not probe protected data to force an entry. On 2026-10-18 the enabled lowercase
  schnauz entry was traced to the current app's Contents/MacOS/schnauz binary.
  Its generic icon does not mean it is obsolete or prove the whole bundle grant.
  The user can add the complete current Schnauz.app manually with +.
  Hide native previews while the introduction is open.
- Folder navigation resets its vertical/horizontal scroll state and recomputes
  visible results. Local listing functions return Result; permission/read failures
  must show an in-pane error with Retry, never an empty-folder view. Offer File
  Access Settings only for a real permission error from a user-requested read.
  Do not infer Full Disk Access from shell access or change macOS privacy settings.
- Clicking the current breadcrumb or Cmd+L edits the full path; ancestors still
  navigate. The single-line editor supports mouse/Shift selection, double-click
  path components, select-all, cut/copy/paste, deletion and caret scrolling.
  Escape/outside clicks dismiss it; invalid Enter retains the text. The old
  current-folder command overlay is removed.
- Todoist empty/loading/error text and rows keep an explicit width, never a
  one-letter column. Open in Todoist uses the native desktop scheme when the
  app is installed, otherwise the browser. Oliver confirmed added tasks reach
  the correct project; preserve that routing rather than recreating projects.
- Add Task uses native/todoist/Composer.swift: an 800×380 pt themed window with
  the same little vector dog as the toolbar, not the app icon. Project is fixed
  to the captured Todoist ID/name; native title/description fields sit beside
  compact Date, Deadline, Priority, Labels, Reminder and Location controls.
  Optional location lookup uses Apple Maps only after a user address search,
  never current-device location. Credentials stay exclusively in Keychain.
  Time/location reminders are real API writes after task creation; plan failures
  preserve the created task and retry only pending options with the same IDs.
  A partial-save retry locks the saved task fields and never posts another task.
  Offline tests cover field payloads and light/dark layout without showing windows.
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

- Projects replace workspaces. Each owns Favourites, saved connection settings,
  Todoist routing and autosaved local tabs/layout. Personal has its own profile;
  new projects start with empty project Favourites. Common is shared across every
  project/window and lives in Library.common_shortcuts in projects.json. Migrate
  missing Common data once with Desktop and Downloads; an explicitly empty list
  means the user removed them and must stay empty. Home is pinned first in Common
  → Folders, shown once with the blue Home badge, never persisted or removable.
  Common accepts local/cloud/SSH folder/file references, with the same provider
  badges and no copies/transfers. Drop on Common targets Common; other sidebar
  space targets the captured project's Favourites. View in… includes Common.
  Common/Favourites are fixed headings; discard the obsolete FAVOURITES collapse
  flag. Common Home stays visible even if other Common folders are folded.
  All Schnauz browser windows share one active project; switching in any window
  switches every window. Within a project, each window browses independently.
  Share project metadata and the folder-view cache in memory, preserve per-window
  live tabs/PTYs on project switches, and skip autosaves for unchanged windows.
  Folder context menus offer Open / Open in New Tab / Open in New Window,
  including sidebar and SSH folders. New windows open only that folder without
  cloning terminals, restoring the other window's tabs or startup reconnects.
  Successful project switching, creation and removal are silent: the toolbar
  project name, dialog and sidebar already show the result. Do not allocate
  success notices or their dismissal timers for these actions; retain errors.
  The expanded sidebar is Devices → Projects → Common → Favourites, with compact 24 pt
  rows and Folders / Files subsections shown only when they contain favourites.
  Remove empty-section drop messages and the Favourites + button/picker code.
  Keep the transient drag-hover hint and whole-sidebar drop target.
  Create projects with the sidebar +, File menu or Cmd+Shift+S; the main toolbar
  has no New project button or its obsolete icon mapping.
  Projects has no leading disclosure marker
  or tab-count badge. Cloud, Servers, Locations and Tagged Items sections and
  their controls are removed. Preserve spacing between the main groups.
  Fit sidebar width to measured visible labels (170–320 pt, capped at 30% of the
  window). Truncate unusually long names with path tooltips, never horizontal
  scrolling. Collapsed still contains only its reopen button.
  All sidebar drops add references: folders to Folders, files to Files. Tag toggles
  the same membership, including SSH items; Finder colour tags remain separate.
  Show “Drop to add to Favourites” while hovering. Local drops validate metadata
  in the background. Remote drags use known listing types and an internal gesture,
  never fake local file URLs, transfers, terminal input or connection attempts.
  Escape / dropping outside the expanded sidebar cancels remote reference drags.
  Central projects.json stores typed shortcuts (path, folder/file kind, optional
  nonsecret SFTP server identity). Deduplicate by path plus server host/user/port,
  keeping identical local and remote paths separate. Never keep a live SSH mux in
  favourites; reuse an active terminal's authenticated transport only on explicit
  favourite navigation. Keep its terminal and PTY visible in the current tab;
  never create an implicit replacement tab that hides the session. A saved remote
  favourite can explicitly reconnect through normal SFTP after restart; never
  inject commands into an SSH terminal.
  Local/cloud folders retain native blue icons with an overlaid cloud badge;
  SSH/network shortcuts have a globe badge. Mounted-network identity is cached
  by the volume helper; never probe mounts or connect while rendering.
  Merge legacy favourites, Locations, Cloud, network folders and Tagged Items
  without dropping missing paths. New saves contain only typed shortcuts plus
  existing account/settings metadata. Preserve original legacy files and shared
  Keychain credentials. Saved connections stay accessible in Settings and
  Go → Connect to Server… (Cmd+K). View in… shares any favourite with All projects
  first, then individual projects, without switching; All affects current projects
  including Personal. Preserve live PTYs and imported workspace layouts.
- Devices is a global fixed section above the scrolling Projects/Favourites area,
  independent of the selected project. Show mounted local startup/external drives
  with compact free-space labels. Clicking only the capacity badge toggles free
  GB / free percent for all drives without navigating; save the mode in central
  prefs. Colours still reflect used capacity. Use cached totals for the toggle.
  Use filesystem capacity, refreshed off-thread
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
- Hide the inspector without an actual previewable single selection. Text uses a
  read-only native NSTextView/NSScrollView at UI_FONT_SIZE (12 pt, matching file
  names), with mouse selection, Cmd+A and Cmd+C. Keep bounded file reads and the
  shortened-preview notice. Preserve native selection/scroll while resizing or
  redrawing; release native focus on clicks elsewhere and hide during app modals.
  Cmd+C in the preview copies text, never the selected file object.
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
- Latest instruction, 2026-10-18: release publication is ON HOLD. Save and reuse
  release.sh plus docs/RELEASING.md, address Oliver's main bugs first, and resume
  publication only when he asks. Public source is already uploaded; do not create
  tags, releases, uploads or further pushes during this hold. Refresh the website kit only when explicitly requested (the product-info and
  screenshot refresh was authorized later on 2026-10-18). Do not regenerate
  screenshots or rebuild release installers for each routine bug fix;
  build the normal development app as needed and rebuild both release targets
  only when release work resumes. Existing candidate artifacts predate bug fixes.
- On 2026-10-18 Oliver authorized publication to his GitHub account, and explicitly
  chose PUBLIC source and PUBLIC downloads. Use weiaoli2/Schnauz and v0.5.0-rc.1
  for the first Apple Silicon + Intel release candidate. This supersedes the
  earlier Gitea plan and private-repository answer. Preserve upstream attribution,
  complete dependency notices and publish source and both verified architectures.
  Use separate release output paths; never restart live terminals or replace the
  running app to publish. The current certificate is Apple Development; clearly
  label the candidate as not notarized. Never upload signing keys or live settings.

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
  Oliver's workflow. File previews and video playback remain independent of Tasks.

- Remote listings enter the requested directory with SFTP `cd` before `ls -la`.
  Store immediate child basenames only; drop dot/parent entries even when a client
  prints full paths, and preserve whitespace inside filenames. Path breadcrumbs
  follow terminal cwd reports; listing prefixes must never become child paths.

- All current branding, implementation types, modules, shell hooks and build flags
  use Schnauz. Original attribution/licenses stay unchanged. `src/legacy.rs`
  isolates read compatibility for earlier settings, script headers/environment,
  SFTP Keychain items and active control sockets. Do not drop saved access when
  changing internal names. The former upstream updater is removed.

- The interface language is English; call the storage map Disk Usage. File preview
  is independent of the framed tools pane, where Todoist and Disk Usage replace
  each other. Both start below the full-width navigation/filter rows. Keep both
  in the fixed order files → preview → Todoist / Disk Usage, never stacked.
  Preview consumes browser width without growing the window. Opening tools grows
  the window to the right, within the display; switching tools must not grow it
  twice. Closing removes only the added width, preserving manual window resizing.
  Resize both preview and tools from their own left edges. The outer right edge
  is exclusively the native window resize border. Changing tools width only
  redistributes existing space; automatic window growth occurs on opening tools. Keep
  split proportions based on the browser/preview area before the tool extension.
  Pane/viewport measurements are per window, never process-wide. In full screen
  or at screen edges, fit panels beside each other without overlap.
  Preview width stays in folder views; tools_width is
  saved separately in central prefs, only after a drag. Use unique resize IDs.
  Always show scan progress or errors instead of an empty panel. Inspect closes
  Tasks; opening Tasks dismisses Disk Usage through the deferred close path.
  Close the map on actual browser selection/navigation, never on a root-level
  mouse press; moving/resizing the window and dragging its panel divider must
  preserve the map. Map-originated navigation explicitly preserves it too.
- Schnauz Light uses black normal/secondary text and controls. Upgrade saved
  stock light colours on load; preserve custom and dark palettes. Native macOS
  folder icons stay blue. Keep semantic status/tag colours and disabled states.
- The Todoist toolbar button (checklist icon) exists only with configured account
  metadata. Never query Keychain while rendering. Disconnect also closes Tasks.
  Successful connection requests Tasks; Settings → Connections offers Show tasks.
  Opening a pane must not invalidate account/project caches. Keep Terminal and
  Preview labeled in the top toolbar; Preview toggles only the file preview.
- Keep the footer on one line. The theme label shows the current Light mode / Dark mode; clicking switches.
  Settings is a centered 14 pt gear in a fixed 26 pt button, with its label and
  Cmd+, shortcut in the tooltip only. New Folder has a closed folder outline
  and an inset plus; keep all strokes clear of the icon canvas edges.
- Settings uses the main app's transparent native titlebar and 56 pt themed
  header, with traffic-light clearance and independently scrolling content.

- General notices, cloud progress and SSH errors belong in the existing fixed-height
  footer, centered in a rounded blue badge matching Home; never allocate a top
  banner or shift browser content. Keep count and mode/settings visible; the footer has no duplicate path,
  truncate long messages with full tooltips and retain SSH error dismissal.
- Todoist uses definite pixel widths from the actual framed tools allocation,
  including text/status rows and the virtual list. test-ui.sh checks the complete
  real GPUI window with empty and populated synthetic tasks, without showing a
  window or reading an account. The ui-checks feature is excluded from normal builds.
  Helper waits and snapshot sorting run on dedicated worker threads, never on the
  UI thread or GPUI's shared file-work pool. Automatic refresh/linking cannot
  display a Keychain prompt, project chooser or activate a window. Explicit Add
  Task opens from the captured project name before any network validation; validate
  that fixed project on Save. Confirmed tasks populate even an uninitialized cache.
- Drag hover is tracked from the root against stable sidebar geometry, with a
  small exit tolerance; the fixed-size hint must not change layout. Paint the
  internal SSH drag preview last, above the sidebar. Preserve reference-only drops
  and live SSH sessions. Native drag QA must use disposable folders, not live data.

- Show only the compiled app VERSION in subtle grey on the upper Schnauz title
  row, clear of native window controls. --version prints the same version.
  Oliver explicitly removed date/build/source identifiers from the UI. Keep the
  app filename Schnauz.app and bump the ordinary version for delivered test stands;
  no per-build app copies or releases. The current local testing version is 0.5.4.
- Todoist task rows use completion circles, thin separators and dates below the
  title. The indigo + Add task row is last in the virtual list, even when empty.
  Named sections/subtasks stay supported; no redundant Tasks heading. Test actual
  task-title/status-text bounds, not merely the enclosing panel width.

- File/folder dragging never tints the full browser/sidebar or destination names.
  Keep real selection unchanged and only the small sidebar drop hint. The former
  drop_hover paint tracking/state and file-row highlight argument are removed.
- Native outer resize uses AppKit exclusively. window_chrome.rs only stabilizes
  resize cursors in the native inner one-point edge; no mouse handlers, animated
  borders, backgrounds or custom resize loop. Verified all four native edges and
  four corners with the normal 0.5.1 app in a disposable profile.

- Local Home breadcrumbs start with ~/ and retain actual navigation targets.
  Clicking ~ opens a native Home/parent-folders menu; cloud badges and remote
  roots retain their existing identity. Cmd+L/current crumb still edits the full
  physical path. The footer shows counts/status/mode/settings, never the path.

- File dragging: normal drag moves; Command (not Option) copies, per Oliver.
  Native AppKit Copy shows its green + badge and the footer says Copy N items;
  Move has no + and says Move N items. Count the frozen payload, not the current
  tab selection. Respect source operation masks; local-to-SFTP remains upload/copy
  and sidebar drops remain references only. Keep drag-out to other apps copy-only.
- A hovered destination tab alone turns blue and activates after 650 ms; hovering
  a normal folder or parent row then opens it after the same dwell. List and Icons
  share this, with one cancellable timer, stable tab IDs and drag identity guards.
  Never restore whole-browser/sidebar/folder-name drag tint.
- native_file_drag.rs adapts only owned browser NSWindow instances because GPUI
  0.2 advertises every native drop as Copy. Forward original callbacks unchanged;
  never alter dependency caches, global classes, native resizing or terminal views.
- Saved terminal favourites preserve a nonsecret ssh_alias in addition to resolved
  identity. Legacy favourites may recover a unique matching OpenSSH Host profile
  off-thread; never guess ambiguous profiles or read private key contents. Live
  mux transports stay ephemeral and fail closed, without reconnect fallback.
  Reuse only active terminal links, never orphaned browser-only mux references.
- SSH -> local favourite -> remote favourite keeps the same terminal, tab and PTY
  visible. Resume following only at that session's reported remote folder; other
  folders/servers stay Unsynced. Never send cd or restart to achieve this. Explicit
  New Tab/New Window and project switching retain their existing semantics.
- test-ui.sh now also checks native drag forwarding, dwell, modifier feedback,
  real copy/move through drop handlers and terminal favourite roundtrips in a
  hidden disposable window with a private pasteboard and an owned test shell.
  No real servers/accounts/user sessions are accessed; ui-checks stays opt-in.

- Oliver explicitly authorized refreshing the website product info and screenshots
  on 2026-10-18, prioritizing project-specific work, seamless terminal use, pasted
  file/folder paths and Todoist. This supersedes the screenshot/kit freeze only;
  release publication, tags, pushes and installers remain on hold. Do not present
  unpublished candidate asset URLs as working downloads. Retain the GitHub link.
- Terminal in an SSH folder starts a NEW PTY and remote shell in that captured
  directory, using remote_terminal.rs. Reuse authenticated masters via an owned
  socket alias; never unlink/replace the original master or send input to another
  session. Live-only transports fail closed. Initial cd is new-session startup
  data, safely quoted including OpenSSH percent expansion, never later injection.
  Background preparation attaches by stable tab/request/project IDs; duplicate
  clicks cancel only a pending launch. Preserve all existing terminals.
- Local path paste while a terminal is present in a remote tab detaches browsing
  in that same tab, preserving the terminal visibly. navigate_pasted_path is shared
  by the clipboard handler and the opt-in preview's path-navigation scene.
- product_preview.rs is excluded from normal builds. Its separate bundle must
  default to isolated fixture mode even without --website-preview; LaunchServices
  can drop launch arguments. Never let that bundle load the normal profile. Preview start
  scenes show synthetic tasks with refresh disabled, an owned test shell
  and sample files; no live accounts or credential reads. CUA captures real UI
  pixels; caption sample data accurately and do not invent UI or remote sessions.

- Reproduce website captures with --website-preview --website-preview-scene=N
  in the separate isolated preview bundle: 0 light overview, 1 Brand Refresh,
  2 pasted file path, 3 dark overview. Only the preview's owned test shell may
  be restarted for captures. The final four 3200x1640 JPEGs and handoff are in
  outputs/Schnauz-Website-Kit; originals are raw CUA captures, not retouched UI.
