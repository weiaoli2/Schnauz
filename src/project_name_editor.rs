//! Single-line project-name editing, independent of project persistence.
use super::*;

#[derive(Default)]
pub(super) struct Editor {
    cursor: usize,
    anchor: Option<usize>,
    pub(super) dragging: bool,
    layout: Rc<RefCell<Option<Layout>>>,
}

struct Layout {
    bounds: Bounds<gpui::Pixels>,
    line: gpui::ShapedLine,
    offset: f32,
}

impl Editor {
    pub(super) fn reset(&mut self, text: &str, selected: bool) {
        *self = Self::default();
        self.cursor = text.chars().count();
        self.anchor = selected.then_some(0);
    }

    pub(super) fn selection(&self, text: &str) -> Option<(usize, usize)> {
        let end = text.chars().count();
        let cursor = self.cursor.min(end);
        self.anchor
            .map(|anchor| anchor.min(end))
            .filter(|anchor| *anchor != cursor)
            .map(|anchor| (anchor.min(cursor), anchor.max(cursor)))
    }

    fn delete_selection(&mut self, text: &mut String) -> bool {
        let selection = self.selection(text);
        self.anchor = None;
        if let Some((lo, hi)) = selection {
            text.replace_range(char_byte(text, lo)..char_byte(text, hi), "");
            self.cursor = lo;
            true
        } else {
            false
        }
    }

    fn insert(&mut self, text: &mut String, value: &str) {
        self.delete_selection(text);
        self.cursor = self.cursor.min(text.chars().count());
        let value: String = value.chars().filter(|ch| !ch.is_control()).collect();
        text.insert_str(char_byte(text, self.cursor), &value);
        self.cursor += value.chars().count();
    }

    pub(super) fn key(&mut self, text: &mut String, key: &gpui::Keystroke, cx: &mut App) {
        self.dragging = false;
        let end = text.chars().count();
        self.cursor = self.cursor.min(end);
        let cmd = key.modifiers.platform;
        let shift = key.modifiers.shift;
        match key.key.as_str() {
            "a" if cmd => {
                self.anchor = Some(0);
                self.cursor = end;
            }
            "c" | "x" if cmd => {
                if let Some((lo, hi)) = self.selection(text) {
                    cx.write_to_clipboard(ClipboardItem::new_string(
                        text[char_byte(text, lo)..char_byte(text, hi)].into(),
                    ));
                    if key.key == "x" {
                        self.delete_selection(text);
                    }
                }
            }
            "v" if cmd => {
                if let Some(value) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    self.insert(text, &value);
                }
            }
            "left" | "right" | "home" | "end" => {
                let left = matches!(key.key.as_str(), "left" | "home");
                let target = if cmd || matches!(key.key.as_str(), "home" | "end") {
                    if left {
                        0
                    } else {
                        end
                    }
                } else if !shift && !key.modifiers.alt && self.selection(text).is_some() {
                    let (lo, hi) = self.selection(text).unwrap();
                    if left {
                        lo
                    } else {
                        hi
                    }
                } else if key.modifiers.alt {
                    if left {
                        prev_word_boundary(text, self.cursor)
                    } else {
                        next_word_boundary(text, self.cursor)
                    }
                } else if left {
                    self.cursor.saturating_sub(1)
                } else {
                    (self.cursor + 1).min(end)
                };
                if shift {
                    if self.anchor.is_none() {
                        self.anchor = Some(self.cursor);
                    }
                } else {
                    self.anchor = None;
                }
                self.cursor = target;
            }
            "backspace" | "delete" => {
                if !self.delete_selection(text) {
                    let left = key.key == "backspace";
                    let target = if cmd {
                        if left {
                            0
                        } else {
                            end
                        }
                    } else if key.modifiers.alt {
                        if left {
                            prev_word_boundary(text, self.cursor)
                        } else {
                            next_word_boundary(text, self.cursor)
                        }
                    } else if left {
                        self.cursor.saturating_sub(1)
                    } else {
                        (self.cursor + 1).min(end)
                    };
                    let lo = target.min(self.cursor);
                    let hi = target.max(self.cursor);
                    text.replace_range(char_byte(text, lo)..char_byte(text, hi), "");
                    self.cursor = lo;
                }
            }
            _ if !cmd && !key.modifiers.control => {
                if let Some(value) = key.key_char.as_deref() {
                    self.insert(text, value);
                }
            }
            _ => {}
        }
    }

    fn pointer(
        &mut self,
        text: &str,
        position: gpui::Point<gpui::Pixels>,
        extend: bool,
        clicks: usize,
    ) {
        let Some(byte) = self.layout.borrow().as_ref().map(|layout| {
            layout
                .line
                .closest_index_for_x(position.x - layout.bounds.left() + px(layout.offset))
        }) else {
            return;
        };
        let mut byte = byte.min(text.len());
        while !text.is_char_boundary(byte) {
            byte -= 1;
        }
        let at = text[..byte].chars().count();
        let chars: Vec<char> = text.chars().collect();
        if clicks >= 3 {
            self.anchor = Some(0);
            self.cursor = chars.len();
        } else if clicks == 2 && !chars.is_empty() {
            let at = at.min(chars.len() - 1);
            let word = |ch: char| ch.is_alphanumeric() || ch == '_';
            let class = word(chars[at]);
            let mut lo = at;
            let mut hi = at + 1;
            while lo > 0 && word(chars[lo - 1]) == class {
                lo -= 1;
            }
            while hi < chars.len() && word(chars[hi]) == class {
                hi += 1;
            }
            self.anchor = Some(lo);
            self.cursor = hi;
        } else {
            if !extend {
                self.anchor = Some(at);
            } else if self.anchor.is_none() {
                self.anchor = Some(self.cursor);
            }
            self.cursor = at;
        }
    }
}

