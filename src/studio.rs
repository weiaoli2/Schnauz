//! Studio's workspace controls and application chrome.
use super::*;
use crate::workspaces::{Layout, Library, SavedPane, SavedTab};

impl Shuffle {
    pub(super) fn capture_layout(&self) -> Layout {
        let mut panes = Vec::new();
        let mut active_pane = 0;
        for (index, pane) in self.panes.iter().enumerate() {
            let mut tabs = Vec::new();
            let mut active = 0;
            for (tab_index, tab) in pane.tabs.iter().enumerate() {
                if tab.remote.is_none() {
                    if tab_index <= pane.active {
                        active = tabs.len();
                    }
                    tabs.push(SavedTab {
                        path: tab.current_dir.clone(),
                        view: view_mode_key(tab.view).into(),
                        sort: sort_key_name(tab.sort_key).into(),
                        ascending: tab.sort_asc,
                    });
                }
            }
            if !tabs.is_empty() {
                if index <= self.active_pane {
                    active_pane = panes.len();
                }
                panes.push(SavedPane { tabs, active });
            }
        }
        Layout {
            panes,
            active_pane,
            split_ratio: self.split_ratio,
        }
    }

    pub(super) fn apply_layout(&mut self, layout: &Layout) -> bool {
        let Some(layout) = layout.available() else {
            return false;
        };
        self.panes = layout
            .panes
            .iter()
            .map(|pane| Pane {
                active: pane.active,
                tabs: pane
                    .tabs
                    .iter()
                    .map(|saved| {
                        let mut tab = Tab::new(saved.path.clone());
                        tab.view = view_mode_from_key(&saved.view).unwrap_or(ViewMode::List);
                        tab.sort_key = sort_key_from_name(&saved.sort).unwrap_or(SortKey::None);
                        tab.sort_asc = saved.ascending;
                        tab
                    })
                    .collect(),
            })
            .collect();
        self.active_pane = layout.active_pane;
        self.split_ratio = layout.split_ratio;
        self.split_epoch += 1;
        self.collapse_anim = None;
        self.divider_drag = None;
        self.context_menu = None;
        self.sidebar_menu = None;
        self.sort_menu = None;
        self.rename = None;
        self.marquee = None;
        self.drag_candidate = None;
        self.hovered = None;
        self.drop_hover = None;
        self.scroll_drag = None;
        self.resize = None;
        self.hbar_drag = None;
        self.preview_page = 0;
        self.cloud_sync_gen += 1;
        true
    }

    pub(super) fn open_workspace_dialog(&mut self, cx: &mut Context<Self>) {
        self.workspace_dialog = Some(String::new());
        self.workspace_dialog_error = self.workspace_load_error.clone();
        self.context_menu = None;
        self.sidebar_menu = None;
        self.palette_open = false;
        cx.notify();
    }

    fn persist_library(&mut self, candidate: Library) -> Result<(), String> {
        if let Some(error) = &self.workspace_load_error {
            return Err(error.clone());
        }
        let path = config_file("workspaces.json").ok_or("Cannot find the settings folder.")?;
        candidate
            .save(&path)
            .map_err(|error| format!("Couldn't save the workspace: {error}"))?;
        self.workspaces = candidate;
        Ok(())
    }

    fn save_workspace(&mut self, cx: &mut Context<Self>) {
        let Some(name) = self.workspace_dialog.clone() else {
            return;
        };
        let mut library = self.workspaces.clone();
        let result = library
            .create(&name, self.capture_layout())
            .and_then(|id| self.persist_library(library).map(|_| id));
        match result {
            Ok(id) => {
                self.active_workspace = Some(id);
                self.workspace_dialog = None;
                self.workspace_dialog_error = None;
                self.collapsed_sections.remove("WORKSPACES");
                self.set_notice(format!("Saved workspace ‘{}’", name.trim()), cx);
            }
            Err(error) => {
                self.workspace_dialog_error = Some(error);
                cx.notify();
            }
        }
    }

