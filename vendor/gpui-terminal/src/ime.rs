//! Native text composition stays local until macOS commits UTF-8 input.
use super::*;
use std::ops::Range;

#[derive(Default)]
pub(super) struct Composition {
    text: String,
    selected: Range<usize>, // UTF-16, as required by NSTextInputClient.
}

fn byte_offset(text: &str, utf16: usize) -> usize {
    let mut offset = 0;
    for (byte, ch) in text.char_indices() {
        if offset + ch.len_utf16() > utf16 {
            return byte;
        }
        offset += ch.len_utf16();
    }
    text.len()
}

impl TerminalView {
    pub fn is_composing(&self) -> bool {
        !self.composition.text.is_empty()
    }

    fn composition_origin(&self, bounds: Bounds<Pixels>) -> Point<Pixels> {
        let (column, row) = self.state.with_term(|term| {
            let cursor = term.grid().cursor.point;
            (cursor.column.0, cursor.line.0.max(0) as usize)
        });
        point(
            (bounds.left() + self.config.padding.left + self.renderer.cell_width * column as f32)
                .min(
                    (bounds.right()
                        - self.config.padding.right
                        - px(12.)
                        - self.renderer.cell_width)
                        .max(bounds.left()),
                ),
            (bounds.top() + self.config.padding.top + self.renderer.cell_height * row as f32).min(
                (bounds.bottom() - self.config.padding.bottom - self.renderer.cell_height)
                    .max(bounds.top()),
            ),
        )
    }

    fn composition_line(&self, window: &Window) -> ShapedLine {
        window.text_system().shape_line(
            self.composition.text.clone().into(),
            self.renderer.font_size,
            &[TextRun {
                len: self.composition.text.len(),
                font: font(self.renderer.font_family.clone()),
                color: rgb(0xdce1ec).into(),
                background_color: None,
                underline: None,
                strikethrough: None,
            }],
            None,
        )
    }

    pub(super) fn paint_composition(
        &self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        if !self.focus_handle.is_focused(window) || !self.is_composing() {
            return;
        }
        let origin = self.composition_origin(bounds);
        let line = self.composition_line(window);
        let area = Bounds::new(
            origin,
            size(
                line.width.max(self.renderer.cell_width),
                self.renderer.cell_height,
            ),
        );
        window.paint_quad(fill(area, rgb(0x151822)));
        let start = line.x_for_index(byte_offset(
            &self.composition.text,
            self.composition.selected.start,
        ));
        let end = line.x_for_index(byte_offset(
            &self.composition.text,
            self.composition.selected.end,
        ));
        if end > start {
            window.paint_quad(fill(
                Bounds::new(
                    origin + point(start, px(0.)),
                    size(end - start, area.size.height),
                ),
                rgba(0x3478c566),
            ));
        }
        let _ = line.paint(origin, self.renderer.cell_height, window, cx);
        window.paint_quad(fill(
            Bounds::new(
                point(origin.x, area.bottom() - px(1.)),
                size(area.size.width, px(1.)),
            ),
            rgb(0xa5b4fc),
        ));
    }
}

impl EntityInputHandler for TerminalView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let length = self.composition.text.encode_utf16().count();
        let start = range.start.min(length);
        let end = range.end.min(length).max(start);
        let start = byte_offset(&self.composition.text, start);
        let end = byte_offset(&self.composition.text, end);
        *adjusted = Some(
            self.composition.text[..start].encode_utf16().count()
                ..self.composition.text[..end].encode_utf16().count(),
        );
        Some(self.composition.text[start..end].into())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.composition.selected.clone(),
            reversed: false,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.is_composing()
            .then(|| 0..self.composition.text.encode_utf16().count())
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        // Cancelling/unmarking must never send unconfirmed pinyin to a shell.
        self.composition = Composition::default();
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use alacritty_terminal::grid::Scroll;
        self.composition = Composition::default();
        if !text.is_empty() {
            self.resume_live(cx);
            self.state.with_term_mut(|term| {
                term.scroll_display(Scroll::Bottom);
                term.selection = None;
            });
            // Only confirmed text is written. Previously sent terminal bytes
            // are never edited through the IME's replacement range.
            let text = text.replace("\r\n", "\r").replace('\n', "\r");
            if text.contains('\r') {
                if let Some(callback) = &self.shell_callback {
                    callback(&crate::shell::ShellEvent::CommandStarted, window, cx);
                }
            }
            self.write_input(text.as_bytes());
        }
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(range) = range {
            let start = byte_offset(&self.composition.text, range.start);
            let end = byte_offset(&self.composition.text, range.end).max(start);
            self.composition.text.replace_range(start..end, text);
        } else {
            self.composition.text = text.into();
        }
        let length = self.composition.text.encode_utf16().count();
        let selected = selected.unwrap_or(length..length);
        let start = selected.start.min(length);
        self.composition.selected = start..selected.end.min(length).max(start);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        // The terminal cursor also moves with program output. It is not a
        // native editable insertion point while no marked text exists. Avoid
        // anchoring macOS's input-source accessory on passive focus/hover;
        // candidate windows still get their real bounds during composition.
        if !self.is_composing() || !self.focus_handle.is_focused(window) {
            return None;
        }
        let origin = self.composition_origin(bounds);
        let line = self.composition_line(window);
        let start = line.x_for_index(byte_offset(&self.composition.text, range.start));
        let end = line.x_for_index(byte_offset(&self.composition.text, range.end));
        let x =
            (origin.x + start).min((bounds.right() - self.renderer.cell_width).max(bounds.left()));
        Some(Bounds::new(
            point(x, origin.y),
            size((end - start).max(px(1.)), self.renderer.cell_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}
