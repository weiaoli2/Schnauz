//! Read-only AppKit text selection, copying and scrolling inside the preview.
use super::*;
use objc2::{define_class, msg_send, rc::Retained, DeclaredClass};
use objc2_app_kit::{NSAutoresizingMaskOptions, NSColor, NSFont, NSScrollView, NSTextView, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use std::cell::Cell;

pub(super) struct Session {
    path: PathBuf,
    source: Arc<text_preview::TextPreview>,
    text: Retained<NSTextView>,
    scroll: Retained<ZoomScrollView>,
    clip: Retained<NSView>,
    blocked: Cell<bool>,
    active: Cell<bool>,
    colors: Cell<(u32, u32)>,
}

#[derive(Default)]
struct ZoomState {
    scale: Cell<f32>,
}
define_class!(
    #[unsafe(super(NSScrollView))]
    #[thread_kind = MainThreadOnly]
    #[name = "SchnauzPreviewZoomScrollView"]
    #[ivars = ZoomState]
    struct ZoomScrollView;
    impl ZoomScrollView {
        #[unsafe(method(scrollWheel:))]
        fn wheel(&self, event: &objc2_app_kit::NSEvent) {
            if !event.modifierFlags().contains(objc2_app_kit::NSEventModifierFlags::Shift) {
                let clip=self.contentView();
                let mut bounds=clip.bounds();
                let step=if event.hasPreciseScrollingDeltas() {1.} else {self.verticalLineScroll()};
                bounds.origin.y-=event.scrollingDeltaY()*step;
                let bounds=clip.constrainBoundsRect(bounds);
                clip.scrollToPoint(bounds.origin);
                self.reflectScrolledClipView(&clip);
                return;
            }
            // AppKit may remap Shift-wheel motion onto the horizontal axis.
            let delta = if event.scrollingDeltaY() != 0. {event.scrollingDeltaY()} else {event.scrollingDeltaX()};
            let delta = if event.hasPreciseScrollingDeltas() {
                delta
            } else { delta * 20. };
            self.zoom(delta as f32);
        }
    }
);
impl ZoomScrollView {
    fn zoom(&self, delta: f32) {
        let scale = native_preview::zoom_step(self.ivars().scale.get(), delta);
        self.ivars().scale.set(scale);
        if let Some(view) = self.documentView() {
            // This private scroll view always owns our read-only NSTextView.
            let text = unsafe { &*(&*view as *const NSView).cast::<NSTextView>() };
            let size = UI_FONT_SIZE as f64 * scale as f64;
            self.setVerticalLineScroll(size * 1.2);
            let font = NSFont::fontWithName_size(&NSString::from_str("Menlo"), size)
                .unwrap_or_else(|| NSFont::monospacedSystemFontOfSize_weight(size, 0.));
            text.setFont(Some(&font));
        }
    }
}

fn color(rgb: u32) -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(
        ((rgb >> 16) & 255) as f64 / 255.,
        ((rgb >> 8) & 255) as f64 / 255.,
        (rgb & 255) as f64 / 255.,
        1.,
    )
}

