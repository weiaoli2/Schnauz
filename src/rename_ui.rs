//! Shared inline name editor and delayed, Finder-style name clicks.
use super::*;

pub(super) type EditText = (String, usize, Option<usize>, bool);

impl Shuffle {
    pub(super) fn rename_text(&self, pane: usize, path: &Path) -> Option<EditText> {
        self.rename
            .as_ref()
            .filter(|r| r.pane == pane && r.path == path)
            .map(|r| {
                (
                    r.text.clone(),
                    r.cursor,
                    r.anchor,
                    self.rename_error().is_some(),
                )
            })
    }

    /// Wait past the system double-click interval. A subsequent press cancels
    /// this task during capture, even if the second press is held down.
    pub(super) fn schedule_click_rename(
        &mut self,
        pane: usize,
        path: PathBuf,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let tab_index = self.pane(pane).active;
        let directory = self.tab(pane).current_dir.clone();
        let view = self.tab(pane).view;
        let search = self.search_in_pane(pane).then_some(self.search_gen);
        let interval = objc2_app_kit::NSEvent::doubleClickInterval();
        let delay = Duration::try_from_secs_f64(interval)
            .unwrap_or(Duration::from_millis(500))
            .saturating_add(Duration::from_millis(30));
        self.pending_click_rename = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(delay).await;
            let _ = this.update_in(cx, |this, window, cx| {
                if pane >= this.panes.len()
                    || this.active_pane != pane
                    || this.pane(pane).active != tab_index
                    || this.tab(pane).current_dir != directory
                    || this.tab(pane).view != view
                    || this.search_in_pane(pane).then_some(this.search_gen) != search
                    || this.rename.is_some()
                    || this.context_menu.is_some()
                    || this.native_menu_pending
                    || this.space_map.is_some()
                    || !window.is_window_active()
                    || !this.focus.is_focused(window)
                    || this.tab(pane).selection.len() != 1
                    || !this.tab(pane).selection.contains(&path)
                    || !this.display_paths(pane).contains(&path)
                {
                    return;
                }
                this.begin_rename(pane, path, window, cx);
            });
        }));
    }
}

/// Same editor in List, Icons and search results, including errors and caret.
pub(super) fn entry_name(name: &str, edit: Option<EditText>) -> AnyElement {
    let t = theme();
    let Some((text, cursor, anchor, invalid)) = edit else {
        return div()
            .flex_1()
            .min_w_0()
            .truncate()
            .text_color(rgb(t.text))
            .child(name.to_string())
            .into_any_element();
    };
    let edge = if invalid { RENAME_ERR_COLOR } else { t.accent };
    let field = div()
        .flex_1()
        .min_w_0()
        .px_1()
        .rounded_sm()
        .bg(rgb(t.bg))
        .border_1()
        .border_color(rgb(edge))
        .text_color(rgb(if invalid { RENAME_ERR_COLOR } else { t.text }))
        .flex()
        .items_center();
    let cursor = cursor.min(text.chars().count());
    if let Some(anchor) = anchor.filter(|a| *a != cursor) {
        let (lo, hi) = (
            char_byte(&text, anchor.min(cursor)),
            char_byte(&text, anchor.max(cursor)),
        );
        field
            .child(div().flex_none().child(text[..lo].to_string()))
            .child(
                div()
                    .flex_none()
                    .bg(Theme::alpha(edge, 0x66))
                    .rounded_sm()
                    .child(text[lo..hi].to_string()),
            )
            .child(div().flex_none().child(text[hi..].to_string()))
            .into_any_element()
    } else {
        let at = char_byte(&text, cursor);
        field
            .child(div().flex_none().child(text[..at].to_string()))
            .child(div().flex_none().w(px(1.5)).h(px(14.0)).bg(rgb(t.text)))
            .child(div().flex_none().child(text[at..].to_string()))
            .into_any_element()
    }
}
