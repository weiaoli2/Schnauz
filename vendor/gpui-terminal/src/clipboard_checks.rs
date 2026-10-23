//! Hidden mouse/clipboard integration fixture, without a PTY or shell input.
use super::*;

struct Fixture(Entity<TerminalView>);
impl Render for Fixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().p(px(20.)).child(self.0.clone())
    }
}

#[derive(Clone, Default)]
struct Input(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
impl std::io::Write for Input {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub fn run(cx: &mut App) {
    let input = Input::default();
    let window = cx
        .open_window(
            WindowOptions {
                show: false,
                focus: false,
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(-4000.), px(-4000.)),
                    size(px(640.), px(300.)),
                ))),
                ..Default::default()
            },
            |_, cx| {
                let view = cx.new(|cx| {
                    let mut view = TerminalView::new(
                        input.clone(),
                        std::io::empty(),
                        TerminalConfig {
                            copy_on_select: true,
                            ..Default::default()
                        },
                        cx,
                    );
                    view.state
                        .process_bytes("Schnauz Grüße\r\nsecond line".as_bytes());
                    let grid = view.state.term_arc();
                    view.with_resize_callback(move |cols, rows| {
                        use alacritty_terminal::grid::Dimensions;
                        let term = grid.try_lock().expect("Release grid lock before signalling the PTY");
                        assert_eq!((term.columns(), term.screen_lines()), (cols, rows),
                            "Child resize notification must see the updated grid dimensions");
                    })
                });
                cx.new(|_| Fixture(view))
            },
        )
        .unwrap();
    let paint = |cx: &mut App| {
        for _ in 0..3 {
            cx.update_window(*window, |_, w, cx| {
                w.refresh();
                w.draw(cx).clear();
            })
            .unwrap();
        }
    };
    paint(cx);
    let point = |col: f32, row: f32, cx: &mut App| {
        window
            .update(cx, |fixture, _, cx| {
                let view = fixture.0.read(cx);
                view.bounds.get().origin
                    + gpui::point(
                        view.renderer.cell_width * (col + 0.25),
                        view.renderer.cell_height * (row + 0.5),
                    )
            })
            .unwrap()
    };
    let gesture =
        |from: Point<Pixels>, to: Point<Pixels>, clicks: usize, shift: bool, cx: &mut App| {
            let modifiers = Modifiers {
                shift,
                ..Default::default()
            };
            window
                .update(cx, |fixture, w, cx| {
                    fixture.0.update(cx, |view, cx| {
                        view.on_mouse_down(
                            &MouseDownEvent {
                                position: from,
                                button: MouseButton::Left,
                                click_count: clicks,
                                modifiers,
                                ..Default::default()
                            },
                            w,
                            cx,
                        );
                        if from != to {
                            view.on_mouse_move(
                                &MouseMoveEvent {
                                    position: to,
                                    pressed_button: Some(MouseButton::Left),
                                    modifiers,
                                },
                                w,
                                cx,
                            );
                        }
                        view.on_mouse_up(
                            &MouseUpEvent {
                                position: to,
                                button: MouseButton::Left,
                                click_count: clicks,
                                modifiers,
                            },
                            w,
                            cx,
                        );
                    })
                })
                .unwrap();
            paint(cx);
        };
    let text = |cx: &App| {
        cx.read_from_clipboard()
            .and_then(|item| item.text())
            .unwrap_or_default()
    };
    let a = point(0., 0., cx);
    let b = point(7., 0., cx);
    cx.write_to_clipboard(ClipboardItem::new_string("sentinel".into()));
    gesture(a, a, 1, false, cx);
    assert_eq!(
        text(cx),
        "sentinel",
        "A plain click cannot replace the clipboard"
    );
    gesture(a, b, 1, false, cx);
    assert_eq!(
        text(cx),
        "Schnauz",
        "Mouse release copies selected characters"
    );
    gesture(b, a, 1, false, cx);
    assert_eq!(
        text(cx),
        "Schnauz",
        "Reverse drag preserves character boundaries"
    );
    let word = point(9., 0., cx);
    gesture(word, word, 2, false, cx);
    assert_eq!(text(cx), "Grüße", "Double-click copies a Unicode word");
    let c = point(6., 1., cx);
    gesture(a, c, 1, false, cx);
    assert_eq!(text(cx), "Schnauz Grüße\nsecond");
    let outside = window
        .update(cx, |fixture, _, cx| {
            fixture.0.read(cx).bounds.get().bottom_right() + gpui::point(px(5.), px(5.))
        })
        .unwrap();
    gesture(a, outside, 1, false, cx);
    assert!(text(cx).starts_with("Schnauz Grüße\nsecond line"));
    window
        .update(cx, |fixture, _, cx| {
            assert!(
                !fixture.0.read(cx).selecting,
                "Release outside retires selection tracking"
            )
        })
        .unwrap();
    window
        .update(cx, |fixture, _, cx| {
            fixture.0.update(cx, |view, _| {
                view.state.process_bytes(b"\x1b[?1000h\x1b[?1006h")
            })
        })
        .unwrap();
    cx.write_to_clipboard(ClipboardItem::new_string("mouse app sentinel".into()));
    gesture(a, b, 1, false, cx);
    assert_eq!(
        text(cx),
        "mouse app sentinel",
        "Application mouse reporting cannot copy stale selection"
    );
    gesture(a, b, 1, true, cx);
    assert_eq!(
        text(cx),
        "Schnauz",
        "Shift permits selection while an app owns mouse reporting"
    );
    window
        .update(cx, |fixture, w, cx| {
            fixture.0.update(cx, |view, cx| {
                view.resume_live(cx);
                view.state.process_bytes(b"\x1b[?1000l\x1b[?1006l");
                for i in 0..120 {
                    view.state
                        .process_bytes(format!("\r\nscrollback line {i}").as_bytes());
                }
                let g = view.scrollbar_geometry();
                assert!(g.history > 100 && g.thumb >= 22. && g.thumb < g.height);
                assert_eq!(g.offset, 0);
                assert!(
                    (g.top + g.thumb - g.height).abs() < 0.1,
                    "Latest output is at the bottom"
                );
                let origin = f32::from(view.bounds.get().origin.y);
                view.begin_scrollbar_drag(origin + 2., cx);
                let g = view.scrollbar_geometry();
                assert_eq!(g.offset, g.history);
                assert_eq!(g.top, 0.);
                view.drag_scrollbar(origin + 10000., cx);
                assert_eq!(
                    view.scrollbar_geometry().offset,
                    0,
                    "Dragging below the viewport reaches latest output"
                );
                view.scrollbar_drag = None;
                view.on_scroll(
                    &ScrollWheelEvent {
                        delta: ScrollDelta::Lines(gpui::point(0., 3.)),
                        position: a,
                        ..Default::default()
                    },
                    w,
                    cx,
                );
                let g = view.scrollbar_geometry();
                assert!(g.offset > 0 && g.top < g.height - g.thumb);
            })
        })
        .unwrap();
    // Observe the actual bytes delivered by wheel/typing, not just mode flags.
    window
        .update(cx, |f, w, cx| {
            f.0.update(cx, |v, cx| {
                input.0.lock().unwrap().clear();
                v.state
                    .process_bytes(b"\x1b[?1049h\x1b[?1000l\x1b[?1006l\x1b[?1007l");
                let position = v.bounds.get().origin
                    + gpui::point(
                        v.config.padding.left + px(1.),
                        v.config.padding.top + px(1.),
                    );
                let scroll = |v: &mut TerminalView,
                              delta: f32,
                              shift: bool,
                              w: &mut Window, cx: &mut Context<TerminalView>| {
                    v.on_scroll(
                        &ScrollWheelEvent {
                            position,
                            delta: ScrollDelta::Lines(gpui::point(0., delta)),
                            modifiers: Modifiers {
                                shift,
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                        w,
                        cx,
                    );
                };
                scroll(v, -2., false, w, cx);
                assert!(
                    input.0.lock().unwrap().is_empty(),
                    "Disabled alternate scrolling must not edit the prompt with arrow keys"
                );
                v.state.process_bytes(b"\x1b[?1007h");
                scroll(v, -2., false, w, cx);
                assert_eq!(&*input.0.lock().unwrap(), b"\x1b[B\x1b[B");
                input.0.lock().unwrap().clear();
                v.state.process_bytes(b"\x1b[?1007l\x1b[?1000h\x1b[?1006h");
                scroll(v, -3., false, w, cx);
                assert_eq!(
                    &*input.0.lock().unwrap(),
                    b"\x1b[<65;1;1M\x1b[<65;1;1M\x1b[<65;1;1M"
                );
                input.0.lock().unwrap().clear();
                v.state.process_bytes(b"\x1b[?1006l");
                scroll(v, 1., false, w, cx);
                assert_eq!(
                    &*input.0.lock().unwrap(),
                    b"\x1b[M`!!",
                    "Legacy mouse reporting cannot receive SGR text"
                );
                input.0.lock().unwrap().clear();
                scroll(v, 2., true, w, cx);
                assert!(
                    input.0.lock().unwrap().is_empty(),
                    "Shift-wheel scrolls locally without PTY input"
                );
                // A partial Shift-scroll cannot leak into the application's
                // wheel reports when Shift is released on the next event.
                scroll(v, 0.75, true, w, cx);
                scroll(v, 0.5, false, w, cx);
                assert!(input.0.lock().unwrap().is_empty());
                scroll(v, 0.5, false, w, cx);
                assert_eq!(&*input.0.lock().unwrap(), b"\x1b[M`!!");
                input.0.lock().unwrap().clear();
                v.toggle_text_selection(w, cx);
                scroll(v, -4., false, w, cx);
                assert!(input.0.lock().unwrap().is_empty(), "Frozen selection never sends wheel input");
                v.resume_live(cx);
                v.state.process_bytes(b"\x1b[?1000l\x1b[?1007h\x1b[?1h");
                for (key, expected) in [
                    ("ctrl-left", "\x1b[1;5D"),
                    ("shift-pageup", "\x1b[5;2~"),
                    ("alt-backspace", "\x1b\x7f"),
                ] {
                    v.on_key_down(&KeyDownEvent { keystroke: Keystroke::parse(key).unwrap(), is_held: false }, w, cx);
                    assert_eq!(&*input.0.lock().unwrap(), expected.as_bytes());
                    input.0.lock().unwrap().clear();
                }
                scroll(v, -9., false, w, cx);
                assert_eq!(&*input.0.lock().unwrap(), &b"\x1bOB".repeat(9), "Fast scrolling must not lose lines after the fifth");
                input.0.lock().unwrap().clear();
                v.state.process_bytes(b"\x1b[?1000l\x1b[?1049l");
                v.state.with_term_mut(|t| {
                    t.scroll_display(alacritty_terminal::grid::Scroll::Delta(3))
                });
                cx.notify();
            })
        })
        .unwrap();
    paint(cx);
    cx.update_window(*window, |_, w, cx| {
        w.dispatch_keystroke(Keystroke::parse("x").unwrap(), cx);
    })
    .unwrap();
    window
        .update(cx, |f, _, cx| {
            assert_eq!(&*input.0.lock().unwrap(), b"x");
            assert_eq!(
                f.0.read(cx).state.with_term(|t| t.grid().display_offset()),
                0,
                "Typing returns to the actual live bottom line"
            );
        })
        .unwrap();
    // Paint the bottom/right cells at several real viewport sizes. Verify the
    // actual glyph origins and masks passed to GPUI rather than outer bounds.
    for (width, height) in [(640., 300.), (633., 311.), (520., 237.)] {
        cx.update_window(*window, |_, w, _| w.resize(size(px(width), px(height))))
            .unwrap();
        paint(cx);
        window.update(cx,|f,_,cx|f.0.update(cx,|v,cx| {
            let rows=v.state.rows();let cols=v.state.cols();
            v.state.process_bytes(format!("\x1b[?25h\x1b[2J\x1b[{};1H界\x1b[{};5H\x1b[8mX\x1b[0m\x1b[{};{}H\x1b[1;3;4mg\x1b[0m",rows-1,rows-1,rows,cols).as_bytes());
            cx.notify();
        })).unwrap();
        paint(cx);
        window
            .update(cx, |f, _, cx| {
                let v = f.0.read(cx);
                let painted = v.renderer.paint_checks.borrow();
                let rows = v.state.rows();
                let cols = v.state.cols();
                let last = painted
                    .glyphs
                    .iter()
                    .find(|g| g.0 == rows - 1 && g.1 == cols - 1)
                    .expect("Bottom-right glyph is actually painted");
                assert_eq!(last.2, 'g');
                assert!(
                    (f32::from(
                        last.3.y
                            - painted.content.origin.y
                            - v.renderer.cell_height * (rows - 1) as f32
                    ))
                    .abs()
                        < 0.01,
                    "GPUI centers glyphs once, without extra row offset"
                );
                assert!(
                    last.4.right() <= painted.content.right()
                        && last.4.bottom() <= painted.content.bottom()
                );
                assert!(
                    painted.content.right()
                        <= v.bounds.get().right() - px(12.) - v.config.padding.right,
                    "Scrollbar gutter stays outside the text mask"
                );
                assert!(
                    !painted.glyphs.iter().any(|g| g.2 == 'X'),
                    "Concealed cells cannot paint stale text"
                );
                let wide = painted.glyphs.iter().find(|g| g.2 == '界').unwrap();
                assert!((f32::from(wide.4.size.width - v.renderer.cell_width * 2.)).abs() < 0.01);
                let cursor = painted.cursor.expect("Live bottom-row cursor is painted");
                assert!(
                    cursor.right() <= painted.content.right()
                        && cursor.bottom() <= painted.content.bottom()
                );
            })
            .unwrap();
    }
    println!(
        "PASS actual bottom-right glyph/cursor bounds and masks across terminal resizing, single baseline centering, wide/concealed cells, disabled alternate scrolling, negotiated mouse protocols, wheel counts, Shift scrolling and typing from scrollback; all input captured"
    );
    cx.update_window(*window, |_, w, _| w.remove_window())
        .unwrap();
    println!(
        "PASS real terminal mouse drag/reverse/Unicode word/multiline selection, automatic copy, outside release, Shift selection with application mouse reporting and scrollback scrollbar top/bottom/wheel positions; no PTY or shell input"
    );
}

/// Test-only controlled output and coordinates for actual native input QA.
pub fn output(view: &mut TerminalView, bytes: &[u8]) {
    view.state.process_bytes(bytes);
}
pub fn cell(view: &TerminalView, col: f32, row: f32) -> Point<Pixels> {
    view.bounds.get().origin
        + point(
            view.config.padding.left + view.renderer.cell_width * (col + 0.25),
            view.config.padding.top + view.renderer.cell_height * (row + 0.5),
        )
}
pub fn selected_text(view: &TerminalView) -> Option<String> {
    view.displayed_state()
        .with_term(|term| term.selection_to_string())
}

/// Inspect the actual emulator grid after output from an owned fixture PTY.
pub fn screen(view: &TerminalView) -> Vec<String> {
    use alacritty_terminal::grid::Dimensions;
    view.state.with_term(|term| (0..term.screen_lines()).map(|row| {
        (0..term.columns()).map(|col| term.grid()[alacritty_terminal::index::Point::new(
            alacritty_terminal::index::Line(row as i32), alacritty_terminal::index::Column(col))].c).collect()
    }).collect())
}
