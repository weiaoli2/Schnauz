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
  Include `Co-authored-by: Codex <codex@openai.com>` in future commit messages,
  as requested by Oliver. The local `.git/hooks/commit-msg` hook adds it
  automatically without duplicating the trailer. Preserve published history
  and release tags; do not commit or publish until Oliver requests it.
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
  Common/Favourites and their Folders/Files subsections are fixed, always-visible
  headings. Ignore legacy FAVOURITES/FOLDERS/FILES/COMMON_FOLDERS/COMMON_FILES
  collapse flags; clicking subsection labels must never hide shortcuts.
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
  The direct offline bridge targets Nextcloud Desktop 34.0.1; older clients
  need not be supported. Discover installations/accounts per user; never depend
  on Oliver's paths, SwiftBar or ncstatus. Internal macOS File Provider selectors
  require availability and ABI checks. Oliver confirmed live sharing/offline-provider QA on 2026-10-30;
  these actions work in his actual workflow and issue #1 is resolved in 0.9.22. Omit Finder handoff.
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
- Oliver resumed publication on 2026-10-18: after footer transfer progress and
  stable tab-drag highlighting are fixed, publish a small public GitHub release
  0.5.5 with an English feature overview and https://techguyzone.com/apps/.
  Reuse release.sh + docs/RELEASING.md for Apple Silicon and Intel. No new release
  screenshots. Preserve all live apps/terminals, source attribution and credentials.
  Publish the signed, verified artifacts as an early prerelease, clearly stating
  Apple Development signing and no notarization. The previous hold is superseded.
- On 2026-10-18 Oliver authorized publication to his GitHub account, and explicitly
  chose PUBLIC source and PUBLIC downloads. Use weiaoli2/Schnauz for Apple Silicon + Intel releases; 0.5.0-rc.1 was
  the unpublished preparation candidate and is superseded by 0.5.5. This supersedes the
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
  no per-build app copies or releases. The published release is 0.8.9; the normal local 0.8.13 update includes project filters, the seamless active tab and finer toolbar icons and is staged while the old app runs.
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
  file/folder paths and Todoist. That request superseded only the screenshot/kit freeze; publication was later
  separately resumed for 0.5.5. Do not present
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
  website/Schnauz-Website-Kit; originals are raw CUA captures, not retouched UI.

- Oliver confirmed both local/SSH and SSH/SSH copying work; the reported issue
  was missing feedback. Retain the working transfer transport. Local jobs and
  SFTP upload/download jobs use the fixed footer (transfer_status.rs): current
  operation/file, real percentage, compact bar and explicit Cancel. Network
  percentages are for the current file, labeled as such, not a fabricated batch
  estimate. Preparation stays indeterminate, completion appears only after success.
  The old floating jobs panel is removed. SFTP uses its own private batch/PTY,
  never a live terminal; batch files are mode 0600 and removed. Cancellation
  signals only the new transfer process group and explains possible partial files.
- File-tab hover colour is stored in spring_drag::State with stable tab IDs and
  pointer bounds. Do not use GPUI drag_over styling for it; hitbox hover state can
  disappear on redraw without mouse movement. Keep blue after dwell activation,
  clearing it only on pointer exit, a new drag, drop or drag completion.

- 0.5.5 publication completed 2026-10-18 at
  https://github.com/weiaoli2/Schnauz/releases/tag/v0.5.5. All four uploaded asset
  digests match local checksums. Release source is tag v0.5.5 / 7c48f14; later
  documentation-only commits do not change that artifact provenance. This request
  is complete, not continuing authorization for future releases. Reuse the saved
  workflow on a new request; never move the existing tag or overwrite its assets.

- Open With owns Schnauz-only, case-insensitive extension choices in central
  file-apps.json (src/file_apps.rs), shared across projects/windows. Different
  apps ask OK (always), Only Now or Cancel; never change LaunchServices defaults.
  Other Application… permits an app outside macOS suggestions. Cache native menu
  app icons by path and mark the effective app. Use macOS Default Again removes
  the override. Ordinary, multi-file, palette and downloaded-SSH opens share the
  resolver; folders/app bundles/extensionless files retain normal behavior.

- Image previews use explicit aspect-ratio dimensions, never percentage width with
  intrinsic pixel height. Wheel zoom applies to visual previews; native text uses
  Shift+wheel for zoom and ordinary wheel for native scrolling;
  selecting another file resets the transient zoom and image scroll. Drag images
  to pan at enlarged sizes. Keep native text selection/copy and bounded reads.
- Quick Actions includes installed file-input Automator workflows from user/system
  Library/Services, matched with native UTType conformance (including Finder-scoped
  actions). Launch only on the action click, with literal captured path arguments;
  never run a user's workflow during QA. Use disposable workflow fixtures.

