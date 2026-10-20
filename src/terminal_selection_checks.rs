//! Real mouse routing with a redraw-heavy terminal fixture and captured input.
use super::*;
use gpui_terminal::{view::clipboard_checks as terminal_fixture, TerminalConfig, TerminalView};
use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType, NSView};

#[derive(Clone)]
struct Input(Arc<Mutex<Vec<u8>>>);
impl std::io::Write for Input {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn paint(window: gpui::WindowHandle<Fixture>, cx: &mut App) {
    for _ in 0..3 {
        cx.update_window(*window, |_, w, cx| {
            w.refresh();
            w.draw(cx).clear();
        })
        .unwrap();
    }
}

pub(super) fn run(
    window: gpui::WindowHandle<Fixture>,
    cx: &mut App,
    done: impl FnOnce(&mut App) + 'static,
) {
    let clipboard = super::clipboard_checks::preserve();
    let input = Input(Arc::new(Mutex::new(Vec::new())));
    let (original, fixture, id, pid) = window.update(cx, |f, _, cx| f.browser.update(cx, |b, cx| {
        let fixture = cx.new(|cx| {
            let mut view = TerminalView::new(input.clone(), std::io::empty(), TerminalConfig {
                copy_on_select: true, padding: gpui::Edges::all(px(8.)), ..Default::default()
            }, cx);
            terminal_fixture::output(&mut view, b"\x1b[?1049h\x1b[?1000h\x1b[?1006h\x1b[?2004h\x1b[H\x1b[2JSelect this text");
            view
        });
        cx.subscribe(&fixture, |_, _, _: &gpui_terminal::view::TextSelectionChanged, cx| cx.notify()).detach();
        let tab = b.tab_mut(b.active_pane);
        let session = tab.terminal.as_mut().unwrap();
        let original = std::mem::replace(&mut session.view, fixture.clone());
        let id = session.id;
        let pid = session.fixture_pid();
        tab.terminal_open = true;
        cx.notify();
        (original, fixture, id, pid)
    })).unwrap();
    paint(window, cx);
    let (ptr, height) = cx
        .update_window(*window, |_, w, _| {
            (
                ns_view_ptr(w).unwrap() as usize,
                f32::from(w.viewport_size().height),
            )
        })
        .unwrap();
    cx.spawn(async move |cx| {
        cx.background_executor().timer(Duration::from_millis(10)).await;
        let view = unsafe { &*(ptr as *const NSView) };
        let number: isize = unsafe { objc2::msg_send![&*view.window().unwrap(), windowNumber] };
        let mouse = |kind, pos: gpui::Point<gpui::Pixels>, shift| {
            let flags = if shift { NSEventModifierFlags::Shift } else { NSEventModifierFlags::empty() };
            let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                kind, objc2_foundation::NSPoint::new(f32::from(pos.x) as f64, (height - f32::from(pos.y)) as f64), flags, 0., number, None, 0, 1, 0.
            ).unwrap();
            match kind {
                NSEventType::LeftMouseDown => view.mouseDown(&event),
                NSEventType::LeftMouseDragged => view.mouseDragged(&event),
                _ => view.mouseUp(&event),
            }
        };
        // Exercise the real Schnauz header button and an unmodified mouse drag.
        let button = cx.update(|_| recorded_bounds("terminal-select-text").center()).unwrap();
        mouse(NSEventType::LeftMouseDown, button, false);
        mouse(NSEventType::LeftMouseUp, button, false);
        cx.update(|cx| { paint(window,cx); assert!(fixture.read(cx).text_selection_active()); }).unwrap();
        for shift in [false, true] {
            if shift {
                cx.update(|cx| {
                    cx.update_window(*window, |_, w, cx| { w.dispatch_keystroke(gpui::Keystroke::parse("escape").unwrap(),cx); }).unwrap();
                    fixture.update(cx, |v,cx| { terminal_fixture::output(v,b"\x1b[H\x1b[2JSelect this text"); cx.notify(); });
                    paint(window,cx);
                }).unwrap();
            }
            let (a,b) = cx.update(|cx| (terminal_fixture::cell(fixture.read(cx),0.,0.),terminal_fixture::cell(fixture.read(cx),6.,0.))).unwrap();
            mouse(NSEventType::LeftMouseDown,a,shift);
            cx.update(|cx| {
                // A real TUI can clear and redraw between press and release.
                fixture.update(cx,|v,cx| { terminal_fixture::output(v,b"\x1b[H\x1b[2JNew live output"); cx.notify(); });
                paint(window,cx);
            }).unwrap();
            mouse(NSEventType::LeftMouseDragged,b,shift);
            mouse(NSEventType::LeftMouseUp,b,shift);
            cx.update(|cx| {
                paint(window,cx);
                assert_eq!(terminal_fixture::selected_text(fixture.read(cx)).as_deref(),Some("Select"));
                assert_eq!(cx.read_from_clipboard().and_then(|i|i.text()).as_deref(),Some("Select"));
                assert!(input.0.lock().unwrap().is_empty(),"Selection must not send mouse reports or other input to the program");
                cx.write_to_clipboard(ClipboardItem::new_string("copy sentinel".into()));
                cx.update_window(*window, |_,w,cx| { w.dispatch_keystroke(gpui::Keystroke::parse("cmd-c").unwrap(),cx); }).unwrap();
                assert_eq!(cx.read_from_clipboard().and_then(|i|i.text()).as_deref(),Some("Select"));
            }).unwrap();
        }
        cx.update(|cx| {
            cx.write_to_clipboard(ClipboardItem::new_string("paste Grüße".into()));
            cx.update_window(*window, |_,w,cx| { w.dispatch_keystroke(gpui::Keystroke::parse("cmd-v").unwrap(),cx); }).unwrap();
            assert!(!fixture.read(cx).text_selection_active());
            assert_eq!(&*input.0.lock().unwrap(),"\x1b[200~paste Grüße\x1b[201~".as_bytes());
            window.update(cx,|f,_,cx|f.browser.update(cx,|b,cx| {
                let session=b.tab_mut(b.active_pane).terminal.as_mut().unwrap();
                assert_eq!(session.id,id); assert_eq!(session.fixture_pid(),pid);
                session.view=original;
                cx.notify();
            })).unwrap();
            drop(clipboard);
            println!("PASS native terminal Select text button, plain/Shift mouse drag across live redraws, automatic copy, Cmd+C, bracketed Cmd+V and unchanged owned session/PID; captured input only, no live Codex/session");
            done(cx);
        }).unwrap();
    }).detach();
}