impl Schnauz {
    pub(super) fn drag_project_name(
        &mut self,
        event: &MouseMoveEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.project_name_editor.dragging {
            return false;
        }
        if self.project_dialog.is_none() || event.pressed_button != Some(MouseButton::Left) {
            self.project_name_editor.dragging = false;
        } else if let Some(text) = self.project_dialog.as_deref() {
            self.project_name_editor
                .pointer(text, event.position, true, 1);
            cx.notify();
        }
        true
    }

    pub(super) fn render_project_name_editor(&self, cx: &Context<Self>) -> AnyElement {
        let text = self.project_dialog.clone().unwrap_or_default();
        let t = theme();
        let cursor = char_byte(
            &text,
            self.project_name_editor.cursor.min(text.chars().count()),
        );
        let selection = self
            .project_name_editor
            .selection(&text)
            .map(|(lo, hi)| (char_byte(&text, lo), char_byte(&text, hi)));
        let stored = self.project_name_editor.layout.clone();
        div()
            .id("project-name-editor")
            .h(px(42.))
            .px_3()
            .min_w_0()
            .rounded_lg()
            .bg(rgb(t.bg))
            .border_1()
            .border_color(rgb(t.accent))
            .cursor_text()
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    window.focus(&this.focus);
                    if let Some(text) = this.project_dialog.as_deref() {
                        this.project_name_editor.pointer(
                            text,
                            event.position,
                            event.modifiers.shift,
                            event.click_count,
                        );
                        this.project_name_editor.dragging = true;
                    }
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                gpui::canvas(
                    move |bounds, window, _| {
                        let run = gpui::TextRun {
                            len: text.len(),
                            font: window.text_style().font(),
                            color: rgb(t.text).into(),
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        };
                        let line = window.text_system().shape_line(
                            text.clone().into(),
                            px(UI_FONT_SIZE),
                            &[run],
                            None,
                        );
                        let previous = stored.borrow().as_ref().map_or(0., |layout| layout.offset);
                        let offset = path_editor::caret_offset(
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
                                    Theme::alpha(t.accent, 0x66),
                                ));
                            }
                            if line.text.is_empty() {
                                let value: SharedString = "Project name".into();
                                let len = value.len();
                                let placeholder = window.text_system().shape_line(
                                    value,
                                    px(UI_FONT_SIZE),
                                    &[gpui::TextRun {
                                        len,
                                        font: window.text_style().font(),
                                        color: rgb(t.text_dim).into(),
                                        background_color: None,
                                        underline: None,
                                        strikethrough: None,
                                    }],
                                    None,
                                );
                                let _ = placeholder.paint(origin, px(17.), window, cx);
                            } else {
                                let _ = line.paint(origin, px(17.), window, cx);
                            }
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
}