- ⌘-right-click opens Quick Actions directly on local file/folder items, including
  Common/Favourites and search results. Settings → Keybinds lists installed actions
  and built-ins. Keep shortcuts central, persistent, conflict-checked and inactive
  in editors/modals/remote selections. Never run a workflow while assigning a key.
- Preview Information uses two label/value pairs per row (four columns), with
  explicit 12 pt side padding for the heading and fields. Keep full-value tooltips
  for truncated data. Video uses the same compact grid and loaded dimensions.

- Shared section headings, especially Information, must be definite-width single
  lines. Preview dates show the locale-formatted date above the locale-formatted
  time, with one-line labels and tight 13 pt lines/2 pt row gaps. Keep the four
  columns and 12 pt side insets. Cache native date-only/time-only formatters;
  never split a localized combined date string on punctuation.
- Preview minimum width is 253 pt (15% above 220), shared by allocation, divider
  drags, saved views and preference loading, subject to available screen space.
- Information text needs explicit pixel widths derived from the actual pane;
  container bounds alone do not prove glyphs are visible. Avoid preliminary
  zero-width truncation. UI checks must assert actual heading/value text and
  nonzero shaped-text bounds as well as compact row geometry.
- Native drag dwell uses a common-run-loop-mode timer so tabs/folders activate
  and repaint during AppKit tracking. Retire GPUI pointer tracking at the native
  handoff and clear source/destination drag state on cancellation or completion;
  same-folder cancellation must allow immediate selection. Target the actual
  GPUI view, not the plain NSWindow content wrapper. QA uses disposable files.
- Recognize home-level cloud symlink aliases from cached startup routing, without
  reading desktop credentials, probing targets or scanning descendants. Use
  physical paths for linked-account matching and logical alias paths for display.
  Mark downloaded cloud files/folders too; never apply local identity to SSH.
- Sidebar folder context menus apply the main pane's current view and sorting to
  that folder and descendants through central folder-view rules, never by scanning
  the tree or changing the sidebar's appearance. Preserve unrelated overrides.
- Oliver's requested 0.5.9 release was published on 2026-10-18 at
  https://github.com/weiaoli2/Schnauz/releases/tag/v0.5.9, source ba395fa.
  Both DMGs, MPL sources and checksum asset match local SHA-256; native ARM and
  Rosetta Intel text-preview checks passed. Apple Development signed, unnotarized
  prerelease. Receipt: dist/0.5.9/RELEASE-VERIFICATION.json. The request is complete;
  future publication needs a new request. Preserve existing tags/assets/sessions.
- Search text owns Cmd+C/X/V while editing, including native Cut/Copy/Paste/
  Select All context menus. Preserve Unicode selections and meaningful spaces;
  pasted text must never trigger file paste/navigation while editing a query.
- Terminal mouse selection copies nonempty text on release, including word/line
  selection and release outside. Plain clicks cannot overwrite the clipboard.
  Respect app mouse reporting; Shift overrides it for text selection. Keep the
  right scrollbar visible and draggable, reflecting actual Alacritty scrollback
  without sending PTY input or covering terminal text.
- Tab menus offer Move to Project… with a scrollable destination list and an
  Activate Project checkbox below it, default on. Move the owned Tab/Session/PTY
  (not a copy or new process), preserving terminal view, output, running jobs,
  SSH identity, selection and navigation history. Persist both layouts before
  changing ownership; save failure leaves sessions intact. Cover Personal,
  parked destinations and last-tab/split handling. Native previews hide in the
  modal. Tab View in… is renamed Add to Favourites…; it adds only a reference,
  with destination feedback. Other shortcut View in… semantics remain.
- Video viewports need explicit pixel widths and flex-none native canvas sizes.
  Test the actual attached NSView, not only metadata/container bounds. Start
  the player independently of optional metadata/duration loading and retain
  paused/muted defaults. Use generated H.264 fixtures for playback/viewport QA.
- About Schnauz (idle mascot / Coffee for Cookie) owns the opt-in startup update
  checkbox, default OFF, with “Updates are checked via GitHub.” beneath it.
  Manual Check for Updates is available there, in Settings and the app menu.
  Use only public weiaoli2/Schnauz releases, including published prereleases;
  ordinary tags/drafts are not updates. Compare semantic version precedence,
  never downgrade local builds. Require the exact DMG for the compiled app's
  architecture (Intel stays Intel under Rosetta), validated to the same GitHub
  repository. No token, telemetry, credentials or personal curl config. Bounded
  HTTPS checks run once on opted-in startup, deduplicated per process, on a
  dedicated worker with timeouts. Never replace or restart the app/terminals.
  Show a 13 pt blue info badge above the toolbar mascot for discovered updates;
  tooltip “Update available”. A locally newer build gets an amber info badge and
  About warning naming installed/latest versions, never a downgrade download.
  Clicking either badge opens About without stopping a running search.
  Downloads/notes open only on a user's click. UI tests use injected releases.
