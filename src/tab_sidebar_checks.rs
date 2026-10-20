//! Real AppKit tab-to-sidebar drops with an owned fixture PTY; no servers.
use super::*;
use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType, NSView};

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
    let (original, tab_id, session_id, view_id, pid, path, remote, counts) = window
        .update(cx, |f, _, cx| {
            f.browser.update(cx, |b, cx| {
                let tab = b.active_tab();
                let terminal = tab.terminal.as_ref().unwrap();
                let result = (
                    b.projects.clone(),
                    tab.id,
                    terminal.id,
                    terminal.view.entity_id(),
                    terminal.fixture_pid(),
                    tab.current_dir.clone(),
                    tab.remote.clone(),
                    b.panes.iter().map(|p| p.tabs.len()).collect::<Vec<_>>(),
                );
                b.projects.common_shortcuts.clear();
                b.projects.sidebar_mut().shortcuts.clear();
                b.persist_library(b.projects.clone(), cx).unwrap();
                cx.notify();
                result
            })
        })
        .unwrap();
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
        // Heading labels toggle their groups without extra icons.
        for (heading, state) in [("projects-heading", "PROJECTS"), ("common-heading", "COMMON")] {
            for collapsed in [true, false] {
                let position=cx.update(|cx| { paint(window,cx); recorded_bounds(heading).center() }).unwrap();
                for kind in [NSEventType::LeftMouseDown,NSEventType::LeftMouseUp] {
                    let event=NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(kind,objc2_foundation::NSPoint::new(f32::from(position.x) as f64,(height-f32::from(position.y)) as f64),NSEventModifierFlags::empty(),0.,number,None,0,1,0.).unwrap();
                    if kind==NSEventType::LeftMouseDown { view.mouseDown(&event); } else { view.mouseUp(&event); }
                    cx.update(|cx|paint(window,cx)).unwrap();
                    cx.background_executor().timer(Duration::from_millis(15)).await;
                }
                cx.update(|cx|window.update(cx,|f,_,cx| {
                    let b=f.browser.read(cx);
                    assert_eq!(b.collapsed_sections.contains(state),collapsed);
                    assert_eq!(read_string_list("collapsed_sections.txt").contains(&state.to_string()),collapsed);
                    if state=="COMMON" && collapsed { assert!(f32::from(b.common_bounds.borrow().unwrap().size.height)<40.); }
                }).unwrap()).unwrap();
            }
        }
        // Local -> Favourites; remote -> Common; duplicate; outside cancellation.
        for phase in 0..4 {
            if phase == 1 {
                cx.update(|cx| window.update(cx,|f,_,cx|f.browser.update(cx,|b,cx| {
                    let server = b.active_tab().terminal.as_ref().unwrap().ssh.as_ref().unwrap().server.clone();
                    b.toggle_section("COMMON".into(),cx);
                    assert!(b.collapsed_sections.contains("COMMON"));
                    let tab = b.tab_mut(b.active_pane);
                    tab.remote=Some(server);
                    tab.current_dir="/fixture/remote folder".into();
                    cx.notify();
                })).unwrap()).unwrap();
            }
            let (source, target) = cx.update(|cx| {
                paint(window,cx);
                let source=recorded_bounds("tab-sidebar-source").center();
                let target=window.update(cx,|f,_,cx| {
                    let b=f.browser.read(cx);
                    let common=b.common_bounds.borrow().unwrap();
                    if phase == 0 { point(common.center().x,common.bottom_right().y+px(12.)) }
                    else if phase < 3 { common.center() }
                    else { point(px(600.),px(450.)) }
                }).unwrap();
                (source,target)
            }).unwrap();
            for (kind, position) in [
                (NSEventType::MouseMoved,source),
                (NSEventType::LeftMouseDown,source),
                (NSEventType::LeftMouseDragged,source+point(px(0.),px(8.))),
                (NSEventType::LeftMouseDragged,target),
                (NSEventType::LeftMouseUp,target),
            ] {
                let event=NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(kind,objc2_foundation::NSPoint::new(f32::from(position.x) as f64,(height-f32::from(position.y)) as f64),NSEventModifierFlags::empty(),0.,number,None,0,1,0.).unwrap();
                match kind {
                    NSEventType::LeftMouseDown=>view.mouseDown(&event),
                    NSEventType::LeftMouseDragged=>view.mouseDragged(&event),
                    NSEventType::LeftMouseUp=>view.mouseUp(&event),
                    _=>view.mouseMoved(&event),
                }
                cx.update(|cx| {
                    paint(window,cx);
                    if kind==NSEventType::LeftMouseDragged && position==target && phase<3 {
                        window.update(cx,|f,_,cx| {
                            let b=f.browser.read(cx);
                            assert!(b.favourites_drop_hover,"Tab drag shows the sidebar reference hint");
                            assert_eq!(b.common_drop_hover,phase>0);
                        }).unwrap();
                    }
                }).unwrap();
                cx.background_executor().timer(Duration::from_millis(15)).await;
            }
            cx.update(|cx|window.update(cx,|f,_,cx|f.browser.update(cx,|b,_| {
                let tab=b.active_tab();
                assert_eq!(tab.id,tab_id);
                assert_eq!(tab.current_dir, if phase == 0 { path.clone() } else { PathBuf::from("/fixture/remote folder") });
                let terminal=tab.terminal.as_ref().unwrap();
                assert_eq!(terminal.id,session_id);
                assert_eq!(terminal.view.entity_id(),view_id);
                assert_eq!(terminal.fixture_pid(),pid);
                assert_eq!(unsafe {libc::kill(pid as i32,0)},0);
                assert_eq!(b.panes.iter().map(|p|p.tabs.len()).collect::<Vec<_>>(),counts);
                assert_eq!(b.projects.active,original.active);
                assert!(b.jobs.is_empty(),"Reference drop never starts a transfer");
                assert!(!b.favourites_drop_hover && !b.common_drop_hover || phase==3);
                assert_eq!(b.projects.sidebar().shortcuts.len(),1);
                assert_eq!(b.projects.sidebar().shortcuts[0].path,path);
                assert!(b.projects.sidebar().shortcuts[0].server.is_none());
                if phase>0 {
                    assert_eq!(b.projects.common_shortcuts.len(),1,"Duplicate and cancelled drops add no extra references");
                    assert!(!b.collapsed_sections.contains("COMMON"),"Dropping on collapsed Common reveals the added reference");
                    let shortcut=&b.projects.common_shortcuts[0];
                    assert_eq!(shortcut.path,Path::new("/fixture/remote folder"));
                    assert_eq!(shortcut.kind,projects::Section::Folders);
                    let server=shortcut.server.as_ref().unwrap();
                    assert!(server.control_path.is_none(),"No live transport is saved");
                    assert_eq!(server.ssh_alias,"fixture-alias");
                }
                let saved=projects::Library::load(&config_file("projects.json").unwrap()).unwrap();
                assert_eq!(saved.common_shortcuts,b.projects.common_shortcuts);
                assert_eq!(saved.sidebar().shortcuts,b.projects.sidebar().shortcuts);
            })).unwrap()).unwrap();
        }
        cx.update(|cx| {
            window.update(cx,|f,_,cx|f.browser.update(cx,|b,cx| {
                let pane=b.active_pane;
                let drag=b.tab_folder_drag(pane,b.panes[pane].active,cx);
                b.move_tab(&drag,pane,0,cx);
                assert_eq!(b.active_tab().id,tab_id,"Tab reorder still moves the original tab");
                assert_eq!(b.active_tab().terminal.as_ref().unwrap().fixture_pid(),pid);
                let ordered=b.panes.iter().flat_map(|p|p.tabs.iter().map(|t|t.id)).collect::<Vec<_>>();
                let mut stale=drag.clone(); stale.tab_id=u64::MAX;
                b.move_tab(&stale,pane,1,cx);
                assert_eq!(ordered,b.panes.iter().flat_map(|p|p.tabs.iter().map(|t|t.id)).collect::<Vec<_>>(),"Stale drag never moves another tab");
                let tab=b.tab_mut(b.active_pane); tab.remote=remote; tab.current_dir=path;
                b.persist_library(original,cx).unwrap(); cx.notify();
            })).unwrap();
            println!("PASS text-only Projects/Common collapse persistence and native local/SSH tab drops to Favourites/Common, hint routing, deduplication, outside cancellation, persisted server alias and unchanged owned tab/PTY/PID; no transfer or connection");
            done(cx);
        }).unwrap();
    }).detach();
}
