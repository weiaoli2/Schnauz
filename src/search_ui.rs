//! Filename/content search controls and cancellable live results.
use super::*;
use crate::file_search::{self, Mode, Scope};

pub(super) struct State {
    project: Option<u64>,
    history_recorded: bool,
    pub(super) history_pos: Option<usize>,
    pub scope: Scope,
    pub mode: Mode,
    pub busy: bool,
    pub stopped: bool,
    pub editing: bool,
    pub pane: usize,
    pub tab: usize,
    pub scroll: UniformListScrollHandle,
    pub root: PathBuf,
    pub progress: file_search::Progress,
    pub error: Option<String>,
    cancel: Arc<AtomicBool>,
    restore_selection: Option<(HashSet<PathBuf>, Option<PathBuf>)>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            project: None,
            history_recorded: false,
            history_pos: None,
            scope: Scope::default(),
            mode: Mode::default(),
            busy: false,
            stopped: false,
            editing: true,
            pane: 0,
            tab: 0,
            scroll: UniformListScrollHandle::new(),
            root: PathBuf::new(),
            progress: Default::default(),
            error: None,
            cancel: Arc::new(AtomicBool::new(false)),
            restore_selection: None,
        }
    }
}
impl State {
    pub fn cancel(&mut self) {
        self.cancel.store(true, Ordering::Release);
        self.busy = false;
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.cancel();
    }
}

impl Schnauz {
    pub(super) fn remember_file_search(&mut self, cx: &mut Context<Self>) {
        if !prefs().search_history || !self.palette_open || self.file_search.history_recorded || self.query.trim().is_empty() { return; }
        let search = projects::RecentSearch {
            query: self.query.clone(),
            content: self.file_search.mode == Mode::Content,
            drive: self.file_search.scope == Scope::Drive,
        };
        let mut candidate = self.projects.clone();
        let Some(sidebar) = candidate.sidebar_for_mut(self.file_search.project) else { return; };
        if sidebar.remember_search(search) {
            if let Err(error) = self.persist_library(candidate, cx) {
                self.set_notice(error, cx);
                return;
            }
        }
        self.file_search.history_recorded = true;
    }

    /// Keyboard recall and the recent-search menu use the same project list.
    pub(super) fn recall_search_history(&mut self, older: bool, cx: &mut Context<Self>) {
        let history = &self.projects.sidebar().recent_searches;
        if history.is_empty() {
            return;
        }
        let next = if older {
            Some(
                self.file_search
                    .history_pos
                    .map_or(0, |i| (i + 1).min(history.len() - 1)),
            )
        } else {
            self.file_search.history_pos.and_then(|i| i.checked_sub(1))
        };
        let search = next.map(|i| history[i].clone());
        self.query = search.as_ref().map(|s| s.query.clone()).unwrap_or_default();
        if let Some(search) = search {
            self.file_search.mode = if search.content {
                Mode::Content
            } else {
                Mode::File
            };
            self.file_search.scope = if search.drive {
                Scope::Drive
            } else {
                Scope::CurrentFolder
            };
        }
        self.query_cursor = self.query.chars().count();
        self.query_anchor = None;
        self.refresh_file_search(cx);
        self.file_search.history_pos = next;
        self.file_search.history_recorded = true;
        cx.notify();
    }

