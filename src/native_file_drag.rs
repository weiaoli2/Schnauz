//! Per-browser AppKit window drop operations. GPUI 0.2 always reports Copy; override
//! only these owned window instances, forwarding every event to GPUI unchanged.
use super::*;
use objc2::runtime::{AnyClass, AnyObject, ClassBuilder, Sel};
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
    static CLASSES: RefCell<HashMap<usize, &'static AnyClass>> = RefCell::new(HashMap::new());
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
extern "C-unwind" fn entered(this: &AnyObject, _: Sel, info: &AnyObject) -> NSDragOperation {
    prepare(this, info);
    let parent = this
        .class()
        .superclass()
        .expect("owned drop view superclass");
    // SAFETY: same NSDraggingDestination ABI; forward to original GPUI method.
    let accepted = unsafe { objc2::msg_send![super(this, parent), draggingEntered: info] };
    advertised(accepted)
}
extern "C-unwind" fn updated(this: &AnyObject, _: Sel, info: &AnyObject) -> NSDragOperation {
    prepare(this, info);
    let parent = this
        .class()
        .superclass()
        .expect("owned drop view superclass");
    let accepted = unsafe { objc2::msg_send![super(this, parent), draggingUpdated: info] };
    advertised(accepted)
}
extern "C-unwind" fn exited(this: &AnyObject, _: Sel, info: &AnyObject) {
    FEEDBACK.with(|s| {
        let mut s = s.borrow_mut();
        if s.view == this as *const _ as usize {
            s.active = false;
        }
    });
    let parent = this
        .class()
        .superclass()
        .expect("owned drop view superclass");
    unsafe {
        let _: () = objc2::msg_send![super(this, parent), draggingExited: info];
    }
}
extern "C-unwind" fn submit(this: &AnyObject, _: Sel, info: &AnyObject) -> objc2::runtime::Bool {
    prepare(this, info); // Read Command again at release, even without movement.
    let parent = this
        .class()
        .superclass()
        .expect("owned drop view superclass");
    let result = unsafe { objc2::msg_send![super(this, parent), performDragOperation: info] };
    // GPUI does not clear external_files_dragged from its conclude callback.
    // Retire only this destination's native tracking after the delivered drop.
    unsafe {
        let _: () = objc2::msg_send![super(this, parent), draggingExited: info];
    }
    ended();
    result
}
// AppKit registers internal KVO forwarding before our per-window subclass is
// installed. Restore its original class before NSWindow tears those observers
// down; otherwise closing a browser can throw an unregistered-observer exception.
extern "C-unwind" fn dealloc_owned_window(this: &AnyObject, _: Sel) {
    let parent=this.class().superclass().expect("owned drop window superclass");
    unsafe {
        AnyObject::set_class(this,parent);
        let _:()=objc2::msg_send![super(this,parent),dealloc];
    }
}
pub(super) fn install(window: &Window) {
    let Some(ptr) = ns_view_ptr(window) else {
        return;
    };
    // GPUI registers NSDraggingDestination on its NSWindow, not its content
    // NSView. Leave the content/preview/terminal views and global classes alone.
    let content = unsafe { &*(ptr as *const objc2_app_kit::NSView) };
    let Some(native_window) = content.window() else {
        return;
    };
    // SAFETY: owned main-thread NSWindow; subclass adds no ivars and preserves
    // GPUI's exact superclass layout and all non-dragging behavior.
    let view: &AnyObject = &native_window;
    let parent = view.class();
    if parent
        .name()
        .to_string_lossy()
        .starts_with("SchnauzFileDrop_")
    {
        return;
    }
    let class = CLASSES.with(|classes| {
        *classes
            .borrow_mut()
            .entry(parent as *const _ as usize)
            .or_insert_with(|| {
                let name = std::ffi::CString::new(format!(
                    "SchnauzFileDrop_{}",
                    parent.name().to_string_lossy()
                ))
                .unwrap();
                let mut builder =
                    ClassBuilder::new(&name, parent).expect("unique browser drop class");
                unsafe {
                    builder.add_method(
                        objc2::sel!(dealloc),
                        dealloc_owned_window as extern "C-unwind" fn(_, _),
                    );
                    builder.add_method(
                        objc2::sel!(draggingEntered:),
                        entered as extern "C-unwind" fn(_, _, _) -> _,
                    );
                    builder.add_method(
                        objc2::sel!(draggingUpdated:),
                        updated as extern "C-unwind" fn(_, _, _) -> _,
                    );
                    builder.add_method(
                        objc2::sel!(draggingExited:),
                        exited as extern "C-unwind" fn(_, _, _),
                    );
                    builder.add_method(
                        objc2::sel!(performDragOperation:),
                        submit as extern "C-unwind" fn(_, _, _) -> _,
                    );
                }
                builder.register()
            })
    });
    unsafe {
        AnyObject::set_class(view, class);
    }
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
