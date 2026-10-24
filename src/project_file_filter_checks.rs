//! Real controls/native editor and cross-window project filter propagation.
use super::*;
use crate::project_file_filter::{Rule, TEST_EDITOR, TEST_HIDDEN};
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
fn contains(b: &Schnauz, name: &str) -> bool {
    b.active_tab().visible_entries().any(|e| e.name == name)
}

pub(super) fn run(
    window: gpui::WindowHandle<Fixture>,
    cx: &mut App,
    done: impl FnOnce(&mut App) + 'static,
) {
    let original_size = cx
        .update_window(*window, |_, w, _| w.viewport_size())
        .unwrap();
    cx.update_window(*window, |_, w, _| {
        w.resize(size(px(1600.), original_size.height))
    })
    .unwrap();
    TEST_HIDDEN.with(|v| v.set(true));
    let (original, project, other, dir, tab_id, pid, session_id, todoist, filter, hidden, snapshot) =
        window
            .update(cx, |f, _, cx| {
                f.browser.update(cx, |b, cx| {
                    let original = b.projects.clone();
                    let project = b.projects.active;
                    let tab = b.active_tab();
                    let snapshot = (
                        tab.current_dir.clone(),
                        tab.entries.clone(),
                        tab.find_query.clone(),
                        tab.selection.clone(),
                        tab.anchor.clone(),
                    );
                    let dir = config_dir().unwrap().join("project-file-filter-fixtures");
                    fs::create_dir_all(&dir).unwrap();
                    b.tab_mut(b.active_pane).current_dir = dir.clone();
                    b.tab_mut(b.active_pane).find_query = None;
                    b.tab_mut(b.active_pane).selection.clear();
                    b.tab_mut(b.active_pane).anchor = None;
                    for name in ["filter-keep.blend", "filter-hide.blend1", "filter-hide.bak"] {
                        fs::write(dir.join(name), "filter fixture data").unwrap();
                    }
                    let tab_id = b.active_tab().id;
                    let session = b.active_tab().terminal.as_ref().unwrap();
                    let pid = session.fixture_pid();
                    let session_id = session.id;
                    let todoist = b.todoist.open;
                    b.todoist.open = false;
                    let filter = b.active_tab().entry_filter;
                    let hidden = b.active_tab().show_hidden;
                    b.tab_mut(b.active_pane).entry_filter = EntryFilter::All;
                    b.tab_mut(b.active_pane).show_hidden = false;
                    let mut library = b.projects.clone();
                    let other = library
                        .create("Other filter fixture", b.capture_layout())
                        .unwrap();
                    library.sidebar_for_mut(project).unwrap().file_filter =
                        Rule::parse(".blend1,.bak", false).unwrap();
                    library.sidebar_for_mut(Some(other)).unwrap().file_filter =
                        Rule::parse(".blend", true).unwrap();
                    b.persist_library(library, cx).unwrap();
                    b.reload_pane(b.active_pane, cx);
                    (
                        original, project, other, dir, tab_id, pid, session_id, todoist, filter,
                        hidden, snapshot,
                    )
                })
            })
            .unwrap();
    paint(window, cx);
    let (ptr, _height) = cx
        .update_window(*window, |_, w, _| {
            (
                ns_view_ptr(w).unwrap() as usize,
                f32::from(w.viewport_size().height),
            )
        })
        .unwrap();
    cx.spawn(async move|cx| {
        let view=unsafe{&*(ptr as *const NSView)};
        let number:isize=unsafe{objc2::msg_send![&*view.window().unwrap(),windowNumber]};
        // Native input must run outside an App borrow so GPUI can dispatch it.
        macro_rules! click {
            ($probe:expr) => {{
                let position=cx.update(|cx|{paint(window,cx);recorded_bounds($probe).center()}).unwrap();
                let height=view.bounds().size.height as f32;
                for kind in [NSEventType::MouseMoved,NSEventType::LeftMouseDown,NSEventType::LeftMouseUp] {
                    let event=NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(kind,
                        objc2_foundation::NSPoint::new(f32::from(position.x) as f64,(height-f32::from(position.y)) as f64),NSEventModifierFlags::empty(),0.,number,None,0,1,0.).unwrap();
                    match kind {NSEventType::LeftMouseDown=>view.mouseDown(&event),NSEventType::LeftMouseUp=>view.mouseUp(&event),_=>view.mouseMoved(&event)}
                    cx.update(|cx|paint(window,cx)).unwrap();
                }
            }};
        }
        cx.background_executor().timer(Duration::from_millis(100)).await;
        click!("project-filter-on");
        cx.background_executor().timer(Duration::from_millis(20)).await;
        cx.update(|cx| {
            paint(window,cx);
            window.update(cx,|f,_,cx|f.browser.update(cx,|b,cx|{
                assert!(b.projects.sidebar().file_filter.enabled);
                assert!(contains(b,"filter-keep.blend"));assert!(!contains(b,"filter-hide.blend1"));assert!(!contains(b,"filter-hide.bak"));
                let previous=b.projects.sidebar().file_filter.clone();
                b.project_load_error=Some("fixture persistence failure".into());
                let mut disabled=previous.clone();disabled.enabled=false;
                assert!(b.save_project_file_filter(project,&previous,disabled,cx).is_err());
                assert_eq!(b.projects.sidebar().file_filter,previous,"Failed save cannot change visibility");
                b.project_load_error=None;
            })).unwrap();
        }).unwrap();
        click!("project-filter-off");
        cx.background_executor().timer(Duration::from_millis(20)).await;
        cx.update(|cx| {
            paint(window,cx);
            window.update(cx,|f,_,cx|{let b=f.browser.read(cx);assert!(contains(b,"filter-hide.bak"));assert!(contains(b,"filter-hide.blend1"));assert_eq!(b.projects.sidebar().file_filter.extensions,[".blend1",".bak"]);}).unwrap();
        }).unwrap();
        click!("project-filter-on");
        click!("project-filter-modify");
        // Cancel a real native accessory text field without changing the list.
        for _ in 0..100 {cx.background_executor().timer(Duration::from_millis(10)).await;if TEST_EDITOR.with(|s|s.borrow().is_some()){break;}}
        let (alert,field)=TEST_EDITOR.with(|s|s.borrow().as_ref().cloned()).expect("Native Modify dialog opened by actual button");
        assert!(field.isEditable()&&field.isSelectable());
        assert!(field.frame().size.width>=480.);
        let native_number:isize=unsafe{objc2::msg_send![&*alert.window(),windowNumber]};
        let key=NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
            NSEventType::KeyDown,objc2_foundation::NSPoint::new(0.,0.),NSEventModifierFlags::Command,0.,native_number,None,&NSString::from_str("a"),&NSString::from_str("a"),false,0).unwrap();
        assert!(field.performKeyEquivalent(&key),"The native field handles Cmd+A without a global Edit menu");
        assert!(field.currentEditor().unwrap().selectedRange().length>0);
        field.setStringValue(&NSString::from_str(".blend"));
        unsafe{alert.buttons().objectAtIndex(1).performClick(None);}
        for _ in 0..100 {
            cx.background_executor().timer(Duration::from_millis(10)).await;
            if cx.update(|cx|window.update(cx,|f,_,cx|!f.browser.read(cx).project_filter_editor).unwrap()).unwrap(){break;}
        }
        cx.update(|cx| {
            window.update(cx,|f,_,cx|assert_eq!(f.browser.read(cx).projects.sidebar().file_filter.extensions,[".blend1",".bak"])).unwrap();
        }).unwrap();
        click!("project-filter-modify");
        for _ in 0..100 {cx.background_executor().timer(Duration::from_millis(10)).await;if TEST_EDITOR.with(|s|s.borrow().is_some()){break;}}
        let (alert,field)=TEST_EDITOR.with(|s|s.borrow().as_ref().cloned()).unwrap();
        // A queued cross-window project switch must not retarget an open editor.
        cx.update(|cx|window.update(cx,|f,_,cx|f.browser.update(cx,|b,cx|b.switch_project(Some(other),cx).unwrap())).unwrap()).unwrap();
        field.setStringValue(&NSString::from_str("*.BLEND1"));
        unsafe{alert.buttons().objectAtIndex(0).performClick(None);}
        for _ in 0..100 {
            cx.background_executor().timer(Duration::from_millis(10)).await;
            if cx.update(|cx|window.update(cx,|f,_,cx|!f.browser.read(cx).project_filter_editor).unwrap()).unwrap(){break;}
        }
        cx.update(|cx|window.update(cx,|f,_,cx|f.browser.update(cx,|b,cx|{
            assert_eq!(b.projects.active,Some(other));
            assert_eq!(b.projects.sidebar().file_filter.extensions,[".blend"]);
            assert_eq!(b.projects.sidebar_for(project).unwrap().file_filter.extensions,[".blend1"]);
            assert!(!contains(b,"filter-keep.blend"));assert!(contains(b,"filter-hide.blend1"));
            b.switch_project(project,cx).unwrap();b.apply_project_file_filters();
            assert!(!contains(b,"filter-hide.blend1"));assert!(contains(b,"filter-hide.bak"));
            assert_eq!(b.active_tab().id,tab_id);let terminal=b.active_tab().terminal.as_ref().unwrap();assert_eq!(terminal.fixture_pid(),pid);assert_eq!(terminal.id,session_id);assert_eq!(unsafe{libc::kill(pid as i32,0)},0);
        })).unwrap()).unwrap();
        let second_dir=dir.clone();
        let second=cx.update(|cx|cx.open_window(WindowOptions{show:false,focus:false,window_bounds:Some(WindowBounds::Windowed(Bounds::new(point(px(-20000.),px(-20000.)),size(px(980.),px(600.))))),..Default::default()},move|_,cx|{
            let browser=cx.new(|cx|{let mut b=Schnauz::new(projects::Shortcut::local(second_dir.clone(),projects::Section::Folders),cx);b.reload_pane(0,cx);b});cx.new(|_|Fixture{browser})
        }).unwrap()).unwrap();
        cx.update(|cx|window.update(cx,|f,_,cx|f.browser.update(cx,|b,cx|b.set_project_file_filter(project,false,cx))).unwrap()).unwrap();
        let mut propagated=false;
        for _ in 0..100 {
            cx.background_executor().timer(Duration::from_millis(10)).await;
            propagated=cx.update(|cx|{paint(second,cx);second.update(cx,|f,_,cx|{let b=f.browser.read(cx);!b.projects.sidebar().file_filter.enabled&&contains(b,"filter-hide.blend1")}).unwrap()}).unwrap();
            if propagated {break;}
        }
        assert!(propagated,"Filter changes reach another browser through the shared project library");
        cx.update(|cx| {
            cx.update_window(*second,|_,w,_|w.remove_window()).unwrap();
            window.update(cx,|f,_,cx|f.browser.update(cx,|b,cx|{
                b.persist_library(original,cx).unwrap();b.project_sessions.remove(&Some(other));b.apply_project_file_filters();
                b.todoist.open=todoist;b.tab_mut(b.active_pane).entry_filter=filter;b.tab_mut(b.active_pane).show_hidden=hidden;
                for name in ["filter-keep.blend","filter-hide.blend1","filter-hide.bak"] {assert_eq!(fs::read_to_string(dir.join(name)).unwrap(),"filter fixture data");fs::remove_file(dir.join(name)).unwrap();}
                let tab=b.tab_mut(b.active_pane);
                tab.current_dir=snapshot.0;tab.entries=snapshot.1;tab.find_query=snapshot.2;
                tab.selection=snapshot.3;tab.anchor=snapshot.4;tab.recompute_results();
                // The owned shell may still have this disposable directory as its cwd; the enclosing fixture cleans it up after terminal teardown.
            })).unwrap();
            TEST_HIDDEN.with(|v|v.set(false));
            cx.update_window(*window,|_,w,_|w.resize(original_size)).unwrap();paint(window,cx);
            println!("PASS actual project Filter On/Off and native Modify Save/Cancel, captured-project editing, persistence failure, independent projects, cross-window propagation and unchanged owned tab/PTY/PID; disposable files only");
            done(cx);
        }).unwrap();
    }).detach();
}
