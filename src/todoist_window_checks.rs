//! Isolated native Todoist window interaction checks; no account access.
use super::*;

pub(crate) fn check(
    window: gpui::WindowHandle<crate::ui_checks::Fixture>,
    cx: &mut App,
    done: impl FnOnce(&mut App) + 'static,
) {
    let original_library = cx.global::<ProjectLibraryGlobal>().0.clone();
    let model = shared(cx);
    let original_caches = model.update(cx, |m, _| std::mem::take(&mut m.todoist.caches));
    let browser = window.update(cx, |f, _, _| f.browser.clone()).unwrap();
    let original_open = browser.read(cx).todoist.open;
    let mut library = original_library.clone();
    let layout = browser.read(cx).capture_layout();
    library.items.push(projects::Project {
        id: 900001,
        name: "Floating fixture".into(),
        layout,
        sidebar: projects::Sidebar {
            todoist: Some(projects::TodoistLink {
                account_id: "fixture-account".into(),
                project_id: "floating-other".into(),
                project_name: "Floating fixture".into(),
            }),
            ..Default::default()
        },
    });
    cx.set_global(ProjectLibraryGlobal(library.clone(), None));
    let fill_model = model.clone();
    let fill = move |project: &str, titles: &[&str], cx: &mut App| {
        let data:todoist::Snapshot=serde_json::from_value(serde_json::json!({"project":{"id":project,"name":"Fixture"},"tasks":titles.iter().enumerate().map(|(i,title)|serde_json::json!({"id":format!("{project}-{i}"),"project_id":project,"content":title,"description":"Task details"})).collect::<Vec<_>>(),"sections":[]})).unwrap();
        fill_model.update(cx, |m, cx| {
            let mut response = todoist::Response::default();
            response.success = true;
            response.snapshot = Some(data);
            m.todoist
                .caches
                .entry(("fixture-account".into(), project.into()))
                .or_default()
                .finish_read(response, project);
            cx.notify();
        });
    };
    fill(
        "fixture-project",
        &["First visible task", "Second visible task"],
        cx,
    );
    fill("floating-other", &["Other project task"], cx);
    browser.update(cx, |b, cx| {
        b.todoist.open = true;
        cx.notify();
    });
    for _ in 0..3 {
        cx.update_window(*window, |_, w, cx| {
            w.refresh();
            w.draw(cx).clear();
        })
        .unwrap();
    }
    let pin = ui_checks::recorded_bounds("todoist-pin");
    assert_eq!(f32::from(pin.size.height), 24.);
    // Native AppKit input must run after releasing GPUI's App borrow.
    let (pointer, height, number) = cx
        .update_window(*window, |_, w, _| {
            let pointer = ns_view_ptr(w).unwrap();
            let native = unsafe { &*pointer.cast::<objc2_app_kit::NSView>() }
                .window()
                .unwrap();
            (
                pointer as usize,
                f32::from(w.viewport_size().height),
                native.windowNumber(),
            )
        })
        .unwrap();
    cx.spawn(async move |cx| {
        cx.background_executor().timer(Duration::from_millis(10)).await;
        let view=unsafe {&*(pointer as *const objc2::runtime::AnyObject)};
        for kind in [objc2_app_kit::NSEventType::LeftMouseDown,objc2_app_kit::NSEventType::LeftMouseUp] {
            let event=objc2_app_kit::NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(kind,objc2_foundation::NSPoint::new(f32::from(pin.center().x) as f64,(height-f32::from(pin.center().y)) as f64),objc2_app_kit::NSEventModifierFlags::empty(),0.,number,None,0,1,0.).unwrap();
            unsafe {if kind==objc2_app_kit::NSEventType::LeftMouseDown {let _:()=objc2::msg_send![view,mouseDown:&*event];} else {let _:()=objc2::msg_send![view,mouseUp:&*event];}}
        }
        let floating=cx.update(|cx| {
    let floating=cx.global::<Shared>().floating.expect("Pin detaches the tasks");
    assert!(!browser.read(cx).todoist.open);
    for _ in 0..3 {cx.update_window(*floating,|_,w,cx|{w.refresh();w.draw(cx).clear();}).unwrap();}
    floating.update(cx,|_,w,_| {
        let pointer=ns_view_ptr(w).unwrap();let native=unsafe {&*pointer.cast::<objc2_app_kit::NSView>()}.window().unwrap();
        assert_eq!(native.level(),objc2_app_kit::NSFloatingWindowLevel);
        assert!(!native.hidesOnDeactivate());
        assert!(native.collectionBehavior().contains(objc2_app_kit::NSWindowCollectionBehavior::CanJoinAllSpaces));
    }).unwrap();
    floating.update(cx,|f,w,cx|f.toggle_compact(w,cx)).unwrap();
    floating
        }).unwrap();
        cx.background_executor().timer(Duration::from_millis(20)).await;
        let (second,second_browser,compact_height)=cx.update(|cx| {
    let paint=|cx:&mut App|for _ in 0..3 {cx.update_window(*floating,|_,w,cx|{w.refresh();w.draw(cx).clear();}).unwrap();};
    paint(cx);
    assert_eq!(ui_checks::recorded_text("todoist-task-content").text(),"First visible task");
    let compact_height=floating.update(cx,|_,w,_|unsafe {&*ns_view_ptr(w).unwrap().cast::<objc2_app_kit::NSView>()}.frame().size.height).unwrap();
    assert!(compact_height<=180.,"Native compact height: {compact_height}");
    assert!(f32::from(ui_checks::recorded_text("todoist-task-secondary").bounds().bottom())<=160.,"First task details fit compact viewport");
    model.update(cx,|m,cx| {m.notice=Some("Cannot reach Todoist. Showing cached tasks.".into());cx.notify();});paint(cx);
    assert!(f32::from(ui_checks::recorded_text("todoist-task-secondary").bounds().bottom())<=160.,"Actual task details stay visible with an error");
    model.update(cx,|m,cx| {m.notice=None;cx.notify();});
    fill("fixture-project",&["Second visible task"],cx);paint(cx);
    assert_eq!(ui_checks::recorded_text("todoist-task-content").text(),"Second visible task");
    library.active=Some(900001);cx.set_global(ProjectLibraryGlobal(library.clone(),None));paint(cx);
    assert_eq!(ui_checks::recorded_text("todoist-task-content").text(),"Other project task");
    let second=cx.open_window(WindowOptions {show:false,focus:false,window_bounds:Some(WindowBounds::Windowed(Bounds::new(point(px(-4000.),px(-4000.)),size(px(980.),px(600.))))),..Default::default()},|_,cx|cx.new(|cx|Schnauz::new(projects::Shortcut::local(config_dir().unwrap(),projects::Section::Folders),cx))).unwrap();
    let second_browser=cx.update_window(*second,|_,w,_|w.root::<Schnauz>().flatten().unwrap()).unwrap();
    cx.update_window(*second,|_,w,cx|detach(Some(second_browser.downgrade()),w,cx)).unwrap();
    assert_eq!(cx.global::<Shared>().floating.unwrap().window_id(),floating.window_id(),"Other windows reuse the sole floater");
    assert!(!second_browser.read(cx).todoist.open);
    floating.update(cx,|f,w,cx|f.toggle_compact(w,cx)).unwrap();
    (second,second_browser,compact_height)
        }).unwrap();
        cx.background_executor().timer(Duration::from_millis(20)).await;
        let second_weak=cx.update(|cx| {
    assert!(floating.update(cx,|_,w,_|unsafe {&*ns_view_ptr(w).unwrap().cast::<objc2_app_kit::NSView>()}.frame().size.height).unwrap()>compact_height);
    dock_here(second_browser.downgrade(),cx);
    assert!(!is_floating(cx));assert!(second_browser.read(cx).todoist.open);
    assert!(!browser.read(cx).todoist.open,"Dock Here selects only its destination browser");
    // A detached window holds no strong browser reference after that browser closes.
    cx.update_window(*second,|_,w,cx|detach(Some(second_browser.downgrade()),w,cx)).unwrap();
    let second_weak=second_browser.downgrade();
    second.update(cx,|_,w,_|w.remove_window()).unwrap();
    second_weak
        }).unwrap();
        drop(second_browser);
        cx.background_executor().timer(Duration::from_millis(20)).await;
        cx.update(move |cx| {
    assert!(second_weak.upgrade().is_none(),"Floating task controller must not retain browser/PTY ownership");
    assert!(is_floating(cx));
    close_floating(cx);
    cx.set_global(ProjectLibraryGlobal(original_library,None));
    model.update(cx,|m,cx|{m.todoist.caches=original_caches;m.projects=cx.global::<ProjectLibraryGlobal>().0.clone();cx.notify();});
    browser.update(cx,|b,cx|{b.todoist.open=original_open;cx.notify();});
    println!("PASS real Todoist pin click, native floating level/all Spaces, compact first/next task, shared project switch, singleton across two browsers, Dock Here and no retained browser/PTY; no network or live account");
        }).unwrap();
        cx.background_executor().timer(Duration::from_millis(10)).await;
        cx.update(done).unwrap();
    }).detach();
}
