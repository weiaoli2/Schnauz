//! A draggable preview boundary. Persist once the drag finishes, not per frame.
use super::*;

pub(super) struct Drag {
    start_x: f32,
    start_width: f32,
    width: f32,
}

fn bounded_width(requested: f32, window_width: f32) -> f32 {
    requested.clamp(220., (window_width - 480.).clamp(220., 1200.))
}

impl Shuffle {
    pub(super) fn resize_preview(&mut self, x: f32, window_width: f32, cx: &mut Context<Self>) {
        if let Some(drag) = &mut self.preview_resize {
            drag.width = bounded_width(drag.start_width + drag.start_x - x, window_width);
            cx.notify();
        }
    }

    pub(super) fn finish_preview_resize(&mut self, cx: &mut Context<Self>) {
        if let Some(drag) = self.preview_resize.take() {
            let mut next = prefs();
            next.preview_width = drag.width.round() as u16;
            apply_prefs(next, cx);
            cx.notify();
        }
    }

    pub(super) fn render_preview_panel(
        &self,
        content: AnyElement,
        window_width: f32,
        cx: &Context<Self>,
    ) -> AnyElement {
        let width = bounded_width(
            self.preview_resize
                .as_ref()
                .map_or(prefs().preview_width as f32, |d| d.width),
            window_width,
        );
        div()
            .relative()
            .flex_none()
            .w(px(width))
            .h_full()
            .min_h_0()
            .child(content)
            .child(
                div()
                    .id("preview-resize")
                    .absolute()
                    .left_0()
                    .top_0()
                    .bottom_0()
                    .w(px(7.))
                    .cursor(CursorStyle::ResizeLeftRight)
                    .hover(|s| s.bg(Theme::alpha(theme().accent, 0x33)))
                    .when(self.preview_resize.is_some(), |s| {
                        s.bg(Theme::alpha(theme().accent, 0x55))
                    })
                    .tooltip(tip("Drag to resize preview · double-click to reset"))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                            if ev.click_count >= 2 {
                                this.preview_resize = None;
                                let mut next = prefs();
                                next.preview_width = 420;
                                apply_prefs(next, cx);
                            } else {
                                this.preview_resize = Some(Drag {
                                    start_x: f32::from(ev.position.x),
                                    start_width: width,
                                    width,
                                });
                            }
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_width_keeps_room_for_browser_and_honors_drag_limits() {
        assert_eq!(bounded_width(420., 1280.), 420.);
        assert_eq!(bounded_width(100., 1280.), 220.);
        assert_eq!(bounded_width(1000., 1280.), 800.);
        assert_eq!(bounded_width(420., 700.), 220.);
        assert_eq!(bounded_width(5000., 3000.), 1200.);
    }
}
