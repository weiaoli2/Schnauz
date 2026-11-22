//! Persistent file-pane scrollbars with exclusive pointer ownership.
use super::*;

impl Schnauz {
    fn file_scroll_handle(&self, pane: usize) -> UniformListScrollHandle {
        if self.search_in_pane(pane) {
            self.file_search.scroll.clone()
        } else {
            self.tab(pane).scroll_handle.clone()
        }
    }

    pub(super) fn begin_scroll_drag(&mut self, pane: usize, y: f32) {
        self.active_pane = pane;
        self.pending_click_rename = None;
        self.drag_candidate = None;
        self.marquee = None;
        self.hbar_drag = None;
        self.scroll_drag = None;
        self.suppress_entry_click = true;
        let handle = self.file_scroll_handle(pane);
        let state = handle.0.borrow();
        let base = &state.base_handle;
        let viewport = f32::from(base.bounds().size.height);
        let max = f32::from(base.max_offset().height);
        if viewport <= 1. || max <= 1. {
            return;
        }
        let thumb = thumb_height(viewport, max);
        let travel = (viewport - thumb).max(1.);
        let mut scrolled = (-f32::from(base.offset().y)).clamp(0., max);
        let local = y - f32::from(base.bounds().top());
        let top = travel * scrolled / max;
        if local < top || local > top + thumb {
            scrolled = ((local - thumb / 2.).clamp(0., travel) / travel) * max;
            base.set_offset(point(base.offset().x, px(-scrolled)));
        }
        drop(state);
        self.scroll_drag = Some(ScrollDrag {
            pane,
            tab: self.tab(pane).id,
            search: self.search_in_pane(pane).then_some(self.search_gen),
            handle,
            start_y: y,
            start_scrolled: scrolled,
        });
    }

    pub(super) fn update_scroll_drag(&mut self, y: f32, cx: &mut Context<Self>) {
        let Some(drag) = self.scroll_drag.clone() else {
            return;
        };
        if drag.pane >= self.panes.len()
            || self.tab(drag.pane).id != drag.tab
            || self.search_in_pane(drag.pane).then_some(self.search_gen) != drag.search
        {
            self.scroll_drag = None;
            cx.notify();
            return;
        }
        let state = drag.handle.0.borrow();
        let base = &state.base_handle;
        let viewport = f32::from(base.bounds().size.height);
        let max = f32::from(base.max_offset().height);
        if viewport <= 1. || max <= 1. {
            return;
        }
        let travel = (viewport - thumb_height(viewport, max)).max(1.);
        let scrolled = (drag.start_scrolled + (y - drag.start_y) * max / travel).clamp(0., max);
        base.set_offset(point(base.offset().x, px(-scrolled)));
        drop(state);
        cx.notify();
    }

    pub(super) fn end_scroll_drag(&mut self, cx: &mut Context<Self>) {
        if self.scroll_drag.take().is_some() {
            cx.notify();
        }
    }

    /// The rail exists even before measurement or when all items fit. Insets
    /// keep it below the column header and above the horizontal scrollbar.
    pub(super) fn scrollbar_thumb(
        &self,
        pane: usize,
        top: f32,
        bottom: f32,
        cx: &Context<Self>,
    ) -> AnyElement {
        let handle = self.file_scroll_handle(pane);
        let state = handle.0.borrow();
        let base = &state.base_handle;
        let viewport = f32::from(base.bounds().size.height).max(0.);
        let max = f32::from(base.max_offset().height).max(0.);
        let enabled = viewport > 1. && max > 1.;
        let height = thumb_height(viewport, max);
        let scrolled = (-f32::from(base.offset().y)).clamp(0., max);
        let thumb_top = if enabled {
            (viewport - height) * scrolled / max
        } else {
            0.
        };
        let dragging = self
            .scroll_drag
            .as_ref()
            .is_some_and(|drag| drag.pane == pane);
        div()
            .id(("file-scrollbar", pane))
            .absolute()
            .right_0()
            .top(px(top))
            .bottom(px(bottom))
            .w(px(12.))
            .bg(rgb(theme().surface))
            .occlude()
            .cursor_default()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    window.focus(&this.focus);
                    this.begin_scroll_drag(pane, f32::from(event.position.y));
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.end_scroll_drag(cx);
                    cx.stop_propagation();
                }),
            )
            .on_click(|_, _, cx| cx.stop_propagation())
            .on_scroll_wheel(
                cx.listener(move |this, event: &ScrollWheelEvent, window, cx| {
                    let handle = this.file_scroll_handle(pane);
                    let state = handle.0.borrow();
                    let base = &state.base_handle;
                    let offset = base.offset();
                    let dy = event.delta.pixel_delta(window.line_height()).y;
                    base.set_offset(point(
                        offset.x,
                        px((f32::from(offset.y + dy))
                            .clamp(-f32::from(base.max_offset().height).max(0.), 0.)),
                    ));
                    this.pending_click_rename = None;
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .child(
                div()
                    .absolute()
                    .right(px(2.))
                    .top(px(thumb_top))
                    .w(px(8.))
                    .rounded_full()
                    .when(viewport > 1., |thumb| thumb.h(px(height)))
                    .when(viewport <= 1., |thumb| thumb.h_full())
                    .bg(Theme::alpha(
                        theme().text,
                        if dragging {
                            0x66
                        } else if enabled {
                            0x44
                        } else {
                            0x18
                        },
                    ))
                    .when(enabled, |thumb| {
                        thumb.hover(|s| s.bg(Theme::alpha(theme().text, 0x66)))
                    }),
            )
            .into_any_element()
    }
}

fn thumb_height(viewport: f32, max: f32) -> f32 {
    if viewport <= 1. {
        return viewport;
    }
    (viewport * viewport / (viewport + max)).clamp(28f32.min(viewport), viewport)
}
