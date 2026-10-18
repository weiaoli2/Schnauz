//! Settings view and its UI helpers, separate from browser and file operations.
use super::*;

/// Which theme color a hex field edits.
#[derive(Clone, Copy, PartialEq)]
enum ColorTarget {
    Bg,
    Text,
    Hover,
}

/// The Settings window: a tabbed customization surface.
struct Settings {
    tab: usize,
    /// The section within the active tab the rail last jumped to (highlighted
    /// in the nested sidebar list).
    section: usize,
    /// Scrolls the content pane so a rail sub-item can jump to its section.
    content_scroll: ScrollHandle,
    focus: FocusHandle,
    /// The action whose keystroke is currently being recorded, if any.
    recording: Option<KeyAction>,
    recording_quick: Option<String>,
    quick_actions: Vec<(String, String)>,
    keybind_error: Option<String>,
    file_access_error: Option<String>,
    /// In-progress hex color entry: (which color, typed hex digits).
    color_edit: Option<(ColorTarget, String)>,
}

/// Section names shown as nested rail items for a settings tab (and the order
/// they appear in the content pane, after the page header at child index 0).
pub(super) fn tab_sections(tab: usize) -> &'static [&'static str] {
    match tab {
        0 => &[
            "Explorer",
            "File Access",
            "Inspector",
            "Search",
            "Sidebar",
            "Connections",
            "Script Actions",
            "Version",
            "Developer",
        ],
        2 => &["Theme", "Colors"],
        _ => &[],
    }
}

impl Settings {
    fn drive_threshold_row(&self, critical: bool, cx: &Context<Self>) -> AnyElement {
        let p = prefs();
        let value = if critical { p.drive_critical } else { p.drive_warning };
        let mut controls = div().flex_none().flex().items_center().gap_2();
        for (index, delta, label) in [(0usize, -1i16, "−"), (1, 1, "+")] {
            controls = controls.child(div().id((if critical { "drive-red" } else { "drive-yellow" }, index))
                .px_2().py_1().rounded_md().cursor_pointer().bg(rgb(theme().surface))
                .hover(|d| d.bg(rgb(theme().hover))).child(label)
                .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                    let mut p = prefs();
                    if critical {
                        p.drive_critical = (i16::from(p.drive_critical) + delta).clamp(i16::from(p.drive_warning) + 1, 99) as u8;
                    } else {
                        p.drive_warning = (i16::from(p.drive_warning) + delta).clamp(1, i16::from(p.drive_critical) - 1) as u8;
                    }
                    apply_prefs(p, cx); cx.notify();
                })));
        }
        div().flex().items_center().justify_between().gap_4()
            .child(div().flex_1().child(if critical { "Red above" } else { "Yellow from" }))
            .child(div().text_color(rgb(if critical { 0xc23a35 } else { 0x9a6700 })).child(format!("{value}% used")))
            .child(controls).into_any_element()
    }

    fn new(cx: &mut Context<Self>) -> Self {
        cx.observe_global::<todoist::Global>(|_, cx| cx.notify()).detach();
        cx.observe_global::<nextcloud::State>(|_, cx| cx.notify())
            .detach();
        Settings {
            tab: 0,
            section: 0,
            content_scroll: ScrollHandle::new(),
            focus: cx.focus_handle(),
            recording: None,
            recording_quick: None,
            quick_actions: quick_actions::assignable(),
            keybind_error: None,
            file_access_error: None,
            color_edit: None,
        }
    }

    /// Route key events: color entry first, then keybind recording.
    fn handle_settings_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        if self.color_edit.is_some() {
            self.handle_hex_key(ev, cx);
        } else if self.recording.is_some() || self.recording_quick.is_some() {
            self.handle_keybind_key(ev, cx);
        }
    }

    /// Capture typed hex digits for a color field; Enter applies, Esc cancels.
    fn handle_hex_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let Some((target, _)) = self.color_edit.clone() else {
            return;
        };
        match ev.keystroke.key.as_str() {
            "escape" => {
                self.color_edit = None;
                cx.notify();
            }
            "backspace" => {
                if let Some((_, s)) = self.color_edit.as_mut() {
                    s.pop();
                }
                cx.notify();
            }
            "enter" => {
                if let Some((_, s)) = &self.color_edit {
                    if let Ok(c) = u32::from_str_radix(s, 16) {
                        match target {
                            ColorTarget::Bg => {
                                let mut nt = theme();
                                nt.bg = c;
                                apply_theme(nt, cx);
                            }
                            ColorTarget::Text => {
                                let mut nt = theme();
                                nt.text = c;
                                apply_theme(nt, cx);
                            }
                            ColorTarget::Hover => {
                                let mut nt = theme();
                                nt.hover = c;
                                apply_theme(nt, cx);
                            }

                        }
                    }
                }
                self.color_edit = None;
                cx.notify();
            }
            _ => {
                if let Some(ch) = ev.keystroke.key_char.as_ref() {
                    // Accept up to 6 hex digits.
                    if let Some((_, s)) = self.color_edit.as_mut() {
                        for c in ch.chars() {
                            if c.is_ascii_hexdigit() && s.len() < 6 {
                                s.push(c.to_ascii_lowercase());
                            }
                        }
                    }
                    cx.notify();
                }
            }
        }
    }

    /// Capture the next keystroke for the action being rebound.
    fn handle_keybind_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        if let Some(id) = self.recording_quick.clone() {
            let key = ev.keystroke.key.as_str();
            let mut km = keymap();
            match key {
                "escape" => { self.recording_quick = None; }
                "backspace" | "delete" => { km.quick.remove(&id); apply_keymap(km, cx); self.recording_quick = None; }
                "cmd" | "ctrl" | "alt" | "shift" | "function" => return,
                _ => {
                    let binding = canon_keystroke(&ev.keystroke);
                    if let Err(error) = quick_actions::validate_binding(&id, &binding, &km) {
                        self.keybind_error = Some(error); cx.notify(); return;
                    }
                    km.quick.insert(id, binding); apply_keymap(km, cx); self.recording_quick = None;
                }
            }
            self.keybind_error = None; cx.notify(); return;
        }
        let Some(action) = self.recording else {
            return;
        };
        let ks = &ev.keystroke;
        let key = ks.key.as_str();
        match key {
            "escape" => {}
            "backspace" | "delete" => {
                let mut km = keymap();
                km.set(action, None);
                apply_keymap(km, cx);
            }
            // Ignore lone modifier presses; wait for a real key.
            "cmd" | "ctrl" | "alt" | "shift" | "function" => return,
            _ => {
                let mut km = keymap();
                let binding = canon_keystroke(ks);
                if km.quick.values().any(|key| *key == binding) {
                    self.keybind_error = Some("Already assigned to a Quick Action. Clear that shortcut first.".into());
                    cx.notify(); return;
                }
                km.set(action, Some(binding));
                apply_keymap(km, cx);
            }
        }
        self.recording = None;
        cx.notify();
    }
}

