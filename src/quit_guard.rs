//! Confirm quitting only for actual processes in owned local/project PTYs.
use super::*;
use gpui::WeakEntity;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ClassBuilder, Sel};
use objc2_app_kit::{NSAlert, NSAlertStyle, NSApplication, NSApplicationTerminateReply};

#[derive(Default)]
struct State {
    browsers: Vec<WeakEntity<Schnauz>>,
    pending: bool,
}
impl gpui::Global for State {}
thread_local! {
    static ALLOW: std::cell::Cell<bool> = const {std::cell::Cell::new(false)};
    static REQUEST: RefCell<Option<Box<dyn FnMut()>>> = const {RefCell::new(None)};
}

pub(super) fn register(cx: &mut Context<Schnauz>) {
    if cx.try_global::<State>().is_none() {
        cx.set_global(State::default());
    }
    let weak = cx.weak_entity();
    cx.update_global::<State, _>(|s, _| {
        s.browsers.retain(|b| b.upgrade().is_some());
        s.browsers.push(weak);
    });
}
pub(super) fn browsers(cx:&App)->Vec<gpui::Entity<Schnauz>> {
    cx.try_global::<State>().map(|s|s.browsers.iter().filter_map(|b|b.upgrade()).collect()).unwrap_or_default()
}

fn finish_setup(choice:Option<bool>,cx:&mut App) {
    cx.update_global::<State,_>(|s,_|s.pending=false);
    let Some(keep)=choice else {return;};
    match startup_setup::save(keep,cx) {
        Ok(())=>quit_now(cx),
        Err(error)=>for b in browsers(cx) { b.update(cx,|b,cx|b.set_notice(error.clone(),cx)); },
    }
}
fn choose_setup(cx:&mut App) {
    cx.update_global::<State,_>(|s,_|s.pending=true);
    #[cfg(feature="ui-checks")]
    if let Some(reply)=TEST_SETUP_REPLY.with(|r|r.get()) {
        finish_setup(match reply {1=>Some(true),0=>Some(false),_=>None},cx);
        return;
    }
    cx.spawn(async move|cx| {
        let app=NSApplication::sharedApplication(objc2::MainThreadMarker::new().unwrap());
        let alert=NSAlert::new(objc2::MainThreadMarker::new().unwrap());
        alert.setMessageText(&NSString::from_str("Keep this setup for next start?"));
        alert.setInformativeText(&NSString::from_str("Keep your open projects, tabs, window layout and terminal panels. Terminals will start fresh; commands and running programs are not resumed. Starting fresh keeps your saved projects and favourites."));
        alert.addButtonWithTitle(&NSString::from_str("Keep Setup & Quit"));
        alert.addButtonWithTitle(&NSString::from_str("Start Fresh Next Time"));
        let cancel=alert.addButtonWithTitle(&NSString::from_str("Cancel"));
        cancel.setKeyEquivalent(&NSString::from_str("\u{1b}"));
        alert.layout(); alert.window().center();
        #[cfg(feature="ui-checks")]
        { alert.window().setFrameOrigin(objc2_foundation::NSPoint::new(-20000.,-20000.)); TEST_SETUP_ALERT.with(|s|*s.borrow_mut()=Some(alert.clone())); }
        let session=app.beginModalSessionForWindow(&alert.window());
        let response=loop {
            let response=unsafe{app.runModalSession(session)};
            if response!=objc2_app_kit::NSModalResponseContinue {break response;}
            cx.background_executor().timer(Duration::from_millis(16)).await;
        };
        unsafe{app.endModalSession(session);}
        alert.window().orderOut(None);
        let choice=if response==objc2_app_kit::NSAlertFirstButtonReturn {Some(true)}
            else if response==objc2_app_kit::NSAlertSecondButtonReturn {Some(false)} else {None};
        let _=cx.update(|cx| {
            finish_setup(choice,cx);
            for window in cx.windows() {let _=cx.update_window(window,|_,w,_|w.refresh());}
        });
    }).detach();
}