    pub(super) fn restore_workspace(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(saved) = self
            .workspaces
            .items
            .iter()
            .find(|workspace| workspace.id == id)
            .cloned()
        else {
            return;
        };
        let old_count = saved.layout.tab_count();
        if !self.apply_layout(&saved.layout) {
            self.sidebar_menu = None;
            self.set_notice("The folders in this workspace are unavailable. Reconnect the drive or update the workspace.".into(), cx);
            return;
        }
        self.active_workspace = Some(id);
        for pane in 0..self.panes.len() {
            self.reload_pane(pane, cx);
        }
        self.save_session();
        let skipped = old_count.saturating_sub(self.capture_layout().tab_count());
        let message = if skipped > 0 {
            format!(
                "Opened ‘{}’ · {skipped} unavailable folder(s) skipped",
                saved.name
            )
        } else {
            format!("Opened workspace ‘{}’", saved.name)
        };
        self.set_notice(message, cx);
    }

    pub(super) fn update_workspace(&mut self, id: u64, cx: &mut Context<Self>) {
        let layout = self.capture_layout();
        self.sidebar_menu = None;
        if layout.tab_count() == 0 {
            self.set_notice(
                "Open a local folder before updating a workspace.".into(),
                cx,
            );
            return;
        }
        let mut library = self.workspaces.clone();
        let Some(workspace) = library
            .items
            .iter_mut()
            .find(|workspace| workspace.id == id)
        else {
            return;
        };
        workspace.layout = layout;
        match self.persist_library(library) {
            Ok(()) => {
                self.active_workspace = Some(id);
                self.set_notice("Workspace updated with the current layout.".into(), cx);
            }
            Err(error) => self.set_notice(error, cx),
        }
    }

    pub(super) fn remove_workspace(&mut self, id: u64, cx: &mut Context<Self>) {
        self.sidebar_menu = None;
        let mut library = self.workspaces.clone();
        library.items.retain(|workspace| workspace.id != id);
        match self.persist_library(library) {
            Ok(()) => {
                if self.active_workspace == Some(id) {
                    self.active_workspace = None;
                }
                self.set_notice(
                    "Workspace removed. Your files and open tabs are unchanged.".into(),
                    cx,
                );
            }
            Err(error) => self.set_notice(error, cx),
        }
    }