impl Render for Settings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme();
        let tabs = [
            ("General", "\u{2699}"),
            ("Keybinds", "\u{2318}"),
            ("Customization", "\u{25d0}"),
        ];

        // Left tab rail.
        let mut tab_items: Vec<AnyElement> = Vec::new();
        for (i, (name, glyph)) in tabs.iter().enumerate() {
            let active = i == self.tab;
            tab_items.push(
                div()
                    .id(("tab", i))
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .mx_2()
                    .rounded_md()
                    .cursor_pointer()
                    .text_color(if active {
                        rgb(t.text)
                    } else {
                        rgb(t.text_muted)
                    })
                    .when(active, |s| s.bg(rgb(t.surface)))
                    .when(!active, |s| s.hover(|s| s.bg(rgb(t.hover))))
                    .child(
                        div()
                            .w(px(16.0))
                            .flex()
                            .justify_center()
                            .text_color(if active {
                                rgb(t.accent)
                            } else {
                                rgb(t.text_dim)
                            })
                            .child(glyph.to_string()),
                    )
                    .child(name.to_string())
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.tab = i;
                        this.section = 0;
                        this.content_scroll.set_offset(point(px(0.0), px(0.0)));
                        cx.notify();
                    }))
                    .into_any_element(),
            );

            // Nested section links under the active tab — click to jump.
            if i == self.tab {
                for (si, sname) in tab_sections(i).iter().enumerate() {
                    let active_sec = si == self.section;
                    tab_items.push(
                        div()
                            .id(("sec", i * 100 + si))
                            .flex()
                            .items_center()
                            .py_1()
                            .mx_2()
                            .pl(px(34.0))
                            .pr_2()
                            .rounded_md()
                            .cursor_pointer()
                            .text_size(px(UI_SMALL_FONT_SIZE))
                            .text_color(if active_sec {
                                rgb(t.text)
                            } else {
                                rgb(t.text_muted)
                            })
                            .when(active_sec, |s| s.bg(rgb(t.hover)))
                            .when(!active_sec, |s| s.hover(|s| s.bg(rgb(t.hover))))
                            .child(sname.to_string())
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.section = si;
                                this.content_scroll.scroll_to_top_of_item(si + 1);
                                cx.notify();
                            }))
                            .into_any_element(),
                    );
                }
            }
        }

        let rail = div()
            .flex_none()
            .w(px(184.0))
            .h_full()
            .pt_3()
            .flex()
            .flex_col()
            .gap_1()
            .bg(rgb(t.sidebar))
            .border_r_1()
            .border_color(rgb(t.border))
            .children(tab_items);

        let children = match self.tab {
            0 => self.render_general(cx),
            1 => self.render_keybinds(cx),
            _ => self.render_customization(cx),
        };

        let body = div()
            .flex()
            .flex_1()
            .min_h_0()
            .w_full()
            .bg(rgb(t.bg))
            .text_size(px(UI_FONT_SIZE))
            .text_color(rgb(t.text))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                this.handle_settings_key(ev, cx);
            }))
            .child(rail)
            .child(
                // Relative wrapper so the scrollbar thumb can overlay the
                // scrolling content at the right edge.
                div()
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(
                        // The section blocks are direct children so a rail link
                        // can scroll_to_top_of_item(section_index + 1) to jump.
                        div()
                            .id("settings-content")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.content_scroll)
                            .flex()
                            .flex_col()
                            .gap_4()
                            .p_5()
                            .children(children),
                    )
                    .children(static_scrollbar_thumb(&self.content_scroll)),
            );

        div().flex().flex_col().size_full().bg(rgb(t.bg)).text_color(rgb(t.text))
            .child(
                div().flex_none().w_full().h(px(TITLEBAR_H)).flex().items_center()
                    .pl(px(88.)).pr_4().bg(rgb(t.sidebar))
                    .border_b_1().border_color(rgb(t.border))
                    .text_size(px(14.)).font_weight(FontWeight::SEMIBOLD)
                    .child("Schnauz · Settings"),
            )
            .child(body)
    }
}

