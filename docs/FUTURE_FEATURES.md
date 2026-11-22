# Possible future features

Ideas in this document are undecided, not approved implementation work.

## Undo per project — under consideration

Discussed with Oliver on 2026-11-12. Give each project, including Personal, its
own Undo history so Cmd+Z only reverses actions started in the current project.
Today each window has one Undo stack spanning project switches.

This appears to be a manageable change, but Oliver has not decided to proceed.
Before implementation, settle whether project history should be shared across
windows and whether it should survive restarting the app. Background operations
must record Undo against their originating project even after a project switch.
Preserve existing conflict checks and avoid undoing an action against files that
have subsequently changed. This idea does not expand which operations support
Undo and does not authorize implementation.

## Schnauz for Linux — deferred

Discussed with Oliver on 2026-11-22. A Linux port is feasible and is on the
list for later. Start with file browsing, projects, tabs and the integrated
terminal, then add Todoist. Choose the first Linux distribution and desktop
session when the work is scheduled.

- Keep the shared Rust file and project logic and the GPUI interface where
  possible. GPUI provides Linux backends for Wayland and X11. Validate the
  existing terminal component and its platform-specific input on Linux.
  [GPUI on Linux](https://zed.dev/blog/zed-decoded-linux-when)
- Move AppKit and macOS-specific dependencies behind platform boundaries.
  Provide Linux implementations for window controls, menus, drag and drop,
  clipboard/file opening, icons, file watching and previews. Review Finder
  tags, iCloud and File Provider features separately. Add Linux packaging
  independently of the signed macOS bundles.
- Schnauz already uses the Todoist API directly. The Linux integration would
  need a Linux credential store and a replacement for the AppKit task composer.
  Keep project linking, captured task destinations and on-demand refresh.
  Todoist also offers a Linux desktop app as a Snap or AppImage. Check desktop
  task links on the chosen distribution and keep the web link available.
  [Todoist for Linux](https://www.todoist.com/downloads/linux)

## Desktop appearance presets — under consideration

Oliver suggested offering macOS-, Linux-, and Windows-inspired Schnauz
styles on every supported platform. A Mac user could choose the Linux-inspired
look, and a future Linux user could choose the macOS-inspired look. Windows-
inspired styling could also be available on both. The presets can start on
macOS independently of the Linux port.

- Consider a **Desktop style** control in **Settings → Customization**, with
  **System default**, **macOS-inspired**, **Linux-inspired**, and
  **Windows-inspired** options. Save it independently of light/dark mode.
- Start with the folder icons and their colours. A yellow or amber folder is
  one option for the Linux-inspired preset. Linux desktop icon themes vary,
  so the system default on Linux can follow the active desktop icon theme.
  [freedesktop.org Icon Theme Specification](https://specifications.freedesktop.org/icon-theme/latest/)
- Consider matching toolbar icons, accents, and tab styling once the folder
  presets are settled. Each preset changes Schnauz's appearance while
  preserving host-platform shortcuts, window controls, file operations,
  integrations, and the fixed Schnauz app icon.

## macOS, Linux, and Windows releases — longer-term goal

Oliver wants Schnauz to eventually ship on all three platforms from the same
codebase. Continue the macOS release track and evaluate the Linux and Windows
ports separately. The appearance presets are independent of port readiness.

- Share the Rust file browser, project model, and GPUI interface where practical.
  Before starting Windows work, check the pinned GPUI and terminal versions
  against Windows and identify which native integrations need replacements.
- Verify file operations, terminal behaviour, Todoist credential storage and
  packaging on each platform before deciding what to include in its first
  release. Each platform can ship when its own build is ready.
