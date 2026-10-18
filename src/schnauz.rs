//! Schnauz's project controls and application chrome.
use super::*;
use crate::projects::{Layout, Library, SavedPane, SavedTab};

pub(super) struct ProjectSession {
    pub panes: Vec<Pane>,
    pub active_pane: usize,
    pub split_ratio: f32,
}

impl Schnauz {
    pub(super) fn render_hidden_button(&self, pane: usize, cx: &Context<Self>) -> AnyElement {
        let t = theme();
        let shown = self.tab(pane).show_hidden;
        div()
            .id(("hidden-toggle", pane))
            .flex_none()
            .w(px(24.0))
            .h(px(22.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_md()
            .cursor_pointer()
            .when(shown, |d| d.bg(rgb(t.surface)))
            .hover(|d| d.bg(rgb(t.hover)))
            .tooltip(tip(if shown {
                "Hide hidden files and folders"
            } else {
                "Show hidden files and folders"
            }))
            .child(ui_icon(if shown { Icon::Eye } else { Icon::EyeOff }, 17., rgb(if shown { t.accent } else { t.text_muted })))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_hidden_preserving_selection(pane, cx)))
            .into_any_element()
    }

    pub(super) fn copy_with_feedback(
        &mut self,
        pane: usize,
        paths: &[PathBuf],
        cx: &mut Context<Self>,
    ) {
        if paths.is_empty() {
            return;
        }
        if !copy_files_to_pasteboard(paths) {
            self.set_notice("Couldn't copy these items to the clipboard.".into(), cx);
            return;
        }
        let message = if paths.len() == 1 {
            let kind = if is_browsable_folder(&paths[0], paths[0].is_dir()) {
                "folder"
            } else {
                "file"
            };
            format!("Copied {kind} ‘{}’", path_label(&paths[0]))
        } else {
            format!(
                "Copied {} items · {}",
                paths.len(),
                paths
                    .iter()
                    .take(3)
                    .map(|p| path_label(p))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        self.tab_mut(pane).copy_feedback = Some((message, Instant::now()));
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(6)).await;
            this.update(cx, |_, cx| cx.notify()).ok();
        })
        .detach();
    }

    pub(super) fn render_copy_feedback(&self, pane: usize) -> Option<AnyElement> {
        let (message, start) = self.tab(pane).copy_feedback.as_ref()?;
        if start.elapsed() >= Duration::from_secs(6) {
            return None;
        }
        let t = theme();
        Some(
            div()
                .absolute()
                .left_3()
                .bottom_3()
                .max_w(relative(0.75))
                .px_3()
                .py_2()
                .rounded_lg()
                .border_1()
                .border_color(rgb(t.border))
                .bg(rgb(t.surface))
                .shadow_md()
                .text_size(px(UI_SMALL_FONT_SIZE))
                .text_color(rgb(t.text))
                .child(div().truncate().child(format!("✓ {message}")))
                .into_any_element(),
        )
    }

    pub(super) fn paste_from_clipboard(&mut self, cx: &mut Context<Self>) {
        let pane = self.active_pane;
        // Finder's copied file objects retain ordinary copy/paste behavior.
        let files = pasteboard_file_paths();
        if !files.is_empty() {
            let dir = self.tab(pane).current_dir.clone();
            if self.tab(pane).remote.is_some() {
                self.upload_to_remote(pane, dir, files, cx);
            } else {
                self.copy_items(dir, files, cx);
            }
            return;
        }
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        let destination = match crate::clipboard_path::resolve(&text, &home_dir()) {
            Ok(Some(destination)) => destination,
            Ok(None) => return,
            Err(error) => {
                self.set_notice(format!("Couldn't open the pasted path: {error}"), cx);
                return;
            }
        };
        let crate::clipboard_path::Destination {
            folder,
            selected_file,
        } = destination;
        // A local path pasted while browsing SFTP opens a local tab.
        if self.tab(pane).remote.is_some() {
            let view = self.tab(pane).view;
            let mut tab = Tab::new(folder.clone());
            tab.view = view;
            self.panes[pane].tabs.push(tab);
            self.panes[pane].active = self.panes[pane].tabs.len() - 1;
        }
        if selected_file.as_ref().is_some_and(|path| {
            use std::os::macos::fs::MetadataExt;
            path.file_name()
                .is_some_and(|s| s.as_encoded_bytes().starts_with(b"."))
                || fs::metadata(path).is_ok_and(|m| m.st_flags() & libc::UF_HIDDEN != 0)
        }) {
            self.tab_mut(pane).show_hidden = true;
        }
        if selected_file.is_some() && self.tab(pane).entry_filter == EntryFilter::Folders {
            self.tab_mut(pane).set_entry_filter(EntryFilter::All);
        }
        self.context_menu = None;
        self.sort_menu = None;
        self.hovered = None;
        self.drag_candidate = None;
        self.marquee = None;
        self.scroll_drag = None;
        self.hbar_drag = None;
        if self.tab(pane).current_dir == folder {
            self.load_dir_in(pane, folder, cx);
        } else {
            self.navigate_in(pane, folder, cx);
        }
        if let Some(file) = selected_file {
            self.reveal_in_pane(pane, file.clone(), cx);
        } else {
            self.tab(pane)
                .scroll_handle
                .scroll_to_item(0, ScrollStrategy::Top);
        }
        self.save_session();
        cx.notify();
    }

    fn set_entry_filter(&mut self, pane: usize, filter: EntryFilter, cx: &mut Context<Self>) {
        self.active_pane = pane;
        if self.tab(pane).entry_filter == filter {
            cx.notify();
            return;
        }
        self.tab_mut(pane).set_entry_filter(filter);
        if self.search_in_pane(pane) { self.refresh_file_search(cx); }
        self.rename = None;
        self.context_menu = None;
        self.sort_menu = None;
        self.marquee = None;
        self.drag_candidate = None;
        self.hovered = None;
        self.drop_hover = None;
        self.preview_page = 0;
        self.save_session();
        self.prewarm_icons(cx);
        cx.notify();
    }

    pub(super) fn render_entry_filter(&self, pane: usize, cx: &Context<Self>) -> AnyElement {
        let t = theme();
        let tab = self.tab(pane);
        let filtered = tab.entry_filter != EntryFilter::All;
        let mut buttons = Vec::new();
        for (index, filter) in [EntryFilter::All, EntryFilter::Folders, EntryFilter::Files]
            .into_iter()
            .enumerate()
        {
            let selected = tab.entry_filter == filter;
            buttons.push(
                div()
                    .id(("entry-filter", pane * 3 + index))
                    .h(px(24.0))
                    .px_3()
                    .rounded_md()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .font_weight(if selected {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .text_color(rgb(if selected { t.accent } else { t.text_muted }))
                    .when(selected, |s| s.bg(rgb(t.bg)).shadow_sm())
                    .hover(|s| s.bg(rgb(t.hover)))
                    .child(filter.label())
                    .tooltip(tip(format!(
                        "Show {} in this tab",
                        filter.description().to_lowercase()
                    )))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.set_entry_filter(pane, filter, cx);
                    }))
                    .into_any_element(),
            );
        }
        let count = if self.search_in_pane(pane) { self.palette_items.len() } else { tab.visible_count() };
        div()
            .flex_none()
            .h(px(36.0))
            .px_3()
            .flex()
            .items_center()
            .gap_2()
            .border_b_1()
            .border_color(rgb(if filtered { t.accent } else { t.border }))
            .bg(rgb(if filtered { filter_tint(t) } else { t.sidebar }))
            .text_size(px(UI_SMALL_FONT_SIZE))
            .text_color(rgb(t.text_muted))
            .child("Show")
            .child(
                div()
                    .flex()
                    .p(px(2.0))
                    .rounded_lg()
                    .bg(rgb(t.surface))
                    .children(buttons),
            )
            .child(div().flex_1())
            .when(tab.remote.is_none(), |d| d.child(
                div().id(("inspect-storage", pane)).h(px(26.)).px_2().flex().items_center().gap_2().rounded_md().cursor_pointer()
                    .hover(|s| s.bg(rgb(t.hover))).tooltip(tip("Disk Usage…"))
                    .child(ui_icon(Icon::Storage, 16., rgb(t.text_muted)))
                    .when(pane_width(pane) > 700., |d| d.child("Disk Usage"))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.inspect_folder(pane, this.tab(pane).current_dir.clone(), window, cx);
                    }))
            ))
            .child(div().truncate().child(if self.search_in_pane(pane) {
                format!("{count} result{}",if count == 1 {""} else {"s"})
            } else if tab.is_filtered() {
                format!("{count} of {} items", tab.entries.len())
            } else {
                format!("{count} item{}",if count == 1 {""} else {"s"})
            }))
            .into_any_element()
    }

    pub(super) fn capture_layout(&self) -> Layout {
        let mut panes = Vec::new();
        let mut active_pane = 0;
        for (index, pane) in self.panes.iter().enumerate() {
            let mut tabs = Vec::new();
            let mut active = 0;
            for (tab_index, tab) in pane.tabs.iter().enumerate() {
                if tab.remote.is_some() && tab.terminal.as_ref().and_then(|s| s.ssh.as_ref()).is_some() {
                    if tab_index <= pane.active { active = tabs.len(); }
                    tabs.push(tab.terminal.as_ref().unwrap().ssh.as_ref().unwrap().saved_local());
                } else if tab.remote.is_none() {
                    if tab_index <= pane.active {
                        active = tabs.len();
                    }
                    tabs.push(SavedTab {
                        path: tab.current_dir.clone(),
                        view: view_mode_key(tab.view).into(),
                        sort: sort_key_name(tab.sort_key).into(),
                        ascending: tab.sort_asc,
                        filter: tab.entry_filter,
                        show_hidden: tab.show_hidden,
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
                        tab.entry_filter = saved.filter;
                        tab.show_hidden = saved.show_hidden;
                        sort_entries(&mut tab.entries, tab.sort_key, tab.sort_asc);
                        tab.recompute_results();
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
        self.apply_all_folder_views();
        true
    }

    pub(super) fn open_project_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus);
        self.new_text_file = None;
        self.project_dialog = Some(String::new());
        self.project_dialog_error = self.project_load_error.clone();
        self.context_menu = None;
        self.sidebar_menu = None;
        self.close_palette(cx);
        cx.notify();
    }

    pub(super) fn persist_library(&mut self, candidate: Library) -> Result<(), String> {
        if let Some(error) = &self.project_load_error {
            return Err(error.clone());
        }
        let path = config_file("projects.json").ok_or("Cannot find the settings folder.")?;
        candidate
            .save(&path)
            .map_err(|error| format!("Couldn't save the project: {error}"))?;
        self.projects = candidate;
        Ok(())
    }

    fn save_project(&mut self, cx: &mut Context<Self>) {
        let Some(name) = self.project_dialog.clone() else {
            return;
        };
        let layout = self.capture_layout();
        let mut library = self.projects.clone();
        library.remember_layout(layout.clone());
        let result = library
            .create(&name, layout)
            .and_then(|id| {
                library.sidebar_for_mut(Some(id)).unwrap().todoist_create_request = Some(todoist::request_id());
                self.persist_library(library).map(|_| id)
            })
            .and_then(|id| self.switch_project(Some(id), cx));
        match result {
            Ok(()) => {
                self.project_dialog = None;
                self.project_dialog_error = None;
                self.collapsed_sections.remove("PROJECTS");
                self.collapsed_sections.remove("LOCATIONS");
                self.set_notice(format!("Created project ‘{}’", name.trim()), cx);
                self.open_todoist(cx);
            }
            Err(error) => {
                self.project_dialog_error = Some(error);
                cx.notify();
            }
        }
    }

    fn switch_project(&mut self, id: Option<u64>, cx: &mut Context<Self>) -> Result<(), String> {
        self.sidebar_menu = None;
        if self.projects.active == id {
            cx.notify();
            return Ok(());
        }
        let mut library = self.projects.clone();
        library.remember_layout(self.capture_layout());
        let layout = match id {
            Some(id) => library
                .items
                .iter()
                .find(|p| p.id == id)
                .map(|p| p.layout.clone())
                .ok_or("Project not found")?,
            None => library
                .personal_layout
                .clone()
                .unwrap_or_else(|| self.capture_layout()),
        };
        let available = layout
            .available()
            .ok_or("This project's folders are unavailable. Reconnect the drive and try again.")?;
        let old = self.projects.active;
        library.active = id;
        self.close_palette(cx);
        self.space_map = None;
        // Persist the outgoing layout and destination together before switching.
        self.persist_library(library)?;
        self.activate_project_sidebar(cx);
        self.server_dialog = None;
        self.ssh_ask = false;
        let current = ProjectSession {
            panes: std::mem::take(&mut self.panes),
            active_pane: self.active_pane,
            split_ratio: self.split_ratio,
        };
        self.project_sessions.insert(old, current);
        self.apply_layout(&available);
        if let Some(live) = self.project_sessions.remove(&id) {
            self.panes = live.panes;
            self.active_pane = live.active_pane;
            self.split_ratio = live.split_ratio;
        }
        self.terminal_resize = None;
        for pane in 0..self.panes.len() {
            self.reload_pane(pane, cx);
            self.sync_terminal_directory(pane);
        }
        self.save_session();
        self.todoist.scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.ensure_todoist(false, cx);
        cx.notify();
        Ok(())
    }

    pub(super) fn restore_project(&mut self, id: u64, cx: &mut Context<Self>) {
        match self.switch_project(Some(id), cx) {
            Ok(()) => {
                let name = self
                    .projects
                    .items
                    .iter()
                    .find(|p| p.id == id)
                    .unwrap()
                    .name
                    .clone();
                self.set_notice(format!("Opened project ‘{name}’"), cx);
            }
            Err(error) => self.set_notice(error, cx),
        }
    }

    fn restore_personal(&mut self, cx: &mut Context<Self>) {
        if let Err(error) = self.switch_project(None, cx) {
            self.set_notice(error, cx);
        }
    }

    pub(super) fn remove_project(&mut self, id: u64, cx: &mut Context<Self>) {
        self.sidebar_menu = None;
        if self.projects.active == Some(id) {
            if let Err(error) = self.switch_project(None, cx) {
                self.set_notice(error, cx);
                return;
            }
        }
        let mut library = self.projects.clone();
        library.items.retain(|project| project.id != id);
        match self.persist_library(library) {
            Ok(()) => {
                self.project_sessions.remove(&Some(id));
                self.set_notice("Project removed. Your files are unchanged.".into(), cx);
            }
            Err(error) => self.set_notice(error, cx),
        }
    }

    pub(super) fn handle_project_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = &ev.keystroke;
        match key.key.as_str() {
            "escape" => {
                self.project_dialog = None;
                cx.notify();
            }
            "enter" => self.save_project(cx),
            "backspace" => {
                if let Some(name) = self.project_dialog.as_mut() {
                    name.pop();
                }
                cx.notify();
            }
            "v" if key.modifiers.platform => {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    if let Some(name) = self.project_dialog.as_mut() {
                        name.push_str(text.trim());
                    }
                    cx.notify();
                }
            }
            _ if !key.modifiers.platform && !key.modifiers.control => {
                if let Some(text) = &key.key_char {
                    if !text.chars().any(char::is_control) {
                        if let Some(name) = self.project_dialog.as_mut() {
                            name.push_str(text);
                        }
                        cx.notify();
                    }
                }
            }
            _ => {}
        }
    }

    pub(super) fn toggle_schnauz_split(&mut self, cx: &mut Context<Self>) {
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
            pane.tabs[0].entry_filter = self.active_tab().entry_filter;
            pane.tabs[0].show_hidden = self.active_tab().show_hidden;
            pane.tabs[0].sort_key = self.active_tab().sort_key;
            pane.tabs[0].sort_asc = self.active_tab().sort_asc;
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

    pub(super) fn render_schnauz_toolbar(&self, cx: &Context<Self>) -> AnyElement {
        let t = theme();
        let filter = self.active_tab().entry_filter;
        let filtered = filter != EntryFilter::All;
        let tint = filter_tint(t);
        let project = self
            .projects
            .active
            .and_then(|id| self.projects.items.iter().find(|project| project.id == id));
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap_2()
            .w_full()
            .h(px(TITLEBAR_H))
            .pl(px(88.0))
            .pr_4()
            .bg(rgb(if filtered { tint } else { t.sidebar }))
            .border_b_1()
            .border_color(rgb(if filtered { t.accent } else { t.border }))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w(px(96.0))
                    .flex_none()
                    .child(
                        div().flex().items_center().gap_1()
                            .child(div().text_size(px(14.0)).font_weight(FontWeight::SEMIBOLD).child("Schnauz"))
                            .child(self.render_search_mascot(cx)),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .truncate()
                            .text_color(rgb(t.text_muted))
                            .child(if filtered {
                                format!("{} · Filter active", filter.description())
                            } else {
                                project
                                    .map(|w| w.name.clone())
                                    .unwrap_or_else(|| "Personal".into())
                            }),
                    ),
            )
            .child(self.render_file_search(cx))
            .child(
                schnauz_button(
                    "schnauz-split",
                    if self.panes.len() > 1 {
                        "Join panes"
                    } else {
                        "Split view"
                    },
                    false,
                )
                .tooltip(tip("Split or join panes · ⌘⇧D"))
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_schnauz_split(cx))),
            )
            .child(
                schnauz_button(
                    "schnauz-terminal",
                    "Terminal",
                    self.active_tab().terminal_open,
                )
                .tooltip(tip("Open a full terminal in this folder"))
                .on_click(
                    cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.toggle_terminal(window, cx)
                    }),
                ),
            )
            .child(
                schnauz_button("schnauz-preview", "Preview", prefs().preview)
                    .tooltip(tip("Show or hide the preview panel · ⌘⇧I"))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.show_file_preview(cx))),
            )
            .when(cx.global::<todoist::Global>().account.is_some(), |d| d.child(schnauz_button("schnauz-todoist", "Todoist", self.todoist.open)
                .tooltip(tip("Todoist tasks for the current project"))
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    if this.todoist.open { this.todoist.open = false; cx.notify(); }
                    else { this.open_todoist(cx); }
                }))))
            .child(
                schnauz_button("schnauz-save", "New project", true)
                    .tooltip(tip("Create a project from these tabs · ⌘⇧S"))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.open_project_dialog(window, cx)
                    })),
            )
            .into_any_element()
    }

    pub(super) fn push_project_sidebar(
        &self,
        items: &mut Vec<AnyElement>,
        cx: &Context<Self>,
    ) {
        let t = theme();
        let add =
            div()
                .id("project-add")
                .px_1()
                .cursor_pointer()
                .text_color(rgb(t.text_muted))
                .hover(|s| s.text_color(rgb(t.accent)))
                .child("+")
                .tooltip(tip("New project"))
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.open_project_dialog(window, cx)
                }))
                .into_any_element();
        items.push(self.section_header_el("PROJECTS", Some(add), cx));
        if self.collapsed_sections.contains("PROJECTS") {
            return;
        }
        if self.projects.items.is_empty() {
            items.push(
                div()
                    .mx_3()
                    .mt_2()
                    .mb_1()
                    .p_3()
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(t.border))
                    .text_size(px(UI_SMALL_FONT_SIZE))
                    .text_color(rgb(t.text_muted))
                    .child("Create a project for its own folders, tags, servers and tabs.")
                    .into_any_element(),
            );
        }
        items.push(
            div()
                .id("project-personal")
                .mx_2()
                .mt_1()
                .px_3()
                .py_2()
                .rounded_lg()
                .flex()
                .items_center()
                .gap_2()
                .cursor_pointer()
                .when(self.projects.active.is_none(), |s| s.bg(rgb(t.selected)))
                .text_color(rgb(if self.projects.active.is_none() {
                    t.accent
                } else {
                    t.text_muted
                }))
                .hover(|s| s.bg(rgb(t.hover)))
                .child("⌂")
                .child("Personal")
                .tooltip(tip("Your general browsing tabs and locations"))
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    window.focus(&this.focus);
                    this.restore_personal(cx);
                }))
                .into_any_element(),
        );
        for project in &self.projects.items {
            let id = project.id;
            let active = self.projects.active == Some(id);
            items.push(
                div()
                    .id(SharedString::from(format!("project-{id}")))
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
                            .child(project.name.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .child(project.layout.tab_count().to_string()),
                    )
                    .tooltip(tip(format!(
                        "{} · {} tab(s) · {} location(s)\nChanges are remembered automatically",
                        project.name,
                        project.layout.tab_count(),
                        project.sidebar.locations.len()
                    )))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        window.focus(&this.focus);
                        this.restore_project(id, cx)
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            this.open_sidebar_menu(
                                f32::from(event.position.x),
                                f32::from(event.position.y),
                                SidebarTarget::Project(id),
                                cx,
                            );
                            cx.stop_propagation();
                        }),
                    )
                    .into_any_element(),
            );
        }
    }

    pub(super) fn render_project_dialog(&self, cx: &Context<Self>) -> AnyElement {
        let t = theme();
        let name = self.project_dialog.clone().unwrap_or_default();
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
                    this.project_dialog = None;
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
                            .child("Create a project"),
                    )
                    .child(div().text_size(px(UI_FONT_SIZE)).text_color(rgb(t.text_muted)).child(
                        "Your own locations, tabs and split panes. Changes are remembered automatically.",
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
                                        "Project name".into()
                                    } else {
                                        name
                                    }),
                            )
                            .child(div().w(px(1.0)).h(px(17.0)).bg(rgb(t.accent))),
                    )
                    .children(self.project_dialog_error.as_ref().map(|error| {
                        div()
                            .text_size(px(UI_SMALL_FONT_SIZE))
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
                                    .text_size(px(UI_SMALL_FONT_SIZE))
                                    .text_color(rgb(t.text_muted))
                                    .child("↵ Create    Esc Cancel"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .child(
                                        schnauz_button("project-cancel", "Cancel", false)
                                            .on_click(cx.listener(
                                                |this, _: &ClickEvent, _, cx| {
                                                    this.project_dialog = None;
                                                    cx.notify();
                                                },
                                            )),
                                    )
                                    .child(
                                        schnauz_button("project-confirm", "Create project", true)
                                            .on_click(cx.listener(
                                                |this, _: &ClickEvent, _, cx| {
                                                    this.save_project(cx)
                                                },
                                            )),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn render_schnauz_status(&self, cx: &Context<Self>) -> AnyElement {
        let t = theme();
        let tab = self.active_tab();
        let searching = self.search_in_pane(self.active_pane);
        let count = if searching { self.palette_items.len() } else if tab.is_filtered() {
            tab.find_results.len()
        } else {
            tab.entries.len()
        };
        let count_label = if searching {
            format!("{count} result{}",if count == 1 {""} else {"s"})
        } else if tab.is_filtered() {
            format!("{count} of {} items", tab.entries.len())
        } else {
            format!("{count} item{}",if count == 1 {""} else {"s"})
        };
        let label = if tab.selection.is_empty() {
            count_label
        } else {
            format!("{} selected · {count_label}", tab.selection.len())
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
            .when(self.file_search.stopped, |bar| {
                bar.child(
                    div()
                        .flex_none()
                        .text_color(rgb(t.accent))
                        .child("Search stopped · Results may be incomplete"),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(display_path(&tab.current_dir)),
            )
            .child(
                div()
                    .id("schnauz-theme")
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
                    .id("schnauz-settings")
                    .cursor_pointer()
                    .hover(|s| s.text_color(rgb(t.accent)))
                    .child(ui_icon(Icon::Settings, 14., rgb(t.text_muted)))
                    .child("Settings  ⌘,")
                    .on_click(|_: &ClickEvent, _, cx: &mut App| open_settings_window(cx)),
            )
            .into_any_element()
    }
}

/// A restrained accent wash that works with both light and dark palettes.
fn filter_tint(t: Theme) -> u32 {
    let blend = |shift: u32| {
        ((((t.sidebar >> shift) & 0xff) * 7 + ((t.accent >> shift) & 0xff)) / 8) << shift
    };
    blend(16) | blend(8) | blend(0)
}

pub(super) fn schnauz_button(
    id: &'static str,
    label: &'static str,
    accented: bool,
) -> gpui::Stateful<gpui::Div> {
    let t = theme();
    let compact = id == "schnauz-split";
    div()
        .id(id)
        .flex_none()
        .h(px(30.0))
        .px_2()
        .when(compact, |d| d.w(px(32.)).px_0())
        .rounded_lg()
        .flex()
        .items_center()
        .gap_1p5()
        .justify_center()
        .text_size(px(UI_SMALL_FONT_SIZE))
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
        .children(match id {
            "schnauz-split" => Some(Icon::Split), "schnauz-terminal" => Some(Icon::Terminal),
            "schnauz-preview" => Some(Icon::Preview), "schnauz-todoist" => Some(Icon::Tasks),
            "schnauz-save" => Some(Icon::Project), _ => None,
        }.map(|icon| ui_icon(icon, 15., rgb(if accented { 0xffffff } else { t.text_muted }))))
        .when(!compact, |d| d.child(label))
}

pub(super) fn toggle_inspector(cx: &mut App) {
    let mut next = prefs();
    next.preview = !next.preview;
    next.info = next.preview;
    apply_prefs(next, cx);
}