    pub(super) fn handle_workspace_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = &ev.keystroke;
        match key.key.as_str() {
            "escape" => {
                self.workspace_dialog = None;
                cx.notify();
            }
            "enter" => self.save_workspace(cx),
            "backspace" => {
                if let Some(name) = self.workspace_dialog.as_mut() {
                    name.pop();
                }
                cx.notify();
            }
            "v" if key.modifiers.platform => {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    if let Some(name) = self.workspace_dialog.as_mut() {
                        name.push_str(text.trim());
                    }
                    cx.notify();
                }
            }
            _ if !key.modifiers.platform && !key.modifiers.control => {
                if let Some(text) = &key.key_char {
                    if !text.chars().any(char::is_control) {
                        if let Some(name) = self.workspace_dialog.as_mut() {
                            name.push_str(text);
                        }
                        cx.notify();
                    }
                }
            }
            _ => {}
        }
    }

    pub(super) fn toggle_studio_split(&mut self, cx: &mut Context<Self>) {
        self.context_menu = None;
        self.sidebar_menu = None;
        self.sort_menu = None;
        self.rename = None;
        self.resize = None;
        self.scroll_drag = None;
        self.hbar_drag = None;
        self.preview_page = 0;
        if self.panes.len() == 1 {
            let mut pane = Pane::new(self.active_tab().current_dir.clone());
            pane.tabs[0].remote = self.active_tab().remote.clone();
            pane.tabs[0].view = self.active_tab().view;
            self.panes.push(pane);
            self.active_pane = 1;
            self.split_ratio = 0.5;
            self.reload_pane(1, cx);
        } else {
            let right = self.panes.remove(1);
            let offset = self.panes[0].tabs.len();
            if self.active_pane == 1 {
                self.panes[0].active = offset + right.active;
            }
            self.panes[0].tabs.extend(right.tabs);
            self.active_pane = 0;
        }
        self.split_epoch += 1;
        self.collapse_anim = None;
        self.divider_drag = None;
        self.hovered = None;
        self.drop_hover = None;
        self.marquee = None;
        self.save_session();
        cx.notify();
    }

    pub(super) fn render_studio_toolbar(&self, cx: &Context<Self>) -> AnyElement {
        let t = theme();
        let workspace = self.active_workspace.and_then(|id| {
            self.workspaces
                .items
                .iter()
                .find(|workspace| workspace.id == id)
        });
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap_3()
            .w_full()
            .h(px(TITLEBAR_H))
            .pl(px(88.0))
            .pr_4()
            .bg(rgb(t.sidebar))
            .border_b_1()
            .border_color(rgb(t.border))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w(px(170.0))
                    .flex_none()
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Shuffle Studio"),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(rgb(t.text_muted))
                            .child(
                                workspace
                                    .map(|w| w.name.clone())
                                    .unwrap_or_else(|| "Your files. Your flow.".into()),
                            ),
                    ),
            )
            .child(div().flex_1())
            .child(
                div()
                    .id("studio-search")
                    .w(px(240.0))
                    .h(px(30.0))
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(t.border))
                    .bg(rgb(t.bg))
                    .text_color(rgb(t.text_muted))
                    .cursor_pointer()
                    .hover(|s| s.border_color(rgb(t.accent)))
                    .child("⌕")
                    .child(div().flex_1().text_xs().child("Search files and commands"))
                    .child(div().text_size(px(10.0)).child("⌘P"))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.toggle_palette(window, cx)
                    })),
            )
            .child(div().flex_1())
            .child(
                studio_button(
                    "studio-split",
                    if self.panes.len() > 1 {
                        "Join panes"
                    } else {
                        "Split view"
                    },
                    false,
                )
                .tooltip(tip("Split or join panes · ⌘⇧D"))
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_studio_split(cx))),
            )
            .child(
                studio_button("studio-preview", "Preview", prefs().preview)
                    .tooltip(tip("Show or hide the preview panel · ⌘⇧I"))
                    .on_click(cx.listener(|_, _: &ClickEvent, _, cx| toggle_inspector(cx))),
            )
            .child(
                studio_button("studio-save", "Save workspace", true)
                    .tooltip(tip("Save these folders and tabs · ⌘⇧S"))
                    .on_click(
                        cx.listener(|this, _: &ClickEvent, _, cx| this.open_workspace_dialog(cx)),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn push_workspace_sidebar(
        &self,
        items: &mut Vec<AnyElement>,
        collapsed: bool,
        cx: &Context<Self>,
    ) {
        let t = theme();
        if collapsed {
            return;
        }
        let add = div()
            .id("workspace-add")
            .px_1()
            .cursor_pointer()
            .text_color(rgb(t.text_muted))
            .hover(|s| s.text_color(rgb(t.accent)))
            .child("+")
            .tooltip(tip("Save workspace"))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.open_workspace_dialog(cx)))
            .into_any_element();
        items.push(self.section_header_el("WORKSPACES", Some(add), cx));
        if self.collapsed_sections.contains("WORKSPACES") {
            return;
        }
        if self.workspaces.items.is_empty() {
            items.push(
                div()
                    .mx_3()
                    .mt_2()
                    .mb_1()
                    .p_3()
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(t.border))
                    .text_xs()
                    .text_color(rgb(t.text_muted))
                    .child("Save a set of folders and tabs. Return to your project in one click.")
                    .into_any_element(),
            );
        }
        for workspace in &self.workspaces.items {
            let id = workspace.id;
            let active = self.active_workspace == Some(id);
            items.push(
                div()
                    .id(SharedString::from(format!("workspace-{id}")))
                    .mx_2()
                    .mt_1()
                    .px_3()
                    .py_2()
                    .rounded_lg()
                    .flex()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .when(active, |s| s.bg(rgb(t.selected)))
                    .hover(|s| s.bg(rgb(t.hover)))
                    .text_color(rgb(if active { t.accent } else { t.text_muted }))
                    .child(div().text_size(px(17.0)).child("▦"))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(workspace.name.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .child(workspace.layout.tab_count().to_string()),
                    )
                    .tooltip(tip(format!(
                        "{} · {} local folder(s)\nRight-click for workspace actions",
                        workspace.name,
                        workspace.layout.tab_count()
                    )))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.restore_workspace(id, cx)
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            this.open_sidebar_menu(
                                f32::from(event.position.x),
                                f32::from(event.position.y),
                                SidebarTarget::Workspace(id),
                                cx,
                            );
                            cx.stop_propagation();
                        }),
                    )
                    .into_any_element(),
            );
        }
    }

    pub(super) fn render_workspace_dialog(&self, cx: &Context<Self>) -> AnyElement {
        let t = theme();
        let name = self.workspace_dialog.clone().unwrap_or_default();
        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x11182766))
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.workspace_dialog = None;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .w(px(430.0))
                    .p_6()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .rounded_xl()
                    .bg(rgb(t.surface))
                    .border_1()
                    .border_color(rgb(t.border))
                    .shadow_lg()
                    .on_mouse_down(MouseButton::Left, |_, _, cx: &mut App| {
                        cx.stop_propagation()
                    })
                    .child(
                        div()
                            .text_size(px(19.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Save your workspace"),
                    )
                    .child(div().text_sm().text_color(rgb(t.text_muted)).child(
                        "Keep local folders, tabs, view modes and your split layout together.",
                    ))
                    .child(
                        div()
                            .h(px(42.0))
                            .px_3()
                            .flex()
                            .items_center()
                            .rounded_lg()
                            .bg(rgb(t.bg))
                            .border_1()
                            .border_color(rgb(t.accent))
                            .child(
                                div()
                                    .text_color(rgb(if name.is_empty() {
                                        t.text_dim
                                    } else {
                                        t.text
                                    }))
                                    .child(if name.is_empty() {
                                        "Workspace name".into()
                                    } else {
                                        name
                                    }),
                            )
                            .child(div().w(px(1.0)).h(px(17.0)).bg(rgb(t.accent))),
                    )
                    .children(self.workspace_dialog_error.as_ref().map(|error| {
                        div()
                            .text_xs()
                            .text_color(rgb(0xd14b57))
                            .child(error.clone())
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(t.text_muted))
                                    .child("↵ Save    Esc Cancel"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .child(
                                        studio_button("workspace-cancel", "Cancel", false)
                                            .on_click(cx.listener(
                                                |this, _: &ClickEvent, _, cx| {
                                                    this.workspace_dialog = None;
                                                    cx.notify();
                                                },
                                            )),
                                    )
                                    .child(
                                        studio_button("workspace-confirm", "Save workspace", true)
                                            .on_click(cx.listener(
                                                |this, _: &ClickEvent, _, cx| {
                                                    this.save_workspace(cx)
                                                },
                                            )),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn render_studio_status(&self, cx: &Context<Self>) -> AnyElement {
        let t = theme();
        let tab = self.active_tab();
        let count = if tab.find_query.is_some() {
            tab.find_results.len()
        } else {
            tab.entries.len()
        };
        let label = if tab.selection.is_empty() {
            format!("{count} items")
        } else {
            format!("{} selected · {count} items", tab.selection.len())
        };
        div()
            .flex_none()
            .h(px(30.0))
            .flex()
            .items_center()
            .gap_3()
            .px_4()
            .bg(rgb(t.sidebar))
            .border_t_1()
            .border_color(rgb(t.border))
            .text_size(px(11.0))
            .text_color(rgb(t.text_muted))
            .child(div().size(px(6.0)).rounded_full().bg(rgb(t.accent)))
            .child(label)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(display_path(&tab.current_dir)),
            )
            .child(
                div()
                    .id("studio-theme")
                    .cursor_pointer()
                    .hover(|s| s.text_color(rgb(t.accent)))
                    .child(if t.bg == PRESETS[0].1.bg {
                        "Dark appearance"
                    } else {
                        "Light appearance"
                    })
                    .on_click(cx.listener(|_, _: &ClickEvent, _, cx| {
                        let next = if theme().bg == PRESETS[0].1.bg {
                            PRESETS[1].1
                        } else {
                            PRESETS[0].1
                        };
                        apply_theme(next, cx);
                    })),
            )
            .child(
                div()
                    .id("studio-settings")
                    .cursor_pointer()
                    .hover(|s| s.text_color(rgb(t.accent)))
                    .child("Settings  ⌘,")
                    .on_click(|_: &ClickEvent, _, cx: &mut App| open_settings_window(cx)),
            )
            .into_any_element()
    }
}

fn studio_button(
    id: &'static str,
    label: &'static str,
    accented: bool,
) -> gpui::Stateful<gpui::Div> {
    let t = theme();
    div()
        .id(id)
        .h(px(30.0))
        .px_3()
        .rounded_lg()
        .flex()
        .items_center()
        .justify_center()
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .bg(rgb(if accented { t.accent } else { t.surface }))
        .text_color(rgb(if accented { 0xffffff } else { t.text_muted }))
        .hover(move |s| {
            s.bg(if accented {
                Theme::alpha(t.accent, 0xdd)
            } else {
                rgb(t.hover)
            })
        })
        .child(label)
}

pub(super) fn toggle_inspector(cx: &mut App) {
    let mut next = prefs();
    next.preview = !next.preview;
    next.info = next.preview;
    apply_prefs(next, cx);
}
