//! Schnauz — a personal Mac file manager based on Shuffle.
//!
//! Developer, design and product direction: Oliver Weber.
//! Implementation assistance for Schnauz adaptations: Codex.
//! Original codebase: Shuffle by Jaime Guzman (WizenPainter), MIT license.
//! Original copyright: Copyright (c) 2026 Jaime Guzman.
//! See LICENSE and CREDITS.txt for attribution and license notices.
//!
//! Settings, projects and saved tabs live in ~/Library/Application Support/Schnauz/.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::OsString;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use gpui::{
    actions, div, ease_out_quint, img, point, prelude::*, px, relative, rgb, rgba, size,
    uniform_list, Animation, AnimationExt, AnyElement, App,
    Application, Bounds, ClickEvent, ClipboardItem, Context, CursorStyle, ElementId, ExternalPaths, FocusHandle, FontWeight, ImageSource,
    KeyBinding, KeyDownEvent, Menu, MenuItem, MouseButton, MouseDownEvent, MouseMoveEvent, ObjectFit,
    PathPromptOptions, Rgba,
    RenderImage, ScrollHandle, ScrollStrategy, ScrollWheelEvent, SharedString, TitlebarOptions,
    UniformListScrollHandle, Window, WindowBounds, WindowOptions,
};
use objc2::runtime::NSObjectProtocol;
use objc2::{define_class, AllocAnyThread, MainThreadOnly};
use objc2_app_kit::{
    NSBitmapImageRep, NSCompositingOperation, NSDeviceRGBColorSpace, NSDraggingContext,
    NSDraggingSession, NSDraggingSource, NSDragOperation, NSGraphicsContext, NSImage,
    NSPDFImageRep, NSWorkspace,
};
use objc2_foundation::{NSData, NSFileManager, NSObject, NSString, NSURL};

const UI_FONT_SIZE: f32 = 12.0;
const UI_SMALL_FONT_SIZE: f32 = 11.0;

// Menu-bar actions. OpenSettings/Quit are app-level; the rest are dispatched to
// the focused explorer window and handled in `Shuffle::render`'s root.
actions!(shuffle, [OpenAbout, OpenSettings, Quit]);
actions!(
    shuffle,
    [
        NewTab,
        NewFolder,
        CloseTab,
        MoveToTrash,
        ViewList,
        ViewIcons,
        ToggleSidebar,
        GoBack,
        GoForward,
        GoHome,
        GoApplications,
        GoComputer,
        FocusSearch,
        NewProject,
        ToggleStudioSplit,
        ToggleInspector,
    ]
);

// ----- theming ---------------------------------------------------------------

mod theme;
use theme::*;
mod projects;
mod project_sidebar;
mod text_preview;
mod new_text_file;
mod entry_filter;
use entry_filter::EntryFilter;
mod studio;
mod clipboard_path;
mod studio_terminal;
mod ssh_sync;
mod remote_listing;
use remote_listing::parse as parse_sftp_ls;
mod file_search;
mod search_ui;
mod space_map;
mod space_ui;
mod cache;
mod folder_watch;
mod folder_views;
mod list_ui;
mod native_menus;
mod toolbar_icons;
use folder_views::{Column, ListLayout, ViewMode};
use toolbar_icons::{ui_icon, Icon};
mod previews;
mod preview_panel;
mod mascot;
mod settings;
mod about;
mod video_preview;
mod system_dates;
mod volumes;
mod cloud_locations;
mod cloud_actions;
use settings::*;
use previews::*;
use cache::Cache;

// ----- feature preferences (the General tab toggles) -------------------------

/// User-toggleable features.
#[derive(Clone, Copy)]
struct Prefs {
    /// Show a file preview in the inspector when a file is selected.
    preview: bool,
    /// Show ‹ › page arrows on multi-page PDF previews in the inspector.
    preview_pages: bool,
    /// Preferred inspector width in logical pixels, retained across launches.
    preview_width: u16,
    /// Show file information in the inspector when a file is selected.
    info: bool,
    /// Show the leading ".." row that goes up one level.
    show_parent: bool,
    /// Collapse the left sidebar to its expand button.
    sidebar_collapsed: bool,
    /// Percentage used: yellow begins at warning, red above critical.
    drive_warning: u8,
    drive_critical: u8,
    /// How many Recents to show in the sidebar (0 hides the section).
    /// Give the command palette (Cmd+P) its own Up/Down query history.
    palette_history: bool,
    /// Enable custom sidebar "groups" of files/folders.
    groups_enabled: bool,
    /// Show the always-on "Filter" pill in the bottom-right (/ still works).
    show_filter_button: bool,
    /// Show a live frame-rate meter in the top-right (debug; costs some CPU).
    show_fps: bool,
    /// Run user shell-script actions from the Scripts folder in menus.
    script_actions: bool,
    /// SFTP auth: true = delegate to the user's ~/.ssh (config/keys/known_hosts);
    /// false = use the explicit key path saved per server.
    ssh_use_system: bool,
    /// Whether the user has made the first-run SFTP auth choice yet.
    ssh_configured: bool,
    /// Show the expandable "waterfall" folder tree at the bottom of the sidebar.
    waterfall: bool,
    /// Command palette / search window opacity, as a percent (20–100). Lower is
    /// more see-through; the default is nearly opaque so it doesn't read as a
    /// confusing floating pane over the explorer.
    palette_opacity: u8,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            preview: true,
            preview_pages: true,
            preview_width: 420,
            info: true,
            show_parent: false,
            sidebar_collapsed: false,
            drive_warning: 80,
            drive_critical: 90,
            palette_history: false,
            groups_enabled: false,
            show_filter_button: true,
            show_fps: false,
            script_actions: false,
            ssh_use_system: true,
            ssh_configured: false,
            waterfall: false,
            palette_opacity: 92,
        }
    }
}

#[derive(Clone, Copy)]
struct PrefsGlobal(Prefs);
impl gpui::Global for PrefsGlobal {}

thread_local! {
    static ACTIVE_PREFS: RefCell<Prefs> = const { RefCell::new(Prefs {
        preview: true,
        preview_pages: true,
        preview_width: 420,
        info: true,
        show_parent: false,
        sidebar_collapsed: false,
        drive_warning: 80,
        drive_critical: 90,
        palette_history: false,
        groups_enabled: false,
        show_filter_button: true,
        show_fps: false,
        script_actions: false,
        ssh_use_system: true,
        ssh_configured: false,
        waterfall: false,
        palette_opacity: 92,
    }) };
}

/// The active preferences. Read this anywhere in render code.
fn prefs() -> Prefs {
    ACTIVE_PREFS.with(|p| *p.borrow())
}

/// The command palette's background alpha (0–255) from the opacity pref.
fn palette_alpha() -> u32 {
    let pct = prefs().palette_opacity.clamp(20, 100) as u32;
    pct * 255 / 100
}

fn set_active_prefs(p: Prefs) {
    ACTIVE_PREFS.with(|c| *c.borrow_mut() = p);
}

/// Apply prefs everywhere: update the render copy, persist, store in the global.
fn apply_prefs(p: Prefs, cx: &mut App) {
    set_active_prefs(p);
    save_prefs(&p);
    cx.set_global(PrefsGlobal(p));
    cx.refresh_windows();
}

// ----- icon packs ------------------------------------------------------------

/// The active icon pack: a folder of images named `folder.png`, `file.png`, and
/// per-extension (e.g. `pdf.png`, `png.png`) that override macOS icons.
#[derive(Clone)]
struct IconPackGlobal(Option<PathBuf>);
impl gpui::Global for IconPackGlobal {}

thread_local! {
    static ACTIVE_ICON_PACK: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

fn icon_pack() -> Option<PathBuf> {
    ACTIVE_ICON_PACK.with(|p| p.borrow().clone())
}

fn set_active_icon_pack(p: Option<PathBuf>) {
    ACTIVE_ICON_PACK.with(|c| *c.borrow_mut() = p);
}

/// Apply an icon pack (or `None` to revert to macOS icons): persist, rebuild
/// icons, and notify the explorer window.
fn apply_icon_pack(p: Option<PathBuf>, cx: &mut App) {
    set_active_icon_pack(p.clone());
    save_icon_pack(&p);
    cx.set_global(IconPackGlobal(p));
    cx.refresh_windows();
}

// ----- menu style (right-click / dropdown menu appearance) -------------------

/// Customizable look of pop-up menus (right-click menu, dropdowns).
#[derive(Clone, Copy, PartialEq)]
struct MenuStyle {
    /// Menu background color.
    bg: u32,
    /// Menu text ("letter") color.
    text: u32,
    /// Background opacity, 0..=100 percent.
    opacity: u8,
    /// Menu font size in pixels.
    font_px: f32,
    /// True once the user picks explicit menu colors. While false, `bg`/`text`
    /// follow the active theme (so switching to a light theme lightens menus
    /// too); opacity and size are always user-controlled.
    custom: bool,
}

impl Default for MenuStyle {
    fn default() -> Self {
        let d = Theme::default();
        MenuStyle { bg: d.surface, text: d.text, opacity: 100, font_px: UI_FONT_SIZE, custom: false }
    }
}

impl MenuStyle {
    /// Re-derive `bg`/`text` from the theme unless the user set explicit colors.
    fn follow_theme(&mut self, t: &Theme) {
        if !self.custom {
            self.bg = t.surface;
            self.text = t.text;
        }
    }
}


#[derive(Clone, Copy)]
struct MenuStyleGlobal(MenuStyle);
impl gpui::Global for MenuStyleGlobal {}

thread_local! {
    static ACTIVE_MENU: RefCell<MenuStyle> = RefCell::new(MenuStyle::default());
}

fn menu_style() -> MenuStyle {
    ACTIVE_MENU.with(|m| *m.borrow())
}

fn set_active_menu(m: MenuStyle) {
    ACTIVE_MENU.with(|c| *c.borrow_mut() = m);
}

/// Apply a menu style everywhere: render copy, persist, global, repaint.
fn apply_menu_style(m: MenuStyle, cx: &mut App) {
    set_active_menu(m);
    save_menu_style(&m);
    cx.set_global(MenuStyleGlobal(m));
    cx.refresh_windows();
}

// ----- app icon background (the Dock icon) -----------------------------------

/// The default app icon (the exact bundle icon: blue folder on a light rounded
/// square with the standard macOS margin). We recolor only its background so a
/// customized icon keeps the identical size, shape, margin, and logo.
const ICON_BASE_PNG: &[u8] = include_bytes!("../icon_base.png");

/// How the app icon's background is drawn behind the logo.
#[derive(Clone, PartialEq)]
enum IconBg {
    /// Keep the built-in bundle icon (light) as-is.
    Default,
    /// Recolor the background to a solid color.
    Color(u32),
    /// Fill the background with a user-supplied image (copied to the config dir).
    Image(PathBuf),
}

thread_local! {
    static ACTIVE_ICON_BG: RefCell<IconBg> = const { RefCell::new(IconBg::Default) };
    /// Cached decoded preview icon: (config it was built for, decoded image).
    static ICON_PREVIEW: RefCell<Option<(IconBg, Option<Arc<RenderImage>>)>> =
        const { RefCell::new(None) };
}

fn icon_bg() -> IconBg {
    ACTIVE_ICON_BG.with(|b| b.borrow().clone())
}

fn set_active_icon_bg(b: IconBg) {
    ACTIVE_ICON_BG.with(|c| *c.borrow_mut() = b);
}

/// The decoded icon for the settings preview (recolored base, cached per config).
fn preview_icon_render() -> Option<Arc<RenderImage>> {
    let bg = icon_bg();
    ICON_PREVIEW.with(|c| {
        {
            let cache = c.borrow();
            if let Some((cached, render)) = cache.as_ref() {
                if *cached == bg {
                    return render.clone();
                }
            }
        }
        let render = match &bg {
            IconBg::Default => decode_icon(ICON_BASE_PNG),
            _ => compose_icon_png(&bg).and_then(|png| decode_icon(&png)),
        };
        *c.borrow_mut() = Some((bg, render.clone()));
        render
    })
}

/// Apply the icon background: update the render copy, persist, redraw the Dock
/// icon, and repaint (so the Settings preview updates).
fn apply_icon_bg(bg: IconBg, cx: &mut App) {
    set_active_icon_bg(bg.clone());
    save_icon_bg(&bg);
    refresh_dock_icon(&bg);
    cx.refresh_windows();
}

/// Recolor the base icon per `bg` and hand it to the Dock. `Default` leaves the
/// bundle's own icon untouched.
fn refresh_dock_icon(bg: &IconBg) {
    if matches!(bg, IconBg::Default) {
        return; // keep the built-in AppIcon.icns
    }
    if let Some(png) = compose_icon_png(bg) {
        set_dock_icon(&png);
    }
}

/// Whether a base-icon pixel belongs to the (light, low-saturation) background
/// rather than the (saturated blue) folder logo.
fn is_icon_background(r: u8, g: u8, b: u8) -> bool {
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    mx >= 200 && (mx - mn) <= 26
}

/// Build the customized icon PNG by recoloring ONLY the base icon's background,
/// preserving its exact shape, margin, corner radius, logo, and shading.
fn compose_icon_png(bg: &IconBg) -> Option<Vec<u8>> {
    use image::imageops;
    let mut base = image::load_from_memory(ICON_BASE_PNG).ok()?.to_rgba8();
    let (w, h) = base.dimensions();

    // For an image background, cover-fit it to the icon so each background pixel
    // has a source color.
    let bg_img = match bg {
        IconBg::Image(path) => {
            let img = image::open(path).ok()?.to_rgba8();
            let (iw, ih) = img.dimensions();
            let scale = (w as f32 / iw as f32).max(h as f32 / ih as f32);
            let (nw, nh) = ((iw as f32 * scale) as u32, (ih as f32 * scale) as u32);
            Some(imageops::resize(&img, nw.max(1), nh.max(1), imageops::FilterType::Lanczos3))
        }
        _ => None,
    };
    let color = match bg {
        IconBg::Color(c) => Some([(c >> 16) as u8, (c >> 8) as u8, *c as u8]),
        _ => None,
    };

    for (x, y, px) in base.enumerate_pixels_mut() {
        let [r, g, b, a] = px.0;
        if a == 0 || !is_icon_background(r, g, b) {
            continue; // transparent margin or the logo → leave untouched
        }
        if let Some(img) = &bg_img {
            let (bw, bh) = img.dimensions();
            let sp = img.get_pixel(x.min(bw - 1), y.min(bh - 1)).0;
            px.0 = [sp[0], sp[1], sp[2], a];
        } else if let Some([cr, cg, cb]) = color {
            // Preserve the base's subtle gradient by scaling the color by this
            // pixel's brightness (near-white → full color, slightly darker →
            // slightly darker color).
            let lum = r.max(g).max(b) as f32 / 255.0;
            px.0 = [
                (cr as f32 * lum) as u8,
                (cg as f32 * lum) as u8,
                (cb as f32 * lum) as u8,
                a,
            ];
        }
    }

    let mut out = Vec::new();
    base.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .ok()?;
    Some(out)
}

/// Hand a PNG to the running app as its Dock / cmd-tab icon.
fn set_dock_icon(png: &[u8]) {
    use objc2::AllocAnyThread;
    use objc2_app_kit::{NSApplication, NSImage};
    let Some(mtm) = objc2::MainThreadMarker::new() else {
        return;
    };
    let data = objc2_foundation::NSData::with_bytes(png);
    let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) else {
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    unsafe { app.setApplicationIconImage(Some(&image)) };
}

/// The pack image overriding the icon for `key` (a file extension, FOLDER_KEY,
/// or FILE_KEY), if the active pack provides one.
fn pack_icon_path(key: &str) -> Option<PathBuf> {
    // An application always keeps its own native icon, including with icon packs.
    if key.starts_with(APP_ICON_PREFIX) {
        return None;
    }
    let pack = icon_pack()?;
    let name = if key == FOLDER_KEY {
        "folder"
    } else if key == FILE_KEY {
        "file"
    } else if let Some(rest) = key.strip_prefix("fav:") {
        // Favorite/location icons (e.g. "fav:documents") map to <name>.png so a
        // pack can override sidebar icons just like file/folder icons.
        rest
    } else {
        key
    };
    for ext in ["png", "jpg", "jpeg", "gif", "tiff", "bmp", "webp"] {
        let p = pack.join(format!("{name}.{ext}"));
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// Decode an image file from disk into a 128px GPUI icon (for pack icons).
fn decode_image_file(path: &Path) -> Option<Arc<RenderImage>> {
    let bytes = fs::read(path).ok()?;
    decode_icon(&bytes)
}

fn clear_icon_cache() {
    ICON_EPOCH.with(|e|e.set(e.get().wrapping_add(1)));
    ICON_PENDING.with(|p|p.borrow_mut().clear());
    ICON_REQUESTS.with(|q|q.borrow_mut().clear());
    ICON_CACHE.with(|c| c.borrow_mut().clear());
}

// ----- keybindings -----------------------------------------------------------

/// Every rebindable action.
#[derive(Clone, Copy, PartialEq)]
enum KeyAction {
    CommandPalette,
    NewTab,
    CloseTab,
    Find,
    SelectAll,
    NewFile,
    NewFolder,
    Rename,
    CopyPath,
    Duplicate,
    MakeAlias,
    Compress,
    MoveToTrash,
    RevealInFinder,
    Open,
    Refresh,
    SwitchPane,
    Share,
    GetInfo,
    BatchRename,
    Undo,
    Paste,
    Copy,
    // Command-palette (Cmd+P) text editing. These act only while the palette is
    // open, so they're excluded from the normal (global) key dispatch.
    PaletteCursorStart,
    PaletteCursorEnd,
    PaletteSelectAll,
    PaletteDeleteToStart,
    PaletteHistoryPrev,
    PaletteHistoryNext,
}

impl KeyAction {
    const ALL: &'static [KeyAction] = &[
        KeyAction::CommandPalette,
        KeyAction::NewTab,
        KeyAction::CloseTab,
        KeyAction::Find,
        KeyAction::SelectAll,
        KeyAction::NewFile,
        KeyAction::NewFolder,
        KeyAction::Rename,
        KeyAction::CopyPath,
        KeyAction::Duplicate,
        KeyAction::MakeAlias,
        KeyAction::Compress,
        KeyAction::MoveToTrash,
        KeyAction::RevealInFinder,
        KeyAction::Open,
        KeyAction::Refresh,
        KeyAction::SwitchPane,
        KeyAction::Share,
        KeyAction::GetInfo,
        KeyAction::BatchRename,
        KeyAction::Undo,
        KeyAction::Paste,
        KeyAction::Copy,
        KeyAction::PaletteCursorStart,
        KeyAction::PaletteCursorEnd,
        KeyAction::PaletteSelectAll,
        KeyAction::PaletteDeleteToStart,
        KeyAction::PaletteHistoryPrev,
        KeyAction::PaletteHistoryNext,
    ];

    /// Palette actions apply only inside the Cmd+P palette (skipped by the
    /// normal global key dispatch).
    fn is_palette(self) -> bool {
        matches!(
            self,
            KeyAction::PaletteCursorStart
                | KeyAction::PaletteCursorEnd
                | KeyAction::PaletteSelectAll
                | KeyAction::PaletteDeleteToStart
                | KeyAction::PaletteHistoryPrev
                | KeyAction::PaletteHistoryNext
        )
    }

    fn id(self) -> usize {
        Self::ALL.iter().position(|a| *a == self).unwrap()
    }

    /// Stable key used for persistence.
    fn key(self) -> &'static str {
        match self {
            KeyAction::CommandPalette => "command_palette",
            KeyAction::NewTab => "new_tab",
            KeyAction::CloseTab => "close_tab",
            KeyAction::Find => "find",
            KeyAction::SelectAll => "select_all",
            KeyAction::NewFile => "new_file",
            KeyAction::NewFolder => "new_folder",
            KeyAction::Rename => "rename",
            KeyAction::CopyPath => "copy_path",
            KeyAction::Duplicate => "duplicate",
            KeyAction::MakeAlias => "make_alias",
            KeyAction::Compress => "compress",
            KeyAction::MoveToTrash => "move_to_trash",
            KeyAction::RevealInFinder => "reveal_in_finder",
            KeyAction::Open => "open",
            KeyAction::Refresh => "refresh",
            KeyAction::SwitchPane => "switch_pane",
            KeyAction::Share => "share",
            KeyAction::GetInfo => "get_info",
            KeyAction::BatchRename => "batch_rename",
            KeyAction::Undo => "undo",
            KeyAction::Paste => "paste",
            KeyAction::Copy => "copy",
            KeyAction::PaletteCursorStart => "palette_cursor_start",
            KeyAction::PaletteCursorEnd => "palette_cursor_end",
            KeyAction::PaletteSelectAll => "palette_select_all",
            KeyAction::PaletteDeleteToStart => "palette_delete_to_start",
            KeyAction::PaletteHistoryPrev => "palette_history_prev",
            KeyAction::PaletteHistoryNext => "palette_history_next",
        }
    }

    /// Human label shown in Settings.
    fn label(self) -> &'static str {
        match self {
            KeyAction::CommandPalette => "File search",
            KeyAction::NewTab => "New tab",
            KeyAction::CloseTab => "Close tab",
            KeyAction::Find => "Filter current folder",
            KeyAction::SelectAll => "Select all",
            KeyAction::NewFile => "New file",
            KeyAction::NewFolder => "New folder",
            KeyAction::Rename => "Rename",
            KeyAction::CopyPath => "Copy path",
            KeyAction::Duplicate => "Duplicate",
            KeyAction::MakeAlias => "Make alias",
            KeyAction::Compress => "Compress",
            KeyAction::MoveToTrash => "Move to Trash",
            KeyAction::RevealInFinder => "Reveal in Finder",
            KeyAction::Open => "Open",
            KeyAction::Refresh => "Refresh (syncs cloud folders)",
            KeyAction::SwitchPane => "Switch split pane",
            KeyAction::Share => "Share…",
            KeyAction::GetInfo => "Get Info",
            KeyAction::BatchRename => "Rename selected items…",
            KeyAction::Undo => "Undo last file operation",
            KeyAction::Paste => "Paste path or files",
            KeyAction::Copy => "Copy files",
            KeyAction::PaletteCursorStart => "Palette: cursor to start",
            KeyAction::PaletteCursorEnd => "Palette: cursor to end",
            KeyAction::PaletteSelectAll => "Palette: select all",
            KeyAction::PaletteDeleteToStart => "Palette: delete to start",
            KeyAction::PaletteHistoryPrev => "Palette: previous history",
            KeyAction::PaletteHistoryNext => "Palette: next history",
        }
    }

    /// Default keystroke (some actions are unbound by default).
    fn default_binding(self) -> Option<&'static str> {
        match self {
            KeyAction::CommandPalette => Some("cmd-p"),
            KeyAction::NewTab => Some("cmd-t"),
            KeyAction::CloseTab => Some("cmd-w"),
            KeyAction::Find => Some("/"),
            KeyAction::SelectAll => Some("cmd-a"),
            KeyAction::Refresh => Some("cmd-r"),
            KeyAction::SwitchPane => Some("ctrl-tab"),
            KeyAction::Share => Some("cmd-alt-s"),
            KeyAction::GetInfo => Some("cmd-i"),
            KeyAction::BatchRename => Some("cmd-shift-r"),
            KeyAction::Undo => Some("cmd-z"),
            KeyAction::Paste => Some("cmd-v"),
            KeyAction::Copy => Some("cmd-c"),
            KeyAction::PaletteCursorStart => Some("cmd-left"),
            KeyAction::PaletteCursorEnd => Some("cmd-right"),
            KeyAction::PaletteSelectAll => Some("cmd-a"),
            KeyAction::PaletteDeleteToStart => Some("ctrl-u"),
            KeyAction::PaletteHistoryPrev => Some("up"),
            KeyAction::PaletteHistoryNext => Some("down"),
            _ => None,
        }
    }
}

/// The active key bindings (one optional keystroke per action).
#[derive(Clone)]
struct Keymap {
    binds: Vec<Option<String>>,
}

impl Keymap {
    fn defaults() -> Self {
        Keymap {
            binds: KeyAction::ALL
                .iter()
                .map(|a| a.default_binding().map(String::from))
                .collect(),
        }
    }
    fn get(&self, a: KeyAction) -> Option<&str> {
        self.binds[a.id()].as_deref()
    }
    fn set(&mut self, a: KeyAction, b: Option<String>) {
        self.binds[a.id()] = b;
    }
    /// The action bound to keystroke `ks`, if any.
    fn action_for(&self, ks: &str) -> Option<KeyAction> {
        KeyAction::ALL.iter().copied().find(|a| self.get(*a) == Some(ks))
    }
}

#[derive(Clone)]
struct KeymapGlobal(Keymap);
impl gpui::Global for KeymapGlobal {}

thread_local! {
    static ACTIVE_KEYMAP: RefCell<Keymap> = RefCell::new(Keymap::defaults());
}

fn keymap() -> Keymap {
    ACTIVE_KEYMAP.with(|k| k.borrow().clone())
}

fn set_active_keymap(k: Keymap) {
    ACTIVE_KEYMAP.with(|c| *c.borrow_mut() = k);
}

fn apply_keymap(k: Keymap, cx: &mut App) {
    set_active_keymap(k.clone());
    save_keymap(&k);
    cx.set_global(KeymapGlobal(k));
    cx.refresh_windows();
}

/// Whether `c` counts as part of a "word" for Option+Arrow navigation.
/// Only alphanumerics are word characters, so separators like `_`, `-`, `.`,
/// `/` and spaces are boundaries — e.g. in "helix_vault" a word jump lands on
/// "vault".
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric()
}

/// Char index of the word boundary to the LEFT of `cursor` in `s`: skip any
/// separators immediately left of the cursor, then skip the word before them.
fn prev_word_boundary(s: &str, cursor: usize) -> usize {
    let chars: Vec<char> = s.chars().collect();
    let mut i = cursor.min(chars.len());
    while i > 0 && !is_word_char(chars[i - 1]) {
        i -= 1;
    }
    while i > 0 && is_word_char(chars[i - 1]) {
        i -= 1;
    }
    i
}

/// Char index of the word boundary to the RIGHT of `cursor` in `s`: skip any
/// separators at the cursor, then skip the following word.
fn next_word_boundary(s: &str, cursor: usize) -> usize {
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut i = cursor.min(len);
    while i < len && !is_word_char(chars[i]) {
        i += 1;
    }
    while i < len && is_word_char(chars[i]) {
        i += 1;
    }
    i
}

/// Byte offset of char index `i` in `s` (or `s.len()` if past the end).
fn char_byte(s: &str, i: usize) -> usize {
    s.char_indices().nth(i).map(|(b, _)| b).unwrap_or(s.len())
}

/// Canonical string for a keystroke, e.g. "cmd-shift-p" or "/".
fn canon_keystroke(ks: &gpui::Keystroke) -> String {
    let m = &ks.modifiers;
    let mut s = String::new();
    if m.platform {
        s.push_str("cmd-");
    }
    if m.control {
        s.push_str("ctrl-");
    }
    if m.alt {
        s.push_str("alt-");
    }
    if m.shift {
        s.push_str("shift-");
    }
    s.push_str(&ks.key);
    s
}

/// Stable names for persisting a tab's view and sort (session.txt).
fn view_mode_key(v: ViewMode) -> &'static str {
    match v {
        ViewMode::List => "list",
        ViewMode::Icons => "icons",
    }
}

fn view_mode_from_key(s: &str) -> Option<ViewMode> {
    Some(match s {
        "list" => ViewMode::List,
        "icons" => ViewMode::Icons,
        "columns" | "gallery" => ViewMode::List, // Migrate legacy saved views.
        _ => return None,
    })
}

fn sort_key_name(k: SortKey) -> &'static str {
    match k {
        SortKey::None => "none",
        SortKey::Name => "name",
        SortKey::Kind => "kind",
        SortKey::Modified => "modified",
        SortKey::Created => "created",
        SortKey::Size => "size",
        SortKey::Extension => "extension",
        SortKey::Permissions => "permissions",
    }
}

fn sort_key_from_name(s: &str) -> Option<SortKey> {
    Some(match s {
        "none" => SortKey::None,
        "name" => SortKey::Name,
        "kind" => SortKey::Kind,
        "modified" => SortKey::Modified,
        "created" => SortKey::Created,
        "size" => SortKey::Size,
        "extension" => SortKey::Extension,
        "permissions" => SortKey::Permissions,
        _ => return None,
    })
}

// Default column widths for the main listing; all are user-resizable.
const ICON_W: f32 = 18.0;

// Command-palette result row height, and how many show before scrolling.
const PALETTE_ROW_H: f32 = 26.0;
const PALETTE_MAX_ROWS: usize = 7;
/// Sidebar width; also the left edge of the content/canvas area.
const SIDEBAR_W: f32 = 232.0;
/// Sidebar width when collapsed to the expand button only.
const SIDEBAR_COLLAPSED_W: f32 = 52.0;
/// Height of the custom titlebar strip (the OS titlebar is transparent, so this
/// colored bar sits behind the traffic lights).
const TITLEBAR_H: f32 = 56.0;
/// Tab strip row height.
const TAB_H: f32 = 34.0;
/// Fixed list-row height (so marquee selection can map y-coordinates to rows).
const ROW_H: f32 = 26.0;
/// Overlay scrollbar: stays solid this long after the last scroll…
const SCROLLBAR_LINGER: f32 = 1.0;
/// …then fades out over this long (seconds).
const SCROLLBAR_FADE: f32 = 0.35;
/// Duration of the pane split open/close animation.
const SPLIT_ANIM_MS: u64 = 220;
/// Red used for an invalid rename (field border/text + the reason pill).
const RENAME_ERR_COLOR: u32 = 0xef4444;

/// True while a *tab* is being dragged (gpui only exposes "some drag is
/// active", and the file-drop highlight must not appear during tab drags).
/// Set by the chip's drag constructor; cleared on drop or when the drag ends.
static TAB_DRAG_LIVE: AtomicBool = AtomicBool::new(false);

/// Width drag uses the displayed column width and writes once on release.
#[derive(Clone, Copy)]
struct Resize { pane: usize, col: Column, start_x: f32, start_w: f32 }

/// An in-progress scrollbar-thumb drag (which pane's list is being scrolled).
#[derive(Clone, Copy)]
struct ScrollDrag {
    pane: usize,
    start_y: f32,
    start_scrolled: f32,
}

/// Reserved cache keys for the shared generic folder and file icons.
const FOLDER_KEY: &str = "\u{0}folder";
const FILE_KEY: &str = "\u{0}file";
const APP_ICON_PREFIX: &str = "\u{0}app:";

/// What activating a command-palette item does.
/// One entry in the palette's ⌘K actions panel.
#[derive(Clone, Copy, PartialEq)]
enum PaletteAction {
    /// Open the item (navigate into a dir / launch a file's default app).
    Open,
    /// Navigate to the file's enclosing folder and select it.
    RevealShuffle,
    /// Open the folder (or the file's folder) in a new tab.
    OpenNewTab,
    /// Reveal in Finder.
    RevealFinder,
    /// Copy the full path to the clipboard.
    CopyPath,
}

#[derive(Clone)]
enum Action {
    /// Open a path (navigate into it if a dir, else open the file).
    Open(PathBuf, bool),
}

/// An open right-click context menu: where it sits, and the entry it targets
/// (None when invoked on empty space).
/// A right-click menu action, invoked with full app access so any existing
/// method (rename needs the window, most need `cx`) can back a menu row.
type MenuOp = Rc<dyn Fn(&mut Shuffle, &mut Window, &mut Context<Shuffle>)>;

/// One entry in the (data-driven) right-click menu. The whole tree is built
/// once when the menu opens and stored on [`ContextMenu`]; rendering, hover
/// flyouts, and keyboard navigation all walk this data — no per-view closures.
#[derive(Clone)]
enum MenuNode {
    Item {
        label: String,
        /// Right-aligned dim hint — only FIXED shortcuts ("⌫", "␣", "↩").
        hint: Option<&'static str>,
        /// Leading color dot (Finder tag colors).
        dot: Option<u32>,
        /// Leading ✓ (e.g. the item is already in that group).
        checked: bool,
        /// Destructive — rendered red (Move to Trash, Delete from Server).
        danger: bool,
        on: MenuOp,
    },
    /// A flyout submenu: opens beside the root on hover (or → / ↩).
    Sub { label: String, items: Vec<MenuNode> },
    Sep,
    /// Inert explanatory row ("No applications").
    Disabled(String),
}


fn mi(
    label: impl Into<String>,
    on: impl Fn(&mut Shuffle, &mut Window, &mut Context<Shuffle>) + 'static,
) -> MenuNode {
    MenuNode::Item {
        label: label.into(),
        hint: None,
        dot: None,
        checked: false,
        danger: false,
        on: Rc::new(on),
    }
}

fn mi_hint(
    label: impl Into<String>,
    hint: &'static str,
    on: impl Fn(&mut Shuffle, &mut Window, &mut Context<Shuffle>) + 'static,
) -> MenuNode {
    match mi(label, on) {
        MenuNode::Item { label, dot, checked, danger, on, .. } => {
            MenuNode::Item { label, hint: Some(hint), dot, checked, danger, on }
        }
        other => other,
    }
}

fn mi_danger(
    label: impl Into<String>,
    hint: Option<&'static str>,
    on: impl Fn(&mut Shuffle, &mut Window, &mut Context<Shuffle>) + 'static,
) -> MenuNode {
    match mi(label, on) {
        MenuNode::Item { label, dot, checked, on, .. } => {
            MenuNode::Item { label, hint, dot, checked, danger: true, on }
        }
        other => other,
    }
}

fn mi_dot(
    label: impl Into<String>,
    dot: u32,
    on: impl Fn(&mut Shuffle, &mut Window, &mut Context<Shuffle>) + 'static,
) -> MenuNode {
    match mi(label, on) {
        MenuNode::Item { label, hint, checked, danger, on, .. } => {
            MenuNode::Item { label, hint, dot: Some(dot), checked, danger, on }
        }
        other => other,
    }
}

fn mi_check(
    label: impl Into<String>,
    checked: bool,
    on: impl Fn(&mut Shuffle, &mut Window, &mut Context<Shuffle>) + 'static,
) -> MenuNode {
    match mi(label, on) {
        MenuNode::Item { label, hint, dot, danger, on, .. } => {
            MenuNode::Item { label, hint, dot, checked, danger, on }
        }
        other => other,
    }
}

struct ContextMenu {
    x: f32,
    y: f32,
    target: Option<(PathBuf, bool)>,
    /// The menu tree, built once at open.
    nodes: Vec<MenuNode>,

}

/// In-progress inline rename of a file/folder.
struct Rename {
    pane: usize,
    path: PathBuf,
    text: String,
    /// Text cursor (char index) within `text`.
    cursor: usize,
    /// Selection anchor (char index); `Some` and different from `cursor` means
    /// that range is selected (starts as the whole name, like Finder).
    anchor: Option<usize>,
}

/// A user-defined sidebar group: a named collection of files/folders.
#[derive(Clone)]
struct Group {
    name: String,
    paths: Vec<PathBuf>,
}

/// What a right-click in the sidebar targeted (drives its context menu).
#[derive(Clone)]
enum SidebarTarget {
    Project(u64),
    ProjectPath(projects::Section, PathBuf),
    /// Empty sidebar space → offer "New Group".
    Empty,
    /// A group's header (by index) → offer "Delete Group".
    GroupHeader(usize),
    /// A member of a group: (group index, path) → offer "Remove from Group".
    GroupMember(usize, PathBuf),
    /// A saved SFTP server → offer "Edit…" / "Remove".
    Sftp(SftpServer),
    /// A tab chip: (pane, tab index) → New Tab / Duplicate / Close / Close Others.
    Tab(usize, usize),
    /// The nav/path bar of a pane → New Tab / Copy Path / Open in Terminal.
    NavBar(usize),
}

/// One row in the command palette: a title, a gray subtitle (full path), and
/// what happens on activation.
struct PaletteItem {
    title: String,
    subtitle: String,
    action: Action,
    is_dir: bool,
}

/// One row in the main listing, with the metadata we display.
#[derive(Clone)]
struct Entry {
    name: String,
    is_dir: bool,
    size: u64,
    modified: Option<SystemTime>,
    created: Option<SystemTime>,
    mode: Option<u32>,
    /// Whether size/dates have been read (the fast first pass leaves them empty).
    loaded: bool,
    hidden_flag: bool,
}

impl Entry {
    fn is_hidden(&self) -> bool { self.name.starts_with('.') || self.hidden_flag }
    /// App bundles retain directory semantics for file operations, but are
    /// presented as applications in the browser and its file/folder filters.
    fn is_folder(&self) -> bool {
        is_browsable_folder(Path::new(&self.name), self.is_dir)
    }
}

fn has_app_extension(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("app"))
}

fn is_app_bundle(path: &Path, is_dir: bool) -> bool {
    is_dir && has_app_extension(path)
}

fn is_browsable_folder(path: &Path, is_dir: bool) -> bool {
    is_dir && !is_app_bundle(path, is_dir)
}

/// A file's cloud-storage materialization state. Detected from the kernel's
/// `SF_DATALESS` flag, which Finder itself uses — set on iCloud and File
/// Provider (Dropbox, Google Drive, OneDrive, …) placeholders whose bytes live
/// in the cloud, not on disk.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CloudSync {
    /// Not a cloud placeholder: an ordinary local file, or a fully downloaded
    /// cloud file. No badge.
    Local,
    /// Online-only: content is in the cloud, not materialized on disk.
    OnlineOnly,
    /// Actively downloading (we kicked off a materialize; clears once local).
    Syncing,
}

/// `SF_DATALESS` (super-user file flag): the bytes aren't on disk — a cloud
/// placeholder. See `chflags(2)` / `<sys/stat.h>`.
const SF_DATALESS: u32 = 0x4000_0000;

/// How the listing is sorted. `None` is the default (folders first, by name).
#[derive(Clone, Copy, PartialEq)]
enum SortKey {
    None,
    Name,
    Kind,
    Modified,
    Created,
    Size,
    Extension,
    Permissions,
}

impl SortKey {
    fn label(self) -> &'static str {
        match self {
            SortKey::None => "None",
            SortKey::Name => "Name",
            SortKey::Kind => "Kind",
            SortKey::Modified => "Date Modified",
            SortKey::Created => "Date Created",
            SortKey::Size => "Size",
            SortKey::Extension => "Extension",
            SortKey::Permissions => "Permissions",
        }
    }
}

/// One open tab: an independent directory view with its own history, scroll,
/// find, and path-edit state.
struct Tab {
    current_dir: PathBuf,
    terminal: Option<studio_terminal::Session>,
    terminal_open: bool,
    terminal_height: f32,
    entries: Vec<Entry>,
    /// Back/forward navigation history and our position within it.
    history: Vec<PathBuf>,
    hist_pos: usize,
    /// When `Some`, the path bar is an editable text field holding this string.
    editing_path: Option<String>,
    /// Text-cursor position (char index) within `editing_path`.
    path_cursor: usize,
    /// Selection anchor (char index) within `editing_path`; `Some` and different
    /// from `path_cursor` means a range is selected (Option+Shift+Arrow, Cmd+A).
    path_anchor: Option<usize>,
    /// When `Some`, an in-directory "find" filter is active (opened by `/`).
    /// `find_results` holds the matching `entries` indices, best match first.
    find_query: Option<String>,
    entry_filter: EntryFilter,
    show_hidden: bool,
    has_hidden: bool,
    copy_feedback: Option<(String, Instant)>,
    find_results: Vec<usize>,
    /// Text cursor (char index) within `find_query`.
    find_cursor: usize,
    /// Selection anchor (char index) within `find_query`; `Some` and different
    /// from `find_cursor` means a range is selected (Option+Shift+Arrow, Cmd+A).
    find_anchor: Option<usize>,
    /// Result set of the active `content:` Spotlight search (absolute paths in
    /// this folder), and the term it corresponds to. `None` = no content
    /// search resolved yet.
    content_hits: Option<HashSet<PathBuf>>,
    content_for: Option<String>,
    /// Bumps each time a content search is kicked off, so a stale mdfind result
    /// is discarded.
    content_gen: u64,
    scroll_handle: UniformListScrollHandle,
    /// Horizontal scroll of the columns (when they're wider than the pane).
    h_scroll: ScrollHandle,
    /// The set of selected items.
    selection: HashSet<PathBuf>,
    /// The focused item (last clicked): used for shift-range and the inspector.
    anchor: Option<PathBuf>,
    /// Sort criterion and direction for this tab's listing.
    sort_key: SortKey,
    sort_asc: bool,
    /// How items are displayed.
    view: ViewMode,
    list_layout: ListLayout,
    preview_width: u16,
    /// Generation of the latest load, so a stale background metadata fill is
    /// discarded if the user navigated away.
    load_gen: u64,
    /// Last time this tab's list scrolled; drives the scrollbar's fade-out.
    last_scroll: Instant,
    /// Bumped per scroll so the fade-out animation restarts from opaque.
    scroll_epoch: u64,
    /// When `Some`, this tab browses a remote SFTP server; `current_dir` is a
    /// remote absolute path on that host. `None` = a normal local tab.
    remote: Option<SftpServer>,
}

impl Tab {
    fn retain_visible_selection(&mut self) {
        let kept: HashSet<PathBuf> = self.visible_entries().map(|e| self.current_dir.join(&e.name))
            .filter(|p| self.selection.contains(p)).collect();
        self.selection = kept;
        if self.anchor.as_ref().is_some_and(|p| !self.selection.contains(p)) { self.anchor = self.selection.iter().next().cloned(); }
    }
    fn set_entry_filter(&mut self, filter: EntryFilter) {
        self.entry_filter = filter;
        self.selection.clear();
        self.anchor = None;
        self.scroll_handle = UniformListScrollHandle::new();
        self.h_scroll = ScrollHandle::new();
        self.recompute_results();
    }

    fn is_filtered(&self) -> bool {
        self.entry_filter != EntryFilter::All || self.find_query.is_some() || (!self.show_hidden && self.has_hidden)
    }

    fn visible_entries(&self) -> impl Iterator<Item = &Entry> {
        let filtered = self.is_filtered();
        let count = self.visible_count();
        (0..count).filter_map(move |row| {
            let index = if filtered { self.find_results[row] } else { row };
            self.entries.get(index)
        })
    }

    fn visible_count(&self) -> usize {
        if self.is_filtered() { self.find_results.len() } else { self.entries.len() }
    }

    /// Combine the type switch with the text query, preserving the current sort.
    fn recompute_results(&mut self) {
        self.has_hidden = self.entries.iter().any(Entry::is_hidden);
        let tab = self;
        let q = tab.find_query.as_deref().unwrap_or("").trim();
        if q.is_empty() {
            tab.find_results = tab.entries.iter().enumerate()
                .filter_map(|(i, entry)| (tab.entry_filter.accepts(entry.is_folder()) && (tab.show_hidden || !entry.is_hidden())).then_some(i))
                .collect();
            return;
        }
        let fq = FilterQuery::parse(q);
        let has_text = !fq.text.is_empty();
        // Content search resolves asynchronously; until its hits for THIS term
        // land, a content: query matches nothing (the spinner-free "searching"
        // state). update_content_search() recomputes when mdfind returns.
        let content_ready = match &fq.content {
            None => None,
            Some(term) => {
                if tab.content_for.as_deref() == Some(term.as_str()) {
                    tab.content_hits.as_ref()
                } else {
                    tab.find_results.clear();
                    return;
                }
            }
        };
        let dir = tab.current_dir.clone();
        let plain = fq.text.clone();

        // Sort order: when a column sort is active OR there's no free text to
        // rank by, keep the list's existing order; otherwise rank by match.
        let keep_order = tab.sort_key != SortKey::None || !has_text;

        let mut scored: Vec<(i32, usize)> = Vec::new();
        for (i, e) in tab.entries.iter().enumerate() {
            if !tab.entry_filter.accepts(e.is_folder()) || (!tab.show_hidden && e.is_hidden()) {
                continue;
            }
            if !fq.matches_entry(&e.name, e.is_dir, e.size, e.modified) {
                continue;
            }
            if let Some(hits) = content_ready {
                if !hits.contains(&dir.join(&e.name)) {
                    continue;
                }
            }
            let score = if has_text {
                match find_score(&plain, &e.name) {
                    Some(s) => s,
                    None => continue,
                }
            } else {
                0
            };
            scored.push((score, i));
        }

        if !keep_order {
            // Best score first; ties → dirs first, then alphabetical.
            let entries = &tab.entries;
            scored.sort_by(|a, b| {
                b.0.cmp(&a.0)
                    .then_with(|| entries[b.1].is_folder().cmp(&entries[a.1].is_folder()))
                    .then_with(|| {
                        entries[a.1].name.to_lowercase().cmp(&entries[b.1].name.to_lowercase())
                    })
            });
        }
        tab.find_results = scored.into_iter().map(|(_, i)| i).collect();
    }

    fn new(dir: PathBuf) -> Self {
        // Fast first paint; full metadata is filled in from the background.
        let entries = read_entries_fast(&dir);
        let mut tab = Tab {
            current_dir: dir.clone(),
            terminal: None,
            terminal_open: false,
            terminal_height: 290.0,
            entries,
            history: vec![dir.clone()],
            hist_pos: 0,
            editing_path: None,
            path_cursor: 0,
            path_anchor: None,
            find_query: None,
            entry_filter: EntryFilter::All,
            show_hidden: false,
            has_hidden: false,
            copy_feedback: None,
            find_results: Vec::new(),
            find_cursor: 0,
            find_anchor: None,
            content_hits: None,
            content_for: None,
            content_gen: 0,
            scroll_handle: UniformListScrollHandle::new(),
            h_scroll: ScrollHandle::new(),
            selection: HashSet::new(),
            anchor: None,
            sort_key: SortKey::None,
            sort_asc: true,
            view: ViewMode::List,
            list_layout: ListLayout::default(),
            preview_width: prefs().preview_width,

            load_gen: 0,
            last_scroll: Instant::now(),
            scroll_epoch: 0,
            remote: None,
        };
        tab.recompute_results();
        tab
    }
}

/// A pane: a column in the canvas holding a stack of tabs.
struct Pane {
    tabs: Vec<Tab>,
    active: usize,
}

impl Pane {
    fn new(dir: PathBuf) -> Self {
        Pane {
            tabs: vec![Tab::new(dir)],
            active: 0,
        }
    }

    fn active_tab(&self) -> &Tab {
        &self.tabs[self.active]
    }

    fn active_tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active]
    }
}

/// Payload for a tab drag: which pane/tab is being dragged.
#[derive(Clone, Copy)]
struct TabDrag {
    pane: usize,
    tab: usize,
}

/// A small floating tooltip for toolbar actions and file paths.
struct TooltipView {
    text: String,
}

impl Render for TooltipView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme();
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(Theme::alpha(t.surface, 0xf2))
            .border_1()
            .border_color(rgb(t.border))
            .text_color(rgb(t.text))
            .text_size(px(UI_SMALL_FONT_SIZE))
            .shadow_lg()
            .child(self.text.clone())
    }
}

/// The floating preview rendered under the cursor while dragging a tab.
struct TabDragPreview {
    label: String,
}

impl Render for TabDragPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme();
        div()
            .px_3()
            .py_1()
            .rounded_md()
            .bg(Theme::alpha(t.surface, 0xf2))
            .border_1()
            .border_color(rgb(t.accent))
            .text_color(rgb(t.text))
            .text_size(px(UI_FONT_SIZE))
            .shadow_lg()
            .child(self.label.clone())
    }
}

/// The root view: a workspace of one or two side-by-side panes.
struct Shuffle {
    volumes: Vec<volumes::Volume>,
    panes: Vec<Pane>,
    active_pane: usize,
    /// Left pane's width fraction when two panes are shown (0.2..0.8).
    split_ratio: f32,
    /// In-progress divider drag: (cursor x at grab, split_ratio at grab).
    divider_drag: Option<(f32, f32)>,
    /// Bumped whenever the pane layout changes (split created or collapsed) so
    /// the one-shot open/close width animations restart.
    split_epoch: usize,
    /// Set when the split collapses back to one pane: the surviving pane's
    /// starting width fraction and whether it's anchored to the right edge
    /// (i.e. the *left* pane was the one removed). Drives the grow animation.
    collapse_anim: Option<(f32, bool)>,
    /// True while the scrollbar-fade repaint ticker task is alive.
    fade_ticker: bool,
    /// User-defined sidebar groups (when the feature is enabled).
    groups: Vec<Group>,
    /// Open sidebar context menu: (x, y, what was clicked).
    sidebar_menu: Option<(f32, f32, SidebarTarget)>,
    /// Open "New Group" naming dialog: the name being typed (None = closed).
    group_dialog: Option<String>,
    /// Sidebar section titles the user has collapsed (hidden their items).
    collapsed_sections: HashSet<String>,
    folder_views: folder_views::Library,
    resize: Option<Resize>,
    scroll_drag: Option<ScrollDrag>,
    /// In-progress drag of a List view's horizontal scrollbar thumb:
    /// (pane, cursor x at grab, columns scrolled at grab).
    hbar_drag: Option<(usize, f32, f32)>,
    // File search (Cmd+P), plus folder context actions.
    focus: FocusHandle,
    palette_open: bool,
    file_search: search_ui::State,
    space_map: Option<space_ui::State>,
    dismiss_space_map: bool,
    preview_pending: RefCell<HashMap<PathBuf,Arc<AtomicBool>>>,
    preview_resize: Option<preview_panel::Drag>,
    video_preview: Option<Rc<video_preview::Session>>,
    info_pending: RefCell<HashSet<PathBuf>>,
    pdf_pending: RefCell<HashSet<(PathBuf,usize)>>,
    query: String,
    /// Text-cursor position in `query`, as a char index (0..=char count).
    query_cursor: usize,
    /// Selection anchor (char index). When `Some` and different from the cursor,
    /// the range between them is selected; Cmd+A selects all, Option+Shift+Arrow
    /// extends by word. `None` means no selection.
    query_anchor: Option<usize>,
    /// Past palette queries (newest last), when the history setting is on.
    palette_hist: Vec<String>,
    /// Which history entry is being browsed with Up/Down (None = live query).
    palette_hist_pos: Option<usize>,
    palette_items: Vec<PaletteItem>,
    selected: usize,
    /// The palette's ⌘K actions panel: selected action index (None = closed).
    palette_actions: Option<usize>,
    search_gen: u64,
    palette_scroll: ScrollHandle,
    context_menu: Option<ContextMenu>,
    native_menu_pending: bool,
    /// In-progress inline rename, if any.
    rename: Option<Rename>,
    /// Open "Sort By" dropdown: (pane, x, y) in window coords.
    sort_menu: Option<(usize, f32, f32)>,
    /// Pending "move to Trash" confirmation: (pane, paths).
    confirm_delete: Option<(usize, Vec<PathBuf>)>,
    /// Monotonic counter tagging each directory load (for stale-result guards).
    next_load_gen: u64,
    /// In-progress marquee (box) selection: (pane, start, current) window coords.
    marquee: Option<(usize, (f32, f32), (f32, f32))>,
    /// A left-press on a draggable row: (pane, path, press position). Promoted to
    /// a native OS drag once the cursor moves past a small threshold.
    drag_candidate: Option<(usize, PathBuf, (f32, f32))>,
    /// Open "Connect to Server" dialog: the URL being typed (None = closed).
    server_dialog: Option<ServerForm>,
    /// First-run SSH prompt (choose ~/.ssh vs. credentials) — shown once.
    ssh_ask: bool,
    /// Recently-connected server URLs (most recent first).
    server_history: Vec<String>,
    /// Dragging the terminal divider: pane, starting Y, starting height.
    terminal_resize: Option<(usize, f32, f32)>,
    /// State of the self-updater's banner. `None` = up to date / not checked /
    /// dismissed.
    update: Option<UpdateStatus>,
    /// Current page of the inspector's PDF preview (0-based; reset on focus).
    preview_page: usize,
    /// The row the mouse is currently over: (pane, path). Enter renames it
    /// even when nothing is selected (point-and-rename).
    hovered: Option<(usize, PathBuf)>,
    /// Where a file drag would land right now: (pane, Some(display row)) for
    /// a folder row, (pane, None) for the pane's own directory. Computed from
    /// the drag position on every synthesized drag-move — one source of truth,
    /// so the highlight can't flicker between panes.
    drop_hover: Option<(usize, Option<usize>)>,
    /// Last SFTP error (connection/listing), shown as a banner until cleared.
    remote_error: Option<String>,
    /// Waterfall sidebar tree: which folders are expanded, their cached child
    /// subfolders, and which are being loaded (to avoid duplicate reads).
    waterfall_expanded: HashSet<PathBuf>,
    waterfall_children: HashMap<PathBuf, Vec<PathBuf>>,
    waterfall_pending: HashSet<PathBuf>,
    /// Each cached waterfall folder's mtime, so the folder watcher can spot an
    /// outside change and invalidate just that folder for a live refresh.
    waterfall_mtime: HashMap<PathBuf, SystemTime>,
    /// Cloud files with a download/evict in flight — shown with the syncing
    /// badge until the operation finishes and the listing is re-read.
    cloud_busy: HashSet<PathBuf>,
    /// A short status message (cloud sync, undo, …) shown as a neutral
    /// banner; the generation lets a stale auto-dismiss leave a newer one be.
    notice: Option<String>,
    notice_gen: u64,
    /// Bumped per Sync Now; the running sync watch stops when it's stale.
    cloud_sync_gen: u64,
    /// Background copies/moves in flight (the progress panel).
    jobs: Vec<FileJob>,
    next_job_id: u64,
    /// File operations that ⌘Z can reverse, newest last.
    undo_stack: Vec<UndoOp>,
    /// The tabs/split layout as last written to session.txt (skip rewrites).
    saved_session: String,
    projects: projects::Library,
    project_load_error: Option<String>,
    project_sessions: HashMap<Option<u64>, studio::ProjectSession>,
    project_dialog: Option<String>,
    project_dialog_error: Option<String>,
    new_text_file: Option<new_text_file::Form>,
    /// The open Get Info card, if any.
    info_panel: Option<InfoPanel>,
    /// The open "Rename N Items" dialog, if any.
    batch_rename: Option<BatchRename>,
}

/// Which cloud store a path belongs to, for routing download/evict.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CloudKind {
    /// iCloud Drive — full download + evict via Foundation.
    ICloud,
    /// A third-party File Provider store (Dropbox, Google Drive, OneDrive, …) —
    /// download works when the provider is running; evict is provider-driven.
    Provider,
}

/// Update-check state shared between the Settings window (which drives the
/// "Check for Updates" button) and the main window (which shows the banner
/// and performs the install).
#[derive(Clone, PartialEq, Default)]
enum UpdateCheck {
    #[default]
    Idle,
    Checking,
    UpToDate,
    /// A newer tag (like "v0.2.7") is available to install.
    Available(String),
    /// The GitHub query failed (offline, rate limit, …).
    Failed,
    /// Settings asked the main window to download + install this tag now.
    Install(String),
}

#[derive(Clone, Default)]
struct UpdateCheckGlobal(UpdateCheck);
impl gpui::Global for UpdateCheckGlobal {}

/// Drives the update banner under the titlebar.
#[derive(Clone)]
enum UpdateStatus {
    /// A newer release (tag like "v0.2.4") is available to install.
    Available(String),
    /// The user clicked Update; we're downloading + verifying it now.
    Downloading(String),
    /// The install couldn't complete; carries a short reason.
    Failed(String),
}

impl Shuffle {
    fn new(dir: PathBuf, cx: &mut Context<Self>) -> Self {
        // A background snapshot, independent of project switching.
        Self::watch_volumes_start(cx);
        cx.on_app_quit(|this, _| {
            this.save_session();
            let job = this.folder_views.write_job();
            async move { if let Some(job) = job { let _ = job.run(); } }
        }).detach();
        ensure_base_icons(); // real folder/file icons ready before first render
        // Sync + repaint whenever the theme changes (e.g. from Settings).
        cx.observe_global::<ThemeGlobal>(|_, cx| {
            set_active_theme(cx.global::<ThemeGlobal>().0);
            cx.notify();
        })
        .detach();
        // Sync + repaint whenever feature prefs change.
        cx.observe_global::<PrefsGlobal>(|this, cx| {
            set_active_prefs(cx.global::<PrefsGlobal>().0);
            this.prewarm_icons(cx);
            cx.notify();
        })
        .detach();
        // Sync + repaint whenever the saved SFTP servers change (from Settings).
        cx.observe_global::<SftpServersGlobal>(|this, cx| {
            let servers = cx.global::<SftpServersGlobal>().0.clone();
            set_active_sftp_servers(servers.clone());
            if this.projects.sidebar().servers != servers {
                let mut candidate = this.projects.clone();
                candidate.sidebar_mut().servers = servers;
                if let Err(error) = this.persist_library(candidate) { this.set_notice(error, cx); }
            }
            cx.notify();
        })
        .detach();
        // Sync + repaint whenever the menu style changes (e.g. from Settings).
        cx.observe_global::<MenuStyleGlobal>(|_, cx| {
            set_active_menu(cx.global::<MenuStyleGlobal>().0);
            cx.notify();
        })
        .detach();
        // Sync the keymap when it changes (e.g. from the Settings window).
        cx.observe_global::<KeymapGlobal>(|_, cx| {
            set_active_keymap(cx.global::<KeymapGlobal>().0.clone());
            cx.notify();
        })
        .detach();
        // React to the Settings window's update checks: an explicit check that
        // finds a new version shows the banner here (even a previously
        // dismissed one), and "Install & Relaunch" starts the self-update.
        cx.observe_global::<UpdateCheckGlobal>(|this, cx| {
            match cx.global::<UpdateCheckGlobal>().0.clone() {
                UpdateCheck::Available(tag) => {
                    this.update = Some(UpdateStatus::Available(tag));
                    cx.notify();
                }
                UpdateCheck::Install(tag) => {
                    if !matches!(this.update, Some(UpdateStatus::Downloading(_))) {
                        this.update = Some(UpdateStatus::Available(tag));
                        this.start_self_update(cx);
                    }
                }
                _ => {}
            }
        })
        .detach();
        // Observe only visible local folders. Filesystem notifications replace
        // the old once-a-second cloud directory hashing.
        cx.spawn(async move |this, cx| {
            let mut watcher = folder_watch::Watcher::new();
            let mut maintenance = Instant::now();
            loop {
                cx.background_executor().timer(Duration::from_millis(250)).await;
                let dirs = match this.update(cx, |this, _| {
                    this.panes.iter().map(|p| {
                        let tab = p.active_tab();
                        if tab.remote.is_some() { return None; }
                        Some(tab.current_dir.clone())
                    }).collect::<Vec<_>>()
                }) { Ok(dirs) => dirs, Err(_) => break };
                let wanted = dirs.iter().flatten().cloned().collect();
                let (returned, changed) = cx.background_spawn(async move {
                    let changed = watcher.changes(wanted); (watcher, changed)
                }).await;
                watcher = returned;
                if this.update(cx, |this,cx| {
                    for (pane,dir) in dirs.into_iter().enumerate() {
                        let Some(root) = dir else {continue;};
                        if pane >= this.panes.len() || this.tab(pane).current_dir != root {continue;}
                        if changed.contains(&root) {
                            this.reload_pane(pane,cx);
                        }

                    }
                    if maintenance.elapsed() >= Duration::from_secs(1) {
                        this.refresh_waterfall(cx);
                        this.save_session();
                        maintenance = Instant::now();
                    }
                }).is_err() {break;}
            }
        }).detach();
        // Hidden debug harness (SHUFFLE_MQ_SIM=1): drive a marquee + edge drag
        // programmatically and log what the auto-scroll loop does. Synthetic OS
        // mouse events don't reach gpui, so this is how the mechanics are
        // verified end-to-end in-process.
        if std::env::var_os("SHUFFLE_MQ_SIM").is_some() {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(Duration::from_secs(3)).await;
                let _ = this.update(cx, |this, cx| {
                    let (top, bottom) = {
                        let st = this.tab(0).scroll_handle.0.borrow();
                        let b = st.base_handle.bounds();
                        let top = f64::from(b.origin.y) as f32;
                        (top, top + f64::from(b.size.height) as f32)
                    };
                    eprintln!("[mq-sim] viewport top={top:.0} bottom={bottom:.0}");
                    let mid = (top + bottom) / 2.0;
                    this.begin_marquee(0, 400.0, mid, cx);
                    // Drag 60px past the bottom edge and hold.
                    this.update_marquee(400.0, bottom + 60.0, cx);
                    eprintln!("[mq-sim] begin at y={mid:.0}, cur held at y={:.0}", bottom + 60.0);
                });
                cx.background_executor().timer(Duration::from_secs(3)).await;
                let _ = this.update(cx, |this, cx| {
                    eprintln!(
                        "[mq-sim] after 3s down: scrolled={:.0} selected={}",
                        this.current_scrolled(0),
                        this.tab(0).selection.len()
                    );
                    // Now hold 40px above the top edge — should scroll back up.
                    let top = {
                        let st = this.tab(0).scroll_handle.0.borrow();
                        f64::from(st.base_handle.bounds().origin.y) as f32
                    };
                    this.update_marquee(400.0, top - 40.0, cx);
                });
                cx.background_executor().timer(Duration::from_secs(3)).await;
                let _ = this.update(cx, |this, cx| {
                    eprintln!(
                        "[mq-sim] after 3s up: scrolled={:.0} selected={}",
                        this.current_scrolled(0),
                        this.tab(0).selection.len()
                    );
                    this.end_marquee(cx);
                    eprintln!("[mq-sim] done");
                });
            })
            .detach();
        }
        // Rebuild icons when the icon pack changes.
        cx.observe_global::<IconPackGlobal>(|this, cx| {
            set_active_icon_pack(cx.global::<IconPackGlobal>().0.clone());
            clear_icon_cache();
            ensure_base_icons();
            cx.notify();
            this.prewarm_icons(cx);
        })
        .detach();
        let project_result = config_dir()
            .ok_or_else(|| "Cannot find the settings folder.".to_string())
            .and_then(|dir| projects::Library::load_migrating(
                &dir.join("projects.json"), &dir.join("workspaces.json"), read_path_list("bookmarks.txt")
            ).map_err(|error| format!("Couldn't read projects: {error}. The existing file has been preserved.")));
        let (mut projects, mut project_load_error) = match project_result {
            Ok(library) => (library, None),
            Err(error) => (projects::Library::default(), Some(error)),
        };
        if project_load_error.is_none() && projects.migrate_sidebar(load_sftp_servers()) {
            if let Some(path) = config_file("projects.json") {
                if let Err(error) = projects.save(&path) { project_load_error = Some(format!("Couldn't migrate project sidebar: {error}")); }
            }
        }
        let servers = projects.sidebar().servers.clone();
        set_active_sftp_servers(servers.clone());
        cx.set_global(SftpServersGlobal(servers));
        Self {
            panes: vec![Pane::new(dir)],
            volumes: Vec::new(),
            active_pane: 0,
            split_ratio: 0.5,
            divider_drag: None,
            split_epoch: 0,
            collapse_anim: None,
            fade_ticker: false,
            groups: load_groups(),
            sidebar_menu: None,
            group_dialog: None,
            collapsed_sections: read_string_list("collapsed_sections.txt").into_iter().collect(),
            folder_views: folder_views::Library::open(config_file("folder-views.json")),
            resize: None,
            scroll_drag: None,
            hbar_drag: None,
            focus: cx.focus_handle(),
            palette_open: false,
            file_search: search_ui::State::default(),
            space_map: None,
            dismiss_space_map: false,
            preview_pending: RefCell::new(HashMap::new()),
            preview_resize: None,
            video_preview: None,
            info_pending: RefCell::new(HashSet::new()),
            pdf_pending: RefCell::new(HashSet::new()),
            query: String::new(),
            query_cursor: 0,
            query_anchor: None,
            palette_hist: read_string_list("palette_history.txt"),
            palette_hist_pos: None,
            palette_items: Vec::new(),
            selected: 0,
            palette_actions: None,
            search_gen: 0,
            palette_scroll: ScrollHandle::new(),
            context_menu: None,
            native_menu_pending: false,
            rename: None,
            sort_menu: None,
            confirm_delete: None,
            next_load_gen: 0,
            marquee: None,
            drag_candidate: None,
            server_dialog: None,
            ssh_ask: false,
            server_history: read_string_list("servers.txt"),
            terminal_resize: None,
            update: None,
            preview_page: 0,
            hovered: None,
            drop_hover: None,
            remote_error: None,
            waterfall_expanded: HashSet::new(),
            waterfall_children: HashMap::new(),
            waterfall_pending: HashSet::new(),
            waterfall_mtime: HashMap::new(),
            cloud_busy: HashSet::new(),
            notice: None,
            notice_gen: 0,
            cloud_sync_gen: 0,
            jobs: Vec::new(),
            next_job_id: 0,
            undo_stack: Vec::new(),
            saved_session: String::new(),
            projects, project_load_error, project_sessions: HashMap::new(),
            project_dialog: None, project_dialog_error: None, new_text_file: None,
            info_panel: None,
            batch_rename: None,
        }
    }

    // ----- pane / tab accessors -----

    fn pane(&self, ix: usize) -> &Pane {
        &self.panes[ix]
    }

    fn pane_mut(&mut self, ix: usize) -> &mut Pane {
        &mut self.panes[ix]
    }

    fn tab(&self, pane: usize) -> &Tab {
        self.panes[pane].active_tab()
    }

    fn tab_mut(&mut self, pane: usize) -> &mut Tab {
        self.panes[pane].active_tab_mut()
    }

    fn active_tab(&self) -> &Tab {
        self.tab(self.active_pane)
    }

    // ----- right-click context menu -----

    fn open_context_menu(&mut self, pane: usize, x: f32, y: f32, target: Option<(PathBuf, bool)>, cx: &mut Context<Self>) {
        self.active_pane = pane.min(self.panes.len() - 1);
        let pane = self.active_pane;
        self.rename = None;
        // Remote (SFTP) tabs get a reduced menu — the local-only actions
        // (Quick Actions, Compress, Tags, aliases, Reveal in Finder…) don't
        // apply to a remote path.
        let nodes = if self.tab(pane).remote.is_some() {
            self.menu_nodes_remote(pane, target.clone())
        } else {
            self.menu_nodes_root(pane, target.clone())
        };
        self.context_menu = Some(ContextMenu {
            x,
            y,
            target,
            nodes,

        });
        cx.notify();
    }

    fn close_context_menu(&mut self, cx: &mut Context<Self>) {
        if self.context_menu.take().is_some() {
            cx.notify();
        }
    }

    /// Mouse entered a root row: highlight it, and open/close the flyout
    /// (hovering a submenu parent opens it immediately, Finder-style).


    /// Mouse entered a flyout row.


    /// Run the action behind a row (`sub` addresses into the open flyout).


    /// Keyboard navigation while the context menu is open (it's modal: every
    /// key is swallowed). Arrows move, → opens a flyout, ← returns, Enter
    /// invokes, Esc closes.


    /// Re-read a pane's current directory contents (after a create/trash).
    fn refresh_pane(&mut self, pane: usize, cx: &mut Context<Self>) {

        self.reload_pane(pane, cx);
    }

    /// Load (or reload) a pane's directory: a near-instant cheap pass for first
    /// paint, then a background pass that fills in sizes/dates without blocking.
    fn reload_pane(&mut self, pane: usize, cx: &mut Context<Self>) {
        // Remote (SFTP) tabs load over the network on a background thread.
        if self.tab(pane).remote.is_some() {
            self.reload_remote_pane(pane, cx);
            return;
        }
        let dir = self.tab(pane).current_dir.clone();
        self.tab_mut(pane).entries = read_entries_fast(&dir);
        self.next_load_gen += 1;
        let gen = self.next_load_gen;
        self.tab_mut(pane).load_gen = gen;
        self.sort_tab(pane);
        if self.tab(pane).is_filtered() {
            self.recompute_find(pane);
        }
        cx.notify();
        self.prewarm_icons(cx);

        // Fill full metadata in the background, then swap it in if still current.
        cx.spawn(async move |this, cx| {
            let d = dir.clone();
            let full = cx.background_spawn(async move { read_entries(&d) }).await;
            let _ = this.update(cx, |this, cx| {
                if pane < this.panes.len()
                    && this.tab(pane).load_gen == gen
                    && this.tab(pane).current_dir == dir
                {
                    this.tab_mut(pane).entries = full;
                    this.sort_tab(pane);
                    if this.tab(pane).is_filtered() {
                        this.recompute_find(pane);
                    }
                    cx.notify();
                    this.prewarm_icons(cx);
                }
            });
        })
        .detach();
    }

    /// Load a remote SFTP directory listing on a background thread, then swap
    /// it in if the tab is still on that path.
    fn reload_remote_pane(&mut self, pane: usize, cx: &mut Context<Self>) {
        let Some(server) = self.tab(pane).remote.clone() else {
            return;
        };
        let dir = self.tab(pane).current_dir.clone();
        let path = dir.to_string_lossy().into_owned();
        self.next_load_gen += 1;
        let gen = self.next_load_gen;
        self.tab_mut(pane).load_gen = gen;
        // Show a placeholder (empty) immediately; the listing arrives async.
        self.tab_mut(pane).entries.clear();
        self.recompute_find(pane);
        cx.notify();
        let use_system = prefs().ssh_use_system;
        cx.spawn(async move |this, cx| {
            let s = server.clone();
            let p = path.clone();
            let result = cx
                .background_spawn(async move { sftp_list(&s, &p, use_system) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if pane >= this.panes.len()
                    || this.tab(pane).load_gen != gen
                    || this.tab(pane).current_dir != dir
                {
                    return;
                }
                match result {
                    Ok(entries) => {
                        this.tab_mut(pane).entries = entries;
                        this.remote_error = None;
                        this.sort_tab(pane);
                        if this.tab(pane).is_filtered() {
                            this.recompute_find(pane);
                        }
                        this.prewarm_icons(cx);
                    }
                    Err(e) => {
                        this.tab_mut(pane).entries.clear();
                        this.recompute_find(pane);
                        this.remote_error = Some(e);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Run a remote operation off-thread, then reload the pane (or show the
    /// error). The closure captures the server + parameters it needs.
    fn run_remote<F>(&mut self, pane: usize, op: F, cx: &mut Context<Self>)
    where
        F: FnOnce() -> Result<(), String> + Send + 'static,
    {
        cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move { op() }).await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        this.remote_error = None;
                        if pane < this.panes.len() {
                            this.reload_pane(pane, cx);
                        }
                    }
                    Err(e) => {
                        this.remote_error = Some(e);
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    /// A unique child name in a remote dir, computed from the current listing.
    fn unique_remote_name(&self, pane: usize, base: &str) -> String {
        let names: Vec<String> = self.tab(pane).entries.iter().map(|e| e.name.clone()).collect();
        if !names.iter().any(|n| n == base) {
            return base.to_string();
        }
        for i in 2..1000 {
            let cand = format!("{base} {i}");
            if !names.iter().any(|n| n == &cand) {
                return cand;
            }
        }
        base.to_string()
    }

    /// Make a new folder (or empty file) on the remote server; the new item is
    /// selected when the listing refreshes (press Enter to rename it).
    fn new_remote_item(&mut self, pane: usize, is_dir: bool, cx: &mut Context<Self>) {
        let Some(server) = self.tab(pane).remote.clone() else {
            return;
        };
        let base = if is_dir { "untitled folder" } else { "untitled file" };
        let name = self.unique_remote_name(pane, base);
        let path = self.tab(pane).current_dir.join(&name);
        let remote_path = path.to_string_lossy().into_owned();
        let use_system = prefs().ssh_use_system;
        cx.spawn(async move |this, cx| {
            let s = server.clone();
            let rp = remote_path.clone();
            let r = cx
                .background_spawn(async move {
                    if is_dir {
                        sftp_mkdir(&s, &rp, use_system)
                    } else {
                        sftp_touch(&s, &rp, use_system)
                    }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if pane >= this.panes.len() {
                    return;
                }
                match r {
                    Ok(()) => {
                        this.remote_error = None;
                        this.tab_mut(pane).selection = std::iter::once(path.clone()).collect();
                        this.tab_mut(pane).anchor = Some(path);
                        this.reload_pane(pane, cx);
                    }
                    Err(e) => {
                        this.remote_error = Some(e);
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    /// Upload local file(s) into a remote directory on `pane`.
    fn upload_to_remote(&mut self, pane: usize, remote_dir: PathBuf, locals: Vec<PathBuf>, cx: &mut Context<Self>) {
        let Some(server) = self.tab(pane).remote.clone() else {
            return;
        };
        let dir = remote_dir.to_string_lossy().into_owned();
        let use_system = prefs().ssh_use_system;
        self.run_remote(
            pane,
            move || {
                for local in &locals {
                    sftp_upload(&server, local, &dir, use_system)?;
                }
                Ok(())
            },
            cx,
        );
    }

    /// Re-apply this tab's sort criterion to its entries.
    fn sort_tab(&mut self, pane: usize) {
        let (key, asc) = (self.tab(pane).sort_key, self.tab(pane).sort_asc);
        sort_entries(&mut self.tab_mut(pane).entries, key, asc);
        let hidden = self.tab(pane).entries.iter().any(Entry::is_hidden);
        self.tab_mut(pane).has_hidden = hidden;
    }

    /// Set the sort criterion (clicking the same column toggles direction).
    fn set_sort(&mut self, pane: usize, key: SortKey, cx: &mut Context<Self>) {
        {
            let tab = self.tab_mut(pane);
            if tab.sort_key == key && key != SortKey::None {
                tab.sort_asc = !tab.sort_asc;
            } else {
                tab.sort_key = key;
                tab.sort_asc = true;
            }
        }
        self.sort_tab(pane);
        if self.tab(pane).is_filtered() {
            self.recompute_find(pane);
        }
        self.remember_folder_view(pane, cx);
        cx.notify();
    }

    /// Switch how a pane displays its items.


    fn new_folder(&mut self, pane: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.tab(pane).remote.is_some() {
            self.new_remote_item(pane, true, cx);
            return;
        }
        let path = unique_child(&self.tab(pane).current_dir, "untitled folder");
        if fs::create_dir(&path).is_ok() {
            self.push_undo(UndoOp::Created(vec![path.clone()]));
            self.refresh_pane(pane, cx);
            self.reveal_and_rename(pane, path, window, cx);
        }
    }

    fn new_file(&mut self, pane: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.tab(pane).remote.is_some() {
            self.new_remote_item(pane, false, cx);
            return;
        }
        let path = unique_child(&self.tab(pane).current_dir, "untitled file");
        if fs::File::create(&path).is_ok() {
            self.push_undo(UndoOp::Created(vec![path.clone()]));
            self.refresh_pane(pane, cx);
            self.reveal_and_rename(pane, path, window, cx);
        }
    }

    /// Scroll an item into view in `pane`, select it, and make it the focused
    /// entry (inspector target / rename target).
    fn reveal_in_pane(&mut self, pane: usize, path: PathBuf, cx: &mut Context<Self>) {
        let paths = self.display_paths(pane);
        if let Some(ix) = paths.iter().position(|p| p == &path) {
            let (view, off) = {
                let tab = self.tab(pane);
                let off = usize::from(
                    !tab.is_filtered()
                        && prefs().show_parent
                        && tab.current_dir.parent().is_some(),
                );
                (tab.view, off)
            };
            let item = match view {
                ViewMode::Icons => ix / self.icon_cols(pane),
                _ => ix + off,
            };
            self.tab(pane).scroll_handle.scroll_to_item(item, ScrollStrategy::Center);
            self.mark_scrolled(pane, cx);
        }
        self.tab_mut(pane).selection = std::iter::once(path.clone()).collect();
        self.focus_entry(pane, path, cx);
    }

    /// Scroll a freshly-created item into view, select it, and open the name
    /// editor so the user can type the real name immediately. Enter with
    /// nothing typed, Escape, or clicking elsewhere keeps the default name.
    fn reveal_and_rename(
        &mut self,
        pane: usize,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.reveal_in_pane(pane, path.clone(), cx);
        self.begin_rename(pane, path, window, cx);
    }

    /// Quick Look the current selection (spacebar), like Finder. Uses macOS
    /// `qlmanage -p`, which pops the system preview panel. Local files only —
    /// remote (SFTP) items aren't on disk for QuickLook to read.
    fn quick_look(&self, pane: usize) {
        if self.tab(pane).remote.is_some() {
            return;
        }
        let mut paths: Vec<PathBuf> = self.tab(pane).selection.iter().cloned().collect();
        if paths.is_empty() {
            if let Some(a) = self.tab(pane).anchor.clone() {
                paths.push(a);
            }
        }
        if paths.is_empty() {
            return;
        }
        paths.sort();
        let _ = Command::new("qlmanage")
            .arg("-p")
            .args(&paths)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }

    fn open_path(&mut self, pane: usize, path: PathBuf, is_dir: bool, cx: &mut Context<Self>) {
        // App bundles are directories, but double-clicking one should *launch*
        // it like Finder — browse inside via right-click → Show Package Contents.
        let is_app = self.tab(pane).remote.is_none() && is_app_bundle(&path, is_dir);
        if is_dir && !is_app {
            self.navigate_in(pane, path, cx);
        } else if !is_app && self.tab(pane).remote.is_some() {
            // Remote file → download to ~/Downloads, then open it.
            self.download_remote(pane, path, None, true, cx);
        } else {
            let _ = Command::new("open").arg(&path).spawn();
        }
    }

    /// Download a remote file to a local directory (default: ~/Downloads) on a
    /// background thread; optionally `open` it when finished.
    fn download_remote(
        &mut self,
        pane: usize,
        remote_path: PathBuf,
        dest_dir: Option<PathBuf>,
        open_after: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(server) = self.tab(pane).remote.clone() else {
            return;
        };
        let name = remote_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "download".into());
        let dest_dir = dest_dir.unwrap_or_else(|| home_dir().join("Downloads"));
        let local = unique_child(&dest_dir, &name);
        let remote = remote_path.to_string_lossy().into_owned();
        let use_system = prefs().ssh_use_system;
        cx.spawn(async move |this, cx| {
            let s = server.clone();
            let r = remote.clone();
            let l = local.clone();
            let result = cx
                .background_spawn(async move {
                    let _ = fs::create_dir_all(l.parent().unwrap_or(&l));
                    // sftp `get "remote" "local"`.
                    let script = format!(
                        "get \"{}\" \"{}\"",
                        r.replace('"', ""),
                        l.to_string_lossy().replace('"', "")
                    );
                    sftp_batch(&s, &script, use_system).map(|_| l)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(local) => {
                        this.remote_error = None;
                        if open_after {
                            let _ = Command::new("open").arg(&local).spawn();
                        }
                    }
                    Err(e) => this.remote_error = Some(format!("Download failed: {e}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Ask to move the current selection (or the focused item) to Trash.
    fn request_delete(&mut self, pane: usize, cx: &mut Context<Self>) {
        let mut paths: Vec<PathBuf> = self.tab(pane).selection.iter().cloned().collect();
        if paths.is_empty() {
            if let Some(a) = self.tab(pane).anchor.clone() {
                paths.push(a);
            }
        }
        if paths.is_empty() {
            return;
        }
        paths.sort();
        self.confirm_delete = Some((pane, paths));
        cx.notify();
    }

    /// Carry out a confirmed delete: Trash locally, or permanently remove on a
    /// remote server (no Trash exists there).
    fn perform_delete(&mut self, cx: &mut Context<Self>) {
        if let Some((pane, paths)) = self.confirm_delete.take() {
            {
                let tab = self.tab_mut(pane);
                tab.selection.clear();
                tab.anchor = None;
            }
            if let Some(server) = self.tab(pane).remote.clone() {
                let use_system = prefs().ssh_use_system;
                let remote_paths: Vec<String> =
                    paths.iter().map(|p| p.to_string_lossy().into_owned()).collect();
                self.run_remote(
                    pane,
                    move || {
                        for rp in &remote_paths {
                            sftp_delete(&server, rp, use_system)?;
                        }
                        Ok(())
                    },
                    cx,
                );
            } else {
                self.trash_items(pane, &paths, cx);
            }
        }
        cx.notify();
    }

    /// Move local items to the Trash, remembering where each went so ⌘Z can
    /// put them back.
    fn trash_items(&mut self, pane: usize, paths: &[PathBuf], cx: &mut Context<Self>) {
        let moved: Vec<(PathBuf, PathBuf)> = paths
            .iter()
            .filter_map(|p| trash_path(p).map(|t| (p.clone(), t)))
            .collect();
        if moved.is_empty() {
            return;
        }
        if moved.iter().all(|(_, t)| !t.as_os_str().is_empty()) {
            self.push_undo(UndoOp::Trashed(moved));
        }
        self.refresh_pane(pane, cx);
    }

    /// Re-read every pane's directory (after a move that may affect two panes).
    fn refresh_all_panes(&mut self, cx: &mut Context<Self>) {

        for p in 0..self.panes.len() {
            self.reload_pane(p, cx);
        }
    }

    /// Move `srcs` into `dest_dir` (drag-and-drop between folders/panes, or
    /// "Move to Other Pane") as one undoable background job. Skips items
    /// already there, a folder into itself/its own descendant, and anything
    /// whose name is taken in the destination (never clobbers).
    fn move_items(&mut self, dest_dir: PathBuf, srcs: Vec<PathBuf>, cx: &mut Context<Self>) {
        if !dest_dir.is_dir() {
            return;
        }
        let pairs: Vec<(PathBuf, PathBuf)> = srcs
            .into_iter()
            .filter(|src| {
                src.parent() != Some(dest_dir.as_path()) && dest_dir != *src && !dest_dir.starts_with(src)
            })
            .filter_map(|src| {
                let dest = dest_dir.join(src.file_name()?);
                (fs::symlink_metadata(&dest).is_err()).then_some((src, dest))
            })
            .collect();
        if pairs.is_empty() {
            return;
        }
        let label = format!("Moving {} to {}", items_label(pairs.len()), path_label(&dest_dir));
        self.start_transfer(pairs, TransferKind::Move, label, true, cx);
    }

    /// Copy `srcs` into `dest_dir` as one undoable background job; a taken
    /// name gets a Finder-style " 2" suffix instead of overwriting.
    fn copy_items(&mut self, dest_dir: PathBuf, srcs: Vec<PathBuf>, cx: &mut Context<Self>) {
        let mut taken: HashSet<PathBuf> = HashSet::new();
        let pairs: Vec<(PathBuf, PathBuf)> = srcs
            .into_iter()
            .filter(|src| !dest_dir.starts_with(src))
            .filter_map(|src| {
                let name = src.file_name()?.to_string_lossy().into_owned();
                let mut dest = unique_dest(&dest_dir, &name);
                // Two sources with the same name in one batch.
                let mut n = 2;
                while taken.contains(&dest) {
                    dest = unique_dest(&dest_dir, &format!("{name} {n}"));
                    n += 1;
                }
                taken.insert(dest.clone());
                Some((src, dest))
            })
            .collect();
        if pairs.is_empty() {
            return;
        }
        let label = format!("Copying {} to {}", items_label(pairs.len()), path_label(&dest_dir));
        self.start_transfer(pairs, TransferKind::Copy, label, true, cx);
    }

    /// Run a copy/move off the main thread. The progress panel shows it once
    /// it has taken longer than a blink; when it finishes the panes reload and
    /// (if `undoable`) the completed part lands on the undo stack.
    fn start_transfer(
        &mut self,
        pairs: Vec<(PathBuf, PathBuf)>,
        kind: TransferKind,
        label: String,
        undoable: bool,
        cx: &mut Context<Self>,
    ) {
        let prog = Arc::new(JobProgress::default());
        self.next_job_id += 1;
        let id = self.next_job_id;
        let was_idle = self.jobs.is_empty();
        self.jobs.push(FileJob { id, label, started: Instant::now(), prog: prog.clone() });
        if was_idle {
            // Repaint the progress panel while anything is running.
            cx.spawn(async move |this, cx| loop {
                cx.background_executor().timer(Duration::from_millis(150)).await;
                let running = this.update(cx, |this, cx| {
                    cx.notify();
                    !this.jobs.is_empty()
                });
                if !matches!(running, Ok(true)) {
                    break;
                }
            })
            .detach();
        }
        cx.spawn(async move |this, cx| {
            let (done, err) = cx.background_spawn(async move { run_transfer(&pairs, kind, &prog) }).await;
            let _ = this.update(cx, |this, cx| {
                this.jobs.retain(|j| j.id != id);
                if undoable && !done.is_empty() {
                    this.push_undo(UndoOp::Transfer { kind, pairs: done });
                }
                if let Some(e) = err {
                    this.remote_error = Some(e);
                }
                this.refresh_all_panes(cx);
            });
        })
        .detach();
    }

    /// The tab/split layout, one line per fact: split ratio, active pane,
    /// then per pane its active tab index and each local tab's view, sort and
    /// folder (tab-separated; paths can't contain tabs in practice). Remote
    /// tabs aren't restored.
    fn session_string(&self) -> String {
        serde_json::to_string(&self.capture_layout()).unwrap_or_default()
    }

    fn save_session(&mut self) {
        let mut candidate = self.projects.clone();
        candidate.remember_layout(self.capture_layout());
        if candidate != self.projects {
            if let Err(error) = self.persist_library(candidate) { eprintln!("{error}"); }
        }
        let session = self.session_string();
        if session.is_empty() { return; }
        if session != self.saved_session {
            if let Some(path) = config_file("session.json") {
                if projects::atomic_write(&path, session.as_bytes()).is_ok() {
                    self.saved_session = session;
                }
            }
        }
    }

    fn restore_session(&mut self) -> bool {
        let saved = self.projects.active
            .and_then(|id| self.projects.items.iter().find(|p| p.id == id).map(|p| p.layout.clone()))
            .or_else(|| self.projects.personal_layout.clone())
            .or_else(|| config_file("session.json").and_then(|p| fs::read(p).ok())
                .and_then(|bytes| serde_json::from_slice::<projects::Layout>(&bytes).ok()));
        if saved.as_ref().is_some_and(|layout| self.apply_layout(layout)) { return true; }
        self.projects.active = None;
        false
    }

    // ----- Get Info -----

    /// Open the Get Info card for `path`. Cheap facts show at once; a folder's
    /// total size and item count are summed in the background.
    fn open_get_info(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.close_context_menu(cx);
        let md = fs::symlink_metadata(&path).ok();
        let is_dir = md.as_ref().is_some_and(|m| m.is_dir());
        let perms = md.as_ref().map(|m| {
            use std::os::unix::fs::PermissionsExt;
            mode_string(m.permissions().mode(), is_dir)
        });
        self.info_panel = Some(InfoPanel {
            path: path.clone(),
            is_dir,
            info: gather_info(&path),
            perms: perms.unwrap_or_default(),
            owner: String::new(),
            total: if is_dir { None } else { md.as_ref().map(|m| (m.len(), 1)) },
        });
        cx.notify();
        cx.spawn(async move |this, cx| {
            let p = path.clone();
            let (owner, total) = cx
                .background_spawn(async move {
                    let owner = Command::new("stat")
                        .args(["-f", "%Su (%Sg)"])
                        .arg(&p)
                        .output()
                        .ok()
                        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                        .unwrap_or_default();
                    (owner, is_dir.then(|| tree_size_count(&p)))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if let Some(ip) = this.info_panel.as_mut().filter(|ip| ip.path == path) {
                    ip.owner = owner;
                    if total.is_some() {
                        ip.total = total;
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn render_info_panel(&self, cx: &Context<Self>) -> impl IntoElement {
        let t = theme();
        let ip = self.info_panel.as_ref().expect("only when open");
        let name = path_label(&ip.path);
        let size = match ip.total {
            Some((bytes, _)) if !ip.is_dir => format!("{} ({} bytes)", format_size(false, bytes), group_digits(bytes)),
            Some((bytes, items)) => format!("{} on disk · {}", format_size(false, bytes), items_label(items as usize)),
            None => "Calculating…".to_string(),
        };
        let row = |label: &str, value: String| {
            div()
                .flex()
                .gap_3()
                .child(div().flex_none().w(px(84.0)).text_color(rgb(t.text_dim)).child(label.to_string()))
                .child(div().flex_1().min_w_0().text_color(rgb(t.text)).child(value))
        };
        let mut rows = div()
            .flex()
            .flex_col()
            .gap_1p5()
            .text_size(px(UI_SMALL_FONT_SIZE))
            .child(row("Kind", ip.info.kind.clone()))
            .child(row("Size", size))
            .child(row("Where", ip.path.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()))
            .child(row("Created", ip.info.created.clone()))
            .child(row("Modified", ip.info.modified.clone()))
            .child(row("Last opened", ip.info.accessed.clone()));
        if let Some(d) = &ip.info.dimensions {
            rows = rows.child(row("Dimensions", d.clone()));
        }
        if let Some(c) = &ip.info.color {
            rows = rows.child(row("Color", c.clone()));
        }
        if let Some(sig) = &ip.info.signed {
            rows = rows.child(row("Signed", sig.clone()));
        }
        rows = rows
            .child(row("Owner", if ip.owner.is_empty() { "…".into() } else { ip.owner.clone() }))
            .child(row("Permissions", ip.perms.clone()));

        let button = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .px_3()
                .py_1()
                .rounded_md()
                .cursor_pointer()
                .text_size(px(UI_SMALL_FONT_SIZE))
                .text_color(rgb(t.text))
                .bg(rgb(t.hover))
                .hover(|s| s.bg(rgb(t.selected)))
                .child(label)
        };
        let copy_path = ip.path.clone();
        let reveal = ip.path.clone();
        modal_backdrop(cx.listener(|this, _, _, cx| {
            this.info_panel = None;
            cx.notify();
        }))
        .child(
            div()
                .w(px(420.0))
                .flex()
                .flex_col()
                .gap_4()
                .p_5()
                .rounded_lg()
                .bg(rgb(t.surface))
                .border_1()
                .border_color(rgb(t.border_strong))
                .shadow_lg()
                .on_mouse_down(MouseButton::Left, |_, _, cx: &mut App| cx.stop_propagation())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(icon_element_sized(&ip.path, ip.is_dir, 48.0))
                        .child(div().flex_1().min_w_0().text_color(rgb(t.text)).child(name)),
                )
                .child(rows)
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child(button("info-copy", "Copy Path").on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(copy_path.to_string_lossy().into_owned()));
                            this.set_notice("Path copied".into(), cx);
                        })))
                        .child(button("info-reveal", "Reveal in Finder").on_click(move |_, _, _| {
                            let _ = Command::new("open").arg("-R").arg(&reveal).spawn();
                        }))
                        .child(button("info-close", "Done").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.info_panel = None;
                            cx.notify();
                        }))),
                ),
        )
    }

    // ----- Batch rename -----

    fn open_batch_rename(&mut self, pane: usize, targets: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        self.close_context_menu(cx);
        // A single item is just an inline rename.
        if targets.len() == 1 {
            let p = targets[0].clone();
            self.begin_rename(pane, p, window, cx);
            return;
        }
        self.batch_rename = Some(BatchRename {
            pane,
            targets,
            mode: BatchMode::Replace,
            a: String::new(),
            b: String::new(),
            field: 0,
            after: true,
        });
        window.focus(&self.focus);
        cx.notify();
    }

    fn handle_batch_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let ks = &ev.keystroke;
        let cmd = ks.modifiers.platform;
        let Some(br) = self.batch_rename.as_mut() else { return };
        let fields = br.mode.field_count();
        match ks.key.as_str() {
            "escape" => self.batch_rename = None,
            "enter" => {
                self.apply_batch_rename(cx);
                return;
            }
            "tab" => br.field = (br.field + 1) % fields,
            "backspace" => {
                br.field_mut().pop();
            }
            "v" if cmd => {
                if let Some(t) = cx.read_from_clipboard().and_then(|i| i.text()) {
                    br.field_mut().push_str(t.lines().next().unwrap_or(""));
                }
            }
            _ => {
                if cmd {
                    return;
                }
                if let Some(ch) = ks.key_char.as_ref() {
                    if !ch.is_empty() && !ch.chars().any(char::is_control) {
                        br.field_mut().push_str(ch);
                    }
                }
            }
        }
        cx.notify();
    }

    /// Rename every target per the dialog, as one undoable step. Refuses (and
    /// keeps the dialog open) if any new name is invalid or collides.
    fn apply_batch_rename(&mut self, cx: &mut Context<Self>) {
        let Some(br) = self.batch_rename.as_ref() else { return };
        let plan = br.plan();
        if plan.iter().any(|(_, _, problem)| problem.is_some()) {
            cx.notify();
            return;
        }
        let pane = br.pane;
        let moves: Vec<(PathBuf, PathBuf)> = plan
            .into_iter()
            .filter(|(from, to, _)| from != to)
            .map(|(from, to, _)| (from, to))
            .collect();
        self.batch_rename = None;
        // Two passes via temporary names, so swaps/cycles ("a"→"b", "b"→"a")
        // can't trip over each other.
        let mut staged: Vec<(PathBuf, PathBuf, PathBuf)> = Vec::new();
        for (i, (from, to)) in moves.iter().enumerate() {
            let tmp = from.with_file_name(format!(".shuffle-rename-{}-{i}", std::process::id()));
            if fs::rename(from, &tmp).is_ok() {
                staged.push((from.clone(), tmp, to.clone()));
            }
        }
        let mut done = Vec::new();
        for (from, tmp, to) in staged {
            if fs::rename(&tmp, &to).is_ok() {
                done.push((from, to));
            } else {
                let _ = fs::rename(&tmp, &from);
            }
        }
        if !done.is_empty() {
            let tab = self.tab_mut(pane);
            for (from, to) in &done {
                if tab.selection.remove(from) {
                    tab.selection.insert(to.clone());
                }
            }
            let n = done.len();
            self.push_undo(UndoOp::Renamed(done));
            self.set_notice(format!("Renamed {}", items_label(n)), cx);
        }
        self.refresh_pane(pane, cx);
    }

    fn render_batch_rename(&self, cx: &Context<Self>) -> impl IntoElement {
        let t = theme();
        let br = self.batch_rename.as_ref().expect("only when open");
        let plan = br.plan();
        let blocked = plan.iter().any(|(_, _, p)| p.is_some());

        let mode_tab = |mode: BatchMode, label: &'static str| {
            let on = br.mode == mode;
            div()
                .id(("batch-mode", mode as usize))
                .px_3()
                .py_1()
                .rounded_md()
                .cursor_pointer()
                .text_size(px(UI_SMALL_FONT_SIZE))
                .text_color(rgb(if on { t.text } else { t.text_dim }))
                .when(on, |s| s.bg(rgb(t.selected)))
                .hover(|s| s.text_color(rgb(t.text)))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(br) = this.batch_rename.as_mut() {
                        br.mode = mode;
                        br.field = 0;
                    }
                    cx.notify();
                }))
        };
        let field = |ix: usize, label: &'static str, value: &str| {
            let focused = br.field == ix;
            div()
                .id(("batch-field", ix))
                .flex()
                .items_center()
                .gap_3()
                .text_size(px(UI_SMALL_FONT_SIZE))
                .child(div().flex_none().w(px(80.0)).text_color(rgb(t.text_dim)).child(label))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h(px(24.0))
                        .px_2()
                        .flex()
                        .items_center()
                        .rounded_md()
                        .bg(rgb(t.bg))
                        .border_1()
                        .border_color(rgb(if focused { t.accent } else { t.border_strong }))
                        .text_color(rgb(t.text))
                        .child(value.to_string())
                        .when(focused, |s| s.child(div().flex_none().w(px(1.5)).h(px(14.0)).bg(rgb(t.text)))),
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(br) = this.batch_rename.as_mut() {
                        br.field = ix;
                    }
                    cx.notify();
                }))
        };
        let fields = match br.mode {
            BatchMode::Replace => div()
                .flex()
                .flex_col()
                .gap_2()
                .child(field(0, "Find", &br.a))
                .child(field(1, "Replace with", &br.b)),
            BatchMode::Add => {
                let after = br.after;
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(field(0, "Text", &br.a))
                    .child(
                        div()
                            .id("batch-where")
                            .flex()
                            .gap_3()
                            .text_size(px(UI_SMALL_FONT_SIZE))
                            .child(div().flex_none().w(px(80.0)).text_color(rgb(t.text_dim)).child("Where"))
                            .child(
                                div()
                                    .cursor_pointer()
                                    .text_color(rgb(t.accent))
                                    .child(if after { "after name ▾" } else { "before name ▾" }),
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                if let Some(br) = this.batch_rename.as_mut() {
                                    br.after = !br.after;
                                }
                                cx.notify();
                            })),
                    )
            }
            BatchMode::Number => div()
                .flex()
                .flex_col()
                .gap_2()
                .child(field(0, "Name", &br.a))
                .child(field(1, "Start at", &br.b)),
        };

        let mut preview = div().flex().flex_col().gap_0p5().text_size(px(UI_SMALL_FONT_SIZE));
        for (from, to, problem) in plan.iter().take(8) {
            let bad = problem.is_some();
            preview = preview.child(
                div()
                    .flex()
                    .gap_2()
                    .child(div().flex_1().min_w_0().truncate().text_color(rgb(t.text_dim)).child(path_label(from)))
                    .child(div().flex_none().text_color(rgb(t.text_dim)).child("→"))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(rgb(if bad { RENAME_ERR_COLOR } else { t.text }))
                            .child(problem.map(String::from).unwrap_or_else(|| path_label(to))),
                    ),
            );
        }
        if plan.len() > 8 {
            preview = preview.child(div().text_color(rgb(t.text_dim)).child(format!("…and {} more", plan.len() - 8)));
        }

        let n = br.targets.len();
        modal_backdrop(cx.listener(|this, _, _, cx| {
            this.batch_rename = None;
            cx.notify();
        }))
        .child(
            div()
                .w(px(480.0))
                .flex()
                .flex_col()
                .gap_4()
                .p_5()
                .rounded_lg()
                .bg(rgb(t.surface))
                .border_1()
                .border_color(rgb(t.border_strong))
                .shadow_lg()
                .on_mouse_down(MouseButton::Left, |_, _, cx: &mut App| cx.stop_propagation())
                .child(div().text_color(rgb(t.text)).child(format!("Rename {n} Items")))
                .child(
                    div()
                        .flex()
                        .gap_1()
                        .child(mode_tab(BatchMode::Replace, "Replace Text"))
                        .child(mode_tab(BatchMode::Add, "Add Text"))
                        .child(mode_tab(BatchMode::Number, "Number")),
                )
                .child(fields)
                .child(preview)
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            div()
                                .id("batch-cancel")
                                .px_3()
                                .py_1()
                                .rounded_md()
                                .cursor_pointer()
                                .text_color(rgb(t.text))
                                .bg(rgb(t.hover))
                                .hover(|s| s.bg(rgb(t.selected)))
                                .child("Cancel")
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.batch_rename = None;
                                    cx.notify();
                                })),
                        )
                        .child(
                            div()
                                .id("batch-apply")
                                .px_3()
                                .py_1()
                                .rounded_md()
                                .cursor_pointer()
                                .text_color(rgb(0xffffff))
                                .bg(rgb(t.accent))
                                .when(blocked, |s| s.opacity(0.5))
                                .child("Rename")
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.apply_batch_rename(cx);
                                })),
                        ),
                ),
        )
    }

    fn push_undo(&mut self, op: UndoOp) {
        self.undo_stack.push(op);
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
    }

    /// ⌘Z: reverse the most recent file operation, if everything it touched
    /// is still where it left it (otherwise say why and leave it alone).
    fn undo_last(&mut self, cx: &mut Context<Self>) {
        let Some(op) = self.undo_stack.pop() else {
            self.set_notice("Nothing to undo".into(), cx);
            return;
        };
        let what = op.label();
        let gone = |p: &Path| fs::symlink_metadata(p).is_err();
        match op {
            UndoOp::Transfer { kind: TransferKind::Copy, pairs } => {
                for (_, c) in pairs.iter().filter(|(_, d)| !gone(d)) {
                    trash_path(c);
                }
            }
            UndoOp::Created(paths) => {
                for p in paths.iter().filter(|p| !gone(p)) {
                    trash_path(p);
                }
            }
            UndoOp::Transfer { kind: TransferKind::Move, pairs } | UndoOp::Trashed(pairs) => {
                // Move each item back from where it went (pairs are
                // (original, current)); skip any that moved on since or whose
                // original spot has been taken.
                let back: Vec<(PathBuf, PathBuf)> = pairs
                    .into_iter()
                    .filter(|(orig, now)| !gone(now) && gone(orig))
                    .map(|(orig, now)| (now, orig))
                    .collect();
                if back.is_empty() {
                    self.set_notice(format!("Couldn't undo {what} — the items have moved since"), cx);
                    return;
                }
                self.start_transfer(back, TransferKind::Move, format!("Undoing {what}"), false, cx);
            }
            UndoOp::Renamed(pairs) => {
                // Rename back in reverse order (via temporary names, so a
                // batch that swapped names undoes cleanly too).
                let mut staged = Vec::new();
                for (i, (from, to)) in pairs.iter().enumerate().rev() {
                    let case_only = from.to_string_lossy().to_lowercase() == to.to_string_lossy().to_lowercase();
                    if gone(to) || (!case_only && !gone(from) && !pairs.iter().any(|(_, t)| t == from)) {
                        continue;
                    }
                    let tmp = to.with_file_name(format!(".shuffle-undo-{}-{i}", std::process::id()));
                    if fs::rename(to, &tmp).is_ok() {
                        staged.push((tmp, from.clone(), to.clone()));
                    }
                }
                if staged.is_empty() {
                    self.set_notice(format!("Couldn't undo {what}"), cx);
                    return;
                }
                for (tmp, from, to) in staged {
                    if fs::rename(&tmp, &from).is_err() {
                        let _ = fs::rename(&tmp, &to);
                    }
                }
            }
        }
        self.set_notice(format!("Undid {what}"), cx);
        self.refresh_all_panes(cx);
    }

    /// Floating panel (bottom-right) listing copies/moves that have run for
    /// more than a moment: label, bar, bytes, and a cancel button.
    fn render_jobs_panel(&self) -> Option<AnyElement> {
        let t = theme();
        let visible: Vec<&FileJob> = self
            .jobs
            .iter()
            .filter(|j| j.started.elapsed() >= Duration::from_millis(500))
            .collect();
        if visible.is_empty() {
            return None;
        }
        let mut panel = div()
            .absolute()
            .bottom(px(16.0))
            .right(px(16.0))
            .w(px(340.0))
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .rounded_lg()
            .bg(rgb(t.surface))
            .border_1()
            .border_color(rgb(t.border_strong))
            .shadow_lg()
            .text_size(px(UI_SMALL_FONT_SIZE))
            .text_color(rgb(t.text));
        for j in visible {
            let total = j.prog.total.load(Ordering::Relaxed);
            let done = j.prog.done.load(Ordering::Relaxed).min(total);
            let frac = if total > 0 { done as f32 / total as f32 } else { 0.0 };
            let detail = if total > 0 {
                format!("{} of {}", format_size(false, done), format_size(false, total))
            } else {
                "Preparing…".to_string()
            };
            let prog = j.prog.clone();
            panel = panel.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().flex_1().min_w_0().truncate().child(j.label.clone()))
                            .child(
                                div()
                                    .id(("job-cancel", j.id as usize))
                                    .flex_none()
                                    .px_1()
                                    .rounded_sm()
                                    .cursor_pointer()
                                    .text_color(rgb(t.text_dim))
                                    .hover(|s| s.bg(rgb(t.hover)).text_color(rgb(t.text)))
                                    .child("✕")
                                    .on_click(move |_, _, _| prog.cancel.store(true, Ordering::Relaxed)),
                            ),
                    )
                    .child(
                        div()
                            .h(px(4.0))
                            .w_full()
                            .rounded_full()
                            .bg(rgb(t.border))
                            .child(div().h_full().w(relative(frac)).rounded_full().bg(rgb(t.accent))),
                    )
                    .child(div().text_color(rgb(t.text_dim)).child(detail)),
            );
        }
        Some(panel.into_any_element())
    }

    /// Handle files dropped onto `dest_dir` in `pane`: upload to the server when
    /// the pane is remote, otherwise move locally. Dropped `ExternalPaths` are
    /// always local (from Finder or another pane), so remote = upload.
    fn drop_files(&mut self, pane: usize, dest_dir: PathBuf, srcs: Vec<PathBuf>, cx: &mut Context<Self>) {
        if self.tab(pane).remote.is_some() {
            let locals: Vec<PathBuf> = srcs.into_iter().filter(|p| p.exists()).collect();
            if !locals.is_empty() {
                self.upload_to_remote(pane, dest_dir, locals, cx);
            }
        } else {
            self.move_items(dest_dir, srcs, cx);
        }
    }

    // ----- inline rename -----

    /// Begin renaming `path`: the row's name becomes an editable field.
    fn begin_rename(&mut self, pane: usize, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.active_pane = pane;
        let text = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        // Start with the whole name selected, like Finder.
        let end = text.chars().count();
        self.rename = Some(Rename {
            pane,
            path,
            text,
            cursor: end,
            anchor: if end > 0 { Some(0) } else { None },
        });
        window.focus(&self.focus);
        cx.notify();
    }

    /// Why the current rename text can't be committed (`None` = it can).
    /// Shown live under the pane and blocks Enter while present.
    fn rename_error(&self) -> Option<&'static str> {
        let r = self.rename.as_ref()?;
        let name = r.text.trim();
        if name.is_empty() {
            return None; // Enter on an empty name just cancels, like Finder
        }
        if name.contains('/') {
            return Some("Names can’t contain “/”");
        }
        if name.contains(':') {
            return Some("Names can’t contain “:”");
        }
        if name == "." || name == ".." {
            return Some("That name is reserved");
        }
        if name.len() > 255 {
            return Some("That name is too long");
        }
        let current = r.path.file_name().map(|n| n.to_string_lossy().into_owned());
        // Renaming to itself (or a case-only variant on this case-insensitive
        // filesystem) is fine; anything else that exists is taken.
        let same = current
            .as_deref()
            .is_some_and(|c| c.eq_ignore_ascii_case(name));
        if !same && r.path.parent().is_some_and(|p| p.join(name).exists()) {
            return Some("Another item here already has this name");
        }
        None
    }

    /// Commit the in-progress rename (Enter), renaming the file on disk.
    fn commit_rename(&mut self, cx: &mut Context<Self>) {
        // An invalid name keeps the field open with the error showing — Enter
        // shouldn't silently discard the edit. Escape still cancels.
        if self.rename.is_some() && self.rename_error().is_some() {
            cx.notify();
            return;
        }
        if let Some(r) = self.rename.take() {
            let new = r.text.trim();
            if !new.is_empty() {
                if let Some(parent) = r.path.parent() {
                    let dest = parent.join(new);
                    if dest == r.path {
                        cx.notify();
                        return;
                    }
                    // Remote rename runs over SFTP on a background thread.
                    if let Some(server) = self.tab(r.pane).remote.clone() {
                        let use_system = prefs().ssh_use_system;
                        let from = r.path.to_string_lossy().into_owned();
                        let to = dest.to_string_lossy().into_owned();
                        let pane = r.pane;
                        self.run_remote(
                            pane,
                            move || sftp_rename(&server, &from, &to, use_system),
                            cx,
                        );
                        cx.notify();
                        return;
                    }
                    // Case-only renames are legal even though the destination
                    // "exists" on APFS's case-insensitive default.
                    let case_only = r
                        .path
                        .file_name()
                        .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(new));
                    if (case_only || !dest.exists()) && fs::rename(&r.path, &dest).is_ok() {
                        self.push_undo(UndoOp::Renamed(vec![(r.path.clone(), dest.clone())]));
                        let tab = self.tab_mut(r.pane);
                        if tab.selection.remove(&r.path) {
                            tab.selection.insert(dest.clone());
                        }
                        if tab.anchor.as_deref() == Some(r.path.as_path()) {
                            tab.anchor = Some(dest);
                        }
                        self.refresh_pane(r.pane, cx);
                    }
                }
            }
        }
        cx.notify();
    }

    /// The rename field's selected range as (lo, hi) char indices, if any.
    fn rename_sel(&self) -> Option<(usize, usize)> {
        let r = self.rename.as_ref()?;
        let a = r.anchor?;
        if a == r.cursor {
            return None;
        }
        Some((a.min(r.cursor), a.max(r.cursor)))
    }

    /// Delete the rename selection; returns whether there was one.
    fn rename_delete_sel(&mut self) -> bool {
        if let Some((lo, hi)) = self.rename_sel() {
            if let Some(r) = self.rename.as_mut() {
                let (bl, bh) = (char_byte(&r.text, lo), char_byte(&r.text, hi));
                r.text.replace_range(bl..bh, "");
                r.cursor = lo;
                r.anchor = None;
            }
            true
        } else {
            if let Some(r) = self.rename.as_mut() {
                r.anchor = None;
            }
            false
        }
    }

    /// Insert `s` at the rename cursor, replacing any selection first.
    fn rename_insert(&mut self, s: &str) {
        self.rename_delete_sel();
        if let Some(r) = self.rename.as_mut() {
            let b = char_byte(&r.text, r.cursor);
            r.text.insert_str(b, s);
            r.cursor += s.chars().count();
        }
    }

    /// Move (or extend) the rename cursor. `word` jumps by word, `select`
    /// extends the selection (Option+Shift selects a word at a time).
    fn rename_move_h(&mut self, left: bool, word: bool, select: bool) {
        let Some(r) = self.rename.as_ref() else {
            return;
        };
        let s = r.text.clone();
        let end = s.chars().count();
        let cursor = r.cursor.min(end);
        let target = match (left, word) {
            (true, true) => prev_word_boundary(&s, cursor),
            (true, false) => cursor.saturating_sub(1),
            (false, true) => next_word_boundary(&s, cursor),
            (false, false) => (cursor + 1).min(end),
        };
        let sel = self.rename_sel();
        let Some(r) = self.rename.as_mut() else {
            return;
        };
        if select {
            if r.anchor.is_none() {
                r.anchor = Some(cursor);
            }
            r.cursor = target;
            if r.anchor == Some(target) {
                r.anchor = None;
            }
        } else if let Some((lo, hi)) = sel {
            r.cursor = if word {
                target
            } else if left {
                lo
            } else {
                hi
            };
            r.anchor = None;
        } else {
            r.cursor = target;
            r.anchor = None;
        }
    }

    /// Keystrokes while a rename field is active. Full text editing: arrows
    /// move, Option jumps words, Cmd jumps to the ends, Shift extends the
    /// selection, Cmd+A/C/X/V work on the selection.
    fn handle_rename_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let ks = &ev.keystroke;
        let cmd = ks.modifiers.platform;
        let alt = ks.modifiers.alt;
        let shift = ks.modifiers.shift;
        match ks.key.as_str() {
            "escape" => {
                self.rename = None;
                cx.notify();
            }
            "enter" => self.commit_rename(cx),
            "left" | "right" => {
                let left = ks.key == "left";
                if cmd {
                    if let Some(r) = self.rename.as_mut() {
                        let end = r.text.chars().count();
                        let cursor = r.cursor;
                        if shift && r.anchor.is_none() {
                            r.anchor = Some(cursor);
                        }
                        r.cursor = if left { 0 } else { end };
                        if !shift {
                            r.anchor = None;
                        }
                    }
                } else {
                    self.rename_move_h(left, alt, shift);
                }
                cx.notify();
            }
            "a" if cmd => {
                if let Some(r) = self.rename.as_mut() {
                    let end = r.text.chars().count();
                    if end == 0 {
                        r.anchor = None;
                    } else {
                        r.anchor = Some(0);
                        r.cursor = end;
                    }
                }
                cx.notify();
            }
            "c" if cmd => {
                let text = match self.rename_sel() {
                    Some((lo, hi)) => self.rename.as_ref().map(|r| {
                        r.text[char_byte(&r.text, lo)..char_byte(&r.text, hi)].to_string()
                    }),
                    None => self.rename.as_ref().map(|r| r.text.clone()),
                };
                if let Some(text) = text {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                }
            }
            "x" if cmd => {
                if let Some((lo, hi)) = self.rename_sel() {
                    if let Some(r) = self.rename.as_ref() {
                        let cut =
                            r.text[char_byte(&r.text, lo)..char_byte(&r.text, hi)].to_string();
                        cx.write_to_clipboard(ClipboardItem::new_string(cut));
                    }
                    self.rename_delete_sel();
                    cx.notify();
                }
            }
            "v" if cmd => {
                if let Some(t) = cx.read_from_clipboard().and_then(|i| i.text()) {
                    self.rename_insert(t.trim());
                    cx.notify();
                }
            }
            "backspace" => {
                if !self.rename_delete_sel() {
                    if let Some(r) = self.rename.as_mut() {
                        if r.cursor > 0 {
                            let start = char_byte(&r.text, r.cursor - 1);
                            let stop = char_byte(&r.text, r.cursor);
                            r.text.replace_range(start..stop, "");
                            r.cursor -= 1;
                        }
                    }
                }
                cx.notify();
            }
            "delete" => {
                if !self.rename_delete_sel() {
                    if let Some(r) = self.rename.as_mut() {
                        let end = r.text.chars().count();
                        if r.cursor < end {
                            let start = char_byte(&r.text, r.cursor);
                            let stop = char_byte(&r.text, r.cursor + 1);
                            r.text.replace_range(start..stop, "");
                        }
                    }
                }
                cx.notify();
            }
            _ => {
                if cmd {
                    return;
                }
                if let Some(ch) = ks.key_char.clone() {
                    if !ch.is_empty() && !ch.chars().any(char::is_control) {
                        self.rename_insert(&ch);
                        cx.notify();
                    }
                }
            }
        }
    }

    /// Duplicate a file/folder as "name copy" (Finder-style), recursively.
    fn duplicate_entry(&mut self, pane: usize, path: PathBuf, cx: &mut Context<Self>) {
        let Some(parent) = path.parent() else { return };
        let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let ext = path.extension().map(|e| e.to_string_lossy().into_owned());
        let base = match &ext {
            Some(e) => format!("{stem} copy.{e}"),
            None => format!("{stem} copy"),
        };
        let dest = unique_child(parent, &base);
        let _ = pane;
        let label = format!("Duplicating {}", path_label(&path));
        self.start_transfer(vec![(path, dest)], TransferKind::Copy, label, true, cx);
    }

    /// Make a Finder alias of `path` in the same folder.
    fn make_alias(&mut self, pane: usize, path: PathBuf, cx: &mut Context<Self>) {
        let Some(parent) = path.parent() else { return };
        let script = format!(
            "tell application \"Finder\" to make alias file to (POSIX file \"{}\") at (POSIX file \"{}\")",
            path.to_string_lossy(),
            parent.to_string_lossy()
        );
        let _ = Command::new("osascript").arg("-e").arg(script).status();
        self.refresh_pane(pane, cx);
    }

    /// Compress a file/folder into a `.zip` beside it (Finder uses `ditto`).
    /// Zip a file/folder beside itself, off-thread so a big folder can't
    /// freeze the UI while `ditto` runs.
    fn compress_entry(&mut self, pane: usize, path: PathBuf, cx: &mut Context<Self>) {
        let Some(parent) = path.parent() else { return };
        let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let dest = unique_child(parent, &format!("{stem}.zip"));
        cx.spawn(async move |this, cx| {
            cx.background_spawn(async move {
                let _ = Command::new("ditto")
                    .args(["-c", "-k", "--sequesterRsrc", "--keepParent"])
                    .arg(&path)
                    .arg(&dest)
                    .status();
            })
            .await;
            let _ = this.update(cx, |this, cx| {
                if pane < this.panes.len() {
                    this.refresh_pane(pane, cx);
                }
            });
        })
        .detach();
    }

    /// "Extract Here" (context menu on archives): unpack into a folder named
    /// after the archive, beside it. Runs off-thread; the listing refreshes
    /// when it finishes.
    fn extract_archive(&mut self, pane: usize, path: PathBuf, cx: &mut Context<Self>) {
        let Some(parent) = path.parent() else { return };
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        // "proj.tar.gz" extracts into "proj", not "proj.tar".
        let stem = match archive_suffix(&path) {
            Some(sfx) => name[..name.len() - sfx.len()].to_string(),
            None => return,
        };
        let dest = unique_child(parent, &stem);
        cx.spawn(async move |this, cx| {
            cx.background_spawn(async move {
                if fs::create_dir_all(&dest).is_err() {
                    return;
                }
                if let Some(mut c) = archive_extract_command(&path) {
                    let _ = c.arg(&dest).stdout(Stdio::null()).stderr(Stdio::null()).status();
                }
            })
            .await;
            let _ = this.update(cx, |this, cx| {
                if pane < this.panes.len() {
                    this.refresh_pane(pane, cx);
                }
            });
        })
        .detach();
    }

    /// Run a user script action on `targets`, off the UI thread. The paths are
    /// passed as arguments and as `$SHUFFLE_PATHS`; `$SHUFFLE_DIR` is the pane's
    /// folder (also the working directory). Refreshes the pane when it finishes
    /// (the script may have created/renamed/deleted files).
    fn run_script_action(
        &mut self,
        pane: usize,
        script: PathBuf,
        targets: Vec<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        let dir = self.tab(pane).current_dir.clone();
        let paths_env = targets
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("\n");
        cx.spawn(async move |this, cx| {
            let d = dir.clone();
            cx.background_spawn(async move {
                let _ = Command::new(&script)
                    .args(&targets)
                    .current_dir(&d)
                    .env("SHUFFLE_DIR", &d)
                    .env("SHUFFLE_PATHS", &paths_env)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            })
            .await;
            let _ = this.update(cx, |this, cx| {
                if pane < this.panes.len() {
                    this.refresh_pane(pane, cx);
                }
            });
        })
        .detach();
    }

    /// Open `path` with a specific application.
    fn open_with(&mut self, app: &Path, path: &Path, cx: &mut Context<Self>) {
        let _ = Command::new("open").arg("-a").arg(app).arg(path).spawn();
        self.close_context_menu(cx);
    }

    /// Set (or clear, index 0) the Finder color label / tag on `path`.
    fn set_tag(&mut self, pane: usize, path: PathBuf, label: u8, cx: &mut Context<Self>) {
        let script = format!(
            "tell application \"Finder\" to set label index of (POSIX file \"{}\" as alias) to {label}",
            path.to_string_lossy()
        );
        let _ = Command::new("osascript").arg("-e").arg(script).status();
        self.close_context_menu(cx);
        self.refresh_pane(pane, cx);
    }

    /// Rotate an image in place by `degrees` (Quick Action).
    fn rotate_image(&mut self, pane: usize, path: PathBuf, degrees: i32, cx: &mut Context<Self>) {
        let _ = Command::new("sips").arg("-r").arg(degrees.to_string()).arg(&path).status();
        self.close_context_menu(cx);
        self.refresh_after_edit(pane, path, cx);
    }

    /// Convert an image to another format beside the original (Quick Action).
    /// `fmt` is the sips format name; `ext` the new file extension.
    fn convert_image(&mut self, pane: usize, path: PathBuf, fmt: &str, ext: &str, cx: &mut Context<Self>) {
        if let Some(parent) = path.parent() {
            let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let dest = unique_child(parent, &format!("{stem}.{ext}"));
            let _ = Command::new("sips")
                .args(["-s", "format", fmt])
                .arg(&path)
                .arg("--out")
                .arg(&dest)
                .status();
        }
        self.close_context_menu(cx);
        self.refresh_pane(pane, cx);
    }

    /// Remove an image's background via the native Vision helper, writing a new
    /// transparent PNG beside it. Runs in the background (Vision can take a sec).
    fn remove_background(&mut self, pane: usize, path: PathBuf, cx: &mut Context<Self>) {
        self.close_context_menu(cx);
        let Some(tool) = removebg_path() else { return };
        let Some(parent) = path.parent() else { return };
        let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let dest = unique_child(parent, &format!("{stem} (no background).png"));
        cx.spawn(async move |this, cx| {
            let ok = cx
                .background_spawn(async move {
                    Command::new(&tool)
                        .arg(&path)
                        .arg(&dest)
                        .status()
                        .map(|s| s.success())
                        .unwrap_or(false)
                })
                .await;
            if ok {
                let _ = this.update(cx, |this, cx| this.refresh_pane(pane, cx));
            }
        })
        .detach();
    }

    /// The badge state for an item: syncing if a download/evict is in flight,
    /// online-only if it's a cloud placeholder, else local (no badge). Only
    /// files under a cloud root are stat'd (cheap, cached) — everything else is
    /// free.
    fn cloud_state(&self, path: &Path, is_dir: bool) -> CloudSync {
        if self.cloud_busy.contains(path) {
            return CloudSync::Syncing;
        }
        if !is_dir && cloud_kind(path).is_some() && cached_dataless(path) {
            return CloudSync::OnlineOnly;
        }
        CloudSync::Local
    }

    /// Download (materialize) online-only cloud files to disk. `path` may be a
    /// file or a folder (downloads its online-only descendants). Runs the
    /// `cloudctl` helper per file in the background; badges show ↻ meanwhile.
    fn cloud_download(&mut self, pane: usize, path: PathBuf, cx: &mut Context<Self>) {
        self.start_cloud_file_operation(pane, path, true, cx);
    }

    /// "Sync Now" for a cloud folder: start the provider's sync app if it's
    /// quit (a File Provider store only syncs while it runs) and have the File
    /// Provider daemon re-enumerate the folder, then watch it until the sync
    /// settles and reload once to show the result. The regular folder watcher
    /// can't be relied on here: providers apply remote changes without bumping
    /// the folder's mtime, so without this you'd have to leave and come back.
    fn cloud_sync(&mut self, pane: usize, dir: PathBuf, cx: &mut Context<Self>) {
        self.close_context_menu(cx);
        let Some(tool) = cloudctl_path() else { return };
        let name = cloud_provider_name(&dir);
        // A newer Sync Now (or leaving the folder) retires this session's watch.
        self.cloud_sync_gen += 1;
        let sync_gen = self.cloud_sync_gen;
        self.set_notice_sticky(format!("Syncing changes with {name}…"), cx);
        cx.spawn(async move |this, cx| {
            let d = dir.clone();
            let t = tool.clone();
            let (out, base) = cx
                .background_spawn(async move {
                    // Snapshot before the nudge so changes it triggers count.
                    let base = cloud_fingerprint(&d);
                    (Command::new(&t).arg("sync").arg(&d).output(), base)
                })
                .await;
            let launched = match out {
                Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().starts_with("launched"),
                other => {
                    let err = match other {
                        Ok(o) => String::from_utf8_lossy(&o.stderr).trim().to_string(),
                        Err(e) => format!("Sync failed: {e}"),
                    };
                    let _ = this.update(cx, |this, cx| {
                        if this.cloud_sync_gen == sync_gen {
                            this.notice = None;
                            this.remote_error = Some(err);
                            cx.notify();
                        }
                    });
                    return;
                }
            };
            if launched {
                let _ = this.update(cx, |this, cx| {
                    if this.cloud_sync_gen == sync_gen {
                        this.set_notice_sticky(format!("Started {name} — syncing changes…"), cx);
                    }
                });
            }

            // Watch: poll the listing + in-flight transfers once a second. It's
            // settled once nothing is transferring and the listing has held
            // still for a couple of seconds after a change; with no change at
            // all, give up after a quiet spell (longer when the app just
            // started and has to catch up). Hard cap so it can't run forever.
            let started = Instant::now();
            let quiet = Duration::from_secs(if launched { 30 } else { 8 });
            const SETTLE: Duration = Duration::from_secs(2);
            const CAP: Duration = Duration::from_secs(300);
            let mut last = base.clone();
            let mut last_change: Option<Instant> = None;
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                // Still the live session, still looking at this folder?
                let live = this.update(cx, |this, cx| {
                    let ok = this.cloud_sync_gen == sync_gen
                        && pane < this.panes.len()
                        && this.tab(pane).current_dir == dir;
                    if !ok && this.cloud_sync_gen == sync_gen {
                        this.notice = None;
                        cx.notify();
                    }
                    ok
                });
                if !matches!(live, Ok(true)) {
                    return;
                }
                let d = dir.clone();
                let t = tool.clone();
                let (fp, busy) = cx
                    .background_spawn(async move { (cloud_fingerprint(&d), cloud_busy_count(&t, &d)) })
                    .await;
                if fp != last {
                    last = fp;
                    last_change = Some(Instant::now());
                }
                let changes = fingerprint_diff(&base, &last);
                let settled = busy == 0 && last_change.is_some_and(|t| t.elapsed() >= SETTLE);
                let idle = busy == 0 && last_change.is_none() && started.elapsed() >= quiet;
                let timed_out = started.elapsed() >= CAP;
                let _ = this.update(cx, |this, cx| {
                    if settled || idle || timed_out {
                        this.refresh_pane(pane, cx);
                        let msg = if timed_out && (busy > 0 || !settled) {
                            format!("{name} is still syncing in the background — folder reloaded with what's arrived so far.")
                        } else if changes == 0 {
                            format!("Up to date with {name}.")
                        } else {
                            let s = if changes == 1 { "" } else { "s" };
                            format!("Synced {changes} change{s} from {name} — folder updated.")
                        };
                        this.set_notice(msg, cx);
                    } else {
                        let mut msg = format!("Syncing changes with {name}…");
                        if busy > 0 {
                            msg.push_str(&format!(" {busy} item{} transferring", if busy == 1 { "" } else { "s" }));
                        } else if changes > 0 {
                            msg.push_str(&format!(" {changes} so far"));
                        }
                        this.set_notice_sticky(msg, cx);
                    }
                });
                if settled || idle || timed_out {
                    return;
                }
            }
        })
        .detach();
    }

    /// Show a cloud status banner that clears itself after a few seconds.
    fn set_notice(&mut self, msg: String, cx: &mut Context<Self>) {
        self.set_notice_sticky(msg, cx);
        let gen_ = self.notice_gen;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(6)).await;
            let _ = this.update(cx, |this, cx| {
                if this.notice_gen == gen_ {
                    this.notice = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Show a cloud status banner that stays until replaced (cancels any
    /// pending auto-dismiss of the previous one).
    fn set_notice_sticky(&mut self, msg: String, cx: &mut Context<Self>) {
        self.notice_gen += 1;
        if self.notice.as_deref() != Some(msg.as_str()) {
            self.notice = Some(msg);
            cx.notify();
        }
    }

    /// Free up space: evict downloaded cloud files back to online-only. iCloud
    /// only (third-party eviction is driven by the provider's own app). `path`
    /// may be a file or a folder (evicts its materialized descendants).
    fn cloud_evict(&mut self, pane: usize, path: PathBuf, cx: &mut Context<Self>) {
        self.start_cloud_file_operation(pane, path, false, cx);
    }

    /// Set the file as the desktop picture on every display (Service).
    fn set_desktop_picture(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let script = format!(
            "tell application \"System Events\" to set picture of every desktop to \"{}\"",
            path.to_string_lossy()
        );
        let _ = Command::new("osascript").arg("-e").arg(script).status();
        self.close_context_menu(cx);
    }

    /// After an in-place edit, drop stale caches and refresh the listing.
    fn refresh_after_edit(&mut self, pane: usize, path: PathBuf, cx: &mut Context<Self>) {
        if let Some(cancel)=self.preview_pending.borrow_mut().remove(&path) {cancel.store(true,Ordering::Release);}
        TEXT_PREVIEW_CACHE.with(|c| { c.borrow_mut().remove(&path); });
        PREVIEW_CACHE.with(|c| {
            c.borrow_mut().remove(&path);
        });
        INFO_CACHE.with(|c| {
            c.borrow_mut().remove(&path);
        });
        self.refresh_pane(pane, cx);
        if self.tab(pane).anchor.as_deref() == Some(path.as_path()) {
            self.ensure_preview(path.clone(), false, cx);
            self.ensure_info(path, cx);
        }
    }

    /// Build the context-menu tree for a local tab. `target` is the
    /// right-clicked entry (`None` = the background of the folder).
    fn menu_nodes_root(&self, pane: usize, target: Option<(PathBuf, bool)>) -> Vec<MenuNode> {
        let mut nodes: Vec<MenuNode> = Vec::new();
        let paste_n = pasteboard_file_paths().len();
        let on_background = target.is_none();
        let inspect = target.as_ref().and_then(|(path,is_dir)| is_dir.then_some(path.clone()))
            .or_else(|| on_background.then(|| self.tab(pane).current_dir.clone()));
        if let Some(folder) = inspect {
            let view_folder = folder.clone();
            nodes.push(mi("Apply view to folder + subfolders", move |this, _, cx| this.apply_view_to_tree(pane, view_folder.clone(), cx)));
            nodes.push(mi("Speicherbelegung…", move |this,window,cx| this.inspect_folder(pane,folder.clone(),window,cx)));
            push_menu_sep(&mut nodes);
        }
        if let Some((path, is_dir)) = target {
            // Clicking inside a multi-selection acts on the whole selection
            // (Finder semantics); clicking outside it acts on just that item.
            let sel = &self.tab(pane).selection;
            let targets: Vec<PathBuf> = if sel.len() > 1 && sel.contains(&path) {
                let mut v: Vec<PathBuf> = sel.iter().cloned().collect();
                v.sort();
                v
            } else {
                vec![path.clone()]
            };
            let n = targets.len();
            let many = n > 1;

            // --- open / preview ---
            {
                let ts = targets.clone();
                let label = if many { format!("Open {n} Items") } else { "Open".to_string() };
                nodes.push(mi(label, move |this, _, cx| {
                    this.close_context_menu(cx);
                    if ts.len() == 1 {
                        this.open_path(pane, ts[0].clone(), is_dir, cx);
                    } else {
                        // Multi-open goes through LaunchServices, like Finder.
                        for p in &ts {
                            let _ = Command::new("open").arg(p).spawn();
                        }
                    }
                }));
            }
            if !many {
                nodes.push(MenuNode::Sub {
                    label: "Open With".into(),
                    items: self.open_with_nodes(&path),
                });
            }
            {
                let ts = targets.clone();
                let label = if many { format!("Quick Look {n} Items") } else { "Quick Look".to_string() };
                nodes.push(mi_hint(label, "␣", move |this, _, cx| {
                    this.close_context_menu(cx);
                    quick_look_paths(&ts);
                }));
            }
            push_menu_sep(&mut nodes);

            // --- cloud + type-specific (single target only) ---
            if !many {
                if let Some(menu) = self.cloud_menu(pane, &path, is_dir) { nodes.push(menu); }
                // Type-specific actions, Finder-style: archives extract in
                // place, app bundles reveal their contents.
                if archive_suffix(&path).is_some() {
                    let p = path.clone();
                    nodes.push(mi("Extract Here", move |this, _, cx| {
                        this.close_context_menu(cx);
                        this.extract_archive(pane, p.clone(), cx);
                    }));
                }
                if is_app_bundle(&path, is_dir) {
                    let p = path.clone();
                    nodes.push(mi("Show Package Contents", move |this, _, cx| {
                        this.close_context_menu(cx);
                        this.navigate_in(pane, p.clone(), cx);
                    }));
                }
            }

            // User script actions that apply to the clicked item; they receive
            // the whole selection as arguments.
            if prefs().script_actions {
                let fname = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let acts: Vec<ScriptAction> = discover_script_actions()
                    .into_iter()
                    .filter(|a| script_action_applies(a, &fname, is_dir))
                    .collect();
                for a in acts.into_iter().take(12) {
                    let script = a.path.clone();
                    let ts = targets.clone();
                    nodes.push(mi(a.name, move |this, _, cx| {
                        this.close_context_menu(cx);
                        this.run_script_action(pane, script.clone(), ts.clone(), cx);
                    }));
                }
            }
            push_menu_sep(&mut nodes);

            // --- clipboard ---
            {
                let ts = targets.clone();
                let label = if many { format!("Copy {n} Items") } else { "Copy".to_string() };
                nodes.push(mi(label, move |this, _, cx| {
                    this.close_context_menu(cx);
                    this.copy_with_feedback(pane, &ts, cx);
                }));
            }
            {
                let ts = targets.clone();
                let label = if many { "Copy Paths" } else { "Copy Path" };
                nodes.push(mi(label, move |this, _, cx| {
                    let text = ts
                        .iter()
                        .map(|p| p.to_string_lossy().into_owned())
                        .collect::<Vec<_>>()
                        .join("\n");
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                    this.close_context_menu(cx);
                }));
            }
            if !many && is_dir && paste_n > 0 {
                let d = path.clone();
                let label = if paste_n == 1 {
                    "Paste 1 Item Into Folder".to_string()
                } else {
                    format!("Paste {paste_n} Items Into Folder")
                };
                nodes.push(mi(label, move |this, _, cx| {
                    this.close_context_menu(cx);
                    this.paste_into(pane, d.clone(), cx);
                }));
            }
            push_menu_sep(&mut nodes);

            // --- file ops ---
            if !many {
                let p = path.clone();
                nodes.push(mi_hint("Rename", "↩", move |this, window, cx| {
                    this.close_context_menu(cx);
                    this.begin_rename(pane, p.clone(), window, cx);
                }));
            }
            {
                let ts = targets.clone();
                let label = if many { format!("Duplicate {n} Items") } else { "Duplicate".to_string() };
                nodes.push(mi(label, move |this, _, cx| {
                    this.close_context_menu(cx);
                    for p in &ts {
                        this.duplicate_entry(pane, p.clone(), cx);
                    }
                }));
            }
            if !many {
                let p = path.clone();
                nodes.push(mi("Make Alias", move |this, _, cx| {
                    this.close_context_menu(cx);
                    this.make_alias(pane, p.clone(), cx);
                }));
            }
            {
                let ts = targets.clone();
                let label = if many { format!("Compress {n} Items") } else { "Compress".to_string() };
                nodes.push(mi(label, move |this, _, cx| {
                    this.close_context_menu(cx);
                    this.compress_targets(pane, ts.clone(), cx);
                }));
            }
            {
                // The native share popover (AirDrop, Messages, Mail, Notes, …),
                // anchored where the menu was opened — same as Finder's Share….
                let ts = targets.clone();
                nodes.push(mi("Share…", move |this, window, cx| {
                    let at = this.context_menu.as_ref().map(|m| (m.x, m.y)).unwrap_or_default();
                    this.close_context_menu(cx);
                    if let Some(view) = ns_view_ptr(window) {
                        show_share_picker(view, at, &ts);
                    }
                }));
            }
            if !many {
                let p = path.clone();
                nodes.push(mi("Get Info", move |this, _, cx| {
                    this.open_get_info(p.clone(), cx);
                }));
            } else {
                let ts = targets.clone();
                nodes.push(mi(format!("Rename {n} Items…"), move |this, window, cx| {
                    this.open_batch_rename(pane, ts.clone(), window, cx);
                }));
            }
            // Split view: send the selection straight to the other pane's folder.
            if self.panes.len() > 1 {
                let other = 1 - pane.min(1);
                let other_tab = self.tab(other);
                if other_tab.remote.is_none() {
                    let dest = other_tab.current_dir.clone();
                    let where_ = path_label(&dest);
                    let (ts, d) = (targets.clone(), dest.clone());
                    nodes.push(mi(format!("Copy to “{where_}”"), move |this, _, cx| {
                        this.close_context_menu(cx);
                        this.copy_items(d.clone(), ts.clone(), cx);
                    }));
                    let (ts, d) = (targets.clone(), dest);
                    nodes.push(mi(format!("Move to “{where_}”"), move |this, _, cx| {
                        this.close_context_menu(cx);
                        this.move_items(d.clone(), ts.clone(), cx);
                    }));
                }
            }
            if !many {
                let p = path.clone();
                nodes.push(mi("Reveal in Finder", move |this, _, cx| {
                    this.close_context_menu(cx);
                    let _ = Command::new("open").arg("-R").arg(&p).spawn();
                }));
                let p = path.clone();
                let already = self.projects.sidebar().locations.contains(&p);
                let label = if already { "Remove from Locations" } else { "Add to Locations" };
                nodes.push(mi(label, move |this, _, cx| {
                    this.close_context_menu(cx);
                    this.set_project_path(projects::Section::Locations, p.clone(), !already, cx);
                }));
                // "Add to Group ▸" (only when groups are enabled and exist).
                if prefs().groups_enabled && !self.groups.is_empty() {
                    nodes.push(MenuNode::Sub {
                        label: "Add to Group".into(),
                        items: self.add_to_group_nodes(&path),
                    });
                }
            }
            push_menu_sep(&mut nodes);

            // --- tags / actions ---
            nodes.push(MenuNode::Sub {
                label: "Tags".into(),
                items: self.tags_nodes(pane, &targets),
            });
            if !many {
                nodes.push(MenuNode::Sub {
                    label: "Quick Actions".into(),
                    items: self.quick_actions_nodes(pane, &path),
                });
                nodes.push(MenuNode::Sub {
                    label: "Services".into(),
                    items: self.services_nodes(&path, is_dir),
                });
            }
            push_menu_sep(&mut nodes);

            // --- destructive, at the bottom like Finder ---
            {
                let ts = targets.clone();
                let label = if many { format!("Move {n} Items to Trash") } else { "Move to Trash".to_string() };
                nodes.push(mi_danger(label, Some("⌫"), move |this, _, cx| {
                    this.close_context_menu(cx);
                    this.trash_items(pane, &ts, cx);
                }));
            }
            push_menu_sep(&mut nodes);
        } else if paste_n > 0 {
            let label = if paste_n == 1 { "Paste 1 Item".to_string() } else { format!("Paste {paste_n} Items") };
            nodes.push(mi(label, move |this, _, cx| {
                this.close_context_menu(cx);
                let dir = this.tab(pane).current_dir.clone();
                this.paste_into(pane, dir, cx);
            }));
            push_menu_sep(&mut nodes);
        }
        // Background of a cloud folder: ask the provider to sync it.
        if on_background {
            let dir = self.tab(pane).current_dir.clone();
            if let Some(menu) = self.cloud_menu(pane, &dir, true) {
                nodes.push(menu); push_menu_sep(&mut nodes);
            }
        }
        nodes.push(mi("New Folder", move |this, window, cx| {
            this.close_context_menu(cx);
            this.new_folder(pane, window, cx);
        }));
        nodes.push(mi("New Text File…", move |this, window, cx| {
            this.open_text_file_dialog(pane, window, cx);
        }));
        nodes.push(mi("New File", move |this, window, cx| {
            this.close_context_menu(cx);
            this.new_file(pane, window, cx);
        }));
        nodes
    }

    /// The right-click menu for a remote (SFTP) tab: only actions that work
    /// over the network.
    fn menu_nodes_remote(&self, pane: usize, target: Option<(PathBuf, bool)>) -> Vec<MenuNode> {
        let mut nodes: Vec<MenuNode> = Vec::new();
        if let Some((path, is_dir)) = target {
            let p = path.clone();
            let label = if is_dir { "Open" } else { "Open (Download)" };
            nodes.push(mi(label, move |this, _, cx| {
                this.close_context_menu(cx);
                this.open_path(pane, p.clone(), is_dir, cx);
            }));
            if !is_dir {
                let p = path.clone();
                nodes.push(mi("Download to Downloads", move |this, _, cx| {
                    this.close_context_menu(cx);
                    this.download_remote(pane, p.clone(), None, false, cx);
                }));
            }
            nodes.push(MenuNode::Sep);
            let p = path.clone();
            nodes.push(mi_hint("Rename", "↩", move |this, window, cx| {
                this.close_context_menu(cx);
                this.begin_rename(pane, p.clone(), window, cx);
            }));
            let p = path.clone();
            nodes.push(mi("Copy Path", move |this, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(p.to_string_lossy().into_owned()));
                this.close_context_menu(cx);
            }));
            nodes.push(MenuNode::Sep);
            let p = path.clone();
            nodes.push(mi_danger("Delete from Server", Some("⌫"), move |this, _, cx| {
                this.close_context_menu(cx);
                this.confirm_delete = Some((pane, vec![p.clone()]));
                cx.notify();
            }));
            nodes.push(MenuNode::Sep);
        }
        nodes.push(mi("New Folder", move |this, window, cx| {
            this.close_context_menu(cx);
            this.new_folder(pane, window, cx);
        }));
        nodes.push(mi("New File", move |this, window, cx| {
            this.close_context_menu(cx);
            this.new_file(pane, window, cx);
        }));
        nodes
    }

    /// "Open With" flyout — apps that can open the target (via LaunchServices).
    fn open_with_nodes(&self, path: &Path) -> Vec<MenuNode> {
        let apps = apps_for_file(path);
        if apps.is_empty() {
            return vec![MenuNode::Disabled("No applications".into())];
        }
        apps.into_iter()
            .map(|(name, app)| {
                let p = path.to_path_buf();
                mi(name, move |this, _, cx| {
                    this.open_with(&app, &p, cx);
                })
            })
            .collect()
    }

    /// "Add to Group" flyout — one row per group, ✓ when already a member.
    fn add_to_group_nodes(&self, path: &Path) -> Vec<MenuNode> {
        if self.groups.is_empty() {
            return vec![MenuNode::Disabled("No groups".into())];
        }
        self.groups
            .iter()
            .enumerate()
            .map(|(i, g)| {
                let p = path.to_path_buf();
                let has = g.paths.contains(&p);
                mi_check(g.name.clone(), has, move |this, _, cx| {
                    this.close_context_menu(cx);
                    this.add_to_group(i, p.clone(), cx);
                })
            })
            .collect()
    }

    /// "Tags" flyout — Finder color labels, applied to the whole selection.
    fn tags_nodes(&self, pane: usize, targets: &[PathBuf]) -> Vec<MenuNode> {
        // (name, color dot, Finder label index)
        const TAGS: &[(&str, u32, u8)] = &[
            ("None", 0x6b6b73, 0),
            ("Red", 0xff5f57, 6),
            ("Orange", 0xff9f0a, 7),
            ("Yellow", 0xffd60a, 5),
            ("Green", 0x34c759, 2),
            ("Blue", 0x0a84ff, 4),
            ("Purple", 0xbf5af0, 3),
            ("Gray", 0x8e8e93, 1),
        ];
        TAGS.iter()
            .map(|&(name, color, label)| {
                let ts = targets.to_vec();
                mi_dot(name, color, move |this, _, cx| {
                    for p in &ts {
                        this.set_tag(pane, p.clone(), label, cx);
                    }
                })
            })
            .collect()
    }

    /// "Quick Actions" flyout — image/PDF operations via built-in tools.
    fn quick_actions_nodes(&self, pane: usize, path: &Path) -> Vec<MenuNode> {
        let mut items: Vec<MenuNode> = Vec::new();
        let img = is_image(path);
        let pdf = is_pdf(path);

        if img {
            let p = path.to_path_buf();
            items.push(mi("Rotate Left", move |this, _, cx| {
                this.rotate_image(pane, p.clone(), -90, cx);
            }));
            let p = path.to_path_buf();
            items.push(mi("Rotate Right", move |this, _, cx| {
                this.rotate_image(pane, p.clone(), 90, cx);
            }));
        }
        if img || pdf {
            let p = path.to_path_buf();
            items.push(mi("Markup", move |this, _, cx| {
                this.close_context_menu(cx);
                let _ = Command::new("open").arg("-a").arg("Preview").arg(&p).spawn();
            }));
        }
        if img {
            let p = path.to_path_buf();
            items.push(mi("Create PDF", move |this, _, cx| {
                this.convert_image(pane, p.clone(), "pdf", "pdf", cx);
            }));
            // Convert Image to … (sips formats).
            for (label, fmt, ext) in [
                ("Convert to JPEG", "jpeg", "jpg"),
                ("Convert to PNG", "png", "png"),
                ("Convert to HEIC", "heic", "heic"),
            ] {
                let (fmt, ext) = (fmt.to_string(), ext.to_string());
                let p = path.to_path_buf();
                items.push(mi(label, move |this, _, cx| {
                    this.convert_image(pane, p.clone(), &fmt, &ext, cx);
                }));
            }
            // Remove Background (native Vision helper, if compiled in).
            if removebg_path().is_some() {
                let p = path.to_path_buf();
                items.push(mi("Remove Background", move |this, _, cx| {
                    this.remove_background(pane, p.clone(), cx);
                }));
            }
        }
        if items.is_empty() {
            items.push(MenuNode::Disabled("No quick actions".into()));
        }
        items
    }

    /// "Services" flyout — a useful, implementable subset of Finder's services.
    fn services_nodes(&self, path: &Path, is_dir: bool) -> Vec<MenuNode> {
        let mut items: Vec<MenuNode> = Vec::new();
        if is_image(path) {
            let p = path.to_path_buf();
            items.push(mi("Set Desktop Picture", move |this, _, cx| {
                this.set_desktop_picture(p.clone(), cx);
            }));
        }
        // "Open in <terminal>" — opens the folder (or the file's folder).
        let dir = if is_dir {
            path.to_path_buf()
        } else {
            path.parent().map(Path::to_path_buf).unwrap_or_else(|| path.to_path_buf())
        };
        for (name, app) in installed_terminals() {
            let d = dir.clone();
            items.push(mi(format!("Open in {name}"), move |this, _, cx| {
                this.close_context_menu(cx);
                let _ = Command::new("open").arg("-a").arg(&app).arg(&d).spawn();
            }));
        }
        if items.is_empty() {
            items.push(MenuNode::Disabled("No services".into()));
        }
        items
    }

    /// Paste the pasteboard's files into `dir` (background copy, then refresh).
    fn paste_into(&mut self, _pane: usize, dir: PathBuf, cx: &mut Context<Self>) {
        let srcs = pasteboard_file_paths();
        if srcs.is_empty() {
            return;
        }
        self.copy_items(dir, srcs, cx);
    }

    /// Compress one entry (ditto, like before) or several into one Archive.zip.
    fn compress_targets(&mut self, pane: usize, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if paths.len() == 1 {
            let p = paths.into_iter().next().unwrap();
            return self.compress_entry(pane, p, cx);
        }
        let Some(parent) = paths.first().and_then(|p| p.parent()).map(Path::to_path_buf) else {
            return;
        };
        let dest = unique_child(&parent, "Archive.zip");
        cx.spawn(async move |this, cx| {
            cx.background_spawn(async move {
                let names: Vec<String> = paths
                    .iter()
                    .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                    .collect();
                // ditto can't zip several roots into one archive; zip can.
                let _ = Command::new("zip")
                    .current_dir(&parent)
                    .args(["-r", "-q", "-y"])
                    .arg(&dest)
                    .args(&names)
                    .status();
            })
            .await;
            let _ = this.update(cx, |this, cx| {
                if pane < this.panes.len() {
                    this.refresh_pane(pane, cx);
                }
            });
        })
        .detach();
    }

    /// One row of the context menu. `sub` is `None` for root-panel rows,
    /// `Some(j)` for the j-th row of the open flyout under root index `root`.

    /// The "Sort By" dropdown (the Finder-style arrange list).


    /// The "Move to Trash" confirmation modal.
    fn render_confirm_delete(&self, cx: &Context<Self>) -> impl IntoElement {
        let t = theme();
        let (_, paths) = self.confirm_delete.as_ref().expect("only when open");
        let msg = if paths.len() == 1 {
            format!("Move “{}” to the Trash?", path_label(&paths[0]))
        } else {
            format!("Move {} items to the Trash?", paths.len())
        };

        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x00000066))
            .occlude()
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                this.confirm_delete = None;
                cx.notify();
            }))
            .child(
                div()
                    .w(px(360.0))
                    .flex()
                    .flex_col()
                    .gap_4()
                    .p_5()
                    .rounded_lg()
                    .bg(rgb(t.surface))
                    .border_1()
                    .border_color(rgb(t.border_strong))
                    .shadow_lg()
                    .on_mouse_down(MouseButton::Left, |_, _, cx: &mut App| cx.stop_propagation())
                    .child(div().text_color(rgb(t.text)).child(msg))
                    .child(
                        div()
                            .text_size(px(UI_SMALL_FONT_SIZE))
                            .text_color(rgb(t.text_muted))
                            .child("They can be recovered from the Trash."),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                div()
                                    .id("del-cancel")
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .text_color(rgb(t.text))
                                    .bg(rgb(t.hover))
                                    .hover(|s| s.bg(rgb(t.selected)))
                                    .child("Cancel")
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.confirm_delete = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .id("del-confirm")
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .text_color(rgb(0xffffff))
                                    .bg(rgb(0xd9544f))
                                    .hover(|s| s.bg(rgb(0xc6433e)))
                                    .child("Move to Trash")
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.perform_delete(cx);
                                    })),
                            ),
                    ),
            )
    }

    // ----- Connect to Server -----

    fn open_server_dialog(&mut self, cx: &mut Context<Self>) {
        // First run: ask how to authenticate over SSH (a permission-style
        // prompt), then open the connect dialog. Afterwards, go straight in.
        if !prefs().ssh_configured {
            self.ssh_ask = true;
        } else {
            let mut form = ServerForm::default();
            form.mode = if prefs().ssh_use_system {
                ServerMode::Quick
            } else {
                ServerMode::Credentials
            };
            self.server_dialog = Some(form);
        }
        cx.notify();
    }

    /// Answer the first-run SSH prompt, remember the default, and open the
    /// connect dialog on the matching tab.
    fn choose_ssh_mode(&mut self, use_system: bool, cx: &mut Context<Self>) {
        let mut np = prefs();
        np.ssh_use_system = use_system;
        np.ssh_configured = true;
        apply_prefs(np, cx);
        self.ssh_ask = false;
        let mut form = ServerForm::default();
        form.mode = if use_system {
            ServerMode::Quick
        } else {
            ServerMode::Credentials
        };
        self.server_dialog = Some(form);
        cx.notify();
    }

    /// Submit whichever connect tab is active.
    fn submit_server_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(form) = self.server_dialog.clone() else {
            return;
        };
        match form.mode {
            ServerMode::Quick => self.connect_to_server(&form.addr, cx),
            ServerMode::Credentials => self.submit_credentials(cx),
        }
    }

    /// Save an SFTP server from the Credentials tab (password → Keychain) and
    /// connect to it.
    fn submit_credentials(&mut self, cx: &mut Context<Self>) {
        let Some(form) = self.server_dialog.clone() else {
            return;
        };
        let host = form.host.trim().to_string();
        if host.is_empty() {
            return;
        }
        let name = if form.name.trim().is_empty() {
            host.clone()
        } else {
            form.name.trim().to_string()
        };
        let server = SftpServer {
            control_path: None,
            name,
            host,
            user: form.user.trim().to_string(),
            port: form.port.trim().parse().unwrap_or(0),
            key: String::new(),
            use_password: true,
            auto_reopen: form.auto_reopen,
        };
        // Only overwrite the Keychain when a password was entered (editing an
        // existing server without retyping keeps the stored one).
        if !form.password.is_empty() {
            keychain_set_password(&server, &form.password);
        }
        let mut list = sftp_servers();
        // When editing, drop the original entry (its target may have changed).
        if let Some(old) = &form.editing {
            list.retain(|s| &s.display() != old);
        }
        if let Some(existing) = list.iter_mut().find(|s| s.display() == server.display()) {
            *existing = server.clone();
        } else {
            list.push(server.clone());
        }
        apply_sftp_servers(list, cx);
        self.server_dialog = None;
        self.connect_sftp(server, cx);
    }

    /// Open the connect dialog pre-filled to edit an existing server.
    fn edit_server(&mut self, server: &SftpServer, cx: &mut Context<Self>) {
        self.sidebar_menu = None;
        self.server_dialog = Some(ServerForm::editing_server(server));
        cx.notify();
    }

    /// Hand a server URL off to macOS (`open smb://…`), which shows the native
    /// auth prompt and mounts the share under /Volumes. Records it in history.
    /// `sftp://[user@]host[:port]` instead saves an SFTP server and connects to
    /// it in-app (browsing over SSH, no mount).
    fn connect_to_server(&mut self, raw: &str, cx: &mut Context<Self>) {
        let mut url = raw.trim().to_string();
        if url.is_empty() {
            return;
        }
        // sftp:// / ssh:// → an in-app SFTP server (saved + connected).
        if let Some(rest) = url
            .strip_prefix("sftp://")
            .or_else(|| url.strip_prefix("ssh://"))
        {
            if let Some(mut server) = parse_sftp_url(rest) {
                // Carry the dialog's "reconnect on launch" toggle onto the server.
                server.auto_reopen =
                    self.server_dialog.as_ref().map(|f| f.auto_reopen).unwrap_or(false);
                let mut list = sftp_servers();
                if let Some(existing) = list.iter_mut().find(|s| s.display() == server.display()) {
                    existing.auto_reopen = server.auto_reopen;
                } else {
                    list.push(server.clone());
                }
                apply_sftp_servers(list, cx);
                self.server_dialog = None;
                self.connect_sftp(server, cx);
                return;
            }
        }
        // Default to SMB when no scheme is given.
        if !url.contains("://") {
            url = format!("smb://{url}");
        }
        let _ = Command::new("open").arg(&url).spawn();

        // Most-recent-first, de-duplicated, capped.
        self.server_history.retain(|u| u != &url);
        self.server_history.insert(0, url);
        self.server_history.truncate(10);
        write_string_list("servers.txt", &self.server_history);

        self.server_dialog = None;
        cx.notify();
    }

    fn handle_server_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let ks = &ev.keystroke;
        let cmd = ks.modifiers.platform;
        match ks.key.as_str() {
            "escape" => {
                self.server_dialog = None;
                cx.notify();
            }
            "enter" => self.submit_server_dialog(cx),
            "tab" => {
                if let Some(f) = self.server_dialog.as_mut() {
                    if f.mode == ServerMode::Credentials {
                        f.next_field();
                    }
                }
                cx.notify();
            }
            "backspace" => {
                if let Some(f) = self.server_dialog.as_mut() {
                    f.active_field().pop();
                }
                cx.notify();
            }
            "v" if cmd => {
                if let Some(t) = cx.read_from_clipboard().and_then(|i| i.text()) {
                    if let Some(f) = self.server_dialog.as_mut() {
                        let s = t.trim().to_string();
                        f.active_field().push_str(&s);
                    }
                    cx.notify();
                }
            }
            _ => {
                if cmd {
                    return;
                }
                if let Some(ch) = ks.key_char.as_ref() {
                    if !ch.is_empty() && !ch.chars().any(char::is_control) {
                        if let Some(f) = self.server_dialog.as_mut() {
                            f.active_field().push_str(ch);
                        }
                        cx.notify();
                    }
                }
            }
        }
    }

    fn render_server_dialog(&self, cx: &Context<Self>) -> impl IntoElement {
        let t = theme();
        let form = self.server_dialog.clone().unwrap_or_default();
        let mode = form.mode;

        // Mode tabs (Quick Connect | Credentials).
        let mode_tab = |label: &'static str, m: ServerMode| {
            let active = mode == m;
            div()
                .id(label)
                .flex_1()
                .flex()
                .justify_center()
                .px_3()
                .py_1()
                .rounded_md()
                .cursor_pointer()
                .text_color(if active { rgb(t.text) } else { rgb(t.text_muted) })
                .bg(if active { rgb(t.selected) } else { rgba(0x00000000) })
                .hover(|s| s.bg(rgb(t.hover)))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(f) = this.server_dialog.as_mut() {
                        f.mode = m;
                    }
                    cx.notify();
                }))
        };
        let tabs = div()
            .flex()
            .gap_1()
            .p_1()
            .rounded_md()
            .bg(rgb(t.bg))
            .child(mode_tab("Quick Connect", ServerMode::Quick))
            .child(mode_tab("Credentials", ServerMode::Credentials));

        // A labeled input row for the Credentials tab.
        let field_row = |label: &'static str, cf: CredField, value: &str, secret: bool| {
            let focused = mode == ServerMode::Credentials && form.field == cf;
            let empty = value.is_empty();
            let shown = if empty {
                label.to_string()
            } else if secret {
                "\u{2022}".repeat(value.chars().count())
            } else {
                value.to_string()
            };
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(div().text_size(px(UI_SMALL_FONT_SIZE)).text_color(rgb(t.text_muted)).child(label))
                .child(
                    div()
                        .id(label)
                        .flex()
                        .items_center()
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .bg(rgb(t.bg))
                        .border_1()
                        .border_color(if focused { rgb(t.accent) } else { rgb(t.border) })
                        .text_color(rgb(if empty { t.text_dim } else { t.text }))
                        .cursor_pointer()
                        .child(shown)
                        .when(focused, |d| {
                            d.child(div().w(px(1.5)).h(px(15.0)).ml(px(1.0)).bg(rgb(t.text)))
                        })
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if let Some(f) = this.server_dialog.as_mut() {
                                f.field = cf;
                            }
                            cx.notify();
                        })),
                )
        };

        let body: AnyElement = match mode {
            ServerMode::Quick => {
                let empty = form.addr.is_empty();
                let shown = if empty {
                    "smb://host/share  or  sftp://user@host".to_string()
                } else {
                    form.addr.clone()
                };
                let mut recent = div().flex().flex_col().gap_1();
                if !self.server_history.is_empty() {
                    recent = recent
                        .child(div().text_size(px(UI_SMALL_FONT_SIZE)).text_color(rgb(t.text_dim)).child("Recent"));
                    for u in self.server_history.iter().take(5) {
                        let target = u.clone();
                        recent = recent.child(
                            div()
                                .id(SharedString::from(format!("srv-{u}")))
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .cursor_pointer()
                                .text_color(rgb(t.text_muted))
                                .hover(|s| s.bg(rgb(t.hover)).text_color(rgb(t.text)))
                                .child(u.clone())
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.connect_to_server(&target, cx);
                                })),
                        );
                    }
                }
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(div().text_size(px(UI_SMALL_FONT_SIZE)).text_color(rgb(t.text_muted)).child(
                        "Address — smb://, afp://, ftp://, or sftp://user@host to browse over SSH.",
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(t.bg))
                            .border_1()
                            .border_color(rgb(t.accent))
                            .text_color(rgb(if empty { t.text_dim } else { t.text }))
                            .child(shown)
                            .child(div().w(px(1.5)).h(px(15.0)).ml(px(1.0)).bg(rgb(t.text))),
                    )
                    .child(recent)
                    .into_any_element()
            }
            ServerMode::Credentials => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(field_row("Name (optional)", CredField::Name, &form.name, false))
                .child(field_row("Host", CredField::Host, &form.host, false))
                .child(field_row("Username", CredField::User, &form.user, false))
                .child(field_row("Port (optional)", CredField::Port, &form.port, false))
                .child(field_row("Password", CredField::Password, &form.password, true))
                .child(
                    div()
                        .text_size(px(UI_SMALL_FONT_SIZE))
                        .text_color(rgb(t.text_dim))
                        .child("Password is stored in your macOS Keychain — Tab moves between fields."),
                )
                .into_any_element(),
        };

        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x00000066))
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.server_dialog = None;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .w(px(440.0))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_5()
                    .rounded_lg()
                    .bg(rgb(t.surface))
                    .border_1()
                    .border_color(rgb(t.border_strong))
                    .shadow_lg()
                    .on_mouse_down(MouseButton::Left, |_, _, cx: &mut App| cx.stop_propagation())
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(t.text))
                            .child("Connect to Server"),
                    )
                    .child(tabs)
                    .child(body)
                    .child(
                        // "Reconnect on launch" toggle for this server.
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .cursor_pointer()
                            .id("srv-autoreopen")
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                if let Some(f) = this.server_dialog.as_mut() {
                                    f.auto_reopen = !f.auto_reopen;
                                }
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .w(px(16.0))
                                    .h(px(16.0))
                                    .rounded_sm()
                                    .border_1()
                                    .border_color(rgb(t.border_strong))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .when(form.auto_reopen, |s| {
                                        s.bg(rgb(t.accent)).text_color(rgb(0xffffff))
                                    })
                                    .child(if form.auto_reopen { "✓" } else { "" }),
                            )
                            .child(
                                div()
                                    .text_size(px(UI_SMALL_FONT_SIZE))
                                    .text_color(rgb(t.text_muted))
                                    .child("Reconnect on launch"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                div()
                                    .id("srv-cancel")
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .text_color(rgb(t.text))
                                    .bg(rgb(t.hover))
                                    .hover(|s| s.bg(rgb(t.selected)))
                                    .child("Cancel")
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.server_dialog = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .id("srv-connect")
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .text_color(rgb(0xffffff))
                                    .bg(rgb(t.accent))
                                    .hover(|s| s.bg(Theme::alpha(t.accent, 0xdd)))
                                    .child("Connect")
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.submit_server_dialog(cx);
                                    })),
                            ),
                    ),
            )
    }

    /// First-run SSH prompt: choose how Shuffle authenticates over SSH.
    fn render_ssh_prompt(&self, cx: &Context<Self>) -> impl IntoElement {
        let t = theme();
        let choice =
            |id: &'static str, title: &'static str, desc: &'static str, use_system: bool, primary: bool| {
                div()
                    .id(id)
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_3()
                    .rounded_md()
                    .cursor_pointer()
                    .bg(rgb(t.bg))
                    .border_1()
                    .border_color(if primary { rgb(t.accent) } else { rgb(t.border) })
                    .hover(|s| s.border_color(rgb(t.accent)))
                    .child(div().text_color(rgb(t.text)).child(title))
                    .child(div().text_size(px(UI_SMALL_FONT_SIZE)).text_color(rgb(t.text_muted)).child(desc))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.choose_ssh_mode(use_system, cx);
                    }))
            };
        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x00000066))
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.ssh_ask = false;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .w(px(460.0))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_5()
                    .rounded_lg()
                    .bg(rgb(t.surface))
                    .border_1()
                    .border_color(rgb(t.border_strong))
                    .shadow_lg()
                    .on_mouse_down(MouseButton::Left, |_, _, cx: &mut App| cx.stop_propagation())
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(t.text))
                            .child("Connect over SSH"),
                    )
                    .child(div().text_size(px(UI_SMALL_FONT_SIZE)).text_color(rgb(t.text_muted)).child(
                        "How should Schnauz authenticate to SFTP servers? You can change this \
                         anytime in Settings \u{2192} Connections.",
                    ))
                    .child(choice(
                        "ssh-use-system",
                        "Use my ~/.ssh configuration",
                        "Connect with your existing SSH keys, config aliases, and known_hosts. \
                         Nothing is stored in the app. Recommended.",
                        true,
                        true,
                    ))
                    .child(choice(
                        "ssh-credentials",
                        "Enter credentials per server",
                        "Type a username and password for each server; passwords are saved in \
                         your macOS Keychain.",
                        false,
                        false,
                    )),
            )
    }

    /// The "New Group" naming dialog.
    fn render_group_dialog(&self, cx: &Context<Self>) -> impl IntoElement {
        let t = theme();
        let name = self.group_dialog.clone().unwrap_or_default();
        let placeholder = name.is_empty();
        let shown = if placeholder { "Group name".to_string() } else { name.clone() };

        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x00000066))
            .occlude()
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                this.group_dialog = None;
                cx.notify();
            }))
            .child(
                div()
                    .w(px(360.0))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_5()
                    .rounded_lg()
                    .bg(rgb(t.surface))
                    .border_1()
                    .border_color(rgb(t.border_strong))
                    .shadow_lg()
                    .on_mouse_down(MouseButton::Left, |_, _, cx: &mut App| cx.stop_propagation())
                    .child(div().text_color(rgb(t.text)).child("New Group"))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(t.bg))
                            .border_1()
                            .border_color(rgb(t.accent))
                            .text_color(rgb(if placeholder { t.text_dim } else { t.text }))
                            .child(shown),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                div()
                                    .id("grp-cancel")
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .text_color(rgb(t.text))
                                    .bg(rgb(t.hover))
                                    .hover(|s| s.bg(rgb(t.selected)))
                                    .child("Cancel")
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.group_dialog = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .id("grp-create")
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .text_color(rgb(0xffffff))
                                    .bg(rgb(t.accent))
                                    .hover(|s| s.bg(Theme::alpha(t.accent, 0xdd)))
                                    .child("Create")
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        if let Some(name) = this.group_dialog.clone() {
                                            this.create_group(&name, cx);
                                        }
                                    })),
                            ),
                    ),
            )
    }

    /// The sidebar right-click context menu (New Group / Remove / Delete Group).
    fn sidebar_menu_nodes(&self, target: SidebarTarget) -> Vec<MenuNode> {
        let groups_on = prefs().groups_enabled;
        let mut items: Vec<MenuNode> = Vec::new();
        let folder = match &target {
            SidebarTarget::ProjectPath(_,p) | SidebarTarget::GroupMember(_,p) => Some(p.clone()),
            SidebarTarget::Empty => Some(self.active_tab().current_dir.clone()).filter(|_| self.active_tab().remote.is_none()),
            SidebarTarget::Tab(p,t) => self.panes.get(*p).and_then(|p|p.tabs.get(*t)).filter(|t|t.remote.is_none()).map(|t|t.current_dir.clone()),
            SidebarTarget::NavBar(p) => Some(self.tab(*p)).filter(|t|t.remote.is_none()).map(|t|t.current_dir.clone()),
            _ => None,
        };
        if let Some(path) = &folder {
            if let Some(menu) = self.cloud_menu(self.active_pane, path, cached_is_dir(path)) {
                items.push(menu); items.push(MenuNode::Sep);
            }
        }
        if let Some(folder) = folder.filter(|p|p.is_dir()) {
            let section = match &target { SidebarTarget::ProjectPath(section, _) => *section, _ => projects::Section::Locations };
            items.push(self.project_visibility_menu(section, folder.clone()));
            items.push(MenuNode::Sep);
            let view_folder = folder.clone();
            let view_pane = match &target { SidebarTarget::NavBar(p) | SidebarTarget::Tab(p, _) => *p, _ => self.active_pane };
            items.push(mi("Apply view to folder + subfolders", move |this, _, cx| this.apply_view_to_tree(view_pane, view_folder.clone(), cx)));
            items.push(mi("Speicherbelegung…", move|this,window,cx|{
                this.inspect_folder(this.active_pane,folder.clone(),window,cx);
            }));
            items.push(MenuNode::Sep);
        }
        match target {
            SidebarTarget::ProjectPath(section, path) => {
                items.push(mi("Remove from this project", move |this, _, cx| this.set_project_path(section, path.clone(), false, cx)));
            },
            SidebarTarget::Project(id) => {
                items.push(mi("Open project", move |this, window, cx| { window.focus(&this.focus); this.restore_project(id, cx); }));
                items.push(MenuNode::Sep);
                items.push(mi("Remove project", move |this, _, cx| this.remove_project(id, cx)));
            }
            SidebarTarget::Empty => {
                if groups_on {
                    items.push(
                        mi("New Group", |this, _, cx| {
                            this.close_sidebar_menu(cx);
                            this.open_group_dialog(cx);
                        }),
                    );
                }
            }
            SidebarTarget::GroupHeader(idx) => {
                if groups_on {
                    items.push(
                        mi("New Group", |this, _, cx| {
                            this.close_sidebar_menu(cx);
                            this.open_group_dialog(cx);
                        }),
                    );
                    items.push(MenuNode::Sep);
                }
                items.push(
                    mi("Delete Group", move |this, _, cx| {
                        this.close_sidebar_menu(cx);
                        this.delete_group(idx, cx);
                    }),
                );
            }
            SidebarTarget::GroupMember(idx, p) => {
                items.push(
                    mi("Remove from Group", move |this, _, cx| {
                        this.close_sidebar_menu(cx);
                        this.remove_from_group(idx, &p, cx);
                    }),
                );
            }
            SidebarTarget::Sftp(server) => {
                let s = server.clone();
                items.push(
                    mi("Connect", move |this, _, cx| {
                        this.close_sidebar_menu(cx);
                        this.connect_sftp(s.clone(), cx);
                    }),
                );
                let s = server.clone();
                items.push(
                    mi("Edit\u{2026}", move |this, _, cx| {
                        this.edit_server(&s, cx);
                    }),
                );
                let s = server.clone();
                items.push(
                    mi("Remove", move |this, _, cx| {
                        this.close_sidebar_menu(cx);
                        let list: Vec<SftpServer> = sftp_servers()
                            .into_iter()
                            .filter(|x| x.display() != s.display())
                            .collect();
                        apply_sftp_servers(list, cx);
                    }),
                );
            }
            SidebarTarget::Tab(pane, tab) => {
                items.push(
                    mi("New Tab", move |this, _, cx| {
                        this.close_sidebar_menu(cx);
                        this.new_tab_in(pane, cx);
                    }),
                );
                items.push(
                    mi("Duplicate Tab", move |this, _, cx| {
                        this.close_sidebar_menu(cx);
                        this.duplicate_tab(pane, tab, cx);
                    }),
                );
                items.push(MenuNode::Sep);
                items.push(
                    mi("Close Tab", move |this, _, cx| {
                        this.close_sidebar_menu(cx);
                        this.close_tab(pane, tab, cx);
                    }),
                );
                // Only offer "Close Other Tabs" when there's more than one.
                if self.pane(pane).tabs.len() > 1 {
                    items.push(
                        mi("Close Other Tabs", move |this, _, cx| {
                            this.close_sidebar_menu(cx);
                            this.close_other_tabs(pane, tab, cx);
                        }),
                    );
                }
            }
            SidebarTarget::NavBar(pane) => {
                items.push(
                    mi("New Tab", move |this, _, cx| {
                        this.close_sidebar_menu(cx);
                        this.new_tab_in(pane, cx);
                    }),
                );
                let dir = self.tab(pane).current_dir.clone();
                let is_remote = self.tab(pane).remote.is_some();
                let dc = dir.clone();
                items.push(
                    mi("Copy Path", move |this, _, cx| {
                        this.close_sidebar_menu(cx);
                        cx.write_to_clipboard(ClipboardItem::new_string(
                            dc.to_string_lossy().into_owned(),
                        ));
                    }),
                );
                // Reveal-in-Finder / terminal only make sense for local folders.
                if !is_remote {
                    let dr = dir.clone();
                    items.push(
                        mi("Reveal in Finder", move |this, _, cx| {
                            this.close_sidebar_menu(cx);
                            let _ = Command::new("open").arg(&dr).spawn();
                        }),
                    );
                }
            }
        }
        if items.is_empty() {
            items.push(MenuNode::Disabled("No actions".into()));
        }

        items
    }

    /// Quietly ask GitHub for the latest published release and, if it's newer
    /// than this build (and the user hasn't dismissed that exact version),
    /// surface the update banner. Best-effort: any failure (offline, timeout,
    /// unparseable) just leaves the banner hidden. Never blocks the UI.
    fn check_for_update(&self, cx: &mut Context<Self>) {
        if !upstream_updates_enabled() { return; }
        cx.spawn(async move |this, cx| {
            let Some(tag) = cx.background_spawn(async move { fetch_latest_tag() }).await else {
                return;
            };
            // Only surface real upgrades over the running build.
            if parse_version(&tag) <= parse_version(env!("CARGO_PKG_VERSION")) {
                return;
            }
            // Respect a prior "dismiss" of this exact version so we don't nag.
            if config_string("update_skip.txt").as_deref() == Some(tag.as_str()) {
                return;
            }
            this.update(cx, |this, cx| {
                this.update = Some(UpdateStatus::Available(tag));
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Hide the update banner and remember the dismissed version so it doesn't
    /// come back on the next launch (until a still-newer release appears).
    fn dismiss_update(&mut self, cx: &mut Context<Self>) {
        if let Some(UpdateStatus::Available(ver)) = self.update.take() {
            write_config_string("update_skip.txt", &ver);
        }
        cx.notify();
    }

    /// Download the newest release, verify it's genuinely ours (notarized +
    /// signed by our Team ID), then swap the running bundle and relaunch. Kicked
    /// off by the banner's "Update" button. All network/disk work is off-thread;
    /// the actual replace happens in a tiny helper that waits for us to quit.
    fn start_self_update(&mut self, cx: &mut Context<Self>) {
        if !upstream_updates_enabled() { return; }
        let ver = match &self.update {
            Some(UpdateStatus::Available(v)) => v.clone(),
            _ => return,
        };
        let bundle = current_app_bundle();
        self.update = Some(UpdateStatus::Downloading(ver));
        cx.notify();
        cx.spawn(async move |this, cx| {
            let prepared = cx
                .background_spawn(async move {
                    let bundle = bundle.ok_or_else(|| "couldn't locate the app bundle".to_string())?;
                    let ready = download_and_verify_update()?;
                    Ok::<_, String>((bundle, ready))
                })
                .await;
            match prepared {
                Ok((bundle, ready)) => {
                    if let Err(e) = launch_swap_and_relaunch(&bundle, &ready) {
                        ready.detach(); // unmount the dmg we couldn't hand off
                        this.update(cx, |this, cx| {
                            this.update = Some(UpdateStatus::Failed(e));
                            cx.notify();
                        })
                        .ok();
                        return;
                    }
                    // Helper is armed and waiting for us to exit; quit so it can
                    // replace the bundle and relaunch the new version.
                    let _ = cx.update(|cx| cx.quit());
                }
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.update = Some(UpdateStatus::Failed(e));
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    /// The slim update bar shown under the titlebar (available / downloading /
    /// failed). Returns `None` when there's nothing to show.
    fn render_update_banner(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let status = self.update.clone()?;
        let t = theme();
        let bar = div()
            .flex_none()
            .w_full()
            .flex()
            .items_center()
            .gap_3()
            .px_4()
            .py_1p5()
            .bg(rgb(t.accent))
            .text_color(rgb(0xffffff))
            .text_size(px(UI_SMALL_FONT_SIZE));

        let bar = match status {
            UpdateStatus::Available(ver) => bar
                .child(div().child("↑").text_size(px(UI_FONT_SIZE)))
                .child(div().flex_1().child(format!(
                    "Shuffle {ver} is available (you have v{}).",
                    env!("CARGO_PKG_VERSION")
                )))
                .child(
                    div()
                        .id("update-install")
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .bg(rgba(0xffffff33))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgba(0xffffff55)))
                        .child("Update")
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| this.start_self_update(cx)),
                        ),
                )
                .child(
                    div()
                        .id("update-dismiss")
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .cursor_pointer()
                        .hover(|s| s.bg(rgba(0xffffff33)))
                        .child("✕")
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| this.dismiss_update(cx)),
                        ),
                ),
            UpdateStatus::Downloading(ver) => bar.child(div().child("↓").text_size(px(UI_FONT_SIZE))).child(
                div()
                    .flex_1()
                    .child(format!("Downloading Shuffle {ver}… the app will relaunch itself.")),
            ),
            UpdateStatus::Failed(reason) => bar
                .child(div().child("⚠").text_size(px(UI_FONT_SIZE)))
                .child(div().flex_1().child(format!("Update failed: {reason}.")))
                .child(
                    div()
                        .id("update-manual")
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .bg(rgba(0xffffff33))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgba(0xffffff55)))
                        .child("Download manually")
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                let _ = Command::new("open").arg(DMG_URL).spawn();
                                this.update = None;
                                cx.notify();
                            }),
                        ),
                )
                .child(
                    div()
                        .id("update-dismiss-failed")
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .cursor_pointer()
                        .hover(|s| s.bg(rgba(0xffffff33)))
                        .child("✕")
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.update = None;
                                cx.notify();
                            }),
                        ),
                ),
        };
        Some(bar.into_any_element())
    }

    /// Current scrolled distance from the top, in pixels.
    fn current_scrolled(&self, pane: usize) -> f32 {
        let state = self.tab(pane).scroll_handle.0.borrow();
        (-(f64::from(state.base_handle.offset().y) as f32)).max(0.0)
    }

    fn begin_scroll_drag(&mut self, pane: usize, y: f32) {
        self.scroll_drag = Some(ScrollDrag {
            pane,
            start_y: y,
            start_scrolled: self.current_scrolled(pane),
        });
    }

    fn update_scroll_drag(&mut self, y: f32, cx: &mut Context<Self>) {
        let Some(drag) = self.scroll_drag else {
            return;
        };
        if drag.pane >= self.panes.len() {
            return;
        }
        let state = self.tab(drag.pane).scroll_handle.0.borrow();
        let base = &state.base_handle;
        let viewport = f64::from(base.bounds().size.height) as f32;
        let max = f64::from(base.max_offset().height) as f32;
        if viewport <= 1.0 || max <= 1.0 {
            return;
        }
        let content = viewport + max;
        let thumb_h = (viewport * viewport / content).clamp(28.0, viewport);
        let travel = (viewport - thumb_h).max(1.0);
        // Thumb moves `delta` px; scale that to content-scroll distance.
        let delta = y - drag.start_y;
        let new_scrolled = (drag.start_scrolled + delta * (max / travel)).clamp(0.0, max);
        let x = base.offset().x;
        base.set_offset(point(x, px(-new_scrolled)));
        drop(state);
        cx.notify();
    }

    fn end_scroll_drag(&mut self, cx: &mut Context<Self>) {
        // Releasing the thumb counts as the last scroll: starts the fade timer.
        if let Some(drag) = self.scroll_drag.take() {
            if drag.pane < self.panes.len() {
                self.mark_scrolled(drag.pane, cx);
            }
        }
    }

    // ----- file search (Cmd+P) and folder actions -----

    fn toggle_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.search_in_pane(self.active_pane) && !self.file_search.editing {
            self.focus_file_search(false, window, cx); return;
        }
        if self.palette_open { self.close_palette(cx); }
        else { self.focus_file_search(true, window, cx); }
    }

    fn close_palette(&mut self, cx: &mut Context<Self>) {
        if self.file_search.inline && self.file_search.busy { self.file_search.stopped = true; }
        self.file_search.cancel();
        if self.file_search.inline {
            let pane = self.file_search.pane;
            if pane < self.panes.len() && self.pane(pane).active == self.file_search.tab {
                let tab = self.tab_mut(pane);
                tab.selection.clear(); tab.anchor = None;
            }
            self.file_search.scope = file_search::Scope::CurrentFolder;
            self.file_search.mode = file_search::Mode::File;
            self.query.clear(); self.query_cursor = 0; self.query_anchor = None;
        }
        self.search_gen = self.search_gen.wrapping_add(1);
        self.palette_open = false;
        self.palette_actions = None;
        cx.notify();
    }

    /// Open the actions popup targeted at `path`: a single selected item
    /// for that folder with the ⌘K actions dropdown already open (Copy path,
    /// Reveal in Finder, Open in new tab, …). Reached by clicking the
    /// breadcrumb's current-location crumb, which previously just re-navigated
    /// to where you already were.
    fn open_palette_for_path(
        &mut self,
        pane: usize,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.active_pane = pane;
        self.file_search.cancel();
        self.file_search.inline = false;
        self.palette_open = true;
        self.query.clear();
        self.query_cursor = 0;
        self.query_anchor = None;
        self.palette_hist_pos = None;
        self.selected = 0;
        self.search_gen = self.search_gen.wrapping_add(1); // drop in-flight results
        self.palette_scroll.set_offset(point(px(0.0), px(0.0)));
        self.palette_items = vec![PaletteItem {
            title: path_label(&path),
            subtitle: path.to_string_lossy().into_owned(),
            action: Action::Open(path, true),
            is_dir: true,
        }];
        self.palette_actions = Some(0); // land straight on the actions list
        window.focus(&self.focus);
        cx.notify();
    }

    fn refresh_palette(&mut self, cx: &mut Context<Self>) {
        self.refresh_file_search(cx);
    }

    fn move_selection(&mut self, delta: i64, cx: &mut Context<Self>) {
        let n = self.palette_items.len();
        if n == 0 {
            return;
        }
        let next = if self.file_search.inline && self.tab(self.file_search.pane).selection.is_empty() {
            if delta >= 0 { 0 } else { n as i64 - 1 }
        } else { (self.selected as i64 + delta).clamp(0, n as i64 - 1) };
        self.selected = next as usize;
        // Keep the highlighted row in view as you arrow through a long list.
        if self.file_search.inline {
            self.file_search.scroll.scroll_to_item(self.selected, ScrollStrategy::Center);
            self.select_search_result(self.selected, cx);
        } else { self.palette_scroll.scroll_to_item(self.selected); }
        cx.notify();
    }

    /// Read-only scroll indicator for the palette results list.
    fn palette_scrollbar_thumb(&self) -> Option<AnyElement> {
        static_scrollbar_thumb(&self.palette_scroll)
    }

    fn activate_selection(&mut self, cx: &mut Context<Self>) {
        let Some(item) = self.palette_items.get(self.selected) else {
            return;
        };
        let action = item.action.clone();
        self.record_palette_history();
        match action {
            Action::Open(path, is_dir) => {
                let search = self.file_search.inline;
                self.close_palette(cx);
                if search {
                    if is_browsable_folder(&path, is_dir) { self.navigate_to(path, cx); }
                    else if let Some(parent) = path.parent() {
                        let pane = self.active_pane;
                        if self.tab(pane).entry_filter == EntryFilter::Folders { self.tab_mut(pane).set_entry_filter(EntryFilter::All); }
                        self.navigate_to(parent.to_path_buf(), cx);
                        self.reveal_in_pane(self.active_pane, path, cx);
                    }
                    return;
                }
                if is_dir {
                    self.navigate_to(path, cx);
                } else {
                    // Enter opens the file itself (Spotlight-style); ⌘K offers
                    // "Show in enclosing folder" for the old navigate behavior.
                    let _ = Command::new("open").arg(&path).spawn();
                }
            }

        }
    }

    /// The ⌘K actions available for the palette's selected item, with labels.
    fn palette_action_list(&self) -> Vec<(PaletteAction, &'static str)> {
        match self.palette_items.get(self.selected).map(|i| &i.action) {
            Some(Action::Open(_, true)) => vec![
                (PaletteAction::Open, "Open folder"),
                (PaletteAction::OpenNewTab, "Open in new tab"),
                (PaletteAction::RevealFinder, "Reveal in Finder"),
                (PaletteAction::CopyPath, "Copy path"),
            ],
            Some(Action::Open(_, false)) => vec![
                (PaletteAction::Open, "Open file"),
                (PaletteAction::RevealShuffle, "Show in enclosing folder"),
                (PaletteAction::OpenNewTab, "Open folder in new tab"),
                (PaletteAction::RevealFinder, "Reveal in Finder"),
                (PaletteAction::CopyPath, "Copy path"),
            ],
            _ => Vec::new(),
        }
    }

    /// Run one of the ⌘K actions on the palette's selected item.
    fn run_palette_action(&mut self, act: PaletteAction, cx: &mut Context<Self>) {
        let Some(item) = self.palette_items.get(self.selected) else {
            return;
        };
        let Action::Open(path, is_dir) = item.action.clone();
        self.record_palette_history();
        let parent = || path.parent().map(Path::to_path_buf).unwrap_or_else(|| path.clone());
        match act {
            PaletteAction::Open => {
                self.close_palette(cx);
                if is_dir {
                    self.navigate_to(path, cx);
                } else {
                    let _ = Command::new("open").arg(&path).spawn();
                }
            }
            PaletteAction::RevealShuffle => {
                self.close_palette(cx);
                self.navigate_to(parent(), cx);
                let pane = self.active_pane;
                self.reveal_in_pane(pane, path, cx);
            }
            PaletteAction::OpenNewTab => {
                self.close_palette(cx);
                let pane = self.active_pane;
                self.new_tab_in(pane, cx);
                let dir = if is_dir { path.clone() } else { parent() };
                self.navigate_in(pane, dir, cx);
                if !is_dir {
                    self.reveal_in_pane(pane, path, cx);
                }
            }
            PaletteAction::RevealFinder => {
                let _ = Command::new("open").arg("-R").arg(&path).spawn();
                self.close_palette(cx);
            }
            PaletteAction::CopyPath => {
                cx.write_to_clipboard(ClipboardItem::new_string(
                    path.to_string_lossy().into_owned(),
                ));
                self.close_palette(cx);
            }
        }
    }

    /// What a keyboard action acts on: the selection (sorted), else the
    /// focused item.
    fn action_targets(&self, pane: usize) -> Vec<PathBuf> {
        let tab = self.tab(pane);
        let mut v: Vec<PathBuf> = tab.selection.iter().cloned().collect();
        if v.is_empty() {
            v.extend(tab.anchor.clone());
        }
        v.sort();
        v
    }

    /// Run a bound key action against the active pane.
    fn run_key_action(&mut self, action: KeyAction, window: &mut Window, cx: &mut Context<Self>) {
        let pane = self.active_pane;
        let anchor = self.tab(pane).anchor.clone();
        match action {
            KeyAction::CommandPalette => self.toggle_palette(window, cx),
            KeyAction::NewTab => self.new_tab_in(pane, cx),
            KeyAction::CloseTab => {
                let a = self.panes[pane].active;
                self.close_tab(pane, a, cx);
            }
            KeyAction::Find => self.open_find(pane, cx),
            KeyAction::SelectAll => {
                let all: HashSet<PathBuf> = self.display_paths(pane).into_iter().collect();
                self.tab_mut(pane).selection = all;
                cx.notify();
            }
            KeyAction::NewFile => self.new_file(pane, window, cx),
            KeyAction::NewFolder => self.new_folder(pane, window, cx),
            KeyAction::Rename => {
                if let Some(p) = anchor {
                    self.begin_rename(pane, p, window, cx);
                }
            }
            KeyAction::CopyPath => {
                if let Some(p) = anchor {
                    cx.write_to_clipboard(ClipboardItem::new_string(p.to_string_lossy().into_owned()));
                }
            }
            KeyAction::Duplicate => {
                if let Some(p) = anchor {
                    self.duplicate_entry(pane, p, cx);
                }
            }
            KeyAction::MakeAlias => {
                if let Some(p) = anchor {
                    self.make_alias(pane, p, cx);
                }
            }
            KeyAction::Compress => {
                if let Some(p) = anchor {
                    self.compress_entry(pane, p, cx);
                }
            }
            KeyAction::MoveToTrash => self.request_delete(pane, cx),
            KeyAction::RevealInFinder => {
                if let Some(p) = anchor {
                    let _ = Command::new("open").arg("-R").arg(&p).spawn();
                }
            }
            KeyAction::Open => {
                if let Some(p) = anchor {
                    let is_dir = p.is_dir();
                    self.open_path(pane, p, is_dir, cx);
                }
            }
            KeyAction::SwitchPane => {
                if self.panes.len() > 1 {
                    self.active_pane = (self.active_pane + 1) % self.panes.len();
                    cx.notify();
                }
            }
            KeyAction::Share => {
                let targets = self.action_targets(pane);
                if !targets.is_empty() && self.tab(pane).remote.is_none() {
                    let m = window.mouse_position();
                    if let Some(view) = ns_view_ptr(window) {
                        show_share_picker(view, (f32::from(m.x), f32::from(m.y)), &targets);
                    }
                }
            }
            KeyAction::GetInfo => {
                let targets = self.action_targets(pane);
                if let Some(p) = targets.first() {
                    self.open_get_info(p.clone(), cx);
                }
            }
            KeyAction::BatchRename => {
                let targets = self.action_targets(pane);
                if !targets.is_empty() && self.tab(pane).remote.is_none() {
                    self.open_batch_rename(pane, targets, window, cx);
                }
            }
            KeyAction::Undo => self.undo_last(cx),
            KeyAction::Paste => self.paste_from_clipboard(cx),
            KeyAction::Copy => {
                let targets = self.action_targets(pane);
                if self.tab(pane).remote.is_none() { self.copy_with_feedback(pane, &targets, cx); }
            }
            KeyAction::Refresh => {
                let dir = self.tab(pane).current_dir.clone();
                if self.tab(pane).remote.is_none() && cloud_kind(&dir).is_some() && cloudctl_path().is_some() {
                    self.cloud_sync(pane, dir, cx);
                } else {
                    self.refresh_pane(pane, cx);
                }
            }
            // Palette editing actions run inside the palette handler, not here.
            KeyAction::PaletteCursorStart
            | KeyAction::PaletteCursorEnd
            | KeyAction::PaletteSelectAll
            | KeyAction::PaletteDeleteToStart
            | KeyAction::PaletteHistoryPrev
            | KeyAction::PaletteHistoryNext => {}
        }
    }

    /// Top-level key handling: Cmd+P toggles; while open, drive the palette.
    fn on_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let ks = &ev.keystroke;
        let cmd = ks.modifiers.platform;
        let key = ks.key.as_str();

        if self.new_text_file.is_some() { self.handle_text_file_key(ev, cx); return; }
        if self.project_dialog.is_some() {
            self.handle_project_key(ev, cx);
            return;
        }

        // The first-run SSH prompt: Escape dismisses; clicks pick a mode.
        if self.ssh_ask {
            if key == "escape" {
                self.ssh_ask = false;
                cx.notify();
            }
            return;
        }
        // The Connect-to-Server dialog captures all typing while open.
        if self.server_dialog.is_some() {
            self.handle_server_key(ev, cx);
            return;
        }

        // The New Group dialog captures all typing while open.
        if self.group_dialog.is_some() {
            self.handle_group_key(ev, cx);
            return;
        }

        // The delete-confirmation dialog captures Enter (confirm) / Esc (cancel).
        if self.confirm_delete.is_some() {
            match key {
                "enter" => self.perform_delete(cx),
                "escape" => {
                    self.confirm_delete = None;
                    cx.notify();
                }
                _ => {}
            }
            return;
        }

        // The batch-rename dialog captures typing while open.
        if self.batch_rename.is_some() {
            self.handle_batch_key(ev, cx);
            return;
        }
        // Get Info: Esc / Enter / ⌘I close it; other keys are swallowed.
        if self.info_panel.is_some() {
            if matches!(key, "escape" | "enter") || (cmd && key == "i") {
                self.info_panel = None;
                cx.notify();
            }
            return;
        }

        // While an inline rename is active, keys feed the rename field.
        if self.rename.is_some() {
            self.handle_rename_key(ev, cx);
            return;
        }

        // While editing the path bar, keys feed the text field.
        if self.active_tab().editing_path.is_some() {
            self.handle_path_edit_key(ev, cx);
            return;
        }

        // While the in-directory find bar is open, keys feed the filter.
        if self.active_tab().find_query.is_some() {
            self.handle_find_key(ev, cx);
            return;
        }

        // An open context menu is modal: arrows navigate, → / ← open and close
        // flyouts, Enter invokes, Esc closes — and every other key is swallowed
        // so keybindings can't fire underneath it.
        if self.native_menu_pending { return; }

        // Search results are ordinary rows; clicking one leaves text-editing
        // focus and enables selection, previews, copying and Quick Look.
        if self.search_in_pane(self.active_pane) && !self.file_search.editing {
            match key {
                "escape" => self.close_palette(cx),
                "down" => self.move_selection(1, cx),
                "up" => self.move_selection(-1, cx),
                "enter" => self.activate_selection(cx),
                "space" if !cmd => self.quick_look(self.active_pane),
                _ => {
                    let kc = canon_keystroke(ks);
                    if let Some(action) = keymap().action_for(&kc) {
                        if matches!(action, KeyAction::CommandPalette | KeyAction::Copy | KeyAction::CopyPath | KeyAction::SelectAll | KeyAction::GetInfo | KeyAction::RevealInFinder | KeyAction::Share | KeyAction::Open | KeyAction::NewTab | KeyAction::CloseTab) {
                            self.run_key_action(action, window, cx);
                        }
                    }
                }
            }
            return;
        }

        // Dispatch a configured keybinding. When the palette is open only its
        // own toggle acts, so typed characters still reach the query.
        let kc = canon_keystroke(ks);
        if let Some(action) = keymap().action_for(&kc) {
            // Palette editing actions are handled inside the palette block only.
            if !action.is_palette() && (!self.palette_open || action == KeyAction::CommandPalette) {
                self.run_key_action(action, window, cx);
                return;
            }
        }
        if !self.palette_open {
            // Arrow keys move the selection within the active pane.
            if !cmd {
                let pane = self.active_pane;
                let delta = match key {
                    "up" => Some((0, -1)),
                    "down" => Some((0, 1)),
                    "left" => Some((-1, 0)),
                    "right" => Some((1, 0)),
                    _ => None,
                };
                if let Some((dx, dy)) = delta {
                    self.arrow_move(pane, dx, dy, cx);
                    return;
                }
                // Spacebar → Quick Look the selection, like Finder.
                if key == "space" {
                    self.quick_look(pane);
                    return;
                }
            }
            // Enter renames the item under the mouse first (point-and-rename),
            // else the focused one (Finder-style; not rebindable). The hover
            // target must still live in that pane's current folder — a stale
            // hover from before a navigation must never rename off-screen.
            if !cmd && key == "enter" {
                let hovered = self.hovered.clone().filter(|(hp, p)| {
                    *hp < self.panes.len()
                        && p.parent() == Some(self.tab(*hp).current_dir.as_path())
                        && p.exists()
                });
                if let Some((hpane, p)) = hovered {
                    self.begin_rename(hpane, p, window, cx);
                } else {
                    let pane = self.active_pane;
                    if let Some(sel) = self.tab(pane).anchor.clone() {
                        self.begin_rename(pane, sel, window, cx);
                    }
                }
            }
            // Backspace / Delete asks to move the selection to Trash.
            if !cmd && (key == "backspace" || key == "delete") {
                self.request_delete(self.active_pane, cx);
            }
            return;
        }

        // Rebindable palette editing actions (resolved against the keymap so
        // they can be changed in Settings › Keybinds). History actions apply
        // only when the palette-history setting is on.
        let km = keymap();
        let end = self.query.chars().count();
        if km.get(KeyAction::PaletteCursorStart) == Some(kc.as_str()) {
            self.query_cursor = 0;
            self.query_anchor = None;
            cx.notify();
            return;
        }
        if km.get(KeyAction::PaletteCursorEnd) == Some(kc.as_str()) {
            self.query_cursor = end;
            self.query_anchor = None;
            cx.notify();
            return;
        }
        if km.get(KeyAction::PaletteSelectAll) == Some(kc.as_str()) {
            if self.query.is_empty() {
                self.query_anchor = None;
            } else {
                self.query_anchor = Some(0);
                self.query_cursor = end;
            }
            cx.notify();
            return;
        }
        if km.get(KeyAction::PaletteDeleteToStart) == Some(kc.as_str()) {
            self.palette_kill_before();
            self.refresh_palette(cx);
            return;
        }
        if prefs().palette_history {
            if km.get(KeyAction::PaletteHistoryPrev) == Some(kc.as_str()) {
                self.palette_history_prev(cx);
                return;
            }
            if km.get(KeyAction::PaletteHistoryNext) == Some(kc.as_str()) {
                self.palette_history_next(cx);
                return;
            }
        }

        // ⌘K toggles the actions panel for the selected file/folder.
        if cmd && key == "k" && !self.file_search.inline {
            if self.palette_actions.is_some() {
                self.palette_actions = None;
            } else if !self.palette_action_list().is_empty() {
                self.palette_actions = Some(0);
            }
            cx.notify();
            return;
        }
        // While the actions panel is open, arrows/Enter drive it; anything
        // else closes it and falls through to normal palette handling.
        if let Some(sel) = self.palette_actions {
            let acts = self.palette_action_list();
            let handled = match key {
                "escape" => {
                    self.palette_actions = None;
                    true
                }
                "up" => {
                    self.palette_actions = Some(sel.saturating_sub(1));
                    true
                }
                "down" => {
                    self.palette_actions =
                        Some((sel + 1).min(acts.len().saturating_sub(1)));
                    true
                }
                "enter" => {
                    self.palette_actions = None;
                    if let Some(&(act, _)) = acts.get(sel) {
                        self.run_palette_action(act, cx);
                    }
                    true
                }
                _ => {
                    self.palette_actions = None;
                    false
                }
            };
            cx.notify();
            if handled {
                return;
            }
        }

        match key {
            "escape" => self.close_palette(cx),
            "enter" => self.activate_selection(cx),
            "down" => self.move_selection(1, cx),
            "up" => self.move_selection(-1, cx),
            // Left/Right move the text cursor. Option jumps by word; Shift
            // extends the selection (Option+Shift selects a word at a time).
            "left" => self.palette_move_h(true, ks.modifiers.alt, ks.modifiers.shift, cx),
            "right" => self.palette_move_h(false, ks.modifiers.alt, ks.modifiers.shift, cx),
            "backspace" => {
                self.palette_backspace();
                self.refresh_palette(cx);
            }
            "v" if cmd => {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    self.palette_insert(text.trim());
                    self.refresh_palette(cx);
                }
            }
            _ => {
                if cmd {
                    return; // ignore other Cmd-combos
                }
                if let Some(ch) = ks.key_char.as_ref() {
                    if !ch.is_empty() && !ch.chars().any(|c| c.is_control()) {
                        self.palette_insert(ch);
                        self.refresh_palette(cx);
                    }
                }
            }
        }
    }

    // ----- palette text editing (cursor-aware) -----

    /// Byte offset of char index `i` in the query (or its end).
    fn query_byte(&self, i: usize) -> usize {
        self.query
            .char_indices()
            .nth(i)
            .map(|(b, _)| b)
            .unwrap_or(self.query.len())
    }

    /// The current selection as a sorted `(start, end)` char range, if any.
    fn query_sel(&self) -> Option<(usize, usize)> {
        match self.query_anchor {
            Some(a) if a != self.query_cursor => {
                Some((a.min(self.query_cursor), a.max(self.query_cursor)))
            }
            _ => None,
        }
    }

    /// Delete the selected range (if any), leaving the cursor at its start.
    /// Returns true when a selection was removed. Always clears the anchor.
    fn query_delete_sel(&mut self) -> bool {
        if let Some((lo, hi)) = self.query_sel() {
            let (bl, bh) = (self.query_byte(lo), self.query_byte(hi));
            self.query.replace_range(bl..bh, "");
            self.query_cursor = lo;
            self.query_anchor = None;
            true
        } else {
            self.query_anchor = None;
            false
        }
    }

    /// New cursor char index one step (or one word) left/right of the cursor.
    fn query_h_target(&self, left: bool, word: bool) -> usize {
        let end = self.query.chars().count();
        match (left, word) {
            (true, true) => prev_word_boundary(&self.query, self.query_cursor),
            (true, false) => self.query_cursor.saturating_sub(1),
            (false, true) => next_word_boundary(&self.query, self.query_cursor),
            (false, false) => (self.query_cursor + 1).min(end),
        }
    }

    /// Move (or extend) the palette text cursor. `word` jumps by word,
    /// `select` extends the selection instead of collapsing it.
    fn palette_move_h(&mut self, left: bool, word: bool, select: bool, cx: &mut Context<Self>) {
        if select {
            if self.query_anchor.is_none() {
                self.query_anchor = Some(self.query_cursor);
            }
            self.query_cursor = self.query_h_target(left, word);
            if self.query_anchor == Some(self.query_cursor) {
                self.query_anchor = None;
            }
        } else if let Some((lo, hi)) = self.query_sel() {
            // Plain arrow collapses a selection: char-move to the edge, word-move
            // continues past it.
            self.query_cursor = if word {
                self.query_h_target(left, word)
            } else if left {
                lo
            } else {
                hi
            };
            self.query_anchor = None;
        } else {
            self.query_cursor = self.query_h_target(left, word);
            self.query_anchor = None;
        }
        cx.notify();
    }

    /// Insert `s` at the cursor (replacing the selection first, if any).
    fn palette_insert(&mut self, s: &str) {
        self.query_delete_sel();
        let b = self.query_byte(self.query_cursor);
        self.query.insert_str(b, s);
        self.query_cursor += s.chars().count();
        self.palette_hist_pos = None;
    }

    /// Delete the char before the cursor (or the whole selection).
    fn palette_backspace(&mut self) {
        if self.query_delete_sel() {
            return;
        }
        if self.query_cursor == 0 {
            return;
        }
        let start = self.query_byte(self.query_cursor - 1);
        let end = self.query_byte(self.query_cursor);
        self.query.replace_range(start..end, "");
        self.query_cursor -= 1;
    }

    /// Ctrl+U: delete everything before the cursor, keeping what's after.
    fn palette_kill_before(&mut self) {
        let b = self.query_byte(self.query_cursor);
        self.query.replace_range(0..b, "");
        self.query_cursor = 0;
        self.query_anchor = None;
    }

    /// Up in history mode: step to the previous (older) query.
    fn palette_history_prev(&mut self, cx: &mut Context<Self>) {
        if self.palette_hist.is_empty() {
            return;
        }
        let pos = match self.palette_hist_pos {
            None => self.palette_hist.len() - 1,
            Some(0) => 0,
            Some(p) => p - 1,
        };
        self.palette_hist_pos = Some(pos);
        self.query = self.palette_hist[pos].clone();
        self.query_cursor = self.query.chars().count();
        self.query_anchor = None;
        self.refresh_palette(cx);
    }

    /// Down in history mode: step to the next (newer) query, past the newest
    /// returns to an empty live query.
    fn palette_history_next(&mut self, cx: &mut Context<Self>) {
        match self.palette_hist_pos {
            None => {}
            Some(p) if p + 1 >= self.palette_hist.len() => {
                self.palette_hist_pos = None;
                self.query.clear();
                self.query_cursor = 0;
                self.query_anchor = None;
                self.refresh_palette(cx);
            }
            Some(p) => {
                let np = p + 1;
                self.palette_hist_pos = Some(np);
                self.query = self.palette_hist[np].clone();
                self.query_cursor = self.query.chars().count();
                self.query_anchor = None;
                self.refresh_palette(cx);
            }
        }
    }

    /// Record a submitted query into the palette history (when enabled).
    fn record_palette_history(&mut self) {
        if !prefs().palette_history {
            return;
        }
        let q = self.query.trim().to_string();
        if q.is_empty() {
            return;
        }
        self.palette_hist.retain(|h| h != &q);
        self.palette_hist.push(q);
        let overflow = self.palette_hist.len().saturating_sub(50);
        if overflow > 0 {
            self.palette_hist.drain(0..overflow);
        }
        write_string_list("palette_history.txt", &self.palette_hist);
    }

    fn render_palette(&self, cx: &Context<Self>) -> impl IntoElement {
        let t = theme();
        // Fit the list to its content, capped — so there's no empty gray space
        // below a short result set, and it scrolls once it's long.
        let visible = self.palette_items.len().min(PALETTE_MAX_ROWS);
        let list_h = visible as f32 * PALETTE_ROW_H;

        let rows: Vec<AnyElement> = self
            .palette_items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let selected = i == self.selected;
                let icon = icon_element(Path::new(&item.subtitle), item.is_dir);
                let base = div()
                    .id(("pal", i))
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .h(px(PALETTE_ROW_H))
                    .cursor_pointer();
                let base = if selected {
                    base.bg(rgb(t.selected))
                } else {
                    base.hover(|s| s.bg(rgb(t.hover)))
                };
                base.child(div().flex_none().w(px(18.0)).flex().justify_center().child(icon))
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap_2()
                            .min_w_0()
                            .flex_1()
                            .child(div().flex_none().text_color(rgb(t.text)).child(item.title.clone()))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(UI_SMALL_FONT_SIZE))
                                    .text_color(rgb(t.text_muted))
                                    .child(item.subtitle.clone()),
                            ),
                    )
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.selected = i;
                        this.activate_selection(cx);
                    }))
                    .into_any_element()
            })
            .collect();

        // Input line: query with a caret, or a dim placeholder. A selection
        // (whole via Cmd+A, or a word range via Option+Shift+Arrow) is drawn as
        // a highlighted span between the unselected head and tail.
        let input = if self.query.is_empty() {
            div()
                .text_color(rgb(t.text_dim))
                .child("Type a path, or a file/folder name…")
        } else if let Some((lo, hi)) = self.query_sel() {
            let (bl, bh) = (self.query_byte(lo), self.query_byte(hi));
            div()
                .flex()
                .text_color(rgb(t.text))
                .child(div().child(self.query[..bl].to_string()))
                .child(
                    div()
                        .bg(Theme::alpha(t.accent, 0x66))
                        .rounded_sm()
                        .child(self.query[bl..bh].to_string()),
                )
                .child(div().child(self.query[bh..].to_string()))
        } else {
            // Insert a caret bar at the cursor position.
            let cursor = self.query_cursor.min(self.query.chars().count());
            let b = self.query_byte(cursor);
            let (before, after) = self.query.split_at(b);
            div()
                .text_color(rgb(t.text))
                .child(format!("{before}\u{2502}{after}"))
        };

        // What Enter would do to the current selection (footer hint).
        let enter_label = match self.palette_items.get(self.selected).map(|i| &i.action) {
            Some(Action::Open(_, true)) => "Open folder",
            Some(Action::Open(_, false)) => "Open file",
            None => "Open",
        };
        let has_actions = !self.palette_action_list().is_empty();

        let panel = div()
            .w_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            // Opacity is user-configurable (Settings → Appearance); default is
            // nearly opaque so the palette doesn't read as a confusing overlay.
            .bg(Theme::alpha(t.surface, palette_alpha()))
            .rounded_lg()
            .border_1()
            .border_color(rgb(t.border_strong))
            .shadow_lg()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(rgb(t.border_strong))
                    .child(div().flex_none().text_color(rgb(t.accent)).child("›"))
                    .child(input),
            )
            // Scrollable, height-capped results with a scroll indicator.
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .id("palette-results")
                            .h(px(list_h))
                            .overflow_y_scroll()
                            .track_scroll(&self.palette_scroll)
                            .on_scroll_wheel(cx.listener(|_, _: &ScrollWheelEvent, _, cx| {
                                cx.notify()
                            }))
                            .flex()
                            .flex_col()
                            .children(rows),
                    )
                    .children(self.palette_scrollbar_thumb()),
            )
            // Footer: what you can do with the selection.
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_3()
                    .py_1()
                    .border_t_1()
                    .border_color(rgb(t.border_strong))
                    .text_size(px(UI_SMALL_FONT_SIZE))
                    .text_color(rgb(t.text_dim))
                    .child(palette_hint("↩", enter_label))
                    .when(has_actions, |f| f.child(palette_hint("⌘K", "Actions")))
                    .child(palette_hint("↑↓", "Navigate"))
                    .child(palette_hint("esc", "Close")),
            );

        // Wrapper: positions the palette and hosts the ⌘K dropdown OUTSIDE the
        // panel's overflow_hidden, so the menu is never clipped.
        let mut wrap = div().relative().mt(px(90.0)).w(px(680.0)).child(panel);

        // The ⌘K actions panel: drops down from the palette's bottom-right
        // corner (there's always window space below — the palette hugs the top).
        if let Some(sel) = self.palette_actions {
            let acts = self.palette_action_list();
            if !acts.is_empty() {
                let sel = sel.min(acts.len() - 1);
                wrap = wrap.child(
                    div()
                        .absolute()
                        .top(relative(1.0))
                        .right(px(10.0))
                        .mt(px(6.0))
                        .w(px(240.0))
                        .flex()
                        .flex_col()
                        .py_1()
                        .bg(Theme::alpha(t.surface, 0xf7))
                        .rounded_md()
                        .border_1()
                        .border_color(rgb(t.border_strong))
                        .shadow_lg()
                        .children(acts.into_iter().enumerate().map(|(i, (act, label))| {
                            let selected = i == sel;
                            div()
                                .id(("pal-act", i))
                                .px_3()
                                .py_1()
                                .cursor_pointer()
                                .text_color(rgb(t.text))
                                .when(selected, |s| s.bg(rgb(t.selected)))
                                .when(!selected, |s| s.hover(|s| s.bg(rgb(t.hover))))
                                .child(label)
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.palette_actions = None;
                                    this.run_palette_action(act, cx);
                                }))
                                .into_any_element()
                        })),
                );
            }
        }

        // Backdrop covering the window, with the panel near the top.
        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .flex()
            .justify_center()
            // Align to top so the panel hugs its content height instead of
            // stretching to fill the whole window.
            .items_start()
            .bg(rgba(0x00000033))
            // Block scroll/click from reaching the file list behind the palette.
            .occlude()
            .child(wrap)
    }

    /// A floating, draggable scrollbar thumb sized/positioned from the list's
    /// scroll state. Returns `None` when the content fits (or isn't measured yet).
    fn scrollbar_thumb(&self, pane: usize, cx: &Context<Self>) -> Option<AnyElement> {
        let state = self.tab(pane).scroll_handle.0.borrow();
        let base = &state.base_handle;
        let viewport = f64::from(base.bounds().size.height) as f32;
        let max = f64::from(base.max_offset().height) as f32;
        if viewport <= 1.0 || max <= 1.0 {
            return None;
        }
        let scrolled = (-(f64::from(base.offset().y) as f32)).clamp(0.0, max);
        let content = viewport + max;
        let min_thumb = 28.0_f32;
        let thumb_h = (viewport * viewport / content).clamp(min_thumb, viewport);
        let thumb_top = (viewport - thumb_h) * (scrolled / max);

        // macOS-style overlay bar: solid while scrolling or dragging, fades out
        // shortly after the last scroll, and disappears entirely once faded.
        let dragging = self.scroll_drag.is_some_and(|d| d.pane == pane);
        let tab = self.tab(pane);
        let idle = tab.last_scroll.elapsed().as_secs_f32();
        if !dragging && idle > SCROLLBAR_LINGER + SCROLLBAR_FADE {
            return None;
        }
        // Derive the thumb from the theme's text color so it stays visible on
        // light themes (a hardcoded white thumb vanished on light backgrounds).
        let color = if dragging {
            Theme::alpha(theme().text, 0x66)
        } else {
            Theme::alpha(theme().text, 0x33)
        };

        let thumb = div()
            .id(("scrollbar-thumb", pane))
            .absolute()
            .top(px(thumb_top))
            .right(px(2.0))
            .w(px(8.0))
            .h(px(thumb_h))
            .rounded_full()
            .bg(color)
            .cursor(CursorStyle::PointingHand)
            .hover(|s| s.bg(Theme::alpha(theme().text, 0x55)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                    this.begin_scroll_drag(pane, f64::from(ev.position.y) as f32);
                    cx.notify();
                }),
            );

        if dragging || idle < SCROLLBAR_LINGER {
            Some(thumb.into_any_element())
        } else {
            // Idle: ease the thumb's opacity out. The epoch keys the animation
            // so each scroll burst restarts the fade from opaque.
            Some(
                thumb
                    .with_animation(
                        SharedString::from(format!("sb-fade-{pane}-{}", tab.scroll_epoch)),
                        Animation::new(Duration::from_millis((SCROLLBAR_FADE * 1000.0) as u64))
                            .with_easing(ease_out_quint()),
                        move |el, t| el.opacity(1.0 - t),
                    )
                    .into_any_element(),
            )
        }
    }

    /// Load `dir` as a pane's current directory: re-read contents, update the
    /// current folder, record it as most-recent, and persist. Does
    /// NOT touch back/forward history (callers manage that).
    fn load_dir_in(&mut self, pane: usize, dir: PathBuf, cx: &mut Context<Self>) {
        if self.search_in_pane(pane) { self.close_palette(cx); }
        self.rename = None;

        {
            let tab = self.tab_mut(pane);
            tab.current_dir = dir.clone();
            tab.editing_path = None;
            tab.find_query = None;
            tab.find_results.clear();
            tab.content_hits = None;
            tab.content_for = None;
            tab.selection.clear();
            tab.anchor = None;

        }
        // Recents / last-dir are for local browsing only — a remote path would
        // be treated as a (missing) local folder on next launch.
        if self.tab(pane).remote.is_none() {
            save_last_dir(&dir);

        }

        self.apply_folder_view(pane, false);
        // Fast first paint + background metadata fill.
        self.reload_pane(pane, cx);
        self.sync_terminal_directory(pane);
    }

    /// Navigate a pane into `dir` if it is a directory. New navigation truncates
    /// any forward history, then appends `dir` as the new tip.
    fn navigate_in(&mut self, pane: usize, dir: PathBuf, cx: &mut Context<Self>) {
        if self.search_in_pane(pane) { self.close_palette(cx); }
        if dir == self.tab(pane).current_dir {
            return;
        }
        // Local dirs are validated with a stat; remote dirs come from a listing
        // (we can't stat them locally), so trust the caller.
        if self.tab(pane).remote.is_none() && !dir.is_dir() {
            return;
        }
        self.active_pane = pane;
        let tab = self.tab_mut(pane);
        tab.history.truncate(tab.hist_pos + 1);
        tab.history.push(dir.clone());
        tab.hist_pos = tab.history.len() - 1;
        self.load_dir_in(pane, dir, cx);
    }

    /// Navigate the active pane (used by the palette).
    fn navigate_to(&mut self, dir: PathBuf, cx: &mut Context<Self>) {
        self.navigate_in(self.active_pane, dir, cx);
    }

    /// Go to the previous directory in a pane's history (the back arrow).
    fn go_back(&mut self, pane: usize, cx: &mut Context<Self>) {
        self.active_pane = pane;
        let tab = self.tab_mut(pane);
        if tab.hist_pos == 0 {
            return;
        }
        tab.hist_pos -= 1;
        let dir = tab.history[tab.hist_pos].clone();
        self.load_dir_in(pane, dir, cx);
    }

    /// Go to the next directory in a pane's history (the forward arrow).
    fn go_forward(&mut self, pane: usize, cx: &mut Context<Self>) {
        self.active_pane = pane;
        let tab = self.tab_mut(pane);
        if tab.hist_pos + 1 >= tab.history.len() {
            return;
        }
        tab.hist_pos += 1;
        let dir = tab.history[tab.hist_pos].clone();
        self.load_dir_in(pane, dir, cx);
    }

    /// Enter path-edit mode for a pane: the bar becomes an editable text field.
    fn begin_path_edit(&mut self, pane: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.active_pane = pane;
        let text = self.tab(pane).current_dir.display().to_string();
        let end = text.chars().count();
        let tab = self.tab_mut(pane);
        tab.editing_path = Some(text);
        tab.path_cursor = end;
        tab.path_anchor = None;
        window.focus(&self.focus);
        cx.notify();
    }

    /// The path editor's selection as a sorted `(start, end)` char range.
    fn path_sel(&self, pane: usize) -> Option<(usize, usize)> {
        let tab = self.tab(pane);
        match tab.path_anchor {
            Some(a) if a != tab.path_cursor => {
                Some((a.min(tab.path_cursor), a.max(tab.path_cursor)))
            }
            _ => None,
        }
    }

    /// Delete the path editor's selected range (if any), placing the cursor at
    /// its start. Returns true when something was removed. Clears the anchor.
    fn path_delete_sel(&mut self, pane: usize) -> bool {
        if let Some((lo, hi)) = self.path_sel(pane) {
            let tab = self.tab_mut(pane);
            if let Some(s) = tab.editing_path.as_mut() {
                let (bl, bh) = (char_byte(s, lo), char_byte(s, hi));
                s.replace_range(bl..bh, "");
            }
            tab.path_cursor = lo;
            tab.path_anchor = None;
            true
        } else {
            self.tab_mut(pane).path_anchor = None;
            false
        }
    }

    /// Insert `s` at the path cursor, replacing any selection first.
    fn path_insert(&mut self, pane: usize, s: &str) {
        self.path_delete_sel(pane);
        let tab = self.tab_mut(pane);
        if let Some(buf) = tab.editing_path.as_mut() {
            let b = char_byte(buf, tab.path_cursor);
            buf.insert_str(b, s);
            tab.path_cursor += s.chars().count();
        }
    }

    /// Move (or extend) the path text cursor. `word` jumps by word, `select`
    /// extends the selection (Option+Shift selects a word at a time).
    fn path_move_h(&mut self, pane: usize, left: bool, word: bool, select: bool) {
        let tab = self.tab(pane);
        let Some(s) = tab.editing_path.clone() else {
            return;
        };
        let end = s.chars().count();
        let cursor = tab.path_cursor.min(end);
        let target = match (left, word) {
            (true, true) => prev_word_boundary(&s, cursor),
            (true, false) => cursor.saturating_sub(1),
            (false, true) => next_word_boundary(&s, cursor),
            (false, false) => (cursor + 1).min(end),
        };
        let sel = self.path_sel(pane);
        let tab = self.tab_mut(pane);
        if select {
            if tab.path_anchor.is_none() {
                tab.path_anchor = Some(cursor);
            }
            tab.path_cursor = target;
            if tab.path_anchor == Some(target) {
                tab.path_anchor = None;
            }
        } else if let Some((lo, hi)) = sel {
            tab.path_cursor = if word {
                target
            } else if left {
                lo
            } else {
                hi
            };
            tab.path_anchor = None;
        } else {
            tab.path_cursor = target;
            tab.path_anchor = None;
        }
    }

    /// Keystrokes while the path bar is being edited (acts on the active pane).
    fn handle_path_edit_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let ks = &ev.keystroke;
        let cmd = ks.modifiers.platform;
        let alt = ks.modifiers.alt;
        let shift = ks.modifiers.shift;
        let pane = self.active_pane;
        match ks.key.as_str() {
            "escape" => {
                self.tab_mut(pane).editing_path = None;
                cx.notify();
            }
            "enter" => {
                if let Some(text) = self.tab_mut(pane).editing_path.take() {
                    let path = expand_path(text.trim());
                    if path.is_dir() {
                        self.navigate_in(pane, path, cx);
                    }
                }
                cx.notify();
            }
            // Cmd+Left/Right jump to start/end (extending with Shift); otherwise
            // Left/Right move by char, Option by word, Shift extends selection.
            "left" => {
                if cmd {
                    let tab = self.tab_mut(pane);
                    if shift && tab.path_anchor.is_none() {
                        tab.path_anchor = Some(tab.path_cursor);
                    }
                    tab.path_cursor = 0;
                    if !shift {
                        tab.path_anchor = None;
                    }
                } else {
                    self.path_move_h(pane, true, alt, shift);
                }
                cx.notify();
            }
            "right" => {
                if cmd {
                    let end = self
                        .tab(pane)
                        .editing_path
                        .as_ref()
                        .map_or(0, |s| s.chars().count());
                    let tab = self.tab_mut(pane);
                    if shift && tab.path_anchor.is_none() {
                        tab.path_anchor = Some(tab.path_cursor);
                    }
                    tab.path_cursor = end;
                    if !shift {
                        tab.path_anchor = None;
                    }
                } else {
                    self.path_move_h(pane, false, alt, shift);
                }
                cx.notify();
            }
            "a" if cmd => {
                let end = self
                    .tab(pane)
                    .editing_path
                    .as_ref()
                    .map_or(0, |s| s.chars().count());
                let tab = self.tab_mut(pane);
                if end == 0 {
                    tab.path_anchor = None;
                } else {
                    tab.path_anchor = Some(0);
                    tab.path_cursor = end;
                }
                cx.notify();
            }
            "backspace" => {
                if !self.path_delete_sel(pane) {
                    let tab = self.tab_mut(pane);
                    if tab.path_cursor > 0 {
                        if let Some(s) = tab.editing_path.as_mut() {
                            let start = char_byte(s, tab.path_cursor - 1);
                            let stop = char_byte(s, tab.path_cursor);
                            s.replace_range(start..stop, "");
                        }
                        tab.path_cursor -= 1;
                    }
                }
                cx.notify();
            }
            "c" if cmd => {
                // Copy the selection if there is one, else the whole path.
                let text = match self.path_sel(pane) {
                    Some((lo, hi)) => self.tab(pane).editing_path.as_ref().map(|s| {
                        s[char_byte(s, lo)..char_byte(s, hi)].to_string()
                    }),
                    None => self.tab(pane).editing_path.clone(),
                };
                if let Some(text) = text {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                }
            }
            "v" if cmd => {
                if let Some(t) = cx.read_from_clipboard().and_then(|i| i.text()) {
                    self.path_insert(pane, t.trim());
                    cx.notify();
                }
            }
            _ => {
                if cmd {
                    return; // leave other Cmd-combos alone
                }
                if let Some(ch) = ks.key_char.as_ref() {
                    if !ch.is_empty() && !ch.chars().any(char::is_control) {
                        self.path_insert(pane, ch);
                        cx.notify();
                    }
                }
            }
        }
    }

    // ----- in-directory find (the "/" filter) -----

    /// Open the find bar for a pane, filtering only its current directory.
    fn open_find(&mut self, pane: usize, cx: &mut Context<Self>) {
        self.active_pane = pane;
        let tab = self.tab_mut(pane);
        tab.find_query = Some(String::new());
        tab.find_cursor = 0;
        tab.find_anchor = None;
        self.recompute_find(pane);
        cx.notify();
    }

    /// The find query's selected range (char indices), if any.
    fn find_sel(&self, pane: usize) -> Option<(usize, usize)> {
        let tab = self.tab(pane);
        match tab.find_anchor {
            Some(a) if a != tab.find_cursor => {
                Some((a.min(tab.find_cursor), a.max(tab.find_cursor)))
            }
            _ => None,
        }
    }

    /// Delete the find query's selected range (if any); cursor goes to its start.
    fn find_delete_sel(&mut self, pane: usize) -> bool {
        if let Some((lo, hi)) = self.find_sel(pane) {
            let tab = self.tab_mut(pane);
            if let Some(s) = tab.find_query.as_mut() {
                let (bl, bh) = (char_byte(s, lo), char_byte(s, hi));
                s.replace_range(bl..bh, "");
            }
            tab.find_cursor = lo;
            tab.find_anchor = None;
            true
        } else {
            self.tab_mut(pane).find_anchor = None;
            false
        }
    }

    /// Insert `s` at the find cursor, replacing any selection first.
    fn find_insert(&mut self, pane: usize, s: &str) {
        self.find_delete_sel(pane);
        let tab = self.tab_mut(pane);
        if let Some(buf) = tab.find_query.as_mut() {
            let b = char_byte(buf, tab.find_cursor);
            buf.insert_str(b, s);
            tab.find_cursor += s.chars().count();
        }
    }

    /// Move (or extend) the find text cursor. `word` jumps by word, `select`
    /// extends the selection (Option+Shift selects a word at a time).
    fn find_move_h(&mut self, pane: usize, left: bool, word: bool, select: bool) {
        let tab = self.tab(pane);
        let Some(s) = tab.find_query.clone() else {
            return;
        };
        let end = s.chars().count();
        let cursor = tab.find_cursor.min(end);
        let target = match (left, word) {
            (true, true) => prev_word_boundary(&s, cursor),
            (true, false) => cursor.saturating_sub(1),
            (false, true) => next_word_boundary(&s, cursor),
            (false, false) => (cursor + 1).min(end),
        };
        let sel = self.find_sel(pane);
        let tab = self.tab_mut(pane);
        if select {
            if tab.find_anchor.is_none() {
                tab.find_anchor = Some(cursor);
            }
            tab.find_cursor = target;
            if tab.find_anchor == Some(target) {
                tab.find_anchor = None;
            }
        } else if let Some((lo, hi)) = sel {
            tab.find_cursor = if word {
                target
            } else if left {
                lo
            } else {
                hi
            };
            tab.find_anchor = None;
        } else {
            tab.find_cursor = target;
            tab.find_anchor = None;
        }
    }

    /// Rebuild the visible results after a query, type, sort or directory change.
    fn recompute_find(&mut self, pane: usize) {
        self.tab_mut(pane).recompute_results();
    }

    /// Kick off (or clear) the Spotlight content search for a pane's filter.
    /// Called whenever the filter text changes. Runs `mdfind` scoped to the
    /// folder off-thread, then recomputes the filtered list.
    fn update_content_search(&mut self, pane: usize, cx: &mut Context<Self>) {
        let term = self
            .tab(pane)
            .find_query
            .as_deref()
            .map(|q| FilterQuery::parse(q).content)
            .unwrap_or(None);

        let Some(term) = term else {
            // No content: operator — drop any stale hits.
            let tab = self.tab_mut(pane);
            if tab.content_for.is_some() {
                tab.content_hits = None;
                tab.content_for = None;
            }
            return;
        };
        // Already resolved for this exact term → nothing to do.
        if self.tab(pane).content_for.as_deref() == Some(term.as_str()) {
            return;
        }
        let dir = self.tab(pane).current_dir.clone();
        let gen = {
            let tab = self.tab_mut(pane);
            tab.content_gen += 1;
            tab.content_gen
        };
        cx.spawn(async move |this, cx| {
            let d = dir.clone();
            let t = term.clone();
            let hits = cx
                .background_spawn(async move { mdfind_content(&d, &t) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if pane >= this.panes.len() || this.tab(pane).content_gen != gen {
                    return;
                }
                let tab = this.tab_mut(pane);
                tab.content_hits = Some(hits);
                tab.content_for = Some(term);
                this.recompute_find(pane);
                cx.notify();
            });
        })
        .detach();
    }

    /// Keystrokes while the find bar is open (acts on the active pane).
    fn handle_find_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let ks = &ev.keystroke;
        let cmd = ks.modifiers.platform;
        let alt = ks.modifiers.alt;
        let shift = ks.modifiers.shift;
        let pane = self.active_pane;
        match ks.key.as_str() {
            "escape" => {
                let tab = self.tab_mut(pane);
                tab.find_query = None;
                tab.find_results.clear();
                tab.content_hits = None;
                tab.content_for = None;
                self.recompute_find(pane);
                cx.notify();
            }
            "enter" => {
                // Open the top match: navigate into dirs, open files.
                let tab = self.tab(pane);
                if let Some(&i) = tab.find_results.first() {
                    let entry = &tab.entries[i];
                    let target = tab.current_dir.join(&entry.name);
                    let is_dir = entry.is_dir;
                    let t = self.tab_mut(pane);
                    t.find_query = None;
                    t.find_results.clear();
                    t.content_hits = None;
                    t.content_for = None;
                    self.recompute_find(pane);
                    self.open_path(pane, target, is_dir, cx);
                } else {
                    cx.notify();
                }
            }
            // Cmd+Left/Right jump to start/end (Shift extends); otherwise Left/
            // Right move by char, Option by word, Shift extends the selection.
            "left" => {
                if cmd {
                    let tab = self.tab_mut(pane);
                    if shift && tab.find_anchor.is_none() {
                        tab.find_anchor = Some(tab.find_cursor);
                    }
                    tab.find_cursor = 0;
                    if !shift {
                        tab.find_anchor = None;
                    }
                } else {
                    self.find_move_h(pane, true, alt, shift);
                }
                cx.notify();
            }
            "right" => {
                if cmd {
                    let end = self
                        .tab(pane)
                        .find_query
                        .as_ref()
                        .map_or(0, |s| s.chars().count());
                    let tab = self.tab_mut(pane);
                    if shift && tab.find_anchor.is_none() {
                        tab.find_anchor = Some(tab.find_cursor);
                    }
                    tab.find_cursor = end;
                    if !shift {
                        tab.find_anchor = None;
                    }
                } else {
                    self.find_move_h(pane, false, alt, shift);
                }
                cx.notify();
            }
            "a" if cmd => {
                let end = self
                    .tab(pane)
                    .find_query
                    .as_ref()
                    .map_or(0, |s| s.chars().count());
                let tab = self.tab_mut(pane);
                if end == 0 {
                    tab.find_anchor = None;
                } else {
                    tab.find_anchor = Some(0);
                    tab.find_cursor = end;
                }
                cx.notify();
            }
            "backspace" => {
                if !self.find_delete_sel(pane) {
                    let tab = self.tab_mut(pane);
                    if tab.find_cursor > 0 {
                        if let Some(s) = tab.find_query.as_mut() {
                            let start = char_byte(s, tab.find_cursor - 1);
                            let stop = char_byte(s, tab.find_cursor);
                            s.replace_range(start..stop, "");
                        }
                        tab.find_cursor -= 1;
                    }
                }
                self.recompute_find(pane);
                self.update_content_search(pane, cx);
                cx.notify();
            }
            "c" if cmd => {
                let text = match self.find_sel(pane) {
                    Some((lo, hi)) => self.tab(pane).find_query.as_ref().map(|s| {
                        s[char_byte(s, lo)..char_byte(s, hi)].to_string()
                    }),
                    None => self.tab(pane).find_query.clone(),
                };
                if let Some(text) = text {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                }
            }
            "v" if cmd => {
                if let Some(t) = cx.read_from_clipboard().and_then(|i| i.text()) {
                    self.find_insert(pane, t.trim());
                    self.recompute_find(pane);
                    self.update_content_search(pane, cx);
                    cx.notify();
                }
            }
            _ => {
                if cmd {
                    return;
                }
                if let Some(ch) = ks.key_char.as_ref() {
                    if !ch.is_empty() && !ch.chars().any(char::is_control) {
                        self.find_insert(pane, ch);
                        self.recompute_find(pane);
                        self.update_content_search(pane, cx);
                        cx.notify();
                    }
                }
            }
        }
    }

    // ----- tabs & split panes -----

    /// Record that the split just collapsed so the surviving pane animates out
    /// of its old footprint. `removed_left` = the removed pane was the left one
    /// (the survivor grows leftward from the right edge). Call after removing
    /// the pane but *before* resetting `split_ratio`.
    fn begin_collapse_anim(&mut self, removed_left: bool) {
        let survivor_w = if removed_left {
            1.0 - self.split_ratio
        } else {
            self.split_ratio
        };
        self.collapse_anim = Some((survivor_w, removed_left));
        self.split_epoch += 1;
    }

    /// Record scroll activity on a pane's list (keeps its overlay scrollbar
    /// visible) and make sure a low-rate ticker is alive to repaint once the
    /// fade-out is due — without it the thumb would linger until the next
    /// unrelated repaint.
    fn mark_scrolled(&mut self, pane: usize, cx: &mut Context<Self>) {
        {
            let tab = self.tab_mut(pane);
            tab.last_scroll = Instant::now();
            tab.scroll_epoch += 1;
        }
        if self.fade_ticker {
            return;
        }
        self.fade_ticker = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                let all_faded = this.update(cx, |this, cx| {
                    cx.notify();
                    let done = this.panes.iter().flat_map(|p| p.tabs.iter()).all(|t| {
                        t.last_scroll.elapsed().as_secs_f32() > SCROLLBAR_LINGER + SCROLLBAR_FADE
                    });
                    if done {
                        this.fade_ticker = false;
                    }
                    done
                });
                match all_faded {
                    Ok(false) => {}
                    _ => break,
                }
            }
        })
        .detach();
    }

    /// Open a new tab in `pane`, starting in that pane's current directory.
    fn new_tab_in(&mut self, pane: usize, cx: &mut Context<Self>) {
        if self.palette_open && self.file_search.inline { self.close_palette(cx); }
        let tab = self.tab(pane);
        let origin = tab.remote.as_ref().and_then(|_| tab.terminal.as_ref())
            .and_then(|s| s.ssh.as_ref()).map(|s| s.origin().to_path_buf());
        let dir = origin.unwrap_or_else(|| tab.current_dir.clone());
        let p = self.pane_mut(pane);
        p.tabs.push(Tab::new(dir));
        p.active = p.tabs.len() - 1;
        self.active_pane = pane;
        self.apply_folder_view(pane, false);
        // Fill the new tab's metadata in the background.
        self.reload_pane(pane, cx);
    }

    /// Connect to a saved SFTP server: open a new tab, resolve the remote home
    /// directory in the background, and browse it. Errors surface in the banner.
    fn connect_sftp(&mut self, server: SftpServer, cx: &mut Context<Self>) {
        // Open a placeholder remote tab in the active pane immediately.
        let pane = self.active_pane;
        let mut tab = Tab::new(home_dir()); // temporary local dir; replaced below
        tab.remote = Some(server.clone());
        tab.current_dir = PathBuf::from("/");
        tab.history = vec![PathBuf::from("/")];
        tab.hist_pos = 0;
        tab.entries.clear();
        let p = self.pane_mut(pane);
        p.tabs.push(tab);
        p.active = p.tabs.len() - 1;
        let tab_ix = p.active;
        self.active_pane = pane;
        self.remote_error = None;
        cx.notify();

        let use_system = prefs().ssh_use_system;
        cx.spawn(async move |this, cx| {
            let s = server.clone();
            let home = cx
                .background_spawn(async move { sftp_home(&s, use_system) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if pane >= this.panes.len() || tab_ix >= this.panes[pane].tabs.len() {
                    return;
                }
                match home {
                    Ok(home) => {
                        let dir = PathBuf::from(&home);
                        {
                            let tab = &mut this.panes[pane].tabs[tab_ix];
                            tab.current_dir = dir.clone();
                            tab.history = vec![dir];
                            tab.hist_pos = 0;
                        }
                        this.reload_pane(pane, cx);
                    }
                    Err(e) => {
                        // Drop the failed placeholder tab and reopen the editor
                        // pre-filled so the user can fix the settings and retry.
                        if pane < this.panes.len() && tab_ix < this.panes[pane].tabs.len() {
                            this.close_tab(pane, tab_ix, cx);
                        }
                        this.remote_error = Some(format!("{}: {e}", server.name));
                        this.edit_server(&server, cx);
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    /// Select a tab in a pane.
    fn select_tab(&mut self, pane: usize, tab: usize, cx: &mut Context<Self>) {
        self.dismiss_space_map |= self.space_map.is_some();
        if self.palette_open && self.file_search.inline { self.close_palette(cx); }
        self.active_pane = pane;
        let p = self.pane_mut(pane);
        if tab < p.tabs.len() {
            p.active = tab;
        }
        cx.notify();
        self.prewarm_icons(cx);
        // If this tab only got the fast pass, finish loading its metadata.
        if self.tab(pane).entries.is_empty() || self.tab(pane).entries.iter().any(|e| !e.loaded) {
            self.reload_pane(pane, cx);
        }
    }

    /// Close a tab. Closing a pane's last tab removes the pane (collapsing the
    /// split); closing the last tab of the last pane is a no-op.
    /// Open a copy of `tab` (same folder) right after it in the same pane.
    fn duplicate_tab(&mut self, pane: usize, tab: usize, cx: &mut Context<Self>) {
        if pane >= self.panes.len() || tab >= self.panes[pane].tabs.len() {
            return;
        }
        if self.palette_open && self.file_search.inline { self.close_palette(cx); }
        let dir = self.panes[pane].tabs[tab].current_dir.clone();
        let remote = self.panes[pane].tabs[tab].remote.clone();
        let mut new = Tab::new(dir);
        new.remote = remote;
        new.entry_filter = self.panes[pane].tabs[tab].entry_filter;
        new.show_hidden = self.panes[pane].tabs[tab].show_hidden;
        new.view = self.panes[pane].tabs[tab].view;
        new.list_layout = self.panes[pane].tabs[tab].list_layout.clone();
        new.preview_width = self.panes[pane].tabs[tab].preview_width;
        new.sort_key = self.panes[pane].tabs[tab].sort_key;
        new.sort_asc = self.panes[pane].tabs[tab].sort_asc;
        let p = self.pane_mut(pane);
        p.tabs.insert(tab + 1, new);
        p.active = tab + 1;
        self.active_pane = pane;
        self.reload_pane(pane, cx);
    }

    /// Close every tab in `pane` except `keep`.
    fn close_other_tabs(&mut self, pane: usize, keep: usize, cx: &mut Context<Self>) {
        if pane >= self.panes.len() || keep >= self.panes[pane].tabs.len() {
            return;
        }
        if self.palette_open && self.file_search.inline { self.close_palette(cx); }
        let kept = self.panes[pane].tabs.remove(keep);
        let p = self.pane_mut(pane);
        p.tabs.clear();
        p.tabs.push(kept);
        p.active = 0;
        self.active_pane = pane;
        cx.notify();
    }

    fn close_tab(&mut self, pane: usize, tab: usize, cx: &mut Context<Self>) {
        if self.palette_open && self.file_search.inline { self.close_palette(cx); }
        if pane >= self.panes.len() || tab >= self.panes[pane].tabs.len() {
            return;
        }
        if self.panes[pane].tabs.len() > 1 {
            let p = self.pane_mut(pane);
            p.tabs.remove(tab);
            if p.active >= p.tabs.len() {
                p.active = p.tabs.len() - 1;
            } else if tab < p.active {
                p.active -= 1;
            }
        } else if self.panes.len() > 1 {
            self.panes.remove(pane);
            self.begin_collapse_anim(pane == 0);
            self.split_ratio = 0.5;
        } else {
            return; // last tab of the only pane — keep it
        }
        if self.active_pane >= self.panes.len() {
            self.active_pane = self.panes.len() - 1;
        }
        let shown = pane.min(self.panes.len() - 1);
        if self.tab(shown).entries.iter().any(|entry| !entry.loaded) {
            self.reload_pane(shown, cx);
        } else {
            self.prewarm_icons(cx);
        }
        cx.notify();
    }

    /// Move a dragged tab to a destination pane at `to_index`. `to_pane ==
    /// panes.len()` means "create a new pane on the right" (the drag-to-split).
    fn move_tab(&mut self, from: TabDrag, to_pane: usize, to_index: usize, cx: &mut Context<Self>) {
        TAB_DRAG_LIVE.store(false, Ordering::Relaxed);
        if from.pane >= self.panes.len() || from.tab >= self.panes[from.pane].tabs.len() {
            return;
        }
        if self.palette_open && self.file_search.inline { self.close_palette(cx); }
        // Two panes max: a split-zone drop while the canvas is already split
        // moves the tab to the end of the right pane instead of creating an
        // invisible third pane.
        let (to_pane, to_index) = if to_pane >= self.panes.len() && self.panes.len() >= 2 {
            let right = self.panes.len() - 1;
            (right, self.panes[right].tabs.len())
        } else {
            (to_pane, to_index)
        };
        let splitting = to_pane >= self.panes.len();
        // Don't bother splitting when the source pane has a single tab and we'd
        // just move it to a brand-new pane (no visible change).
        if splitting && self.panes[from.pane].tabs.len() == 1 && self.panes.len() == 1 {
            return;
        }
        let tab = self.panes[from.pane].tabs.remove(from.tab);
        // Fix up the source pane's active index after removal.
        {
            let p = &mut self.panes[from.pane];
            if !p.tabs.is_empty() {
                if p.active >= p.tabs.len() {
                    p.active = p.tabs.len() - 1;
                } else if from.tab < p.active {
                    p.active -= 1;
                }
            }
        }

        if splitting {
            // New pane on the right with just this tab.
            self.panes.push(Pane { tabs: vec![tab], active: 0 });
            self.split_ratio = 0.5;
            self.split_epoch += 1;
            self.collapse_anim = None;
            // Drop the source pane if it's now empty.
            if self.panes[from.pane].tabs.is_empty() {
                self.panes.remove(from.pane);
            }
            self.active_pane = self.panes.len() - 1;
        } else {
            // Account for removal shifting indices when within the same pane.
            let mut dst = to_pane;
            let mut idx = to_index;
            if to_pane == from.pane && from.tab < to_index {
                idx = idx.saturating_sub(1);
            }
            // Insert, then prune an emptied source pane (which may shift dst).
            let src_now_empty = self.panes[from.pane].tabs.is_empty();
            let at = idx.min(self.panes[dst].tabs.len());
            self.panes[dst].tabs.insert(at, tab);
            let new_len = self.panes[dst].tabs.len();
            self.panes[dst].active = at.min(new_len - 1);
            if src_now_empty && from.pane != dst {
                self.panes.remove(from.pane);
                if from.pane < dst {
                    dst -= 1;
                }
                self.begin_collapse_anim(from.pane == 0);
                self.split_ratio = 0.5;
            }
            self.active_pane = dst;
        }
        if self.active_pane >= self.panes.len() {
            self.active_pane = self.panes.len() - 1;
        }
        cx.notify();
        self.prewarm_icons(cx);
    }

    // ----- selection -----

    /// The paths shown in the focused list or column, in display order.
    fn display_paths(&self, pane: usize) -> Vec<PathBuf> {
        if self.search_in_pane(pane) {
            return self.palette_items.iter().map(|item| match &item.action { Action::Open(path,_)=>path.clone() }).collect();
        }
        let tab = self.tab(pane);
        tab.visible_entries().map(|entry| tab.current_dir.join(&entry.name)).collect()
    }

    /// Make `path` the inspector focus and load its preview/info.
    fn focus_entry(&mut self, pane: usize, path: PathBuf, cx: &mut Context<Self>) {
        self.tab_mut(pane).anchor = Some(path.clone());
        self.preview_page = 0;
        self.cancel_unused_previews();
        if self.space_map.is_some() { cx.notify(); return; }
        // Remote files aren't on the local disk, so QuickLook/PDF rendering
        // can't read them directly. Fetch a copy to a temp cache and preview
        // that (images & PDFs only, size-capped). Info (local stat) is skipped.
        if self.tab(pane).remote.is_some() {
            let size = {
                let tab = self.tab(pane);
                let dir = tab.current_dir.clone();
                tab.entries
                    .iter()
                    .find(|e| dir.join(&e.name) == path)
                    .map(|e| e.size)
                    .unwrap_or(0)
            };
            self.ensure_remote_preview(path, size, cx);
            cx.notify();
            return;
        }
        self.ensure_preview(path.clone(), false, cx);
        if prefs().preview && prefs().preview_pages {
            self.ensure_pdf_page(path.clone(), 0, cx);
        }
        self.ensure_info(path, cx);
    }

    /// Single-click: select just this item.
    fn select_entry(&mut self, pane: usize, path: PathBuf, cx: &mut Context<Self>) {
        self.active_pane = pane;
        self.rename = None;
        {
            let tab = self.tab_mut(pane);
            tab.selection.clear();
            tab.selection.insert(path.clone());
        }
        cx.notify();
        self.focus_entry(pane, path, cx);
    }

    /// Cmd-click: toggle this item's membership in the selection.
    fn toggle_entry(&mut self, pane: usize, path: PathBuf, cx: &mut Context<Self>) {
        self.active_pane = pane;
        self.rename = None;
        let now_selected = {
            let tab = self.tab_mut(pane);
            if tab.selection.contains(&path) {
                tab.selection.remove(&path);
                false
            } else {
                tab.selection.insert(path.clone());
                true
            }
        };
        cx.notify();
        if now_selected {
            self.focus_entry(pane, path, cx);
        }
    }

    /// Shift-click: select the contiguous range from the anchor to this item.
    fn range_select(&mut self, pane: usize, path: PathBuf, cx: &mut Context<Self>) {
        self.active_pane = pane;
        self.rename = None;
        let paths = self.display_paths(pane);
        let to = paths.iter().position(|p| p == &path);
        let from = self
            .tab(pane)
            .anchor
            .as_ref()
            .and_then(|a| paths.iter().position(|p| p == a));
        let sel: HashSet<PathBuf> = match (from, to) {
            (Some(a), Some(b)) => {
                let (lo, hi) = (a.min(b), a.max(b));
                paths[lo..=hi].iter().cloned().collect()
            }
            _ => std::iter::once(path.clone()).collect(),
        };
        self.tab_mut(pane).selection = sel;
        cx.notify();
        // Keep the existing anchor; just refresh the inspector to the clicked one.
        self.focus_entry(pane, path, cx);
    }

    /// Whether a marquee drag is in flight with real displacement — used to
    /// swallow the row click that lands on the same mouse-up (row `on_click`
    /// bubbles before the root's `end_marquee`), so finishing a box-select over
    /// a row doesn't replace the fresh multi-selection with that one row.
    /// Stationary press-releases (displacement ≈ 0) still click through.
    fn marquee_click_suppressed(&self) -> bool {
        self.marquee.is_some_and(|(pane, s, c)| {
            // The anchor is content-space; convert the cursor before comparing
            // (comparing across spaces would suppress every click once the
            // list is scrolled). Scroll during the marquee counts as movement —
            // exactly then must the click not collapse the selection.
            let (top, scrolled) = self.list_geometry(pane);
            let cur_c = c.1 - top + scrolled;
            (c.0 - s.0).abs().max((cur_c - s.1).abs()) > 6.0
        })
    }

    /// Dispatch a left-click on an item, honoring Cmd / Shift modifiers.
    fn click_entry(
        &mut self,
        pane: usize,
        path: PathBuf,
        is_dir: bool,
        ev: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.space_map.is_some() { self.close_space_map(window, cx); }
        if self.marquee_click_suppressed() {
            return;
        }
        self.active_pane = pane;
        let mods = ev.modifiers();
        if mods.platform {
            self.toggle_entry(pane, path, cx);
        } else if mods.shift {
            self.range_select(pane, path, cx);
        } else if is_dir && (self.tab(pane).remote.is_some() || !is_app_bundle(&path, is_dir)) {
            self.navigate_in(pane, path, cx);
        } else if ev.click_count() >= 2 {
            self.open_path(pane, path, is_dir, cx);
        } else {
            self.select_entry(pane, path, cx);
        }
    }

    /// A pane list's `(viewport_top, scrolled)` in window coords / content px —
    /// the numbers needed to convert between window y and content y.
    fn list_geometry(&self, pane: usize) -> (f32, f32) {
        let st = self.tab(pane).scroll_handle.0.borrow();
        let top = f64::from(st.base_handle.bounds().origin.y) as f32;
        let scrolled = (-(f64::from(st.base_handle.offset().y) as f32)).max(0.0);
        (top, scrolled)
    }

    /// Start a marquee (rubber-band) selection from empty list space.
    ///
    /// The anchor is stored in CONTENT coordinates (`y - list_top + scrolled`),
    /// not window coordinates: the list can scroll underneath an in-flight
    /// marquee (wheel/trackpad, or edge auto-scroll), and a window-space anchor
    /// would slide with the scroll — moving the box off the rows it had already
    /// swept and re-selecting the wrong ones.
    fn begin_marquee(&mut self, pane: usize, x: f32, y: f32, cx: &mut Context<Self>) {
        if mq_log() {
            eprintln!("[mq] begin pane={pane} at ({x:.0},{y:.0})");
        }
        self.active_pane = pane;
        self.rename = None;
        {
            let tab = self.tab_mut(pane);
            tab.selection.clear();
            tab.anchor = None;
        }
        let (top, scrolled) = self.list_geometry(pane);
        self.marquee = Some((pane, (x, y - top + scrolled), (x, y)));
        // Auto-scroll while the drag sits past the top/bottom edge of the list,
        // so a marquee can keep selecting through folders taller than the view.
        // A timer loop (not mouse-move driven) so it scrolls even while the
        // cursor holds still past the edge; it exits when the marquee ends.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let alive = this
                    .update(cx, |this, cx| {
                        if this.marquee.is_none() {
                            return false;
                        }
                        this.marquee_autoscroll_tick(cx);
                        true
                    })
                    .unwrap_or(false);
                if !alive {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }

    /// One 16ms step for an active marquee. Recomputes the selection against
    /// the CURRENT scroll position (the wheel/trackpad can scroll the list
    /// mid-marquee with no mouse-move event — the content-space anchor stays
    /// glued, and the rows under the box change), then, if the pointer sits in
    /// an edge zone, auto-scrolls — slow near the edge, faster further past it.
    fn marquee_autoscroll_tick(&mut self, cx: &mut Context<Self>) {
        let Some((pane, _, cur)) = self.marquee else {
            return;
        };
        if pane >= self.panes.len() {
            return;
        }
        // Track external scrolls: re-derive selection + box from the live
        // scroll offset every tick (cheap — only the covered rows).
        self.update_marquee(cur.0, cur.1, cx);
        let Some((pane, start, cur)) = self.marquee else {
            return;
        };
        // Distance past the viewport edge decides direction and speed.
        let (top, bottom, scrolled, max) = {
            let st = self.tab(pane).scroll_handle.0.borrow();
            let b = st.base_handle.bounds();
            let top = f64::from(b.origin.y) as f32;
            let bottom = top + f64::from(b.size.height) as f32;
            let scrolled = (-(f64::from(st.base_handle.offset().y) as f32)).max(0.0);
            let max = f64::from(st.base_handle.max_offset().height) as f32;
            (top, bottom, scrolled, max)
        };
        if max <= 1.0 {
            return; // everything fits; nothing to scroll
        }
        // Scroll when the cursor is within an edge ZONE inside the viewport, not
        // only past the edge — when the list reaches the window bottom the
        // cursor may never report coordinates beyond it. Zone-scrolling only
        // engages once the box has actually been dragged a little, so a plain
        // press inside a zone doesn't scroll; after that the zone side alone
        // picks the direction (an anchor-side gate here would block reversing:
        // after a long down-scroll the glued anchor clamps to the top edge and
        // no reachable in-zone position passes it).
        const ZONE: f32 = 28.0;
        // `start` is content-space; convert the cursor for the displacement gate.
        let cur_content_y = cur.1 - top + scrolled;
        let moved =
            (cur.0 - start.0).abs().max((cur_content_y - start.1).abs()) > 8.0;
        let overshoot = if moved && cur.1 < top + ZONE {
            (cur.1 - (top + ZONE)).min(-0.1) // negative → scroll up
        } else if moved && cur.1 > bottom - ZONE {
            (cur.1 - (bottom - ZONE)).max(0.1) // positive → scroll down
        } else {
            if mq_log() {
                eprintln!(
                    "[mq] tick idle: cur.y={:.0} top={top:.0} bottom={bottom:.0} moved={moved} zone={ZONE}",
                    cur.1
                );
            }
            return; // outside both zones, or the box hasn't been dragged yet
        };
        if mq_log() {
            eprintln!(
                "[mq] tick scroll: cur.y={:.0} overshoot={overshoot:.0} scrolled={scrolled:.0}/{max:.0}",
                cur.1
            );
        }
        // ~2px/tick at the zone boundary up to ~28px/tick (≈1700px/s) far past
        // the edge — "slowly or faster depending on where the mouse is".
        let speed = (overshoot.abs() * 0.22).clamp(2.0, 28.0);
        let target = (scrolled + speed.copysign(overshoot)).clamp(0.0, max);
        let applied = target - scrolled;
        if applied.abs() < 0.5 {
            return; // already at that end of the list
        }
        {
            let st = self.tab(pane).scroll_handle.0.borrow();
            let x = st.base_handle.offset().x;
            st.base_handle.set_offset(point(x, px(-target)));
        }
        let _ = applied; // the content-space anchor is scroll-invariant
        self.mark_scrolled(pane, cx); // show the scrollbar while auto-scrolling
        // Recompute the covered rows against the new scroll position.
        self.update_marquee(cur.0, cur.1, cx);
    }

    /// Update the marquee end point and recompute which rows it covers.
    fn update_marquee(&mut self, x: f32, y: f32, cx: &mut Context<Self>) {
        let Some((pane, start, _)) = self.marquee else {
            return;
        };
        if mq_log() {
            eprintln!("[mq] move ({x:.0},{y:.0})");
        }
        self.marquee = Some((pane, start, (x, y)));

        // Map the vertical span to row indices using the list's geometry.
        let (list_top, scrolled) = self.list_geometry(pane);
        // The anchor (`start.1`) is content-space; only the cursor converts.
        let cur_c = y - list_top + scrolled;
        let c0 = start.1.min(cur_c).max(0.0);
        let c1 = start.1.max(cur_c).max(0.0);
        let i0 = (c0 / ROW_H).floor() as i64;
        let i1 = (c1 / ROW_H).floor() as i64;
        // Row 0 is the ".." entry when present; offset to display indices.
        let has_parent =
            !self.tab(pane).is_filtered() && prefs().show_parent && self.tab(pane).current_dir.parent().is_some();
        let off = i64::from(has_parent);
        // Only touch the covered rows — cloning every path in the directory per
        // mouse-move made marquee drags crawl in large folders.
        let mut sel = HashSet::new();
        {
            let tab = self.tab(pane);
            let find = tab.is_filtered();
            let len = if find {
                tab.find_results.len()
            } else {
                tab.entries.len()
            };
            for i in i0..=i1 {
                let di = i - off;
                if di < 0 {
                    continue;
                }
                let di = di as usize;
                if di >= len {
                    break;
                }
                let ix = if find { tab.find_results[di] } else { di };
                sel.insert(tab.current_dir.join(&tab.entries[ix].name));
            }
        }
        self.tab_mut(pane).selection = sel;
        cx.notify();
    }

    fn end_marquee(&mut self, cx: &mut Context<Self>) {
        if let Some((pane, _, _)) = self.marquee.take() {
            // Give the marquee'd selection a focused item so Enter (rename),
            // the inspector, and shift-extension have a target.
            if self.tab(pane).anchor.is_none() {
                let last = self
                    .display_paths(pane)
                    .into_iter()
                    .rev()
                    .find(|p| self.tab(pane).selection.contains(p));
                if let Some(p) = last {
                    self.focus_entry(pane, p, cx);
                }
            }
            cx.notify();
        }
    }

    /// Recompute [`Self::drop_hover`] from the drag position. Runs on every
    /// (synthesized) mouse move while a drag is in flight; notifies only when
    /// the target actually changes.
    fn update_drop_hover(&mut self, x: f32, y: f32, cx: &mut Context<Self>) {
        if !cx.has_active_drag() {
            TAB_DRAG_LIVE.store(false, Ordering::Relaxed);
        }
        let next = if cx.has_active_drag() && !TAB_DRAG_LIVE.load(Ordering::Relaxed) {
            self.compute_drop_hover(x, y)
        } else {
            None
        };
        if self.drop_hover != next {
            self.drop_hover = next;
            cx.notify();
        }
    }

    /// Which drop target is under (x, y): a folder row's name cell → that
    /// folder, anywhere else in a pane → the pane's directory. Mirrors the
    /// actual drop hitboxes (name cell = 12px row padding + name column).
    fn compute_drop_hover(&self, x: f32, y: f32) -> Option<(usize, Option<usize>)> {
        for pane in 0..self.panes.len() {
            let tab = self.tab(pane);
            let (origin, size, scrolled) = {
                let st = tab.scroll_handle.0.borrow();
                let b = st.base_handle.bounds();
                let scr = (-(f64::from(st.base_handle.offset().y) as f32)).max(0.0);
                (b.origin, b.size, scr)
            };
            let (y0, h) = (f64::from(origin.y) as f32, f64::from(size.height) as f32);
            // The pane's horizontal extent must come from the VIEWPORT (the
            // hscroll wrapper) — the list's own bounds grow to the full column
            // width, which can silently extend underneath the neighbor pane
            // when the columns are wider than a split pane.
            let vp = tab.h_scroll.bounds();
            let (mut x0, mut w) = (
                f64::from(vp.origin.x) as f32,
                f64::from(vp.size.width) as f32,
            );
            if w <= 1.0 {
                // Views that don't track the hscroll wrapper (Icons, …) use
                // the list bounds, which don't overflow there.
                x0 = f64::from(origin.x) as f32;
                w = f64::from(size.width) as f32;
            }
            if w <= 1.0 || x < x0 || x >= x0 + w {
                continue;
            }
            // Fine-grained folder targeting only in the List view.
            if tab.view != ViewMode::List || y < y0 || y >= y0 + h {
                return Some((pane, None));
            }
            let row = ((y - y0 + scrolled) / ROW_H).floor();
            if row < 0.0 {
                return Some((pane, None));
            }
            let row = row as usize;
            let find_active = tab.is_filtered();
            let has_parent =
                !find_active && prefs().show_parent && tab.current_dir.parent().is_some();
            let count = if find_active {
                tab.find_results.len()
            } else {
                tab.entries.len() + usize::from(has_parent)
            };
            if row >= count {
                return Some((pane, None));
            }
            // Inside the name cell horizontally? (Row padding is px_3 = 12,
            // shifted by any horizontal column scroll.)
            let hx = f64::from(tab.h_scroll.offset().x) as f32;
            let name_x0 = x0 + hx + 12.0 + tab.list_layout.offset(Column::Name);
            if x < name_x0 || x >= name_x0 + tab.list_layout.width(Column::Name) {
                return Some((pane, None));
            }
            // ".." row targets the parent; entries target real folders only.
            let is_folder = if has_parent && row == 0 {
                true
            } else {
                let ix = if find_active {
                    tab.find_results[row]
                } else {
                    row - usize::from(has_parent)
                };
                tab.entries.get(ix).is_some_and(|e| e.is_folder())
            };
            return Some((pane, is_folder.then_some(row)));
        }
        None
    }

    /// If a left-press on a row has moved past the drag threshold, hand the
    /// current selection to a native macOS drag session so the files can be
    /// dropped into Finder, Claude, Mail, or any other app (and back into
    /// Shuffle's own folders/panes, which arrive as an external-file drop).
    fn maybe_start_os_drag(&mut self, x: f32, y: f32, window: &Window, cx: &mut Context<Self>) {
        let Some((pane, path, (sx, sy))) = self.drag_candidate.clone() else {
            return;
        };
        if (x - sx).abs() < 6.0 && (y - sy).abs() < 6.0 {
            return; // still within the click slop; not a drag yet
        }
        self.drag_candidate = None;
        self.marquee = None;

        // Drag the whole selection if the pressed item is part of it; otherwise
        // just the pressed item.
        let sel = &self.tab(pane).selection;
        let mut paths: Vec<PathBuf> = if sel.contains(&path) {
            sel.iter().cloned().collect()
        } else {
            vec![path.clone()]
        };
        paths.sort();

        if let Some(view) = ns_view_ptr(window) {
            start_os_file_drag(view, &paths);
        }
        cx.notify();
    }

    /// The visible marquee rectangle for `pane` (in the listing-local frame).
    fn marquee_rect(&self, pane: usize) -> Option<AnyElement> {
        let (mp, start, cur) = self.marquee?;
        if mp != pane {
            return None;
        }
        let (ox, oy) = {
            let st = self.tab(pane).scroll_handle.0.borrow();
            let o = st.base_handle.bounds().origin;
            (f64::from(o.x) as f32, f64::from(o.y) as f32)
        };
        // The anchor is content-space: convert to window y with the LIVE scroll
        // offset, so the box's fixed edge glides with the content when the list
        // scrolls under an in-flight marquee.
        let (_, scrolled) = self.list_geometry(pane);
        let start_y = start.1 + oy - scrolled;
        let x = start.0.min(cur.0) - ox;
        let y = start_y.min(cur.1) - oy;
        let w = (start.0 - cur.0).abs();
        let mut h = (start_y - cur.1).abs();
        // Clamp to the container: when the anchored edge has scrolled off the
        // top, shrink the height by the clipped amount (clamping only `top`
        // would push the box's far edge past the cursor).
        if y < 0.0 {
            h = (h + y).max(0.0);
        }
        let t = theme();
        Some(
            div()
                .absolute()
                .left(px(x.max(0.0)))
                .top(px(y.max(0.0)))
                .w(px(w))
                .h(px(h))
                .bg(Theme::alpha(t.accent, 0x22))
                .border_1()
                .border_color(rgb(t.accent))
                .into_any_element(),
        )
    }

    // ----- keyboard arrow navigation -----

    /// Columns across the grid in Icons view (from the last measured width).
    fn icon_cols(&self, pane: usize) -> usize {
        let width = browser_width(pane).max(240.0);
        ((width / 108.0).floor() as usize).max(1)
    }

    /// Move the selection with an arrow key. `dx`/`dy` are -1/0/1.
    fn arrow_move(&mut self, pane: usize, dx: i32, dy: i32, cx: &mut Context<Self>) {
        self.dismiss_space_map |= self.space_map.is_some();
        self.active_pane = pane;
        match self.tab(pane).view {
            ViewMode::Icons => {
                let cols = self.icon_cols(pane) as i32;
                self.arrow_grid(pane, dx + dy * cols, cx);
            }
            // Lists are 1-D: only up/down move.
            _ => {
                if dy != 0 {
                    self.arrow_grid(pane, dy, cx);
                }
            }
        }
    }

    /// Move the anchor by `delta` positions in display order.
    fn arrow_grid(&mut self, pane: usize, delta: i32, cx: &mut Context<Self>) {
        let paths = self.display_paths(pane);
        if paths.is_empty() {
            return;
        }
        let cur = self
            .tab(pane)
            .anchor
            .as_ref()
            .and_then(|a| paths.iter().position(|p| p == a));
        let ni = match cur {
            Some(i) => (i as i32 + delta).clamp(0, paths.len() as i32 - 1),
            None => 0,
        };
        let target = paths[ni as usize].clone();
        {
            let tab = self.tab_mut(pane);
            tab.selection.clear();
            tab.selection.insert(target.clone());
        }
        // Keep the row/cell visible (List uses a ".." offset; Icons rows of N).
        let view = self.tab(pane).view;
        let offset = usize::from(
            !self.tab(pane).is_filtered()
                && prefs().show_parent
                && self.tab(pane).current_dir.parent().is_some(),
        );
        let item = match view {
            ViewMode::Icons => ni as usize / self.icon_cols(pane),
            _ => ni as usize + offset,
        };
        self.tab(pane).scroll_handle.scroll_to_item(item, ScrollStrategy::Center);
        // Keyboard scrolling shows the overlay scrollbar too.
        self.mark_scrolled(pane, cx);
        self.focus_entry(pane, target, cx);
    }


    /// The floating "why this name won't work" pill shown over `pane` while an
    /// in-progress rename there has an invalid name.
    fn rename_error_pill(&self, pane: usize) -> Option<AnyElement> {
        let r = self.rename.as_ref()?;
        if r.pane != pane {
            return None;
        }
        let msg = self.rename_error()?;
        let t = theme();
        Some(
            div()
                .absolute()
                .bottom(px(16.0))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .py_2()
                        .rounded_lg()
                        .bg(Theme::alpha(t.surface, 0xf2))
                        .border_1()
                        .border_color(rgb(RENAME_ERR_COLOR))
                        .shadow_lg()
                        .text_color(rgb(RENAME_ERR_COLOR))
                        .child("⚠")
                        .child(div().text_color(rgb(t.text)).child(msg)),
                )
                .into_any_element(),
        )
    }

    /// The right-hand inspector: preview and/or information for the selected
    /// file. `None` when neither feature is on or nothing is selected.
    fn render_inspector(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let p = prefs();
        let show_preview = p.preview;
        if !show_preview && !p.info {
            return None;
        }
        let tab = self.active_tab();
        if tab.selection.len() != 1 { return None; }
        let sel = tab.selection.iter().next()?.clone();
        if show_preview && tab.remote.is_none() && video_preview::is_video(&sel) {
            return self.render_video_preview(cx);
        }
        let text = TEXT_PREVIEW_CACHE.with(|c| c.borrow().get(&sel).cloned().flatten());
        let visual = lookup_preview(&sel).flatten().is_some()
            || lookup_pdf_page(&sel, self.preview_page).flatten().is_some();
        if text.is_none() && !visual { return None; }
        let t = theme();
        if show_preview {
            if let Some(text) = text {
                return Some(
                    div().id("inspector").flex_none().w_full().h_full()
                        .min_h_0().flex().flex_col().gap_3().p_4()
                        .bg(rgb(t.sidebar)).border_l_1().border_color(rgb(t.border))
                        .child(div().flex_none().font_weight(FontWeight::SEMIBOLD)
                            .truncate().child(path_label(&sel)))
                        .child(div().id(SharedString::from(format!("text-preview:{}", sel.display())))
                            .flex_1().min_h_0().overflow_y_scroll().p_4().rounded_md()
                            .bg(rgb(t.bg)).text_color(rgb(t.text))
                            .text_size(px(14.0)).font_family("Menlo")
                            .child(if text.text.is_empty() { "Empty text file".to_string() } else { text.text.clone() }))
                        .when(text.truncated, |d| d.child(div().text_size(px(UI_SMALL_FONT_SIZE))
                            .text_color(rgb(t.text_muted)).child("Preview shortened for this large file.")))
                        .into_any_element()
                );
            }
        }

        let mut col = div()
            .id("inspector")
            .flex_none()
            .w_full()
            .h_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_3()
            .when(!video_preview::is_picture(&sel), |d| d.p_4())
            .bg(rgb(t.sidebar))
            .border_l_1()
            .border_color(rgb(t.border))
            .child(
                div()
                    .flex_none()
                    .text_color(rgb(t.text))
                    .truncate()
                    .child(path_label(&sel)),
            );

        if show_preview {
            // Multi-page PDFs: show the natively rendered current page when the
            // pager is on, falling back to the QuickLook thumbnail (page 1)
            // while it builds.
            let paging = p.preview_pages && is_pdf(&sel);
            let page_count = if paging { lookup_pdf_count(&sel) } else { None };
            let page = self
                .preview_page
                .min(page_count.unwrap_or(1).saturating_sub(1));
            let page_img = if paging {
                lookup_pdf_page(&sel, page).flatten()
            } else {
                None
            };
            let handle = page_img.or_else(|| lookup_preview(&sel).flatten());
            let body: AnyElement = match handle {
                Some(handle) => {
                    let image = img(ImageSource::Render(handle)).object_fit(ObjectFit::Contain);
                    if video_preview::is_picture(&sel) { image.w_full().into_any_element() }
                    else { image.max_w_full().max_h(px(600.)).into_any_element() }
                },
                // Not ready yet or unavailable → show the file's icon.
                None => icon_element_sized(&sel, false, 96.0),
            };
            let preview_box = div()
                .relative()
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .w_full()
                .min_h(px(120.0))
                // Clip so a tall preview can't paint over the title above or the
                // "Information" section below.
                .when(!video_preview::is_picture(&sel), |d| d.max_h(px(620.)).p_2())
                .overflow_hidden()
                .rounded_md()
                // White "page" so document/text previews (black text, often
                // transparent background) stay readable on dark themes.
                .bg(rgb(0xffffff))
                .child(body);

            // Finder-style ‹ 2 / 14 › pill over the bottom of the preview.
            if let Some(count) = page_count.filter(|&c| c > 1) {
                let prev_sel = sel.clone();
                let next_sel = sel.clone();
                let pager_btn = |id: &'static str, label: &'static str, enabled: bool| {
                    div()
                        .id(id)
                        .px_1p5()
                        .rounded_full()
                        .cursor_pointer()
                        .text_color(if enabled {
                            rgba(0xffffffff)
                        } else {
                            rgba(0xffffff44)
                        })
                        .when(enabled, |s| s.hover(|s| s.bg(rgba(0xffffff33))))
                        .child(label)
                };
                col = col.child(
                    preview_box.child(
                        div()
                            .absolute()
                            .bottom_2()
                            .left_0()
                            .right_0()
                            .flex()
                            .justify_center()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .px_1()
                                    .py_0p5()
                                    .rounded_full()
                                    .bg(rgba(0x000000aa))
                                    .text_size(px(UI_SMALL_FONT_SIZE))
                                    .child(pager_btn("pdf-prev", "‹", page > 0).on_click(
                                        cx.listener(move |this, _: &ClickEvent, _, cx| {
                                            if this.preview_page > 0 {
                                                this.preview_page -= 1;
                                                let pg = this.preview_page;
                                                this.ensure_pdf_page(prev_sel.clone(), pg, cx);
                                                if pg > 0 {
                                                    this.ensure_pdf_page(prev_sel.clone(), pg - 1, cx);
                                                }
                                                cx.notify();
                                            }
                                        }),
                                    ))
                                    .child(
                                        div()
                                            .text_color(rgba(0xffffffdd))
                                            .child(format!("{} / {}", page + 1, count)),
                                    )
                                    .child(pager_btn("pdf-next", "›", page + 1 < count).on_click(
                                        cx.listener(move |this, _: &ClickEvent, _, cx| {
                                            let count =
                                                lookup_pdf_count(&next_sel).unwrap_or(1);
                                            if this.preview_page + 1 < count {
                                                this.preview_page += 1;
                                                let pg = this.preview_page;
                                                this.ensure_pdf_page(next_sel.clone(), pg, cx);
                                                if pg + 1 < count {
                                                    this.ensure_pdf_page(next_sel.clone(), pg + 1, cx);
                                                }
                                                cx.notify();
                                            }
                                        }),
                                    )),
                            ),
                    ),
                );
            } else {
                col = col.child(preview_box);
            }
        }

        if p.info {
            col = col.child(settings_title("Information"));
            if let Some(info) = lookup_info(&sel) {
                let mut rows = div().flex().flex_col().gap_1();
                rows = rows.child(info_row("Kind", &info.kind));
                rows = rows.child(info_row("Size", &info.size));
                rows = rows.child(info_row("Created", &info.created));
                rows = rows.child(info_row("Modified", &info.modified));
                rows = rows.child(info_row("Last opened", &info.accessed));
                if let Some(d) = &info.dimensions {
                    rows = rows.child(info_row("Dimensions", d));
                }
                if let Some(c) = &info.color {
                    rows = rows.child(info_row("Color", c));
                }
                if let Some(s) = &info.signed {
                    rows = rows.child(info_row("Signature", s));
                }
                col = col.child(rows);
            } else {
                col = col.child(
                    div()
                        .text_size(px(UI_SMALL_FONT_SIZE))
                        .text_color(rgb(t.text_dim))
                        .child("Loading…"),
                );
            }
        }

        Some(col.into_any_element())
    }

    /// The floating filter box, anchored bottom-right while find is active.
    fn render_find_box(&self, pane: usize, query: &str) -> impl IntoElement {
        let t = theme();
        let tab = self.tab(pane);
        let count = tab.find_results.len();
        // Operator awareness: colour the label when filters are active, and show
        // a "searching…" state while a content: query resolves via Spotlight.
        let fq = FilterQuery::parse(query);
        let filtered = fq.has_local_filters() || fq.content.is_some();
        let content_pending = fq
            .content
            .as_deref()
            .is_some_and(|term| tab.content_for.as_deref() != Some(term));
        // The editable text: placeholder, a highlighted selection span, or the
        // text split around a static caret at the cursor.
        let field = if query.is_empty() {
            div()
                .min_w(px(80.0))
                .text_color(rgb(t.text_dim))
                .child("filter…  kind: ext: size: date: content:")
        } else if let Some((lo, hi)) = self.find_sel(pane) {
            let (bl, bh) = (char_byte(query, lo), char_byte(query, hi));
            div()
                .flex()
                .items_center()
                .min_w(px(80.0))
                .child(div().flex_none().child(query[..bl].to_string()))
                .child(
                    div()
                        .flex_none()
                        .bg(Theme::alpha(t.accent, 0x66))
                        .rounded_sm()
                        .child(query[bl..bh].to_string()),
                )
                .child(div().flex_none().child(query[bh..].to_string()))
        } else {
            let cursor = tab.find_cursor.min(query.chars().count());
            let b = char_byte(query, cursor);
            div()
                .flex()
                .items_center()
                .min_w(px(80.0))
                .child(div().flex_none().child(query[..b].to_string()))
                .child(div().flex_none().w(px(1.5)).h(px(14.0)).bg(rgb(t.text)))
                .child(div().flex_none().child(query[b..].to_string()))
        };
        div()
            .absolute()
            .bottom(px(16.0))
            .right(px(16.0))
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .py_2()
            .rounded_lg()
            .bg(Theme::alpha(t.surface, 0xf2))
            .border_1()
            .border_color(rgb(t.accent))
            .shadow_lg()
            .text_color(rgb(t.text))
            .child(
                div()
                    .flex_none()
                    .text_color(rgb(if filtered { t.accent } else { t.text_muted }))
                    .child("Filter"),
            )
            .child(field)
            .child(
                div()
                    .flex_none()
                    .text_color(rgb(t.text_muted))
                    .text_size(px(UI_SMALL_FONT_SIZE))
                    .child(if content_pending {
                        "searching…".to_string()
                    } else {
                        format!("{count}")
                    }),
            )
    }

    /// Bottom-right filter affordance for a pane. When a find is active it shows
    /// the live editable box; otherwise it shows a compact, clickable "Filter"
    /// pill so mouse users can start filtering without pressing "/".
    fn render_filter_box(&self, pane: usize, cx: &Context<Self>) -> AnyElement {
        let t = theme();
        match self.tab(pane).find_query.clone() {
            Some(q) => self.render_find_box(pane, &q).into_any_element(),
            // The always-on pill can be hidden in Settings; "/" still opens it.
            None if !prefs().show_filter_button => gpui::Empty.into_any_element(),
            None => div()
                .id(("filter-pill", pane))
                .absolute()
                .bottom(px(16.0))
                .right(px(16.0))
                .flex()
                .items_center()
                .gap_1()
                .px_3()
                .py_1()
                .rounded_lg()
                .cursor_pointer()
                .bg(Theme::alpha(t.surface, 0xe6))
                .border_1()
                .border_color(rgb(t.border_strong))
                .shadow_lg()
                .text_color(rgb(t.text_muted))
                .hover(|s| s.text_color(rgb(t.text)).border_color(rgb(t.accent)))
                .child(div().flex_none().text_size(px(UI_SMALL_FONT_SIZE)).child("🔍"))
                .child(div().flex_none().child("Filter"))
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    this.active_pane = pane;
                    this.open_find(pane, cx);
                    window.focus(&this.focus);
                }))
                .into_any_element(),
        }
    }

    /// Drain requests from rows that were actually laid out this frame.
    fn prewarm_icons(&self, cx: &mut Context<Self>) {
        let jobs=ICON_REQUESTS.with(|q|std::mem::take(&mut *q.borrow_mut()));
        if jobs.is_empty(){return;}
        cx.spawn(async move |this,cx| {
            for (key,(path,epoch)) in jobs {
                if ICON_EPOCH.with(|e|e.get())!=epoch {continue;}
                let built=if let Some(pack)=pack_icon_path(&key){
                    cx.background_spawn(async move {decode_image_file(&pack)}).await
                }else{
                    match icon_tiff(&path){Some(bytes)=>cx.background_spawn(async move {decode_icon(&bytes)}).await,None=>None}
                };
                if ICON_EPOCH.with(|e|e.get())==epoch {
                    ICON_CACHE.with(|c|c.borrow_mut().insert(key.clone(),built));
                    ICON_PENDING.with(|p|{p.borrow_mut().remove(&key);});
                }
                if this.update(cx,|_,cx|cx.notify()).is_err(){break;}
            }
        }).detach();
    }

    /// Whether `path` is the target of the currently-open right-click menu
    /// (so its row can keep the hovered look while the menu is up).
    fn is_ctx_target(&self, path: &Path) -> bool {
        self.context_menu
            .as_ref()
            .and_then(|m| m.target.as_ref())
            .is_some_and(|(p, _)| p.as_path() == path)
    }

    // ----- sidebar groups -----

    /// Open the "New Group" naming dialog.
    fn open_group_dialog(&mut self, cx: &mut Context<Self>) {
        self.sidebar_menu = None;
        self.group_dialog = Some(String::new());
        cx.notify();
    }

    /// Create a group with the given name (ignored if blank or a duplicate).
    fn create_group(&mut self, name: &str, cx: &mut Context<Self>) {
        let name = name.trim().to_string();
        if !name.is_empty() && !self.groups.iter().any(|g| g.name == name) {
            self.groups.push(Group { name, paths: Vec::new() });
            save_groups(&self.groups);
        }
        self.group_dialog = None;
        cx.notify();
    }

    /// Delete a group entirely.
    fn delete_group(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx < self.groups.len() {
            self.groups.remove(idx);
            save_groups(&self.groups);
            cx.notify();
        }
    }

    /// Add a path (file or folder) to a group.
    fn add_to_group(&mut self, idx: usize, path: PathBuf, cx: &mut Context<Self>) {
        if let Some(g) = self.groups.get_mut(idx) {
            if !g.paths.contains(&path) {
                g.paths.push(path);
                save_groups(&self.groups);
                cx.notify();
            }
        }
    }

    /// Remove a path from a group.
    fn remove_from_group(&mut self, idx: usize, path: &Path, cx: &mut Context<Self>) {
        if let Some(g) = self.groups.get_mut(idx) {
            let before = g.paths.len();
            g.paths.retain(|p| p != path);
            if g.paths.len() != before {
                save_groups(&self.groups);
                cx.notify();
            }
        }
    }

    fn open_sidebar_menu(&mut self, x: f32, y: f32, target: SidebarTarget, cx: &mut Context<Self>) {
        self.sidebar_menu = Some((x, y, target));
        cx.notify();
    }

    fn close_sidebar_menu(&mut self, cx: &mut Context<Self>) {
        if self.sidebar_menu.take().is_some() {
            cx.notify();
        }
    }

    fn handle_group_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let ks = &ev.keystroke;
        let cmd = ks.modifiers.platform;
        match ks.key.as_str() {
            "escape" => {
                self.group_dialog = None;
                cx.notify();
            }
            "enter" => {
                if let Some(name) = self.group_dialog.clone() {
                    self.create_group(&name, cx);
                }
            }
            "backspace" => {
                if let Some(s) = self.group_dialog.as_mut() {
                    s.pop();
                }
                cx.notify();
            }
            "v" if cmd => {
                if let Some(t) = cx.read_from_clipboard().and_then(|i| i.text()) {
                    if let Some(s) = self.group_dialog.as_mut() {
                        s.push_str(t.trim());
                    }
                    cx.notify();
                }
            }
            _ => {
                if cmd {
                    return;
                }
                if let Some(ch) = ks.key_char.as_ref() {
                    if !ch.is_empty() && !ch.chars().any(char::is_control) {
                        if let Some(s) = self.group_dialog.as_mut() {
                            s.push_str(ch);
                        }
                        cx.notify();
                    }
                }
            }
        }
    }

    /// Toggle whether a sidebar section (by title) is collapsed, and persist it.
    fn toggle_section(&mut self, title: String, cx: &mut Context<Self>) {
        if !self.collapsed_sections.remove(&title) {
            self.collapsed_sections.insert(title);
        }
        let list: Vec<String> = self.collapsed_sections.iter().cloned().collect();
        write_string_list("collapsed_sections.txt", &list);
        cx.notify();
    }

    /// A collapsible section header: a ▾/▸ arrow + title that toggles the
    /// section, plus an optional trailing element (e.g. the Locations "+").
    fn section_header_el(
        &self,
        title: &'static str,
        trailing: Option<AnyElement>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let t = theme();
        let is_col = self.collapsed_sections.contains(title);
        let arrow = if is_col { "▸" } else { "▾" };
        let title_owned = title.to_string();
        let mut row = div()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .pt_4()
            .pb_1()
            .child(
                div()
                    .id(SharedString::from(format!("sec-{title}")))
                    .flex()
                    .items_center()
                    .gap_1()
                    .cursor_pointer()
                    .text_size(px(UI_SMALL_FONT_SIZE))
                    .text_color(rgb(t.text_dim))
                    .hover(|s| s.text_color(rgb(t.text)))
                    .child(div().w(px(10.0)).child(arrow.to_string()))
                    .child(title.to_string())
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.toggle_section(title_owned.clone(), cx);
                    })),
            );
        if let Some(tr) = trailing {
            row = row.child(tr);
        }
        row.into_any_element()
    }

    /// Ensure a folder's subfolders are loaded into the waterfall cache. Reads
    /// the directory off the main thread; when it lands, the tree re-renders.
    fn ensure_waterfall_children(&mut self, dir: PathBuf, cx: &mut Context<Self>) {
        if self.waterfall_children.contains_key(&dir) || self.waterfall_pending.contains(&dir) {
            return;
        }
        self.waterfall_pending.insert(dir.clone());
        cx.spawn(async move |this, cx| {
            let d = dir.clone();
            let (subs, mtime) = cx.background_spawn(async move { read_subdirs(&d) }).await;
            let _ = this.update(cx, |this, cx| {
                if let Some(m) = mtime {
                    this.waterfall_mtime.insert(dir.clone(), m);
                }
                this.waterfall_children.insert(dir.clone(), subs);
                this.waterfall_pending.remove(&dir);
                cx.notify();
            });
        })
        .detach();
    }

    /// Make sure every folder the tree will render has its children loading —
    /// the root, each expanded folder's children, and one level beyond the
    /// visible frontier so we know which rows get a disclosure triangle.
    /// Called each render, before the tree is built (which only has `&self`).
    fn ensure_waterfall_loaded(&mut self, cx: &mut Context<Self>) {
        if !prefs().waterfall || prefs().sidebar_collapsed {
            return;
        }
        // The waterfall is local-only; remote tabs have no cheap subdir listing.
        if self.active_tab().remote.is_some() {
            return;
        }
        let root = self.active_tab().current_dir.clone();
        self.waterfall_prefetch(root, cx);
    }

    /// Load `dir`'s children, and — so every visible row knows whether to show a
    /// triangle — each child's children too. Recurse only into expanded folders.
    fn waterfall_prefetch(&mut self, dir: PathBuf, cx: &mut Context<Self>) {
        self.ensure_waterfall_children(dir.clone(), cx);
        let Some(children) = self.waterfall_children.get(&dir).cloned() else {
            return; // still loading; we'll prefetch its children next frame
        };
        for child in children {
            if self.waterfall_expanded.contains(&child) {
                self.waterfall_prefetch(child, cx);
            } else {
                // One level ahead: enough to know if it has subfolders.
                self.ensure_waterfall_children(child, cx);
            }
        }
    }

    /// Expand or collapse a folder in the waterfall tree.
    fn toggle_waterfall(&mut self, dir: PathBuf, cx: &mut Context<Self>) {
        if self.waterfall_expanded.contains(&dir) {
            self.waterfall_expanded.remove(&dir);
        } else {
            self.waterfall_expanded.insert(dir.clone());
            self.ensure_waterfall_children(dir, cx);
        }
        cx.notify();
    }

    /// Watcher hook: re-stat every cached waterfall folder and invalidate any
    /// whose mtime changed, so the tree live-refreshes when files appear or
    /// disappear. Runs off the main thread; returns true if anything changed.
    fn refresh_waterfall(&mut self, cx: &mut Context<Self>) {
        if !prefs().waterfall || self.waterfall_mtime.is_empty() {
            return;
        }
        let paths: Vec<PathBuf> = self.waterfall_mtime.keys().cloned().collect();
        cx.spawn(async move |this, cx| {
            let sampled = paths.clone();
            let stats = cx
                .background_spawn(async move {
                    sampled
                        .iter()
                        .map(|d| fs::metadata(d).ok().and_then(|m| m.modified().ok()))
                        .collect::<Vec<Option<SystemTime>>>()
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                let mut dirty = false;
                for (dir, cur) in paths.into_iter().zip(stats) {
                    match (this.waterfall_mtime.get(&dir).copied(), cur) {
                        // Folder changed on disk: drop the cache so the next
                        // render reloads it via `ensure_waterfall_children`.
                        (Some(prev), Some(now)) if prev != now => {
                            this.waterfall_children.remove(&dir);
                            this.waterfall_mtime.remove(&dir);
                            dirty = true;
                        }
                        // Folder vanished: forget it entirely.
                        (Some(_), None) => {
                            this.waterfall_children.remove(&dir);
                            this.waterfall_mtime.remove(&dir);
                            this.waterfall_expanded.remove(&dir);
                            dirty = true;
                        }
                        _ => {}
                    }
                }
                if dirty {
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Append the waterfall folder tree (rooted at the active dir) to `items`.
    fn render_waterfall(
        &self,
        items: &mut Vec<AnyElement>,
        key: &mut usize,
        current: &Path,
        cx: &Context<Self>,
    ) {
        let root = self.active_tab().current_dir.clone();
        items.push(self.section_header_el("WATERFALL", None, cx));
        if self.collapsed_sections.contains("WATERFALL") {
            return;
        }
        let mut rows: Vec<AnyElement> = Vec::new();
        self.waterfall_rows(&root, 0, key, current, &mut rows, cx);
        if rows.is_empty() {
            items.push(empty_hint("No subfolders").into_any_element());
        } else {
            items.append(&mut rows);
        }
    }

    /// Depth-first: render each subfolder of `dir`, recursing into expanded ones
    /// so children appear indented directly beneath their parent.
    fn waterfall_rows(
        &self,
        dir: &Path,
        depth: usize,
        key: &mut usize,
        current: &Path,
        out: &mut Vec<AnyElement>,
        cx: &Context<Self>,
    ) {
        let Some(children) = self.waterfall_children.get(dir) else {
            return;
        };
        let t = theme();
        for child in children {
            *key += 1;
            let expanded = self.waterfall_expanded.contains(child);
            let active = child.as_path() == current;
            let label = path_label(child);
            let indent = 8.0 + depth as f32 * 12.0;
            // Only folders with subfolders get a triangle. `None` means the
            // child's listing hasn't loaded yet (prefetch fills it in shortly),
            // so we show a spacer to avoid a triangle that flips to nothing.
            let has_subdirs = self
                .waterfall_children
                .get(child)
                .map(|v| !v.is_empty())
                .unwrap_or(false);

            let toggle_target = child.clone();
            let nav_target = child.clone();
            let mut row = div()
                .id(("wf", *key))
                .flex()
                .items_center()
                .gap_1()
                .mx_2()
                .py_1()
                .pr_2()
                .rounded_md()
                .cursor_pointer()
                .text_color(rgb(if active { t.text } else { t.text_muted }));
            row = if active {
                row.bg(rgb(t.surface))
            } else {
                row.hover(|s| s.bg(rgb(t.hover))).active(|s| s.bg(rgb(t.selected)))
            };
            // Disclosure triangle (or a blank spacer of the same width so names
            // stay aligned whether or not a folder can expand).
            let tri = if has_subdirs {
                let arrow = if expanded { "▾" } else { "▸" };
                div()
                    .id(("wf-tri", *key))
                    .w(px(12.0))
                    .flex()
                    .justify_center()
                    .text_color(rgb(t.text_dim))
                    .hover(|s| s.text_color(rgb(t.text)))
                    .child(arrow)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.toggle_waterfall(toggle_target.clone(), cx);
                        cx.stop_propagation();
                    }))
                    .into_any_element()
            } else {
                div().w(px(12.0)).into_any_element()
            };
            let row = row
                .pl(px(indent))
                .child(tri)
                .child(icon_element(child, true))
                .child(div().min_w_0().overflow_hidden().child(label))
                // Clicking the row (name/icon) opens the folder in the pane.
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.unsync_ssh_browser(this.active_pane, cx);
                    this.navigate_to(nav_target.clone(), cx);
                }));
            out.push(row.into_any_element());

            if expanded && has_subdirs {
                self.waterfall_rows(child, depth + 1, key, current, out, cx);
            }
        }
    }

    /// Push a section header and report whether its items are expanded.
    fn begin_section(
        &self,
        items: &mut Vec<AnyElement>,
        title: &'static str,
        cx: &Context<Self>,
    ) -> bool {
        items.push(self.section_header_el(title, None, cx));
        !self.collapsed_sections.contains(title)
    }

    fn render_sidebar(&self, cx: &Context<Self>) -> impl IntoElement {
        let collapsed = prefs().sidebar_collapsed;
        let mut items: Vec<AnyElement> = Vec::new();

        // --- Collapse / expand toggle (always present) ---
        let chevron = if collapsed { "»" } else { "«" };
        items.push(
            div()
                .id("sidebar-toggle")
                .flex()
                .items_center()
                .when(collapsed, |d| d.justify_center())
                .when(!collapsed, |d| d.justify_end())
                .px_2()
                .pt_2()
                .pb_1()
                .cursor_pointer()
                .text_color(rgb(theme().text_dim))
                .hover(|s| s.text_color(rgb(theme().text)))
                .child(chevron)
                .tooltip(tip(if collapsed { "Expand sidebar" } else { "Collapse sidebar" }))
                .on_click(cx.listener(|_, _: &ClickEvent, _, cx| {
                    let mut np = prefs();
                    np.sidebar_collapsed = !np.sidebar_collapsed;
                    apply_prefs(np, cx);
                    cx.notify();
                }))
                .into_any_element(),
        );

        let sidebar = div()
            .id("sidebar")
            .flex_none()
            .w(px(if collapsed { SIDEBAR_COLLAPSED_W } else { SIDEBAR_W }))
            .h_full()
            .flex()
            .flex_col()
            .pb_3()
            .bg(rgb(theme().sidebar))
            .border_r_1()
            .border_color(rgb(theme().border));
        // Section content is constructed only for the expanded sidebar.
        if collapsed {
            return sidebar.children(items);
        }

        let current = self.active_tab().current_dir.clone();
        let current = current.as_path();
        let mut key = 0usize;
        self.push_volumes(&mut items, cx);
        let fixed = items;
        let mut items = Vec::new();
        self.push_project_sidebar(&mut items, cx);

        self.project_folder_section(&mut items, &mut key, "LOCATIONS", projects::Section::Locations, cx);
        self.project_tagged_section(&mut items, &mut key, cx);

        // --- Groups (user-defined; only when the feature is enabled) ---
        if prefs().groups_enabled {
            for (gidx, g) in self.groups.iter().enumerate() {
                let ckey = format!("group:{}", g.name);
                let is_col = self.collapsed_sections.contains(&ckey);
                let arrow = if is_col { "▸" } else { "▾" };
                let toggle_key = ckey.clone();
                items.push(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .px_3()
                        .pt_4()
                        .pb_1()
                        .child(
                            div()
                                .id(SharedString::from(format!("grp-{gidx}")))
                                .flex()
                                .items_center()
                                .gap_1()
                                .cursor_pointer()
                                .text_size(px(UI_SMALL_FONT_SIZE))
                                .text_color(rgb(theme().text_dim))
                                .hover(|s| s.text_color(rgb(theme().text)))
                                .child(div().w(px(10.0)).child(arrow.to_string()))
                                .child(g.name.to_uppercase())
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.toggle_section(toggle_key.clone(), cx);
                                }))
                                .on_mouse_down(
                                    MouseButton::Right,
                                    cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                                        let (x, y) = (
                                            f64::from(ev.position.x) as f32,
                                            f64::from(ev.position.y) as f32,
                                        );
                                        this.open_sidebar_menu(
                                            x,
                                            y,
                                            SidebarTarget::GroupHeader(gidx),
                                            cx,
                                        );
                                        cx.stop_propagation();
                                    }),
                                ),
                        )
                        .into_any_element(),
                );
                if !is_col {
                    if g.paths.is_empty() {
                        items.push(empty_hint("Right-click a file to add it").into_any_element());
                    } else {
                        for p in &g.paths {
                            push_group_member(&mut items, cx, &mut key, gidx, p.clone(), current);
                        }
                    }
                }
            }
        }

        self.project_folder_section(&mut items, &mut key, "CLOUD", projects::Section::Cloud, cx);

        // Saved connections and explicit network-folder shortcuts belong to this project.
        if self.begin_section(&mut items, "SERVERS", cx) {
            for path in &self.projects.sidebar().server_folders {
                self.project_path_row(&mut items, &mut key, path, projects::Section::ServerFolders, cx);
            }
            // Saved SFTP servers — click to connect and browse remotely.
            for server in sftp_servers() {
                key += 1;
                let s = server.clone();
                let label = server.name.clone();
                let tip_text = format!("{}\nsftp://{}", server.name, server.display());
                let row = div()
                    .id(("sftp", key))
                    .flex()
                    .items_center()
                    .rounded_md()
                    .cursor_pointer()
                    .text_color(rgb(theme().text_muted))
                    .hover(|s| s.bg(rgb(theme().hover)).text_color(rgb(theme().text)))
                    .mx_2()
                    .px_2()
                    .py_1()
                    .gap_2()
                    .child(div().w(px(16.0)).flex().justify_center().child("🖧"))
                    .child(div().min_w_0().truncate().child(label));
                let s2 = server.clone();
                items.push(
                    row.tooltip(tip(tip_text))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.connect_sftp(s.clone(), cx);
                        }))
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                                let (x, y) = (
                                    f64::from(ev.position.x) as f32,
                                    f64::from(ev.position.y) as f32,
                                );
                                this.open_sidebar_menu(x, y, SidebarTarget::Sftp(s2.clone()), cx);
                                cx.stop_propagation();
                            }),
                        )
                        .into_any_element(),
                );
            }
            items.push(div().id("add-server-folder").mx_2().px_2().py_1().cursor_pointer().text_color(rgb(theme().text_muted))
                .child("+ Add network folder…").on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.choose_project_folder(projects::Section::ServerFolders, cx))).into_any_element());
            // "Connect to Server…" action row.
            let base = div()
                .id("connect-server")
                .flex()
                .items_center()
                .rounded_md()
                .cursor_pointer()
                .text_color(rgb(theme().text_muted))
                .hover(|s| s.bg(rgb(theme().hover)).text_color(rgb(theme().text)))
                .mx_2()
                .px_2()
                .py_1()
                .gap_2()
                .child(div().w(px(16.0)).flex().justify_center().child("🌐"))
                .child("Connect to Server…");
            items.push(
                base.tooltip(tip("Connect to Server…"))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.open_server_dialog(cx);
                    }))
                    .into_any_element(),
            );
        }

        // --- Waterfall: an inline, expandable folder tree of the active dir ---
        if prefs().waterfall && self.active_tab().remote.is_none() {
            self.render_waterfall(&mut items, &mut key, current, cx);
        }

        let groups_on = prefs().groups_enabled;
        let sections = div().id("sidebar-sections").flex_1().min_h_0().flex().flex_col()
            .overflow_y_scroll()
            // Right-click empty sidebar space → "New Group" (when enabled).
            .when(groups_on, |d| {
                d.on_mouse_down(
                    MouseButton::Right,
                    cx.listener(|this, ev: &MouseDownEvent, _, cx| {
                        let (x, y) = (
                            f64::from(ev.position.x) as f32,
                            f64::from(ev.position.y) as f32,
                        );
                        this.open_sidebar_menu(x, y, SidebarTarget::Empty, cx);
                    }),
                )
            })
            .children(items);
        sidebar.children(fixed).child(sections)
    }

    /// The top bar for a pane: back/forward arrows then either the clickable
    /// breadcrumb or, in edit mode, an editable text field.
    fn render_path_bar(&self, pane: usize, cx: &Context<Self>) -> impl IntoElement {
        let tab = self.tab(pane);
        let can_back = tab.hist_pos > 0;
        let can_fwd = tab.hist_pos + 1 < tab.history.len();

        let content: AnyElement = if tab.editing_path.is_some() {
            self.render_path_editor(pane).into_any_element()
        } else {
            self.render_breadcrumb(pane, cx).into_any_element()
        };

        div()
            .flex_none()
            .flex()
            .items_center()
            .gap_1()
            .px_3()
            .py_2()
            .min_w_0()
            .overflow_hidden()
            .border_b_1()
            .border_color(rgb(theme().border))
            // Right-click the nav bar → New Tab / Copy Path / Reveal in Finder.
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                    let (x, y) = (
                        f64::from(ev.position.x) as f32,
                        f64::from(ev.position.y) as f32,
                    );
                    this.open_sidebar_menu(x, y, SidebarTarget::NavBar(pane), cx);
                    cx.stop_propagation();
                }),
            )
            .child(nav_arrow(
                ("nav-back", pane),
                Icon::Left,
                can_back,
                cx.listener(move |this, _, _, cx| this.go_back(pane, cx)),
            ))
            .child(nav_arrow(
                ("nav-fwd", pane),
                Icon::Right,
                can_fwd,
                cx.listener(move |this, _, _, cx| this.go_forward(pane, cx)),
            ))
            .child(content)
            .child(self.render_view_toolbar(pane, cx))
    }

    /// Compact vector controls for the two supported views and folder options.
    fn render_view_toolbar(&self, pane: usize, cx: &Context<Self>) -> impl IntoElement {
        use toolbar_icons::toolbar_button as button;
        let view = self.tab(pane).view;
        let local = self.tab(pane).remote.is_none();
        let has_selection = !self.tab(pane).selection.is_empty();
        let tiny = pane_width(pane) > 0. && pane_width(pane) < 470.;
        let mut row = div().flex_none().flex().items_center().gap_1().pl_2();
        if local && !tiny {
            row = row.child(button("tb-new-folder", pane, Icon::FolderPlus, false, true, "New folder · ⇧⌘N")
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| { this.active_pane = pane; this.new_folder(pane, window, cx); })));
            row = row.child(button("tb-share", pane, Icon::Share, false, has_selection, "Share selection · ⇧⌘S")
                .when(has_selection, |d| d.on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, window, cx| {
                    this.active_pane = pane;
                    if let Some(view) = ns_view_ptr(window) { show_share_picker(view, (f32::from(ev.position.x), f32::from(ev.position.y)), &this.action_targets(pane)); }
                    cx.stop_propagation();
                }))));
        }
        if local {
            let tagged = has_selection && self.tab(pane).selection.iter().all(|p| self.projects.sidebar().tagged.contains(p));
            row = row.child(button("tb-project-tag", pane, Icon::Tag, tagged, has_selection, "Tag selection in current project")
                .when(has_selection, |d| d.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_project_tags(pane, cx)))));
            row = row.child(button("tb-delete", pane, Icon::Trash, false, has_selection, "Move selection to Trash · ⌘⌫")
                .when(has_selection, |d| d.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| { this.active_pane = pane; this.request_delete(pane, cx); }))));
        }
        row = row.child(self.render_hidden_button(pane, cx));
        if !tiny {
            row = row.child(button("palette-search", pane, Icon::Search, false, true, "Search files · ⌘P")
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| { this.active_pane = pane; this.focus_file_search(false, window, cx); })));
        }
        row.child(div().flex_none().w(px(1.)).h(px(18.)).mx_1().bg(rgb(theme().border)))
            .child(div().flex().items_center().gap(px(1.)).p(px(2.)).rounded_lg().border_1().border_color(rgb(theme().border)).bg(rgb(theme().sidebar))
                .children([(ViewMode::List, Icon::List, "view-list", "List view"), (ViewMode::Icons, Icon::Grid, "view-icons", "Icon view")]
                    .into_iter().map(|(mode, icon, id, label)| button(id, pane, icon, view == mode, true, label)
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_view(pane, mode, cx))))))
            .child(button("view-options", pane, Icon::Columns, false, true, "View options · columns and saved folder views")
                .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                    this.open_view_menu(pane, f32::from(ev.position.x), f32::from(ev.position.y), cx); cx.stop_propagation();
                })))
            .child(button("sort-by", pane, Icon::Sort, false, true, "Sort by")
                .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                    this.sort_menu = Some((pane, f32::from(ev.position.x), f32::from(ev.position.y))); cx.stop_propagation(); cx.notify();
                })))
    }

    /// Clickable path ending at the current folder. The inner row keeps its
    /// natural width and is anchored on the right when it overflows, so clipping
    /// hides ancestors on the left instead of the folder being browsed.
    fn render_breadcrumb(&self, pane: usize, cx: &Context<Self>) -> impl IntoElement {
        use std::path::Component;

        let tab = self.tab(pane);
        let current_dir = tab.current_dir.clone();
        let (badge, segs) = if let Some((badge, crumbs)) = self.cloud_breadcrumbs(pane, cx) {
            (Some(badge), crumbs)
        } else {
            // A remote tab's root is the server, not the local disk.
            let root_label = match &tab.remote {
                Some(s) => s.name.clone(),
                None => "Macintosh HD".to_string(),
            };
            let mut segs: Vec<AnyElement> = Vec::new();
            let mut acc = PathBuf::new();
            let mut idx = 0usize;
            for comp in current_dir.components() {
                let (label, full) = match comp {
                    Component::RootDir => {
                        acc.push("/");
                        (root_label.clone(), acc.clone())
                    }
                    Component::Normal(s) => {
                        acc.push(s);
                        (s.to_string_lossy().into_owned(), acc.clone())
                    }
                    _ => continue,
                };
                if idx > 0 {
                    segs.push(breadcrumb_sep());
                }
                // Palette-on-click only for the crumb of the CURRENT dir, and only
                // on local tabs (Reveal in Finder etc. make no sense for a remote
                // path — there the crumb keeps its refresh/navigate behavior).
                let is_current = full == current_dir && tab.remote.is_none();
                segs.push(breadcrumb_seg(
                    pane * 4096 + idx,
                    pane,
                    label,
                    full,
                    true,
                    is_current,
                    cx,
                ));
                idx += 1;
            }
            (None, segs)
        };

        div()
            .id(("breadcrumb", pane))
            .flex()
            .items_center()
            .justify_end()
            .flex_1()
            .min_w_0()
            // Clip so long paths can't paint over the view/sort toolbar (or
            // bleed into the neighbouring pane) when a pane is narrow.
            .overflow_hidden()
            .h(px(22.0))
            .tooltip(tip(current_dir.to_string_lossy().into_owned()))
            .children(badge)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .justify_end()
                    .overflow_hidden()
                    .child(
                        div()
                            .flex_none()
                            .min_w_full()
                            .flex()
                            .items_center()
                            .whitespace_nowrap()
                            .h_full()
                            .children(segs)
                            // With a short path, spare room remains clickable to edit.
                            .child(
                                div()
                                    .id(("path-edit-zone", pane))
                                    .flex_1()
                                    .h_full()
                                    .min_w_0()
                                    .cursor_text()
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.begin_path_edit(pane, window, cx)
                                    })),
                            ),
                    ),
            )
    }

    /// The editable address-bar field shown in edit mode.
    fn render_path_editor(&self, pane: usize) -> impl IntoElement {
        let t = theme();
        let tab = self.tab(pane);
        let text = tab.editing_path.clone().unwrap_or_default();

        // The editable text: either a highlighted selection span, or the text
        // split around a static caret at the cursor.
        let field = if let Some((lo, hi)) = self.path_sel(pane) {
            let (bl, bh) = (char_byte(&text, lo), char_byte(&text, hi));
            div()
                .flex()
                .items_center()
                .min_w_0()
                .child(div().flex_none().child(text[..bl].to_string()))
                .child(
                    div()
                        .flex_none()
                        .bg(Theme::alpha(t.accent, 0x66))
                        .rounded_sm()
                        .child(text[bl..bh].to_string()),
                )
                .child(div().flex_none().child(text[bh..].to_string()))
        } else {
            let cursor = tab.path_cursor.min(text.chars().count());
            let b = char_byte(&text, cursor);
            div()
                .flex()
                .items_center()
                .min_w_0()
                .child(div().flex_none().child(text[..b].to_string()))
                // Blinking would need a timer; a static caret reads clearly.
                .child(div().flex_none().w(px(1.5)).h(px(14.0)).bg(rgb(t.text)))
                .child(div().flex_none().child(text[b..].to_string()))
        };

        div()
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .px_2()
            .h(px(22.0))
            .rounded_md()
            .bg(rgb(t.surface))
            .border_1()
            .border_color(rgb(t.accent))
            .text_color(rgb(t.text))
            .child(field)
    }

    /// The canvas: one full-width pane, or two panes split by a draggable
    /// divider, plus a right-edge drop zone (shown while dragging a tab).
    fn render_content(&self, cx: &Context<Self>) -> impl IntoElement {
        let mut row = div().flex_1().flex().min_w_0().h_full().relative();

        if self.panes.len() == 1 {
            // After a split collapses, the survivor eases from its old width to
            // full; anchored right when the *left* pane was the one closed, so
            // it grows toward where its sibling was.
            if let Some((start_w, anchor_right)) = self.collapse_anim {
                if anchor_right {
                    row = row.justify_end();
                }
                row = row.child(
                    div()
                        .flex_none()
                        .min_w_0()
                        .h_full()
                        .child(self.render_pane(0, cx))
                        .with_animation(
                            ("split-close", self.split_epoch),
                            Animation::new(Duration::from_millis(SPLIT_ANIM_MS))
                                .with_easing(ease_out_quint()),
                            move |el, t| el.w(relative(start_w + (1.0 - start_w) * t)),
                        ),
                );
            } else {
                row = row.child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .child(self.render_pane(0, cx)),
                );
            }
        } else {
            // On split creation the left pane eases from full width down to the
            // ratio, so the right pane slides in from the edge. Once finished
            // the animation pins at t=1, i.e. exactly `split_ratio`, which the
            // divider drag then updates live.
            let ratio = self.split_ratio;
            row = row
                .child(
                    div()
                        .flex_none()
                        .min_w_0()
                        .h_full()
                        .child(self.render_pane(0, cx))
                        .with_animation(
                            ("split-open", self.split_epoch),
                            Animation::new(Duration::from_millis(SPLIT_ANIM_MS))
                                .with_easing(ease_out_quint()),
                            move |el, t| el.w(relative(1.0 + (ratio - 1.0) * t)),
                        ),
                )
                .child(self.render_divider(cx))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .child(self.render_pane(1, cx)),
                );
        }

        // Right-edge split target — only present while a *tab* is being
        // dragged, so it never blocks normal interaction and its border never
        // shows up as a stray line during file drags.
        if cx.has_active_drag() && TAB_DRAG_LIVE.load(Ordering::Relaxed) {
            let new_pane = self.panes.len();
            row = row.child(
                div()
                    .id("split-zone")
                    .absolute()
                    .top_0()
                    .right_0()
                    .bottom_0()
                    .w(relative(0.32))
                    .border_l_2()
                    .border_color(rgb(theme().border))
                    .drag_over::<TabDrag>(|s, _, _, _| {
                        s.bg(Theme::alpha(theme().accent, 0x33))
                            .border_color(rgb(theme().accent))
                    })
                    .on_drop(cx.listener(move |this, drag: &TabDrag, _, cx| {
                        this.move_tab(*drag, new_pane, 0, cx);
                    })),
            );
        }
        row
    }

    /// The draggable divider between two panes.
    fn render_divider(&self, cx: &Context<Self>) -> impl IntoElement {
        // Accent + slightly thicker while grabbed, so the drag feels "held".
        let dragging = self.divider_drag.is_some();
        div()
            .id("pane-divider")
            .flex_none()
            .w(px(6.0))
            .h_full()
            .flex()
            .justify_center()
            .cursor(CursorStyle::ResizeLeftRight)
            .child(
                div()
                    .w(px(if dragging { 2.0 } else { 1.0 }))
                    .h_full()
                    .bg(if dragging {
                        rgb(theme().accent)
                    } else {
                        rgb(theme().border_strong)
                    }),
            )
            .when(dragging, |s| s.bg(Theme::alpha(theme().accent, 0x22)))
            .hover(|s| s.bg(rgb(theme().hover)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, ev: &MouseDownEvent, _, cx| {
                    // Double-click puts the divider back in the middle.
                    if ev.click_count >= 2 {
                        this.divider_drag = None;
                        this.split_ratio = 0.5;
                        this.save_session();
                        cx.notify();
                        return;
                    }
                    // Remember where the grab started and the ratio at that
                    // moment, so the drag continues from the current size rather
                    // than snapping the divider to the cursor.
                    this.divider_drag = Some((f64::from(ev.position.x) as f32, this.split_ratio));
                    cx.notify();
                }),
            )
    }

    /// Update the split ratio while dragging the divider. `x` is window-relative.
    /// Delta-based: new ratio = ratio-at-grab + (cursor moved) / content width.
    fn update_divider(&mut self, x: f32, cx: &mut Context<Self>) {
        let Some((start_x, start_ratio)) = self.divider_drag else {
            return;
        };
        if self.panes.len() < 2 {
            return;
        }
        let content_w = (pane_width(0) + pane_width(1)).max(1.0);
        let delta = (x - start_x) / content_w;
        self.split_ratio = (start_ratio + delta).clamp(0.2, 0.8);
        cx.notify();
    }

    /// Bordered tabs sit against the path bar, with a "+" button alongside.
    fn render_tab_strip(&self, pane: usize, cx: &Context<Self>) -> impl IntoElement {
        let t = theme();
        let p = self.pane(pane);
        let mut chips: Vec<AnyElement> = Vec::new();
        for (i, tab) in p.tabs.iter().enumerate() {
            let active = i == p.active && pane == self.active_pane;
            // A remote tab at its root shows the server name, not "Macintosh HD".
            let label = match &tab.remote {
                Some(s) if tab.current_dir == Path::new("/") => s.name.clone(),
                _ => path_label(&tab.current_dir),
            };
            let drag_label = label.clone();
            let drag = TabDrag { pane, tab: i };
            chips.push(
                div()
                    .id(("tab", pane * 4096 + i))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_2()
                    .h(px(TAB_H - 4.0))
                    .max_w(px(180.0))
                    .rounded_t_md()
                    .border_1()
                    .border_color(rgb(if active { t.border_strong } else { t.border }))
                    .when(active, |s| s.border_b_0())
                    .cursor_pointer()
                    .text_color(if active { rgb(t.text) } else { rgb(t.text_muted) })
                    .bg(if active { rgb(t.bg) } else { rgb(t.sidebar) })
                    .when(!active, |s| s.hover(|s| s.bg(rgb(t.hover))))
                    .active(|s| s.bg(rgb(t.selected)))
                    // Drag this tab (live floating preview).
                    .on_drag(drag, move |_, _, _, cx| {
                        TAB_DRAG_LIVE.store(true, Ordering::Relaxed);
                        cx.new(|_| TabDragPreview {
                            label: drag_label.clone(),
                        })
                    })
                    // Drop another tab here → insert at this position.
                    .drag_over::<TabDrag>(|s, _, _, _| s.bg(Theme::alpha(theme().accent, 0x33)))
                    .on_drop(cx.listener(move |this, from: &TabDrag, _, cx| {
                        this.move_tab(*from, pane, i, cx);
                    }))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.select_tab(pane, i, cx);
                    }))
                    // Right-click → tab context menu (New/Duplicate/Close/…).
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                            let (x, y) = (
                                f64::from(ev.position.x) as f32,
                                f64::from(ev.position.y) as f32,
                            );
                            this.open_sidebar_menu(x, y, SidebarTarget::Tab(pane, i), cx);
                            cx.stop_propagation();
                        }),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .child(label.clone()),
                    )
                    .child(
                        div()
                            .id(("tab-close", pane * 4096 + i))
                            .flex_none()
                            .px_1()
                            .rounded_sm()
                            .text_color(rgb(t.text_dim))
                            .hover(|s| s.text_color(rgb(t.text)).bg(rgb(t.selected)))
                            .child(ui_icon(Icon::Close, 11., rgb(t.text_muted)))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.close_tab(pane, i, cx);
                                cx.stop_propagation();
                            })),
                    )
                    .into_any_element(),
            );
        }

        div()
            .flex_none()
            .flex()
            .items_end()
            .gap_1()
            .px_1()
            .h(px(TAB_H))
            .bg(rgb(t.sidebar))
            .border_b_1()
            .border_color(rgb(t.border))
            // Dropping on empty strip space appends to this pane.
            .drag_over::<TabDrag>(|s, _, _, _| s.bg(rgb(theme().hover)))
            .on_drop(cx.listener(move |this, from: &TabDrag, _, cx| {
                let len = this.pane(pane).tabs.len();
                this.move_tab(*from, pane, len, cx);
            }))
            .children(chips)
            .child(
                div()
                    .id(("tab-add", pane))
                    .flex_none()
                    .w(px(22.0))
                    .h(px(22.0))
                    .mb(px(4.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_md()
                    .cursor_pointer()
                    .text_color(rgb(t.text_muted))
                    .hover(|s| s.bg(rgb(t.hover)).text_color(rgb(t.text)))
                    .active(|s| s.bg(rgb(t.selected)))
                    .child(ui_icon(Icon::Plus, 14., rgb(t.text_muted)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.new_tab_in(pane, cx);
                    })),
            )
    }

    /// Render one pane: tab strip → path bar → column header → virtualized
    /// listing + scrollbar, plus the right-edge split drop zone.
    fn render_pane(&self, pane: usize, cx: &Context<Self>) -> impl IntoElement {
        let view = self.tab(pane).view;
        let weak = cx.entity().downgrade();
        // Highlight the active pane's border (only meaningful when split).
        let split = self.panes.len() > 1;
        let active = pane == self.active_pane;
        let searching = self.search_in_pane(pane);
        let body: AnyElement = if searching { self.render_file_results(cx) } else { match view {
            ViewMode::List => self.render_list_body(pane, cx),
            ViewMode::Icons => self.render_icons_body(pane, cx),
        }};

        let browser_owner = cx.entity().downgrade();
        let browser = div()
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h_0()
            // The file viewport can be narrower than its full-width headers.
            .child(
                gpui::canvas(
                    move |bounds, window, _| {
                        let width = f32::from(bounds.size.width);
                        let changed = BROWSER_W.with(|w| {
                            let mut widths = w.borrow_mut();
                            let old = widths[pane.min(1)];
                            widths[pane.min(1)] = width;
                            (old - width).abs() > 0.5
                        });
                        if changed {
                            window.on_next_frame(move |_, cx| {
                                browser_owner.update(cx, |_, cx| cx.notify()).ok();
                            });
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .child(body)
                    .children(self.rename_error_pill(pane))
                    .when(!searching, |body| body.child(self.render_filter_box(pane, cx)))
                    .children(self.render_copy_feedback(pane)),
            )
            .children(self.render_terminal_panel(pane, cx));
        let mut body_row = div().flex().flex_1().min_h_0().min_w_0().child(browser);
        // Keep the shared selection preview on the far right, below every
        // navigation/filter row. Inspect retains its separate full-height map.
        if pane + 1 == self.panes.len() {
            if let Some(map) = self.render_space_map(cx) {
                body_row = body_row.child(map);
            } else if let Some(inspector) = self.render_inspector(cx) {
                body_row = body_row.child(self.render_preview_panel(inspector, pane, cx));
            }
        }

        div()
            .relative()
            .flex()
            .flex_col()
            .min_w_0()
            .h_full()
            // Measure the full pane for its toolbar and split divider.
            .child(
                gpui::canvas(
                    move |bounds, window, _| {
                        let width = f64::from(bounds.size.width) as f32;
                        let changed = PANE_W.with(|w| {
                            let mut widths = w.borrow_mut();
                            let old = widths[pane.min(1)];
                            widths[pane.min(1)] = width;
                            (old-width).abs() > 0.5
                        });
                        // Re-render when the toolbar or preview bounds change.
                        if changed {
                            window.on_next_frame(move |_, cx| {
                                weak.update(cx, |_, cx| cx.notify()).ok();
                            });
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            // When split, a thin accent rule marks the pane that keystrokes
            // go to (the other pane's rule is transparent so nothing shifts).
            .when(split, |s| {
                s.border_t_2().border_color(if active {
                    rgb(theme().accent)
                } else {
                    rgba(0x00000000)
                })
            })
            // Clicking anywhere in the pane focuses it (and leaves terminal
            // input) — repaint right away so the focus rule follows the click.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                    window.focus(&this.focus);
                    if this.active_pane != pane {
                        if this.palette_open && this.file_search.inline { this.close_palette(cx); }
                        this.active_pane = pane;
                        cx.notify();
                    }
                }),
            )
            .child(self.render_tab_strip(pane, cx))
            // Path bar: back/forward arrows + breadcrumb + view/sort toolbar.
            .child(self.render_path_bar(pane, cx))
            .child(self.render_entry_filter(pane, cx))
            .child(body_row)
    }

    /// The List view body: clickable column header + virtualized rows.
    fn render_list_body(&self, pane: usize, cx: &Context<Self>) -> AnyElement {
        let tab = self.tab(pane);
        // In find mode the list shows the filtered results (no ".." row);
        // otherwise the full directory with a leading ".." when there's a parent.
        let find_active = tab.is_filtered();
        let has_parent = !find_active && prefs().show_parent && tab.current_dir.parent().is_some();
        let item_count = if find_active {
            tab.find_results.len()
        } else {
            tab.entries.len() + usize::from(has_parent)
        };
        let scroll = tab.scroll_handle.clone();
        let h_scroll = tab.h_scroll.clone();
        let pane_dir = tab.current_dir.clone();
        let fw = &self.tab(pane).list_layout;
        let total_w = fw.total_width();
        let fw = fw.clone();
        // Only engage horizontal scrolling when the columns genuinely overflow
        // the pane — otherwise the small x component of a trackpad flick
        // jiggles the listing sideways while scrolling vertically.
        let h_vw = f64::from(h_scroll.bounds().size.width) as f32;
        let h_overflows = h_vw > 1.0 && total_w > h_vw + 1.0;
        let h_max = (total_w - h_vw).max(0.0);
        let hx = f64::from(h_scroll.offset().x) as f32;
        if hx < -h_max || hx > 0.0 {
            let y = h_scroll.offset().y;
            h_scroll.set_offset(point(px(hx.clamp(-h_max, 0.0)), y));
        }

                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    // Dropping file(s) on empty pane space moves them here. The
                    // tint comes from our tracked drop target rather than
                    // gpui's per-element drag hover, so it can't flicker
                    // between the source and destination panes.
                    .when(
                        cx.has_active_drag() && self.drop_hover == Some((pane, None)),
                        |s| s.bg(Theme::alpha(theme().accent, 0x22)),
                    )
                    .on_drop(cx.listener(move |this, drag: &ExternalPaths, _, cx| {
                        this.drop_files(pane, pane_dir.clone(), drag.paths().to_vec(), cx);
                    }))
                    .child(
                        // Horizontal scroller holding the column header + rows, so
                        // they scroll sideways together and never overflow the pane.
                        div()
                            .id(("hscroll", pane))
                            .size_full()
                            // Horizontal scrolling is driven by hand, not by
                            // overflow_x_scroll: gpui turns a plain vertical
                            // wheel into a sideways scroll on an x-only
                            // scroller, and trackpad drift nudged it too
                            // (issue #13). Only a mostly-horizontal gesture
                            // (or Shift+wheel) moves the columns sideways; the
                            // offset still lives in `h_scroll` via track_scroll.
                            .overflow_hidden()
                            .track_scroll(&h_scroll)
                            .when(h_overflows, |d| {
                                d.on_scroll_wheel(cx.listener(move |this, ev: &ScrollWheelEvent, window, cx| {
                                    let delta = ev.delta.pixel_delta(window.line_height());
                                    let (dx, dy) = (f32::from(delta.x), f32::from(delta.y));
                                    let sideways = if ev.modifiers.shift && dx == 0.0 {
                                        dy
                                    } else if dx.abs() > dy.abs() * 1.5 {
                                        dx
                                    } else {
                                        return;
                                    };
                                    let h = this.tab(pane).h_scroll.clone();
                                    let vw = f64::from(h.bounds().size.width) as f32;
                                    let max = (total_w - vw).max(0.0);
                                    let cur = f64::from(h.offset().x) as f32;
                                    let y = h.offset().y;
                                    h.set_offset(point(px((cur + sideways).clamp(-max, 0.0)), y));
                                    cx.notify();
                                }))
                            })
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .h_full()
                                    .when(h_overflows, |d| d.pb(px(12.)))
                                    .w_full()
                                    .min_w(px(total_w))
                                    .child(self.column_header(pane, cx))
                                    .child(
                                        div()
                                            .relative()
                                            .flex_1()
                                            .min_h_0()
                                            // Right-click empty space → New menu.
                                            .on_mouse_down(
                                                MouseButton::Right,
                                                cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                                                    let (x, y) = (
                                                        f64::from(ev.position.x) as f32,
                                                        f64::from(ev.position.y) as f32,
                                                    );
                                                    this.open_context_menu(pane, x, y, None, cx);
                                                }),
                                            )
                                            // Press on blank space, or on a row's
                                            // Kind/Date/Size area (only the name
                                            // cell stops propagation), → start a
                                            // marquee. Skip modified presses so
                                            // Cmd/Shift-click keep their toggle /
                                            // range-extend meaning (a marquee
                                            // start would clear the selection).
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                                                    if ev.modifiers.platform || ev.modifiers.shift {
                                                        return;
                                                    }
                                                    // A press on a row's NAME cell armed a
                                                    // file drag (runs first, it's deeper);
                                                    // that gesture is a click-or-drag, never
                                                    // a marquee. drag_candidate is cleared on
                                                    // every mouse-up, so this can't go stale.
                                                    if this.drag_candidate.is_some() {
                                                        return;
                                                    }
                                                    this.begin_marquee(
                                                        pane,
                                                        f64::from(ev.position.x) as f32,
                                                        f64::from(ev.position.y) as f32,
                                                        cx,
                                                    );
                                                }),
                                            )
                                            .children(self.marquee_rect(pane))
                                            .child(uniform_list(
                    ("file-list", pane),
                    item_count,
                    cx.processor(move |this, range: std::ops::Range<usize>, _window, cx| {
                        let widths = &fw;
                        let tab = this.tab(pane);
                        let find_active = tab.is_filtered();
                        let has_parent = !find_active && prefs().show_parent && tab.current_dir.parent().is_some();
                        let base_dir = tab.current_dir.clone();
                        let mut items: Vec<AnyElement> = Vec::with_capacity(range.len());

                        for ix in range {
                            let row_key = pane * 100_000 + ix;
                            if has_parent && ix == 0 {
                                let parent =
                                    base_dir.parent().unwrap_or(Path::new("/")).to_path_buf();
                                let icon = icon_element(&parent, true);
                                items.push(
                                    file_row(
                                        &Entry { name: "..".into(), is_dir: true, size: 0, modified: None, created: None, mode: None, loaded: true, hidden_flag: false },
                                        &base_dir,
                                        row_key,
                                        false,
                                        false,       // ".." is never the menu target
                                        widths,
                                        icon,
                                        true,        // accepts drops (move into parent)
                                        this.drop_hover == Some((pane, Some(ix))),
                                        None,        // never renamed
                                        cx.listener({
                                            let parent = parent.clone();
                                            move |this, _: &ClickEvent, _, cx| {
                                                // Finishing a marquee over ".." must
                                                // not navigate away.
                                                if this.marquee_click_suppressed() {
                                                    return;
                                                }
                                                this.navigate_in(pane, parent.clone(), cx);
                                            }
                                        }),
                                        // ".." → background (New Folder/File) menu.
                                        cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                                            let (x, y) = (
                                                f64::from(ev.position.x) as f32,
                                                f64::from(ev.position.y) as f32,
                                            );
                                            this.open_context_menu(pane, x, y, None, cx);
                                            cx.stop_propagation();
                                        }),
                                        cx.listener(move |this, drag: &ExternalPaths, _, cx| {
                                            this.drop_files(pane, parent.clone(), drag.paths().to_vec(), cx);
                                        }),
                                        // ".." isn't draggable — no drag candidate. Let the
                                        // press bubble so the row's click (navigate up)
                                        // registers; swallowing it here dead-ends clicks
                                        // on the ".." name. A marquee starting from this
                                        // press is fine: a stationary release still
                                        // navigates (zero displacement isn't suppressed).
                                        |_: &MouseDownEvent, _, _| {},
                                        // ".." can't be renamed; no hover tracking.
                                        |_: &bool, _, _| {},
                                    )
                                    .into_any_element(),
                                );
                                continue;
                            }

                            let tab = this.tab(pane);
                            let entry_ix = if find_active {
                                tab.find_results[ix]
                            } else if has_parent {
                                ix - 1
                            } else {
                                ix
                            };
                            let entry = &tab.entries[entry_ix];
                            let name = entry.name.clone();
                            let is_dir = entry.is_dir;
                            let target = base_dir.join(&name);
                            let cloud = this.cloud_state(&target, is_dir);
                            let ctx_target = target.clone();
                            let drop_target = target.clone();
                            let hover_target = target.clone();
                            let is_selected = tab.selection.contains(&target);
                            let ctx_active = this.is_ctx_target(&target);
                            let rename_text = this
                                .rename
                                .as_ref()
                                .filter(|r| r.path == target)
                                .map(|r| (r.text.clone(), r.cursor, r.anchor, this.rename_error().is_some()));
                            // Don't drag the row while it's being renamed.
                            let drag_target = if rename_text.is_some() {
                                None
                            } else {
                                Some(target.clone())
                            };
                            let icon = cloud_badged_icon(icon_element(&target, is_dir), cloud, 11.0);

                            items.push(
                                file_row(
                                    entry,
                                    &base_dir,
                                    row_key,
                                    is_selected,
                                    ctx_active,
                                    widths,
                                    icon,
                                    entry.is_folder(), // ordinary folders accept drops
                                    this.drop_hover == Some((pane, Some(ix))),
                                    rename_text,       // editable name when renaming
                                    cx.listener(move |this, ev: &ClickEvent, window, cx| {
                                        // Cmd/Shift extend the selection; otherwise
                                        // folders open and files select / double-open.
                                        this.click_entry(pane, target.clone(), is_dir, ev, window, cx);
                                    }),
                                    cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                                        let (x, y) = (
                                            f64::from(ev.position.x) as f32,
                                            f64::from(ev.position.y) as f32,
                                        );
                                        this.open_context_menu(
                                            pane,
                                            x,
                                            y,
                                            Some((ctx_target.clone(), is_dir)),
                                            cx,
                                        );
                                        cx.stop_propagation();
                                    }),
                                    cx.listener(move |this, drag: &ExternalPaths, _, cx| {
                                        // Drop onto a folder → move (or upload) the file(s) into it.
                                        this.drop_files(pane, drop_target.clone(), drag.paths().to_vec(), cx);
                                    }),
                                    cx.listener(move |this, ev: &MouseDownEvent, _, _cx| {
                                        // Arm a file drag, but DON'T stop propagation:
                                        // the row's own click handler is an ANCESTOR of
                                        // the name cell this press lands on, and gpui
                                        // only registers a click if the row sees the
                                        // mouse-down too (swallowing it here made name
                                        // clicks select nothing). The marquee container
                                        // skips presses that armed a drag instead.
                                        if let Some(dp) = &drag_target {
                                            let (x, y) = (
                                                f64::from(ev.position.x) as f32,
                                                f64::from(ev.position.y) as f32,
                                            );
                                            this.drag_candidate = Some((pane, dp.clone(), (x, y)));
                                        }
                                    }),
                                    cx.listener(move |this, on: &bool, _, _| {
                                        if *on {
                                            this.hovered = Some((pane, hover_target.clone()));
                                        } else if this
                                            .hovered
                                            .as_ref()
                                            .is_some_and(|(hp, p)| *hp == pane && *p == hover_target)
                                        {
                                            this.hovered = None;
                                        }
                                    }),
                                )
                                .into_any_element(),
                            );
                        }
                        items
                    }),
                )
                .size_full()
                .track_scroll(scroll)
                .on_scroll_wheel(cx.listener(move |this, _: &ScrollWheelEvent, _, cx| {
                    this.active_pane = pane;
                    this.mark_scrolled(pane, cx);
                    cx.notify()
                }))
                // Vertical only: gpui would otherwise feed a purely sideways
                // swipe into the vertical scroll.
                .map(restrict_scroll_axis),
                                            ) // close listing div .child(uniform_list)
                                    ) // close content flex_col .child(listing div)
                            ) // close hscroll .child(content)
                    ) // close body .child(hscroll)
                    // Vertical scrollbar, pinned to the pane's right edge.
                    .children(self.scrollbar_thumb(pane, cx))
                    // Horizontal scrollbar, shown when columns overflow.
                    .children(self.h_scrollbar_thumb(pane, total_w, cx))
                    // (The filter box is rendered once at the pane level.)
                    .into_any_element()
    }

    /// The Icons view body: a virtualized grid of large icons.
    fn render_icons_body(&self, pane: usize, cx: &Context<Self>) -> AnyElement {
        let tab = self.tab(pane);
        let scroll = tab.scroll_handle.clone();
        let pane_dir = tab.current_dir.clone();
        let width = browser_width(pane).max(240.0);
        let cell_w = 108.0_f32;
        let cols = ((width / cell_w).floor() as usize).max(1);
        let n = if tab.is_filtered() {
            tab.find_results.len()
        } else {
            tab.entries.len()
        };
        let rows = n.div_ceil(cols);

        div()
            .relative()
            .flex_1()
            .min_h_0()
            // Tracked drop target (see render_list_body) — no flicker.
            .when(
                cx.has_active_drag() && self.drop_hover == Some((pane, None)),
                |s| s.bg(Theme::alpha(theme().accent, 0x22)),
            )
            .on_drop(cx.listener(move |this, drag: &ExternalPaths, _, cx| {
                this.drop_files(pane, pane_dir.clone(), drag.paths().to_vec(), cx);
            }))
            .child(
                uniform_list(
                    ("icons", pane),
                    rows,
                    cx.processor(move |this, range: std::ops::Range<usize>, _w, cx| {
                        let tab = this.tab(pane);
                        let find_active = tab.is_filtered();
                        let base_dir = tab.current_dir.clone();
                        let n = if find_active {
                            tab.find_results.len()
                        } else {
                            tab.entries.len()
                        };
                        let mut out: Vec<AnyElement> = Vec::with_capacity(range.len());
                        for row in range {
                            let mut cells: Vec<AnyElement> = Vec::with_capacity(cols);
                            for c in 0..cols {
                                let i = row * cols + c;
                                if i >= n {
                                    break;
                                }
                                let tab = this.tab(pane);
                                let entry_ix = if find_active { tab.find_results[i] } else { i };
                                let entry = &tab.entries[entry_ix];
                                let name = entry.name.clone();
                                let is_dir = entry.is_dir;
                                let target = base_dir.join(&name);
                                let selected = tab.selection.contains(&target);
                                let ctx_active = this.is_ctx_target(&target);
                                cells.push(icon_cell(pane, name, target, is_dir, selected, ctx_active, cell_w, cx));
                            }
                            out.push(div().flex().w_full().px_2().children(cells).into_any_element());
                        }
                        out
                    }),
                )
                .size_full()
                .track_scroll(scroll)
                .on_scroll_wheel(cx.listener(move |this, _: &ScrollWheelEvent, _, cx| {
                    this.active_pane = pane;
                    this.mark_scrolled(pane, cx);
                    cx.notify()
                })),
            )
            .children(self.scrollbar_thumb(pane, cx))
            // (The filter box is rendered once at the pane level.)
            .into_any_element()
    }

    /// Read-only horizontal scroll indicator for a pane's columns. Returns
    /// `None` when the columns fit (no horizontal overflow).
    /// Geometry of a pane's horizontal scrollbar: (viewport, max scroll,
    /// thumb width, thumb left) — `None` when the columns fit.
    fn hbar_geometry(&self, pane: usize, total_w: f32) -> Option<(f32, f32, f32, f32)> {
        let base = &self.tab(pane).h_scroll;
        let viewport = f64::from(base.bounds().size.width) as f32;
        let max = total_w - viewport;
        if viewport <= 1.0 || max <= 1.0 {
            return None;
        }
        let scrolled = (-(f64::from(base.offset().x) as f32)).clamp(0.0, max);
        let thumb_w = (viewport * viewport / total_w).clamp(28.0, viewport);
        let thumb_left = (viewport - thumb_w) * (scrolled / max);
        Some((viewport, max, thumb_w, thumb_left))
    }

    /// The horizontal scrollbar: drag the thumb, or press the track to jump
    /// there (and keep dragging).
    fn h_scrollbar_thumb(&self, pane: usize, total_w: f32, cx: &Context<Self>) -> Option<AnyElement> {
        let (_, _, thumb_w, thumb_left) = self.hbar_geometry(pane, total_w)?;
        let dragging = self.hbar_drag.is_some_and(|d| d.0 == pane);
        Some(
            div()
                .id(("hbar", pane))
                .absolute()
                .left_0()
                .right_0()
                .bottom_0()
                .h(px(12.0))
                .bg(rgb(theme().sidebar))
                .border_t_1().border_color(rgb(theme().border))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                        this.begin_hbar_drag(pane, total_w, f32::from(ev.position.x), cx);
                    }),
                )
                .child(
                    div()
                        .absolute()
                        .bottom(px(2.0))
                        .left(px(thumb_left))
                        .h(px(8.0))
                        .w(px(thumb_w))
                        .rounded_full()
                        .bg(Theme::alpha(theme().text, if dragging { 0x66 } else { 0x33 }))
                        .hover(|s| s.bg(Theme::alpha(theme().text, 0x55))),
                )
                .into_any_element(),
        )
    }

    /// Press on the horizontal scrollbar: on the thumb, grab it where it is;
    /// on the track, jump the thumb's center to the cursor first.
    fn begin_hbar_drag(&mut self, pane: usize, total_w: f32, x: f32, cx: &mut Context<Self>) {
        let Some((viewport, max, thumb_w, thumb_left)) = self.hbar_geometry(pane, total_w) else {
            return;
        };
        self.active_pane = pane;
        let h = self.tab(pane).h_scroll.clone();
        let origin = f64::from(h.bounds().origin.x) as f32;
        let local = x - origin;
        let travel = (viewport - thumb_w).max(1.0);
        let mut scrolled = (-(f64::from(h.offset().x) as f32)).clamp(0.0, max);
        if local < thumb_left || local > thumb_left + thumb_w {
            let left = (local - thumb_w / 2.0).clamp(0.0, travel);
            scrolled = left / travel * max;
            let y = h.offset().y;
            h.set_offset(point(px(-scrolled), y));
        }
        self.hbar_drag = Some((pane, x, scrolled));
        cx.stop_propagation();
        cx.notify();
    }

    fn update_hbar_drag(&mut self, x: f32, cx: &mut Context<Self>) {
        let Some((pane, start_x, start_scrolled)) = self.hbar_drag else { return };
        if pane >= self.panes.len() {
            return;
        }
        let fw = &self.tab(pane).list_layout;
        let total_w = fw.total_width();
        let Some((viewport, max, thumb_w, _)) = self.hbar_geometry(pane, total_w) else { return };
        let travel = (viewport - thumb_w).max(1.0);
        let scrolled = (start_scrolled + (x - start_x) * (max / travel)).clamp(0.0, max);
        let h = self.tab(pane).h_scroll.clone();
        let y = h.offset().y;
        h.set_offset(point(px(-scrolled), y));
        cx.notify();
    }


}

#[allow(clippy::too_many_arguments)]


impl Render for Shuffle {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.dismiss_space_map) { self.close_space_map(window, cx); }
        self.sync_video_preview(cx);
        let owner=cx.entity().downgrade();
        window.on_next_frame(move|_,cx|{let _=owner.update(cx,|this,cx|this.prewarm_icons(cx));});
        let t = theme();
        self.cancel_unused_previews();
        // Kick off any pending waterfall folder reads before rendering the tree.
        self.ensure_waterfall_loaded(cx);
        // Main row: sidebar | content (including preview) | optional size map.
        let main_row = div()
            .flex()
            .flex_1()
            .min_h_0()
            .child(self.render_sidebar(cx))
            .child(self.render_content(cx));

        let mut root = div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(t.bg))
            .text_color(rgb(t.text))
            .text_size(px(UI_FONT_SIZE))
            // Focusable so it receives key events (Cmd+P, palette typing).
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _: &MouseDownEvent, window, cx| {
                if this.space_map.is_some() { this.close_space_map(window, cx); }
            }))
            // Native menu-bar actions (File / View / Go menus).
            .on_action(cx.listener(|this, _: &NewProject, window, cx| this.open_project_dialog(window, cx)))
            .on_action(cx.listener(|this, _: &ToggleStudioSplit, _, cx| this.toggle_studio_split(cx)))
            .on_action(cx.listener(|_, _: &ToggleInspector, _, cx| studio::toggle_inspector(cx)))
            .on_action(cx.listener(|this, _: &NewTab, _, cx| {
                let p = this.active_pane;
                this.new_tab_in(p, cx);
            }))
            .on_action(cx.listener(|this, _: &NewFolder, window, cx| {
                let p = this.active_pane;
                this.new_folder(p, window, cx);
            }))
            .on_action(cx.listener(|this, _: &CloseTab, _, cx| {
                let p = this.active_pane;
                let t = this.pane(p).active;
                this.close_tab(p, t, cx);
            }))
            .on_action(cx.listener(|this, _: &MoveToTrash, _, cx| {
                let p = this.active_pane;
                this.request_delete(p, cx);
            }))
            .on_action(cx.listener(|this, _: &ViewList, _, cx| {
                let p = this.active_pane;
                this.set_view(p, ViewMode::List, cx);
            }))
            .on_action(cx.listener(|this, _: &ViewIcons, _, cx| {
                let p = this.active_pane;
                this.set_view(p, ViewMode::Icons, cx);
            }))
            .on_action(cx.listener(|_, _: &ToggleSidebar, _, cx| {
                let mut np = prefs();
                np.sidebar_collapsed = !np.sidebar_collapsed;
                apply_prefs(np, cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &GoBack, _, cx| {
                let p = this.active_pane;
                this.go_back(p, cx);
            }))
            .on_action(cx.listener(|this, _: &GoForward, _, cx| {
                let p = this.active_pane;
                this.go_forward(p, cx);
            }))
            .on_action(cx.listener(|this, _: &GoHome, _, cx| {
                this.navigate_to(home_dir(), cx);
            }))
            .on_action(cx.listener(|this, _: &GoApplications, _, cx| {
                this.navigate_to(PathBuf::from("/Applications"), cx);
            }))
            .on_action(cx.listener(|this, _: &GoComputer, _, cx| {
                this.navigate_to(PathBuf::from("/"), cx);
            }))
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                this.focus_file_search(false, window, cx);
            }))
            // Track column drags anywhere in the window so the cursor can leave
            // the thin handle without dropping the resize.
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, window, cx| {
                let x = f64::from(ev.position.x) as f32;
                let y = f64::from(ev.position.y) as f32;
                this.resize_preview(x, cx);
                // A press-then-move on a file row becomes a native OS drag (so
                // files can be dropped into Finder or any other app).
                this.maybe_start_os_drag(x, y, window, cx);
                if let Some((pane, start_y, start_height)) = this.terminal_resize {
                    if pane < this.panes.len() {
                        this.tab_mut(pane).terminal_height = (start_height + start_y - y).clamp(150.0, (f32::from(window.bounds().size.height) - 230.0).max(150.0));
                        cx.notify();
                    }
                }
                this.update_resize(x, cx);
                this.update_scroll_drag(y, cx);
                this.update_hbar_drag(x, cx);
                this.update_divider(x, cx);
                this.update_marquee(x, y, cx);
                this.update_drop_hover(x, y, cx);
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.finish_preview_resize(cx);
                    this.drag_candidate = None;
                    this.terminal_resize = None;
                    this.end_resize(cx);
                    this.end_scroll_drag(cx);
                    this.hbar_drag = None;
                    this.divider_drag = None;
                    this.end_marquee(cx);
                    // A native file drag ends as a synthesized mouse-up.
                    if this.drop_hover.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            // A slim titlebar strip in the app's own background color. With the
            // OS titlebar transparent, this is what shows behind the traffic
            // lights, and it keeps the content clear of them.
            .child(self.render_studio_toolbar(cx));

        // The update bar (available / downloading / failed), just under the titlebar.
        if let Some(banner) = self.render_update_banner(cx) {
            root = root.child(banner);
        }
        // SFTP error bar (connection / transfer failure).
        if let Some(err) = self.remote_error.clone() {
            root = root.child(
                div()
                    .flex_none()
                    .w_full()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_4()
                    .py_1p5()
                    .bg(rgb(0xef4444))
                    .text_color(rgb(0xffffff))
                    .child(div().flex_none().child("⚠"))
                    .child(div().flex_1().min_w_0().truncate().child(err))
                    .child(
                        div()
                            .id("sftp-err-dismiss")
                            .flex_none()
                            .px_2()
                            .rounded_md()
                            .cursor_pointer()
                            .hover(|s| s.bg(rgba(0xffffff33)))
                            .child("✕")
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.remote_error = None;
                                cx.notify();
                            })),
                    ),
            );
        }

        // Cloud sync status (neutral; clears itself).
        if let Some(msg) = self.notice.clone() {
            root = root.child(
                div()
                    .flex_none()
                    .w_full()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_4()
                    .py_1p5()
                    .bg(rgb(t.accent))
                    .text_color(rgb(0xffffff))
                    .child(div().flex_none().child("↻"))
                    .child(div().flex_1().min_w_0().truncate().child(msg)),
            );
        }

        root = root.child(main_row).child(self.render_studio_status(cx));

        if let Some(panel) = self.render_jobs_panel() {
            root = root.child(panel);
        }
        if self.palette_open && !self.file_search.inline {
            root = root.child(self.render_palette(cx));
        }
        self.schedule_native_menu(window, cx);
        if self.confirm_delete.is_some() {
            root = root.child(self.render_confirm_delete(cx));
        }
        if self.info_panel.is_some() {
            root = root.child(self.render_info_panel(cx));
        }
        if self.batch_rename.is_some() {
            root = root.child(self.render_batch_rename(cx));
        }
        if self.ssh_ask {
            root = root.child(self.render_ssh_prompt(cx));
        }
        if self.server_dialog.is_some() {
            root = root.child(self.render_server_dialog(cx));
        }
        if self.group_dialog.is_some() {
            root = root.child(self.render_group_dialog(cx));
        }
        if self.new_text_file.is_some() { root = root.child(self.render_text_file_dialog(cx)); }
        if self.project_dialog.is_some() {
            root = root.child(self.render_project_dialog(cx));
        }
        if prefs().show_fps {
            root = root.child(fps_overlay());
        }
        root
    }
}

/// A group member row: click opens/navigates, right-click offers "Remove from
/// Group". `gidx` is the owning group's index.
fn push_group_member(
    items: &mut Vec<AnyElement>,
    cx: &Context<Shuffle>,
    key: &mut usize,
    gidx: usize,
    target: PathBuf,
    current: &Path,
) {
    *key += 1;
    let is_dir = cached_is_dir(&target);
    let active = is_dir && target.as_path() == current;
    let label = path_label(&target);
    let path_str = display_path(&target);
    let tooltip = if path_str == label {
        format!("{label}\n{path_str}")
    } else {
        path_str
    };
    let nav_target = target.clone();
    let right_target = target.clone();
    let item = nav_item(
        label,
        tooltip,
        icon_element(&target, is_dir),
        *key,
        active,
        cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.unsync_ssh_browser(this.active_pane, cx);
            if nav_target.is_dir() {
                this.navigate_to(nav_target.clone(), cx);
            } else {
                let _ = Command::new("open").arg(&nav_target).spawn();
            }
        }),
        cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
            let (x, y) = (f64::from(ev.position.x) as f32, f64::from(ev.position.y) as f32);
            this.open_sidebar_menu(
                x,
                y,
                SidebarTarget::GroupMember(gidx, right_target.clone()),
                cx,
            );
            cx.stop_propagation();
        }),
    );
    items.push(item.into_any_element());
}

/// A path shown with the home directory abbreviated to `~`.
fn display_path(p: &Path) -> String {
    let home = home_dir();
    if let Ok(rest) = p.strip_prefix(&home) {
        if rest.as_os_str().is_empty() {
            return "~".to_string();
        }
        return format!("~/{}", rest.display());
    }
    p.display().to_string()
}

/// A back/forward navigation arrow. Dimmed and inert when `enabled` is false.
fn nav_arrow(
    id: impl Into<ElementId>,
    icon: Icon,
    enabled: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let t = theme();
    let base = div()
        .id(id)
        .flex_none()
        .w(px(22.0))
        .h(px(22.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .text_color(if enabled { rgb(t.text) } else { rgb(t.text_dim) })
        .child(ui_icon(icon, 16., if enabled { rgb(t.text_muted) } else { Theme::alpha(t.text_dim, 0x66) }));
    if enabled {
        base.cursor_pointer()
            .hover(|s| s.bg(rgb(t.hover)))
            .on_click(on_click)
            .into_any_element()
    } else {
        base.into_any_element()
    }
}

/// One clickable breadcrumb segment that navigates `pane` to `full` when clicked.
fn breadcrumb_seg(
    key: usize,
    pane: usize,
    label: String,
    full: PathBuf,
    active: bool,
    // The crumb for the pane's CURRENT directory: clicking it would only
    // re-navigate to where you already are, so it opens the command palette
    // targeted at that folder instead (Copy path / Reveal / new tab / …).
    is_current: bool,
    cx: &Context<Shuffle>,
) -> AnyElement {
    let t = theme();
    div()
        .id(("crumb", key))
        .flex_none()
        .px_1()
        .h(px(20.0))
        .flex()
        .items_center()
        .rounded_md()
        .cursor_pointer()
        .text_color(if active { rgb(t.text) } else { rgb(t.text_dim) })
        .hover(|s| s.bg(rgb(t.hover)).text_color(rgb(t.text)))
        .child(label)
        .when(is_current, |d| d.tooltip(tip("Actions for this folder…")))
        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
            if is_current {
                this.open_palette_for_path(pane, full.clone(), window, cx);
            } else {
                this.navigate_in(pane, full.clone(), cx);
            }
            cx.stop_propagation();
        }))
        .into_any_element()
}

/// The "/" divider between breadcrumb segments.
fn breadcrumb_sep() -> AnyElement {
    div()
        .flex_none()
        .px(px(2.0))
        .text_color(rgb(theme().text_dim))
        .child("/")
        .into_any_element()
}

/// Expand a leading `~` to the home directory.
fn expand_path(s: &str) -> PathBuf {
    if s == "~" {
        home_dir()
    } else if let Some(rest) = s.strip_prefix("~/") {
        home_dir().join(rest)
    } else {
        PathBuf::from(s)
    }
}

/// One clickable row in the right-click context menu.
/// Append a separator, unless the menu is empty or one is already last
/// (sections are conditional, so back-to-back separators would show up).
fn push_menu_sep(nodes: &mut Vec<MenuNode>) {
    if !matches!(nodes.last(), None | Some(MenuNode::Sep)) {
        nodes.push(MenuNode::Sep);
    }
}

/// Preview files with Quick Look (the same viewer as pressing Space).
fn quick_look_paths(paths: &[PathBuf]) {
    if paths.is_empty() {
        return;
    }
    let _ = Command::new("qlmanage")
        .arg("-p")
        .args(paths)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

/// Put files on the general pasteboard as file URLs — Finder, Terminal, and
/// every other app understand a paste after this (same writer the drag-out
/// code uses).
fn copy_files_to_pasteboard(paths: &[PathBuf]) -> bool {
    use objc2::runtime::ProtocolObject;
    use objc2_app_kit::{NSPasteboard, NSPasteboardWriting};
    use objc2_foundation::NSArray;
    let pb = NSPasteboard::generalPasteboard();
    let _ = pb.clearContents();
    let urls: Vec<_> = paths
        .iter()
        .map(|p| NSURL::fileURLWithPath(&NSString::from_str(&p.to_string_lossy())))
        .collect();
    let writers: Vec<&ProtocolObject<dyn NSPasteboardWriting>> =
        urls.iter().map(|u| ProtocolObject::from_ref(&**u)).collect();
    pb.writeObjects(&NSArray::from_slice(&writers))
}

/// Read file URLs off the general pasteboard (whatever Copy above — or
/// Finder's ⌘C — put there).
fn pasteboard_file_paths() -> Vec<PathBuf> {
    use objc2_app_kit::NSPasteboard;
    let pb = NSPasteboard::generalPasteboard();
    let Some(items) = pb.pasteboardItems() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in items.iter() {
        let Some(s) = item.stringForType(&NSString::from_str("public.file-url")) else {
            continue;
        };
        if let Some(p) = file_url_to_path(&s.to_string()) {
            if p.exists() {
                out.push(p);
            }
        }
    }
    out
}

/// "file:///Users/x/My%20File.txt" → PathBuf (strips the host, percent-decodes).
fn file_url_to_path(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix("file://")?;
    // Skip a host component if present (file://localhost/…).
    let rest = if rest.starts_with('/') {
        rest
    } else {
        &rest[rest.find('/')?..]
    };
    let hex = |b: u8| -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        }
    };
    let bytes = rest.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    let s = String::from_utf8(out).ok()?;
    if s.is_empty() || s == "/" {
        None
    } else {
        Some(PathBuf::from(s.trim_end_matches('/')))
    }
}

/// A non-interactive, dimmed context-menu row.


/// Whether `path` looks like a raster image we can act on.
fn is_image(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).as_deref(),
        Some("jpg" | "jpeg" | "png" | "gif" | "heic" | "heif" | "tiff" | "tif" | "bmp" | "webp")
    )
}

/// Whether `path` is a PDF.
fn is_pdf(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).as_deref() == Some("pdf")
}

/// Archive suffixes "Extract Here" can unpack with built-in tools, longest
/// first so `foo.tar.gz` strips as a tarball rather than a ".gz".
const ARCHIVE_SUFFIXES: &[&str] = &[
    ".tar.gz", ".tar.bz2", ".tar.xz", ".tgz", ".tbz", ".txz", ".tar", ".zip",
];

/// The matched archive suffix of `path`, if it's an extractable archive.
fn archive_suffix(path: &Path) -> Option<&'static str> {
    let name = path.file_name()?.to_string_lossy().to_lowercase();
    ARCHIVE_SUFFIXES.iter().find(|s| name.ends_with(*s)).copied()
}

/// The extraction command for `path`, ready for the destination directory to
/// be appended as its final argument (`ditto -xk src DEST` / `tar -xf src -C DEST`).
fn archive_extract_command(path: &Path) -> Option<Command> {
    let suffix = archive_suffix(path)?;
    let mut c;
    if suffix == ".zip" {
        c = Command::new("ditto");
        c.arg("-xk").arg(path);
    } else {
        c = Command::new("tar");
        c.arg("-xf").arg(path).arg("-C");
    }
    Some(c)
}

// ----- user shell-script actions (the "Scripts" extension point) -----

/// A user-provided shell-script action discovered in the Scripts folder. The
/// user drops an executable script there; Shuffle offers it in the right-click
/// menu for matching items and runs it with the selected paths as arguments.
struct ScriptAction {
    name: String,
    path: PathBuf,
    /// Type matchers (OR'd): `any`, `folder`/`file`, a kind word (image/video/
    /// pdf/…), or a bare extension (`png`).
    types: Vec<String>,
}

/// The folder scripts live in: `…/Application Support/Schnauz/actions`.
fn script_actions_dir() -> Option<PathBuf> {
    config_dir().map(|d| d.join("actions"))
}

/// A regular file the current user can execute (a runnable script).
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    match fs::metadata(path) {
        Ok(m) => m.is_file() && (m.permissions().mode() & 0o111 != 0),
        Err(_) => false,
    }
}

/// Match a `@shuffle.KEY: value` metadata line (case-insensitive key).
fn script_header_value(line: &str, key: &str) -> Option<String> {
    let prefix = format!("@shuffle.{key}:");
    line.to_lowercase()
        .starts_with(&prefix)
        .then(|| line[prefix.len()..].trim().to_string())
}

/// Read a script's leading metadata: display name (defaults to the file stem)
/// and the item types it applies to (defaults to `any`).
fn parse_script_header(path: &Path, fname: &str) -> (String, Vec<String>) {
    let mut name = fname.rsplit_once('.').map(|(s, _)| s).unwrap_or(fname).to_string();
    let mut types = vec!["any".to_string()];
    if let Ok(f) = fs::File::open(path) {
        for line in BufReader::new(f).lines().take(40).map_while(Result::ok) {
            let Some(rest) = line.trim().strip_prefix('#') else {
                continue;
            };
            let rest = rest.trim();
            if let Some(v) = script_header_value(rest, "name") {
                if !v.is_empty() {
                    name = v;
                }
            } else if let Some(v) = script_header_value(rest, "types") {
                let parsed: Vec<String> = v
                    .split(',')
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty())
                    .collect();
                if !parsed.is_empty() {
                    types = parsed;
                }
            }
        }
    }
    (name, types)
}

/// Scan the Scripts folder for executable actions. Cheap (small folder), called
/// when a context menu opens. Skips dotfiles, the README, and non-executables.
fn discover_script_actions() -> Vec<ScriptAction> {
    let Some(dir) = script_actions_dir() else {
        return Vec::new();
    };
    let Ok(rd) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let path = e.path();
        let fname = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if fname.starts_with('.') || fname.eq_ignore_ascii_case("readme.md") {
            continue;
        }
        if !is_executable_file(&path) {
            continue;
        }
        let (name, types) = parse_script_header(&path, &fname);
        out.push(ScriptAction { name, path, types });
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// Whether a script action applies to an item of this name/kind.
fn script_action_applies(a: &ScriptAction, name: &str, is_dir: bool) -> bool {
    let ext = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());
    a.types.iter().any(|t| match t.as_str() {
        "any" | "*" | "all" => true,
        "folder" | "dir" | "directory" => is_dir,
        "file" => !is_dir,
        other => match KindClass::from_word(other) {
            Some(k) => k.matches(name, is_dir),
            None => ext.as_deref() == Some(other),
        },
    })
}

/// Create the Scripts folder (and, on first creation, a README + a couple of
/// safe example scripts) and return its path. Idempotent.
fn ensure_scripts_dir() -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let dir = script_actions_dir()?;
    let fresh = !dir.exists();
    fs::create_dir_all(&dir).ok()?;
    if fresh {
        let readme = r#"# Shuffle — Script Actions

Drop an **executable** script in this folder and Shuffle shows it in the
right-click menu for matching items. The selected paths are passed as arguments
(`"$@"`), and also as `$SHUFFLE_PATHS` (newline-separated) with `$SHUFFLE_DIR`
set to the current folder.

Add an optional metadata header (any comment style) to control how it appears:

    # @shuffle.name: Optimize Image
    # @shuffle.types: png,jpg,jpeg      # extensions, or: image video audio pdf
    #                                   # doc archive code app folder file any

- No `name` → the file's name is used.
- No `types` → shows for everything (`any`).
- The file must be executable: `chmod +x "your script"`.

Turn the feature on in Settings → Script Actions.
"#;
        let _ = fs::write(dir.join("README.md"), readme);

        let copy_path = "#!/bin/bash\n# @shuffle.name: Copy Path\n# @shuffle.types: any\nprintf '%s' \"$1\" | pbcopy\n";
        let open_term = "#!/bin/bash\n# @shuffle.name: Open in Terminal\n# @shuffle.types: folder\nopen -a Terminal \"$1\"\n";
        for (fname, body) in [("Copy Path.sh", copy_path), ("Open in Terminal.sh", open_term)] {
            let p = dir.join(fname);
            if fs::write(&p, body).is_ok() {
                if let Ok(md) = fs::metadata(&p) {
                    let mut perm = md.permissions();
                    perm.set_mode(0o755);
                    let _ = fs::set_permissions(&p, perm);
                }
            }
        }
    }
    Some(dir)
}

/// Path to the bundled `removebg` Swift helper, if it was compiled in.
fn removebg_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let cand = exe.parent()?.join("removebg");
    cand.exists().then_some(cand)
}

/// Path to the bundled `cloudctl` Swift helper (download/evict cloud files).
fn cloudctl_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let cand = exe.parent()?.join("cloudctl");
    cand.exists().then_some(cand)
}

/// A folder's visible entries → (size, mtime, online-only), for spotting when
/// a cloud provider has changed anything in it. Hidden files are skipped
/// (`.DS_Store` churns on its own and providers don't sync it).
type CloudFingerprint = BTreeMap<OsString, (u64, Option<SystemTime>, bool)>;

fn cloud_fingerprint(dir: &Path) -> CloudFingerprint {
    use std::os::macos::fs::MetadataExt;
    let mut out = BTreeMap::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let name = e.file_name();
            if name.as_encoded_bytes().first() == Some(&b'.') || name == "Icon\r" {
                continue;
            }
            if let Ok(m) = fs::symlink_metadata(e.path()) {
                out.insert(name, (m.len(), m.modified().ok(), m.st_flags() & SF_DATALESS != 0));
            }
        }
    }
    out
}

/// How many entries differ (added, removed, or changed) between two snapshots.
fn fingerprint_diff(a: &CloudFingerprint, b: &CloudFingerprint) -> usize {
    let changed = a.iter().filter(|(k, v)| b.get(*k) != Some(v)).count();
    changed + b.keys().filter(|k| !a.contains_key(*k)).count()
}

/// Items in `dir` mid-download/upload, per the `cloudctl status` helper.
fn cloud_busy_count(tool: &Path, dir: &Path) -> usize {
    Command::new(tool)
        .arg("status")
        .arg(dir)
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().strip_prefix("busy ").and_then(|n| n.parse().ok()))
        .unwrap_or(0)
}

/// Friendly name of the cloud store holding `path` ("Dropbox", "OneDrive",
/// "iCloud Drive", …), for menu labels and sync notices.
fn cloud_provider_name(path: &Path) -> String {
    let cs = home_dir().join("Library/CloudStorage");
    match path.strip_prefix(&cs).ok().and_then(|r| r.components().next()) {
        Some(c) => pretty_cloud_name(&c.as_os_str().to_string_lossy()),
        None => "iCloud Drive".to_string(),
    }
}

/// Which cloud store `path` lives in, or `None` if it isn't under one. iCloud
/// Drive is `~/Library/Mobile Documents`; third-party File Provider stores live
/// under `~/Library/CloudStorage`.
fn cloud_kind(path: &Path) -> Option<CloudKind> {
    let home = home_dir();
    if path.starts_with(home.join("Library/Mobile Documents")) {
        return Some(CloudKind::ICloud);
    }
    if path.starts_with(home.join("Library/CloudStorage")) {
        return Some(CloudKind::Provider);
    }
    None
}

/// Whether `path` is an online-only cloud placeholder (kernel `SF_DATALESS`).
/// Whether marquee debug logging is on (`SHUFFLE_MARQUEE_LOG=1`).
fn mq_log() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("SHUFFLE_MARQUEE_LOG").is_some())
}

fn is_dataless(path: &Path) -> bool {
    use std::os::macos::fs::MetadataExt;
    fs::metadata(path)
        .map(|m| m.st_flags() & SF_DATALESS != 0)
        .unwrap_or(false)
}

/// `SF_DATALESS` cache for render, keyed by path with a short TTL. The flag
/// isn't reliably captured during bulk directory enumeration (it lags on a
/// background thread), so the badge reads it lazily here instead. The stat is a
/// local metadata read (cloud placeholders live on-disk; only their bytes are
/// remote), so it's fast on the main thread — no network blocking like a dead
/// mount, hence no background dance.
static DATALESS_STAT: OnceLock<Mutex<HashMap<PathBuf, (bool, Instant)>>> = OnceLock::new();

fn cached_dataless(path: &Path) -> bool {
    const TTL: Duration = Duration::from_secs(3);
    let map = DATALESS_STAT.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some((v, at)) = map.lock().unwrap().get(path).copied() {
        if at.elapsed() <= TTL {
            return v;
        }
    }
    let v = is_dataless(path);
    map.lock().unwrap().insert(path.to_path_buf(), (v, Instant::now()));
    v
}

/// Files to act on for a cloud download/evict: `path` itself when it's a file,
/// or the matching descendants when it's a folder. `want_dataless` selects
/// online-only files (download) or materialized ones (evict). Capped at `cap`
/// so a huge tree can't spawn an unbounded number of helper calls.
fn collect_cloud_files(path: &Path, want_dataless: bool, cap: usize) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    if path.is_file() {
        if is_dataless(path) == want_dataless {
            out.push(path.to_path_buf());
        }
        return out;
    }
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if out.len() >= cap {
            break;
        }
        let Ok(rd) = fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            if out.len() >= cap {
                break;
            }
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(e.path()),
                Ok(t) if t.is_file() => {
                    let p = e.path();
                    if is_dataless(&p) == want_dataless {
                        out.push(p);
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// Installed terminal emulators, for the "Open in …" services.
fn installed_terminals() -> Vec<(&'static str, PathBuf)> {
    [
        ("Terminal", "/System/Applications/Utilities/Terminal.app"),
        ("iTerm", "/Applications/iTerm.app"),
        ("Ghostty", "/Applications/Ghostty.app"),
        ("kitty", "/Applications/kitty.app"),
        ("WezTerm", "/Applications/WezTerm.app"),
        ("Warp", "/Applications/Warp.app"),
        ("Alacritty", "/Applications/Alacritty.app"),
    ]
    .into_iter()
    .filter(|(_, p)| Path::new(p).exists())
    .map(|(n, p)| (n, PathBuf::from(p)))
    .collect()
}

/// Applications that can open `path`, via LaunchServices (Finder's "Open With").
fn apps_for_file(path: &Path) -> Vec<(String, PathBuf)> {
    let Some(s) = path.to_str() else {
        return Vec::new();
    };
    let ns = NSString::from_str(s);
    let url = NSURL::fileURLWithPath(&ns);
    let ws = NSWorkspace::sharedWorkspace();
    let arr = ws.URLsForApplicationsToOpenURL(&url);
    let mut out: Vec<(String, PathBuf)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for i in 0..arr.count() {
        let u = arr.objectAtIndex(i);
        if let Some(p) = u.path() {
            let pb = PathBuf::from(p.to_string());
            let name = pb
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            if !name.is_empty() && seen.insert(name.clone()) {
                out.push((name, pb));
            }
        }
        if out.len() >= 16 {
            break;
        }
    }
    out.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    out
}

/// A non-existing child path under `dir` based on `base` (adds " 2", " 3" …).
/// Like [`unique_child`], but keeps the extension at the end: pasting
/// "alpha.txt" beside itself yields "alpha 2.txt", not "alpha.txt 2".
fn unique_dest(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), Some(e.to_string())),
        _ => (name.to_string(), None),
    };
    let mut n = 2;
    loop {
        let candidate = match &ext {
            Some(e) => format!("{stem} {n}.{e}"),
            None => format!("{stem} {n}"),
        };
        let path = dir.join(candidate);
        if !path.exists() {
            return path;
        }
        n += 1;
    }
}

fn unique_child(dir: &Path, base: &str) -> PathBuf {
    let mut path = dir.join(base);
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{base} {n}"));
        n += 1;
    }
    path
}

/// Move a path to the macOS Trash (recoverable). Returns whether it succeeded.
// ----- Get Info / batch rename models ---------------------------------------

/// The open Get Info card.
struct InfoPanel {
    path: PathBuf,
    is_dir: bool,
    info: FileInfo,
    /// "rwxr-xr-x"-style, with the octal mode.
    perms: String,
    /// "user (group)", filled in off-thread.
    owner: String,
    /// (bytes, items); `None` while a folder is still being summed.
    total: Option<(u64, u64)>,
}

#[derive(Clone, Copy, PartialEq)]
enum BatchMode {
    /// Find → replace within each name (extension untouched).
    Replace,
    /// Insert text before or after each name (before the extension).
    Add,
    /// "Name 1", "Name 2", … keeping each extension.
    Number,
}

impl BatchMode {
    fn field_count(self) -> usize {
        match self {
            BatchMode::Add => 1,
            _ => 2,
        }
    }
}

/// The open "Rename N Items" dialog.
struct BatchRename {
    pane: usize,
    targets: Vec<PathBuf>,
    mode: BatchMode,
    /// Find / Text / Name, depending on the mode.
    a: String,
    /// Replace with / Start at.
    b: String,
    /// Which text field has focus (0 = `a`, 1 = `b`).
    field: usize,
    /// Add mode: insert after the name (else before).
    after: bool,
}

impl BatchRename {
    fn field_mut(&mut self) -> &mut String {
        if self.field == 0 { &mut self.a } else { &mut self.b }
    }

    /// The rename each target would get: (from, to, problem). A problem
    /// (bad character, empty, or two items ending up with the same name /
    /// clobbering an item not being renamed) blocks applying.
    fn plan(&self) -> Vec<(PathBuf, PathBuf, Option<&'static str>)> {
        let start: u64 = self.b.trim().parse().unwrap_or(1);
        let mut out: Vec<(PathBuf, PathBuf, Option<&'static str>)> = self
            .targets
            .iter()
            .enumerate()
            .map(|(i, from)| {
                let name = from.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                // Folders and dotfiles have no extension to protect.
                let (stem, ext) = match name.rfind('.') {
                    Some(dot) if dot > 0 && !from.is_dir() => (name[..dot].to_string(), name[dot..].to_string()),
                    _ => (name.clone(), String::new()),
                };
                let new = match self.mode {
                    BatchMode::Replace if self.a.is_empty() => name.clone(),
                    BatchMode::Replace => format!("{}{ext}", stem.replace(&self.a, &self.b)),
                    BatchMode::Add if self.after => format!("{stem}{}{ext}", self.a),
                    BatchMode::Add => format!("{}{stem}{ext}", self.a),
                    BatchMode::Number if self.a.trim().is_empty() => name.clone(),
                    BatchMode::Number => format!("{} {}{ext}", self.a.trim(), start + i as u64),
                };
                let problem = if new.trim().is_empty() {
                    Some("Name can't be empty")
                } else if new.contains('/') || new.contains(':') {
                    Some("Names can’t contain “/” or “:”")
                } else {
                    None
                };
                (from.clone(), from.with_file_name(new), problem)
            })
            .collect();
        // Collisions: within the batch, or with an existing item that isn't
        // itself being renamed away.
        let mut seen: HashMap<String, usize> = HashMap::new();
        for (from, to, _) in &out {
            if from != to {
                *seen.entry(to.to_string_lossy().to_lowercase()).or_default() += 1;
            }
        }
        let renamed_away: HashSet<String> = out
            .iter()
            .filter(|(f, t, _)| f != t)
            .map(|(f, _, _)| f.to_string_lossy().to_lowercase())
            .collect();
        for (from, to, problem) in out.iter_mut() {
            if problem.is_some() || from == to {
                continue;
            }
            let key = to.to_string_lossy().to_lowercase();
            let case_only = key == from.to_string_lossy().to_lowercase();
            if seen.get(&key).copied().unwrap_or(0) > 1
                || (!case_only && to.exists() && !renamed_away.contains(&key))
            {
                *problem = Some("Name already taken");
            }
        }
        out
    }
}

/// `ls -l`-style permission string plus octal, e.g. "rwxr-xr-x (755)".
fn mode_string(mode: u32, is_dir: bool) -> String {
    let bits = ['r', 'w', 'x'];
    let mut s = String::from(if is_dir { "d" } else { "-" });
    for i in (0..9).rev() {
        s.push(if mode & (1 << i) != 0 { bits[(8 - i) % 3] } else { '-' });
    }
    format!("{s} ({:o})", mode & 0o777)
}

/// Total bytes and item count under a folder (symlinks not followed).
fn tree_size_count(p: &Path) -> (u64, u64) {
    let (mut bytes, mut items) = (0u64, 0u64);
    let mut stack = vec![p.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            items += 1;
            match e.file_type() {
                Ok(ft) if ft.is_dir() => stack.push(e.path()),
                Ok(ft) if ft.is_file() => bytes += e.metadata().map(|m| m.len()).unwrap_or(0),
                _ => {}
            }
        }
    }
    (bytes, items)
}

/// 1234567 → "1,234,567".
fn group_digits(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Make a scroller ignore the other axis's wheel delta (gpui maps a pure
/// x-swipe onto y — and vice versa — for single-axis scrollers by default).
fn restrict_scroll_axis<E: Styled>(mut e: E) -> E {
    e.style().restrict_scroll_to_axis = Some(true);
    e
}

/// Dimmed full-window backdrop for a modal card; clicking it runs `on_dismiss`.
fn modal_backdrop(on_dismiss: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static) -> gpui::Div {
    div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .bottom_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(rgba(0x00000066))
        .occlude()
        .on_mouse_down(MouseButton::Left, on_dismiss)
}

// ----- file operations: background copy/move with progress + undo ---------

#[derive(Clone, Copy, PartialEq)]
enum TransferKind {
    Copy,
    Move,
}

/// Live progress of one background copy/move, shared with its worker.
#[derive(Default)]
struct JobProgress {
    /// Bytes that need copying (0 while sizing, and for pure renames).
    total: AtomicU64,
    done: AtomicU64,
    cancel: AtomicBool,
}

/// A copy/move in flight, as the progress panel sees it.
struct FileJob {
    id: u64,
    label: String,
    started: Instant,
    prog: Arc<JobProgress>,
}

/// A reversible file operation. Paths are (original, where it is now).
enum UndoOp {
    Transfer { kind: TransferKind, pairs: Vec<(PathBuf, PathBuf)> },
    Renamed(Vec<(PathBuf, PathBuf)>),
    Trashed(Vec<(PathBuf, PathBuf)>),
    /// Items this app created (new folder/file, archive): undo trashes them.
    Created(Vec<PathBuf>),
}

impl UndoOp {
    fn label(&self) -> String {
        match self {
            UndoOp::Transfer { kind: TransferKind::Copy, pairs } => format!("Copy of {}", items_label(pairs.len())),
            UndoOp::Transfer { kind: TransferKind::Move, pairs } => format!("Move of {}", items_label(pairs.len())),
            UndoOp::Renamed(p) if p.len() == 1 => format!("Rename of “{}”", path_label(&p[0].1)),
            UndoOp::Renamed(p) => format!("Rename of {}", items_label(p.len())),
            UndoOp::Trashed(p) => format!("Move to Trash of {}", items_label(p.len())),
            UndoOp::Created(p) if p.len() == 1 => format!("New “{}”", path_label(&p[0])),
            UndoOp::Created(p) => format!("New {}", items_label(p.len())),
        }
    }
}

/// "1 item" / "3 items".
fn items_label(n: usize) -> String {
    if n == 1 { "1 item".to_string() } else { format!("{n} items") }
}

/// Copy or move each (src, dest) pair. Moves try a rename first (instant on
/// the same volume) and fall back to copy + delete across volumes. Copies use
/// `fs::copy`, which clones on APFS (instant, no extra space) and otherwise
/// copies data + xattrs. Never overwrites: a pair whose destination exists is
/// skipped. Stops after the current file when cancelled (the half-done item is
/// removed). Returns the pairs that completed, and the last error if any.
fn run_transfer(
    pairs: &[(PathBuf, PathBuf)],
    kind: TransferKind,
    prog: &JobProgress,
) -> (Vec<(PathBuf, PathBuf)>, Option<String>) {
    const EXDEV: i32 = 18;
    let mut done = Vec::new();
    let mut err = None;
    let mut to_copy = Vec::new();
    for (src, dest) in pairs {
        if fs::symlink_metadata(dest).is_ok() {
            err = Some(format!("“{}” already exists there", path_label(dest)));
            continue;
        }
        if kind == TransferKind::Move {
            match fs::rename(src, dest) {
                Ok(()) => {
                    done.push((src.clone(), dest.clone()));
                    continue;
                }
                Err(e) if e.raw_os_error() == Some(EXDEV) => {}
                Err(e) => {
                    err = Some(format!("Couldn't move “{}”: {e}", path_label(src)));
                    continue;
                }
            }
        }
        to_copy.push((src.clone(), dest.clone()));
    }
    prog.total.store(to_copy.iter().map(|(s, _)| tree_size(s)).sum(), Ordering::Relaxed);
    for (src, dest) in to_copy {
        match copy_tree(&src, &dest, prog) {
            Ok(()) => {
                if kind == TransferKind::Move {
                    let removed = if fs::symlink_metadata(&src).is_ok_and(|m| m.is_dir()) {
                        fs::remove_dir_all(&src)
                    } else {
                        fs::remove_file(&src)
                    };
                    if let Err(e) = removed {
                        err = Some(format!("Copied “{}” but couldn't remove the original: {e}", path_label(&src)));
                    }
                }
                done.push((src, dest));
            }
            Err(e) => {
                let _ = if fs::symlink_metadata(&dest).is_ok_and(|m| m.is_dir()) {
                    fs::remove_dir_all(&dest)
                } else {
                    fs::remove_file(&dest)
                };
                if prog.cancel.load(Ordering::Relaxed) {
                    break;
                }
                err = Some(format!("Couldn't copy “{}”: {e}", path_label(&src)));
            }
        }
    }
    (done, err)
}

/// Total bytes of regular files under `p` (symlinks not followed).
fn tree_size(p: &Path) -> u64 {
    match fs::symlink_metadata(p) {
        Ok(m) if m.is_dir() => fs::read_dir(p)
            .map(|rd| rd.flatten().map(|e| tree_size(&e.path())).sum())
            .unwrap_or(0),
        Ok(m) if m.is_file() => m.len(),
        _ => 0,
    }
}

/// Recursively copy `src` to `dest` (which must not exist), keeping symlinks
/// as symlinks, permissions, and file modification dates.
fn copy_tree(src: &Path, dest: &Path, prog: &JobProgress) -> std::io::Result<()> {
    if prog.cancel.load(Ordering::Relaxed) {
        return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "cancelled"));
    }
    let md = fs::symlink_metadata(src)?;
    if md.file_type().is_symlink() {
        std::os::unix::fs::symlink(fs::read_link(src)?, dest)?;
    } else if md.is_dir() {
        fs::create_dir(dest)?;
        for e in fs::read_dir(src)? {
            let e = e?;
            copy_tree(&e.path(), &dest.join(e.file_name()), prog)?;
        }
        fs::set_permissions(dest, md.permissions())?;
    } else {
        fs::copy(src, dest)?;
        if let (Ok(f), Ok(t)) = (fs::File::options().write(true).open(dest), md.modified()) {
            let _ = f.set_modified(t);
        }
        prog.done.fetch_add(md.len(), Ordering::Relaxed);
    }
    Ok(())
}

/// Move `path` to the Trash; returns where it landed there (for undo).
fn trash_path(path: &Path) -> Option<PathBuf> {
    let ns_path = NSString::from_str(&path.to_string_lossy());
    let url = NSURL::fileURLWithPath(&ns_path);
    let fm = NSFileManager::defaultManager();
    let mut out = None;
    fm.trashItemAtURL_resultingItemURL_error(&url, Some(&mut out)).ok()?;
    Some(out.and_then(|u| u.path()).map(|p| PathBuf::from(p.to_string())).unwrap_or_default())
}

fn nav_item(
    label: String,
    tooltip: String,
    icon: AnyElement,
    key: usize,
    active: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    on_right: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let t = theme();
    let base = div()
        .id(("nav", key))
        .flex()
        .items_center()
        .rounded_md()
        .cursor_pointer()
        .text_color(rgb(if active { t.text } else { t.text_muted }))
        .mx_2().px_3().py_1p5().gap_2();
    let base = if active {
        base.bg(rgb(t.selected))
    } else {
        base.hover(|s| s.bg(rgb(t.hover)))
            .active(|s| s.bg(rgb(t.selected)))
    };
    base.child(icon)
        .child(div().min_w_0().overflow_hidden().child(label))
        .tooltip(tip(tooltip))
        .on_click(on_click)
        .on_mouse_down(MouseButton::Right, on_right)
}

/// A "key chip + label" pair for the palette's footer hints ("↩ Open file").
fn palette_hint(key: &'static str, label: &'static str) -> impl IntoElement {
    let t = theme();
    div()
        .flex()
        .items_center()
        .gap_1()
        .child(
            div()
                .px_1()
                .rounded_sm()
                .bg(Theme::alpha(t.text_dim, 0x22))
                .text_color(rgb(t.text_muted))
                .child(key),
        )
        .child(label)
}

fn empty_hint(text: &str) -> impl IntoElement {
    div()
        .px_3()
        .py_1()
        .text_color(rgb(theme().text_dim))
        .child(text.to_string())
}

/// One clickable listing row in the main pane: icon · name · kind · date · size.
#[allow(clippy::too_many_arguments)]
fn file_row(
    entry: &Entry,
    location: &Path,
    key: usize,
    selected: bool,
    // True while this row is the open right-click menu's target — keep it looking
    // hovered even though the cursor is over the menu backdrop.
    ctx_active: bool,
    widths: &ListLayout,
    icon: AnyElement,
    // Drag-and-drop: `accept_drop` true => it's a drop target (a folder or the
    // ".." row) that runs `on_drop_file` when files are dropped on it.
    accept_drop: bool,
    // True while the tracked drag target is this row's folder: highlights the
    // name cell (driven by Shuffle::drop_hover, not gpui's drag hover).
    drop_hilite: bool,
    // When Some, this row is being renamed: the name shows as an editable field
    // with (text, cursor char-index, selection-anchor char-index, invalid).
    // Invalid names render the field in red (the pane shows the reason).
    rename_text: Option<(String, usize, Option<usize>, bool)>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    on_right: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    on_drop_file: impl Fn(&ExternalPaths, &mut Window, &mut App) + 'static,
    // Left-press handler: records a drag candidate + stops marquee propagation.
    on_press: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    // Hover tracking, so Enter can rename the row under the mouse.
    on_hover_row: impl Fn(&bool, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let t = theme();
    let name = &entry.name;
    // The name element: an editable field while renaming, else the label.
    let name_el: AnyElement = match &rename_text {
        Some((txt, cursor, anchor, invalid)) => {
            let edge = if *invalid { RENAME_ERR_COLOR } else { t.accent };
            let field = div()
                .flex_1()
                .min_w_0()
                .px_1()
                .rounded_sm()
                .bg(rgb(t.bg))
                .border_1()
                .border_color(rgb(edge))
                .text_color(rgb(if *invalid { RENAME_ERR_COLOR } else { t.text }))
                .flex()
                .items_center();
            let n = txt.chars().count();
            let cursor = (*cursor).min(n);
            let sel = anchor
                .filter(|&a| a != cursor)
                .map(|a| (a.min(cursor), a.max(cursor)));
            if let Some((lo, hi)) = sel {
                // Highlighted selection between the plain head and tail.
                let (bl, bh) = (char_byte(txt, lo), char_byte(txt, hi));
                field
                    .child(div().flex_none().child(txt[..bl].to_string()))
                    .child(
                        div()
                            .flex_none()
                            .bg(Theme::alpha(edge, 0x66))
                            .rounded_sm()
                            .child(txt[bl..bh].to_string()),
                    )
                    .child(div().flex_none().child(txt[bh..].to_string()))
                    .into_any_element()
            } else {
                // Static caret at the cursor position.
                let b = char_byte(txt, cursor);
                field
                    .child(div().flex_none().child(txt[..b].to_string()))
                    .child(div().flex_none().w(px(1.5)).h(px(14.0)).bg(rgb(t.text)))
                    .child(div().flex_none().child(txt[b..].to_string()))
                    .into_any_element()
            }
        }
        None => div()
            .flex_1()
            .min_w_0()
            .truncate()
            .text_color(rgb(t.text))
            .child(name.to_string())
            .into_any_element(),
    };

    let name_cell = div()
        .flex_none().w(px(widths.width(Column::Name))).flex().items_center().gap_2().pr_2()
        .on_mouse_down(MouseButton::Left, on_press)
        .when(accept_drop, |c| c.rounded_sm().on_drop(on_drop_file))
        .when(drop_hilite, |c| c.bg(rgb(theme().selected)))
        .child(div().flex_none().w(px(ICON_W)).flex().justify_center().child(icon))
        .child(name_el).into_any_element();
    let mut name_cell = Some(name_cell);
    let cells: Vec<AnyElement> = widths.columns.iter().map(|spec| {
        if spec.column == Column::Name { return name_cell.take().unwrap(); }
        div().flex_none().w(px(spec.width)).pr_3().min_w_0().truncate()
            .text_color(rgb(t.text_muted))
            .when(spec.column == Column::Size, |d| d.text_right())
            .child(list_ui::column_text(entry, spec.column, location)).into_any_element()
    }).collect();
    div().id(("row", key)).flex().items_center().px_3().h(px(ROW_H)).cursor_pointer()
        .when(selected, |s| s.bg(rgb(t.selected)))
        .when(ctx_active && !selected, |s| s.bg(rgb(t.hover)))
        .hover(|s| s.bg(rgb(t.hover))).active(|s| s.bg(rgb(t.selected)))
        .on_hover(on_hover_row).children(cells).child(div().flex_1())
        .on_click(on_click)
        .on_mouse_down(MouseButton::Right, on_right)
}

/// One cell in the Icons view: a large icon above a (truncated) name, with the
/// same click / drag / drop / context-menu behavior as a list row.
fn icon_cell(
    pane: usize,
    name: String,
    target: PathBuf,
    is_dir: bool,
    selected: bool,
    ctx_active: bool,
    cell_w: f32,
    cx: &Context<Shuffle>,
) -> AnyElement {
    let t = theme();
    let is_folder = is_browsable_folder(&target, is_dir);
    let press_t = target.clone();
    let drop_t = target.clone();
    let ctx_t = target.clone();
    let click_t = target.clone();
    let hover_t = target.clone();
    let mut cell = div()
        .id(SharedString::from(format!("cell:{}", target.to_string_lossy())))
        .flex_none()
        .w(px(cell_w))
        .flex()
        .flex_col()
        .items_center()
        .gap_1()
        .p_2()
        .rounded_md()
        .cursor_pointer()
        .when(selected, |s| s.bg(rgb(t.selected)))
        .when(ctx_active && !selected, |s| s.bg(rgb(t.hover)))
        .hover(|s| s.bg(rgb(t.hover)))
        // Instant press feedback (the click action lands on mouse-up).
        .active(|s| s.bg(rgb(t.selected)))
        // Track the hovered item so Enter renames the cell under the mouse.
        .on_hover(cx.listener(move |this, on: &bool, _, _| {
            if *on {
                this.hovered = Some((pane, hover_t.clone()));
            } else if this
                .hovered
                .as_ref()
                .is_some_and(|(hp, p)| *hp == pane && *p == hover_t)
            {
                this.hovered = None;
            }
        }))
        // Press records a drag candidate; moving past the threshold starts a
        // native OS drag (drop into Finder / other apps or back into Shuffle).
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                let (x, y) = (
                    f64::from(ev.position.x) as f32,
                    f64::from(ev.position.y) as f32,
                );
                this.drag_candidate = Some((pane, press_t.clone(), (x, y)));
                cx.stop_propagation();
            }),
        )
        .child(
            div()
                .h(px(56.0))
                .flex()
                .items_center()
                .child(icon_element_sized(&target, is_dir, 52.0)),
        )
        .child(
            div()
                .w_full()
                .truncate()
                .text_size(px(UI_SMALL_FONT_SIZE))
                .text_color(rgb(t.text))
                .child(name),
        )
        .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
            this.click_entry(pane, click_t.clone(), is_dir, ev, window, cx);
        }))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                let (x, y) = (
                    f64::from(ev.position.x) as f32,
                    f64::from(ev.position.y) as f32,
                );
                this.open_context_menu(pane, x, y, Some((ctx_t.clone(), is_dir)), cx);
                cx.stop_propagation();
            }),
        );
    if is_folder {
        cell = cell
            .drag_over::<ExternalPaths>(|s, _, _, _| s.bg(rgb(theme().selected)))
            .on_drop(cx.listener(move |this, d: &ExternalPaths, _, cx| {
                this.drop_files(pane, drop_t.clone(), d.paths().to_vec(), cx);
            }));
    }
    cell.into_any_element()
}

/// A human-readable kind label for a file (e.g. "Microsoft Excel", "DWG File").
fn kind_label(name: &str, is_dir: bool) -> String {
    if is_app_bundle(Path::new(name), is_dir) {
        return "Application".to_string();
    }
    if is_dir {
        return "Directory".to_string();
    }
    let ext = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    match ext.as_deref() {
        Some("xlsx" | "xls" | "xlsm" | "xlsb") => "Microsoft Excel".to_string(),
        Some("docx" | "doc") => "Microsoft Word".to_string(),
        Some("pptx" | "ppt") => "Microsoft PowerPoint".to_string(),
        Some("pdf") => "PDF Document".to_string(),
        Some("txt" | "md" | "rtf" | "log") => "Text Document".to_string(),
        Some("csv" | "tsv") => "CSV File".to_string(),
        Some("dwg") => "DWG File".to_string(),
        Some("dxf") => "DXF File".to_string(),
        // Images/video/audio/archives show their format, e.g. "PNG Image".
        Some("jpg" | "jpeg") => "JPEG Image".to_string(),
        Some(e @ ("png" | "gif" | "bmp" | "tiff" | "heic" | "webp" | "svg")) => {
            format!("{} Image", e.to_uppercase())
        }
        Some(e @ ("mp4" | "mov" | "avi" | "mkv" | "webm" | "m4v")) => {
            format!("{} Video", e.to_uppercase())
        }
        Some(e @ ("mp3" | "wav" | "flac" | "aac" | "m4a" | "ogg")) => {
            format!("{} Audio", e.to_uppercase())
        }
        Some(e @ ("zip" | "tar" | "gz" | "7z" | "rar" | "bz2" | "xz" | "dmg")) => {
            format!("{} Archive", e.to_uppercase())
        }
        Some(
            "rs" | "py" | "js" | "ts" | "tsx" | "jsx" | "c" | "cpp" | "h" | "hpp" | "go" | "java"
            | "rb" | "swift" | "zig" | "sh" | "json" | "toml" | "yaml" | "yml" | "html" | "css",
        ) => "Source Code".to_string(),
        Some("app") => "Application".to_string(),
        Some(other) => format!("{} File", other.to_uppercase()),
        None => "Document".to_string(),
    }
}

/// Build the icon element for an entry: a real macOS file-type icon when we can
/// fetch one, otherwise a generic fallback. Applications use their own icon;
/// ordinary folders share the generic folder icon.
fn icon_element(path: &Path, is_dir: bool) -> AnyElement {
    icon_element_sized(path, is_dir, 16.0)
}

/// Wrap a file icon with a small corner badge showing its cloud-sync state:
/// a cloud for online-only files, a rotating arrow while downloading. Local
/// (fully-present) files get no badge. `badge` is the badge diameter in px.
fn cloud_badged_icon(icon: AnyElement, cloud: CloudSync, badge: f32) -> AnyElement {
    if cloud == CloudSync::Local {
        return icon;
    }
    let t = theme();
    // A small corner chip. `☁`/`↻` aren't in the UI font (they render blank),
    // so use a solid colored disc with a glyph the app already draws: `▾`
    // (points down = "download this") in blue for online-only, amber while a
    // download/evict is in flight. A ring in the surface color separates it.
    let (bg, glyph) = match cloud {
        CloudSync::OnlineOnly => (0x3b82f6u32, "\u{25BE}"), // blue ▾ — online-only
        CloudSync::Syncing => (0xf59e0bu32, "\u{25BE}"),    // amber ▾ — working
        CloudSync::Local => unreachable!(),
    };
    div()
        .relative()
        .flex()
        .child(icon)
        .child(
            div()
                .absolute()
                .bottom(px(-3.0))
                .right(px(-4.0))
                .w(px(badge))
                .h(px(badge))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(rgb(bg))
                .border_1()
                .border_color(rgb(t.bg))
                .text_size(px(badge * 0.64))
                .text_color(rgb(0xffffff))
                .child(glyph),
        )
        .into_any_element()
}

/// Like [`icon_element`] but at an explicit pixel size (for the icon/gallery views).
fn icon_element_sized(path: &Path, is_dir: bool, size: f32) -> AnyElement {
    if !is_browsable_folder(path,is_dir) {
        if let Some(key)=icon_key(path) {
            if !ICON_CACHE.with(|c|c.borrow().contains_key(&key)) && !ICON_PENDING.with(|p|p.borrow().contains_key(&key)) {
                let epoch=ICON_EPOCH.with(|e|e.get());
                ICON_PENDING.with(|p|{p.borrow_mut().insert(key.clone(),epoch);});
                ICON_REQUESTS.with(|q|{q.borrow_mut().insert(key,(path.to_path_buf(),epoch));});
            }
        }
    }
    // Cache-only lookup — never build on the render thread. Directories use the
    // shared generic folder icon (built synchronously at startup, so no emoji
    // placeholder). Files use their type-specific icon once the background
    // pre-warm has it, otherwise the shared generic file icon.
    let is_folder = is_browsable_folder(path, is_dir);
    let handle = if is_folder {
        lookup_cached(FOLDER_KEY)
    } else {
        lookup_icon(path).or_else(|| lookup_cached(FILE_KEY))
    };
    if let Some(handle) = handle {
        return img(ImageSource::Render(handle))
            .w(px(size))
            .h(px(size))
            .into_any_element();
    }
    // Last resort only if the base icons couldn't be built.
    div()
        .child(if is_folder { "📁" } else { "📄" })
        .into_any_element()
}

thread_local! {
    // Documents share an extension icon; apps use a separate key for each path.
    // `None` records "couldn't build one" to avoid retrying.
    static ICON_CACHE: RefCell<Cache<String, Option<Arc<RenderImage>>>> =
        RefCell::new(Cache::new(512, 32*1024*1024, image_bytes, |k| k==FOLDER_KEY || k==FILE_KEY || k.starts_with("fav:")));
    static ICON_REQUESTS: RefCell<HashMap<String, (PathBuf,u64)>> = RefCell::new(HashMap::new());
    static ICON_PENDING: RefCell<HashMap<String,u64>> = RefCell::new(HashMap::new());
    static ICON_EPOCH: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

fn icon_key(path: &Path) -> Option<String> {
    if has_app_extension(path) {
        return Some(format!("{APP_ICON_PREFIX}{}", path.to_string_lossy()));
    }
    let key = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())?;
    if key.is_empty() {
        None
    } else {
        Some(key)
    }
}

/// Read a previously-built icon from the cache by its key. Never builds.
fn image_bytes(image:&Option<Arc<RenderImage>>)->usize {
    image.as_ref().map_or(1,|image|(0..image.frame_count()).map(|i|image.as_bytes(i).map_or(0,<[u8]>::len)).sum())
}

fn lookup_cached(key: &str) -> Option<Arc<RenderImage>> {
    ICON_CACHE.with(|cache| cache.borrow().get(key).cloned().flatten())
}

/// Read a previously-built icon (keyed by app path or document extension). Never
/// builds, so it's safe to call every frame from `render`.
fn lookup_icon(path: &Path) -> Option<Arc<RenderImage>> {
    let key = icon_key(path)?;
    lookup_cached(&key)
}

/// A guaranteed-plain folder whose icon is the generic macOS folder icon (our
/// own config dir — we create it, so it never has a custom icon).
fn folder_dir_path() -> PathBuf {
    let dir = config_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    let _ = fs::create_dir_all(&dir);
    dir
}

/// A guaranteed-plain, extensionless file whose icon is the generic macOS
/// document icon.
fn file_probe_path() -> PathBuf {
    let probe = folder_dir_path().join("icon_probe");
    if !probe.exists() {
        let _ = fs::write(&probe, b"");
    }
    probe
}

/// Build the generic folder + file icons synchronously (a few ms, once) so the
/// very first render shows real Finder icons — no emoji placeholder / swap.
fn ensure_base_icons() {
    if ICON_CACHE.with(|c| !c.borrow().contains_key(FOLDER_KEY)) {
        let icon = pack_icon_path(FOLDER_KEY)
            .and_then(|p| decode_image_file(&p))
            .or_else(|| build_macos_icon(&folder_dir_path()));
        ICON_CACHE.with(|c| {
            c.borrow_mut().insert(FOLDER_KEY.to_string(), icon);
        });
    }
    if ICON_CACHE.with(|c| !c.borrow().contains_key(FILE_KEY)) {
        let icon = pack_icon_path(FILE_KEY)
            .and_then(|p| decode_image_file(&p))
            .or_else(|| build_macos_icon(&file_probe_path()));
        ICON_CACHE.with(|c| {
            c.borrow_mut().insert(FILE_KEY.to_string(), icon);
        });
    }
}

thread_local! {
    /// Render timestamps from the last second, for the FPS meter.
    static FRAME_TIMES: RefCell<std::collections::VecDeque<Instant>> =
        RefCell::new(std::collections::VecDeque::new());
}

/// Record this render and return how many happened in the last second.
fn fps_sample() -> usize {
    let now = Instant::now();
    FRAME_TIMES.with(|q| {
        let mut q = q.borrow_mut();
        q.push_back(now);
        while q.front().is_some_and(|t| now.duration_since(*t) > Duration::from_secs(1)) {
            q.pop_front();
        }
        q.len()
    })
}

/// The live FPS readout (Settings → "Frame rate meter"). The repeating no-op
/// animation forces a redraw every vsync, so the number reflects the real
/// achievable frame rate rather than going stale between repaints.
fn fps_overlay() -> AnyElement {
    let fps = fps_sample();
    let color = if fps >= 55 {
        rgb(0x4ade80) // green: smooth
    } else if fps >= 30 {
        rgb(0xfacc15) // yellow: noticeable
    } else {
        rgb(0xf87171) // red: janky
    };
    div()
        .absolute()
        .top(px(TITLEBAR_H + 8.0))
        .right(px(12.0))
        .px_2()
        .py_1()
        .rounded_md()
        .bg(rgba(0x000000aa))
        .text_size(px(UI_SMALL_FONT_SIZE))
        .text_color(color)
        .child(format!("{fps} fps"))
        .with_animation(
            "fps-tick",
            Animation::new(Duration::from_millis(100)).repeat(),
            |el, _| el,
        )
        .into_any_element()
}

/// List a folder's visible subdirectories (sorted case-insensitively) plus its
/// current mtime. Used by the waterfall sidebar tree; runs off the main thread
/// (may block on I/O). The mtime lets the watcher detect outside changes.
fn read_subdirs(dir: &Path) -> (Vec<PathBuf>, Option<SystemTime>) {
    let mut out = Vec::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            let is_dir = match e.file_type() {
                Ok(ft) if ft.is_dir() => true,
                Ok(ft) if ft.is_symlink() => p.is_dir(),
                _ => false,
            };
            let hidden = p
                .file_name()
                .map(|n| n.to_string_lossy().starts_with('.'))
                .unwrap_or(false);
            if is_dir && !hidden {
                out.push(p);
            }
        }
    }
    out.sort_by_cached_key(|p| {
        p.file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    });
    let mtime = fs::metadata(dir).ok().and_then(|m| m.modified().ok());
    (out, mtime)
}

/// `stat` results for sidebar locations, tags and group members,
/// cached so `render` never touches the filesystem per frame — a single dead
/// network shortcut would otherwise freeze every repaint. Stale entries are
/// re-checked on a background thread.
static DIR_STAT: OnceLock<Mutex<HashMap<PathBuf, (bool, Instant)>>> = OnceLock::new();
static DIR_STAT_SCANNING: AtomicBool = AtomicBool::new(false);

fn cached_is_dir(path: &Path) -> bool {
    const TTL: Duration = Duration::from_secs(3);
    let map = DIR_STAT.get_or_init(|| Mutex::new(HashMap::new()));
    let hit = map.lock().unwrap().get(path).copied();
    match hit {
        Some((v, at)) => {
            if at.elapsed() > TTL && !DIR_STAT_SCANNING.swap(true, Ordering::SeqCst) {
                std::thread::spawn(|| {
                    let map = DIR_STAT.get().unwrap();
                    let stale: Vec<PathBuf> = {
                        let m = map.lock().unwrap();
                        m.iter()
                            .filter(|(_, (_, at))| at.elapsed() > TTL)
                            .map(|(p, _)| p.clone())
                            .collect()
                    };
                    for p in stale {
                        let v = p.is_dir(); // may block; we're off-thread
                        map.lock().unwrap().insert(p, (v, Instant::now()));
                    }
                    DIR_STAT_SCANNING.store(false, Ordering::SeqCst);
                });
            }
            v
        }
        None => {
            // First sighting (startup / newly added): one synchronous stat so
            // the answer is correct immediately.
            let v = path.is_dir();
            map.lock().unwrap().insert(path.to_path_buf(), (v, Instant::now()));
            v
        }
    }
}

/// Turn a raw CloudStorage folder name ("GoogleDrive-me@x.com", "OneDrive-Personal")
/// into a friendly label ("Google Drive", "OneDrive").
fn pretty_cloud_name(raw: &str) -> String {
    let base = raw.split('-').next().unwrap_or(raw).trim();
    match base {
        "GoogleDrive" => "Google Drive".to_string(),
        "" => raw.to_string(),
        other => other.to_string(),
    }
}

// ----- SFTP servers (browse remote hosts over SSH) ---------------------------

/// A saved SFTP server. Auth is delegated to the system `ssh`/`sftp`, so a
/// server is mostly a host reference; `key` is only used in "configure in-app"
/// mode (otherwise the user's `~/.ssh` config/keys/agent handle auth).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct SftpServer {
    /// Live terminal transport; never persisted as a saved project connection.
    #[serde(skip)]
    control_path: Option<PathBuf>,
    /// Display label in the sidebar.
    name: String,
    /// Hostname or a `~/.ssh/config` alias.
    host: String,
    /// Login user (empty = from ssh config / current user).
    user: String,
    /// Port (0 = default / from ssh config).
    port: u16,
    /// Explicit private-key path (in-app auth mode only; empty otherwise).
    key: String,
    /// True when this server authenticates with a password (stored in the macOS
    /// Keychain, fetched at connect time via an askpass helper). False = use the
    /// user's ~/.ssh keys/agent (or the explicit `key`).
    use_password: bool,
    /// Reconnect and reopen this server automatically on launch.
    auto_reopen: bool,
}

impl SftpServer {
    /// The `[user@]host` target passed to ssh/sftp.
    fn target(&self) -> String {
        if self.user.trim().is_empty() {
            self.host.trim().to_string()
        } else {
            format!("{}@{}", self.user.trim(), self.host.trim())
        }
    }

    /// A short display of the connection ("user@host:port").
    fn display(&self) -> String {
        let mut s = self.target();
        if self.port != 0 {
            s.push_str(&format!(":{}", self.port));
        }
        s
    }
}

/// Which tab of the Connect-to-Server dialog is showing.
#[derive(Clone, Copy, PartialEq)]
enum ServerMode {
    /// A single address (smb://, afp://, ftp://, or sftp://[user@]host).
    Quick,
    /// SFTP with an explicit username + password (stored in the Keychain).
    Credentials,
}

/// A focused field in the Credentials tab.
#[derive(Clone, Copy, PartialEq)]
enum CredField {
    Name,
    Host,
    User,
    Port,
    Password,
}

/// State of the Connect-to-Server dialog.
#[derive(Clone)]
struct ServerForm {
    mode: ServerMode,
    /// Quick tab: the address being typed.
    addr: String,
    /// Credentials tab fields.
    name: String,
    host: String,
    user: String,
    port: String,
    password: String,
    field: CredField,
    /// "Reconnect on launch" toggle for the server being added.
    auto_reopen: bool,
    /// When editing an existing server, its original `display()` (so submit
    /// replaces it instead of adding a duplicate). `None` = adding a new one.
    editing: Option<String>,
}

impl Default for ServerForm {
    fn default() -> Self {
        ServerForm {
            mode: ServerMode::Quick,
            addr: String::new(),
            name: String::new(),
            host: String::new(),
            user: String::new(),
            port: String::new(),
            password: String::new(),
            field: CredField::Host,
            auto_reopen: false,
            editing: None,
        }
    }
}

impl ServerForm {
    /// Pre-fill the dialog from an existing server, for editing. Password
    /// servers open the Credentials tab; key/agent servers open Quick.
    fn editing_server(s: &SftpServer) -> Self {
        if s.use_password {
            ServerForm {
                mode: ServerMode::Credentials,
                name: s.name.clone(),
                host: s.host.clone(),
                user: s.user.clone(),
                port: if s.port == 0 { String::new() } else { s.port.to_string() },
                auto_reopen: s.auto_reopen,
                editing: Some(s.display()),
                field: CredField::Host,
                ..ServerForm::default()
            }
        } else {
            ServerForm {
                mode: ServerMode::Quick,
                addr: format!("sftp://{}", s.display()),
                auto_reopen: s.auto_reopen,
                editing: Some(s.display()),
                ..ServerForm::default()
            }
        }
    }
}

impl ServerForm {
    /// The credentials field currently focused, as a mutable string.
    fn field_mut(&mut self) -> &mut String {
        match self.field {
            CredField::Name => &mut self.name,
            CredField::Host => &mut self.host,
            CredField::User => &mut self.user,
            CredField::Port => &mut self.port,
            CredField::Password => &mut self.password,
        }
    }

    /// The string being edited: the Quick address, or the focused credentials
    /// field.
    fn active_field(&mut self) -> &mut String {
        match self.mode {
            ServerMode::Quick => &mut self.addr,
            ServerMode::Credentials => self.field_mut(),
        }
    }

    /// Advance focus to the next credentials field (Tab).
    fn next_field(&mut self) {
        self.field = match self.field {
            CredField::Name => CredField::Host,
            CredField::Host => CredField::User,
            CredField::User => CredField::Port,
            CredField::Port => CredField::Password,
            CredField::Password => CredField::Name,
        };
    }
}

/// The Keychain account name for a server's password.
fn keychain_account(s: &SftpServer) -> String {
    format!("sftp:{}", s.display())
}

/// Store a server's password in the login Keychain (updating any existing one).
fn keychain_set_password(s: &SftpServer, pw: &str) {
    let _ = Command::new("security")
        .args(["add-generic-password", "-U", "-s", "shuffle-studio-sftp", "-a"])
        .arg(keychain_account(s))
        .arg("-w")
        .arg(pw)
        .output();
}

/// Remove a server's stored password.
fn keychain_get_password(s: &SftpServer) -> Option<String> {
    let out = Command::new("security")
        .args(["find-generic-password", "-w", "-s", "shuffle-studio-sftp", "-a"])
        .arg(keychain_account(s))
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let pw = String::from_utf8_lossy(&out.stdout);
    let pw = pw.trim_end_matches(['\n', '\r']);
    (!pw.is_empty()).then(|| pw.to_string())
}

/// The `expect` script that establishes a password SSH connection: it drives
/// `sftp`, types the password at the prompt (password AND keyboard-interactive
/// servers both use this prompt), and leaves a persistent ControlMaster socket
/// that later operations reuse without re-authenticating. Created once.
/// Args: <control-socket> <user@host> <port>. Env: SHUFFLE_SFTP_PW.
fn ensure_connect_expect() -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let path = config_dir()?.join("sftp-connect.exp");
    let body = r#"#!/usr/bin/expect -f
set timeout 25
set sock [lindex $argv 0]
set target [lindex $argv 1]
set port [lindex $argv 2]
set pw $env(SHUFFLE_SFTP_PW)
spawn sftp -o ControlMaster=yes -o ControlPath=$sock -o ControlPersist=180 \
  -o ConnectTimeout=12 -o StrictHostKeyChecking=accept-new \
  -o PubkeyAuthentication=no -o NumberOfPasswordPrompts=1 -P $port $target
expect {
  -re "(?i)permission denied" { exit 11 }
  -re "(?i)could not resolve|name or service not known" { exit 12 }
  -re "(?i)assword:" { send -- "$pw\r"; exp_continue }
  "sftp>" { send -- "quit\r"; exit 0 }
  timeout { exit 13 }
  eof { exit 14 }
}
"#;
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(&path, body).ok()?;
    if let Ok(md) = fs::metadata(&path) {
        let mut perm = md.permissions();
        perm.set_mode(0o755);
        let _ = fs::set_permissions(&path, perm);
    }
    Some(path)
}

/// Is the multiplexed master connection for a password server alive?
fn password_master_alive(s: &SftpServer) -> bool {
    Command::new("ssh")
        .arg("-o")
        .arg(format!("ControlPath={}", ssh_control_path(s).to_string_lossy()))
        .arg("-O")
        .arg("check")
        .arg(s.target())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Ensure a password server has a live authenticated master connection,
/// establishing it via `expect` (typing the Keychain password) if needed.
/// No-op for key/agent servers.
fn ensure_master(server: &SftpServer) -> Result<(), String> {
    if !server.use_password {
        return Ok(());
    }
    if password_master_alive(server) {
        return Ok(());
    }
    let pw = keychain_get_password(server)
        .ok_or_else(|| "No saved password — edit the server to set it.".to_string())?;
    let script = ensure_connect_expect().ok_or_else(|| "helper unavailable".to_string())?;
    let port = if server.port == 0 { 22 } else { server.port };
    let out = Command::new("expect")
        .arg(script)
        .arg(ssh_control_path(server))
        .arg(server.target())
        .arg(port.to_string())
        .env("SHUFFLE_SFTP_PW", pw)
        .output()
        .map_err(|e| format!("couldn't launch expect: {e}"))?;
    match out.status.code() {
        Some(0) => Ok(()),
        Some(11) => Err("Authentication failed — check the username and password.".into()),
        Some(12) => Err("Host not found.".into()),
        Some(13) => Err("Connection timed out.".into()),
        _ => Err("Couldn't connect to the server.".into()),
    }
}

/// Parse `[user@]host[:port]` (the part after `sftp://`) into a server. The
/// display name defaults to the host/alias.
fn parse_sftp_url(s: &str) -> Option<SftpServer> {
    // Drop any trailing path.
    let s = s.split('/').next().unwrap_or(s).trim();
    if s.is_empty() {
        return None;
    }
    let (user, hostport) = match s.split_once('@') {
        Some((u, h)) => (u.to_string(), h),
        None => (String::new(), s),
    };
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) if p.chars().all(|c| c.is_ascii_digit()) && !p.is_empty() => {
            (h.to_string(), p.parse().unwrap_or(0))
        }
        _ => (hostport.to_string(), 0u16),
    };
    if host.is_empty() {
        return None;
    }
    Some(SftpServer {
        control_path: None,
        name: host.clone(),
        host,
        user,
        port,
        key: String::new(),
        use_password: false,
        auto_reopen: false,
    })
}

#[derive(Clone)]
struct SftpServersGlobal(Vec<SftpServer>);
impl gpui::Global for SftpServersGlobal {}

thread_local! {
    static ACTIVE_SERVERS: RefCell<Vec<SftpServer>> = const { RefCell::new(Vec::new()) };
}

/// The saved SFTP servers (read this in render code).
fn sftp_servers() -> Vec<SftpServer> {
    ACTIVE_SERVERS.with(|s| s.borrow().clone())
}

fn set_active_sftp_servers(list: Vec<SftpServer>) {
    ACTIVE_SERVERS.with(|c| *c.borrow_mut() = list);
}

/// Persist + broadcast a new server list to every window.
fn apply_sftp_servers(list: Vec<SftpServer>, cx: &mut App) {
    set_active_sftp_servers(list.clone());
    cx.set_global(SftpServersGlobal(list));
    cx.refresh_windows();
}

/// A per-server socket path for SSH connection multiplexing (ControlMaster), so
/// repeated listings/transfers reuse one authenticated connection.
fn ssh_control_path(s: &SftpServer) -> PathBuf {
    if let Some(path) = &s.control_path { return path.clone(); }
    // Unix socket paths are capped near 104 bytes, so we can't use macOS's long
    // per-user temp dir (/var/folders/…/T). A short, stable /tmp name keyed by a
    // hash of the target stays well under the limit.
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.display().hash(&mut h);
    PathBuf::from(format!("/tmp/shuffle-studio-ssh-{:016x}.sock", h.finish()))
}

/// Common `ssh`/`sftp` options for a server: multiplexing, non-interactive
/// (fail fast instead of hanging on a password prompt), trust-on-first-use host
/// keys, and an explicit key when in "configure in-app" mode.
fn ssh_options(s: &SftpServer, use_system: bool) -> Vec<String> {
    let cp = ssh_control_path(s);
    if s.control_path.is_some() {
        // Reuse only the authenticated live terminal transport. If it vanishes,
        // fail closed instead of reconnecting with different keys or prompts.
        return vec!["-o".into(), format!("ControlPath={}", cp.display()),
            "-o".into(), "ControlMaster=no".into(), "-o".into(), "BatchMode=yes".into(),
            "-o".into(), "ProxyCommand=false".into(), "-o".into(), "StrictHostKeyChecking=yes".into()];
    }
    let mut o = vec![
        "-o".into(),
        format!("ControlPath={}", cp.to_string_lossy()),
        "-o".into(),
        "ConnectTimeout=12".into(),
        "-o".into(),
        "StrictHostKeyChecking=accept-new".into(),
    ];
    if s.use_password {
        // Password auth: reuse the authenticated master that `ensure_master`
        // established via expect; never prompt (fail fast if it's gone).
        o.push("-o".into());
        o.push("ControlMaster=no".into());
        o.push("-o".into());
        o.push("BatchMode=yes".into());
    } else {
        // Key/agent auth: multiplex directly, fail fast instead of prompting.
        o.push("-o".into());
        o.push("ControlMaster=auto".into());
        o.push("-o".into());
        o.push("ControlPersist=120".into());
        o.push("-o".into());
        o.push("BatchMode=yes".into());
        if !use_system && !s.key.trim().is_empty() {
            o.push("-o".into());
            o.push("IdentitiesOnly=yes".into());
            o.push("-i".into());
            o.push(s.key.trim().to_string());
        }
    }
    o
}

/// Single-quote a path for a remote POSIX shell command.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Run a shell command over ssh on `server` (reusing the multiplexed
/// connection). Used for operations sftp lacks (recursive delete, touch).
fn ssh_exec(server: &SftpServer, remote_cmd: &str, use_system: bool) -> Result<String, String> {
    ensure_master(server)?;
    let mut cmd = Command::new("ssh");
    for opt in ssh_options(server, use_system) {
        cmd.arg(opt);
    }
    if server.port != 0 {
        cmd.arg("-p").arg(server.port.to_string());
    }
    cmd.arg(server.target()).arg(remote_cmd);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let out = cmd.output().map_err(|e| format!("couldn't launch ssh: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(err
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("command failed")
            .to_string())
    }
}

/// Run a batch of sftp commands against `server`, returning stdout on success.
/// `use_system` selects auth mode (delegate to ~/.ssh vs. explicit key).
fn sftp_batch(server: &SftpServer, script: &str, use_system: bool) -> Result<String, String> {
    use std::io::Write;
    // Password servers: make sure the authenticated master is up first.
    ensure_master(server)?;
    let mut cmd = Command::new("sftp");
    cmd.arg("-b").arg("-"); // read commands from stdin
    for opt in ssh_options(server, use_system) {
        cmd.arg(opt);
    }
    if server.port != 0 {
        cmd.arg("-P").arg(server.port.to_string());
    }
    cmd.arg(server.target());
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("couldn't launch sftp: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(script.as_bytes());
        let _ = stdin.write_all(b"\nquit\n");
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        // Surface the most useful line (auth/host/connection failure).
        let msg = err
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("connection failed")
            .to_string();
        Err(msg)
    }
}

/// The server's default remote directory (the login home), via `sftp pwd`.
fn sftp_home(server: &SftpServer, use_system: bool) -> Result<String, String> {
    let out = sftp_batch(server, "pwd", use_system)?;
    // sftp prints: "Remote working directory: /home/user"
    for line in out.lines() {
        if let Some(p) = line.split_once("working directory:") {
            let path = p.1.trim();
            if !path.is_empty() {
                return Ok(path.to_string());
            }
        }
    }
    Ok("/".to_string())
}

/// A literal SFTP glob argument. Batch commands are line-oriented, so reject
/// line breaks rather than allowing a remote cwd to become another command.
fn sftp_path_arg(path: &str) -> Result<String, String> {
    if path.contains(['\0', '\r', '\n']) { return Err("SFTP cannot browse a path containing line breaks or NUL bytes.".into()); }
    // OpenSSH treats wildcard characters inside quotes literally. Escaping
    // them again would introduce literal backslashes into the remote path.
    let mut quoted = String::from("\"");
    for ch in path.chars() {
        if matches!(ch, '\\' | '"') { quoted.push('\\'); }
        quoted.push(ch);
    }
    quoted.push('"');
    Ok(quoted)
}

/// List a remote directory, returning entries parsed from sftp's `ls -l`.
fn sftp_list(server: &SftpServer, path: &str, use_system: bool) -> Result<Vec<Entry>, String> {
    let script = remote_listing::list_command(path)?;
    let out = sftp_batch(server, &script, use_system)?;
    Ok(parse_sftp_ls(&out))
}

/// Create a remote directory.
fn sftp_mkdir(server: &SftpServer, path: &str, use_system: bool) -> Result<(), String> {
    sftp_batch(server, &format!("mkdir \"{}\"", path.replace('"', "")), use_system).map(|_| ())
}

/// Rename/move a remote path (same server).
fn sftp_rename(server: &SftpServer, from: &str, to: &str, use_system: bool) -> Result<(), String> {
    sftp_batch(
        server,
        &format!("rename \"{}\" \"{}\"", from.replace('"', ""), to.replace('"', "")),
        use_system,
    )
    .map(|_| ())
}

/// Upload a local file/folder into a remote directory (recursive for folders).
fn sftp_upload(
    server: &SftpServer,
    local: &Path,
    remote_dir: &str,
    use_system: bool,
) -> Result<(), String> {
    let l = local.to_string_lossy().replace('"', "");
    let name = local
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let dest = format!("{}/{}", remote_dir.trim_end_matches('/'), name).replace('"', "");
    let recurse = if local.is_dir() { "-r " } else { "" };
    sftp_batch(server, &format!("put {recurse}\"{l}\" \"{dest}\""), use_system).map(|_| ())
}

/// Permanently delete a remote path (files or folders, recursively) via ssh.
fn sftp_delete(server: &SftpServer, path: &str, use_system: bool) -> Result<(), String> {
    ssh_exec(server, &format!("rm -rf {}", shell_quote(path)), use_system).map(|_| ())
}

/// Create an empty remote file via ssh `touch`.
fn sftp_touch(server: &SftpServer, path: &str, use_system: bool) -> Result<(), String> {
    ssh_exec(server, &format!("touch {}", shell_quote(path)), use_system).map(|_| ())
}

/// Load saved servers.
fn load_sftp_servers() -> Vec<SftpServer> {
    let mut out = Vec::new();
    if let Some(file) = config_file("sftp_servers.txt") {
        if let Ok(s) = fs::read_to_string(&file) {
            for line in s.lines() {
                let f: Vec<&str> = line.split('\t').collect();
                if f.len() < 2 || f[0].trim().is_empty() {
                    continue;
                }
                out.push(SftpServer {
                    control_path: None,
                    name: f[0].to_string(),
                    host: f.get(1).unwrap_or(&"").to_string(),
                    user: f.get(2).unwrap_or(&"").to_string(),
                    port: f.get(3).and_then(|p| p.parse().ok()).unwrap_or(0),
                    key: f.get(4).unwrap_or(&"").to_string(),
                    use_password: f.get(5).map(|v| v.trim() == "true").unwrap_or(false),
                    auto_reopen: f.get(6).map(|v| v.trim() == "true").unwrap_or(false),
                });
            }
        }
    }
    out
}

// ----- native OS file drag-out (drag files into Finder / other apps) ---------

define_class!(
    // A minimal NSDraggingSource: it only needs to say the drag is a "copy".
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "ShuffleDragSource"]
    struct DragSource;

    unsafe impl NSObjectProtocol for DragSource {}

    unsafe impl NSDraggingSource for DragSource {
        #[unsafe(method(draggingSession:sourceOperationMaskForDraggingContext:))]
        fn source_operation_mask(
            &self,
            _session: &NSDraggingSession,
            _context: NSDraggingContext,
        ) -> NSDragOperation {
            // Copy only: dragging out to another app never moves/deletes the
            // original file.
            NSDragOperation::Copy
        }
    }
);

thread_local! {
    /// One shared dragging-source instance, reused for every drag.
    static DRAG_SOURCE: RefCell<Option<objc2::rc::Retained<DragSource>>> = RefCell::new(None);
}

fn drag_source(mtm: objc2::MainThreadMarker) -> objc2::rc::Retained<DragSource> {
    DRAG_SOURCE.with(|c| {
        c.borrow_mut()
            .get_or_insert_with(|| unsafe { objc2::msg_send![DragSource::alloc(mtm), init] })
            .clone()
    })
}

/// The GPUI content `NSView` pointer for this window (for native drag sessions).
fn ns_view_ptr(window: &Window) -> Option<*mut std::ffi::c_void> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = HasWindowHandle::window_handle(window).ok()?;
    match handle.as_raw() {
        RawWindowHandle::AppKit(h) => Some(h.ns_view.as_ptr()),
        _ => None,
    }
}

thread_local! {
    /// Full header width, including any preview (0 = not yet measured).
    static PANE_W: RefCell<[f32; 2]> = const { RefCell::new([0.0; 2]) };
    /// File viewport width, excluding the preview.
    static BROWSER_W: RefCell<[f32; 2]> = const { RefCell::new([0.0; 2]) };
}

fn pane_width(pane: usize) -> f32 {
    PANE_W.with(|w| w.borrow()[pane.min(1)])
}

fn browser_width(pane: usize) -> f32 {
    BROWSER_W.with(|w| w.borrow()[pane.min(1)])
}

thread_local! {
    /// The open share picker, kept alive while its popover is showing.
    static SHARE_PICKER: RefCell<Option<objc2::rc::Retained<objc2_app_kit::NSSharingServicePicker>>> =
        const { RefCell::new(None) };
}

/// Show macOS's share popover (AirDrop, Messages, Mail, Notes, share
/// extensions…) for `paths`, pointing at `at` (window coords, top-left origin).
/// Picking a service runs it; the system handles everything from there.
fn show_share_picker(view_ptr: *mut std::ffi::c_void, at: (f32, f32), paths: &[PathBuf]) {
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::{NSSharingServicePicker, NSView};
    use objc2_foundation::{NSArray, NSPoint, NSRect, NSRectEdge, NSSize};

    if paths.is_empty() || view_ptr.is_null() || objc2::MainThreadMarker::new().is_none() {
        return;
    }
    // SAFETY: the GPUI content view is a live NSView on the main thread.
    let view: &NSView = unsafe { &*(view_ptr as *const NSView) };
    let urls: Vec<Retained<AnyObject>> = paths
        .iter()
        .filter_map(|p| p.to_str())
        .map(|s| Retained::into_super(Retained::into_super(NSURL::fileURLWithPath(&NSString::from_str(s)))))
        .collect();
    if urls.is_empty() {
        return;
    }
    let items = NSArray::from_retained_slice(&urls);
    // SAFETY: file NSURLs conform to NSPasteboardWriting, as the picker requires.
    let picker = unsafe { NSSharingServicePicker::initWithItems(NSSharingServicePicker::alloc(), &items) };
    let (x, y) = (at.0 as f64, at.1 as f64);
    let y = if view.isFlipped() { y } else { view.bounds().size.height - y };
    let rect = NSRect { origin: NSPoint { x, y }, size: NSSize { width: 1.0, height: 1.0 } };
    picker.showRelativeToRect_ofView_preferredEdge(rect, view, NSRectEdge::MinY);
    SHARE_PICKER.with(|p| *p.borrow_mut() = Some(picker));
}

/// Start a native macOS drag session carrying `paths` as file URLs. macOS then
/// drives the drag through the normal run loop; dropping on another app copies
/// the files there, and dropping back on Shuffle arrives as an external-file
/// drop (handled by our `ExternalPaths` drop targets).
fn start_os_file_drag(view_ptr: *mut std::ffi::c_void, paths: &[PathBuf]) {
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, ProtocolObject};
    use objc2::AllocAnyThread;
    use objc2_app_kit::{
        NSApplication, NSDraggingItem, NSDraggingSource, NSPasteboardWriting, NSView, NSWorkspace,
    };
    use objc2_foundation::{NSArray, NSPoint, NSRect, NSSize, NSString, NSURL};

    if paths.is_empty() || view_ptr.is_null() {
        return;
    }
    let Some(mtm) = objc2::MainThreadMarker::new() else {
        return;
    };
    // SAFETY: the GPUI content view is a live NSView on the main thread.
    let view: &NSView = unsafe { &*(view_ptr as *const NSView) };
    let app = NSApplication::sharedApplication(mtm);
    let Some(event) = app.currentEvent() else {
        return;
    };
    let base = event.locationInWindow();
    let workspace = NSWorkspace::sharedWorkspace();

    let mut items: Vec<Retained<NSDraggingItem>> = Vec::new();
    for (i, p) in paths.iter().enumerate() {
        let Some(s) = p.to_str() else { continue };
        let ns = NSString::from_str(s);
        let url = NSURL::fileURLWithPath(&ns);
        let writer: &ProtocolObject<dyn NSPasteboardWriting> = ProtocolObject::from_ref(&*url);
        let item = NSDraggingItem::initWithPasteboardWriter(NSDraggingItem::alloc(), writer);

        let icon = workspace.iconForFile(&ns);
        let size = NSSize { width: 48.0, height: 48.0 };
        icon.setSize(size);
        // Fan the icons out slightly so a multi-file drag reads as a stack.
        let off = i as f64 * 6.0;
        let frame = NSRect {
            origin: NSPoint { x: base.x - 24.0 + off, y: base.y - 24.0 - off },
            size,
        };
        let contents: &AnyObject = &icon;
        unsafe { item.setDraggingFrame_contents(frame, Some(contents)) };
        items.push(item);
    }
    if items.is_empty() {
        return;
    }
    let array = NSArray::from_retained_slice(&items);

    let source = drag_source(mtm);
    let source_proto: &ProtocolObject<dyn NSDraggingSource> = ProtocolObject::from_ref(&*source);
    let _ = view.beginDraggingSessionWithItems_event_source(&array, &event, source_proto);
}

/// A minimal, non-interactive vertical scrollbar thumb for any tracked
/// `ScrollHandle`. Returns `None` when the content fits (nothing to scroll).
/// Used by the palette and Settings (the main list has its own
/// richer draggable/fading bar).
fn static_scrollbar_thumb(base: &gpui::ScrollHandle) -> Option<AnyElement> {
    let viewport = f64::from(base.bounds().size.height) as f32;
    let max = f64::from(base.max_offset().height) as f32;
    if viewport <= 1.0 || max <= 1.0 {
        return None;
    }
    let scrolled = (-(f64::from(base.offset().y) as f32)).clamp(0.0, max);
    let content = viewport + max;
    let thumb_h = (viewport * viewport / content).clamp(20.0, viewport);
    let thumb_top = (viewport - thumb_h) * (scrolled / max);
    Some(
        div()
            .absolute()
            .top(px(thumb_top))
            .right(px(2.0))
            .w(px(6.0))
            .h(px(thumb_h))
            .rounded_full()
            .bg(Theme::alpha(theme().text, 0x44))
            .into_any_element(),
    )
}

/// Build a `.tooltip(...)` callback showing `text` in a small floating label.
fn tip(text: impl Into<String>) -> impl Fn(&mut Window, &mut App) -> gpui::AnyView + 'static {
    let text = text.into();
    move |_, cx| cx.new(|_| TooltipView { text: text.clone() }).into()
}

/// A sidebar icon element by cache key, falling back to the generic folder icon
/// so a row is never blank.
/// Ask NSWorkspace for `path`'s icon, decode it, and convert to a GPUI image.
/// This is the expensive part (AppKit + TIFF decode + resize), so it runs only
/// off the render path, in the background pre-warm task.
/// Fetch a file's macOS icon as TIFF bytes, rendered at a small fixed size.
///
/// `iconForFile` is instant, but `NSImage::TIFFRepresentation` renders *every*
/// representation up to 1024px (tens to hundreds of ms). Instead we draw the
/// icon once into a 128px bitmap and serialize only that — a few ms. Touches
/// AppKit, so it must run on the main thread.
fn icon_tiff(path: &Path) -> Option<Vec<u8>> {
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    let path_str = path.to_str()?;
    let workspace = NSWorkspace::sharedWorkspace();
    let ns_path = NSString::from_str(path_str);
    let image: objc2::rc::Retained<NSImage> = workspace.iconForFile(&ns_path);

    let rep = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            128,
            128,
            8,
            4,
            true,
            false,
            NSDeviceRGBColorSpace,
            0,
            0,
        )
    }?;
    let ctx = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)?;
    NSGraphicsContext::saveGraphicsState_class();
    NSGraphicsContext::setCurrentContext(Some(&ctx));
    let dst = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(128.0, 128.0));
    let zero = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0));
    image.drawInRect_fromRect_operation_fraction(dst, zero, NSCompositingOperation::Copy, 1.0);
    NSGraphicsContext::restoreGraphicsState_class();

    let data: objc2::rc::Retained<NSData> = rep.TIFFRepresentation()?;
    if data.len() == 0 {
        return None;
    }
    Some(data.to_vec())
}

/// Decode TIFF icon bytes and convert to a 128px GPUI image. Pure CPU (the
/// expensive part — the large TIFF decode), safe to run off the main thread.
fn decode_icon(tiff: &[u8]) -> Option<Arc<RenderImage>> {
    decode_image(tiff, 128)
}

fn decode_image(bytes: &[u8], edge: u32) -> Option<Arc<RenderImage>> {
    let decoded = image::load_from_memory(bytes).ok()?;
    // 128px stays crisp in the Icons view; the GPU downscales cleanly
    // to 16px in list view. One cached icon per file type.
    let decoded = decoded.resize_exact(edge, edge, image::imageops::FilterType::Lanczos3);
    let rgba = decoded.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut raw = rgba.into_raw();
    // RenderImage expects BGRA; the decoded buffer is RGBA, so swap R and B.
    for px in raw.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let buffer = image::RgbaImage::from_raw(w, h, raw)?;
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
}

/// Build a file's icon synchronously (AppKit fetch + decode). Used only at
/// startup for the base folder/file icons; the per-type prewarm splits these
/// across threads to keep navigation smooth.
fn build_macos_icon(path: &Path) -> Option<Arc<RenderImage>> {
    let tiff = icon_tiff(path)?;
    decode_icon(&tiff)
}

/// Format a modification time as a local date/time, or "--" if unknown.
fn format_date(modified: Option<SystemTime>) -> String {
    system_dates::format(modified)
}

/// Human-readable size; directories show "--".
fn format_size(is_dir: bool, size: u64) -> String {
    if is_dir {
        return "--".to_string();
    }
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = size as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{size} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Read a directory's entries with full metadata (one `stat` per entry), sorted
/// directories-first then case-insensitive by name. This is the slow path; the
/// UI shows `read_entries_fast` first and swaps this in from the background.
fn read_entries(dir: &Path) -> Vec<Entry> {
    let mut entries: Vec<Entry> = Vec::new();
    if let Ok(read_dir) = fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            // One stat, following symlinks: gives is_dir, size, and mtime.
            let md = fs::metadata(entry.path()).ok();
            let is_dir = md.as_ref().map(|m| m.is_dir()).unwrap_or(false);
            let size = md.as_ref().map(|m| m.len()).unwrap_or(0);
            let modified = md.as_ref().and_then(|m| m.modified().ok());
            let created = md.as_ref().and_then(|m| m.created().ok());
            entries.push(Entry {
                name,
                is_dir,
                size,
                modified,
                created,
                mode: md.as_ref().map(|m| { use std::os::unix::fs::PermissionsExt; m.permissions().mode() }),
                loaded: true,
                hidden_flag: md.as_ref().is_some_and(|m| {
                    use std::os::macos::fs::MetadataExt;
                    m.st_flags() & libc::UF_HIDDEN != 0
                }),
            });
        }
    }
    sort_default(&mut entries);
    entries
}

/// Read a directory's entries cheaply — names + folder/file from the readdir
/// `d_type`, with **no** per-file `stat` (only symlinks are resolved). This is
/// near-instant even for very large folders; size/dates fill in later.
fn read_entries_fast(dir: &Path) -> Vec<Entry> {
    let mut entries: Vec<Entry> = Vec::new();
    if let Ok(read_dir) = fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_dir = match entry.file_type() {
                Ok(t) if t.is_dir() => true,
                // Resolving a symlink needs a stat, but symlinks are rare.
                Ok(t) if t.is_symlink() => entry.path().is_dir(),
                _ => false,
            };
            entries.push(Entry {
                name,
                is_dir,
                size: 0,
                modified: None,
                created: None,
                mode: None,
                loaded: false,
                hidden_flag: false,
            });
        }
    }
    sort_default(&mut entries);
    entries
}

/// The default ordering (folders first, then case-insensitive by name). Uses
/// `sort_by_cached_key` so each name is lowercased once, not on every compare.
fn sort_default(entries: &mut [Entry]) {
    entries.sort_by_cached_key(|e| (!e.is_folder(), e.name.to_lowercase()));
}

/// Sort a directory listing in place by the given criterion/direction. Uses
/// `sort_by_cached_key` for name/kind so strings are lowercased once per item
/// (not on every comparison) — important for large folders.
fn sort_entries(entries: &mut [Entry], key: SortKey, asc: bool) {
    match key {
        SortKey::None => {
            sort_default(entries);
            return;
        }
        SortKey::Name => entries.sort_by_cached_key(|e| e.name.to_lowercase()),
        SortKey::Kind => {
            entries.sort_by_cached_key(|e| {
                (kind_label(&e.name, e.is_dir).to_lowercase(), e.name.to_lowercase())
            });
        }
        SortKey::Modified => entries.sort_by_key(|e| e.modified),
        SortKey::Created => entries.sort_by_key(|e| e.created),
        SortKey::Size => entries.sort_by_key(|e| e.size),
        SortKey::Extension => entries.sort_by_cached_key(|e| Path::new(&e.name).extension().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default()),
        SortKey::Permissions => entries.sort_by_key(|e| e.mode.map(|m| m & 0o7777)),
    }
    if !asc {
        entries.reverse();
    }
}

/// Character bigrams of a string.
fn bigrams(s: &str) -> Vec<(char, char)> {
    let chars: Vec<char> = s.chars().collect();
    chars.windows(2).map(|w| (w[0], w[1])).collect()
}

/// Sørensen–Dice similarity (0..1) over character bigrams. Tolerant of typos
/// and transpositions (e.g. "dcouments" vs "documents"), unlike subsequence.
fn dice(a: &str, b: &str) -> f32 {
    let ba = bigrams(a);
    let bb = bigrams(b);
    if ba.is_empty() || bb.is_empty() {
        return if a == b { 1.0 } else { 0.0 };
    }
    let mut counts: HashMap<(char, char), i32> = HashMap::new();
    for g in &bb {
        *counts.entry(*g).or_insert(0) += 1;
    }
    let mut inter = 0;
    for g in &ba {
        if let Some(c) = counts.get_mut(g) {
            if *c > 0 {
                *c -= 1;
                inter += 1;
            }
        }
    }
    2.0 * inter as f32 / (ba.len() + bb.len()) as f32
}

/// Typo-tolerant match score of a partial name against a candidate name.
/// Combines exact/prefix/substring/subsequence signals with Dice similarity.
/// Score one directory entry against the in-directory find query. Returns
/// `None` for non-matches so they're filtered out. Subsequence matches rank
/// highest; close typos (Sørensen–Dice ≥ 0.5) still match so "dcouments"
/// finds "Documents".
// ----- filter query operators (kind: / ext: / size: / date: / content:) -----

/// Broad file-type categories for `kind:` filters.
#[derive(Clone, Copy, PartialEq)]
enum KindClass {
    Folder,
    Image,
    Video,
    Audio,
    Pdf,
    Doc,
    Archive,
    Code,
    App,
}

impl KindClass {
    fn from_word(w: &str) -> Option<KindClass> {
        Some(match w {
            "folder" | "dir" | "directory" | "folders" => KindClass::Folder,
            "image" | "img" | "images" | "picture" | "photo" | "pic" => KindClass::Image,
            "video" | "videos" | "movie" | "movies" | "film" => KindClass::Video,
            "audio" | "music" | "sound" | "song" => KindClass::Audio,
            "pdf" | "pdfs" => KindClass::Pdf,
            "doc" | "docs" | "document" | "documents" | "text" => KindClass::Doc,
            "archive" | "archives" | "zip" | "compressed" => KindClass::Archive,
            "code" | "source" | "src" => KindClass::Code,
            "app" | "apps" | "application" => KindClass::App,
            _ => return None,
        })
    }

    fn matches(self, name: &str, is_dir: bool) -> bool {
        let ext = Path::new(name)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());
        let ext = ext.as_deref();
        match self {
            KindClass::Folder => is_browsable_folder(Path::new(name), is_dir),
            KindClass::App => ext == Some("app"),
            // Every remaining category is file-only.
            _ if is_dir => false,
            KindClass::Image => matches!(
                ext,
                Some("jpg" | "jpeg" | "png" | "gif" | "heic" | "heif" | "tiff" | "tif" | "bmp"
                    | "webp" | "svg" | "ico" | "raw")
            ),
            KindClass::Video => matches!(
                ext,
                Some("mp4" | "mov" | "avi" | "mkv" | "webm" | "m4v" | "mpg" | "mpeg" | "wmv" | "flv")
            ),
            KindClass::Audio => matches!(
                ext,
                Some("mp3" | "wav" | "flac" | "aac" | "m4a" | "ogg" | "aiff" | "aif" | "opus")
            ),
            KindClass::Pdf => ext == Some("pdf"),
            KindClass::Doc => matches!(
                ext,
                Some("doc" | "docx" | "txt" | "md" | "rtf" | "pages" | "key" | "numbers" | "xls"
                    | "xlsx" | "ppt" | "pptx" | "odt" | "csv" | "tsv" | "log")
            ),
            KindClass::Archive => matches!(
                ext,
                Some("zip" | "tar" | "gz" | "tgz" | "bz2" | "tbz" | "xz" | "txz" | "7z" | "rar")
            ),
            KindClass::Code => matches!(
                ext,
                Some("rs" | "go" | "py" | "js" | "ts" | "tsx" | "jsx" | "c" | "cpp" | "cc" | "h"
                    | "hpp" | "java" | "swift" | "zig" | "rb" | "php" | "sh" | "json" | "toml"
                    | "yaml" | "yml" | "html" | "css" | "sql" | "lua" | "kt" | "scala")
            ),
        }
    }
}

/// A parsed size constraint in bytes.
#[derive(Clone, Copy)]
enum SizeBound {
    Gt(u64),
    Lt(u64),
    Range(u64, u64),
}

impl SizeBound {
    fn matches(self, size: u64) -> bool {
        match self {
            SizeBound::Gt(n) => size >= n,
            SizeBound::Lt(n) => size <= n,
            SizeBound::Range(a, b) => size >= a && size <= b,
        }
    }
}

/// Parse a byte count with an optional unit: `500`, `10k`, `2mb`, `1.5g`.
fn parse_bytes(s: &str) -> Option<u64> {
    let s = s.trim().to_lowercase();
    if s.is_empty() {
        return None;
    }
    let (num, mult) = if let Some(n) = s.strip_suffix("gb").or_else(|| s.strip_suffix('g')) {
        (n, 1024u64 * 1024 * 1024)
    } else if let Some(n) = s.strip_suffix("mb").or_else(|| s.strip_suffix('m')) {
        (n, 1024 * 1024)
    } else if let Some(n) = s.strip_suffix("kb").or_else(|| s.strip_suffix('k')) {
        (n, 1024)
    } else if let Some(n) = s.strip_suffix('b') {
        (n, 1)
    } else {
        (s.as_str(), 1)
    };
    let val: f64 = num.trim().parse().ok()?;
    if val < 0.0 {
        return None;
    }
    Some((val * mult as f64) as u64)
}

/// Parse a `size:` value: `>1mb`, `<100k`, `1mb..5mb`, or a bare `2gb` (exact-ish
/// → treated as "at least").
fn parse_size(s: &str) -> Option<SizeBound> {
    let s = s.trim();
    if let Some((a, b)) = s.split_once("..") {
        return Some(SizeBound::Range(parse_bytes(a)?, parse_bytes(b)?));
    }
    if let Some(rest) = s.strip_prefix(">=").or_else(|| s.strip_prefix('>')) {
        return Some(SizeBound::Gt(parse_bytes(rest)?));
    }
    if let Some(rest) = s.strip_prefix("<=").or_else(|| s.strip_prefix('<')) {
        return Some(SizeBound::Lt(parse_bytes(rest)?));
    }
    Some(SizeBound::Gt(parse_bytes(s)?))
}

/// Local midnight `days` days ago, as a `SystemTime`.
fn day_start_ago(days: i64) -> Option<SystemTime> {
    use chrono::TimeZone;
    let day = (chrono::Local::now() - chrono::Duration::days(days)).date_naive();
    let naive = day.and_hms_opt(0, 0, 0)?;
    let local = chrono::Local.from_local_datetime(&naive).single()?;
    let ts = local.timestamp();
    (ts >= 0).then(|| std::time::UNIX_EPOCH + Duration::from_secs(ts as u64))
}

/// Midnight of an explicit `YYYY-MM-DD`, as a `SystemTime`.
fn date_start(ymd: &str) -> Option<SystemTime> {
    use chrono::TimeZone;
    let day = chrono::NaiveDate::parse_from_str(ymd.trim(), "%Y-%m-%d").ok()?;
    let naive = day.and_hms_opt(0, 0, 0)?;
    let local = chrono::Local.from_local_datetime(&naive).single()?;
    let ts = local.timestamp();
    (ts >= 0).then(|| std::time::UNIX_EPOCH + Duration::from_secs(ts as u64))
}

/// A parsed search query: free text plus any filter operators.
#[derive(Default, Clone)]
struct FilterQuery {
    /// Free text for name matching (operators stripped out).
    text: String,
    kinds: Vec<KindClass>,
    exts: Vec<String>,
    size: Option<SizeBound>,
    after: Option<SystemTime>,
    before: Option<SystemTime>,
    /// `content:`/`text:` term, searched via Spotlight (mdfind).
    content: Option<String>,
}

impl FilterQuery {
    /// Split a raw query into free text + `key:value` operators. Unknown keys
    /// are treated as plain text so a stray colon never eats the query.
    fn parse(q: &str) -> FilterQuery {
        let mut fq = FilterQuery::default();
        let mut text_parts: Vec<&str> = Vec::new();
        for tok in q.split_whitespace() {
            let Some((key, val)) = tok.split_once(':') else {
                text_parts.push(tok);
                continue;
            };
            if val.is_empty() {
                text_parts.push(tok);
                continue;
            }
            match key.to_lowercase().as_str() {
                "kind" | "type" => match KindClass::from_word(&val.to_lowercase()) {
                    Some(k) => fq.kinds.push(k),
                    None => text_parts.push(tok),
                },
                "ext" | "extension" => fq.exts.push(val.trim_start_matches('.').to_lowercase()),
                "size" => match parse_size(val) {
                    Some(s) => fq.size = Some(s),
                    None => text_parts.push(tok),
                },
                "date" | "modified" | "mtime" => {
                    let v = val.to_lowercase();
                    let applied = match v.as_str() {
                        "today" => {
                            fq.after = day_start_ago(0);
                            fq.after.is_some()
                        }
                        "yesterday" => {
                            fq.after = day_start_ago(1);
                            fq.before = day_start_ago(0);
                            fq.after.is_some()
                        }
                        "week" | "thisweek" | "7d" => {
                            fq.after = day_start_ago(7);
                            fq.after.is_some()
                        }
                        "month" | "thismonth" | "30d" => {
                            fq.after = day_start_ago(30);
                            fq.after.is_some()
                        }
                        "year" | "365d" => {
                            fq.after = day_start_ago(365);
                            fq.after.is_some()
                        }
                        _ => {
                            if let Some(d) = v.strip_prefix(">=").or_else(|| v.strip_prefix('>')) {
                                fq.after = date_start(d);
                                fq.after.is_some()
                            } else if let Some(d) =
                                v.strip_prefix("<=").or_else(|| v.strip_prefix('<'))
                            {
                                fq.before = date_start(d);
                                fq.before.is_some()
                            } else {
                                false
                            }
                        }
                    };
                    if !applied {
                        text_parts.push(tok);
                    }
                }
                "content" | "text" | "contains" => fq.content = Some(val.to_string()),
                _ => text_parts.push(tok),
            }
        }
        fq.text = text_parts.join(" ");
        fq
    }

    /// True when any local (non-content) operator is present.
    fn has_local_filters(&self) -> bool {
        !self.kinds.is_empty()
            || !self.exts.is_empty()
            || self.size.is_some()
            || self.after.is_some()
            || self.before.is_some()
    }

    /// Whether an entry passes the local operators (kind/ext/size/date). Content
    /// is handled separately (async, via Spotlight).
    fn matches_entry(&self, name: &str, is_dir: bool, size: u64, modified: Option<SystemTime>) -> bool {
        if !self.kinds.is_empty() && !self.kinds.iter().any(|k| k.matches(name, is_dir)) {
            return false;
        }
        if !self.exts.is_empty() {
            let ext = Path::new(name)
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase());
            match ext {
                Some(e) if self.exts.iter().any(|x| x == &e) => {}
                _ => return false,
            }
        }
        if let Some(sb) = self.size {
            if is_dir || !sb.matches(size) {
                return false;
            }
        }
        if self.after.is_some() || self.before.is_some() {
            let Some(m) = modified else { return false };
            if let Some(a) = self.after {
                if m < a {
                    return false;
                }
            }
            if let Some(b) = self.before {
                if m >= b {
                    return false;
                }
            }
        }
        true
    }
}

fn find_score(q: &str, name: &str) -> Option<i32> {
    let ql = q.to_lowercase();
    let nl = name.to_lowercase();
    let penalty = nl.len() as i32 / 4;
    if nl == ql {
        return Some(100_000);
    }
    if nl.starts_with(&ql) {
        return Some(50_000 - penalty);
    }
    if let Some(fs) = fuzzy_score(&ql, &nl) {
        return Some(10_000 + fs - penalty);
    }
    let d = dice(&ql, &nl);
    if d >= 0.5 {
        return Some((d * 5_000.0) as i32 - penalty);
    }
    None
}

/// Case-insensitive fuzzy (subsequence) score of `needle` against `haystack`.
/// Higher is better; `None` if `needle` isn't a subsequence. Rewards
/// contiguous runs and word-start matches, lightly penalizes length.
fn fuzzy_score(needle: &str, haystack: &str) -> Option<i32> {
    let n: Vec<char> = needle.to_lowercase().chars().collect();
    let h: Vec<char> = haystack.to_lowercase().chars().collect();
    if n.is_empty() {
        return Some(0);
    }
    let mut hi = 0usize;
    let mut score = 0i32;
    let mut last_match: i32 = -2;
    for &nc in &n {
        let mut found = None;
        while hi < h.len() {
            if h[hi] == nc {
                found = Some(hi);
                break;
            }
            hi += 1;
        }
        let pos = found?;
        if pos as i32 == last_match + 1 {
            score += 6; // contiguous run
        }
        if pos == 0
            || matches!(h[pos - 1], '/' | ' ' | '_' | '-' | '.')
        {
            score += 10; // start of a word/segment
        }
        score -= (pos as i32) / 4; // earlier matches slightly better
        last_match = pos as i32;
        hi = pos + 1;
    }
    score -= (h.len() as i32) / 8; // prefer shorter names
    Some(score)
}

/// Full-text search inside `dir` via Spotlight: paths whose content matches
/// `term`. Scoped with `-onlyin` so it's fast and folder-local. Returns the
/// absolute paths (the in-folder filter keeps only direct children of these).
fn mdfind_content(dir: &Path, term: &str) -> HashSet<PathBuf> {
    let term = term.trim();
    let mut out = HashSet::new();
    if term.is_empty() {
        return out;
    }
    // Case- and diacritic-insensitive substring match on indexed text content.
    let query = format!("kMDItemTextContent == \"*{}*\"cd", term.replace('"', ""));
    let output = Command::new("mdfind")
        .arg("-onlyin")
        .arg(dir)
        .arg(query)
        .stderr(Stdio::null())
        .output();
    if let Ok(o) = output {
        if o.status.success() {
            for line in String::from_utf8_lossy(&o.stdout).lines() {
                if !line.is_empty() {
                    out.insert(PathBuf::from(line));
                }
            }
        }
    }
    out
}

/// A short, human label for a path (last component; "/" → "Macintosh HD").
fn path_label(p: &Path) -> String {
    if p == Path::new("/") {
        return "Macintosh HD".to_string();
    }
    p.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.display().to_string())
}

// ----- persisted state -------------------------------------------------------

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

fn config_dir() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SCHNAUZ_CONFIG_DIR").or_else(|| std::env::var_os("GETTER_CONFIG_DIR"))
        .or_else(|| std::env::var_os("SHUFFLE_STUDIO_CONFIG_DIR")) { return Some(PathBuf::from(path)); }
    let support = home_dir().join("Library/Application Support");
    let current = support.join("Schnauz");
    if current.is_dir() { return Some(current); }
    for name in ["Getter", "Shuffle Studio"] {
        let legacy = support.join(name);
        if legacy.is_dir() { return Some(legacy); }
    }
    Some(current)
}

fn config_file(name: &str) -> Option<PathBuf> {
    Some(config_dir()?.join(name))
}

/// The directory to open on launch: the last-visited one if still valid,
/// otherwise the home directory.
fn load_last_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("SCHNAUZ_START_DIR").or_else(|| std::env::var_os("GETTER_START_DIR")).or_else(|| std::env::var_os("SHUFFLE_STUDIO_START_DIR")) {
        let path = PathBuf::from(path);
        if path.is_dir() { return path; }
    }
    if let Some(path) = config_file("last_dir.txt") {
        if let Ok(saved) = fs::read_to_string(&path) {
            let dir = PathBuf::from(saved.trim());
            if dir.is_dir() {
                return dir;
            }
        }
    }
    home_dir()
}

fn save_last_dir(dir: &Path) {
    if let Some(path) = config_file("last_dir.txt") {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&path, dir.to_string_lossy().as_bytes());
    }
}

/// Read a newline-separated list of paths, keeping only ones that still exist.
fn read_path_list(name: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(file) = config_file(name) {
        if let Ok(contents) = fs::read_to_string(&file) {
            for line in contents.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let path = PathBuf::from(line);
                // Keep files too (bookmarks can be files); drop only stale paths.
                if path.exists() {
                    paths.push(path);
                }
            }
        }
    }
    paths
}

/// Read a newline-separated list of arbitrary strings (e.g. server URLs).
fn read_string_list(name: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(file) = config_file(name) {
        if let Ok(contents) = fs::read_to_string(&file) {
            for line in contents.lines() {
                let line = line.trim();
                if !line.is_empty() {
                    out.push(line.to_string());
                }
            }
        }
    }
    out
}

fn write_string_list(name: &str, items: &[String]) {
    if let Some(file) = config_file(name) {
        if let Some(parent) = file.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&file, items.join("\n"));
    }
}

/// Read a single-line config value (trimmed), or `None` if absent/empty.
fn config_string(name: &str) -> Option<String> {
    let file = config_file(name)?;
    let s = fs::read_to_string(&file).ok()?;
    let s = s.trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Write a single-line config value.
fn write_config_string(name: &str, value: &str) {
    if let Some(file) = config_file(name) {
        if let Some(parent) = file.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&file, value.as_bytes());
    }
}

// --- Update check (GitHub "latest release" → dismissible banner) ---

/// The GitHub repo whose latest release drives the update banner.
fn upstream_updates_enabled() -> bool { false }

const GITHUB_REPO: &str = "WizenPainter/shuffle";
/// The stable "always the newest DMG" download link (same one the website uses).
const DMG_URL: &str = "https://github.com/WizenPainter/shuffle/releases/latest/download/Shuffle.dmg";

/// Parse a version like "v0.2.3" / "0.2.3" into numeric components for
/// comparison. `Vec<u32>` compares lexicographically, so [0,2,4] > [0,2,3].
/// Non-numeric junk in a component parses as 0 rather than failing.
fn parse_version(s: &str) -> Vec<u32> {
    s.trim()
        .trim_start_matches(['v', 'V'])
        .split('.')
        .map(|p| {
            p.chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .collect()
}

/// Pull a top-level string field out of a JSON blob without a JSON dependency:
/// find `"key"`, then the first `"..."` after the following `:`. Good enough for
/// GitHub's `tag_name`; not a general JSON parser.
fn json_string_field(body: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\"");
    let after_key = &body[body.find(&pat)? + pat.len()..];
    let after_colon = &after_key[after_key.find(':')? + 1..];
    let open = after_colon.find('"')? + 1;
    let rest = &after_colon[open..];
    let close = rest.find('"')?;
    Some(rest[..close].to_string())
}

/// Ask GitHub for the newest release tag (like "v0.2.6"). Blocking network
/// call — run it off the render thread.
fn fetch_latest_tag() -> Option<String> {
    let out = Command::new("curl")
        .args([
            "-sL",
            "--max-time",
            "8",
            "-H",
            "Accept: application/vnd.github+json",
            &format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest"),
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let body = String::from_utf8_lossy(&out.stdout).into_owned();
    json_string_field(&body, "tag_name").map(|t| t.trim().to_string())
}

/// The Apple Developer Team that legitimately signs Shuffle. The self-updater
/// refuses to install a download signed by anyone else, so a tampered or
/// spoofed DMG can never replace the app.
const EXPECTED_TEAM_ID: &str = "Z69U4AQSH3";

/// A downloaded, mounted, and verified update, ready to be swapped in.
struct PreparedUpdate {
    dmg: PathBuf,
    mount: PathBuf,
    app: PathBuf,
}

impl PreparedUpdate {
    /// Unmount the update DMG (used on the error path).
    fn detach(&self) {
        let _ = Command::new("hdiutil")
            .args(["detach", "-quiet"])
            .arg(&self.mount)
            .status();
    }
}

/// The `.app` bundle the running process lives in (…/Shuffle.app), if it can be
/// located from the executable path.
fn current_app_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    exe.ancestors()
        .find(|a| a.extension().is_some_and(|e| e == "app"))
        .map(Path::to_path_buf)
}

/// Download the latest DMG, mount it, and verify the app inside is notarized and
/// signed by our Team ID. Returns the mounted, verified update or a short reason
/// on failure. Runs entirely off the UI thread.
fn download_and_verify_update() -> Result<PreparedUpdate, String> {
    let dir = std::env::temp_dir().join("shuffle-update");
    let _ = fs::create_dir_all(&dir);
    let dmg = dir.join("Shuffle-latest.dmg");

    // Download (follow redirects, fail on HTTP errors, cap the time).
    let ok = Command::new("curl")
        .args(["-sL", "--fail", "--max-time", "180", "-o"])
        .arg(&dmg)
        .arg(DMG_URL)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        return Err("couldn't download the update".to_string());
    }

    // Mount it read-only.
    let out = Command::new("hdiutil")
        .args(["attach", "-nobrowse", "-readonly", "-noverify"])
        .arg(&dmg)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err("couldn't mount the update".to_string());
    }
    let mount = parse_hdiutil_mount(&String::from_utf8_lossy(&out.stdout))
        .ok_or_else(|| "couldn't find the mounted volume".to_string())?;

    let app = mount.join("Shuffle.app");
    let prepared = PreparedUpdate { dmg, mount, app: app.clone() };
    if !app.exists() {
        prepared.detach();
        return Err("the update didn't contain Shuffle.app".to_string());
    }
    // Only install a download that is genuinely, verifiably ours.
    if let Err(e) = verify_app(&app) {
        prepared.detach();
        return Err(e);
    }
    Ok(prepared)
}

/// Confirm `app` has a valid signature, passes Gatekeeper (i.e. is notarized),
/// and is signed by our Team ID. Any of these failing aborts the update.
fn verify_app(app: &Path) -> Result<(), String> {
    let sig_ok = Command::new("codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(app)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !sig_ok {
        return Err("the update's signature was invalid".to_string());
    }

    let gatekeeper_ok = Command::new("spctl")
        .args(["-a", "-t", "exec", "-vv"])
        .arg(app)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !gatekeeper_ok {
        return Err("the update wasn't notarized by Apple".to_string());
    }

    // codesign prints the signing info to stderr.
    let info = Command::new("codesign")
        .args(["-dv", "--verbose=4"])
        .arg(app)
        .output()
        .map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&info.stderr);
    let team_matches = text
        .lines()
        .any(|l| l.trim() == format!("TeamIdentifier={EXPECTED_TEAM_ID}"));
    if !team_matches {
        return Err("the update was signed by an unexpected developer".to_string());
    }
    Ok(())
}

/// Parse `hdiutil attach` output for the `/Volumes/…` mount point (the volume
/// name may contain spaces, so take everything from `/Volumes/` to line end).
fn parse_hdiutil_mount(output: &str) -> Option<PathBuf> {
    output.lines().find_map(|line| {
        line.find("/Volumes/")
            .map(|i| PathBuf::from(line[i..].trim_end()))
    })
}

/// Write and launch a detached helper that waits for this process to quit, then
/// replaces the installed bundle with the verified update and relaunches it. If
/// the replace fails (e.g. no write permission), it falls back to opening the
/// DMG so the user can install by hand. We must do this from a separate process
/// because a running app can't overwrite its own executable.
fn launch_swap_and_relaunch(bundle: &Path, ready: &PreparedUpdate) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let pid = std::process::id();
    let script = std::env::temp_dir().join("shuffle-selfupdate.sh");
    // $1 DEST bundle  $2 SRC app  $3 MOUNT  $4 DMG  $5 PID  $6 fallback URL
    // The swap NEVER deletes the old bundle — it renames it into the Trash
    // (Sparkle-style). Hard-deleting an app reads as an uninstall to macOS,
    // which revokes its TCC privacy grants and makes the updated app re-ask
    // for every folder permission. A rename keeps the identity alive, so
    // grants carry over. Both renames are atomic on the same volume.
    let body = r#"#!/bin/bash
DEST="$1"; SRC="$2"; MOUNT="$3"; DMG="$4"; PID="$5"; URL="$6"
# Wait (up to ~20s) for the old app to exit before touching its bundle.
for _ in $(seq 1 200); do kill -0 "$PID" 2>/dev/null || break; sleep 0.1; done
TRASH="$HOME/.Trash"
OLD="$TRASH/Shuffle-old-$$.app"
ok=""
if ditto "$SRC" "$DEST.updnew" 2>/dev/null; then
  # Defensive: a quarantined copy would launch translocated (and re-prompt).
  xattr -dr com.apple.quarantine "$DEST.updnew" 2>/dev/null
  # Move the old app aside (Trash first, sibling rename as fallback)…
  if ! mv "$DEST" "$OLD" 2>/dev/null; then
    OLD="${DEST%.app}.updold.app"
    mv "$DEST" "$OLD" 2>/dev/null
  fi
  # …then the new one into place; roll the old back if that fails.
  if [ ! -e "$DEST" ] && mv "$DEST.updnew" "$DEST" 2>/dev/null; then
    ok=1
  else
    [ -d "$OLD" ] && [ ! -e "$DEST" ] && mv "$OLD" "$DEST" 2>/dev/null
  fi
fi
if [ -n "$ok" ]; then
  hdiutil detach "$MOUNT" -quiet 2>/dev/null
  rm -f "$DMG" 2>/dev/null
  # If the Trash move had failed, try once more now; never hard-delete it.
  case "$OLD" in
    "$TRASH"/*) : ;;
    *) mv "$OLD" "$TRASH/Shuffle-old-$$.app" 2>/dev/null ;;
  esac
  open "$DEST"
else
  rm -rf "$DEST.updnew" 2>/dev/null
  hdiutil detach "$MOUNT" -quiet 2>/dev/null
  open "$DMG" 2>/dev/null || open "$URL" 2>/dev/null
fi
"#;
    fs::write(&script, body).map_err(|e| e.to_string())?;
    let _ = fs::set_permissions(&script, fs::Permissions::from_mode(0o755));

    Command::new("/bin/bash")
        .arg(&script)
        .arg(bundle)
        .arg(&ready.app)
        .arg(&ready.mount)
        .arg(&ready.dmg)
        .arg(pid.to_string())
        .arg(DMG_URL)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Load sidebar groups from `groups.txt`. A `[name]` line starts a group; the
/// lines after it are member paths, until the next `[name]`.
fn load_groups() -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    if let Some(file) = config_file("groups.txt") {
        if let Ok(contents) = fs::read_to_string(&file) {
            for line in contents.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if let Some(name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                    groups.push(Group { name: name.to_string(), paths: Vec::new() });
                } else if let Some(g) = groups.last_mut() {
                    g.paths.push(PathBuf::from(line));
                }
            }
        }
    }
    groups
}

/// Persist sidebar groups to `groups.txt`.
fn save_groups(groups: &[Group]) {
    if let Some(file) = config_file("groups.txt") {
        if let Some(parent) = file.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let mut body = String::new();
        for g in groups {
            body.push('[');
            body.push_str(&g.name);
            body.push_str("]\n");
            for p in &g.paths {
                body.push_str(&p.to_string_lossy());
                body.push('\n');
            }
        }
        let _ = fs::write(&file, body);
    }
}

/// Persist the active theme (all eleven colors as hex, one per line).
fn save_theme(t: &Theme) {
    if let Some(file) = config_file("theme.txt") {
        if let Some(parent) = file.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let v = [
            t.bg, t.sidebar, t.surface, t.hover, t.selected, t.border, t.border_strong, t.text,
            t.text_muted, t.text_dim, t.accent,
        ];
        let body: String = v.iter().map(|c| format!("{c:06x}\n")).collect();
        let _ = fs::write(&file, body);
    }
}

/// Load the saved theme, falling back to the default if absent/corrupt.
fn load_theme() -> Theme {
    if let Some(file) = config_file("theme.txt") {
        if let Ok(s) = fs::read_to_string(&file) {
            let v: Vec<u32> = s
                .lines()
                .filter_map(|l| u32::from_str_radix(l.trim(), 16).ok())
                .collect();
            if v.len() == 11 {
                return Theme {
                    bg: v[0],
                    sidebar: v[1],
                    surface: v[2],
                    hover: v[3],
                    selected: v[4],
                    border: v[5],
                    border_strong: v[6],
                    text: v[7],
                    text_muted: v[8],
                    text_dim: v[9],
                    accent: v[10],
                };
            }
        }
    }
    Theme::default()
}

/// Persist the menu style.
fn save_menu_style(m: &MenuStyle) {
    if let Some(file) = config_file("menu.txt") {
        if let Some(parent) = file.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let body = format!(
            "bg={:06x}\ntext={:06x}\nopacity={}\nfont_px={}\ncustom={}\n",
            m.bg, m.text, m.opacity, m.font_px, m.custom
        );
        let _ = fs::write(&file, body);
    }
}

/// Load the saved menu style, falling back to defaults for missing fields.
fn load_menu_style() -> MenuStyle {
    let mut m = MenuStyle::default();
    if let Some(file) = config_file("menu.txt") {
        if let Ok(s) = fs::read_to_string(&file) {
            for line in s.lines() {
                let Some((k, v)) = line.split_once('=') else {
                    continue;
                };
                let (k, v) = (k.trim(), v.trim());
                match k {
                    "bg" => {
                        if let Ok(c) = u32::from_str_radix(v, 16) {
                            m.bg = c;
                        }
                    }
                    "text" => {
                        if let Ok(c) = u32::from_str_radix(v, 16) {
                            m.text = c;
                        }
                    }
                    "opacity" => {
                        if let Ok(n) = v.parse::<u8>() {
                            m.opacity = n.min(100);
                        }
                    }
                    "font_px" => {
                        if let Ok(n) = v.parse::<f32>() {
                            m.font_px = n.clamp(9.0, 24.0);
                        }
                    }
                    "custom" => m.custom = v == "true",
                    _ => {}
                }
            }
        }
    }
    m
}

/// Persist feature preferences as key/value lines.
fn save_prefs(p: &Prefs) {
    if let Some(file) = config_file("prefs.txt") {
        if let Some(parent) = file.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let mut body = format!(
            "preview={}\npreview_pages={}\ninfo={}\nshow_parent={}\nsidebar_collapsed={}\npalette_history={}\ngroups_enabled={}\nshow_filter_button={}\nshow_fps={}\nscript_actions={}\nssh_use_system={}\nssh_configured={}\nwaterfall={}\npalette_opacity={}\n",
            p.preview, p.preview_pages, p.info, p.show_parent, p.sidebar_collapsed, p.palette_history, p.groups_enabled, p.show_filter_button, p.show_fps, p.script_actions, p.ssh_use_system, p.ssh_configured, p.waterfall, p.palette_opacity
        );
        body.push_str(&format!("preview_width={}\n",p.preview_width));
        body.push_str(&format!("drive_warning={}\ndrive_critical={}\n", p.drive_warning, p.drive_critical));
        let _ = fs::write(&file, body);
    }
}

/// Persist the keymap as `action=keystroke` lines (empty = unbound).
fn save_keymap(k: &Keymap) {
    if let Some(file) = config_file("keymap.txt") {
        if let Some(parent) = file.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let body: String = KeyAction::ALL
            .iter()
            .map(|a| format!("{}={}\n", a.key(), k.get(*a).unwrap_or("")))
            .collect();
        let _ = fs::write(&file, body);
    }
}

/// Persist the active icon-pack folder (empty = none).
fn save_icon_pack(p: &Option<PathBuf>) {
    if let Some(file) = config_file("icon_pack.txt") {
        if let Some(parent) = file.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let body = p.as_ref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        let _ = fs::write(&file, body);
    }
}

/// Persist the app-icon background choice.
fn save_icon_bg(bg: &IconBg) {
    if let Some(file) = config_file("app_icon.txt") {
        if let Some(parent) = file.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let body = match bg {
            IconBg::Default => String::new(),
            IconBg::Color(c) => format!("color={c:06x}"),
            IconBg::Image(p) => format!("image={}", p.to_string_lossy()),
        };
        let _ = fs::write(&file, body);
    }
}

/// Load the saved app-icon background choice.
fn load_icon_bg() -> IconBg {
    let Some(file) = config_file("app_icon.txt") else {
        return IconBg::Default;
    };
    let Ok(s) = fs::read_to_string(&file) else {
        return IconBg::Default;
    };
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("color=") {
        if let Ok(c) = u32::from_str_radix(hex.trim(), 16) {
            return IconBg::Color(c);
        }
    } else if let Some(path) = s.strip_prefix("image=") {
        let p = PathBuf::from(path.trim());
        if p.is_file() {
            return IconBg::Image(p);
        }
    }
    IconBg::Default
}

/// Copy an uploaded background image into the config dir so it persists, and
/// return the stored path.
fn store_icon_bg_image(src: &Path) -> Option<PathBuf> {
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("png");
    let dest = config_file(&format!("app_icon_bg.{ext}"))?;
    if let Some(parent) = dest.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::copy(src, &dest).ok()?;
    Some(dest)
}

/// Load the saved icon-pack folder, if it still exists.
fn load_icon_pack() -> Option<PathBuf> {
    let file = config_file("icon_pack.txt")?;
    let s = fs::read_to_string(&file).ok()?;
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let p = PathBuf::from(s);
    p.is_dir().then_some(p)
}

/// Load the keymap, starting from defaults and applying saved overrides.
fn load_keymap() -> Keymap {
    let mut k = Keymap::defaults();
    if let Some(file) = config_file("keymap.txt") {
        if let Ok(s) = fs::read_to_string(&file) {
            for line in s.lines() {
                let Some((name, val)) = line.split_once('=') else {
                    continue;
                };
                if let Some(a) = KeyAction::ALL.iter().copied().find(|a| a.key() == name.trim()) {
                    let v = val.trim();
                    k.set(a, if v.is_empty() { None } else { Some(v.to_string()) });
                }
            }
        }
    }
    k
}

/// Load preferences, retaining defaults for missing or invalid entries.
fn load_prefs() -> Prefs {
    let mut p = Prefs::default();
    if let Some(file) = config_file("prefs.txt") {
        if let Ok(s) = fs::read_to_string(&file) {
            for line in s.lines() {
                let Some((k, v)) = line.split_once('=') else {
                    continue;
                };
                let on = v.trim() == "true";
                match k.trim() {
                    "preview" => p.preview = on,
                    "preview_pages" => p.preview_pages = on,
                    "preview_width" => if let Ok(width) = v.trim().parse::<u16>() { p.preview_width = width.clamp(220,1200); },
                    "info" => p.info = on,
                    "show_parent" => p.show_parent = on,
                    "sidebar_collapsed" => p.sidebar_collapsed = on,
                    "drive_warning" => if let Ok(value) = v.trim().parse::<u8>() { p.drive_warning = value.clamp(1,98); },
                    "drive_critical" => if let Ok(value) = v.trim().parse::<u8>() { p.drive_critical = value.clamp(2,99); },

                    "palette_history" => p.palette_history = on,
                    "groups_enabled" => p.groups_enabled = on,
                    "show_filter_button" => p.show_filter_button = on,
                    "show_fps" => p.show_fps = on,
                    "script_actions" => p.script_actions = on,
                    "ssh_use_system" => p.ssh_use_system = on,
                    "ssh_configured" => p.ssh_configured = on,
                    "waterfall" => p.waterfall = on,
                    "palette_opacity" => {
                        if let Ok(n) = v.trim().parse::<u8>() {
                            p.palette_opacity = n.clamp(20, 100);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    if p.drive_critical <= p.drive_warning { p.drive_critical = p.drive_warning + 1; }
    p
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "--video-preview-test" {
        match video_preview::smoke_test(PathBuf::from(&args[2])) {
            Ok(message) => println!("{message}; system date: {}", format_date(Some(SystemTime::now()))),
            Err(message) => { eprintln!("{message}"); std::process::exit(1); }
        }
        return;
    }
    // Hidden check: `shuffle --pdf-bench <file> [page]` renders one PDF page
    // through the inspector-pager pipeline and prints the result, then exits.
    if args.len() >= 3 && args[1] == "--pdf-bench" {
        let path = PathBuf::from(&args[2]);
        let page: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
        let t0 = std::time::Instant::now();
        match render_pdf_page(&path, page) {
            Some((_, count)) => eprintln!(
                "ok: rendered page {} of {} in {} ms",
                page + 1,
                count,
                t0.elapsed().as_millis()
            ),
            None => eprintln!("failed to render {}", path.display()),
        }
        return;
    }

    // Scaffold + discover script actions, and test matching/execution.
    // Parse a canned sftp `ls -l` block (arg 2 = file), or live-list a server.
    if args.len() >= 2 && args[1] == "--sftp-parse-test" {
        let sample = "drwxr-xr-x    5 1000     1000         4096 Jul 30 10:00 my folder\n\
                      -rw-r--r--    1 1000     1000       123456 Jul 29 09:00 report.pdf\n\
                      lrwxrwxrwx    1 1000     1000           11 Jan  1  2025 link -> target.txt\n\
                      drwxr-xr-x    2 1000     1000         4096 Jul 30 10:00 .\n\
                      drwxr-xr-x    9 1000     1000         4096 Jul 30 10:00 ..\n\
                      sftp> ls -la /home/user";
        let fixture = args
            .get(2)
            .map(|path| fs::read_to_string(path).expect("Cannot read SFTP listing fixture"));
        for e in parse_sftp_ls(fixture.as_deref().unwrap_or(sample)) {
            eprintln!("  {:<14} dir={} size={}", e.name, e.is_dir, e.size);
        }
        return;
    }
    if args.len() >= 3 && args[1] == "--sftp-list" {
        // --sftp-list <[user@]host[:port]> [path] [password]
        let raw = args[2].clone();
        let (user, hostport) = raw.split_once('@').map_or(("", raw.as_str()), |(u, h)| (u, h));
        let (host, port) = hostport
            .rsplit_once(':')
            .filter(|(_, p)| p.chars().all(|c| c.is_ascii_digit()) && !p.is_empty())
            .map(|(h, p)| (h.to_string(), p.parse().unwrap_or(0)))
            .unwrap_or((hostport.to_string(), 0));
        let password = args.get(4).cloned();
        let srv = SftpServer {
            control_path: None,
            name: host.clone(),
            host,
            user: user.to_string(),
            port,
            key: String::new(),
            use_password: password.is_some(),
            auto_reopen: false,
        };
        if let Some(pw) = &password {
            keychain_set_password(&srv, pw);
            eprintln!("(password mode: stored in Keychain, using expect+ControlMaster)");
        }
        let path = args.get(3).cloned().unwrap_or_else(|| ".".into());
        match sftp_home(&srv, true) {
            Ok(h) => eprintln!("home: {h}"),
            Err(e) => eprintln!("home error: {e}"),
        }
        match sftp_list(&srv, &path, true) {
            Ok(es) => {
                eprintln!("{} entries:", es.len());
                for e in es.iter().take(20) {
                    eprintln!("  {:<30} dir={} size={}", e.name, e.is_dir, e.size);
                }
            }
            Err(e) => eprintln!("list error: {e}"),
        }
        return;
    }

    if args.len() >= 4 && args[1] == "--sftp-preview-test" {
        // --sftp-preview-test <[user@]host[:port]> <remote_path> [password]
        // Exercises the remote-preview pipeline headlessly: fetch → QuickLook.
        let raw = args[2].clone();
        let (user, hostport) = raw.split_once('@').map_or(("", raw.as_str()), |(u, h)| (u, h));
        let (host, port) = hostport
            .rsplit_once(':')
            .filter(|(_, p)| p.chars().all(|c| c.is_ascii_digit()) && !p.is_empty())
            .map(|(h, p)| (h.to_string(), p.parse().unwrap_or(0)))
            .unwrap_or((hostport.to_string(), 0));
        let password = args.get(4).cloned();
        let srv = SftpServer {
            control_path: None,
            name: host.clone(),
            host,
            user: user.to_string(),
            port,
            key: String::new(),
            use_password: password.is_some(),
            auto_reopen: false,
        };
        if let Some(pw) = &password {
            keychain_set_password(&srv, pw);
        }
        let remote = PathBuf::from(&args[3]);
        let local = remote_preview_temp(&remote);
        if let Some(parent) = local.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let r = remote.to_string_lossy().replace('"', "");
        let l = local.to_string_lossy().replace('"', "");
        eprintln!("fetching {r} -> {l}");
        match sftp_batch(&srv, &format!("get \"{r}\" \"{l}\""), true) {
            Ok(_) => {
                let bytes = fs::metadata(&local).map(|m| m.len()).unwrap_or(0);
                eprintln!("downloaded {bytes} bytes");
                match build_preview(&local) {
                    Some(img) => {
                        let sz = img.size(0);
                        eprintln!("build_preview OK: {}x{}", sz.width.0, sz.height.0);
                    }
                    None => eprintln!("build_preview FAILED"),
                }
                if is_pdf(&remote) {
                    match render_pdf_page(&local, 0) {
                        Some((img, count)) => {
                            let sz = img.size(0);
                            eprintln!("render_pdf_page OK: {}x{}, {count} page(s)", sz.width.0, sz.height.0);
                        }
                        None => eprintln!("render_pdf_page FAILED"),
                    }
                }
            }
            Err(e) => eprintln!("download error: {e}"),
        }
        return;
    }

    if args.len() >= 3 && args[1] == "--cloud-test" {
        use std::os::macos::fs::MetadataExt;
        let p = PathBuf::from(&args[2]);
        match fs::metadata(&p) {
            Ok(m) => {
                let flags = m.st_flags();
                eprintln!("st_flags = 0x{flags:08X}");
                eprintln!("SF_DATALESS set = {}", flags & SF_DATALESS != 0);
                eprintln!("is_dataless() = {}", is_dataless(&p));
                eprintln!("cloud_kind = {:?}", cloud_kind(&p).map(|k| match k {
                    CloudKind::ICloud => "iCloud",
                    CloudKind::Provider => "Provider",
                }));
            }
            Err(e) => eprintln!("stat error: {e}"),
        }
        return;
    }

    if args.len() >= 2 && args[1] == "--script-test" {
        let dir = ensure_scripts_dir();
        eprintln!("scripts dir: {:?}", dir);
        let acts = discover_script_actions();
        eprintln!("discovered {} action(s):", acts.len());
        for a in &acts {
            eprintln!(
                "  {:<20} types={:?}  applies(png={}, folder={})",
                a.name,
                a.types,
                script_action_applies(a, "photo.png", false),
                script_action_applies(a, "MyFolder", true),
            );
        }
        // Execute "Copy Path" (if present) on a sample path to confirm it runs.
        if let Some(cp) = acts.iter().find(|a| a.name == "Copy Path") {
            let out = Command::new(&cp.path)
                .arg("/tmp/shuffle-script-test.txt")
                .output();
            eprintln!("ran Copy Path -> exit {:?}", out.map(|o| o.status.code()));
        }
        return;
    }

    // Parse a filter query and print the operators + a few match checks.
    if args.len() >= 3 && args[1] == "--filter-test" {
        let fq = FilterQuery::parse(&args[2]);
        eprintln!("text={:?}", fq.text);
        eprintln!("kinds={}", fq.kinds.len());
        eprintln!("exts={:?}", fq.exts);
        eprintln!("size={}", fq.size.is_some());
        eprintln!("after={} before={}", fq.after.is_some(), fq.before.is_some());
        eprintln!("content={:?}", fq.content);
        // Match a few synthetic entries (name, is_dir, size, mtime=now).
        let now = Some(SystemTime::now());
        let cases: &[(&str, bool, u64)] = &[
            ("report.pdf", false, 2_000_000),
            ("photo.png", false, 500_000),
            ("notes.txt", false, 1_000),
            ("Projects", true, 0),
            ("archive.zip", false, 50_000_000),
        ];
        for (n, d, s) in cases {
            eprintln!("  {:>16}  match={}", n, fq.matches_entry(n, *d, *s, now));
        }
        return;
    }

    // Hidden timing bench: `shuffle --dir-bench <path>` reports how long each
    // load phase takes for a real folder.
    if args.len() >= 3 && args[1] == "--dir-bench" {
        let p = PathBuf::from(&args[2]);

        let t = std::time::Instant::now();
        let fast = read_entries_fast(&p);
        eprintln!(
            "read_entries_fast: {} entries in {} ms",
            fast.len(),
            t.elapsed().as_millis()
        );

        let t = std::time::Instant::now();
        let full = read_entries(&p);
        eprintln!(
            "read_entries (stat each): {} entries in {} ms",
            full.len(),
            t.elapsed().as_millis()
        );

        // Distinct file types (what prewarm actually builds).
        let mut types: HashSet<String> = HashSet::new();
        for e in &fast {
            if let Some(k) = icon_key(&p.join(&e.name)) {
                types.insert(k);
            }
        }
        eprintln!("distinct file types: {}", types.len());

        ensure_base_icons();
        // Separate iconForFile from TIFFRepresentation to see which is slow.
        for e in fast.iter().filter(|e| !e.is_dir).take(3) {
            let path = p.join(&e.name);
            let ps = path.to_str().unwrap();
            let ws = NSWorkspace::sharedWorkspace();
            let ns = NSString::from_str(ps);
            let t1 = std::time::Instant::now();
            let img = ws.iconForFile(&ns);
            let icon_ms = t1.elapsed().as_millis();
            let t2 = std::time::Instant::now();
            let _ = img.TIFFRepresentation();
            let tiff_ms = t2.elapsed().as_millis();
            eprintln!("{}: iconForFile {icon_ms} ms, TIFFRepresentation {tiff_ms} ms", e.name);
        }

        let t = std::time::Instant::now();
        let mut total_tiff = 0u128;
        let mut seen: HashSet<String> = HashSet::new();
        for e in &fast {
            if e.is_dir {
                continue;
            }
            let path = p.join(&e.name);
            if let Some(k) = icon_key(&path) {
                if !seen.insert(k) {
                    continue;
                }
                let t1 = std::time::Instant::now();
                let _ = icon_tiff(&path);
                total_tiff += t1.elapsed().as_millis();
            }
        }
        eprintln!(
            "all distinct icon TIFF fetches: {} types in {} ms (total wall {} ms)",
            seen.len(),
            total_tiff,
            t.elapsed().as_millis()
        );
        return;
    }

    let app = Application::new();
    // Re-open the window when the Dock icon is clicked after the last window was
    // closed (otherwise the app stays running with no way to show it).
    app.on_reopen(|cx: &mut App| {
        if cx.windows().is_empty() {
            open_main_window(cx);
            cx.activate(true);
        }
    });
    app.run(|cx: &mut App| {
        // Load the saved theme into both the render-side copy and the global.
        let saved_theme = load_theme();
        set_active_theme(saved_theme);
        cx.set_global(ThemeGlobal(saved_theme));

        // Load feature prefs (terminal / preview / info), all off by default.
        let saved_prefs = load_prefs();
        set_active_prefs(saved_prefs);
        cx.set_global(PrefsGlobal(saved_prefs));

        // Load key bindings.
        let saved_keymap = load_keymap();
        set_active_keymap(saved_keymap.clone());
        cx.set_global(KeymapGlobal(saved_keymap));

        // Shared update-check state (Settings drives it, the main window
        // installs from it).
        cx.set_global(UpdateCheckGlobal::default());

        // Load the icon pack (if any).
        let saved_pack = load_icon_pack();
        set_active_icon_pack(saved_pack.clone());
        cx.set_global(IconPackGlobal(saved_pack));

        // Load the menu style; unless the user chose menu colors, derive them
        // from the active theme so menus match (incl. light themes).
        let mut saved_menu = load_menu_style();
        saved_menu.follow_theme(&saved_theme);
        set_active_menu(saved_menu);
        cx.set_global(MenuStyleGlobal(saved_menu));

        // Load saved SFTP servers.
        let saved_servers = load_sftp_servers();
        set_active_sftp_servers(saved_servers.clone());
        cx.set_global(SftpServersGlobal(saved_servers));

        // Load the app-icon background and apply it to the Dock icon.
        let saved_icon_bg = load_icon_bg();
        set_active_icon_bg(saved_icon_bg.clone());
        refresh_dock_icon(&saved_icon_bg);

        // Menu bar: app menu with Settings + Quit, plus their shortcuts.
        cx.bind_keys([
            KeyBinding::new("cmd-,", OpenSettings, None),
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("cmd-shift-s", NewProject, None),
            KeyBinding::new("cmd-shift-d", ToggleStudioSplit, None),
            KeyBinding::new("cmd-shift-i", ToggleInspector, None),
        ]);
        cx.set_menus(vec![
            Menu {
                name: "Schnauz".into(),
                items: vec![
                    MenuItem::action("About Schnauz", OpenAbout),
                    MenuItem::separator(),
                    MenuItem::action("Settings…", OpenSettings),
                    MenuItem::separator(),
                    MenuItem::action("Quit Schnauz", Quit),
                ],
            },
            Menu {
                name: "File".into(),
                items: vec![
                    MenuItem::action("New Tab", NewTab),
                    MenuItem::action("New Folder", NewFolder),
                    MenuItem::action("New Project…", NewProject),
                    MenuItem::separator(),
                    MenuItem::action("Close Tab", CloseTab),
                    MenuItem::action("Move to Trash", MoveToTrash),
                ],
            },
            Menu {
                name: "View".into(),
                items: vec![
                    MenuItem::action("as List", ViewList),
                    MenuItem::action("as Icons", ViewIcons),
                    MenuItem::separator(),
                    MenuItem::action("Hide/Show Sidebar", ToggleSidebar),
                    MenuItem::action("Search…", FocusSearch),
                    MenuItem::action("Split / Join Panes", ToggleStudioSplit),
                    MenuItem::action("Show / Hide Preview", ToggleInspector),
                ],
            },
            Menu {
                name: "Go".into(),
                items: vec![
                    MenuItem::action("Back", GoBack),
                    MenuItem::action("Forward", GoForward),
                    MenuItem::separator(),
                    MenuItem::action("Home", GoHome),
                    MenuItem::action("Applications", GoApplications),
                    MenuItem::action("Computer", GoComputer),
                ],
            },
        ]);
        cx.on_action(|_: &Quit, cx: &mut App| cx.quit());
        cx.on_action(|_: &OpenSettings, cx: &mut App| open_settings_window(cx));
        cx.on_action(|_: &OpenAbout, cx: &mut App| about::open_about(cx));

        open_main_window(cx);
        cx.activate(true);
    });
}

/// Open the main explorer window.
fn open_main_window(cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(1280.0), px(800.0)), cx);
    let _ = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(980.0), px(600.0))),
            titlebar: Some(TitlebarOptions {
                // No title text; transparent so our own colored bar shows.
                title: None,
                appears_transparent: true,
                ..Default::default()
            }),
            ..Default::default()
        },
        |window, cx| {
            let view = cx.new(|cx| {
                let mut finder = Shuffle::new(load_last_dir(), cx);
                finder.prewarm_icons(cx);
                // Quietly check GitHub for a newer release (shows a banner if so).
                finder.check_for_update(cx);
                // Reopen last run's tabs + split (else the last folder), then
                // fill each pane's metadata in the background.
                finder.restore_session();
                finder.activate_project_sidebar(cx);
                finder.apply_all_folder_views();
                for pane in 0..finder.panes.len() {
                    finder.reload_pane(pane, cx);
                }
                // Reconnect any SFTP servers marked "reconnect on launch".
                for server in sftp_servers().into_iter().filter(|s| s.auto_reopen) {
                    finder.connect_sftp(server, cx);
                }
                finder
            });
            // Focus the root so it receives keystrokes (Cmd+P) immediately.
            window.focus(&view.read(cx).focus);
            view
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_bundles_keep_distinct_icons_and_file_visibility() {
        let dir = scratch("app-bundles");
        fs::create_dir(dir.join("Utilities")).unwrap();
        fs::create_dir_all(dir.join("Alpha.app/Contents")).unwrap();
        fs::create_dir_all(dir.join("Beta.APP/Contents")).unwrap();
        fs::write(dir.join("notes.txt"), "notes").unwrap();
        let mut tab = Tab::new(dir.clone());
        let names = |tab: &Tab| tab.visible_entries().map(|e| e.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&tab), ["Utilities", "Alpha.app", "Beta.APP", "notes.txt"]);
        for entries in [read_entries_fast(&dir), read_entries(&dir)] {
            for entry in entries.iter().filter(|e| has_app_extension(Path::new(&e.name))) {
                assert!(entry.is_dir, "copy and move must retain the whole bundle");
                assert!(!entry.is_folder());
                assert_eq!(kind_label(&entry.name, entry.is_dir), "Application");
                assert!(!KindClass::Folder.matches(&entry.name, entry.is_dir));
                assert!(KindClass::App.matches(&entry.name, entry.is_dir));
            }
        }
        tab.set_entry_filter(EntryFilter::Folders);
        assert_eq!(names(&tab), ["Utilities"]);
        tab.set_entry_filter(EntryFilter::Files);
        assert_eq!(names(&tab), ["Alpha.app", "Beta.APP", "notes.txt"]);
        tab.find_query = Some("kind:app".into());
        tab.recompute_results();
        assert_eq!(names(&tab), ["Alpha.app", "Beta.APP"]);

        let alpha = dir.join("Alpha.app");
        let beta = dir.join("Beta.APP");
        assert!(is_app_bundle(&alpha, true));
        assert!(!is_browsable_folder(&alpha, true));
        assert!(is_browsable_folder(&dir.join("Utilities"), true));
        assert!(!is_app_bundle(&alpha, false));
        let alpha_key = icon_key(&alpha).unwrap();
        assert_ne!(alpha_key, icon_key(&beta).unwrap());
        assert_ne!(alpha_key, icon_key(&dir.join("Other/Alpha.app")).unwrap());
        assert_eq!(icon_key(&dir.join("a.txt")), icon_key(&dir.join("b.TXT")));
        assert!(pack_icon_path(&alpha_key).is_none());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn type_filter_combines_with_search_sort_and_directory_reloads() {
        let dir = scratch("type-filter");
        fs::create_dir(dir.join("Reports")).unwrap();
        fs::create_dir(dir.join("Assets")).unwrap();
        fs::write(dir.join("report.txt"), "report").unwrap();
        fs::write(dir.join("photo.jpg"), "photo").unwrap();
        std::os::unix::fs::symlink("Reports", dir.join("Report link")).unwrap();
        let mut tab = Tab::new(dir.clone());
        let names = |tab: &Tab| tab.visible_entries().map(|e| e.name.clone()).collect::<Vec<_>>();
        assert_eq!(tab.visible_count(), 5);

        tab.selection.insert(dir.join("report.txt"));
        tab.anchor = Some(dir.join("report.txt"));
        tab.set_entry_filter(EntryFilter::Folders);
        assert_eq!(names(&tab), ["Assets", "Report link", "Reports"]);
        assert!(tab.selection.is_empty() && tab.anchor.is_none());

        tab.find_query = Some("report".into());
        tab.recompute_results();
        assert_eq!(tab.visible_count(), 2);
        assert!(tab.visible_entries().all(|e| e.is_dir));
        tab.set_entry_filter(EntryFilter::Files);
        assert_eq!(names(&tab), ["report.txt"]);
        // Closing text search keeps the type filter.
        tab.find_query = None;
        tab.recompute_results();
        assert_eq!(names(&tab), ["photo.jpg", "report.txt"]);

        tab.sort_key = SortKey::Name;
        tab.sort_asc = false;
        sort_entries(&mut tab.entries, tab.sort_key, tab.sort_asc);
        tab.recompute_results();
        assert_eq!(names(&tab), ["report.txt", "photo.jpg"]);
        fs::write(dir.join("z.txt"), "new").unwrap();
        tab.entries = read_entries_fast(&dir);
        sort_entries(&mut tab.entries, tab.sort_key, tab.sort_asc);
        tab.recompute_results();
        assert_eq!(names(&tab), ["z.txt", "report.txt", "photo.jpg"]);
        tab.find_query = Some("kind:folder".into());
        tab.recompute_results();
        assert_eq!(tab.visible_count(), 0);
        tab.find_query = None;
        tab.set_entry_filter(EntryFilter::All);
        assert_eq!(tab.visible_count(), 6);
        assert!(!tab.is_filtered());
        // Another tab has its own default and results.
        let other = Tab::new(dir.clone());
        tab.set_entry_filter(EntryFilter::Files);
        assert_eq!(other.entry_filter, EntryFilter::All);
        assert_eq!(other.visible_count(), 6);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn hidden_visibility_combines_with_type_and_text_filters() {
        let dir = scratch("hidden-filter");
        for name in ["Visible", ".secret folder"] { fs::create_dir(dir.join(name)).unwrap(); }
        for name in ["visible.txt", ".secret.txt", "Finder hidden.txt"] { fs::write(dir.join(name), "text").unwrap(); }
        let flagged = std::ffi::CString::new(dir.join("Finder hidden.txt").as_os_str().as_encoded_bytes()).unwrap();
        assert_eq!(unsafe { libc::chflags(flagged.as_ptr(), libc::UF_HIDDEN) }, 0);
        let mut tab = Tab::new(dir.clone());
        tab.entries = read_entries(&dir);
        tab.recompute_results();
        let names = |t: &Tab| t.visible_entries().map(|e| e.name.clone()).collect::<Vec<_>>();
        assert!(!tab.show_hidden);
        assert_eq!(names(&tab), ["Visible", "visible.txt"]);
        tab.show_hidden = true;
        tab.recompute_results();
        assert_eq!(tab.visible_count(), 5);
        tab.set_entry_filter(EntryFilter::Folders);
        assert_eq!(tab.visible_count(), 2);
        tab.show_hidden = false;
        tab.recompute_results();
        assert_eq!(names(&tab), ["Visible"]);
        tab.set_entry_filter(EntryFilter::Files);
        tab.find_query = Some("secret".into());
        tab.recompute_results();
        assert_eq!(tab.visible_count(), 0);
        tab.show_hidden = true;
        tab.recompute_results();
        assert_eq!(names(&tab), [".secret.txt"]);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn visibility_changes_keep_selected_files_that_remain_visible() {
        let dir = scratch("selection-visibility");
        fs::write(dir.join("keep.txt"), "visible").unwrap();
        fs::write(dir.join(".hidden.txt"), "hidden").unwrap();
        let mut tab = Tab::new(dir.clone());
        tab.show_hidden = true;
        tab.recompute_results();
        tab.selection.extend([dir.join("keep.txt"), dir.join(".hidden.txt")]);
        tab.anchor = Some(dir.join(".hidden.txt"));
        tab.show_hidden = false;
        tab.recompute_results();
        tab.retain_visible_selection();
        assert_eq!(tab.selection, HashSet::from([dir.join("keep.txt")]));
        assert_eq!(tab.anchor, Some(dir.join("keep.txt")));
        tab.view = ViewMode::Icons;
        tab.recompute_results();
        tab.retain_visible_selection();
        assert_eq!(tab.selection, HashSet::from([dir.join("keep.txt")]));
        assert_eq!(view_mode_from_key("columns"), Some(ViewMode::List));
        assert_eq!(view_mode_from_key("gallery"), Some(ViewMode::List));
        fs::remove_dir_all(dir).unwrap();
    }

    pub(super) fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("shuffle-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn batch_plan_modes_and_conflicts() {
        let d = scratch("batch");
        let a = d.join("IMG_001.jpg");
        let b = d.join("IMG_002.jpg");
        fs::write(&a, "a").unwrap();
        fs::write(&b, "b").unwrap();
        fs::write(d.join("taken.jpg"), "x").unwrap();
        let mut br = BatchRename {
            pane: 0,
            targets: vec![a.clone(), b.clone()],
            mode: BatchMode::Replace,
            a: "IMG_".into(),
            b: "Trip ".into(),
            field: 0,
            after: true,
        };
        let names: Vec<String> = br.plan().iter().map(|(_, t, _)| path_label(t)).collect();
        assert_eq!(names, ["Trip 001.jpg", "Trip 002.jpg"]);

        br.mode = BatchMode::Number;
        br.a = "Beach".into();
        br.b = "7".into();
        let names: Vec<String> = br.plan().iter().map(|(_, t, _)| path_label(t)).collect();
        assert_eq!(names, ["Beach 7.jpg", "Beach 8.jpg"]);

        br.mode = BatchMode::Add;
        br.a = "-v2".into();
        let names: Vec<String> = br.plan().iter().map(|(_, t, _)| path_label(t)).collect();
        assert_eq!(names, ["IMG_001-v2.jpg", "IMG_002-v2.jpg"]);

        // Everything → the same name, or onto an existing file: blocked.
        br.mode = BatchMode::Replace;
        br.a = "IMG_00".into();
        br.b = "".into();
        br.targets = vec![a.clone()];
        br.a = "IMG_001".into();
        br.b = "taken".into();
        assert!(br.plan()[0].2.is_some());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn transfer_copy_move_and_no_clobber() {
        let d = scratch("xfer");
        let src = d.join("src");
        fs::create_dir_all(src.join("sub")).unwrap();
        fs::write(src.join("sub/f.txt"), "hello").unwrap();
        std::os::unix::fs::symlink("sub/f.txt", src.join("link")).unwrap();
        let dst = d.join("dst");
        fs::create_dir(&dst).unwrap();

        let prog = JobProgress::default();
        let (done, err) = run_transfer(&[(src.clone(), dst.join("copy"))], TransferKind::Copy, &prog);
        assert!(err.is_none(), "{err:?}");
        assert_eq!(done.len(), 1);
        assert_eq!(fs::read_to_string(dst.join("copy/sub/f.txt")).unwrap(), "hello");
        assert!(fs::symlink_metadata(dst.join("copy/link")).unwrap().file_type().is_symlink());
        assert_eq!(prog.done.load(Ordering::Relaxed), 5);

        // Destination exists → skipped, nothing overwritten.
        let (done, err) = run_transfer(&[(src.clone(), dst.join("copy"))], TransferKind::Copy, &prog);
        assert!(done.is_empty() && err.is_some());

        // Same-volume move is a rename.
        let (done, _) = run_transfer(&[(src.clone(), dst.join("moved"))], TransferKind::Move, &prog);
        assert_eq!(done.len(), 1);
        assert!(!src.exists() && dst.join("moved/sub/f.txt").exists());

        // Cancelled before starting: nothing copied, no partial left behind.
        let cancelled = JobProgress::default();
        cancelled.cancel.store(true, Ordering::Relaxed);
        let (done, _) = run_transfer(&[(dst.join("moved"), dst.join("c2"))], TransferKind::Copy, &cancelled);
        assert!(done.is_empty() && !dst.join("c2").exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn mode_and_digits_formatting() {
        assert_eq!(mode_string(0o755, true), "drwxr-xr-x (755)");
        assert_eq!(mode_string(0o640, false), "-rw-r----- (640)");
        assert_eq!(group_digits(1234567), "1,234,567");
        assert_eq!(group_digits(12), "12");
    }

    #[test]
    fn session_keys_roundtrip() {
        for v in [ViewMode::List, ViewMode::Icons] {
            assert!(view_mode_from_key(view_mode_key(v)) == Some(v));
        }
        for k in [SortKey::None, SortKey::Name, SortKey::Kind, SortKey::Modified, SortKey::Created, SortKey::Size, SortKey::Extension, SortKey::Permissions] {
            assert!(sort_key_from_name(sort_key_name(k)) == Some(k));
        }
    }
}
