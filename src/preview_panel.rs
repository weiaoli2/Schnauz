//! A draggable preview boundary. Persist once the drag finishes, not per frame.
use super::*;

pub(super) struct Drag {
    pane: usize,
    owner: usize,
    start_x: f32,
    start_width: f32,
    width: f32,
}

fn bounded_width(requested: f32, pane_width: f32) -> f32 {
    // Keep room for files even in a narrow split. The preferred width is
    // retained so the preview grows back when more space becomes available.
    let max = (pane_width - 220.).clamp(0., 1200.);
    requested.clamp(220_f32.min(max), max)
}

pub(super) fn preview_width(this: &Schnauz, pane: usize) -> f32 {
    bounded_width(
        this.preview_resize
            .as_ref()
            .map_or(this.active_tab().preview_width as f32, |d| d.width),
        pane_width(pane),
    )
}

impl Schnauz {
    pub(super) fn resize_preview(&mut self, x: f32, cx: &mut Context<Self>) {
        if let Some(drag) = &mut self.preview_resize {
            drag.width = bounded_width(drag.start_width + drag.start_x - x, pane_width(drag.pane));
            cx.notify();
        }
    }

    pub(super) fn finish_preview_resize(&mut self, cx: &mut Context<Self>) {
        if let Some(drag) = self.preview_resize.take() {
            let owner = drag.owner.min(self.panes.len() - 1);
            self.tab_mut(owner).preview_width = (drag.width.round() as u16).clamp(220, 1200);
            self.remember_folder_view(owner, cx);
            cx.notify();
        }
    }

    pub(super) fn render_preview_panel(
        &self,
        content: AnyElement,
        pane: usize,
        cx: &Context<Self>,
    ) -> AnyElement {
        let width = preview_width(self, pane);
        div()
            .relative()
            .flex_none()
            .w(px(width))
            .h_full()
            .min_h_0()
            .overflow_hidden()
            // The preview can show a selection from the other split pane.
            // Interacting with it must not activate this pane and replace it.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
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
                    .tooltip(tip("Drag to resize panel · double-click to reset"))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                            if ev.click_count >= 2 {
                                this.preview_resize = None;
                                this.tab_mut(this.active_pane).preview_width = 420;
                                this.remember_folder_view(this.active_pane, cx);
                            } else {
                                this.preview_resize = Some(Drag {
                                    pane,
                                    owner: this.active_pane,
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
        assert_eq!(bounded_width(1200., 1280.), 1060.);
        assert_eq!(bounded_width(420., 700.), 420.);
        assert_eq!(bounded_width(420., 500.), 280.);
        assert_eq!(bounded_width(420., 350.), 130.);
        assert_eq!(bounded_width(420., 200.), 0.);
        assert_eq!(bounded_width(5000., 3000.), 1200.);
    }
}