    fn recent_search_nodes(&self) -> Vec<MenuNode> {
        let project = self.projects.active;
        let mut nodes: Vec<_> = self.projects.sidebar().recent_searches.iter().map(|search| {
            let label = format!("{} · {} · {}", search.query.chars().take(70).collect::<String>(),
                if search.content { "Content" } else { "File" },
                if search.drive { "Drive" } else { "Current Folder" });
            let search = search.clone();
            mi(label, move |this, window, cx| {
                this.close_context_menu(cx);
                if this.projects.active != project { return; }
                this.remember_file_search(cx);
                this.focus_file_search(true, window, cx);
                this.query = search.query.clone();
                this.query_cursor = this.query.chars().count();
                this.query_anchor = None;
                this.file_search.mode = if search.content { Mode::Content } else { Mode::File };
                this.file_search.scope = if search.drive { Scope::Drive } else { Scope::CurrentFolder };
                this.refresh_file_search(cx);
            })
        }).collect();
        if nodes.is_empty() {
            nodes.push(MenuNode::Disabled("No recent searches in this project".into()));
        } else {
            nodes.push(MenuNode::Sep);
            nodes.push(mi("Clear Search History", move |this, _, cx| {
                this.close_context_menu(cx);
                let mut candidate = this.projects.clone();
                let Some(sidebar) = candidate.sidebar_for_mut(project) else { return; };
                sidebar.recent_searches.clear();
                match this.persist_library(candidate, cx) {
                    Ok(()) if this.file_search.project == project => this.file_search.history_recorded = true,
                    Err(error) => this.set_notice(error, cx),
                    _ => {},
                }
            }));
        }
        nodes
    }

    pub(super) fn search_edit_nodes(&self) -> Vec<MenuNode> {
        let selected = self.query_sel().is_some();
        vec![
            if selected {
                mi("Cut", |this, _, cx| this.copy_query_selection(true, cx))
            } else {
                MenuNode::Disabled("Cut".into())
            },
            if selected {
                mi("Copy", |this, _, cx| this.copy_query_selection(false, cx))
            } else {
                MenuNode::Disabled("Copy".into())
            },
            mi("Paste", |this, _, cx| this.paste_query(cx)),
            MenuNode::Sep,
            mi("Select All", |this, _, cx| {
                this.query_anchor = Some(0);
                this.query_cursor = this.query.chars().count();
                cx.notify();
            }),
        ]
    }
    pub(super) fn copy_query_selection(&mut self, cut: bool, cx: &mut Context<Self>) {
        if let Some((lo, hi)) = self.query_sel() {
            let text = self.query[self.query_byte(lo)..self.query_byte(hi)].to_owned();
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            if cut {
                self.query_delete_sel();
                self.refresh_palette(cx);
            }
        }
    }

