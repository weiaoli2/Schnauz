# Schnauz

Independent personal macOS app based on WizenPainter/shuffle, starting at
50a94466dadef2c483913bc2cd5c76999cda4dda. The local branch is `studio`;
`studio-starting-point` preserves the unmodified source. `upstream` is reference only.

- Rust + GPUI. Keep the native renderer and Mac integrations.
- `src/theme.rs`: themes; `src/studio.rs`: custom chrome and project UI;
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
  `STUDIO_BUILD_PROFILE=debug ./build.sh`. Rust stable and Xcode are required.
- Run `./test.sh` after project or file-operation changes. Use disposable
  folders for copy, move, rename and delete verification.
- Native UI QA can set `SCHNAUZ_CONFIG_DIR` and `SCHNAUZ_START_DIR`
  to disposable paths. Do not change HOME.
- `dev-env.sh` reuses the prepared cache under `../../work` and builds offline.
  Use `CARGO_NET_OFFLINE=false` when a new dependency must be downloaded.
- Show a working first version, gather feedback, then iterate. Default visual
  direction: quiet Mac-style light/dark palettes, indigo accent, clear spacing.
- The first preview was launched with `../../work/qa-profile`. The user began
  browsing in that window. Preserve its settings/workspaces if switching the
  running app to its normal profile; it may now contain user-created state.
- The live app now uses `~/Library/Application Support/Schnauz/`. Its projects
  and tabs were preserved from both preview profiles, including the user's two
  Desktop tabs and earlier Applications/Home tabs. Backups are in
  `../../work/profile-backups/`. Treat preview profiles as user data too; never
  replace live state with QA fixtures or leave a testing view active for the user.
- User feedback: no automatic recursive folder-size calculations while browsing.
  Main text is 12 logical px/pt, small labels 11, file rows 26 px. Keep it compact.
- Tabs have visible outlines and rounded top corners. Folder names use the same
  primary text color as files; their icons distinguish them.
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

- Projects replace the old workspace UI. Each owns its favorites, tagged items,
  Locations, Cloud, saved servers/network folders and autosaved local layout.
  New project sidebars start empty; Personal has its own profile. Recents and
  automatic cloud/volume discovery are removed. The Tag icon adds/removes local
  selection from Tagged Items; clicking a tagged file reveals it. Sidebar folders
  offer View in… with All projects first, then checkable individual projects;
  update destinations without switching. All affects existing projects only.
  Central projects.json stores profiles; migrate legacy servers once to Personal,
  preserve original legacy files and shared Keychain credentials. Migrate legacy workspaces
  and bookmarks without modifying their original files. Keep live PTYs when switching.
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
  It is labeled “Speicherbelegung…” and also has a button in the filter row.
  Like the preview, its panel starts below the path and filter bars. Selecting
  outside it closes the map; navigation from within its graphic keeps it open.

- Use Apple's actual square.and.arrow.up SF Symbol for Share. Cache rasterized
  symbols per color and preserve their aspect ratio.
- Photos and videos fill the preview width. Native AVKit clips start paused and
  muted, offer playback/seek/unmute, and stop when their preview closes. Clip native
  views below GPUI headers and inside the scroll viewport; hide during app modals.
- Dates use NSDateFormatter and current macOS locale, time zone and hour cycle.
  Never hard-code English or AM/PM; reuse a cached formatter rather than per-row I/O.
