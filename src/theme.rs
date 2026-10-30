use super::save_theme;
use gpui::{rgba, App, Rgba};
use std::cell::RefCell;

/// A complete color theme. Every field is a 0xRRGGBB color.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct Theme {
    pub(super) bg: u32,            // app background
    pub(super) sidebar: u32,       // sidebar background
    pub(super) surface: u32,       // elevated surfaces (menus, panels, active nav)
    pub(super) hover: u32,         // row / item hover background (the "mouseover" color)
    pub(super) selected: u32,      // selected / highlighted background
    pub(super) border: u32,        // hairline borders
    pub(super) border_strong: u32, // stronger dividers
    pub(super) text: u32,          // primary text
    pub(super) text_muted: u32,    // secondary text (kind / date / size)
    pub(super) text_dim: u32,      // section headers, placeholders
    pub(super) accent: u32,        // folders, carets, active highlights
}

impl Theme {
    /// Keep surfaces and dividers coherent when changing the window background.
    /// A move between light/dark also restores readable foregrounds.
    pub(super) fn with_background(self, bg: u32) -> Self {
        let light = Self { bg, ..self }.is_light();
        let mut derived = if light {
            light_theme(bg, self.text, self.accent)
        } else {
            dark_theme(bg, self.text, self.accent)
        };
        if light == self.is_light() {
            derived.text_muted = self.text_muted;
            derived.text_dim = self.text_dim;
        } else {
            let stock = PRESETS[if light { 0 } else { 1 }].1;
            derived.text = stock.text;
            derived.text_muted = stock.text_muted;
            derived.text_dim = stock.text_dim;
            derived.accent = stock.accent;
        }
        derived
    }

    pub(super) fn with_text(self, text: u32) -> Self {
        // Keep the black-label light-mode convention. Dark secondary labels
        // recede toward the background without leaving old palette colours.
        let secondary = |percent| blend(text, self.bg, percent);
        Self {
            text,
            text_muted: if self.is_light() { text } else { secondary(75) },
            text_dim: if self.is_light() { text } else { secondary(60) },
            ..self
        }
    }

    pub(super) fn is_light(self) -> bool {
        let r = (self.bg >> 16) & 0xff;
        let g = (self.bg >> 8) & 0xff;
        let b = self.bg & 0xff;
        r * 299 + g * 587 + b * 114 >= 128_000
    }

    /// A translucent variant of a base color, for floating panels. `a` is 0..=255.
    pub(super) fn alpha(color: u32, a: u32) -> Rgba {
        rgba((color << 8) | (a & 0xff))
    }
}

fn blend(foreground: u32, background: u32, percent: u32) -> u32 {
    let channel = |shift: u32| -> u32 {
        (((foreground >> shift) & 255) * percent
            + ((background >> shift) & 255) * (100 - percent)) / 100
    };
    (channel(16) << 16) | (channel(8) << 8) | channel(0)
}

impl Default for Theme {
    fn default() -> Self {
        PRESETS[0].1
    }
}

/// Upgrade the former default's grey foregrounds without changing custom or
/// dark palettes. Existing saved defaults should receive the new black text too.
pub(super) fn upgrade_saved_foreground(mut saved: Theme) -> Theme {
    let mut previous = Theme::default();
    previous.text = 0x272a3c;
    previous.text_muted = 0x626980;
    previous.text_dim = 0x7b8298;
    if saved == previous {
        saved.text = 0x000000;
        saved.text_muted = 0x000000;
        saved.text_dim = 0x000000;
    }
    saved
}

/// Clamp a channel value to 0..=255.
pub(super) const fn clamp8(x: u32) -> u32 {
    if x > 0xff {
        0xff
    } else {
        x
    }
}

/// Scale every channel of an 0xRRGGBB color by `num/den` (lighten if >1, darken
/// if <1), clamping each channel. Used to derive coherent shades from one base.
pub(super) const fn scale(c: u32, num: u32, den: u32) -> u32 {
    let r = clamp8(((c >> 16) & 0xff) * num / den);
    let g = clamp8(((c >> 8) & 0xff) * num / den);
    let b = clamp8((c & 0xff) * num / den);
    (r << 16) | (g << 8) | b
}

