//! Hidden, disposable browser interaction regressions. Never used by normal builds.
#![allow(deprecated)] // GPUI 0.2 reads the legacy filenames pasteboard type.
use super::*;
fn paint(window: gpui::WindowHandle<Fixture>, cx: &mut App) {
    for _ in 0..3 {
        cx.update_window(*window, |_, window, cx| {
            window.refresh();
            window.draw(cx).clear();
        })
        .unwrap();
    }
}
fn center(name: &'static str) -> gpui::Point<gpui::Pixels> {
    BOUNDS.with(|all| {
        all.borrow()
            .get(name)
            .unwrap_or_else(|| panic!("missing {name}"))
            .center()
    })
}
use objc2::runtime::AnyObject;
use objc2::{rc::Retained, DeclaredClass};
use objc2_app_kit::{NSFilenamesPboardType, NSPasteboard};
use objc2_foundation::{NSArray, NSPoint};
struct DragInfoState {
    board: Retained<NSPasteboard>,
    location: std::cell::Cell<NSPoint>,
}
define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "SchnauzFixtureDraggingInfo"]
    #[ivars = DragInfoState]
    struct DragInfo;
    unsafe impl NSObjectProtocol for DragInfo {}
    impl DragInfo {
        #[unsafe(method(draggingPasteboard))]
        fn board(&self) -> &NSPasteboard { &self.ivars().board }
        #[unsafe(method(draggingLocation))]
        fn location(&self) -> NSPoint {self.ivars().location.get()}
        #[unsafe(method(draggingSourceOperationMask))]
        fn mask(&self) -> NSDragOperation { NSDragOperation::Copy | NSDragOperation::Move }
    }
);
thread_local! { static INFO: RefCell<Option<Retained<DragInfo>>> = const {RefCell::new(None)}; }
fn prepare_info(paths: &[PathBuf]) {
    let mtm = objc2::MainThreadMarker::new().unwrap();
    let board = NSPasteboard::pasteboardWithUniqueName();
    let names: Vec<_> = paths
        .iter()
        .map(|p| NSString::from_str(p.to_str().unwrap()))
        .collect();
    let names = NSArray::from_retained_slice(&names);
    unsafe {
        board.declareTypes_owner(&NSArray::from_slice(&[NSFilenamesPboardType]), None);
        assert!(board.setPropertyList_forType(&names, NSFilenamesPboardType));
    }
    let allocated = DragInfo::alloc(mtm).set_ivars(DragInfoState {
        board,
        location: std::cell::Cell::new(NSPoint::new(0., 0.)),
    });
    let info: Retained<DragInfo> = unsafe { objc2::msg_send![super(allocated), init] };
    INFO.with(|i| *i.borrow_mut() = Some(info));
}
fn event(window: gpui::WindowHandle<Fixture>, event: gpui::FileDropEvent, cx: &mut App) {
    let (ptr, height) = cx
        .update_window(*window, |_, window, _| {
            (
                ns_view_ptr(window).unwrap(),
                f32::from(window.viewport_size().height),
            )
        })
        .unwrap();
    let content = unsafe { &*(ptr as *const objc2_app_kit::NSView) };
    let native = content.window().unwrap();
    let ptr = (&*native as *const objc2_app_kit::NSWindow).cast::<std::ffi::c_void>();
    let info = INFO.with(|i| i.borrow().as_ref().unwrap().clone());
    // Invoke our native callbacks only after releasing GPUI's App borrow. This
    // exercises the actual AppKit -> GPUI -> browser path, with a private pasteboard.
    cx.spawn(async move |cx| {
        cx.background_executor()
            .timer(Duration::from_millis(10))
            .await;
        let view = unsafe { &*(ptr as *const AnyObject) };
        let position = match &event {
            gpui::FileDropEvent::Entered { position, .. }
            | gpui::FileDropEvent::Pending { position }
            | gpui::FileDropEvent::Submit { position } => Some(*position),
            _ => None,
        };
        if let Some(position) = position {
            info.ivars().location.set(NSPoint::new(
                f32::from(position.x) as f64,
                (height - f32::from(position.y)) as f64,
            ));
        }
        unsafe {
            match event {
                gpui::FileDropEvent::Entered { .. } => {
                    let op: NSDragOperation = objc2::msg_send![view,draggingEntered:&*info];
                    assert!(!op.is_empty());
                }
                gpui::FileDropEvent::Pending { .. } => {
                    let op: NSDragOperation = objc2::msg_send![view,draggingUpdated:&*info];
                    assert!(!op.is_empty());
                }
                gpui::FileDropEvent::Submit { .. } => {
                    let accepted: objc2::runtime::Bool =
                        objc2::msg_send![view,performDragOperation:&*info];
                    assert!(accepted.as_bool());
                }
                gpui::FileDropEvent::Exited => {
                    let _: () = objc2::msg_send![view,draggingExited:&*info];
                }
            }
        }
    })
    .detach();
}
pub(super) fn run(window: gpui::WindowHandle<Fixture>, directory: PathBuf, cx: &mut App) {
    let source = directory.join("Source");
    let target = directory.join("Target");
    let nested = target.join("Nested");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&nested).unwrap();
    fs::write(source.join("a.txt"), "drag fixture").unwrap();
    window
        .update(cx, |fixture, _, cx| {
            fixture.browser.update(cx, |b, cx| {
                b.todoist.open = false;
                b.notice = None;
                b.panes[0].tabs = vec![Tab::new(source.clone()), Tab::new(target.clone())];
                b.panes[0].active = 0;
                b.active_pane = 0;
                cx.notify();
            })
        })
        .unwrap();
    paint(window, cx);
    prepare_info(&[source.join("a.txt")]);
    let position = center("drag-target-tab");
    native_file_drag::fixture_feedback(false);
    event(
        window,
        gpui::FileDropEvent::Entered {
            position,
            paths: ExternalPaths::default(),
        },
        cx,
    );
    paint(window, cx);
    cx.spawn(async move |cx| {
        cx.background_executor().timer(Duration::from_millis(80)).await;
        cx.update(|cx| {
            window.update(cx, |fixture, _, cx| fixture.browser.update(cx, |b, _| {
                assert_eq!(b.file_drag.label().as_deref(),Some("Move 1 item"));
            })).unwrap();
            native_file_drag::fixture_feedback(true);
            event(window,gpui::FileDropEvent::Pending{position},cx);
        }).unwrap();
        cx.background_executor().timer(Duration::from_millis(80)).await;
        cx.update(|cx| {
            window.update(cx, |fixture, _, cx| fixture.browser.update(cx, |b, _| {
                assert_eq!(b.file_drag.label().as_deref(),Some("Copy 1 item"));
            })).unwrap();
            native_file_drag::fixture_feedback(false);
            event(window,gpui::FileDropEvent::Pending{position},cx);
        }).unwrap();
        cx.background_executor().timer(Duration::from_millis(700)).await;
        cx.update(|cx| {
            window.update(cx, |fixture, _, cx| fixture.browser.update(cx, |b, _| {
                assert_eq!(b.panes[0].active,1,"Dwell must activate the target tab");
                assert_eq!(b.tab(0).current_dir,target);
            })).unwrap();
            paint(window,cx);
            let position=center("drag-folder");
            event(window,gpui::FileDropEvent::Pending{position},cx);
        }).unwrap();
        cx.background_executor().timer(Duration::from_millis(800)).await;
        cx.update(|cx| {
            window.update(cx, |fixture, _, cx| fixture.browser.update(cx, |b, _| {
                assert_eq!(b.tab(0).current_dir,nested,"Dwell must open a real subfolder");
            })).unwrap();
            paint(window,cx);
            native_file_drag::fixture_feedback(true);
            event(window,gpui::FileDropEvent::Submit{position:center("drag-file-body")},cx);
        }).unwrap();
        cx.background_executor().timer(Duration::from_millis(250)).await;
        cx.update(|cx| {
            assert!(source.join("a.txt").exists(),"Copy must retain source");
            assert_eq!(fs::read_to_string(nested.join("a.txt")).unwrap(),"drag fixture");
            native_file_drag::fixture_feedback(false);
            window.update(cx, |fixture, _, cx| fixture.browser.update(cx, |b, cx| {
                b.load_dir_in(0,target.clone(),cx);
            })).unwrap();
            paint(window,cx);
            event(window,gpui::FileDropEvent::Entered{position:center("drag-target-tab"),paths:ExternalPaths::default()},cx);
        }).unwrap();
        cx.background_executor().timer(Duration::from_millis(60)).await;
        cx.update(|cx| event(window,gpui::FileDropEvent::Submit{position:center("drag-target-tab")},cx)).unwrap();
        cx.background_executor().timer(Duration::from_millis(250)).await;
        cx.update(|cx| {
            assert!(!source.join("a.txt").exists(),"Move removes only its source after success");
            assert!(target.join("a.txt").exists());
            window.update(cx, |fixture, _, cx| fixture.browser.update(cx, |b, cx| {
                b.tab_mut(0).remote = None;
                b.load_dir_in(0,source.clone(),cx);
                assert!(b.start_fixture_terminal(0,cx));
                let session=b.tab(0).terminal.as_ref().unwrap();
                let record=session.ssh_record().to_path_buf();
                fs::write(record,format!("schnauz-ssh.fixture\0/tmp/schnauz-ssh.fixture/mux\0192.0.2.1\0fixture\022\0{}\0fixture-alias\0",source.display())).unwrap();
                let id=session.id;
                b.ssh_shell_event(id,"start;schnauz-ssh.fixture",cx);
                b.ssh_shell_event(id,"ready;schnauz-ssh.fixture;2f686f6d652f66697874757265",cx);
                let server=b.tab(0).terminal.as_ref().unwrap().ssh.as_ref().unwrap().server.clone();
                let view=b.tab(0).terminal.as_ref().unwrap().view.entity_id();
                let tab_id=b.tab(0).id; let count=b.panes[0].tabs.len();
                let remote=projects::Shortcut{path:"/home/fixture".into(),kind:projects::Section::Folders,server:Some(server.clone())};
                b.open_favourite(projects::Shortcut::local(source.clone(),projects::Section::Folders),cx);
                assert!(b.tab(0).remote.is_none());
                assert!(!b.tab(0).terminal.as_ref().unwrap().ssh.as_ref().unwrap().following);
                b.open_favourite(remote,cx);
                assert_eq!(b.panes[0].tabs.len(),count,"Favourite must not hide the terminal in another tab");
                assert!(b.tab(0).terminal_open);
                assert_eq!(b.tab(0).id,tab_id); assert_eq!(b.tab(0).terminal.as_ref().unwrap().id,id);
                assert_eq!(b.tab(0).terminal.as_ref().unwrap().view.entity_id(),view);
                assert!(b.tab(0).terminal.as_ref().unwrap().ssh.as_ref().unwrap().following);
                // A different remote folder stays unsynced with the same PTY.
                b.open_favourite(projects::Shortcut{path:"/other".into(),kind:projects::Section::Folders,server:Some(server)},cx);
                assert!(b.tab(0).terminal_open); assert_eq!(b.tab(0).terminal.as_ref().unwrap().id,id);
                assert!(!b.tab(0).terminal.as_ref().unwrap().ssh.as_ref().unwrap().following);
                let mut other=b.tab(0).remote.clone().unwrap(); other.host="192.0.2.2".into();
                b.open_favourite(projects::Shortcut{path:"/elsewhere".into(),kind:projects::Section::Folders,server:Some(other)},cx);
                assert!(b.tab(0).terminal_open); assert_eq!(b.tab(0).terminal.as_ref().unwrap().id,id);
                b.open_favourite(projects::Shortcut::local(source.clone(),projects::Section::Folders),cx);
                assert!(b.tab(0).terminal_open && b.tab(0).remote.is_none());
                // Drop only this owned fixture session, never a live user PTY.
                b.tab_mut(0).terminal.take(); b.tab_mut(0).terminal_open=false;
            })).unwrap();
            native_file_drag::ended();
            println!("PASS hidden tab/folder dwell, actual background copy/move, and SSH favourite roundtrip retaining the same terminal/tab/view; no live server, account or user session accessed");
            fs::remove_dir_all(directory).unwrap(); std::process::exit(0);
        }).unwrap();
    }).detach();
}