impl Settings {
    /// The General tab as an ordered list of blocks: page header, then one card
    /// per section (matching `tab_sections(0)`), then the reset button. Returned
    /// as separate children so the rail can scroll to any section.
    fn render_general(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let p = prefs();
        let t = theme();
        let script_count = discover_script_actions().len();

        let scripts_folder_row = div()
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_color(rgb(t.text)).child("Scripts folder"))
                    .child(
                        div()
                            .text_size(px(UI_SMALL_FONT_SIZE))
                            .text_color(rgb(t.text_muted))
                            .child(format!(
                                "{} script{} loaded — the README there explains the format.",
                                script_count,
                                if script_count == 1 { "" } else { "s" }
                            )),
                    ),
            )
            .child(
                div()
                    .id("open-scripts")
                    .flex_none()
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .bg(rgb(t.surface))
                    .text_color(rgb(t.text))
                    .hover(|s| s.bg(rgb(t.hover)))
                    .active(|s| s.bg(rgb(t.selected)))
                    .child("Open Scripts Folder")
                    .on_click(cx.listener(|_, _: &ClickEvent, _, _| {
                        if let Some(dir) = ensure_scripts_dir() {
                            let _ = Command::new("open").arg(&dir).spawn();
                        }
                    })),
            )
            .into_any_element();

        // Cloud/task accounts, followed by SSH auth and saved servers.
        let mut connection_rows: Vec<AnyElement> = Vec::new();
        connection_rows.extend(nextcloud::settings_rows(cx));
        connection_rows.extend(todoist_ui::settings_rows(cx));
        connection_rows.push(
            toggle_row(
                "tg-ssh-system",
                "Use my ~/.ssh configuration",
                "On: connect using your existing SSH keys, ~/.ssh/config aliases, and \
                 known_hosts — nothing is stored in the app. Off: use the private-key \
                 path you set per server.",
                p.ssh_use_system,
                cx.listener(|_, _: &ClickEvent, _, cx| {
                    let mut np = prefs();
                    np.ssh_use_system = !np.ssh_use_system;
                    np.ssh_configured = true;
                    apply_prefs(np, cx);
                    cx.notify();
                }),
            )
            .into_any_element(),
        );
        let servers = sftp_servers();
        if servers.is_empty() {
            connection_rows.push(
                div()
                    .text_size(px(UI_SMALL_FONT_SIZE))
                    .text_color(rgb(t.text_dim))
                    .child(
                        "No saved servers in this project. Add one from the sidebar: Connect to \
                         Server\u{2026} then enter sftp://user@host (or an ~/.ssh alias).",
                    )
                    .into_any_element(),
            );
        } else {
            for (i, s) in servers.iter().enumerate() {
                let srv = s.clone();
                connection_rows.push(
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_4()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(div().text_color(rgb(t.text)).child(s.name.clone()))
                                .child(
                                    div()
                                        .text_size(px(UI_SMALL_FONT_SIZE))
                                        .text_color(rgb(t.text_muted))
                                        .child(format!("sftp://{}", s.display())),
                                ),
                        )
                        .child(
                            div()
                                .id(("sftp-remove", i))
                                .flex_none()
                                .px_3()
                                .py_1()
                                .rounded_md()
                                .cursor_pointer()
                                .bg(rgb(t.hover))
                                .text_color(rgb(t.text))
                                .hover(|s| s.border_color(rgb(0xd9544f)).text_color(rgb(0xd9544f)))
                                .border_1()
                                .border_color(rgb(t.border))
                                .child("Remove")
                                .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                                    let list: Vec<SftpServer> = sftp_servers()
                                        .into_iter()
                                        .filter(|x| x.display() != srv.display())
                                        .collect();
                                    apply_sftp_servers(list, cx);
                                    cx.notify();
                                })),
                        )
                        .into_any_element(),
                );
            }
        }

        vec![
            settings_header(
                "General",
                "How the explorer behaves and what the side panels show.",
            )
            .into_any_element(),
            settings_section(
                "Explorer",
                Some("Navigation and the file list."),
                vec![
                    toggle_row(
                        "tg-show-parent",
                        "Show \u{201c}..\u{201d} (up one level)",
                        "Show the leading \u{201c}..\u{201d} entry in list view that navigates to the parent folder.",
                        p.show_parent,
                        cx.listener(|_, _: &ClickEvent, _, cx| {
                            let mut np = prefs();
                            np.show_parent = !np.show_parent;
                            apply_prefs(np, cx);
                            cx.notify();
                        }),
                    )
                    .into_any_element(),
                    toggle_row(
                        "tg-filter-button",
                        "Filter button",
                        "Show the always-on \u{201c}Filter\u{201d} button in the bottom-right corner. Pressing / still opens the filter either way.",
                        p.show_filter_button,
                        cx.listener(|_, _: &ClickEvent, _, cx| {
                            let mut np = prefs();
                            np.show_filter_button = !np.show_filter_button;
                            apply_prefs(np, cx);
                            cx.notify();
                        }),
                    )
                    .into_any_element(),
                ],
            ),
            settings_section(
                "File Access",
                Some(file_access::HELP),
                vec![div().flex().flex_col().gap_3()
                    .child(div().text_size(px(UI_SMALL_FONT_SIZE)).text_color(rgb(t.text)).child(file_access::STEPS))
                    .child(div().flex().child(div().id("settings-file-access")
                        .px_3().py_2().rounded_md().cursor_pointer()
                        .bg(rgb(t.hover)).text_color(rgb(t.text))
                        .hover(|s| s.bg(rgb(t.selected))).child("Open System Settings")
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.file_access_error = file_access::open_settings().err();
                            cx.notify();
                        }))))
                    .children(self.file_access_error.clone().map(|error| div()
                        .text_size(px(UI_SMALL_FONT_SIZE)).text_color(rgb(0xc23a35)).child(error)))
                    .into_any_element()],
            ),
            settings_section(
                "Inspector",
                Some("The side panel shown when you click a file."),
                vec![
                    toggle_row(
                        "tg-preview",
                        "Preview",
                        "Click a file once to preview it (images, PDFs, documents, \u{2026}) in the inspector panel.",
                        p.preview,
                        cx.listener(|_, _: &ClickEvent, _, cx| {
                            let mut np = prefs();
                            np.preview = !np.preview;
                            apply_prefs(np, cx);
                            cx.notify();
                        }),
                    )
                    .into_any_element(),
                    toggle_row(
                        "tg-preview-pages",
                        "Preview page controls",
                        "Show \u{2039} \u{203a} arrows under multi-page PDF previews to flip through the pages without opening the file.",
                        p.preview_pages,
                        cx.listener(|_, _: &ClickEvent, _, cx| {
                            let mut np = prefs();
                            np.preview_pages = !np.preview_pages;
                            apply_prefs(np, cx);
                            cx.notify();
                        }),
                    )
                    .into_any_element(),
                    toggle_row(
                        "tg-info",
                        "Information",
                        "Click a file once to see its details (kind, size, dates, dimensions, color space, and more) in the inspector panel.",
                        p.info,
                        cx.listener(|_, _: &ClickEvent, _, cx| {
                            let mut np = prefs();
                            np.info = !np.info;
                            apply_prefs(np, cx);
                            cx.notify();
                        }),
                    )
                    .into_any_element(),
                ],
            ),
            settings_section(
                "Search",
                Some("Find files by name or text with Cmd+P."),
                vec![
                    toggle_row(
                        "tg-palette-history",
                        "Search history",
                        "Remember submitted file searches. Use the configured history shortcuts to revisit them.",
                        p.palette_history,
                        cx.listener(|_, _: &ClickEvent, _, cx| {
                            let mut np = prefs();
                            np.palette_history = !np.palette_history;
                            apply_prefs(np, cx);
                            cx.notify();
                        }),
                    )
                    .into_any_element(),
                ],
            ),
            settings_section(
                "Sidebar",
                Some("Devices appear above Projects. Their labels show free space; colours reflect used capacity."),
                vec![
                    self.drive_threshold_row(false, cx),
                    self.drive_threshold_row(true, cx),
                    toggle_row(
                        "tg-groups",
                        "Sidebar groups",
                        "Create custom groups of files/folders in the sidebar. Right-click the sidebar to make a group, then right-click any item to add it.",
                        p.groups_enabled,
                        cx.listener(|_, _: &ClickEvent, _, cx| {
                            let mut np = prefs();
                            np.groups_enabled = !np.groups_enabled;
                            apply_prefs(np, cx);
                            cx.notify();
                        }),
                    )
                    .into_any_element(),
                    toggle_row(
                        "tg-waterfall",
                        "Waterfall folder tree",
                        "Show an expandable tree of the current folder's subfolders at the bottom of the sidebar. Click a triangle to drill in, click a name to open it in the pane.",
                        p.waterfall,
                        cx.listener(|_, _: &ClickEvent, _, cx| {
                            let mut np = prefs();
                            np.waterfall = !np.waterfall;
                            apply_prefs(np, cx);
                            cx.notify();
                        }),
                    )
                    .into_any_element(),

                ],
            ),
            settings_section(
                "Connections",
                Some("Saved SFTP servers belong to the current project."),
                connection_rows,
            ),
            settings_section(
                "Script Actions",
                Some("Run your own shell scripts from the right-click menu."),
                vec![
                    toggle_row(
                        "tg-script-actions",
                        "Run script actions",
                        "Show your own shell scripts (from the Scripts folder) in the right-click menu for matching files, and run them with the selected paths. Only scripts you put there run \u{2014} treat it like your ~/bin.",
                        p.script_actions,
                        cx.listener(|_, _: &ClickEvent, _, cx| {
                            let mut np = prefs();
                            np.script_actions = !np.script_actions;
                            if np.script_actions {
                                ensure_scripts_dir();
                            }
                            apply_prefs(np, cx);
                            cx.notify();
                        }),
                    )
                    .into_any_element(),
                    scripts_folder_row,
                ],
            ),
            settings_section(
                "Version",
                None,
                vec![self.render_version_row()],
            ),
            settings_section(
                "Developer",
                Some("Diagnostics for checking performance."),
                vec![toggle_row(
                    "tg-show-fps",
                    "Frame rate meter",
                    "Show a live FPS readout in the top-right of the explorer window. Forces continuous redraws to measure them, so it uses some CPU while on \u{2014} meant for checking smoothness, not everyday use.",
                    p.show_fps,
                    cx.listener(|_, _: &ClickEvent, _, cx| {
                        let mut np = prefs();
                        np.show_fps = !np.show_fps;
                        apply_prefs(np, cx);
                        cx.notify();
                    }),
                )
                .into_any_element()],
            ),
            div()
                .pt_1()
                .child(reset_button(
                    "reset-general",
                    "Reset General to Default",
                    cx.listener(|_, _: &ClickEvent, _, cx| {
                        apply_prefs(Prefs::default(), cx);
                        cx.notify();
                    }),
                ))
                .into_any_element(),
        ]
    }

    /// Current local build; release installation is managed separately.
    fn render_version_row(&self) -> AnyElement {
        div().flex().flex_col().gap_1()
            .child(format!("Schnauz {}", env!("CARGO_PKG_VERSION")))
            .child(div().text_size(px(UI_SMALL_FONT_SIZE))
                .text_color(rgb(theme().text_muted))
                .child("Personal build · updates are installed from this project's builds."))
            .into_any_element()
    }

    fn render_keybinds(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let t = theme();
        let km = keymap();
        let mut rows: Vec<AnyElement> = Vec::new();
        for action in KeyAction::ALL.iter().copied() {
            let recording = self.recording == Some(action);
            let binding = km.get(action);
            // The binding chip: shows the keystroke, "Unbound", or a recording hint.
            let (chip_text, chip_dim) = if recording {
                ("Press keys… (⌫ clear, esc cancel)".to_string(), false)
            } else {
                match binding {
                    Some(b) => (pretty_keystroke(b), false),
                    None => ("Unbound".to_string(), true),
                }
            };
            rows.push(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .py_1()
                    .child(div().text_color(rgb(t.text)).child(action.label()))
                    .child(
                        div()
                            .id(("kb", action.id()))
                            .flex_none()
                            .min_w(px(150.0))
                            .px_2()
                            .py(px(2.0))
                            .rounded_md()
                            .cursor_pointer()
                            .text_size(px(UI_SMALL_FONT_SIZE))
                            .bg(rgb(t.surface))
                            .border_1()
                            .border_color(if recording {
                                rgb(t.accent)
                            } else {
                                rgb(t.border)
                            })
                            .text_color(if chip_dim {
                                rgb(t.text_dim)
                            } else {
                                rgb(t.text)
                            })
                            .hover(|s| s.border_color(rgb(t.accent)))
                            .child(chip_text)
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                this.recording_quick = None;
                                this.keybind_error = None;
                                this.recording = Some(action);
                                window.focus(&this.focus);
                                cx.notify();
                            })),
                    )
                    .into_any_element(),
            );
        }
        let mut quick_rows = Vec::new();
        for (index, (id, name)) in self.quick_actions.iter().enumerate() {
            let recording = self.recording_quick.as_ref() == Some(id);
            let text = if recording { "Press keys… (⌫ clear, esc cancel)".into() }
                else { km.quick.get(id).map(|key| pretty_keystroke(key)).unwrap_or_else(|| "Unbound".into()) };
            let id = id.clone();
            quick_rows.push(div().flex().items_center().justify_between().gap_4().py_1()
                .child(div().flex_1().min_w_0().truncate().child(name.clone()))
                .child(div().id(("quick-kb", index)).flex_none().min_w(px(150.)).px_2().py(px(2.))
                    .rounded_md().cursor_pointer().text_size(px(UI_SMALL_FONT_SIZE))
                    .bg(rgb(t.surface)).border_1().border_color(rgb(if recording { t.accent } else { t.border }))
                    .child(text).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.recording = None; this.recording_quick = Some(id.clone()); this.keybind_error = None;
                        window.focus(&this.focus); cx.notify();
                    }))).into_any_element());
        }
        if let Some(error) = &self.keybind_error {
            quick_rows.insert(0, div().text_color(rgb(0xc23a35)).child(error.clone()).into_any_element());
        }
        vec![
            settings_header(
                "Keybinds",
                "Click a shortcut, then press the keys. \u{232b} clears it; Esc cancels.",
            )
            .into_any_element(),
            settings_section("Shortcuts", None, rows),
            settings_section("Quick Actions", Some("⌘-right-click a file or folder to open its actions directly. Shortcuts act on one selected local item; include Command, Control or Option."), quick_rows),
            div()
                .pt_1()
                .child(reset_button(
                    "reset-keybinds",
                    "Reset Shortcuts to Default",
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.recording = None; this.recording_quick = None; this.keybind_error = None;
                        apply_keymap(Keymap::defaults(), cx);
                        cx.notify();
                    }),
                ))
                .into_any_element(),
        ]
    }

    fn render_customization(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let t = theme();

        // Preset palette cards.
        let mut presets: Vec<AnyElement> = Vec::new();
        for (i, (name, preset)) in PRESETS.iter().enumerate() {
            let preset = *preset;
            let selected = preset == t;
            presets.push(
                div()
                    .id(("preset", i))
                    .w(px(150.0))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .rounded_lg()
                    .cursor_pointer()
                    .border_1()
                    .border_color(if selected {
                        rgb(t.accent)
                    } else {
                        rgb(t.border)
                    })
                    .bg(rgb(preset.bg))
                    .hover(|s| s.border_color(rgb(t.accent)))
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .child(swatch_dot(preset.sidebar))
                            .child(swatch_dot(preset.surface))
                            .child(swatch_dot(preset.accent))
                            .child(swatch_dot(preset.text)),
                    )
                    .child(div().text_color(rgb(preset.text)).child(name.to_string()))
                    .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                        apply_theme(preset, cx);
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }
        let presets_grid = div()
            .flex()
            .flex_wrap()
            .gap_3()
            .children(presets)
            .into_any_element();

        // A color control: name + description on the left, hex field on the
        // right, and the swatch spectrum below.
        let color_block = |title: &str,
                           desc: &str,
                           target: ColorTarget,
                           current: u32,
                           spectrum: AnyElement,
                           hex: AnyElement| {
            let t = theme();
            let _ = (target, current);
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_4()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(div().text_color(rgb(t.text)).child(title.to_string()))
                                .child(
                                    div()
                                        .text_size(px(UI_SMALL_FONT_SIZE))
                                        .text_color(rgb(t.text_muted))
                                        .child(desc.to_string()),
                                ),
                        )
                        .child(hex),
                )
                .child(spectrum)
                .into_any_element()
        };

        vec![
            settings_header(
                "Customization",
                "Choose a theme and fine-tune the interface colors.",
            )
            .into_any_element(),
            settings_section(
                "Theme",
                Some("Pick a starting palette, then fine-tune the colors below."),
                vec![presets_grid],
            ),
            settings_section(
                "Colors",
                Some("Core interface colors. Type a hex value or pick from the swatches."),
                vec![
                    color_block(
                        "Background",
                        "The main window and file list.",
                        ColorTarget::Bg,
                        t.bg,
                        self.color_row("bg", t.bg, |t, c| t.bg = c, cx)
                            .into_any_element(),
                        self.hex_field(ColorTarget::Bg, t.bg, cx).into_any_element(),
                    ),
                    color_block(
                        "Text",
                        "Primary text throughout the app.",
                        ColorTarget::Text,
                        t.text,
                        self.color_row("text", t.text, |t, c| t.text = c, cx)
                            .into_any_element(),
                        self.hex_field(ColorTarget::Text, t.text, cx)
                            .into_any_element(),
                    ),
                    color_block(
                        "Mouseover",
                        "The highlight behind hovered rows and buttons.",
                        ColorTarget::Hover,
                        t.hover,
                        self.color_row("hover", t.hover, |t, c| t.hover = c, cx)
                            .into_any_element(),
                        self.hex_field(ColorTarget::Hover, t.hover, cx)
                            .into_any_element(),
                    ),
                ],
            ),
            div()
                .pt_1()
                .child(reset_button(
                    "reset-customization",
                    "Reset Customization to Default",
                    cx.listener(|_, _: &ClickEvent, _, cx| {
                        apply_theme(Theme::default(), cx);
                        apply_menu_style(MenuStyle::default(), cx);
                        cx.notify();
                    }),
                ))
                .into_any_element(),
        ]
    }

    /// A grid of color swatches; clicking one sets a single theme field via
    /// `set`. `tag` keeps element ids unique across rows that share colors.
    fn color_row(
        &self,
        tag: &'static str,
        current: u32,
        set: fn(&mut Theme, u32),
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = theme();
        let mut swatches: Vec<AnyElement> = Vec::new();
        for c in palette_colors() {
            let selected = c == current;
            swatches.push(
                div()
                    .id((tag, c as usize))
                    .w(px(22.0))
                    .h(px(22.0))
                    .rounded_md()
                    .cursor_pointer()
                    .bg(rgb(c))
                    .border_2()
                    .border_color(if selected {
                        rgb(t.accent)
                    } else {
                        rgb(t.border)
                    })
                    .hover(|s| s.border_color(rgb(t.accent)))
                    .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                        let mut nt = theme();
                        set(&mut nt, c);
                        apply_theme(nt, cx);
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }
        div().flex().flex_wrap().gap_1().children(swatches)
    }


    /// A small "current value + editable hex" control for a theme color.
    fn hex_field(
        &self,
        target: ColorTarget,
        current: u32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = theme();
        let editing = self.color_edit.as_ref().filter(|(tt, _)| *tt == target);
        let text = match editing {
            Some((_, s)) => format!("#{s}\u{2502}"),
            None => format!("#{current:06x}"),
        };
        div()
            .id(("hex", target as usize))
            .flex_none()
            .px_2()
            .py(px(2.0))
            .rounded_md()
            .cursor_text()
            .bg(rgb(t.surface))
            .border_1()
            .border_color(if editing.is_some() {
                rgb(t.accent)
            } else {
                rgb(t.border)
            })
            .text_size(px(UI_SMALL_FONT_SIZE))
            .text_color(rgb(t.text))
            .child(text)
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.color_edit = Some((target, String::new()));
                window.focus(&this.focus);
                cx.notify();
            }))
    }
}