/// Build a coherent DARK theme from a background, text, and accent color.
/// Surfaces are derived lighter than the background; muted/dim text darker.
pub(super) const fn dark_theme(bg: u32, text: u32, accent: u32) -> Theme {
    Theme {
        bg,
        sidebar: scale(bg, 8, 10),
        surface: scale(bg, 15, 10),
        hover: scale(bg, 13, 10),
        selected: scale(bg, 19, 10),
        border: scale(bg, 14, 10),
        border_strong: scale(bg, 20, 10),
        text,
        text_muted: scale(text, 7, 10),
        text_dim: scale(text, 5, 10),
        accent,
    }
}

/// Build a coherent LIGHT theme. Surfaces are derived darker than the (light)
/// background; muted/dim text lighter so it recedes.
pub(super) const fn light_theme(bg: u32, text: u32, accent: u32) -> Theme {
    Theme {
        bg,
        sidebar: scale(bg, 95, 100),
        surface: scale(bg, 90, 100),
        hover: scale(bg, 93, 100),
        selected: scale(bg, 84, 100),
        border: scale(bg, 88, 100),
        border_strong: scale(bg, 80, 100),
        text,
        text_muted: scale(text, 13, 10),
        text_dim: scale(text, 16, 10),
        accent,
    }
}

/// Built-in palettes shown in Settings — a broad spread across hues, dark and
/// light. (name, theme)
pub(super) const PRESETS: &[(&str, Theme)] = &[
    (
        "Schnauz Light",
        Theme {
            bg: 0xfafbfd,
            sidebar: 0xf1f2f7,
            surface: 0xffffff,
            hover: 0xeceef6,
            selected: 0xe7e8fb,
            border: 0xe0e3ed,
            border_strong: 0xcbd0e0,
            text: 0x000000,
            text_muted: 0x000000,
            text_dim: 0x000000,
            accent: 0x5a5de6,
        },
    ),
    (
        "Schnauz Dark",
        Theme {
            bg: 0x171923,
            sidebar: 0x1d202d,
            surface: 0x262a3b,
            hover: 0x2b3043,
            selected: 0x343758,
            border: 0x303549,
            border_strong: 0x424966,
            text: 0xe8eaf5,
            text_muted: 0xa4abc4,
            text_dim: 0x8c95b3,
            accent: 0x9296ff,
        },
    ),
    // --- Additional palettes ---
    (
        "Schnauz Classic Dark",
        Theme {
            bg: 0x1e1e22,
            sidebar: 0x17171a,
            surface: 0x2f2f3a,
            hover: 0x2a2a30,
            selected: 0x33334a,
            border: 0x303036,
            border_strong: 0x3a3a44,
            text: 0xf0f0f4,
            text_muted: 0x8a8a92,
            text_dim: 0x6b6b73,
            accent: 0x7aa2f7,
        },
    ),
    (
        "Catppuccin Mocha",
        Theme {
            bg: 0x1e1e2e,
            sidebar: 0x181825,
            surface: 0x313244,
            hover: 0x2a2b3c,
            selected: 0x45475a,
            border: 0x313244,
            border_strong: 0x45475a,
            text: 0xcdd6f4,
            text_muted: 0xa6adc8,
            text_dim: 0x6c7086,
            accent: 0x89b4fa,
        },
    ),
    (
        "Catppuccin Macchiato",
        Theme {
            bg: 0x24273a,
            sidebar: 0x1e2030,
            surface: 0x363a4f,
            hover: 0x2e3148,
            selected: 0x494d64,
            border: 0x363a4f,
            border_strong: 0x494d64,
            text: 0xcad3f5,
            text_muted: 0xa5adcb,
            text_dim: 0x6e738d,
            accent: 0x8aadf4,
        },
    ),
    (
        "Catppuccin Frappé",
        Theme {
            bg: 0x303446,
            sidebar: 0x292c3c,
            surface: 0x414559,
            hover: 0x3a3e52,
            selected: 0x51576d,
            border: 0x414559,
            border_strong: 0x51576d,
            text: 0xc6d0f5,
            text_muted: 0xa5adce,
            text_dim: 0x737994,
            accent: 0x8caaee,
        },
    ),
    (
        "Catppuccin Latte",
        Theme {
            bg: 0xeff1f5,
            sidebar: 0xe6e9ef,
            surface: 0xccd0da,
            hover: 0xdce0e8,
            selected: 0xbcc0cc,
            border: 0xccd0da,
            border_strong: 0xbcc0cc,
            text: 0x4c4f69,
            text_muted: 0x6c6f85,
            text_dim: 0x9ca0b0,
            accent: 0x1e66f5,
        },
    ),
    // --- Popular dark themes (varied accent hues) ---
    ("Dracula", dark_theme(0x282a36, 0xf8f8f2, 0xbd93f9)),
    ("Nord", dark_theme(0x2e3440, 0xe5e9f0, 0x88c0d0)),
    ("Tokyo Night", dark_theme(0x1a1b26, 0xc0caf5, 0x7aa2f7)),
    ("Gruvbox Dark", dark_theme(0x282828, 0xebdbb2, 0xfe8019)),
    ("One Dark", dark_theme(0x282c34, 0xabb2bf, 0x61afef)),
    ("Solarized Dark", dark_theme(0x002b36, 0x93a1a1, 0x268bd2)),
    ("Monokai", dark_theme(0x272822, 0xf8f8f2, 0xa6e22e)),
    ("Everforest", dark_theme(0x2d353b, 0xd3c6aa, 0xa7c080)),
    ("Rosé Pine", dark_theme(0x191724, 0xe0def4, 0xeb6f92)),
    // --- Bold single-hue darks (green / red / blue / …) ---
    ("Forest", dark_theme(0x10211a, 0xd6f5e3, 0x4ade80)),
    ("Crimson", dark_theme(0x211315, 0xf8dcdc, 0xf87171)),
    ("Ocean", dark_theme(0x0e1a26, 0xd6ecff, 0x38bdf8)),
    ("Grape", dark_theme(0x1a1326, 0xece0f8, 0xc084fc)),
    ("Amber", dark_theme(0x221a10, 0xf8ecd6, 0xfbbf24)),
    ("Rose", dark_theme(0x241620, 0xf8dcee, 0xf472b6)),
    ("Teal", dark_theme(0x0e201e, 0xd6f5f0, 0x2dd4bf)),
    ("Sunset", dark_theme(0x241712, 0xfae5d8, 0xfb7185)),
    // --- Light themes ---
    ("Solarized Light", light_theme(0xfdf6e3, 0x586e75, 0x268bd2)),
    ("GitHub Light", light_theme(0xffffff, 0x24292f, 0x0969da)),
    ("Rosé Pine Dawn", light_theme(0xfaf4ed, 0x575279, 0xd7827e)),
    ("Mint", light_theme(0xf0faf4, 0x14532d, 0x16a34a)),
    ("Sky", light_theme(0xeff6ff, 0x1e3a5f, 0x2563eb)),
    ("Lavender", light_theme(0xf6f2fe, 0x4c3a66, 0x8b5cf6)),
];