- Oliver requested a committed/tagged 0.5.11 checkpoint before implementing the
  detached Todoist window; no GitHub Release or DMG publication at this stage.
  Then implement/test one global floating Todoist window (shared active project),
  full/compact-first-task modes, always-on-top/all-Spaces and Dock Here. Multiple
  browser windows must show/reuse the same floater rather than detach duplicates.
  Preserve the checkpoint for rollback. Publish a later release only after the
  new integration has passed testing, including Oliver's live workflow review.

- Todoist uses one shared controller independent of browsers, with weak browser
  references only. A small upper-right pin detaches a singleton floating window;
  all browser panes close. Show Floating Tasks reuses it, Dock Here targets that
  browser, and the floater pin returns to its origin or another surviving browser.
  Follow the shared active project without changing browser/terminal ownership.
  Compact shows the first open task; Expand restores the previous full size.
  Native floating level, no hide-on-deactivate, all Spaces and eligible other-app
  full-screen/Stage Manager participation are explicit AppKit configuration.
  Refresh while the floater is open even if another app is active. Account
  disconnect closes it; writes retain their captured account/project guards.
  Keep per-window scroll handles and show errors without hiding the compact task.
  Native pin/collapse/close QA uses hidden, disposable windows and synthetic tasks.
- The per-window native file-drop subclass restores its original GPUI class before
  deallocation. AppKit's existing KVO forwarding otherwise throws an unregistered
  _windowLayerContext observer exception when a browser is destroyed. Exercise
  actual browser destruction between event-loop turns, not merely hide/remove.

- Todoist pin controls use two neutral outline macOS symbols: pin while docked,
  pin.slash while floating. Keep normal text/secondary colour, never accent blue;
  hover tooltips name the current state and the Detach/Dock Todoist action.

- Oliver requested and approved the 0.5.14 public release on 2026-10-19.
  Published https://github.com/weiaoli2/Schnauz/releases/tag/v0.5.14, source
  5b26dd7. Both DMGs, MPL sources and SHA256SUMS match all GitHub digests;
  source ZIP, helper architectures/signatures, mounted installers and native ARM /
  Rosetta Intel text-preview checks passed. It remains Apple Development signed,
  not notarized, and an early prerelease. Receipts live in dist/0.5.14.
  This publication request is complete. Future releases need a new request;
  preserve existing tags/assets, including v0.5.11, and all live sessions.

- Sidebar projects and references within each Common/Favourites Folders/Files group
  reorder by internal drag, with an insertion line and central persisted order.
  Personal, Home and section order remain fixed. Do not turn ordering gestures
  into file transfers, navigation or terminal commands. Single-click sidebar files
  reveals/selects; double-click opens via existing app preferences/download flow.
  A 5 pt green dot precedes project icons with live tabs/terminals across any
  browser, including parked project sessions; saved layouts alone are inactive.
  Runtime activity is removed when its owning browser is released.

- Folder tabs can be dragged onto Common or the expanded sidebar to add a typed
  folder reference to Common or the captured project's Favourites. Freeze folder,
  server and project in the tab-drag payload; never move/close its tab or PTY,
  transfer files, read remote metadata or connect. Reuse the small reference-drop
  hint, without destination tint. Keep tab reordering and drag-to-split intact,
  rejecting stale tab identities and foreign-window tab moves.

- Oliver requested text-only collapse/expand by clicking PROJECTS and COMMON.
  Both headings persist while collapsed, with no new disclosure icon. Save their
  state in collapsed_sections.txt; Common uses the COMMON key, leaving the legacy
  Folders/Files flags ignored. Dropping a reference on collapsed Common expands
  it to reveal the result. Favourites and subsection labels retain their existing
  noncollapsing behavior. Collapsing groups never closes tabs or terminals.

- Keep Common/Favourites compact: main headings use 8 pt top/no bottom padding,
  with 4 pt above the Folders/Files labels and 2 pt below. Rows stay 24 pt.

- Weblinks is a third sidebar reference type in Common and project Favourites.
  Each centrally stored bookmark has a stable ID, name, HTTP(S) URL and optional
  browser app path. Single-click opens; Edit weblink changes name/link/browser;
  Copy link, Remove and drag ordering are supported. Per-link browsers never
  change macOS defaults. Discover browsers only on explicit editor opening, not
  during rendering. Missing chosen browsers require editing, never silently
  fall back. native/weblinks/ provides native fields and browser choices through
  signed weblinkctl; keep clipboard shortcuts and preserve failed-save drafts.
  Capture ownership across project switches and reject stale edits. Use offline
  fixture forms and intercepted launches in tests; never open real user URLs.

