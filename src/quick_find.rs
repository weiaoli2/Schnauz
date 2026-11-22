//! Plain filename substring highlighting over the current displayed order.
use super::*;

pub(super) struct State {
    project: Option<u64>,
    tab: u64,
    directory: PathBuf,
    search: Option<u64>,
    text: String,
    folded: String,
}

impl Schnauz {
    fn quick_find_context(&self, pane: usize, state: &State) -> bool {
        let tab = self.tab(pane);
        pane == self.active_pane
            && state.project == self.projects.active
            && state.tab == tab.id
            && state.directory == tab.current_dir
            && state.search == self.search_in_pane(pane).then_some(self.search_gen)
    }

    pub(super) fn reconcile_quick_find(&mut self) {
        if self.quick_find.as_ref().is_some_and(|state| {
            !self.quick_find_context(self.active_pane, state)
                || self.rename.is_some()
                || self.active_tab().editing_path.is_some()
                || self.active_tab().find_query.is_some()
                || self.palette_open && self.file_search.editing
        }) {
            self.quick_find = None;
        }
    }

    pub(super) fn quick_find_matches(&self, pane: usize, name: &str) -> bool {
        self.quick_find.as_ref().is_some_and(|state| {
            self.quick_find_context(pane, state)
                && !state.folded.is_empty()
                && name.to_lowercase().contains(&state.folded)
        })
    }

    /// Called only after modal/editor routing, before ordinary file shortcuts.
    pub(super) fn quick_find_key(
        &mut self,
        event: &KeyDownEvent,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let key = &event.keystroke;
        if !self.focus.is_focused(window)
            || key.modifiers.platform
            || key.modifiers.control
            || key.modifiers.alt
            || self.palette_open && self.file_search.editing
            || self.text_view.as_ref().is_some_and(|view| view.focused())
        {
            return false;
        }
        self.reconcile_quick_find();
        match key.key.as_str() {
            "escape" if self.quick_find.is_some() => self.quick_find = None,
            "backspace" | "delete" if self.quick_find.is_some() => {
                let state = self.quick_find.as_mut().unwrap();
                state.text.pop();
                state.folded = state.text.to_lowercase();
                // Keep the empty box open so held/repeated Backspace cannot
                // fall through to deleting the selected files.
            }
            _ => {
                let Some(text) = key
                    .key_char
                    .as_deref()
                    .filter(|text| !text.is_empty() && !text.chars().any(char::is_control))
                else {
                    return false;
                };
                // A bare space does not start a search; spaces within the query
                // are literal filename characters, never AND operators.
                if self.quick_find.is_none() && text.chars().all(char::is_whitespace) {
                    return false;
                }
                if self.quick_find.is_none() {
                    let tab = self.active_tab();
                    self.quick_find = Some(State {
                        project: self.projects.active,
                        tab: tab.id,
                        directory: tab.current_dir.clone(),
                        search: self
                            .search_in_pane(self.active_pane)
                            .then_some(self.search_gen),
                        text: String::new(),
                        folded: String::new(),
                    });
                }
                let state = self.quick_find.as_mut().unwrap();
                let remaining = 256usize.saturating_sub(state.text.chars().count());
                state.text.extend(text.chars().take(remaining));
                state.folded = state.text.to_lowercase();
            }
        }
        self.scroll_to_quick_find();
        cx.stop_propagation();
        cx.notify();
        true
    }

    fn scroll_to_quick_find(&self) {
        let pane = self.active_pane;
        if self.quick_find.is_none() {
            return;
        }
        if self.search_in_pane(pane) {
            if let Some(index) = self
                .palette_items
                .iter()
                .position(|item| self.quick_find_matches(pane, &item.title))
            {
                self.file_search
                    .scroll
                    .scroll_to_item_strict(index, ScrollStrategy::Top);
            }
            return;
        }
        let tab = self.tab(pane);
        let Some(index) = tab
            .visible_entries()
            .position(|entry| self.quick_find_matches(pane, &entry.name))
        else {
            return;
        };
        let row = match tab.view {
            ViewMode::Icons => tab.sticky_icon_row(index, self.icon_cols(pane)),
            ViewMode::List => {
                index
                    + usize::from(
                        !tab.is_filtered()
                            && prefs().show_parent
                            && tab.current_dir.parent().is_some(),
                    )
            }
        };
        tab.scroll_handle
            .scroll_to_item_strict(row, ScrollStrategy::Top);
    }

    pub(super) fn render_quick_find(&self, pane: usize, cx: &Context<Self>) -> Option<AnyElement> {
        let state = self
            .quick_find
            .as_ref()
            .filter(|state| self.quick_find_context(pane, state))?;
        Some(
            div()
                .id("quick-filename-find")
                .flex()
                .items_center()
                .gap_2()
                .min_w_0()
                .tooltip(tip(
                    "Highlight names containing this text · Backspace to edit · Escape to clear",
                ))
                .child(ui_icon(Icon::Search, 14., rgb(CHROME_HIGHLIGHT)))
                .child(div().min_w_0().truncate().child(state.text.clone()))
                .child(
                    div()
                        .flex_none()
                        .w(px(1.))
                        .h(px(13.))
                        .bg(rgb(CHROME_HIGHLIGHT)),
                )
                .child(
                    div()
                        .id("clear-quick-filename-find")
                        .flex_none()
                        .size(px(16.))
                        .rounded_sm()
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(theme().hover)))
                        .child(ui_icon(Icon::Close, 12., rgb(theme().text_muted)))
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            this.quick_find = None;
                            window.focus(&this.focus);
                            cx.notify();
                        })),
                )
                .into_any_element(),
        )
    }
}
