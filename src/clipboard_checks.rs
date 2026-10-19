//! Real clipboard routing in hidden disposable windows. Restore every original
//! pasteboard item's data formats afterwards; never print clipboard contents.
use super::*;
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_app_kit::{NSPasteboard, NSPasteboardItem, NSPasteboardWriting};
use objc2_foundation::NSArray;

struct SavedClipboard(Vec<Retained<NSPasteboardItem>>);
thread_local! { static ORIGINAL: RefCell<Option<SavedClipboard>> = const {RefCell::new(None)}; }
pub(super) fn restore_on_failure() {
    ORIGINAL.with(|saved| {
        saved.borrow_mut().take();
    });
}
struct RestoreClipboard;
impl Drop for RestoreClipboard {
    fn drop(&mut self) {
        restore_on_failure();
    }
}
impl SavedClipboard {
    fn new() -> Self {
        let board = NSPasteboard::generalPasteboard();
        let mut saved = Vec::new();
        if let Some(items) = board.pasteboardItems() {
            for item in items.iter() {
                let copy = NSPasteboardItem::new();
                for kind in item.types().iter() {
                    if let Some(data) = item.dataForType(&kind) {
                        assert!(copy.setData_forType(&data, &kind));
                    }
                }
                saved.push(copy);
            }
        }
        Self(saved)
    }
}
impl Drop for SavedClipboard {
    fn drop(&mut self) {
        let board = NSPasteboard::generalPasteboard();
        board.clearContents();
        let writers: Vec<&ProtocolObject<dyn NSPasteboardWriting>> = self
            .0
            .iter()
            .map(|item| ProtocolObject::from_ref(&**item))
            .collect();
        if !writers.is_empty() {
            let _ = board.writeObjects(&NSArray::from_slice(&writers));
        }
    }
}

pub(super) fn run(window: gpui::WindowHandle<Fixture>, cx: &mut App) {
    ORIGINAL.with(|saved| *saved.borrow_mut() = Some(SavedClipboard::new()));
    let _restore = RestoreClipboard;
    let paint = |cx: &mut App| {
        for _ in 0..3 {
            cx.update_window(*window, |_, w, cx| {
                w.refresh();
                w.draw(cx).clear();
            })
            .unwrap();
        }
    };
    window
        .update(cx, |fixture, w, cx| {
            fixture.browser.update(cx, |b, cx| {
                b.focus_file_search(true, w, cx);
                b.query = "prefix Grüße 日本語 suffix".into();
                b.query_anchor = Some(7);
                b.query_cursor = 16;
            })
        })
        .unwrap();
    paint(cx);
    let key = |value: &str, cx: &mut App| {
        cx.update_window(*window, |_, w, cx| {
            w.dispatch_keystroke(gpui::Keystroke::parse(value).unwrap(), cx);
        })
        .unwrap();
        paint(cx);
    };
    key("cmd-c", cx);
    assert_eq!(
        cx.read_from_clipboard()
            .and_then(|item| item.text())
            .unwrap(),
        "Grüße 日本語"
    );
    key("cmd-x", cx);
    window
        .update(cx, |f, _, cx| {
            assert_eq!(f.browser.read(cx).query, "prefix  suffix")
        })
        .unwrap();
    key("cmd-v", cx);
    window
        .update(cx, |f, _, cx| {
            assert_eq!(f.browser.read(cx).query, "prefix Grüße 日本語 suffix")
        })
        .unwrap();
    key("cmd-a", cx);
    cx.write_to_clipboard(ClipboardItem::new_string("  Datei\nneu.txt  ".into()));
    let dir = window
        .update(cx, |f, _, cx| {
            f.browser.read(cx).active_tab().current_dir.clone()
        })
        .unwrap();
    key("cmd-v", cx);
    window
        .update(cx, |f, _, cx| {
            let b = f.browser.read(cx);
            assert_eq!(b.query, "  Datei neu.txt  ");
            assert_eq!(
                b.active_tab().current_dir,
                dir,
                "Search paste edits text, never navigates"
            );
            assert!(b.file_search.editing);
        })
        .unwrap();
    window
        .update(cx, |f, _, cx| {
            f.browser.update(cx, |b, cx| b.close_palette(cx))
        })
        .unwrap();
    println!("PASS real focused search Copy/Cut/Paste, Unicode selection, select-all replacement, single-line spaces and no path navigation");
    gpui_terminal::view::clipboard_checks::run(cx);
}
