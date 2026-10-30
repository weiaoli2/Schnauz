//! Native titlebar project switching with an owned terminal and isolated folders.
use super::*;
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
    let (original, target, terminal_id, pid) = window
        .update(cx, |f, _, cx| {
            f.browser.update(cx, |b, cx| {
                let original = b.projects.clone();
                let terminal = b.active_tab().terminal.as_ref().unwrap();
                let id = terminal.id;
                let pid = terminal.fixture_pid();
                let layout = b.capture_layout();
                let mut candidate = b.projects.clone();
                let target = candidate
                    .create("Header Active Project", layout.clone())
                    .unwrap();
                let saved = candidate.create("Header Saved Only", layout).unwrap();
                b.persist_library(candidate, cx).unwrap();
                b.project_sessions.insert(
                    Some(target),
                    schnauz::ProjectSession {
                        panes: vec![Pane::new(config_dir().unwrap())],
                        active_pane: 0,
                        split_ratio: 0.5,
                    },
                );
                let chips = b.active_project_chips(cx);
                assert!(chips.iter().any(|(id, _)| *id == Some(target)));
                assert!(
                    !chips.iter().any(|(id, _)| *id == Some(saved)),
                    "Saved layouts alone do not populate active projects"
                );
                cx.notify();
                (original, target, id, pid)
            })
        })
        .unwrap();
    paint(window, cx);
    let title = recorded_bounds("schnauz-title");
    let search = recorded_bounds("top-search-field");
    assert_eq!(search.size.height,px(TOOLBAR_CONTROL_H));
    for id in ["top-search-scope","top-search-mode","search-drive","search-current","search-content","search-file",
        "schnauz-split","schnauz-terminal","schnauz-preview"] {
        let bounds=recorded_bounds(id);
        assert_eq!(bounds.size.height,search.size.height,"Top control height differs: {id}");
        assert!((f32::from(bounds.top()-search.top())).abs()<0.1,"Top control alignment differs: {id}");
    }
    let chip = recorded_bounds("current-project-chip");
    assert!(
        (f32::from(title.origin.y - search.origin.y)).abs() <= 1.1,
        "Search field aligns with the Schnauz title: {title:?} / {search:?}"
    );
    assert!(
        chip.origin.y >= search.bottom_right().y && chip.bottom_right().y <= px(BROWSER_HEADER_H),
        "Project strip fits below search inside existing toolbar"
    );
    let divider=recorded_bounds("project-row-divider");
    assert_eq!(divider.size.height,px(1.));
    assert!(divider.origin.y>=search.bottom()+px(2.) && chip.origin.y>=divider.bottom()+px(2.));
    // The chip probe covers its inner box, excluding its 1 pt outline.
    let above = chip.origin.y - px(1.) - divider.bottom();
    let below = px(BROWSER_HEADER_H - 1.) - (chip.bottom() + px(1.));
    assert!(
        (f32::from(above) - 8.).abs() < 0.1 && (f32::from(above - below)).abs() < 0.1,
        "Project chips have equal clearance between the divider and toolbar bottom border: above={above:?}, below={below:?}"
    );
    window.update(cx,|f,w,cx| {
        let b=f.browser.read(cx);let count=b.active_project_chips(cx).len();
        let first=b.project_strip_scroll.bounds_for_item(0).unwrap();
        let last=b.project_strip_scroll.bounds_for_item(count-1).unwrap();
        let center=(first.origin.x+last.right())/2.+b.project_strip_scroll.offset().x;
        assert!((f32::from(center-w.viewport_size().width/2.)).abs()<1.,"Project group centers on the whole window: first={first:?}, last={last:?}, center={center:?}, offset={:?}, viewport={:?}",b.project_strip_scroll.offset(),w.viewport_size());
    }).unwrap();
    let (ptr, height) = cx
        .update_window(*window, |_, w, _| {
            (
                ns_view_ptr(w).unwrap() as usize,
                f32::from(w.viewport_size().height),
            )
        })
        .unwrap();
    cx.spawn(async move|cx| {
        use objc2_app_kit::{NSEvent,NSEventType,NSEventModifierFlags,NSView};
        cx.background_executor().timer(Duration::from_millis(10)).await;
        let view=unsafe{&*(ptr as *const NSView)};
        let number:isize=unsafe{objc2::msg_send![&*view.window().unwrap(),windowNumber]};
        for project in [Some(target),original.active] {
            let position=cx.update(|cx| {
                paint(window,cx);
                window.update(cx,|f,_,cx| {
                    let b=f.browser.read(cx);let index=b.active_project_chips(cx).iter().position(|(id,_)|*id==project).unwrap();
                    b.project_strip_scroll.bounds_for_item(index).unwrap().center()+b.project_strip_scroll.offset()
                }).unwrap()
            }).unwrap();
            for kind in [NSEventType::LeftMouseDown,NSEventType::LeftMouseUp] {
                let event=NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(kind,objc2_foundation::NSPoint::new(f32::from(position.x) as f64,(height-f32::from(position.y)) as f64),NSEventModifierFlags::empty(),0.,number,None,0,1,0.).unwrap();
                if kind==NSEventType::LeftMouseDown {view.mouseDown(&event);}else{view.mouseUp(&event);}
                cx.update(|cx|paint(window,cx)).unwrap();
                cx.background_executor().timer(Duration::from_millis(20)).await;
            }
            cx.update(|cx|window.update(cx,|f,_,cx| {
                let b=f.browser.read(cx);assert_eq!(b.projects.active,project,"Clicking a project chip switches projects");
                let terminal=if project==original.active {b.active_tab().terminal.as_ref().unwrap()}else{
                    b.project_sessions.get(&original.active).unwrap().panes.iter().flat_map(|p|&p.tabs).find_map(|t|t.terminal.as_ref().filter(|s|s.id==terminal_id)).unwrap()
                };
                assert_eq!(terminal.id,terminal_id);assert_eq!(terminal.fixture_pid(),pid);assert_eq!(unsafe{libc::kill(pid as i32,0)},0);
            }).unwrap()).unwrap();
        }
        cx.update(|cx| {
            window.update(cx,|f,_,cx|f.browser.update(cx,|b,cx| {
                b.project_sessions.remove(&Some(target));b.persist_library(original,cx).unwrap();cx.notify();
            })).unwrap();
            paint(window,cx);
            println!("PASS title/search top alignment, window-centered active-project strip with thin divider and symmetric 8 pt spacing, native project-chip switching and preserved owned terminal/session/PID");
            super::project_file_filter_checks::run(window, cx, move |cx| text_view::check_external_edits(window,cx,done));
        }).unwrap();
    }).detach();
}