/// A small color dot used in preset previews.
pub(super) fn swatch_dot(color: u32) -> impl IntoElement {
    div().w(px(14.0)).h(px(14.0)).rounded_full().bg(rgb(color))
}



/// Pretty-print a stored keystroke string ("cmd-shift-p") with Mac symbols.
pub(super) fn pretty_keystroke(s: &str) -> String {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() == 1 {
        return key_symbol(parts[0]);
    }
    let mut out = String::new();
    for m in &parts[..parts.len() - 1] {
        out.push_str(match *m {
            "cmd" => "⌘",
            "ctrl" => "⌃",
            "alt" => "⌥",
            "shift" => "⇧",
            o => o,
        });
    }
    out.push_str(&key_symbol(parts[parts.len() - 1]));
    out
}

pub(super) fn key_symbol(k: &str) -> String {
    match k {
        "enter" => "↩".into(),
        "escape" => "⎋".into(),
        "backspace" => "⌫".into(),
        "delete" => "⌦".into(),
        "up" => "↑".into(),
        "down" => "↓".into(),
        "left" => "←".into(),
        "right" => "→".into(),
        "space" => "␣".into(),
        "tab" => "⇥".into(),
        o => o.to_uppercase(),
    }
}

/// A section heading inside Settings.
pub(super) fn settings_title(text: &str, width: f32) -> impl IntoElement {
    div()
        .relative()
        .w(px(width))
        .flex()
        .min_w_0()
        .flex_none()
        .whitespace_nowrap()
        .overflow_hidden()
        .when(text == "Information", |d| d.children(ui_checks::probe("preview-info-heading")))
        .text_color(rgb(theme().text_muted))
        .text_size(px(UI_SMALL_FONT_SIZE))
        .child(ui_checks::text("section-heading", text.to_uppercase()))
}

