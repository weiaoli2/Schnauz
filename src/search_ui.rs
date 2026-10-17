//! Filename/content search controls and cancellable live results.
use super::*;
use crate::file_search::{self, Mode, Scope};

pub(super) struct State {
    pub inline: bool,
    pub scope: Scope,
    pub mode: Mode,
    pub busy: bool,
    pub root: PathBuf,
    pub progress: file_search::Progress,
    pub error: Option<String>,
    cancel: Arc<AtomicBool>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            inline: true,
            scope: Scope::default(),
            mode: Mode::default(),
            busy: false,
            root: PathBuf::new(),
            progress: Default::default(),
            error: None,
            cancel: Arc::new(AtomicBool::new(false)),
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

impl Shuffle {
    pub(super) fn focus_file_search(
        &mut self,
        reset: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if reset || !self.file_search.inline {
            self.file_search.cancel();
            self.file_search = State::default();
            self.query.clear();
            self.query_cursor = 0;
            self.query_anchor = None;
        }
        self.file_search.inline = true;
        self.palette_open = true;
        self.context_menu = None;
        let pane = self.active_pane;
        let tab = self.tab_mut(pane);
        tab.find_query = None;
        tab.editing_path = None;
        tab.recompute_results();
        self.palette_hist_pos = None;
        window.focus(&self.focus);
        self.refresh_file_search(cx);
    }

    pub(super) fn refresh_file_search(&mut self, cx: &mut Context<Self>) {
        self.file_search.cancel();
        self.file_search.inline = true;
        self.search_gen = self.search_gen.wrapping_add(1);
        let generation = self.search_gen;
        self.palette_items.clear();
        self.selected = 0;
        self.palette_actions = None;
        self.palette_scroll.set_offset(point(px(0.0), px(0.0)));
        self.file_search.progress = Default::default();
        self.file_search.error = None;
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
        let request = file_search::Request {
            root,
            mode: self.file_search.mode,
            query: self.query.clone(),
            show_hidden: self.active_tab().show_hidden,
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
                                    subtitle: hit.path.to_string_lossy().into_owned(),
                                    action: Action::Open(hit.path, hit.is_dir),
                                    is_dir: hit.is_dir,
                                }));
                        }
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
        let active = self.palette_open && self.file_search.inline;
        let mut input = div()
            .flex()
            .min_w_0()
            .flex_1()
            .overflow_hidden()
            .whitespace_nowrap();
        if self.query.is_empty() || !self.file_search.inline {
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
            .flex()
            .flex_none()
            .items_center()
            .gap_0p5()
            .p_0p5()
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
                    },
                )),
            );
        }
        let mut modes = div()
            .flex()
            .flex_none()
            .items_center()
            .gap_0p5()
            .p_0p5()
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
                    },
                )),
            );
        }
        div()
            .flex()
            .flex_1()
            .min_w(px(380.0))
            .items_center()
            .gap_2()
            .child(scopes)
            .child(
                div()
                    .id("studio-search")
                    .flex_1()
                    .min_w_0()
                    .h(px(30.0))
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
                    .child(div().text_color(rgb(t.text_dim)).child("⌕"))
                    .child(input)
                    .child(
                        div()
                            .flex_none()
                            .text_size(px(9.0))
                            .text_color(rgb(t.text_dim))
                            .child("⌘P"),
                    )
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.focus_file_search(false, window, cx)
                    })),
            )
            .child(modes)
            .into_any_element()
    }

    pub(super) fn render_file_results(&self, cx: &Context<Self>) -> AnyElement {
        let t = theme();
        let summary = if let Some(error) = &self.file_search.error {
            error.clone()
        } else if self.query.trim().is_empty() {
            "Type to search · spaces mean AND · .txt matches the extension".into()
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
        let list_h = self.palette_items.len().min(12) as f32 * PALETTE_ROW_H;
        let mut panel = div()
            .id("file-search-panel")
            .w(px(690.0))
            .max_w(relative(0.95))
            .mt_2()
            .flex()
            .flex_col()
            .bg(rgb(t.surface))
            .rounded_lg()
            .border_1()
            .border_color(rgb(t.border_strong))
            .shadow_lg()
            .on_mouse_down(MouseButton::Left, |_, _, cx: &mut App| {
                cx.stop_propagation()
            })
            .child(
                div()
                    .px_3()
                    .py_2()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .border_b_1()
                    .border_color(rgb(t.border))
                    .child(div().text_size(px(UI_SMALL_FONT_SIZE)).child(summary))
                    .child(
                        div()
                            .truncate()
                            .text_size(px(10.0))
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
                    ),
            );
        if !self.palette_items.is_empty() {
            panel = panel.child(
                div()
                    .id("file-search-results")
                    .h(px(list_h))
                    .overflow_y_scroll()
                    .track_scroll(&self.palette_scroll)
                    .children(self.palette_items.iter().enumerate().map(|(i, item)| {
                        div()
                            .id(("file-search-hit", i))
                            .h(px(PALETTE_ROW_H))
                            .px_3()
                            .flex()
                            .items_center()
                            .gap_2()
                            .cursor_pointer()
                            .when(i == self.selected, |d| d.bg(rgb(t.selected)))
                            .hover(|s| s.bg(rgb(t.hover)))
                            .child(icon_element(Path::new(&item.subtitle), item.is_dir))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .truncate()
                                            .text_size(px(UI_FONT_SIZE))
                                            .child(item.title.clone()),
                                    )
                                    .child(
                                        div()
                                            .truncate()
                                            .text_size(px(10.0))
                                            .text_color(rgb(t.text_muted))
                                            .child(item.subtitle.clone()),
                                    ),
                            )
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.selected = i;
                                this.activate_selection(cx)
                            }))
                    })),
            );
        } else if !self.file_search.busy
            && !self.query.trim().is_empty()
            && self.file_search.error.is_none()
        {
            panel = panel.child(
                div()
                    .px_3()
                    .py_4()
                    .text_color(rgb(t.text_muted))
                    .child("No matching files or folders."),
            );
        }
        if self.file_search.progress.unreadable > 0 {
            panel = panel.child(
                div()
                    .px_3()
                    .py_1()
                    .text_size(px(10.0))
                    .text_color(rgb(t.text_muted))
                    .child("Some locations could not be read."),
            );
        }
        panel = panel.child(
            div()
                .px_3()
                .py_2()
                .flex()
                .gap_3()
                .text_size(px(10.0))
                .text_color(rgb(t.text_dim))
                .child("↑↓ Select")
                .child("↩ Show in folder")
                .child("Esc Close"),
        );
        div()
            .absolute()
            .top(px(TITLEBAR_H))
            .left_0()
            .right_0()
            .bottom_0()
            .flex()
            .justify_center()
            .items_start()
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.close_palette(cx);
                    cx.stop_propagation();
                }),
            )
            .child(panel)
            .into_any_element()
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
        .px_2()
        .py_1()
        .rounded_md()
        .cursor_pointer()
        .text_size(px(10.0))
        .whitespace_nowrap()
        .text_color(rgb(if selected { t.accent } else { t.text_muted }))
        .when(selected, |d| d.bg(rgb(t.bg)).shadow_sm())
        .hover(|s| s.bg(rgb(t.hover)))
        .child(label)
}