- Quit Schnauz checks only active terminal work, not the mere presence of terminal
  tabs/idle shells. src/quit_guard.rs scans owned PTY children on an explicit quit
  request, groups processes by current/parked project across all browser windows,
  and offers Cancel / Quit Anyway. Include background/suspended jobs; never inject
  terminal commands or query remote processes for this check. The owned AppKit app
  delegate subclass also guards Dock Quit, inheriting GPUI's normal shutdown.
  Tests use fixture PTYs and intercept actual quitting; preserve live sessions.
- The 62 pt browser toolbar aligns the search field with the Schnauz title and uses
  the row beneath for active-project chips centered across the full window
  when they fit, with horizontal scrolling on overflow. Active means live tabs/terminals in
  any window, matching sidebar dots; saved layouts alone are excluded. Current
  project has a blue background. Click chips through the existing global project
  switch, retaining every per-window live PTY. Keep a horizontal scroll handle per
  window and reveal the selected chip after project/order changes. No header
  overlap with native window controls, tabs or path/filter bars. Keep a 1 pt
  subtle divider with 1 pt above and 2 pt below the line between rows, and 2 pt
  below the project chips to match their top clearance; Settings
  retains its independent 56 pt header.

- Oliver chose 0.7 for this next feature build; use ordinary version 0.7.0.
  No publication is implied by the version choice; public release remains 0.5.14.
  Oliver will test 0.7.0 first; wait for his explicit go-ahead before any release.
- Show the filter-row reload button only for SSH/SFTP tabs (remote.is_some()).
  Local and locally synced cloud folders use the existing watcher and hide it.
  Retain Cmd+R for manual refresh everywhere; do not change refresh transport.

- Saved sidebar folders and files support optional Alias and Add-on through “Edit alias and
  add-on…” in their context menu. Display alias or original basename, followed
  by an optional parenthesized add-on. Store both on the typed references in
  projects.json; changes always update all matching destinations across Common,
  Personal and every project, without options or confirmation. Preserve paths, server identity,
  ordering and labels when re-adding a destination. Home remains fixed. Native
  fields support ordinary clipboard editing; stale editors cannot replace changed
  labels or recreate removed references. Tab hover shows the full current folder
  path, with server identity for SSH, without connecting or reading metadata.

- Quit-warning Cancel must close the native dialog, end its modal session and
  restore normal browser drawing while keeping the app and all programs open.
  Use a standalone NSAlert rather than a sheet: attaching a sheet to the adapted
  browser NSWindow conflicts with AppKit's internal KVO bookkeeping. Own the
  alert through completion and leave AppKit's button response tags intact. Native
  modal-session polling runs outside GPUI borrows. Regression QA clicks real
  Cancel buttons and verifies no visible dialog or modal session remains and the
  same owned terminal/job survives; mocked replies alone cannot check dismissal.

- SSH connection and remote-folder loading progress uses the existing centered
  footer badge, with server identity. Keep it per tab/load, clear on completion
  or failure, and show it ahead of stale general notices. Do not start duplicate
  listings when selecting a pending connection or update a different tab after
  tab switching/reordering. QA uses synthetic progress, never a live SSH server.

- Terminal Select text holds a bounded local grid snapshot for selecting/copying
  while a foreground program keeps running and redrawing. The header switches
  to Live terminal; typing, pasting or Escape returns live. Shift-drag in app
  mouse/alternate-screen mode also snapshots. Keep processing live PTY output,
  protocol replies and resizes; never send mouse/scroll input to the program
  while selecting a snapshot. Cmd+C copies the snapshot selection; Cmd+V resumes
  and follows the live bracketed-paste mode. Test real AppKit mouse events across
  a redraw, with captured input and preserved owned session/PID.

- Tabs with a live terminal session show a small neutral terminal vector icon
  on the right, before Close. Keep it visible when the panel is hidden; ended
  sessions and tabs without a terminal have no indicator. The icon is informational
  and keeps ordinary tab selection/drag behavior.

- SSH browser tabs (remote.is_some()) show the same Globe vector icon as the
  SSH favourite-folder badge, beside any terminal indicator before Close. Both
  indicators are strictly black in light mode and white in dark mode, including
  inactive tabs. Browsing a local folder after detaching SSH hides the globe;
  its live terminal indicator remains independent.

- On Quit or closing the last browser window, ask Keep Setup & Quit / Start Fresh
  Next Time / Cancel, after any existing active-terminal warning. Standalone
  native modal sessions must end and hide on every choice. Cancel changes no
  startup choice; a save failure leaves the app open with an error. Start Fresh
  never deletes projects, favourites, credentials or folder-view preferences.