/// The large title + one-line description shown at the top of a settings tab.
pub(super) fn settings_header(title: &str, subtitle: &str) -> impl IntoElement {
    let t = theme();
    div()
        .flex()
        .flex_col()
        .gap_1()
        .pb_1()
        .child(
            div()
                .text_base()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(t.text))
                .child(title.to_string()),
        )
        .child(
            div()
                .text_size(px(UI_SMALL_FONT_SIZE))
                .text_color(rgb(t.text_muted))
                .child(subtitle.to_string()),
        )
}

/// A hairline divider used inside settings cards.
pub(super) fn settings_divider() -> impl IntoElement {
    div().h(px(1.0)).bg(rgb(theme().border))
}

/// A settings "card": a titled, bordered container grouping related rows, each
/// row padded and separated by a hairline. This is the core of the settings
/// look — related controls read as one unit instead of a flat list.
pub(super) fn settings_section(
    title: &str,
    subtitle: Option<&str>,
    rows: Vec<AnyElement>,
) -> AnyElement {
    let t = theme();
    let mut header = div().flex().flex_col().gap_1().px_4().pt_3().pb_3().child(
        div()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(t.text))
            .child(title.to_string()),
    );
    if let Some(s) = subtitle {
        header = header.child(
            div()
                .text_size(px(UI_SMALL_FONT_SIZE))
                .text_color(rgb(t.text_dim))
                .child(s.to_string()),
        );
    }

    let mut card = div()
        .w_full()
        .flex()
        .flex_col()
        .rounded_lg()
        .border_1()
        .border_color(rgb(t.border))
        .bg(Theme::alpha(t.surface, 0x33))
        .child(header)
        .child(settings_divider());

    for (i, row) in rows.into_iter().enumerate() {
        if i > 0 {
            card = card.child(settings_divider());
        }
        card = card.child(div().px_4().py_3().child(row));
    }
    card.into_any_element()
}