impl Session {
    fn new(path: PathBuf, source: Arc<text_preview::TextPreview>) -> Option<Rc<Self>> {
        let mtm = objc2::MainThreadMarker::new()?;
        let frame = NSRect::new(NSPoint::new(0., 0.), NSSize::new(320., 240.));
        let text = NSTextView::initWithFrame(NSTextView::alloc(mtm), frame);
        text.setRichText(false);
        text.setAllowsUndo(false);
        text.setString(&NSString::from_str(&source.text));
        text.setEditable(false);
        text.setSelectable(true);
        let font = NSFont::fontWithName_size(&NSString::from_str("Menlo"), UI_FONT_SIZE as f64)
            .unwrap_or_else(|| NSFont::monospacedSystemFontOfSize_weight(UI_FONT_SIZE as f64, 0.));
        text.setFont(Some(&font));
        text.setTextContainerInset(NSSize::new(2., 2.));
        text.setMinSize(NSSize::new(0., 0.));
        text.setMaxSize(NSSize::new(f64::MAX, f64::MAX));
        text.setVerticallyResizable(true);
        text.setHorizontallyResizable(false);
        text.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
        if let Some(container) = unsafe { text.textContainer() } {
            container.setLineFragmentPadding(0.);
            container.setContainerSize(NSSize::new(320., f64::MAX));
            container.setWidthTracksTextView(true);
        }
        let allocated = ZoomScrollView::alloc(mtm).set_ivars(ZoomState {
            scale: Cell::new(1.),
        });
        let scroll: Retained<ZoomScrollView> =
            unsafe { msg_send![super(allocated), initWithFrame: frame] };
        scroll.setHasVerticalScroller(true);
        scroll.setVerticalLineScroll(UI_FONT_SIZE as f64 * 1.2);
        scroll.setHasHorizontalScroller(false);
        scroll.setAutohidesScrollers(true);
        scroll.setDocumentView(Some(&text));
        let clip = NSView::initWithFrame(NSView::alloc(mtm), frame);
        clip.setWantsLayer(true);
        unsafe {
            let layer: Option<Retained<objc2::runtime::AnyObject>> = msg_send![&*clip, layer];
            if let Some(layer) = layer {
                let _: () = msg_send![&*layer, setMasksToBounds: true];
            }
        }
        clip.addSubview(&scroll);
        clip.setHidden(true);
        let session = Rc::new(Self {
            path,
            source,
            text,
            scroll,
            clip,
            blocked: Cell::new(false),
            active: Cell::new(true),
            colors: Cell::new((u32::MAX, u32::MAX)),
        });
        session.update_colors();
        Some(session)
    }

    fn update_colors(&self) {
        let t = theme();
        if self.colors.replace((t.bg, t.text)) != (t.bg, t.text) {
            let background = color(t.bg);
            self.text.setBackgroundColor(&background);
            self.scroll.setBackgroundColor(&background);
            self.text.setTextColor(Some(&color(t.text)));
        }
    }

    fn focused(&self) -> bool {
        self.text
            .window()
            .and_then(|w| w.firstResponder())
            .is_some_and(|r| r.isEqual(Some(&self.text)))
    }

    pub(super) fn release_focus(&self) {
        if self.focused() {
            if let Some(window) = self.text.window() {
                let parent = unsafe { self.clip.superview() };
                let responder = parent.as_ref().map(|v| &**v as &objc2_app_kit::NSResponder);
                window.makeFirstResponder(responder);
            }
        }
    }

    fn deactivate(&self) {
        self.release_focus();
        self.active.set(false);
        self.clip.setHidden(true);
        self.clip.removeFromSuperview();
    }

    fn block(&self, blocked: bool) {
        self.blocked.set(blocked);
        if blocked {
            self.release_focus();
            self.clip.setHidden(true);
        }
    }

    fn paint(&self, bounds: Bounds<gpui::Pixels>, window: &mut Window) {
        if !self.active.get() || self.blocked.get() {
            self.clip.setHidden(true);
            return;
        }
        let visible = bounds.intersect(&window.content_mask().bounds);
        if f32::from(visible.size.width) <= 1. || f32::from(visible.size.height) <= 1. {
            self.clip.setHidden(true);
            return;
        }
        let Some(ptr) = ns_view_ptr(window) else {
            return;
        };
        let parent = unsafe { &*ptr.cast::<NSView>() };
        let (outer, inner) = native_preview::frames(
            bounds,
            visible,
            parent.isFlipped(),
            parent.bounds().size.height,
        );
        if unsafe { self.clip.superview() }.is_none() {
            parent.addSubview(&self.clip);
        }
        self.clip.setFrame(outer);
        self.scroll.setFrame(inner);
        self.clip.setHidden(false);
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.deactivate();
    }
}

impl Schnauz {
    pub(super) fn sync_text_preview(&mut self) {
        let tab = self.active_tab();
        let wanted = (prefs().preview && tab.selection.len() == 1)
            .then(|| tab.selection.iter().next().cloned())
            .flatten()
            .and_then(|path| {
                TEXT_PREVIEW_CACHE
                    .with(|cache| cache.borrow().get(&path).cloned().flatten())
                    .map(|text| (path, text))
            });
        let same = match (&self.text_view, &wanted) {
            (Some(current), Some((path, text))) => {
                current.path == *path && Arc::ptr_eq(&current.source, text)
            }
            (None, None) => true,
            _ => false,
        };
        if !same {
            if let Some(old) = self.text_view.take() {
                old.deactivate();
            }
            self.text_view = wanted.and_then(|(path, text)| Session::new(path, text));
        }
        if let Some(session) = &self.text_view {
            session.update_colors();
            session.block(self.native_preview_blocked());
        }
    }

