//! Single-line address editor with measured mouse selection and caret scrolling.
use super::*;

pub(super) struct Layout {
    pub bounds: gpui::Bounds<gpui::Pixels>,
    line: gpui::ShapedLine,
    offset: f32,
}

pub(super) fn caret_offset(previous: f32, caret: f32, text: f32, viewport: f32) -> f32 {
    let width = (viewport - 3.).max(1.);
    let mut offset = previous.max(0.);
    if caret < offset {
        offset = caret;
    }
    if caret > offset + width {
        offset = caret - width;
    }
    offset.clamp(0., (text - width).max(0.))
}

impl Schnauz {
    /// Enter path-edit mode for a pane: the bar becomes an editable text field.
    pub(super) fn begin_path_edit(
        &mut self,
        pane: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.active_pane = pane;
        let text = self.tab(pane).current_dir.display().to_string();
        let end = text.chars().count();
        let tab = self.tab_mut(pane);
        tab.editing_path = Some(text);
        tab.path_cursor = end;
        tab.path_anchor = Some(0);
        tab.path_dragging = false;
        *tab.path_layout.borrow_mut() = None;
        window.focus(&self.focus);
        cx.notify();
    }

    /// The path editor's selection as a sorted `(start, end)` char range.
    pub(super) fn path_sel(&self, pane: usize) -> Option<(usize, usize)> {
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
    pub(super) fn path_delete_sel(&mut self, pane: usize) -> bool {
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
    pub(super) fn path_insert(&mut self, pane: usize, s: &str) {
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
    pub(super) fn path_move_h(&mut self, pane: usize, left: bool, word: bool, select: bool) {
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
    pub(super) fn handle_path_edit_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
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
                if let Some(text) = self.tab(pane).editing_path.as_ref() {
                    let path = expand_path(text.trim());
                    if self.tab(pane).remote.is_some() || path.is_dir() {
                        self.tab_mut(pane).editing_path = None;
                        self.navigate_in(pane, path, cx);
                    } else {
                        self.set_notice(
                            "This folder is unavailable. Check the path and file access.".into(),
                            cx,
                        );
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
                    Some((lo, hi)) => self
                        .tab(pane)
                        .editing_path
                        .as_ref()
                        .map(|s| s[char_byte(s, lo)..char_byte(s, hi)].to_string()),
                    None => self.tab(pane).editing_path.clone(),
                };
                if let Some(text) = text {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                }
            }
            "x" if cmd => {
                if let Some((lo, hi)) = self.path_sel(pane) {
                    let text = self.tab(pane).editing_path.as_ref().unwrap();
                    cx.write_to_clipboard(ClipboardItem::new_string(
                        text[char_byte(text, lo)..char_byte(text, hi)].to_string(),
                    ));
                    self.path_delete_sel(pane);
                    cx.notify();
                }
            }
            "delete" => {
                if !self.path_delete_sel(pane) {
                    let tab = self.tab_mut(pane);
                    if let Some(text) = tab.editing_path.as_mut() {
                        let lo = char_byte(text, tab.path_cursor);
                        let hi = char_byte(text, (tab.path_cursor + 1).min(text.chars().count()));
                        text.replace_range(lo..hi, "");
                    }
                }
                cx.notify();
            }
            "v" if cmd => {
                if let Some(t) = cx.read_from_clipboard().and_then(|i| i.text()) {
                    self.path_insert(pane, &t.replace(['\r', '\n'], ""));
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

    pub(super) fn path_pointer(
        &mut self,
        pane: usize,
        point: gpui::Point<gpui::Pixels>,
        extend: bool,
        clicks: usize,
    ) {
        let tab = self.tab_mut(pane);
        let Some(text) = tab.editing_path.as_ref() else {
            return;
        };
        let Some(layout) = tab.path_layout.borrow().as_ref().map(|l| {
            l.line
                .closest_index_for_x(point.x - l.bounds.left() + px(l.offset))
        }) else {
            return;
        };
        let at = text[..layout.min(text.len())].chars().count();
        if clicks >= 3 {
            tab.path_anchor = Some(0);
            tab.path_cursor = text.chars().count();
        } else if clicks == 2 {
            let byte = char_byte(text, at);
            let lo = text[..byte].rfind('/').map_or(0, |i| i + 1);
            let hi = text[byte..].find('/').map_or(text.len(), |i| byte + i);
            tab.path_anchor = Some(text[..lo].chars().count());
            tab.path_cursor = text[..hi].chars().count();
        } else {
            if !extend || tab.path_anchor.is_none() {
                tab.path_anchor = Some(if extend { tab.path_cursor } else { at });
            }
            tab.path_cursor = at;
        }
    }

    pub(super) fn path_drag(
        &mut self,
        position: gpui::Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) {
        let pane = self.active_pane;
        if self.tab(pane).path_dragging {
            self.path_pointer(pane, position, true, 1);
            cx.notify();
        }
    }

    pub(super) fn render_path_editor(&self, pane: usize, cx: &Context<Self>) -> impl IntoElement {
        let t = theme();
        let tab = self.tab(pane);
        let text = tab.editing_path.clone().unwrap_or_default();
        let cursor = char_byte(&text, tab.path_cursor.min(text.chars().count()));
        let selection = self
            .path_sel(pane)
            .map(|(lo, hi)| (char_byte(&text, lo), char_byte(&text, hi)));
        let stored = tab.path_layout.clone();
        div()
            .id(("path-editor", pane))
            .flex_1()
            .min_w_0()
            .h(px(24.))
            .px_2()
            .rounded_md()
            .bg(rgb(t.surface))
            .border_1()
            .border_color(rgb(t.accent))
            .cursor_text()
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, ev: &MouseDownEvent, window, cx| {
                    this.active_pane = pane;
                    window.focus(&this.focus);
                    this.path_pointer(pane, ev.position, ev.modifiers.shift, ev.click_count);
                    this.tab_mut(pane).path_dragging = true;
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .child(
                gpui::canvas(
                    move |bounds, window, _| {
                        let text: SharedString = text.clone().into();
                        let run = gpui::TextRun {
                            len: text.len(),
                            font: window.text_style().font(),
                            color: rgb(t.text).into(),
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        };
                        let line =
                            window
                                .text_system()
                                .shape_line(text, px(UI_FONT_SIZE), &[run], None);
                        let previous = stored.borrow().as_ref().map_or(0., |l| l.offset);
                        let offset = caret_offset(
                            previous,
                            f32::from(line.x_for_index(cursor)),
                            f32::from(line.width),
                            f32::from(bounds.size.width),
                        );
                        *stored.borrow_mut() = Some(Layout {
                            bounds,
                            line: line.clone(),
                            offset,
                        });
                        (line, offset)
                    },
                    move |bounds, (line, offset), window, cx| {
                        let origin = point(bounds.left() - px(offset), bounds.top() + px(3.));
                        window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
                            if let Some((lo, hi)) = selection {
                                window.paint_quad(gpui::fill(
                                    gpui::Bounds::new(
                                        point(origin.x + line.x_for_index(lo), origin.y),
                                        size(line.x_for_index(hi) - line.x_for_index(lo), px(17.)),
                                    ),
                                    Theme::alpha(t.accent, 0x55),
                                ));
                            } else {
                                window.paint_quad(gpui::fill(
                                    gpui::Bounds::new(
                                        point(origin.x + line.x_for_index(cursor), origin.y),
                                        size(px(1.5), px(17.)),
                                    ),
                                    rgb(t.text),
                                ));
                            }
                            let _ = line.paint(origin, px(17.), window, cx);
                        });
                    },
                )
                .size_full(),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn long_path_caret_stays_visible_when_replacing_or_selecting_parts() {
        assert_eq!(caret_offset(0., 900., 900., 300.), 603.);
        assert_eq!(caret_offset(603., 20., 900., 300.), 20.);
        assert_eq!(caret_offset(603., 25., 25., 300.), 0.);
    }
}