/// A "Reset to Default" button (used once per settings tab).
pub(super) fn reset_button(
    id: &'static str,
    label: &str,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let t = theme();
    div().flex().pt_2().child(
        div()
            .id(id)
            .px_3()
            .py_1()
            .rounded_md()
            .cursor_pointer()
            .bg(rgb(t.hover))
            .text_color(rgb(t.text))
            .border_1()
            .border_color(rgb(t.border))
            .hover(|s| s.border_color(rgb(0xd9544f)).text_color(rgb(0xd9544f)))
            .child(label.to_string())
            .on_click(on_click),
    )
}

/// A labelled on/off toggle row used in the General settings tab. `id` must be
/// unique per row, or only the first switch receives clicks.
pub(super) fn toggle_row(
    id: &'static str,
    title: &str,
    desc: &str,
    on: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let t = theme();
    // The pill switch.
    let knob = div()
        .absolute()
        .top(px(2.0))
        .left(if on { px(20.0) } else { px(2.0) })
        .w(px(16.0))
        .h(px(16.0))
        .rounded_full()
        .bg(rgb(0xffffff));
    let switch = div()
        .id(id)
        .flex_none()
        .relative()
        .w(px(38.0))
        .h(px(20.0))
        .rounded_full()
        .cursor_pointer()
        .bg(if on { rgb(t.accent) } else { rgb(t.surface) })
        .child(knob)
        .on_click(on_click);

    div()
        .w_full()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_1()
                .child(div().text_color(rgb(t.text)).child(title.to_string()))
                .child(
                    div()
                        .text_size(px(UI_SMALL_FONT_SIZE))
                        .text_color(rgb(t.text_muted))
                        .child(desc.to_string()),
                ),
        )
        .child(switch)
}



