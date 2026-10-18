<div align="center">
  <img src="assets/Schnauz-icon.png" width="128" alt="Schnauz — the schnauzer for your projects">
  <h1>Schnauz</h1>
  <p><strong>Your projects. Your files. Your terminal. One native Mac workspace.</strong></p>
  <p>A project-first Finder alternative, built for speed and made to feel at home on macOS.</p>
  <p>
    <a href="https://github.com/weiaoli2/Schnauz/releases"><img alt="Download Schnauz" src="https://img.shields.io/github/v/release/weiaoli2/Schnauz?include_prereleases&label=download&color=5263ef"></a>
    <img alt="macOS 14 or later" src="https://img.shields.io/badge/macOS-14%2B-black">
    <img alt="Apple Silicon and Intel" src="https://img.shields.io/badge/Mac-Apple%20Silicon%20%2B%20Intel-5263ef">
    <a href="LICENSE"><img alt="MIT license" src="https://img.shields.io/badge/license-MIT-blue"></a>
  </p>
  <p><a href="https://techguyzone.com/apps/">Website</a> · <a href="https://github.com/weiaoli2/Schnauz/releases">Download</a> · <a href="docs/FEATURES.md">Feature guide</a> · <a href="https://github.com/weiaoli2/Schnauz/issues">Feedback</a></p>
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
| **Project workspaces** | Project-specific favourites, saved tabs and split layouts. One active project shared across windows. |
| **A real terminal** | An interactive PTY with scrollback, selection and copying. Local folders sync at an idle prompt; running CLI apps keep control of their session. |
| **SSH that stays connected** | Follow remote Bash/Zsh directories through your terminal's authenticated SSH connection. Detach the browser without ending SSH. |
| **Focused search** | Start in the current folder. Search filenames or text-file contents, combine words, and stop a scan whenever you want. Results stay in the file pane. |
| **Useful previews** | Select and copy text, inspect photos, and play videos with sound muted by default. Resize the preview and remember it with the folder view. |
| **Your own folder views** | List or Icons. Choose, resize and reorder list columns. Save per-folder layouts or apply one to a folder tree. |
| **Cloud awareness** | Clear Nextcloud and iCloud breadcrumbs, cloud badges and optional direct Nextcloud sharing, links and supported file locks. |
| **Todoist, in context** | Optionally connect your account to see, add and complete tasks for the matching project. New Schnauz projects can reuse or create a same-named Todoist project. |
| **Storage at a glance** | Drive free-space badges, configurable capacity colours and an on-demand Disk Usage map. |

## Built for speed, without the baggage

Schnauz uses **Rust and GPUI's native renderer**, with native macOS integrations
and no Electron runtime. File lists render visible rows; icons and previews use
bounded caches. There is no automatic recursive folder-size scan or startup
home-folder index. Search and Disk Usage run on demand and can be cancelled.

Folder preferences live in central settings, rather than scattering configuration
files through your projects. Todoist and Nextcloud are optional; their credentials
stay in macOS Keychain.

## Download for Mac

**macOS 14 Sonoma or later.** Choose the build for your Mac:

| Mac | Download |
| --- | --- |
| Apple Silicon — M1, M2, M3, M4 and newer | [Apple Silicon release](https://github.com/weiaoli2/Schnauz/releases/download/v0.5.0-rc.1/Schnauz-0.5.0-rc.1-macos-arm64.dmg) |
| Intel | [Intel release](https://github.com/weiaoli2/Schnauz/releases/download/v0.5.0-rc.1/Schnauz-0.5.0-rc.1-macos-x86_64.dmg) |

Open the disk image and drag **Schnauz.app** to **Applications**.
Release checksums are included alongside the downloads.

**0.5.0-rc.1 is an early release candidate.** It is signed with the developer's
Apple Development certificate and is **not Apple-notarized**. macOS may block the
first launch; after trying to open it, use **System Settings → Privacy & Security →
Open Anyway** if you trust this download. See [security and verification](SECURITY.md).

The app explains macOS file-access permissions on first launch. Full Disk Access
is optional for ordinary folders, but may be needed for protected locations.
There is no automatic updater; new builds are published on the Releases page.

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