    // GPUI handles key equivalents on its parent NSView before AppKit reaches
    // the child. Route text commands to the native first responder explicitly.
    pub(super) fn text_preview_key(&self, ev: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let Some(session) = self.text_view.as_ref().filter(|s| s.focused()) else {
            return false;
        };
        if ev.keystroke.modifiers.platform && !ev.keystroke.modifiers.shift {
            unsafe {
                match ev.keystroke.key.as_str() {
                    "c" => session.text.copy(None),
                    "a" => session.text.selectAll(None),
                    // A read-only preview must never fall through to file cut/paste.
                    "x" | "v" | "z" => {}
                    _ => {
                        session.release_focus();
                        return false;
                    }
                }
            }
            cx.stop_propagation();
            return true;
        }
        false
    }

    pub(super) fn render_text_preview(&self) -> Option<AnyElement> {
        let session = self.text_view.clone()?;
        let t = theme();
        Some(
            div()
                .id("text-inspector")
                .w_full()
                .h_full()
                .min_h_0()
                .flex()
                .flex_col()
                .gap(px(6.))
                .p(px(6.))
                .relative()
                .children(ui_checks::probe("preview-text-panel"))
                .bg(rgb(t.sidebar))
                .border_l_1()
                .border_color(rgb(t.border))
                .child(
                    div()
                        .relative()
                        .children(ui_checks::probe("preview-text-title"))
                        .flex_none()
                        .font_weight(FontWeight::SEMIBOLD)
                        .truncate()
                        .child(path_label(&session.path)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .relative()
                        .children(ui_checks::probe("preview-text-viewport"))
                        .rounded_md()
                        .overflow_hidden()
                        .bg(rgb(t.bg))
                        .child(
                            gpui::canvas(|bounds, _, _| bounds, {
                                let s = session.clone();
                                move |_, bounds, window, _| s.paint(bounds, window)
                            })
                            .size_full(),
                        ),
                )
                .when(session.source.truncated, |d| {
                    d.child(
                        div()
                            .flex_none()
                            .text_size(px(UI_SMALL_FONT_SIZE))
                            .text_color(rgb(t.text_muted))
                            .child("Preview shortened for this large file."),
                    )
                })
                .into_any_element(),
        )
    }
}

/// Native regression check: a private pasteboard leaves the user's clipboard intact.
pub(super) fn smoke_test() -> Result<String, String> {
    use objc2_app_kit::{NSApplication, NSPasteboard};
    use objc2_foundation::NSRange;
    let mtm = objc2::MainThreadMarker::new().ok_or("Text preview requires the main thread")?;
    let _ = NSApplication::sharedApplication(mtm);
    let original = "Cookie 🐕\nGrüße — 日本語\nLast line";
    let session = Session::new(
        PathBuf::from("fixture.txt"),
        Arc::new(text_preview::TextPreview {
            text: original.into(),
            truncated: false,
        }),
    )
    .ok_or("Cannot create text preview")?;
    if session.text.isEditable() || !session.text.isSelectable() {
        return Err("Preview is not read-only and selectable".into());
    }
    let font = session.text.font().ok_or("Missing preview font")?;
    if font.pointSize() != UI_FONT_SIZE as f64 {
        return Err("Preview font differs from file names".into());
    }
    if session.text.textContainerInset() != NSSize::new(2., 2.)
        || unsafe { session.text.textContainer() }.is_none_or(|c| c.lineFragmentPadding()!=0.) {
        return Err("Native text preview must use two-point insets without extra line padding".into());
    }
    let board = NSPasteboard::pasteboardWithUniqueName();
    let result = (|| {
        let start = "Cookie ".encode_utf16().count();
        session
            .text
            .setSelectedRange(NSRange::new(start, "🐕".encode_utf16().count()));
        let types = session.text.writablePasteboardTypes();
        let text_type = types
            .firstObject()
            .ok_or("Native preview has no copyable text type")?;
        unsafe {
            board.declareTypes_owner(&types, None);
        }
        if !session
            .text
            .writeSelectionToPasteboard_type(&board, &text_type)
        {
            return Err("Could not copy Unicode selection".into());
        }
        if board
            .stringForType(&text_type)
            .map(|s| s.to_string())
            .as_deref()
            != Some("🐕")
        {
            return Err("Unicode selection was corrupted".into());
        }
        unsafe {
            session.text.selectAll(None);
        }
        if session.text.selectedRange().length != original.encode_utf16().count() {
            return Err("Select All did not select all lines".into());
        }
        unsafe {
            board.declareTypes_owner(&types, None);
        }
        if !session
            .text
            .writeSelectionToPasteboard_type(&board, &text_type)
            || board
                .stringForType(&text_type)
                .map(|s| s.to_string())
                .as_deref()
                != Some(original)
        {
            return Err("Copy All did not preserve text".into());
        }
        session.scroll.zoom(20.);
        if session.text.font().unwrap().pointSize() <= UI_FONT_SIZE as f64
            || session.text.selectedRange().length != original.encode_utf16().count()
        {
            return Err("Wheel zoom failed or changed text selection".into());
        }
        session.scroll.zoom(-20.);
        if (session.text.font().unwrap().pointSize() - UI_FONT_SIZE as f64).abs() > 0.001 {
            return Err("Zoom out did not restore text size".into());
        }
        session.scroll.setFrameSize(NSSize::new(240., 180.));
        if session.text.string().to_string() != original
            || session.text.selectedRange().length != original.encode_utf16().count()
        {
            return Err("Resize changed text or selection".into());
        }
        // Dispatch real native scroll events to our own view only, never to
        // the system event queue or a user's window. No accessibility access.
        #[repr(C)]
        struct CGEvent {
            _opaque: [u8; 0],
        }
        unsafe impl objc2::RefEncode for CGEvent {
            const ENCODING_REF: objc2::Encoding =
                objc2::Encoding::Pointer(&objc2::Encoding::Struct("__CGEvent", &[]));
        }
        #[link(name = "CoreGraphics", kind = "framework")]
        unsafe extern "C" {
            fn CGEventCreateScrollWheelEvent(
                source: *const std::ffi::c_void,
                units: u32,
                count: u32,
                first_wheel: i32,
                ...
            ) -> *mut CGEvent;
            fn CGEventSetFlags(event: *mut CGEvent, flags: u64);
        }
        #[link(name = "CoreFoundation", kind = "framework")]
        unsafe extern "C" {
            fn CFRelease(value: *const std::ffi::c_void);
        }
        let wheel = |shift: bool, delta: i32| -> Result<Retained<objc2_app_kit::NSEvent>, String> {
            unsafe {
                let raw = CGEventCreateScrollWheelEvent(std::ptr::null(), 1, 1, delta);
                if raw.is_null() {
                    return Err("Cannot create synthetic wheel event".into());
                }
                CGEventSetFlags(
                    raw,
                    if shift {
                        objc2_app_kit::NSEventModifierFlags::Shift.bits() as u64
                    } else {
                        0
                    },
                );
                let event: Option<Retained<objc2_app_kit::NSEvent>> =
                    msg_send![objc2::class!(NSEvent),eventWithCGEvent:raw];
                CFRelease(raw.cast());
                event.ok_or("Cannot wrap synthetic wheel event".into())
            }
        };
        let long = Session::new(
            PathBuf::from("long-fixture.txt"),
            Arc::new(text_preview::TextPreview {
                text: (0..200).map(|i| format!("Line {i}\n")).collect(),
                truncated: false,
            }),
        )
        .ok_or("Cannot create scroll fixture")?;
        long.scroll.setFrameSize(NSSize::new(240., 120.));
        long.text.setFrameSize(NSSize::new(240., 4000.));
        long.scroll
            .contentView()
            .scrollToPoint(NSPoint::new(0., 200.));
        let before = long.scroll.contentView().bounds().origin.y;
        let ordinary = wheel(false, -3)?;
        long.scroll.scrollWheel(&ordinary);
        if long.text.font().unwrap().pointSize() != UI_FONT_SIZE as f64 {
            return Err("Ordinary wheel changed text size".into());
        }
        if long.scroll.contentView().bounds().origin.y == before {
            return Err(format!("Ordinary wheel failed to scroll native text: delta {}, scrolling delta {}, doc {:?}, clip {:?}",ordinary.deltaY(),ordinary.scrollingDeltaY(),long.text.frame(),long.scroll.contentView().bounds()));
        }
        long.scroll.scrollWheel(&*wheel(true, 1)?);
        if long.text.font().unwrap().pointSize() <= UI_FONT_SIZE as f64 {
            return Err("Shift-wheel failed to zoom native text".into());
        }
        Ok("Native text preview: read-only, file-name font size, Unicode selection, Copy All, Shift-wheel zoom and resize passed".into())
    })();
    unsafe {
        let _: () = msg_send![&*board, releaseGlobally];
    }
    session.deactivate();
    result
}


/// The real watcher -> async cache -> attached native text view path, without an editor app.
#[cfg(feature = "ui-checks")]
pub(super) fn check_external_edits(
    window: gpui::WindowHandle<ui_checks::Fixture>, cx: &mut App,
    done: impl FnOnce(&mut App) + 'static,
) {
    let root = config_dir().unwrap().join("external-preview-edit");
    fs::create_dir_all(&root).unwrap();
    let path = root.join("notes.txt");
    fs::write(&path, "old text").unwrap();
    let (pane, old_active) = window.update(cx, |f, _, cx| f.browser.update(cx, |b, cx| {
        let pane = b.active_pane;
        let old_active = b.panes[pane].active;
        b.panes[pane].tabs.push(Tab::new(root.clone()));
        b.panes[pane].active = b.panes[pane].tabs.len() - 1;
        b.select_entry(pane, path.clone(), cx);
        (pane, old_active)
    })).unwrap();
    cx.spawn(async move |cx| {
        for (stage, expected) in ["old text", "new text", "atomic replacement", "final rapid edit"].into_iter().enumerate() {
            match stage {
                1 => fs::write(&path, expected).unwrap(), // same-length, in-place save
                2 => {
                    let replacement = root.join("save.tmp");
                    fs::write(&replacement, expected).unwrap();
                    fs::rename(replacement, &path).unwrap();
                },
                3 => {
                    fs::write(&path, "intermediate edit").unwrap();
                    fs::write(&path, expected).unwrap();
                },
                _ => {},
            }
            let deadline = Instant::now() + Duration::from_secs(8);
            loop {
                cx.background_executor().timer(Duration::from_millis(50)).await;
                let matched = cx.update(|cx| {
                    cx.update_window(*window, |_, w, cx| { w.refresh(); w.draw(cx).clear(); }).unwrap();
                    window.update(cx, |f, _, cx| {
                        let b = f.browser.read(cx);
                        b.text_view.as_ref().is_some_and(|session| session.path == path
                            && session.text.string().to_string() == expected)
                    }).unwrap()
                }).unwrap();
                if matched { break; }
                assert!(Instant::now() < deadline, "External edit stage {stage} never reached native NSTextView: {expected}");
            }
        }
        let source = cx.update(|cx| window.update(cx, |f, _, cx| {
            f.browser.read(cx).text_view.as_ref().unwrap().source.clone()
        }).unwrap()).unwrap();
        // Unrelated changes must not replace the native session/selection.
        fs::write(root.join("unrelated.txt"), "unrelated").unwrap();
        cx.background_executor().timer(Duration::from_millis(1400)).await;
        cx.update(|cx| {
            window.update(cx, |f, _, cx| f.browser.update(cx, |b, cx| {
                b.sync_text_preview();
                assert!(Arc::ptr_eq(&source, &b.text_view.as_ref().unwrap().source));
                assert_eq!(b.text_view.as_ref().unwrap().text.string().to_string(), "final rapid edit");
                b.panes[pane].tabs.pop();
                b.panes[pane].active = old_active;
                b.saved_session = b.session_string();
                cx.notify();
            })).unwrap();
            println!("PASS actual selected text preview refresh after same-length in-place save, atomic replacement and rapid edits; unchanged native text after unrelated folder change; no editor launched");
            done(cx);
        }).unwrap();
    }).detach();
}
