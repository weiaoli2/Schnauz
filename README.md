<div align="center">
  <img src="assets/Schnauz-icon.png" width="128" alt="Schnauz — the schnauzer for your projects">
  <h1>Schnauz</h1>
  <p><strong>Your projects. Your files. Your terminal. One native Mac workspace.</strong></p>
  <p>Schnauz is a free, open-source file manager for macOS and a project-focused alternative to Finder.</p>
  <p>
    <a href="https://github.com/weiaoli2/Schnauz/releases"><img alt="Schnauz releases" src="https://img.shields.io/github/v/release/weiaoli2/Schnauz?include_prereleases&label=release&color=5263ef"></a>
    <img alt="macOS 14 or later" src="https://img.shields.io/badge/macOS-14%2B-black">
    <img alt="Apple Silicon and Intel" src="https://img.shields.io/badge/Mac-Apple%20Silicon%20%2B%20Intel-5263ef">
    <a href="LICENSE"><img alt="MIT license" src="https://img.shields.io/badge/license-MIT-blue"></a>
  </p>
  <p><a href="https://techguyzone.com/apps/">Website</a> · <a href="https://github.com/weiaoli2/Schnauz/releases">Releases</a> · <a href="docs/FEATURES.md">Feature guide</a> · <a href="https://github.com/weiaoli2/Schnauz/issues">Feedback</a></p>
</div>

## A better home for every project

Stop rebuilding your workspace every time you change what you are working on.
Schnauz gives each project its own favourites, saved tabs and layout, with a real
interactive terminal right beside your files. Switch projects and pick up where
you left off. Open another window and keep the same project in focus.

Browse, build, inspect and organise in one compact native app — with the tools
you need close at hand and expensive work started only when you ask for it.

| What you need | What Schnauz brings |
| --- | --- |
| **Your project workspace** | Each project remembers its own favourites, pinned files, tabs and layout. Common stays available across projects, and all windows share the active project. |
| **A seamless real terminal** | Each tab owns a persistent interactive PTY. Move tabs between projects without restarting their terminals. Select text to copy it automatically, with visible scrollback navigation. |
| **Paste a path and get there** | Paste a folder path into the file browser to open it. Paste a file path to open its folder and select the file. Text inputs and the terminal keep ordinary paste. |
| **Todoist in context** | See, add and complete tasks for the matching project. Detach one floating task window, keep just the first task visible, or dock it in another browser window. |
| **SSH in the current folder** | Start an interactive terminal directly in an SSH folder, or follow directories from `ssh host`. Existing authenticated transports can be reused without replacing their running sessions. |
| **Visible copy progress** | A compact footer bar shows local and SSH copy progress, the current file and cancellation, without covering your files. |
| **Smooth file dragging** | Hover a tab or folder to open it during a drag. Move normally, or hold Command to copy with a green + and a clear item count. |
| **Focused search** | Start in the current folder. Search filenames or text-file contents, combine words, and stop a scan whenever you want. Results stay in the file pane. |
| **Useful previews** | Select and copy text, inspect photos, and play muted-by-default video. Resize the preview and remember it with the folder view. |
| **Your own folder views** | List or Icons. Choose, resize and reorder columns. Save per-folder layouts or apply one to a folder tree. |
| **Cloud awareness** | Nextcloud/iCloud identity, cloud badges and optional direct Nextcloud sharing, links and supported file locks. |
| **Storage at a glance** | Drive free-space badges, configurable capacity colours and an on-demand Disk Usage map. |

## Built for speed, without the baggage

Schnauz uses **Rust and GPUI's native renderer**, with native macOS integrations
and no Electron runtime. File lists render visible rows; icons and previews use
bounded caches. There is no automatic recursive folder-size scan or startup
home-folder index. Search and Disk Usage run on demand and can be cancelled.

Folder preferences live in central settings, rather than scattering configuration
files through your projects. Todoist and Nextcloud are optional; their credentials
stay in macOS Keychain.

## Download for Mac — 0.9.22 preview

**macOS 14 Sonoma or later.** Choose the build for your Mac:

| Mac | Download |
| --- | --- |
| Apple Silicon — M1 and newer | [Apple Silicon DMG](https://github.com/weiaoli2/Schnauz/releases/download/v0.9.22/Schnauz-0.9.22-macos-arm64.dmg) |
| Intel | [Intel DMG](https://github.com/weiaoli2/Schnauz/releases/download/v0.9.22/Schnauz-0.9.22-macos-x86_64.dmg) |

For an update, finish any terminal/SSH work and **quit Schnauz before replacing it**.
Open the disk image and drag **Schnauz.app** to **Applications**. Checksums and
release notes accompany the [release](https://github.com/weiaoli2/Schnauz/releases/tag/v0.9.22).
Explore the app on [TechGuyZone](https://techguyzone.com/apps/).

This is an **early preview**, signed with an Apple Development certificate and
**not Apple-notarized**. See [security and verification](SECURITY.md) for signature,
checksum and first-launch information. The app explains macOS file-access
permissions; macOS and you control any grants. Optional startup update checks are off by default. Enable them in About Schnauz; checks use GitHub and offer the download for your installed architecture. Installation stays manual.

## Build it yourself

Install Rust stable and the full Xcode app, then:

```sh
git clone https://github.com/weiaoli2/Schnauz.git
cd Schnauz
SCHNAUZ_CODESIGN_IDENTITY=- ./build.sh
```

This creates a temporary ad-hoc development build. For a stable local identity,
use an Apple Development or Developer ID certificate instead. The normal build
automatically selects a suitable certificate, or you can choose one with
`SCHNAUZ_CODESIGN_IDENTITY="certificate name or SHA-1" ./build.sh`.

```sh
./test.sh
SCHNAUZ_TARGET=x86_64-apple-darwin ./build.sh
```

Cross-building requires the corresponding Rust target. See
[release and signing instructions](docs/RELEASING.md) and the
[development handoff](PROJECT.md). Test fixtures run offline without connecting
to your Todoist, Nextcloud or SSH accounts.

## Meet Cookie

<img src="assets/Cookie-portrait.png" width="160" align="right" alt="A painted portrait of Cookie, Oliver's schnauzer">

Schnauz is developed and designed by **Oliver Weber**, with implementation
assistance from **Codex**. Cookie is the real schnauzer behind the name — and
an enthusiastic supporter of good food.

If Schnauz makes your day easier, you can send an optional **Coffee for Cookie ·
US$2.50** through [PayPal](https://www.paypal.com/donate/?business=o%40freedom-net.de&currency_code=USD)
to **o@freedom-net.de**. You choose and confirm the amount on PayPal.
The app's About window has the same optional support link.

## Credits and license

Schnauz is an independent app based on **[Shuffle](https://github.com/WizenPainter/shuffle)
by Jaime Guzman**, whose original MIT license and copyright are preserved.
The terminal builds on gpui-terminal, Alacritty and portable-pty. Original authors
and dependency licenses are recorded in [CREDITS.txt](CREDITS.txt),
[LICENSE](LICENSE) and [THIRD_PARTY_NOTICES.txt](THIRD_PARTY_NOTICES.txt).

Ideas and bug reports are welcome through [GitHub Issues](https://github.com/weiaoli2/Schnauz/issues).
Please report security issues [privately](SECURITY.md).