- Explicit Keep saves nonsecret startup-setup.json separately from ordinary
  project layouts: browser windows/frames, active and parked projects, local/SSH
  tabs, active tab/pane/splits, view/filter/selection, terminal presence/visibility,
  height and cwd. Restore fresh processes; never resume commands, output, jobs,
  environment or old SSH control sockets. Retain original Host aliases, never
  credentials. Keep authorizes fresh SSH reconnection on the next launch; parked
  projects restart their terminals when visited. Failed restarts retain their
  saved intent without automatically retrying each frame. Native QA uses fixture
  shells/captured remote metadata, never real servers, user startup files or
  live sessions. Disk Usage remains an explicit scan.

- Network-drive toolbar button opens native SMB/AFP connection/share selection.
  `src/network.rs` keeps mounted shares as local paths with typed server/share,
  user and share-relative references; `native/network/` uses public NetFS APIs.
  Passwords stay exclusively in Schnauz's own macOS Keychain service, never URLs,
  process arguments, projects or session JSON. Favourites preserve the reference
  and reconnect explicitly; restored disconnected tabs require Retry, without
  automatic authentication or mount probes while rendering. Mount/list waits run
  on dedicated workers. Test using synthetic paths/forms, never live servers or
  Keychain. Do not unmount a share on cancel or close: other apps may use it.

- Terminal/Preview/Todoist (and floating-task Dock Here) backgrounds match the
  26 pt Search scope / File-Content groups via TOOLBAR_CONTROL_H. Preserve the
  existing label/icon sizes; do not change shared dialog-button heights.

- Selected active-project chips and Terminal/Preview/Todoist toolbar toggles share
  CHROME_HIGHLIGHT blue and the same darker hover blue. Other dialogs retain the
  configured theme accent; text/icon sizes and neutral inactive controls stay intact.

- Add Weblink… is available by right-clicking COMMON or FAVOURITES, including
  collapsed Common. No inline add row; hide empty Weblinks subsections. Capture
  the target scope in the menu and preserve failed drafts for retry there.

- The network-drive globe belongs in each pane’s file action icon bar, before the
  hidden-items toggle, never beside the top Split view/Terminal controls. Clicking
  it targets that pane before opening the native connection dialog.

- Oliver authorized GitHub release 0.8.0 on 2026-10-20 after shared alias/add-on
  labels. Publish both architectures and English notes covering changes since
  0.5.14; retain Apple Development / unnotarized prerelease disclosure.

- During 0.8.0 draft upload Oliver reported stale text previews after external
  saves. Preserve v0.8.0 and its unpublished draft; publish the corrected 0.8
  series build as 0.8.1 under the ongoing release request. Selected local file
  vnode watches must refresh cached preview/info for in-place and atomic saves,
  cancel stale workers and preserve native text selection on unrelated redraws.

- Published 0.8.1 on 2026-10-20, source c8a4275, at
  https://github.com/weiaoli2/Schnauz/releases/tag/v0.8.1. Both DMGs, MPL sources,
  checksums and GitHub source archive verified; ARM native / Intel Rosetta smoke
  checks passed. Apple Development signed, not notarized, early prerelease.
  v0.8.0 remains a superseded unpublished draft. Receipts: dist/0.8.1. Request
  complete; future publication needs a new request. Preserve all tags/sessions.

- Project context menus in the sidebar and top bar offer Close for runtime-open
  projects, including Personal. Explicit Close clears all windows' owned tabs/PTYs
  for that project, retaining saved metadata; select the next runtime-open project
  in sidebar order with wraparound. Individual final-tab closure drops only its
  window's outgoing session and switches globally, preserving other windows' tabs.
  No other open project falls back to Personal/Home. Persist before dropping tabs.
- Release architecture outputs under dist/ do not update the usual root
  Schnauz.app. Also build/verify that local bundle after release preparation;
  never restart the user's running app or terminals to apply it.

- File-action toolbar controls reorder by direct drag with a stable insertion
  marker, using a shared persisted prefs order. Keep List/Icons as one group and
  retain hidden narrow/remote controls in the stored order. The document-plus
  New Text File button defaults after New Folder and uses the existing named
  .txt dialog/create_new path. Share/Sort/View Options trigger on click release
  so dragging never opens their menus. No file payloads or session changes.


- Local 0.8.7 fixes the reported terminal-divider SIGABRT: shrinking file-list
  viewports must bound scrollbar minimums to the available extent, including
  scrollbar painting/dragging and horizontal/static bars. Preserve the actual
  native upward-divider drag regression with a live fixture PTY.