/// Wrapper so the theme lives in the GPUI global store and notifies observers.
#[derive(Clone, Copy)]
pub(super) struct ThemeGlobal(pub(super) Theme);
impl gpui::Global for ThemeGlobal {}

thread_local! {
    /// The render-side copy of the active theme, read by all draw code on the
    /// main thread without needing an `App` handle.
    static ACTIVE_THEME: RefCell<Theme> = RefCell::new(Theme::default());
}

/// The active theme. Read this anywhere in render code.
pub(super) fn theme() -> Theme {
    ACTIVE_THEME.with(|t| *t.borrow())
}

pub(super) fn set_active_theme(t: Theme) {
    ACTIVE_THEME.with(|c| *c.borrow_mut() = t);
}

/// Apply a theme everywhere: update the render-side copy, persist it, and store
/// it in the global so observers (the main window) repaint.
pub(super) fn apply_theme(t: Theme, cx: &mut App) {
    set_active_theme(t);
    save_theme(&t);
    cx.set_global(ThemeGlobal(t));
    cx.refresh_windows();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_default_gets_black_foregrounds_without_replacing_custom_or_dark_palettes() {
        let mut previous = Theme::default();
        previous.text = 0x272a3c;
        previous.text_muted = 0x626980;
        previous.text_dim = 0x7b8298;
        let upgraded = upgrade_saved_foreground(previous);
        assert!(upgraded == Theme::default());
        assert_eq!((upgraded.text, upgraded.text_muted, upgraded.text_dim), (0, 0, 0));
        for (_, preset) in PRESETS.iter().skip(1) {
            assert!(upgrade_saved_foreground(*preset) == *preset);
        }
        previous.text = 0x123456;
        assert!(upgrade_saved_foreground(previous) == previous);
    }
}
