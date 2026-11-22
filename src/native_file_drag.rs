//! Per-browser AppKit drop destination. GPUI 0.2 always reports Copy; a dedicated
//! view advertises Move/Copy and forwards events to the unchanged GPUI window.
use super::*;
use objc2::runtime::AnyObject;
use objc2::ClassType;
use objc2_app_kit::{NSAutoresizingMaskOptions, NSView};
use objc2_app_kit::{NSEvent, NSEventModifierFlags};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Operation {
    Move,
    Copy,
}
impl Operation {
    pub fn label(self, count: usize) -> String {
        format!(
            "{} {count} item{}",
            if self == Self::Copy { "Copy" } else { "Move" },
            if count == 1 { "" } else { "s" }
        )
    }
    fn native(self) -> NSDragOperation {
        match self {
            Self::Copy => NSDragOperation::Copy,
            Self::Move => NSDragOperation::Move,
        }
    }
}
#[derive(Clone, Copy)]
struct Feedback {
    view: usize,
    allowed: NSDragOperation,
    operation: Operation,
    active: bool,
}
thread_local! {
    static FEEDBACK: RefCell<Feedback> = const { RefCell::new(Feedback { view: 0, allowed: NSDragOperation::Copy, operation: Operation::Copy, active: false }) };
}
#[cfg(feature = "ui-checks")]
thread_local! { static TEST_COMMAND: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) }; }
#[cfg(feature = "ui-checks")]
pub(super) fn fixture_feedback(command: bool) {
    TEST_COMMAND.with(|c| c.set(Some(command)));
    FEEDBACK.with(|s| {
        *s.borrow_mut() = Feedback {
            view: 0,
            allowed: NSDragOperation::Copy | NSDragOperation::Move,
            operation: if command {
                Operation::Copy
            } else {
                Operation::Move
            },
            active: true,
        }
    });
}
pub(super) fn command_down() -> bool {
    #[cfg(feature = "ui-checks")]
    if let Some(command) = TEST_COMMAND.with(|c| c.get()) {
        return command;
    }

    NSEvent::modifierFlags_class().contains(NSEventModifierFlags::Command)
}
fn choose(command: bool, allowed: NSDragOperation, copy_only: bool) -> Option<Operation> {
    if (command || copy_only) && allowed.contains(NSDragOperation::Copy) {
        Some(Operation::Copy)
    } else if !copy_only && allowed.contains(NSDragOperation::Move) {
        Some(Operation::Move)
    } else if allowed.contains(NSDragOperation::Copy) {
        Some(Operation::Copy)
    } else {
        None
    }
}
pub(super) fn operation() -> Operation {
    FEEDBACK.with(|s| s.borrow().operation)
}
pub(super) fn destination_id() -> usize {
    FEEDBACK.with(|s| s.borrow().view)
}
pub(super) fn active_in(view: usize) -> bool {
    FEEDBACK.with(|s| {
        let s = s.borrow();
        s.active && s.view == view
    })
}
pub(super) fn destination(copy_only: bool) -> Operation {
    FEEDBACK.with(|s| {
        let mut s = s.borrow_mut();
        s.operation = choose(command_down(), s.allowed, copy_only).unwrap_or(Operation::Copy);
        s.operation
    })
}
pub(super) fn ended() {
    FEEDBACK.with(|s| s.borrow_mut().active = false);
}
fn prepare(view: &AnyObject, info: &AnyObject) {
    // SAFETY: AppKit supplies its NSDraggingInfo object for these callbacks.
    let allowed: NSDragOperation = unsafe { objc2::msg_send![info, draggingSourceOperationMask] };
    FEEDBACK.with(|state| {
        *state.borrow_mut() = Feedback {
            view: view as *const _ as usize,
            allowed,
            operation: choose(command_down(), allowed, false).unwrap_or(Operation::Copy),
            active: true,
        }
    });
}
fn advertised(accepted: NSDragOperation) -> NSDragOperation {
    if accepted.is_empty() {
        return accepted;
    }
    FEEDBACK.with(|s| {
        let s = s.borrow();
        if s.allowed.contains(s.operation.native()) {
            s.operation.native()
        } else {
            NSDragOperation::None
        }
    })
}
// Allocate the destination with its final class. Changing an initialized
// NSWindow's isa loses AppKit's KVO bookkeeping: full-screen titlebar reparenting
// then throws from NSTitlebarView::viewWillMoveToWindow, leaving a broken transition.
// A normal autoresizing container keeps both NSWindow and GPUIView unchanged.
define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "SchnauzFileDropDestination"]
    struct DropDestination;
    unsafe impl NSObjectProtocol for DropDestination {}
    impl DropDestination {
        #[unsafe(method(draggingEntered:))]
        fn entered(&self, info: &AnyObject) -> NSDragOperation {
            prepare(self, info);
            let Some(window) = self.window() else { return NSDragOperation::None; };
            let accepted = unsafe { msg_send![&*window, draggingEntered: info] };
            advertised(accepted)
        }
        #[unsafe(method(draggingUpdated:))]
        fn updated(&self, info: &AnyObject) -> NSDragOperation {
            prepare(self, info);
            let Some(window) = self.window() else { return NSDragOperation::None; };
            let accepted = unsafe { msg_send![&*window, draggingUpdated: info] };
            advertised(accepted)
        }
        #[unsafe(method(draggingExited:))]
        fn exited(&self, info: &AnyObject) {
            FEEDBACK.with(|s| {
                let mut s = s.borrow_mut();
                if s.view == self as *const _ as usize { s.active = false; }
            });
            if let Some(window) = self.window() {
                unsafe { let _: () = msg_send![&*window, draggingExited: info]; }
            }
        }
        #[unsafe(method(performDragOperation:))]
        fn submit(&self, info: &AnyObject) -> objc2::runtime::Bool {
            prepare(self, info); // Read Command at release, even without movement.
            let Some(window) = self.window() else {
                ended();
                return objc2::runtime::Bool::NO;
            };
            let result = unsafe { msg_send![&*window, performDragOperation: info] };
            // GPUI's conclude callback alone leaves external_files_dragged set.
            unsafe { let _: () = msg_send![&*window, draggingExited: info]; }
            ended();
            result
        }
        #[unsafe(method(concludeDragOperation:))]
        fn conclude(&self, info: &AnyObject) {
            unsafe { let _: () = msg_send![self, draggingExited: info]; }
        }
    }
);
pub(super) fn install(window: &Window, cx: &mut App) {
    let Some(ptr) = ns_view_ptr(window) else { return; };
    // Retain across reparenting: the original parent may own the last reference.
    let Some(content) = (unsafe { Retained::retain(ptr.cast::<NSView>()) }) else { return; };
    let Some(parent) = (unsafe { content.superview() }) else { return; };
    if parent.isKindOfClass(DropDestination::class()) { return; }
    // Reparenting can dispatch native focus/appearance/resize callbacks. Do it
    // outside render's GPUI App/Window/entity borrows, just like native menus.
    cx.spawn(async move |_| { install_content(&content); }).detach();
}
fn install_content(content: &NSView) {
    let Some(parent) = (unsafe { content.superview() }) else { return; };
    if parent.isKindOfClass(DropDestination::class()) { return; }
    let Some(native_window) = content.window() else { return; };
    let responder = native_window.firstResponder();
    let mtm = objc2::MainThreadMarker::new().expect("browser drop destination on main thread");
    let destination: Retained<DropDestination> = unsafe {
        msg_send![super(DropDestination::alloc(mtm).set_ivars(())), initWithFrame: content.frame()]
    };
    destination.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    // Match GPUI 0.2's single registered file-list type.
    #[allow(deprecated)]
    destination.registerForDraggedTypes(&objc2_foundation::NSArray::from_slice(&[
        unsafe { objc2_app_kit::NSFilenamesPboardType },
    ]));
    parent.addSubview(&destination);
    content.removeFromSuperview();
    content.setFrameOrigin(objc2_foundation::NSPoint::new(0., 0.));
    destination.addSubview(content);
    if let Some(responder) = responder { native_window.makeFirstResponder(Some(&responder)); }
    // The registered ancestor view receives drops over GPUI and native previews;
    // forwarding explicitly to GPUI's original NSWindow avoids double delivery.
    native_window.unregisterDraggedTypes();
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_copy_badge_matches_actual_operation_and_source_permissions() {
        let both = NSDragOperation::Copy | NSDragOperation::Move;
        assert_eq!(choose(false, both, false), Some(Operation::Move));
        assert_eq!(choose(true, both, false), Some(Operation::Copy));
        assert_eq!(choose(false, both, true), Some(Operation::Copy));
        assert_eq!(
            choose(false, NSDragOperation::Copy, false),
            Some(Operation::Copy)
        );
        assert_eq!(choose(true, NSDragOperation::None, false), None);
        assert_eq!(Operation::Copy.label(3), "Copy 3 items");
        assert_eq!(Operation::Move.label(1), "Move 1 item");
    }
}