/// Per-instance delegate subclass inherits GPUI's normal startup/menu/shutdown.
/// AppKit reaches this for Dock Quit as well as application menu requests.
unsafe extern "C-unwind" fn should_terminate(
    _: &AnyObject,
    _: Sel,
    _: &AnyObject,
) -> NSApplicationTerminateReply {
    if ALLOW.with(|allow| allow.replace(false)) {
        return NSApplicationTerminateReply::TerminateNow;
    }
    REQUEST.with(|request| {
        if let Some(request) = request.borrow_mut().as_mut() {
            request();
        }
    });
    NSApplicationTerminateReply::TerminateCancel
}
pub(super) fn install(cx: &mut App) {
    let app_cx = cx.to_async();
    REQUEST.with(|request| {
        *request.borrow_mut() = Some(Box::new(move || {
            let _ = app_cx.update(request_quit);
        }))
    });
    let Some(mtm) = objc2::MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    let Some(delegate) = app.delegate() else {
        return;
    };
    let delegate: &AnyObject = (&*delegate).as_ref();
    if delegate
        .class()
        .name()
        .to_string_lossy()
        .starts_with("SchnauzQuitGuard_")
    {
        return;
    }
    let name = std::ffi::CString::new(format!(
        "SchnauzQuitGuard_{}",
        delegate.class().name().to_string_lossy()
    ))
    .unwrap();
    let mut builder =
        ClassBuilder::new(&name, delegate.class()).expect("unique application delegate guard");
    unsafe {
        builder.add_method(
            objc2::sel!(applicationShouldTerminate:),
            should_terminate as unsafe extern "C-unwind" fn(_, _, _) -> _,
        );
    }
    let class = builder.register();
    unsafe {
        AnyObject::set_class(delegate, class);
    }
}
#[derive(Clone)]
struct Terminal {
    project: String,
    probe: schnauz_terminal::QuitProbe,
}
fn snapshots(cx: &App) -> Vec<Terminal> {
    let mut terminals = Vec::new();
    let Some(state) = cx.try_global::<State>() else {
        return terminals;
    };
    for browser in state.browsers.iter().filter_map(|b| b.upgrade()) {
        let b = browser.read(cx);
        let title = |id: Option<u64>| {
            id.and_then(|id| {
                b.projects
                    .items
                    .iter()
                    .find(|p| p.id == id)
                    .map(|p| p.name.clone())
            })
            .unwrap_or_else(|| {
                if id.is_none() {
                    "Personal".into()
                } else {
                    "Removed project".into()
                }
            })
        };
        let mut collect = |id, panes: &[Pane]| {
            for tab in panes.iter().flat_map(|p| &p.tabs) {
                if let Some(probe) = tab.terminal.as_ref().and_then(|s| s.quit_probe()) {
                    terminals.push(Terminal {
                        project: title(id),
                        probe,
                    });
                }
            }
        };
        collect(b.projects.active, &b.panes);
        for (id, session) in &b.project_sessions {
            collect(*id, &session.panes);
        }
    }
    terminals
}
fn scan(terminals: Vec<Terminal>) -> BTreeMap<String, Vec<String>> {
    let mut projects = BTreeMap::<String, Vec<String>>::new();
    for terminal in terminals {
        let names = terminal.probe.running_names();
        if !names.is_empty() {
            projects
                .entry(terminal.project)
                .or_default()
                .push(names.join(", "));
        }
    }
    projects
}
fn detail(projects: &BTreeMap<String, Vec<String>>) -> String {
    let mut text = String::from("These projects have active terminal processes:\n\n");
    for (name, terminals) in projects {
        text.push_str(&format!(
            "{name} — {} terminal{}\n",
            terminals.len(),
            if terminals.len() == 1 { "" } else { "s" }
        ));
        for names in terminals.iter().take(6) {
            text.push_str(&format!("  {names}\n"));
        }
        if terminals.len() > 6 {
            text.push_str("  …\n");
        }
    }
    text.push_str("\nQuitting closes these terminal sessions and may stop their work.");
    text
}
fn quit_now(cx: &mut App) {
    #[cfg(feature = "ui-checks")]
    if TEST_QUITS.with(|q| {
        if let Some(n) = q.get() {
            q.set(Some(n + 1));
            true
        } else {
            false
        }
    }) {
        return;
    }
    ALLOW.with(|allow| allow.set(true));
    cx.quit();
}
pub(super) fn request(cx: &mut App) {
    request_quit(cx);
}
/// Use a standalone native dialog: attaching sheets to the dynamically adapted
/// browser NSWindow interferes with AppKit's internal observation bookkeeping.
fn warning(text: &str) -> Retained<NSAlert> {
    let alert = NSAlert::new(objc2::MainThreadMarker::new().unwrap());
    alert.setAlertStyle(NSAlertStyle::Warning);
    alert.setMessageText(&NSString::from_str(
        "Quit Schnauz with running terminal processes?",
    ));
    alert.setInformativeText(&NSString::from_str(text));
    let cancel = alert.addButtonWithTitle(&NSString::from_str("Cancel"));
    cancel.setKeyEquivalent(&NSString::from_str("\u{1b}"));
    let quit = alert.addButtonWithTitle(&NSString::from_str("Quit Anyway"));
    quit.setKeyEquivalent(&NSString::from_str(""));
    alert.layout();
    alert.window().center();
    #[cfg(feature = "ui-checks")]
    alert.window().setFrameOrigin(objc2_foundation::NSPoint::new(-20000., -20000.));
    alert
}

