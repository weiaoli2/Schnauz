# Shuffle Studio

Independent personal macOS app based on WizenPainter/shuffle, starting at
50a94466dadef2c483913bc2cd5c76999cda4dda. The local branch is `studio`;
`studio-starting-point` preserves the unmodified source. `upstream` is reference only.

- Rust + GPUI. Keep the native renderer and Mac integrations.
- `src/theme.rs`: themes; `src/studio.rs`: custom chrome and workspace UI;
  `src/workspaces.rs`: typed persistence; `src/main.rs`: existing file manager.
- Retain the original MIT LICENSE and attribution.
- App identity: Shuffle Studio, `local.shuffle-studio.app`. Never enable the
  original Shuffle self-updater or publish to its GitHub repository.
- For a normal build run `./build.sh`; for a faster development build use
  `STUDIO_BUILD_PROFILE=debug ./build.sh`. Rust stable and Xcode are required.
- Run `./test.sh` after workspace or file-operation changes. Use disposable
  folders for copy, move, rename and delete verification.
- Native UI QA can set `SHUFFLE_STUDIO_CONFIG_DIR`, `SHUFFLE_STUDIO_START_DIR`
  and `SHUFFLE_STUDIO_INDEX_ROOT` to disposable paths. Do not change HOME.
- `dev-env.sh` reuses the prepared cache under `../../work` and builds offline.
  Use `CARGO_NET_OFFLINE=false` when a new dependency must be downloaded.
- Show a working first version, gather feedback, then iterate. Default visual
  direction: quiet Mac-style light/dark palettes, indigo accent, clear spacing.
- The first preview was launched with `../../work/qa-profile`. The user began
  browsing in that window. Preserve its settings/workspaces if switching the
  running app to its normal profile; it may now contain user-created state.
- User feedback: no automatic recursive folder-size calculations while browsing.
  Main text is 12 logical px/pt, small labels 11, file rows 26 px. Keep it compact.
- All/Folders/Files visibility is independent for each tab and persists in sessions
  and workspaces. Keep every view, selection and keyboard navigation consistent
  with the visible results; tint the top bar while a type filter is active.