/// Dwell must also run inside AppKit's NSEventTrackingRunLoopMode. GPUI's
/// foreground executor is dispatched to the main queue and pauses in that loop.
use objc2::{define_class, msg_send, rc::Retained, DeclaredClass};
use objc2_foundation::{NSObject, NSObjectProtocol, NSRunLoop, NSRunLoopCommonModes, NSTimer};
struct TimerState {
    callback: RefCell<Option<Box<dyn FnOnce()>>>,
}
define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "SchnauzDragDwellTarget"]
    #[ivars = TimerState]
    struct TimerTarget;
    unsafe impl NSObjectProtocol for TimerTarget {}
    impl TimerTarget {
        #[unsafe(method(fire:))]
        fn fire(&self, _: &NSTimer) {
            let callback = self.ivars().callback.borrow_mut().take();
            if let Some(callback) = callback { callback(); }
        }
    }
);
pub(super) struct DwellTimer(Retained<NSTimer>);
impl DwellTimer {
    pub fn new(delay: Duration, callback: impl FnOnce() + 'static) -> Self {
        let mtm = objc2::MainThreadMarker::new().expect("drag timer on main thread");
        let allocated = TimerTarget::alloc(mtm).set_ivars(TimerState {
            callback: RefCell::new(Some(Box::new(callback))),
        });
        let target: Retained<TimerTarget> = unsafe { msg_send![super(allocated), init] };
        let timer = unsafe {
            NSTimer::timerWithTimeInterval_target_selector_userInfo_repeats(
                delay.as_secs_f64(),
                &target,
                objc2::sel!(fire:),
                None,
                false,
            )
        };
        unsafe {
            NSRunLoop::currentRunLoop().addTimer_forMode(&timer, NSRunLoopCommonModes);
        }
        Self(timer)
    }
}
impl Drop for DwellTimer {
    fn drop(&mut self) {
        self.0.invalidate();
    }
}