- Normal app launches save private, bounded session diagnostics under
  ~/Library/Logs/Schnauz; explicit/legacy profile overrides use diagnostics/ in
  that profile. Keep forced Rust panic stacks, native uncaught-exception stacks,
  safe fatal-signal markers and ordinary macOS crash handling. Raw signal faults
  have no custom stack. Fixed event labels only; no terminal output/commands,
  file contents or credentials. Retain 12 sessions. Report destination and
  automatic/manual sending are undecided; no external upload is configured.

- Local 0.8.8 supersedes the compact project-row spacing: Oliver requested
  8 pt above/below the project chips, four times the former 2 pt clearance.
  Browser header is 74 pt; retain chip size, Search row and Settings geometry.


- 2026-10-23: keeping a PID alive is insufficient when packaging. Never move or
  unlink a running app bundle. The prior temporary-backup cleanup deleted the
  live executable and broke TCC attribution (ENOENT), denying Downloads reads.
  make_app.sh must use scripts/install_bundle.py to defer installation while the
  destination or its helpers are active. Apply pending only after a deliberate
  quit; never auto-restart. Preserve retired bundles outside temporary roots.
  Do not delete retained bundles while any process might still use them. Keep
  the owned-process installation regression in test.sh. No privacy/TCC reset.


- Oliver requested an updated GitHub release on 2026-10-23 after the confirmed
  live-bundle deletion bug. Prepare/publish 0.8.9 as an early prerelease with
  both architectures, the accumulated verified local fixes, local diagnostics,
  exact MPL sources and checksums. Preserve all prior tags/assets and sessions.
  Explain that the safety fix is in the source build/install workflow; manual
  DMG/Finder replacement still requires quitting Schnauz first. No auto-updater
  or automatic crash-report upload is being introduced.


- Published 0.8.9 on 2026-10-23, source 0ec6e3c. Both DMGs, exact MPL sources,
  checksum file, all GitHub asset digests and all 1746 tagged source-archive files
  verified. ARM native / Intel Rosetta smoke and signature/installer checks passed.
  Apple Development signed, not notarized, early prerelease. Normal local 0.8.9
  is pending while PID 32977 remains alive; installed files were not replaced.
  Publication request complete. Future releases require a new request; preserve
  this tag/assets, all live sessions and the safe after-quit installation policy.