/// Open the Settings window (shared by the menu action and the palette).
pub(super) fn open_settings_window(cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(760.0), px(560.0)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(760.), px(500.))),
            titlebar: Some(TitlebarOptions {
                title: None,
                appears_transparent: true,
                ..Default::default()
            }),
            ..Default::default()
        },
        |window, cx| {
            let view = cx.new(Settings::new);
            window.focus(&view.read(cx).focus);
            view
        },
    )
    .ok();
    cx.activate(true);
}

#[cfg(feature = "ui-checks")]
pub(super) fn check_quick_keybinds(cx: &mut App) {
    let original = keymap();
    let settings = cx.new(Settings::new);
    let event = |key: &str| KeyDownEvent { keystroke: gpui::Keystroke::parse(key).unwrap(), is_held: false };
    let id = "workflow:/tmp/disposable/Measure=Render.workflow".to_string();
    settings.update(cx, |settings, cx| {
        settings.recording_quick = Some(id.clone());
        settings.handle_keybind_key(&event("cmd-c"), cx);
        assert!(settings.keybind_error.is_some() && settings.recording_quick.is_some());
        settings.handle_keybind_key(&event("cmd-alt-m"), cx);
        assert!(settings.keybind_error.is_none() && settings.recording_quick.is_none());
        assert_eq!(keymap().quick.get(&id).map(String::as_str), Some("cmd-alt-m"));
        assert_eq!(load_keymap().quick.get(&id).map(String::as_str), Some("cmd-alt-m"));
        settings.recording = Some(KeyAction::Duplicate);
        settings.handle_keybind_key(&event("cmd-alt-m"), cx);
        assert!(settings.keybind_error.is_some() && settings.recording.is_some());
        settings.recording = None; settings.recording_quick = Some(id.clone());
        settings.handle_keybind_key(&event("escape"), cx);
        assert_eq!(keymap().quick.get(&id).map(String::as_str), Some("cmd-alt-m"));
        settings.recording_quick = Some(id.clone());
        settings.handle_keybind_key(&event("backspace"), cx);
        assert!(!load_keymap().quick.contains_key(&id));
    });
    apply_keymap(original, cx);
    println!("PASS Quick Action shortcut recording, conflicts in both directions, escaped persistence, cancel and clear; no workflows run");
}
