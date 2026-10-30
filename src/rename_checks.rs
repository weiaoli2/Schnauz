//! Native mouse editing in disposable file views; never rename live user data.
use super::*;
use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType, NSView};

pub(super) fn run(
    window: gpui::WindowHandle<Fixture>,
    cx: &mut App,
    done: impl FnOnce(&mut App) + 'static,
) {
    let clipboard = clipboard_checks::preserve();
    let paint = move |cx: &mut App| {
        for _ in 0..3 {
            cx.update_window(*window, |_, w, cx| {
                w.refresh();
                w.draw(cx).clear();
            })
            .unwrap();
        }
    };
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
    let mouse = |kind, position: gpui::Point<gpui::Pixels>, shift, clicks| {
        let flags = if shift { NSEventModifierFlags::Shift } else { NSEventModifierFlags::empty() };
        let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
            kind, objc2_foundation::NSPoint::new(f32::from(position.x) as f64, (height-f32::from(position.y)) as f64),
            flags, 0., number, None, 0, clicks, 0.).unwrap();
        match kind { NSEventType::LeftMouseDown => view.mouseDown(&event),
            NSEventType::LeftMouseDragged => view.mouseDragged(&event),
            NSEventType::MouseMoved => view.mouseMoved(&event), _ => view.mouseUp(&event) }
    };
    for mode in 0..3 {
        let path = cx.update(|cx| window.update(cx, |f, w, cx| f.browser.update(cx, |b, cx| {
            b.close_palette(cx); b.todoist.open = false; b.space_map = None;
            let pane = b.active_pane;
                let path = b.tab(pane).current_dir.join("Grüße.txt");
            fs::write(&path, "rename fixture").unwrap();
            b.refresh_pane(pane,cx);
            b.tab_mut(pane).selection=HashSet::from([path.clone()]);
            b.tab_mut(pane).anchor=Some(path.clone());
            b.tab_mut(pane).view=if mode == 1 { ViewMode::Icons } else { ViewMode::List };
            if mode == 2 {
                b.palette_open=true;
                b.file_search.pane=pane; b.file_search.tab=b.pane(pane).active;
                b.file_search.editing=false;
                    b.palette_items=vec![PaletteItem { title:"Grüße.txt".into(), action:Action::Open(path.clone(),false), is_dir:false }];
            }
            let nodes=b.menu_nodes_root(pane,Some((path.clone(),false)),cx);
            assert!(!nodes.iter().any(|n|matches!(n,MenuNode::Item {label,..} if label.starts_with("Quick Look") || label.starts_with("New "))));
            let background=b.menu_nodes_root(pane,None,cx);
            assert!(background.iter().any(|n|matches!(n,MenuNode::Item {label,..} if label=="New Text File…")));
            let remote=b.menu_nodes_remote(pane,Some((path.clone(),false)));
            assert!(!remote.iter().any(|n|matches!(n,MenuNode::Item {label,..} if label.starts_with("New "))));
            let folder = path.parent().unwrap().to_path_buf();
            for nodes in [b.menu_nodes_root(pane,Some((folder.clone(),true)),cx),b.menu_nodes_remote(pane,Some((folder,true)))] {
                assert!(!nodes.iter().any(|n|matches!(n,MenuNode::Item {label,..} if label.starts_with("Quick Look") || label.starts_with("New "))));
                assert!(!matches!(nodes.last(),Some(MenuNode::Sep)));
            }
            assert!(b.menu_nodes_remote(pane,None).iter().any(|n|matches!(n,MenuNode::Item {label,..} if label=="New Folder")));
            b.begin_rename(pane,path.clone(),w,cx); path
        })).unwrap()).unwrap();
        cx.update(|cx|paint(cx)).unwrap();
        let point = |index, cx: &mut App| window.update(cx,|f,_,cx|
            rename_ui::fixture_point(f.browser.read(cx).rename.as_ref().unwrap(),index)).unwrap();
        let start=cx.update(|cx|point(2,cx)).unwrap();
        mouse(NSEventType::LeftMouseDown,start,false,1);
        cx.background_executor().timer(Duration::from_millis(10)).await;
        mouse(NSEventType::LeftMouseUp,start,false,1);
        cx.background_executor().timer(Duration::from_millis(10)).await;
        cx.update(|cx| {
        paint(cx);
        window.update(cx,|f,_,cx| { let b=f.browser.read(cx); let r=b.rename.as_ref().unwrap();
            assert_eq!(r.cursor,2,"Click positions the Unicode caret in mode {mode}"); assert!(b.rename_sel().is_none()); }).unwrap();
        }).unwrap();
        let end=cx.update(|cx|point(5,cx)).unwrap();
        mouse(NSEventType::LeftMouseDown,start,false,1);
        cx.background_executor().timer(Duration::from_millis(10)).await;
        mouse(NSEventType::LeftMouseDragged,end,false,1);
        cx.background_executor().timer(Duration::from_millis(10)).await;
        mouse(NSEventType::LeftMouseUp,end,false,1);
        cx.background_executor().timer(Duration::from_millis(10)).await;
        cx.update(|cx| {
        paint(cx);
        window.update(cx,|f,_,cx| assert_eq!(f.browser.read(cx).rename_sel(),Some((2,5)))).unwrap();
        cx.update_window(*window,|_,w,cx|w.dispatch_keystroke(gpui::Keystroke::parse("cmd-c").unwrap(),cx)).unwrap();
        assert_eq!(cx.read_from_clipboard().and_then(|i|i.text()).as_deref(),Some("üße"));
        }).unwrap();
        let end=cx.update(|cx|point(9,cx)).unwrap();
        mouse(NSEventType::LeftMouseDown,end,true,1);
        cx.background_executor().timer(Duration::from_millis(10)).await;
        mouse(NSEventType::LeftMouseUp,end,true,1);
        cx.background_executor().timer(Duration::from_millis(10)).await;
        cx.update(|cx| {
        paint(cx);
        window.update(cx,|f,_,cx| assert_eq!(f.browser.read(cx).rename_sel(),Some((2,9)))).unwrap();
        window.update(cx,|f,_,cx|f.browser.update(cx,|b,_| b.rename.as_mut().unwrap().dragging=true)).unwrap();
        }).unwrap();
        mouse(NSEventType::MouseMoved,start,false,1);
        cx.background_executor().timer(Duration::from_millis(10)).await;
        cx.update(|cx| {
        window.update(cx,|f,_,cx| {
            let b=f.browser.read(cx);
            assert!(!b.rename.as_ref().unwrap().dragging,"Hover after a lost mouse-up must end selection drag");
            assert_eq!(b.rename_sel(),Some((2,9)));
        }).unwrap();
        window.update(cx,|f,_,cx|f.browser.update(cx,|b,cx| {b.rename=None; b.close_palette(cx);cx.notify();})).unwrap();
        }).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(),"rename fixture");
    }
    cx.update(|cx| {
    drop(clipboard);
    println!("PASS native rename click/caret, Unicode mouse drag, Shift selection and clipboard in List/Icons/search; no Quick Look or New actions on selected-item menus; disposable files only");
    done(cx);
    }).unwrap();
    }).detach();
}
