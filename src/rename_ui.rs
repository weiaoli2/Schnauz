//! Shared inline name editor and delayed, Finder-style name clicks.
use super::*;

pub(super) struct Layout {
    pub bounds: Bounds<gpui::Pixels>,
    line: gpui::ShapedLine,
    offset: f32,
}

#[cfg(feature = "ui-checks")]
pub(super) fn fixture_point(rename: &Rename, index: usize) -> gpui::Point<gpui::Pixels> {
    let layout = rename.layout.borrow();
    let layout = layout.as_ref().expect("Rename field was laid out");
    point(
        layout.bounds.left() + layout.line.x_for_index(char_byte(&rename.text, index))
            - px(layout.offset),
        layout.bounds.center().y,
    )
}
pub(super) struct EditText {
    text: String,
    cursor: usize,
    anchor: Option<usize>,
    invalid: bool,
    pane: usize,
    path: PathBuf,
    owner: gpui::WeakEntity<Schnauz>,
    layout: Rc<RefCell<Option<Layout>>>,
}

impl Schnauz {
    pub(super) fn rename_text(
        &self,
        pane: usize,
        path: &Path,
        cx: &Context<Self>,
    ) -> Option<EditText> {
        self.rename
            .as_ref()
            .filter(|r| r.pane == pane && r.path == path)
            .map(|r| EditText {
                text: r.text.clone(),
                cursor: r.cursor,
                anchor: r.anchor,
                invalid: self.rename_error().is_some(),
                pane,
                path: path.into(),
                owner: cx.entity().downgrade(),
                layout: r.layout.clone(),
            })
    }

    fn rename_pointer(&mut self, position: gpui::Point<gpui::Pixels>, extend: bool, clicks: usize) {
        let Some(rename) = &mut self.rename else {
            return;
        };
        let Some(byte) = rename.layout.borrow().as_ref().map(|layout| {
            layout
                .line
                .closest_index_for_x(position.x - layout.bounds.left() + px(layout.offset))
        }) else {
            return;
        };
        let at = rename.text[..byte.min(rename.text.len())].chars().count();
        if clicks >= 3 {
            rename.anchor = Some(0);
            rename.cursor = rename.text.chars().count();
        } else if clicks == 2 {
            let chars: Vec<char> = rename.text.chars().collect();
            let mut lo = at.min(chars.len().saturating_sub(1));
            let mut hi = lo;
            let word = |c: char| c.is_alphanumeric() || c == '_';
            let class = chars.get(lo).copied().map(word);
            while lo > 0 && Some(word(chars[lo - 1])) == class {
                lo -= 1;
            }
            while hi < chars.len() && Some(word(chars[hi])) == class {
                hi += 1;
            }
            rename.anchor = Some(lo);
            rename.cursor = hi;
        } else {
            if !extend || rename.anchor.is_none() {
                rename.anchor = Some(if extend { rename.cursor } else { at });
            }
            rename.cursor = at;
        }
    }

    pub(super) fn rename_drag(
        &mut self,
        position: gpui::Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) {
        if self.rename.as_ref().is_some_and(|r| r.dragging) {
            self.rename_pointer(position, true, 1);
            cx.notify();
        }
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

/// The marker hugs the filename. Selection stays on the row/cell, and the
/// native-style inline editor retains its own background and hit geometry.
pub(super) fn marked_entry_name(name: &str, edit: Option<EditText>, marker: Option<u32>) -> AnyElement {
    let t = theme();
    let Some(EditText {
        text,
        cursor,
        anchor,
        invalid,
        pane,
        path,
        owner,
        layout,
    }) = edit
    else {
        return div()
            .when(marker.is_none(), |d| d.flex_1())
            .min_w_0()
            .max_w_full()
            .truncate()
            .when_some(marker, |d, color| d.rounded_sm().bg(rgb(color)))
            .text_color(rgb(if marker.is_some() { 0x1b1b1b } else { t.text }))
            .child(name.to_string())
            .into_any_element();
    };
    let edge = if invalid { RENAME_ERR_COLOR } else { t.accent };
    let cursor = char_byte(&text, cursor.min(text.chars().count()));
    let selection = anchor.filter(|a| char_byte(&text, *a) != cursor).map(|a| {
        let a = char_byte(&text, a);
        (a.min(cursor), a.max(cursor))
    });
    div()
        .id("inline-rename-editor")
        .flex_1()
        .min_w_0()
        .h(px(22.))
        .px(px(4.))
        .rounded_sm()
        .bg(rgb(t.bg))
        .border_1()
        .border_color(rgb(edge))
        .text_color(rgb(if invalid { RENAME_ERR_COLOR } else { t.text }))
        .cursor_text()
        .overflow_hidden()
        .on_mouse_down(MouseButton::Left, move |event, window, cx| {
            let _ = owner.update(cx, |this, cx| {
                if !this
                    .rename
                    .as_ref()
                    .is_some_and(|r| r.pane == pane && r.path == path)
                {
                    return;
                }
                this.drag_candidate = None;
                this.rename_pointer(event.position, event.modifiers.shift, event.click_count);
                this.rename.as_mut().unwrap().dragging = true;
                window.focus(&this.focus);
                cx.stop_propagation();
                cx.notify();
            });
        })
        .on_click(|_, _, cx| cx.stop_propagation())
        .child(
            gpui::canvas(
                move |bounds, window, _| {
                    let text: SharedString = text.into();
                    let run = gpui::TextRun {
                        len: text.len(),
                        font: window.text_style().font(),
                        color: rgb(if invalid { RENAME_ERR_COLOR } else { t.text }).into(),
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    let line =
                        window
                            .text_system()
                            .shape_line(text, px(UI_FONT_SIZE), &[run], None);
                    let previous = layout.borrow().as_ref().map_or(0., |l| l.offset);
                    let offset = path_editor::caret_offset(
                        previous,
                        f32::from(line.x_for_index(cursor)),
                        f32::from(line.width),
                        f32::from(bounds.size.width),
                    );
                    *layout.borrow_mut() = Some(Layout {
                        bounds,
                        line: line.clone(),
                        offset,
                    });
                    (line, offset)
                },
                move |bounds, (line, offset), window, cx| {
                    let origin = point(
                        bounds.left() - px(offset),
                        bounds.top() + (bounds.size.height - px(17.)) / 2.,
                    );
                    window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
                        if let Some((lo, hi)) = selection {
                            window.paint_quad(gpui::fill(
                                Bounds::new(
                                    point(origin.x + line.x_for_index(lo), origin.y),
                                    size(line.x_for_index(hi) - line.x_for_index(lo), px(17.)),
                                ),
                                Theme::alpha(edge, 0x66),
                            ));
                        }
                        let _ = line.paint(origin, px(17.), window, cx);
                        window.paint_quad(gpui::fill(
                            Bounds::new(
                                point(origin.x + line.x_for_index(cursor), origin.y),
                                size(px(1.5), px(17.)),
                            ),
                            rgb(t.text),
                        ));
                    });
                },
            )
            .size_full(),
        )
        .into_any_element()
}
