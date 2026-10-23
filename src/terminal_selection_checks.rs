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
        }).unwrap();
        let (saved, close_pid) = cx.update(|cx| {
            let result = window.update(cx, |f, _, cx| f.browser.update(cx, |b, cx| {
                let pane = b.active_pane;
                let saved = b.tab_mut(pane).terminal.take().unwrap();
                assert!(b.start_fixture_terminal(pane, cx));
                let pid = b.tab(pane).terminal.as_ref().unwrap().fixture_pid();
                (saved, pid)
            })).unwrap();
            paint(window, cx);
            let hide = recorded_bounds("terminal-hide");
            let close = recorded_bounds("terminal-close");
            assert!(close.left() >= hide.right(), "Close must follow Hide");
            result
        }).unwrap();
        // Exercise a real full-screen editor through its owned PTY. The
        // shell's terminal size and vi's command row must agree with our grid.
        cx.update(|cx| window.update(cx, |f, _, cx| f.browser.update(cx, |b, _| {
            b.tab(b.active_pane).terminal.as_ref().unwrap().fixture_input(
                b"/usr/bin/vim -Nu NONE -n -i NONE -c 'call setline(1, range(1,160))'\r");
        })).unwrap()).unwrap();
        for _ in 0..200 {
            cx.background_executor().timer(Duration::from_millis(10)).await;
            let ready = cx.update(|cx| {
                paint(window, cx);
                window.update(cx, |f, _, cx| f.browser.update(cx, |b, cx| {
                    let v = b.tab(b.active_pane).terminal.as_ref().unwrap().view.read(cx);
                    let screen = terminal_fixture::screen(v);
                    screen.first().is_some_and(|line| line.trim() == "1") && screen.get(1).is_some_and(|line|line.trim() == "2")
                })).unwrap()
            }).unwrap();
            if ready { break; }
        }
        let original_size = cx.update(|cx| cx.update_window(*window, |_, w, _| w.viewport_size()).unwrap()).unwrap();
        let divider = cx.update(|_| recorded_bounds("terminal-divider").center()).unwrap();
        mouse(NSEventType::LeftMouseDown, divider, false);
        let travel = (f32::from(divider.y) - 70.).max(0.);
        for step in 1..=40 {
            mouse(NSEventType::LeftMouseDragged, divider - point(px(0.), px((step as f32 * 10.).min(travel))), false);
            cx.update(|cx| paint(window, cx)).unwrap();
            cx.background_executor().timer(Duration::from_millis(2)).await;
        }
        mouse(NSEventType::LeftMouseUp, divider - point(px(0.), px(travel)), false);
        cx.update(|cx| {
            window.update(cx, |f, _, cx| f.browser.update(cx, |b, cx| {
                assert!(b.terminal_resize.is_none());
                assert_eq!(b.tab(b.active_pane).terminal.as_ref().unwrap().fixture_pid(), close_pid);
                b.tab_mut(b.active_pane).terminal_height = 250.;
                cx.notify();
            })).unwrap();
            paint(window, cx);
        }).unwrap();
        println!("PASS native terminal divider upward drag through maximum height with live vi and preserved PTY");

        for (width, height) in [(1000., 700.), (1600., 920.), (1100., 760.)] {
            cx.update(|cx| {
                cx.update_window(*window, |_, w, _| w.resize(gpui::size(px(width), px(height)))).unwrap();
                paint(window, cx);
            }).unwrap();
            // AppKit delivers native resize events asynchronously. Let those
            // settle before asking vi to report the resulting PTY geometry.
            for _ in 0..10 {
                cx.background_executor().timer(Duration::from_millis(10)).await;
                cx.update(|cx| paint(window, cx)).unwrap();
            }
            let dimensions = cx.update(|cx| {
                window.update(cx, |f, _, cx| f.browser.update(cx, |b, cx| {
                    let session = b.tab(b.active_pane).terminal.as_ref().unwrap();
                    let dimensions = session.view.read(cx).dimensions();
                    session.fixture_input(b":echo 'SNZ-SIZE-' . &columns . 'x' . &lines\r");
                    dimensions
                })).unwrap()
            }).unwrap();
            let expected = format!("SNZ-SIZE-{}x{}", dimensions.0, dimensions.1);
            let mut matched = false;
            for _ in 0..200 {
                cx.background_executor().timer(Duration::from_millis(10)).await;
                matched = cx.update(|cx| {
                    paint(window, cx);
                    window.update(cx, |f, _, cx| f.browser.update(cx, |b, cx| {
                        let screen = terminal_fixture::screen(b.tab(b.active_pane).terminal.as_ref().unwrap().view.read(cx));
                        screen.last().is_some_and(|line|line.trim() == expected)
                    })).unwrap()
                }).unwrap();
                if matched { break; }
            }
            if !matched {
                cx.update(|cx| window.update(cx, |f, _, cx| f.browser.update(cx, |b, cx| {
                    eprintln!("vi screen expected {expected}: {:?}", terminal_fixture::screen(b.tab(b.active_pane).terminal.as_ref().unwrap().view.read(cx)));
                })).unwrap()).unwrap();
            }
            assert!(matched, "vi's actual last command row must report {expected}, matching the full viewport");
        }
        cx.update(|cx| window.update(cx, |f, _, cx| f.browser.update(cx, |b, _| {
            b.tab(b.active_pane).terminal.as_ref().unwrap().fixture_input(
                b":set nomore\r:echo join(range(1,60), \"\\n\")\r");
        })).unwrap()).unwrap();
        let mut scrolled = false;
        for _ in 0..200 {
            cx.background_executor().timer(Duration::from_millis(10)).await;
            scrolled = cx.update(|cx| {
                paint(window, cx);
                window.update(cx, |f, _, cx| f.browser.update(cx, |b, cx| {
                    let screen = terminal_fixture::screen(b.tab(b.active_pane).terminal.as_ref().unwrap().view.read(cx));
                    screen[screen.len()-2].trim() == "60" && screen.last().unwrap().contains("Press ENTER")
                })).unwrap()
            }).unwrap();
            if scrolled { break; }
        }
        assert!(scrolled, "Multiline vi command output must scroll the text upward and reserve the last row for the prompt");
        cx.update(|cx| {
            cx.update_window(*window, |_, w, _| w.resize(original_size)).unwrap();
            paint(window, cx);
        }).unwrap();
        for _ in 0..10 {
            cx.background_executor().timer(Duration::from_millis(10)).await;
            cx.update(|cx| paint(window, cx)).unwrap();
        }
        println!("PASS actual vi command row, multiline output scrolling and reported PTY rows/columns across narrow, wide and resized terminal viewports");
        let hide = cx.update(|_| recorded_bounds("terminal-hide").center()).unwrap();
        mouse(NSEventType::LeftMouseDown, hide, false);
        mouse(NSEventType::LeftMouseUp, hide, false);
        cx.update(|cx| {
            window.update(cx, |f, w, cx| f.browser.update(cx, |b, cx| {
                assert!(!b.tab(b.active_pane).terminal_open);
                assert_eq!(b.tab(b.active_pane).terminal.as_ref().unwrap().fixture_pid(), close_pid);
                assert_eq!(unsafe { libc::kill(close_pid as i32, 0) }, 0);
                b.toggle_terminal(w, cx);
            })).unwrap();
            paint(window, cx);
        }).unwrap();
        // Model following over an owned transport without connecting anywhere.
        // Closing must run ordinary SSH end cleanup before dropping the PTY.
        let local_folder = cx.update(|cx| window.update(cx, |f, _, cx| f.browser.update(cx, |b, _| {
            let tab = b.tab_mut(b.active_pane);
            let folder = tab.current_dir.clone();
            let server = SftpServer { name: "Close fixture".into(), host: "192.0.2.1".into(),
                control_path: Some(PathBuf::from("/tmp/schnauz-close-fixture-unused")), ..Default::default() };
            let link = ssh_sync::Link::remote_start("close-fixture".into(), server.clone(), folder.clone(), tab, true);
            tab.terminal.as_mut().unwrap().ssh = Some(link);
            tab.remote = Some(server);
            folder
        })).unwrap()).unwrap();
        let close = cx.update(|_| recorded_bounds("terminal-close").center()).unwrap();
        mouse(NSEventType::LeftMouseDown, close, false);
        mouse(NSEventType::LeftMouseUp, close, false);
        cx.update(|cx| {
            super::BOUNDS.with(|bounds| { bounds.borrow_mut().remove("active-tab-terminal"); });
            paint(window, cx);
            super::BOUNDS.with(|bounds| assert!(!bounds.borrow().contains_key("active-tab-terminal"), "Closed terminal badge must not render"));
            window.update(cx, |f, _, cx| f.browser.update(cx, |b, _| {
                let tab = b.tab(b.active_pane);
                assert!(!tab.terminal_open && tab.terminal.is_none() && tab.terminal_restore.is_none());
                assert!(tab.remote.is_none(), "SSH close must not leave a dead owned control socket in the browser");
                assert_eq!(tab.current_dir, local_folder);
            })).unwrap();
        }).unwrap();
        for _ in 0..100 {
            if unsafe { libc::kill(close_pid as i32, 0) } != 0 { break; }
            cx.background_executor().timer(Duration::from_millis(10)).await;
        }
        assert_ne!(unsafe { libc::kill(close_pid as i32, 0) }, 0, "Close must terminate its owned shell");
        cx.update(|cx| {
            window.update(cx, |f, _, cx| f.browser.update(cx, |b, cx| {
                let pane = b.active_pane;
                assert_eq!(saved.id, id);
                assert_eq!(saved.fixture_pid(), pid);
                assert_eq!(unsafe { libc::kill(pid as i32, 0) }, 0);
                b.tab_mut(pane).terminal = Some(saved);
                b.tab_mut(pane).terminal_open = true;
                cx.notify();
            })).unwrap();
            println!("PASS actual Hide preserves PTY, adjacent Close ends only its disposable shell and clears terminal state; original session/PID preserved");
            done(cx);
        }).unwrap();
    }).detach();
}