thread_local! {
    static SOURCE_VIEW: RefCell<Option<Retained<objc2_app_kit::NSView>>> = const { RefCell::new(None) };
    static SOURCE_COMPLETION: RefCell<Option<Box<dyn FnOnce()>>> = const { RefCell::new(None) };
}
pub(super) fn source_completion(callback: impl FnOnce() + 'static) {
    SOURCE_COMPLETION.with(|slot| *slot.borrow_mut() = Some(Box::new(callback)));
}
/// Release only this owned view's GPUI pointer tracking when AppKit takes over.
/// The off-view release cannot click/drop an item. Passing through GPUI's native
/// entry point retires its repeating source-motion task as well as AnyDrag.
pub(super) fn release_pointer_tracking(view: &objc2_app_kit::NSView) {
    let Some(window) = view.window() else {
        return;
    };
    let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
        objc2_app_kit::NSEventType::LeftMouseUp,
        objc2_foundation::NSPoint::new(-10000., -10000.),
        NSEventModifierFlags::empty(), 0., window.windowNumber(), None, 0, 0, 0.);
    if let Some(event) = event {
        view.mouseUp(&event);
    }
}
pub(super) fn source_started(view: &objc2_app_kit::NSView) {
    release_pointer_tracking(view);
    SOURCE_VIEW.with(|slot| {
        *slot.borrow_mut() =
            unsafe { Retained::retain(view as *const _ as *mut objc2_app_kit::NSView) }
    });
}
pub(super) fn source_ended() {
    ended();
    let view = SOURCE_VIEW.with(|slot| slot.borrow_mut().take());
    if let Some(view) = view {
        release_pointer_tracking(&view);
        if let Some(window) = view.window() {
            // GPUI's exit callback also clears its external-files flag. No
            // NSDraggingInfo fields are read by the exit forwarding path.
            let info = NSObject::new();
            unsafe {
                let _: () = msg_send![&*window, draggingExited: &*info];
            }
        }
    }
    let callback = SOURCE_COMPLETION.with(|slot| slot.borrow_mut().take());
    if let Some(callback) = callback {
        callback();
    }
}

pub(super) fn destination_view(window: &Window) -> Option<Retained<objc2_app_kit::NSView>> {
    let ptr = ns_view_ptr(window)?;
    // raw-window-handle identifies GPUI's view; NSWindow.contentView may be a
    // plain AppKit wrapper which does not implement GPUI's layer delegate.
    unsafe { Retained::retain(ptr.cast::<objc2_app_kit::NSView>()) }
}
pub(super) fn present_tracking_frame(view: &objc2_app_kit::NSView) {
    if view.window().is_none() {
        return;
    }
    // GPUI's display link also uses the paused main queue. Ask its existing
    // layer delegate to draw/present this one changed frame after releasing the
    // App borrow. No custom rendering loop or global view changes.
    unsafe {
        let layer: Option<Retained<AnyObject>> = msg_send![view, layer];
        if let Some(layer) = layer {
            let _: () = msg_send![view, displayLayer: &*layer];
        }
    }
}
