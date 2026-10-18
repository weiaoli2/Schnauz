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
    ended();
    result
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
