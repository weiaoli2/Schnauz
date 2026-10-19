//! Hidden mouse/clipboard integration fixture, without a PTY or shell input.
use super::*;

struct Fixture(Entity<TerminalView>);
impl Render for Fixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().p(px(20.)).child(self.0.clone())
    }
}

pub fn run(cx: &mut App) {
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
                        std::io::sink(),
                        std::io::empty(),
                        TerminalConfig {
                            copy_on_select: true,
                            ..Default::default()
                        },
                        cx,
                    );
                    view.state
                        .process_bytes("Schnauz Grüße\r\nsecond line".as_bytes());
                    view
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
    cx.update_window(*window, |_, w, _| w.remove_window())
        .unwrap();
    println!(
        "PASS real terminal mouse drag/reverse/Unicode word/multiline selection, automatic copy, outside release, Shift selection with application mouse reporting and scrollback scrollbar top/bottom/wheel positions; no PTY or shell input"
    );
}