fn request_quit(cx: &mut App) {
    if cx.try_global::<State>().is_some_and(|s| s.pending) {
        return;
    }
    if cx.try_global::<State>().is_none() {
        cx.set_global(State::default());
    }
    cx.update_global::<State, _>(|s, _| s.pending = true);
    let terminals = snapshots(cx);
    cx.spawn(async move |cx| {
        let projects = cx.background_spawn(async move { scan(terminals) }).await;
        let question = cx
            .update(|cx| {
                if projects.is_empty() {
                    cx.update_global::<State, _>(|s, _| s.pending = false);
                    choose_setup(cx);
                    return None;
                }
                let text = detail(&projects);
                #[cfg(feature = "ui-checks")]
                if let Some(reply) = TEST_REPLY.with(|r| r.get()) {
                    TEST_WARNINGS.with(|w| w.borrow_mut().push(text));
                    cx.update_global::<State, _>(|s, _| s.pending = false);
                    if reply {
                        choose_setup(cx);
                    }
                    return None;
                }
                Some(text)
            })
            .ok()
            .flatten();
        if let Some(text) = question {
            // Run outside an App/Window borrow: AppKit can dispatch focus events.
            let app = NSApplication::sharedApplication(objc2::MainThreadMarker::new().unwrap());
            let alert = warning(&text);
            let session = app.beginModalSessionForWindow(&alert.window());
            #[cfg(feature = "ui-checks")]
            TEST_ALERT.with(|slot| *slot.borrow_mut() = Some(alert.clone()));
            let response = loop {
                let response = unsafe { app.runModalSession(session) };
                if response != objc2_app_kit::NSModalResponseContinue {
                    break response;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
            };
            unsafe {
                app.endModalSession(session);
            }
            alert.window().orderOut(None);
            let reply = response == objc2_app_kit::NSAlertSecondButtonReturn;
            let _ = cx.update(|cx| {
                cx.update_global::<State, _>(|s, _| s.pending = false);
                alert.window().orderOut(None);
                for window in cx.windows() {
                    let _ = cx.update_window(window, |_, window, _| window.refresh());
                }
                if reply {
                    choose_setup(cx);
                }
            });
        }
    })
    .detach();
}

#[cfg(feature = "ui-checks")]
thread_local! {
    static TEST_QUITS:std::cell::Cell<Option<usize>>=const{std::cell::Cell::new(None)};
    static TEST_REPLY:std::cell::Cell<Option<bool>>=const{std::cell::Cell::new(None)};
    static TEST_WARNINGS:RefCell<Vec<String>>=const{RefCell::new(Vec::new())};
    static TEST_SETUP_REPLY:std::cell::Cell<Option<i8>>=const{std::cell::Cell::new(None)};
    static TEST_SETUP_ALERT:RefCell<Option<Retained<NSAlert>>>=const{RefCell::new(None)};
    static TEST_ALERT: RefCell<Option<Retained<NSAlert>>> = const { RefCell::new(None) };
}

#[cfg(feature = "ui-checks")]
pub(super) fn check(
    window: gpui::WindowHandle<ui_checks::Fixture>,
    cx: &mut App,
    done: impl FnOnce(&mut App) + 'static,
) {
    TEST_QUITS.with(|q| q.set(Some(0)));
    TEST_SETUP_REPLY.with(|r| r.set(Some(1)));
    TEST_REPLY.with(|r| r.set(Some(false)));
    TEST_WARNINGS.with(|w| w.borrow_mut().clear());
    install(cx);
    let (original, pid, terminal_id) = window
        .update(cx, |f, _, cx| {
            let b = f.browser.read(cx);
            let terminal = b.active_tab().terminal.as_ref().unwrap();
            (b.projects.clone(), terminal.fixture_pid(), terminal.id)
        })
        .unwrap();
    let delegate = {
        let app = NSApplication::sharedApplication(objc2::MainThreadMarker::new().unwrap());
        let delegate = app.delegate().unwrap();
        ({
            let obj: &AnyObject = (&*delegate).as_ref();
            obj as *const AnyObject
        }) as usize
    };
    request(cx);
    cx.spawn(async move|cx| {
        // Idle shells do not prompt or block. All quit requests are intercepted.
        for _ in 0..80 {
            cx.background_executor().timer(Duration::from_millis(20)).await;
            if cx.update(|cx|!cx.global::<State>().pending).unwrap(){break;}
        }
        cx.update(|cx| {
            assert!(!cx.global::<State>().pending);
            TEST_QUITS.with(|q|assert_eq!(q.get(),Some(1)));
            TEST_WARNINGS.with(|w|assert!(w.borrow().is_empty(),"Idle terminal must not prompt"));
            window.update(cx,|f,_,cx| {f.browser.read(cx).active_tab().terminal.as_ref().unwrap().fixture_input(b"sleep 30\r");}).unwrap();
        }).unwrap();
        cx.background_executor().timer(Duration::from_millis(150)).await;
        // Exercise AppKit's actual delegate route, including Dock Quit.
        let native=unsafe{&*(delegate as *const AnyObject)};
        let app=NSApplication::sharedApplication(objc2::MainThreadMarker::new().unwrap());
        let reply:NSApplicationTerminateReply=unsafe{objc2::msg_send![native,applicationShouldTerminate:&*app]};
        assert_eq!(reply,NSApplicationTerminateReply::TerminateCancel);
        for _ in 0..80 {
            cx.background_executor().timer(Duration::from_millis(20)).await;
            if cx.update(|cx|!cx.global::<State>().pending).unwrap(){break;}
        }
        // Use the real AppKit dialog and its actual Cancel button. Mocked
        // decisions alone cannot detect a warning left visible after cancellation.
        for _ in 0..2 {
            cx.update(|cx| {
                TEST_REPLY.with(|r| r.set(None));
                request(cx);
            }).unwrap();
            for _ in 0..80 {
                cx.background_executor().timer(Duration::from_millis(20)).await;
                if TEST_ALERT.with(|slot| slot.borrow().is_some()) { break; }
            }
            let dialog = cx.update(|cx| {
                let alert = TEST_ALERT.with(|slot| slot.borrow_mut().take()).expect("Real quit alert presented");
                let dialog = alert.window();
                assert!(app.modalWindow().is_some(), "Warning owns a native modal session");
                assert!(cx.global::<State>().pending);
                request(cx);
                assert!(TEST_ALERT.with(|slot| slot.borrow().is_none()), "Repeated quit does not duplicate the sheet");
                unsafe { alert.buttons().objectAtIndex(0).performClick(None); }
                dialog
            }).unwrap();
            for _ in 0..80 {
                cx.background_executor().timer(Duration::from_millis(20)).await;
                if cx.update(|cx| !cx.global::<State>().pending).unwrap() && app.modalWindow().is_none() { break; }
            }
            cx.update(|cx| {
                assert!(!cx.global::<State>().pending);
                assert!(app.modalWindow().is_none(), "Cancel releases application modality");
                assert!(!dialog.isVisible(), "Cancel removes the warning window");
                TEST_QUITS.with(|q| assert_eq!(q.get(), Some(1), "Cancel keeps the app open"));
                assert_eq!(unsafe { libc::kill(pid as i32, 0) }, 0);
                window.update(cx, |f, w, cx| f.browser.update(cx, |b, cx| {
                    let terminal = b.active_tab().terminal.as_ref().unwrap();
                    assert_eq!(terminal.fixture_pid(), pid);
                    assert_eq!(terminal.id, terminal_id);
                    assert!(terminal.quit_probe().unwrap().running_names().iter().any(|name| name.contains("sleep")));
                    w.focus(&b.focus);
                    w.refresh();
                    cx.notify();
                })).unwrap();
            }).unwrap();
        }
        TEST_REPLY.with(|r| r.set(Some(false)));
        let parked=cx.update(|cx| {
            TEST_QUITS.with(|q|assert_eq!(q.get(),Some(1),"Cancel never authorizes exit"));
            TEST_WARNINGS.with(|w| {let w=w.borrow();assert_eq!(w.len(),1);assert!(w[0].contains("sleep"));});
            window.update(cx,|f,_,cx|f.browser.update(cx,|b,cx| {
                let name=b.projects.active.and_then(|id|b.projects.items.iter().find(|p|p.id==id).map(|p|p.name.clone())).unwrap_or("Personal".into());
                TEST_WARNINGS.with(|w|assert!(w.borrow()[0].contains(&name)));
                let mut candidate=b.projects.clone();
                let project=candidate.create("Quit Guard Parked Project",b.capture_layout()).unwrap();
                b.persist_library(candidate,cx).unwrap();
                let active=b.active_pane;let split=b.split_ratio;
                let panes=std::mem::replace(&mut b.panes,vec![Pane::new(config_dir().unwrap())]);
                b.project_sessions.insert(Some(project),schnauz::ProjectSession{panes,active_pane:active,split_ratio:split});
                b.active_pane=0;b.split_ratio=0.5;
                (project,active,split)
            })).unwrap()
        }).unwrap();
        cx.update(request).unwrap();
        for _ in 0..80 {
            cx.background_executor().timer(Duration::from_millis(20)).await;
            if cx.update(|cx|!cx.global::<State>().pending).unwrap(){break;}
        }
        cx.update(|cx| {
            TEST_WARNINGS.with(|w|assert!(w.borrow().last().unwrap().contains("Quit Guard Parked Project")));
            TEST_QUITS.with(|q|assert_eq!(q.get(),Some(1)));
            TEST_REPLY.with(|r|r.set(Some(true)));
            request(cx);
        }).unwrap();
        for _ in 0..80 {
            cx.background_executor().timer(Duration::from_millis(20)).await;
            if cx.update(|cx|!cx.global::<State>().pending).unwrap(){break;}
        }
        cx.update(|cx| {
            TEST_QUITS.with(|q|assert_eq!(q.get(),Some(2),"Quit Anyway authorizes the quit path"));
            assert_eq!(unsafe{libc::kill(pid as i32,0)},0,"No fixture or user process was quit by QA");
            window.update(cx,|f,_,cx|f.browser.update(cx,|b,cx| {
                let session=b.project_sessions.remove(&Some(parked.0)).unwrap();
                b.panes=session.panes;b.active_pane=parked.1;b.split_ratio=parked.2;
                let terminal=b.active_tab().terminal.as_ref().unwrap();
                assert_eq!(terminal.id,terminal_id);assert_eq!(terminal.fixture_pid(),pid);
                terminal.fixture_input(b"\x03");
                b.persist_library(original,cx).unwrap();cx.notify();
            })).unwrap();
        }).unwrap();
        // Click every real setup-dialog outcome; quitting stays intercepted.
        for button in [2usize,0,1] {
            let before=cx.update(|cx| {
                let before=(fs::read(config_file("startup-setup.json").unwrap()).ok(),cx.global::<ProjectLibraryGlobal>().0.clone());
                TEST_SETUP_REPLY.with(|r|r.set(None));
                choose_setup(cx);
                before
            }).unwrap();
            for _ in 0..80 {
                cx.background_executor().timer(Duration::from_millis(20)).await;
                if TEST_SETUP_ALERT.with(|slot|slot.borrow().is_some()) {break;}
            }
            let alert=TEST_SETUP_ALERT.with(|slot|slot.borrow_mut().take()).expect("Actual setup choice dialog is shown");
            let dialog=alert.window();
            unsafe{alert.buttons().objectAtIndex(button).performClick(None);}
            for _ in 0..80 {
                cx.background_executor().timer(Duration::from_millis(20)).await;
                if cx.update(|cx|!cx.global::<State>().pending).unwrap() {break;}
            }
            cx.update(|cx| {
                assert!(!cx.global::<State>().pending);
                assert!(!dialog.isVisible());assert!(app.modalWindow().is_none());
                assert!(cx.global::<ProjectLibraryGlobal>().0==before.1,"Setup choice cannot delete saved projects or favourites");
                if button==2 {assert_eq!(fs::read(config_file("startup-setup.json").unwrap()).ok(),before.0);}
                else {
                    assert_eq!(startup_setup::load().unwrap().unwrap().restore,button==0);
                    // Exercise cold library initialization, as on the next process start.
                    let shared=cx.remove_global::<ProjectLibraryGlobal>();
                    let plan=startup_setup::startup(cx).unwrap().unwrap();
                    assert_eq!(plan.is_empty(),button==1);
                    assert!(cx.global::<ProjectLibraryGlobal>().0.items==shared.0.items);
                    if button==1 {assert!(cx.global::<ProjectLibraryGlobal>().0.active.is_none());}
                    cx.set_global(shared);
                }
                assert_eq!(unsafe{libc::kill(pid as i32,0)},0,"Actual quitting is intercepted; session survives all fixture choices");
            }).unwrap();
        }
        cx.update(|_| {
            TEST_QUITS.with(|q|assert_eq!(q.get(),Some(4)));
            TEST_REPLY.with(|r|r.set(None));TEST_QUITS.with(|q|q.set(None));
            TEST_SETUP_REPLY.with(|r|r.set(None));
            ALLOW.with(|a|a.set(true));
        }).unwrap();
        let reply:NSApplicationTerminateReply=unsafe{objc2::msg_send![native,applicationShouldTerminate:&*app]};
        assert_eq!(reply,NSApplicationTerminateReply::TerminateNow,"Confirmed quit passes once through AppKit");
        assert!(!ALLOW.with(|a|a.get()));
        cx.update(|cx| {
            println!("PASS real native Cancel buttons twice remove the warning and application modality, preserve running jobs and allow another quit; native Quit interception, idle-shell bypass, foreground process/project details, parked project scan, Cancel and explicit Quit Anyway routing with identical owned PTY/PID; actual quitting intercepted");
            println!("PASS actual Keep Setup / Start Fresh / Cancel native buttons, atomic next-start choice, dismissed modal sessions, unchanged projects/favourites and live fixture PID; quitting intercepted");
            done(cx);
        }).unwrap();
    }).detach();
}
