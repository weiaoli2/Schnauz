//! Read-only AppKit text selection, copying and scrolling inside the preview.
use super::*;
use objc2::{msg_send, rc::Retained};
use objc2_app_kit::{NSAutoresizingMaskOptions, NSColor, NSFont, NSScrollView, NSTextView, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use std::cell::Cell;

pub(super) struct Session {
    path: PathBuf,
    source: Arc<text_preview::TextPreview>,
    text: Retained<NSTextView>,
    scroll: Retained<NSScrollView>,
    clip: Retained<NSView>,
    blocked: Cell<bool>,
    active: Cell<bool>,
    colors: Cell<(u32, u32)>,
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
        text.setTextContainerInset(NSSize::new(12., 12.));
        text.setMinSize(NSSize::new(0., 0.));
        text.setMaxSize(NSSize::new(f64::MAX, f64::MAX));
        text.setVerticallyResizable(true);
        text.setHorizontallyResizable(false);
        text.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
        if let Some(container) = unsafe { text.textContainer() } {
            container.setContainerSize(NSSize::new(320., f64::MAX));
            container.setWidthTracksTextView(true);
        }
        let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), frame);
        scroll.setHasVerticalScroller(true);
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
                .gap_3()
                .p_4()
                .bg(rgb(t.sidebar))
                .border_l_1()
                .border_color(rgb(t.border))
                .child(
                    div()
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
        session.scroll.setFrameSize(NSSize::new(240., 180.));
        if session.text.string().to_string() != original
            || session.text.selectedRange().length != original.encode_utf16().count()
        {
            return Err("Resize changed text or selection".into());
        }
        Ok("Native text preview: read-only, file-name font size, Unicode selection, Copy All and resize passed".into())
    })();
    unsafe {
        let _: () = msg_send![&*board, releaseGlobally];
    }
    session.deactivate();
    result
}