    pub(super) fn paste_query(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            // Keep this a single-line field, without stripping meaningful spaces.
            let text = text.replace("\r\n", "\n").replace(['\r', '\n', '\t'], " ");
            let text: String = text.chars().filter(|ch| !ch.is_control()).collect();
            self.palette_insert(&text);
            self.refresh_palette(cx);
        }
    }

    pub(super) fn search_in_pane(&self, pane: usize) -> bool {
        self.palette_open
            && self.file_search.pane == pane
            && self.pane(pane).active == self.file_search.tab
    }
    pub(super) fn stop_file_search(&mut self, cx: &mut Context<Self>) {
        self.remember_file_search(cx);
        self.file_search.cancel();
        self.file_search.stopped = true;
        self.search_gen = self.search_gen.wrapping_add(1);
        cx.notify();
    }
    pub(super) fn focus_file_search(
        &mut self,
        reset: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let existing = !reset && self.search_in_pane(self.active_pane);
        if reset {
            self.file_search.cancel();
            self.file_search = State::default();
            self.query.clear();
            self.query_cursor = 0;
            self.query_anchor = None;
        }
        self.file_search.editing = true;
        self.file_search.pane = self.active_pane;
        self.file_search.tab = self.pane(self.active_pane).active;
        self.palette_open = true;
        self.context_menu = None;
        let pane = self.active_pane;
        let tab = self.tab_mut(pane);
        tab.find_query = None;
        tab.editing_path = None;
        tab.recompute_results();
        self.file_search.history_pos = None;
        window.focus(&self.focus);
        if !existing {
            self.refresh_file_search(cx);
        } else {
            cx.notify();
        }
    }

    pub(super) fn refresh_file_search_preserving_selection(&mut self, cx: &mut Context<Self>) {
        let kept = (
            self.active_tab().selection.clone(),
            self.active_tab().anchor.clone(),
        );
        self.refresh_file_search(cx);
        self.file_search.restore_selection = Some(kept);
    }
    fn restore_search_selection(&mut self, done: bool, cx: &mut Context<Self>) {
        let Some((wanted, anchor)) = &self.file_search.restore_selection else {
            return;
        };
        let kept: HashSet<PathBuf> = self
            .palette_items
            .iter()
            .filter_map(|item| {
                let Action::Open(path, _) = &item.action;
                wanted.contains(path).then(|| path.clone())
            })
            .collect();
        let anchor = anchor
            .as_ref()
            .filter(|p| kept.contains(*p))
            .cloned()
            .or_else(|| kept.iter().next().cloned());
        let pane = self.file_search.pane;
        let changed = self.tab(pane).anchor != anchor;
        self.tab_mut(pane).selection = kept;
        self.tab_mut(pane).anchor = anchor.clone();
        if changed {
            if let Some(path) = anchor {
                if let Some(row) = self
                    .palette_items
                    .iter()
                    .position(|i| matches!(&i.action, Action::Open(p, _) if p == &path))
                {
                    self.selected = row;
                    self.file_search
                        .scroll
                        .scroll_to_item(row, ScrollStrategy::Center);
                }
                self.focus_entry(pane, path, cx);
            }
        }
        if done {
            self.file_search.restore_selection = None;
        }
    }
    pub(super) fn refresh_file_search(&mut self, cx: &mut Context<Self>) {
        self.file_search.project = self.projects.active;
        self.file_search.history_recorded = false;
        self.file_search.history_pos = None;
        self.file_search.restore_selection = None;
        self.file_search.cancel();
        self.file_search.pane = self.active_pane;
        self.file_search.tab = self.pane(self.active_pane).active;
        self.file_search
            .scroll
            .scroll_to_item(0, ScrollStrategy::Top);
        let pane = self.active_pane;
        self.tab_mut(pane).selection.clear();
        self.tab_mut(pane).anchor = None;
        self.cancel_unused_previews();
        self.search_gen = self.search_gen.wrapping_add(1);
        let generation = self.search_gen;
        self.palette_items.clear();
        self.selected = 0;
        self.file_search.progress = Default::default();
        self.file_search.error = None;
        self.file_search.stopped = false;
        let folder = self.active_tab().current_dir.clone();
        self.file_search.root = folder.clone();
        if self.active_tab().remote.is_some() {
            self.file_search.error = Some("Search is available in local folders.".into());
            cx.notify();
            return;
        }
        if self.query.trim().is_empty() {
            cx.notify();
            return;
        }
        let root = if self.file_search.scope == Scope::Drive {
            match file_search::drive_root(&folder) {
                Ok(root) => root,
                Err(_) => {
                    self.file_search.error = Some("Couldn't locate this drive.".into());
                    cx.notify();
                    return;
                }
            }
        } else {
            folder
        };
        self.file_search.root = root.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        self.file_search.cancel = cancel.clone();
        self.file_search.busy = true;
        let history_project = self.file_search.project;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(1200)).await;
            let _ = this.update(cx, |this, cx| {
                if this.search_gen == generation && this.file_search.project == history_project {
                    this.remember_file_search(cx);
                }
            });
        }).detach();
        let request = file_search::Request {
            root,
            mode: self.file_search.mode,
            query: self.query.clone(),
            show_hidden: self.active_tab().show_hidden,
            filter: self.active_tab().entry_filter,
            file_filter: self.projects.sidebar().file_filter.clone(),
            cancel: cancel.clone(),
        };
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(160))
                .await;
            if cancel.load(Ordering::Acquire) {
                return;
            }
            let (tx, rx) = std::sync::mpsc::channel();
            cx.background_spawn(async move { file_search::run(request, tx) })
                .detach();
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(60))
                    .await;
                if cancel.load(Ordering::Acquire) {
                    return;
                }
                let mut updates = Vec::new();
                let mut disconnected = false;
                loop {
                    match rx.try_recv() {
                        Ok(update) => updates.push(update),
                        Err(std::sync::mpsc::TryRecvError::Empty) => break,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            disconnected = true;
                            break;
                        }
                    }
                }
                if updates.is_empty() && !disconnected {
                    continue;
                }
                let complete = updates.iter().any(|u| u.done);
                let done = complete || disconnected;
                let active = this
                    .update(cx, |this, cx| {
                        if this.search_gen != generation || !this.palette_open {
                            return false;
                        }
                        for update in updates {
                            this.file_search.progress = update.progress;
                            this.palette_items
                                .extend(update.hits.into_iter().map(|hit| PaletteItem {
                                    title: path_label(&hit.path),

                                    action: Action::Open(hit.path, hit.is_dir),
                                    is_dir: hit.is_dir,
                                }));
                        }
                        this.restore_search_selection(done, cx);
                        this.file_search.busy = !done;
                        if disconnected && !complete {
                            this.file_search.error =
                                Some("Search stopped before completing.".into());
                        }
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !active {
                    cancel.store(true, Ordering::Release);
                    return;
                }
                if done {
                    return;
                }
            }
        })
        .detach();
    }

    pub(super) fn render_file_search(&self, cx: &Context<Self>) -> AnyElement {
        let t = theme();
        let active = self.palette_open && self.file_search.editing;
        let mut input = div()
            .flex()
            .min_w_0()
            .flex_1()
            .overflow_hidden()
            .whitespace_nowrap();
        if self.query.is_empty() {
            input =
                input
                    .text_color(rgb(t.text_dim))
                    .child(if self.file_search.mode == Mode::File {
                        "Search names…"
                    } else {
                        "Search text…"
                    });
        } else if active {
            if let Some((lo, hi)) = self.query_sel() {
                let (a, b) = (self.query_byte(lo), self.query_byte(hi));
                input = input
                    .child(self.query[..a].to_owned())
                    .child(
                        div()
                            .bg(Theme::alpha(t.accent, 0x55))
                            .child(self.query[a..b].to_owned()),
                    )
                    .child(self.query[b..].to_owned());
            } else {
                let b = self.query_byte(self.query_cursor);
                input = input.child(format!("{}│{}", &self.query[..b], &self.query[b..]));
            }
        } else {
            input = input.child(self.query.clone());
        }
        let mut scopes = div()
            .relative()
            .children(ui_checks::probe("top-search-scope"))
            .flex()
            .flex_none()
            .h(px(TOOLBAR_CONTROL_H))
            .items_center()
            .gap_0p5()
            .px_0p5()
            .rounded_md()
            .bg(rgb(t.surface));
        for (id, label, scope) in [
            ("search-drive", "Drive", Scope::Drive),
            ("search-current", "Current Folder", Scope::CurrentFolder),
        ] {
            scopes = scopes.child(
                search_segment(id, label, self.file_search.scope == scope).on_click(cx.listener(
                    move |this, _: &ClickEvent, window, cx| {
                        this.file_search.scope = scope;
                        this.focus_file_search(false, window, cx);
                        this.refresh_file_search(cx);
                    },
                )),
            );
        }
        let mut modes = div()
            .relative()
            .children(ui_checks::probe("top-search-mode"))
            .flex()
            .flex_none()
            .h(px(TOOLBAR_CONTROL_H))
            .items_center()
            .gap_0p5()
            .px_0p5()
            .rounded_md()
            .bg(rgb(t.surface));
        for (id, label, mode) in [
            ("search-content", "Content", Mode::Content),
            ("search-file", "File", Mode::File),
        ] {
            modes = modes.child(
                search_segment(id, label, self.file_search.mode == mode).on_click(cx.listener(
                    move |this, _: &ClickEvent, window, cx| {
                        this.file_search.mode = mode;
                        this.focus_file_search(false, window, cx);
                        this.refresh_file_search(cx);
                    },
                )),
            );
        }
        div()
            .flex()
            .flex_1()
            .min_w_0()
            .items_center()
            .gap_2()
            .child(scopes)
            .child(
                div()
                    .id("schnauz-search")
                    .relative()
                    .children(ui_checks::probe("top-search-field"))
                    .flex_1()
                    .min_w_0()
                    .h(px(TOOLBAR_CONTROL_H))
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_2()
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(if active { t.accent } else { t.border }))
                    .bg(rgb(t.bg))
                    .cursor(CursorStyle::IBeam)
                    .text_size(px(UI_SMALL_FONT_SIZE))
                    .child(ui_icon(Icon::Search, 16., rgb(t.text_muted)))
                    .child(input)
                    .child(
                        div().id("search-history").flex_none().size(px(20.))
                            .flex().items_center().justify_center().rounded_sm().cursor_pointer()
                            .hover(|d| d.bg(rgb(t.hover)))
                            .tooltip(tip("Recent searches for this project"))
                            .child(ui_icon(Icon::History, 14., rgb(t.text_muted)))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                                this.remember_file_search(cx);
                                this.context_menu = Some(ContextMenu {
                                    x: f32::from(event.position().x), y: f32::from(event.position().y),
                                    target: None, nodes: this.recent_search_nodes(),
                                });
                                cx.stop_propagation();
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_size(px(9.0))
                            .text_color(rgb(t.text_dim))
                            .child("⌘P"),
                    )
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.focus_file_search(false, window, cx)
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                            this.focus_file_search(false, window, cx);
                            this.context_menu = Some(ContextMenu {
                                x: f32::from(event.position.x),
                                y: f32::from(event.position.y),
                                target: None,
                                nodes: this.search_edit_nodes(),
                            });
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
            .child(modes)
            .into_any_element()
    }

    pub(super) fn click_search_entry(
        &mut self,
        row: usize,
        name_click: bool,
        ev: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.suppress_entry_click {
            return;
        }
        let Some(item) = self.palette_items.get(row) else {
            return;
        };
        let Action::Open(path, is_dir) = item.action.clone();
        let pane = self.file_search.pane;
        if name_click && !ev.modifiers().modified() && self.rename_text(pane, &path, cx).is_some() {
            return;
        }
        self.selected = row;
        self.file_search.editing = false;
        self.file_search.restore_selection = None;
        window.focus(&self.focus);
        // Keep the established search behavior: opening ends the search first.
        if ev.click_count() >= 2 && !ev.modifiers().modified() {
            self.pending_click_rename = None;
            self.open_search_result(cx);
        } else {
            self.click_entry(pane, path, is_dir, name_click, ev, window, cx);
        }
    }

    pub(super) fn select_search_result(&mut self, row: usize, cx: &mut Context<Self>) {
        self.file_search.restore_selection = None;
        let Some(item) = self.palette_items.get(row) else {
            return;
        };
        let Action::Open(path, _) = item.action.clone();
        self.selected = row;
        self.file_search.editing = false;
        self.select_entry(self.file_search.pane, path, cx);
    }

    fn open_search_result(&mut self, cx: &mut Context<Self>) {
        let Some(item) = self.palette_items.get(self.selected) else {
            return;
        };
        let Action::Open(path, is_dir) = item.action.clone();
        let pane = self.file_search.pane;
        self.close_palette(cx);
        self.open_path(pane, path, is_dir, cx);
    }

    fn search_result_menu(
        &mut self,
        row: usize,
        x: f32,
        y: f32,
        direct_actions: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(item) = self.palette_items.get(row) else {
            return;
        };
        let Action::Open(path, is_dir) = item.action.clone();
        let pane = self.file_search.pane;
        self.selected = row;
        self.file_search.editing = false;
        self.file_search.restore_selection = None;
        self.select_context_entry(pane, path.clone(), cx);
        if direct_actions {
            self.open_quick_actions(pane, x, y, path, is_dir, cx);
            return;
        }
        let mut nodes = vec![
            mi("Show in Folder", |this, _, cx| {
                this.close_context_menu(cx);
                this.activate_selection(cx);
            }),
            mi("Open", |this, _, cx| {
                this.close_context_menu(cx);
                this.open_search_result(cx);
            }),
            MenuNode::Sep,
            mi("Copy", move |this, _, cx| {
                this.close_context_menu(cx);
                let targets = this.action_targets(pane);
                this.copy_with_feedback(pane, &targets, cx);
            }),
            mi("Copy Path", |this, _, cx| {
                this.close_context_menu(cx);
                if let Some(path) = this.active_tab().anchor.as_ref() {
                    cx.write_to_clipboard(ClipboardItem::new_string(
                        path.to_string_lossy().into_owned(),
                    ));
                }
            }),
        ];
        nodes.extend(self.add_reference_nodes(pane, &self.action_targets(pane)));
        if is_dir {
            let folder = path.clone();
            nodes.push(mi("Inspect…", move |this, window, cx| {
                this.inspect_folder(pane, folder.clone(), window, cx)
            }));
        }
        let info_path = path.clone();
        nodes.push(mi("Get Info", move |this, _, cx| {
            this.open_get_info(info_path.clone(), cx)
        }));
        self.context_menu = Some(ContextMenu {
            x,
            y,
            target: Some((path, is_dir)),
            nodes,
        });
        cx.notify();
    }

    /// Results occupy the file pane; only visible rows request native icons.
    pub(super) fn render_file_results(&self, cx: &Context<Self>) -> AnyElement {
        let t = theme();
        let summary = if let Some(error) = &self.file_search.error {
            error.clone()
        } else if self.query.trim().is_empty() {
            "Type to search · spaces mean AND · .txt matches the extension".into()
        } else if self.file_search.stopped {
            format!("Search stopped · {} results kept", self.palette_items.len())
        } else if self.file_search.busy {
            format!(
                "Searching… {} found · {} items checked",
                self.palette_items.len(),
                self.file_search.progress.scanned
            )
        } else if self.file_search.progress.limited {
            format!(
                "{}+ results · add more words to narrow your search",
                file_search::RESULT_LIMIT
            )
        } else {
            format!(
                "{} result{}",
                self.palette_items.len(),
                if self.palette_items.len() == 1 {
                    ""
                } else {
                    "s"
                }
            )
        };
        let pane = self.file_search.pane;
        let scroll = self.file_search.scroll.clone();
        let base = scroll.0.borrow().base_handle.clone();
        let has_selection = !self.tab(pane).selection.is_empty();
        let mut panel = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_3()
                    .h(px(34.))
                    .bg(Theme::alpha(t.accent, 0x10))
                    .border_b_1()
                    .border_color(rgb(t.border))
                    .child(div().flex_none().text_color(rgb(t.accent)).child(summary))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(UI_SMALL_FONT_SIZE))
                            .text_color(rgb(t.text_muted))
                            .child(format!(
                                "{} · {}",
                                if self.file_search.scope == Scope::Drive {
                                    "Drive"
                                } else {
                                    "Current Folder + subfolders"
                                },
                                display_path(&self.file_search.root)
                            )),
                    )
                    .when(has_selection, |bar| {
                        bar.child(
                            div()
                                .id("search-show-folder")
                                .flex_none()
                                .cursor_pointer()
                                .text_color(rgb(t.accent))
                                .hover(|s| s.text_color(rgb(t.text)))
                                .child("Show in Folder")
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.activate_selection(cx)
                                })),
                        )
                    })
                    .child(
                        div()
                            .id("search-done")
                            .flex_none()
                            .cursor_pointer()
                            .text_color(rgb(t.text_muted))
                            .child("Done ×")
                            .hover(|s| s.text_color(rgb(t.text)))
                            .tooltip(tip("Close search (Esc)"))
                            .on_click(
                                cx.listener(|this, _: &ClickEvent, _, cx| this.close_palette(cx)),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .h(px(26.))
                    .px_3()
                    .gap_3()
                    .bg(rgb(t.sidebar))
                    .border_b_1()
                    .border_color(rgb(t.border))
                    .text_size(px(UI_SMALL_FONT_SIZE))
                    .text_color(rgb(t.text_muted))
                    .child(div().flex_1().min_w_0().child("Name"))
                    .child(div().flex_1().min_w_0().child("Location")),
            );
        if !self.palette_items.is_empty() {
            panel = panel.child(div().relative().flex_1().min_h_0()
                .child(uniform_list("file-search-results",self.palette_items.len(),cx.processor(move|this,range:std::ops::Range<usize>,_,cx| {
                    let width = this.browser_width(pane).max(1.);
                    let cell_width = ((width-36.)/2.).max(0.);
                    range.filter_map(|i| {
                        let item = this.palette_items.get(i)?;
                        let Action::Open(path,is_dir) = &item.action;
                        let selected = this.tab(pane).selection.contains(path);
                        let location = path.parent().map(display_path).unwrap_or_default();
                        let cloud = if this.tab(pane).remote.is_none() { this.cloud_state(path, *is_dir, cx) } else { CloudSync::Local };
                        Some(div().id(("file-search-hit",i)).w(px(width)).h(px(ROW_H)).flex().items_center().px_3().gap_3()
                            .text_size(px(UI_FONT_SIZE)).cursor_pointer()
                            .when(selected,|s|s.bg(rgb(t.selected)))
                            .when(!selected && this.is_ctx_target(path),|s|s.bg(rgb(t.hover)))
                            .when(!selected,|s|s.hover(|s|s.bg(rgb(t.hover))))
                            .child(div().flex_none().w(px(cell_width)).min_w_0().flex().items_center().gap_2()
                                .child(div().flex_none().child(icon_element(path,*is_dir)))
                                .child(div().id(("search-entry-name",i)).flex_1().min_w_0().flex()
                                    .child(rename_ui::entry_name(&item.title,this.rename_text(pane,path,cx)))
                                    .on_click(cx.listener(move|this,ev:&ClickEvent,window,cx| {
                                        this.click_search_entry(i,true,ev,window,cx);
                                        this.suppress_entry_click = true;
                                    })))
                                .children(cloud_status_icon(cloud,14.)))
                            .child(div().flex_none().w(px(cell_width)).min_w_0().truncate().text_color(rgb(t.text_muted)).child(location))
                            .on_click(cx.listener(move|this,ev:&ClickEvent,window,cx| {
                                this.click_search_entry(i,false,ev,window,cx);
                            }))
                            .on_mouse_down(MouseButton::Right,cx.listener(move|this,ev:&MouseDownEvent,window,cx| {
                                window.focus(&this.focus);
                                this.search_result_menu(i,f32::from(ev.position.x),f32::from(ev.position.y),ev.modifiers.platform,cx);
                                cx.stop_propagation();
                            })))
                    }).collect()
                })).size_full().track_scroll(scroll)
                    .on_scroll_wheel(cx.listener(|this,_:&ScrollWheelEvent,_,cx| {this.pending_click_rename=None;cx.notify();})))
                .children(static_scrollbar_thumb(&base)));
        } else {
            panel = panel.child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(t.text_muted))
                    .child(if self.query.trim().is_empty() {
                        "Search this folder by name or text."
                    } else if self.file_search.busy {
                        "Searching…"
                    } else if self.file_search.stopped {
                        "No results found before stopping."
                    } else {
                        "No matching files or folders."
                    }),
            );
        }
        if self.file_search.progress.unreadable > 0 {
            panel = panel.child(
                div()
                    .flex_none()
                    .px_3()
                    .py_1()
                    .text_size(px(UI_SMALL_FONT_SIZE))
                    .text_color(rgb(t.text_muted))
                    .child("Some locations could not be read."),
            );
        }
        panel.into_any_element()
    }
}
fn search_segment(
    id: &'static str,
    label: &'static str,
    selected: bool,
) -> gpui::Stateful<gpui::Div> {
    let t = theme();
    div()
        .id(id)
        .relative()
        .children(ui_checks::probe(id))
        .h(px(TOOLBAR_CONTROL_H))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .px_2()
        .rounded_md()
        .cursor_pointer()
        .text_size(px(10.0))
        .whitespace_nowrap()
        .text_color(rgb(if selected { t.accent } else { t.text_muted }))
        .when(selected, |d| d.bg(rgb(t.bg)).shadow_sm())
        .hover(|s| s.bg(rgb(t.hover)))
        .child(label)
}
