//! Let AppKit resize the window; keep GPUI from replacing its edge cursor.
use super::*;

pub(super) fn resize_cursors(window: &Window) -> Option<AnyElement> {
    if window.is_fullscreen() {
        return None;
    }
    let width = f32::from(window.viewport_size().width);
    let height = f32::from(window.viewport_size().height);
    // Only the native inner one-point edge. These invisible cursor rectangles
    // have no mouse handlers, backgrounds, borders or drag state. Wider client
    // hit areas would advertise resizing where AppKit doesn't accept it.
    let corner = 8.;
    let mut regions = vec![
        (
            corner,
            0.,
            (width - 2. * corner).max(0.),
            1.,
            CursorStyle::ResizeUpDown,
        ),
        (
            corner,
            height - 1.,
            (width - 2. * corner).max(0.),
            1.,
            CursorStyle::ResizeUpDown,
        ),
        (
            0.,
            corner,
            1.,
            (height - 2. * corner).max(0.),
            CursorStyle::ResizeLeftRight,
        ),
        (
            width - 1.,
            corner,
            1.,
            (height - 2. * corner).max(0.),
            CursorStyle::ResizeLeftRight,
        ),
    ];
    for (left, top, cursor) in [
        (true, true, CursorStyle::ResizeUpLeftDownRight),
        (false, false, CursorStyle::ResizeUpLeftDownRight),
        (false, true, CursorStyle::ResizeUpRightDownLeft),
        (true, false, CursorStyle::ResizeUpRightDownLeft),
    ] {
        let x = if left { 0. } else { width - corner };
        let y = if top { 0. } else { height - corner };
        regions.push((x, if top { 0. } else { height - 1. }, corner, 1., cursor));
        regions.push((if left { 0. } else { width - 1. }, y, 1., corner, cursor));
    }
    Some(
        div()
            .absolute()
            .size_full()
            .children(
                regions
                    .into_iter()
                    .enumerate()
                    .map(|(index, (x, y, w, h, cursor))| {
                        div()
                            .id(("native-window-edge", index))
                            .absolute()
                            .left(px(x))
                            .top(px(y))
                            .w(px(w))
                            .h(px(h))
                            .cursor(cursor)
                    }),
            )
            .into_any_element(),
    )
}
