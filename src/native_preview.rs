//! Geometry shared by native preview views embedded below GPUI's headers.
use super::*;
use objc2_foundation::{NSPoint, NSRect, NSSize};

pub(super) fn frames(
    full: Bounds<gpui::Pixels>,
    visible: Bounds<gpui::Pixels>,
    flipped: bool,
    parent_height: f64,
) -> (NSRect, NSRect) {
    let x = f32::from(visible.origin.x) as f64;
    let y = f32::from(visible.origin.y) as f64;
    let w = f32::from(visible.size.width) as f64;
    let h = f32::from(visible.size.height) as f64;
    let outer = NSRect::new(
        NSPoint::new(x, if flipped { y } else { parent_height - y - h }),
        NSSize::new(w, h),
    );
    let inner = NSRect::new(
        NSPoint::new(
            f32::from(full.origin.x) as f64 - x,
            y + h - f32::from(full.bottom()) as f64,
        ),
        NSSize::new(
            f32::from(full.size.width) as f64,
            f32::from(full.size.height) as f64,
        ),
    );
    (outer, inner)
}

impl Schnauz {
    pub(super) fn native_preview_blocked(&self) -> bool {
        self.confirm_delete.is_some()
            || self.info_panel.is_some()
            || self.batch_rename.is_some()
            || self.ssh_ask
            || self.server_dialog.is_some()
            || self.group_dialog.is_some()
            || self.new_text_file.is_some()
            || self.project_dialog.is_some()
            || (self.palette_open && !self.file_search.inline)
    }
}
