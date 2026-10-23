//! Real toolbar pointer drags and new-text-file click in a hidden disposable window.
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
    let original = prefs();
    let mut defaults = original;
    defaults.toolbar_order = toolbar_order::DEFAULT;
    apply_prefs(defaults, cx);
    paint(window, cx);
    assert!(recorded_bounds("new-file").left() > recorded_bounds("new-folder").right());
    let (ptr, height) = cx
        .update_window(*window, |_, w, _| {
            (
                ns_view_ptr(w).unwrap() as usize,
                f32::from(w.viewport_size().height),
            )
        })
        .unwrap();
    let (tab_id, path, hidden) = window
        .update(cx, |f, _, cx| {
            let b = f.browser.read(cx);
            (
                b.active_tab().id,
                b.active_tab().current_dir.clone(),
                b.active_tab().show_hidden,
            )
        })
        .unwrap();
    cx.spawn(async move |cx| {
        cx.background_executor().timer(Duration::from_millis(10)).await;
        let view=unsafe{&*(ptr as *const NSView)};
        let number:isize=unsafe{objc2::msg_send![&*view.window().unwrap(),windowNumber]};
        // Both directions, an inactive button, menu controls and cancellation outside.
        for (source,target,after) in [("new-folder","sort",true),("new-file","hidden",false),("options","new-file",true),("share","options",false),("views","new-folder",false),("network","views",true),("sort","outside",false)] {
            let (from,to,expected)=cx.update(|cx| {
                paint(window,cx);
                let from=recorded_bounds(source).center();
                let to=if target=="outside" {point(px(500.),px(400.))} else {let b=recorded_bounds(target);point(if after {b.right()-px(3.)}else{b.left()+px(3.)},b.center().y)};
                let mut order=prefs().toolbar_order.to_vec();
                if target!="outside" {let source_control=order.iter().find(|c|c.key()==source).copied().unwrap();let target_control=order.iter().find(|c|c.key()==target).copied().unwrap();order.retain(|c|*c!=source_control);let i=order.iter().position(|c|*c==target_control).unwrap()+usize::from(after);order.insert(i,source_control);}
                (from,to,order)
            }).unwrap();
            for (kind,pos) in [(NSEventType::MouseMoved,from),(NSEventType::LeftMouseDown,from),(NSEventType::LeftMouseDragged,from+point(px(0.),px(8.))),(NSEventType::LeftMouseDragged,to),(NSEventType::LeftMouseUp,to)] {
                let event=NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(kind,objc2_foundation::NSPoint::new(f32::from(pos.x) as f64,(height-f32::from(pos.y)) as f64),NSEventModifierFlags::empty(),0.,number,None,0,1,0.).unwrap();
                match kind {NSEventType::LeftMouseDown=>view.mouseDown(&event),NSEventType::LeftMouseDragged=>view.mouseDragged(&event),NSEventType::LeftMouseUp=>view.mouseUp(&event),_=>view.mouseMoved(&event)}
                cx.update(|cx|paint(window,cx)).unwrap();
                cx.background_executor().timer(Duration::from_millis(10)).await;
            }
            cx.update(|cx| {
                assert_eq!(prefs().toolbar_order.to_vec(),expected,"Pointer drop reorders {source} at {target}");
                assert_eq!(load_prefs().toolbar_order.to_vec(),expected,"Order survives disk reload");
                assert!(!cx.has_active_drag());
                window.update(cx,|f,_,cx|{let b=f.browser.read(cx);assert_eq!(b.active_tab().id,tab_id);assert_eq!(b.active_tab().current_dir,path);assert_eq!(b.active_tab().show_hidden,hidden);assert!(b.new_text_file.is_none()&&b.rename.is_none()&&b.sort_menu.is_none()&&b.context_menu.is_none(),"Dragging never clicks the action");assert!(b.jobs.is_empty());}).unwrap();
            }).unwrap();
        }
        let at=cx.update(|cx|{paint(window,cx);recorded_bounds("new-file").center()}).unwrap();
        for kind in [NSEventType::LeftMouseDown,NSEventType::LeftMouseUp] {
            let event=NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(kind,objc2_foundation::NSPoint::new(f32::from(at.x) as f64,(height-f32::from(at.y)) as f64),NSEventModifierFlags::empty(),0.,number,None,0,1,0.).unwrap();
            if kind==NSEventType::LeftMouseDown{view.mouseDown(&event);}else{view.mouseUp(&event);}
            cx.update(|cx|paint(window,cx)).unwrap();
            cx.background_executor().timer(Duration::from_millis(10)).await;
        }
        cx.update(|cx| {
            window.update(cx,|f,_,cx| {
                f.browser.update(cx,|b,cx| {
                    assert!(b.new_text_file.is_some(),"New file button opens the naming dialog");
                    let mut key=gpui::Keystroke::parse("x").unwrap();key.key_char=Some("toolbar-fixture".into());
                    b.handle_text_file_key(&KeyDownEvent{keystroke:key,is_held:false},cx);
                });
            }).unwrap();
            cx.update_window(*window,|_,w,cx|w.dispatch_keystroke(gpui::Keystroke::parse("enter").unwrap(),cx)).unwrap();
            window.update(cx,|f,_,cx| {
                f.browser.update(cx,|b,cx| {
                    assert!(b.new_text_file.is_none());let created=path.join("toolbar-fixture.txt");assert!(created.is_file());assert_eq!(fs::read(&created).unwrap(),Vec::<u8>::new());fs::remove_file(created).unwrap();b.tab_mut(b.active_pane).selection.clear();b.refresh_pane(b.active_pane,cx);
                });
            }).unwrap();
            apply_prefs(original,cx);paint(window,cx);
            println!("PASS native toolbar drags in both directions, disabled/menu controls, outside cancellation, saved order reload, no action during drag, and new-file click/name/create .txt; disposable folder only");
            done(cx);
        }).unwrap();
    }).detach();
}