- Local 0.8.10: Oliver requested a full-width Schnauz-blue (#3478c5) project row,
  including the 8 pt top/bottom clearance. The selected project chip remains blue
  with white text and a white 1 pt outline. Preserve neutral inactive chips,
  header geometry, Search styling, horizontal scrolling and live project sessions.

- Local 0.8.11: each project (including Personal) owns a persistent file-ending
  exclusion list and independent On/Off flag in Sidebar.file_filter. Show retains
  its per-tab All/Folders/Files semantics. Filter: On / Off and Modify… sit beside
  Show, with matching segmented controls. Native Modify captures the project,
  supports normal text-edit shortcuts, validates literal comma-separated endings
  and saves atomically; Cancel preserves both list and flag. Never hide folders
  or alter files. Apply rules to local/loaded SFTP listings and recursive search,
  and synchronize live/parked tabs and browser windows without reconnecting or
  replacing terminals. Off retains the configured list. Existing profiles default
  to Off. Preserve the local blue-project-bar changes from 0.8.10.

- Local 0.8.12: file tabs have top/side outlines only. The strip owns the bottom
  divider; the active tab extends 1 pt over it with the content background so it
  joins the path area seamlessly. Keep the divider visible beneath inactive
  tabs/empty strip, and preserve tab top alignment and total strip height.

- Local 0.8.13: modern path-bar action icons use native SF Symbols Light
  (folder/doc badge plus, Share, bookmark, Trash, globe, Eye/Eye Off, Search).
  Retain view icon geometry; common vector strokes are 1.2/20 instead of 1.5/20.
  Keep all toolbar action routing, order, hit areas and state colours unchanged.

- Audio Preview reuses the embedded AVKit session with compact 88 pt native
  controls. Audio starts paused with sound enabled on explicit Play; video keeps
  its paused/muted default. Selection changes and closing Preview stop playback;
  native modals pause it. Test with silent generated WAV/AAC fixtures and isolated
  windows, never personal music or live sessions. Decoding follows macOS; show a
  readable error for unsupported/corrupt audio instead of an empty preview.

- Local 0.9.0: selected local/SFTP file and folder menus omit all New actions;
  creation remains on the background and existing toolbar/menu commands. The
  separate Quick Look viewer and Space shortcut are removed, while embedded
  preview thumbnail generation remains. Rename supports measured Unicode mouse
  positioning/drag/Shift selection and a visible, clipped caret in List, Icons
  and search; text selection must never start a file drag.
- A detected Nextcloud sync folder is distinct from Schnauz's own server login.
  Use “Enable sharing in Schnauz…” for the latter, never “Connect this folder”
  inside an already detected root. Keep maintenance in Settings. Cloud badges
  show local file data with a green check, online placeholders with a blue cloud,
  and explicit unknown status on metadata failure. A materialized directory
  does not imply all descendants are offline. The direct keep-downloaded bridge is implemented with runtime checks,
  and Oliver confirmed live offline/sharing behavior on 2026-10-30;
  issue #1 is resolved in published 0.9.22. Offer it only for verified
  nonlocal Nextcloud items, excluding kept/downloading items. Omit Finder handoff.
  Never import desktop credentials or scan descendants for a menu/status badge.

- Local 0.9.19 implements the approved Shuffle cleanup review. File copies and
  moves use `src/file_transfer.rs`: private staging, exclusive destination
  publication and cancellation during preparation. Never restore check-then-
  overwrite moves or delete a destination the job did not create. A filesystem
  without exclusive rename must use genuinely exclusive destination creation or
  report an error; never fall back to a check followed by overwriting rename.
- Folder constructors perform no reads. `reload_pane` loads both listing passes
  on owned workers, with tab identity/generation checks and cancellation.
  Parked terminal cwd updates mark listings dirty; load when their tab is shown.
  `src/workers.rs` isolates blocking processes from GPUI's shared work pool;
  cancellation/deadlines signal only the newly owned child, never a live SSH mux.
- Session autosaves use `src/persistence.rs` for ordered background commits.
  Explicit Library.save transactions share its generation order, and quit
  flushes pending saves. Do not bypass it with a second projects.json writer.
- The inherited Waterfall folder tree is retired, including its preference,
  prefetch, cached directory polling and UI. Keep user-created sidebar groups
  and every existing folder/reference. The floating command palette, its action
  panel and unused MenuStyle/opacity state are also removed. Search has one
  per-project history for its menu and keyboard, controlled by Search history;
  retain old keybinding identifiers and read the old preference as a fallback,
  but never import the shared palette_history.txt into individual projects.
- Remote preview cache keys include server/account/port/alias plus path, separate
  from local keys. Downloads use private, bounded temporary storage and literal
  SFTP arguments; cancellation rejects stale results. Bound/serialize PDF source
  reads as well as decoded caches. Never show a local text preview or local
  Information metadata for an identically named SSH file.
- `native/sftp/` and signed `sftpctl` handle native Keychain writes/reads with
  private stdin/stdout. No passwords in argv, environment, files or logs. Retain
  the read-only CLI fallback in legacy.rs for existing credentials created with
  that client's access policy; never rewrite/delete them merely to migrate.
  Password authentication sends an encoded private stdin line to the owned
  expect helper. Live CLI diagnostics accepting credentials or running scripts
  are removed; other diagnostics require ui-checks. Unknown options exit before
  opening the app. Current verification remains compile/sign/stage only; do not
  run test binaries, helpers, apps or live account operations during this chat.

- Local 0.9.20 adds native Devices → right-click → Eject for eligible external
  local volumes. volumectl.swift + native/volumes/Core.swift capture a persistent
  identity plus kernel mount ID and revalidate against fresh mounted volumes
  before NSWorkspace's normal unmount/eject operation. Never force unmount,
  eject the startup disk, terminate jobs, unmount network shares or perform an
  eject while opening/rendering a menu. Busy errors use the fixed footer.
  Coordinate requests across windows, release related previews and refresh
  Devices after completion. Preserve tabs' paths and live PTYs; an ejected
  location shows an unavailable/reconnect message. Native/Rust eject fixtures
  are compile-only under the current live-session restriction; no real drive
  may be ejected for QA without a separate scoped user request.


- Local 0.9.21: Devices listens to NSWorkspace mount/unmount/rename/wake events
  as well as its 30-second capacity timer. Notification callbacks only wake
  subscribers; listing stays on owned workers and coalesces overlapping requests.
  Drive right-click always offers Info, using cached capacity/path without a
  recursive scan. Show Eject and a native eject symbol only for eligible drives;
  omit both and any cannot-eject placeholder otherwise. Eject button clicks never
  navigate. Oliver requested continuing the public release with these changes.


- Local 0.9.22: Oliver paused release publication for flickering selected .ai
  previews. Keep the 0.9.21 commit/installers unpublished. Ignore attribute-only
  vnode events for regular files with unchanged dev/inode/size/mtime, so preview
  generator metadata cannot repeatedly invalidate the image. Preserve writes,
  replacements/removals and directory attribute refreshes. Verification remains
  compile/sign/stage only; live .ai confirmation is Oliver's manual workflow.


- Oliver resumed publication of 0.9.22 after the Illustrator preview fix.
  This supersedes the preview hold above. Use both verified signed architecture
  installers, exact MPL sources, checksums and English notes since 0.8.9.
  Retain compile/sign/package-only QA and preserve all live app/terminal sessions.


- Published 0.9.22 on 2026-10-30, source 2538957, at
  https://github.com/weiaoli2/Schnauz/releases/tag/v0.9.22. Both signed DMGs,
  exact MPL sources, checksums, all four GitHub digests and all 1767 tagged
  source-archive files verified. Compile/sign/package-only validation; no fresh
  app/helper/test execution or Intel hardware test. Apple Development signed,
  not notarized, early prerelease. Receipts: dist/0.9.22. Normal local 0.9.22 is
  staged; all live bundle files/windows/PTYs/SSH were preserved. Issues #1/#2
  received version-specific status comments and remain open for unfinished work.
  Request complete; future releases need a new request. Preserve all tags/assets.


- Oliver confirmed the Nextcloud controls were already verified in live use,
  including making files available offline where his Finder workflow fails.
  Issue #1 is closed as completed in published 0.9.22; the earlier pending-QA
  notes are superseded. Attribute this runtime evidence to Oliver's workflow,
  not to automated tests. Issue #2's preserved-search-results return remains
  unimplemented and open; ordinary recent-search recall is a separate feature.


- Local 0.9.23 diagnoses the SMB error-45 report with operation-specific footer
  errors and fixed, nonsecret diagnostic labels. Finder copying works and app
  reopening does not resolve the report; neither mount restoration nor exclusive
  rename is a confirmed cause yet. Keep no-overwrite transfer safety, existing
  mounts and credentials unchanged. Compile/sign/stage only; Oliver retries the
  live copy. No further release/commit is requested.


- Local 0.9.24 adds per-project Mark: On / Off / Settings… next to Show/Filter.
  Use filename-only numeric version suffixes (default vX, vX.X, vX.X.X), with
  spaces/underscores/dashes/dots, separate parent-folder/base/extension groups,
  and all highest ties marked when a group contains at least two files. Default
  Off, pastel yellow; native settings offers six text-marker colors. Background
  only the filename, retaining readable text, real selection and inline rename.
  Persist centrally in Sidebar.version_mark, with captured-project/stale-save
  guards. Cache winner sets on listing/rule/search-result changes; never scan
  subfolders, read files or mutate them for marks. List/Icons/search agree.
  Compile/sign/stage only; retain pending SMB diagnostics and all live sessions.


- Local 0.9.25 supersedes the single-rule Mark editor with ordered per-project
  rows: Files/Folders target, Newest version / Version match / Name ending,
  comma-separated patterns and one of exactly six pastel text-marker colors.
  Display six clickable round swatches with a selected ring for every row.
  Add/remove/reorder drafts, Save atomically, Cancel unchanged; first matching
  row wins overlaps. Retain old single-rule settings through read migration and
  preserve explicitly empty rows. Folder versions use full folder names. Cache
  per-path colors for List/Icons/search; no file changes or extra scans. Retain
  SMB diagnostics, native-modal guards and all live sessions. Compile/sign/stage
  only; do not execute apps/helpers/test binaries or publish without a request.


- Local 0.9.26: Todoist Add Task title/description carets follow the captured
  theme ink explicitly. The composer supplies its own native field editor for
  editable title/date/labels and reapplies caret color/blinking on editing focus.
  Preserve native fonts/selection, task drafts, routing and locked partial-save
  retry state. Compile/sign/stage only; no account or native-runtime QA launches.


- Local 0.9.27: Oliver's live 0.9.26 diagnostic log confirms SMB copy error 45
  at exclusive copy finalization, after staging data succeeds. On ENOTSUP/ENOSYS
  only, publish from the private stage using create_new file handles, exclusive
  mkdir/symlink and cancellable copying; never use ordinary overwriting rename.
  Same-volume moves lacking exclusive rename use copy then original-identity
  recheck/removal. Keep existing targets untouched. Track every created object's
  dev/inode; clean children first, remove only owned files and empty directories,
  preserving other clients' added/replaced objects. Fallback progress includes
  the second data pass. This fallback is not atomic whole-tree publication.
  Compile/sign/stage only; Oliver verifies the real SMB retry. Preserve mounts,
  live windows/PTYs, credentials and all 0.9.26 features. No commit/release.


- Oliver confirmed the SMB copy works in 0.9.27 and requested a new public
  GitHub release on 2026-10-31. This supersedes the no-release notes above.
  Publish 0.9.27 source, both signed architecture DMGs, exact MPL sources and
  checksums as an Apple Development-signed, unnotarized early prerelease.
  Include Mark rules and Todoist caret changes since 0.9.22. Retain compile/
  sign/package-only QA, attribute SMB live verification to Oliver, exclude
  website/video/test.csv and preserve every live session and existing tag/asset.
