# Schnauz

Independent personal macOS app based on WizenPainter/shuffle, starting at
50a94466dadef2c483913bc2cd5c76999cda4dda. The local branch is `studio`;
`studio-starting-point` preserves the unmodified source. `upstream` is reference only.

- Rust + GPUI. Keep the native renderer and Mac integrations.
- `src/theme.rs`: themes; `src/studio.rs`: custom chrome and project UI;
  `src/projects.rs`: typed persistence; `src/main.rs`: existing file manager.
- Retain the original MIT LICENSE and attribution.
- Schnauz credits: design and product direction by Oliver Weber, implementation
  of Schnauz adaptations by Codex; original Shuffle code by Jaime Guzman.
  Keep `CREDITS.txt`, the source header and README consistent, and ship the
  credits alongside the unchanged upstream and third-party license notices.
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

- Projects replace the old workspace UI. Each owns its favorites and autosaved
  local layout; Personal has separate favorites/layout. Migrate legacy workspaces
  and bookmarks without modifying their original files. Keep live PTYs when switching.
- Hidden items (dot names and macOS UF_HIDDEN) are hidden by default; the per-tab eye
  toggle applies in every view and combines with type/text filters.
- Hide the inspector without an actual previewable single selection. Show text as
  readable native text in a scrollable panel, with bounded reads for large files.
- The preview's left edge resizes both text and visual previews. Remember width
  after the drag, retain room for browsing, and double-click to reset to 420 pt.
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
  it stops the scan, keeps the results, and shows a short status-bar notice that
  results may be incomplete. Clear that notice when a new search starts.
- Inspect… is an explicit folder-size scan, available on folder, background and
  sidebar menus. Keep automatic browsing calculations off. Preserve the right-hand
  map while navigating from its file tiles or folder headings; never open a file
  merely by clicking its tile. Cancel replaced/closed scans and flag partial maps.
