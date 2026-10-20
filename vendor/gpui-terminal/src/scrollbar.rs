//! Scrollback position and direct scrollbar navigation; never sends PTY input.
use super::*;

pub(super) struct Geometry {
    pub history: usize,
    pub offset: usize,
    pub height: f32,
    pub thumb: f32,
    pub top: f32,
}
impl TerminalView {
    pub(super) fn scrollbar_geometry(&self) -> Geometry {
        use alacritty_terminal::grid::Dimensions;
        let (history, offset, visible) = self.displayed_state().with_term(|term| {
            (
                term.total_lines() - term.screen_lines(),
                term.grid().display_offset(),
                term.screen_lines(),
            )
        });
        let height = (f32::from(self.bounds.get().size.height) - 4.).max(0.);
        let thumb = (height * visible as f32 / (history + visible).max(1) as f32)
            .max(22.)
            .min(height);
        let top = if history == 0 {
            0.
        } else {
            (height - thumb) * (1. - offset as f32 / history as f32)
        };
        Geometry {
            history,
            offset,
            height,
            thumb,
            top,
        }
    }

    pub(super) fn begin_scrollbar_drag(&mut self, y: f32, cx: &mut Context<Self>) {
        let g = self.scrollbar_geometry();
        if g.history == 0 {
            return;
        }
        let local = y - f32::from(self.bounds.get().origin.y) - 2.;
        let grab = if local >= g.top && local <= g.top + g.thumb {
            local - g.top
        } else {
            g.thumb / 2.
        };
        self.selecting = false;
        self.selection_origin = None;
        self.scrollbar_drag = Some(grab);
        self.drag_scrollbar(y, cx);
    }

    pub(super) fn drag_scrollbar(&mut self, y: f32, cx: &mut Context<Self>) {
        use alacritty_terminal::grid::Scroll;
        let Some(grab) = self.scrollbar_drag else {
            return;
        };
        let g = self.scrollbar_geometry();
        let travel = g.height - g.thumb;
        if travel <= 0. || g.history == 0 {
            return;
        }
        let top = (y - f32::from(self.bounds.get().origin.y) - 2. - grab).clamp(0., travel);
        let offset = ((1. - top / travel) * g.history as f32).round() as usize;
        let delta = offset as i64 - g.offset as i64;
        self.displayed_state().with_term_mut(|term| {
            term.scroll_display(Scroll::Delta(
                delta.clamp(i32::MIN as i64, i32::MAX as i64) as i32
            ))
        });
        cx.notify();
    }

    pub(super) fn render_scrollbar(&self, cx: &Context<Self>) -> AnyElement {
        let g = self.scrollbar_geometry();
        let owner = cx.entity().downgrade();
        div()
            .id("terminal-scrollback")
            .absolute()
            .top(px(2.))
            .bottom(px(2.))
            .right(px(1.))
            .w(px(10.))
            .bg(rgba(0xaab4cb18))
            .rounded_full()
            .cursor_default()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, event: &MouseDownEvent, _, cx| {
                    view.begin_scrollbar_drag(f32::from(event.position.y), cx);
                    cx.stop_propagation();
                }),
            )
            .child(
                div()
                    .absolute()
                    .top(px(g.top))
                    .left(px(2.))
                    .w(px(6.))
                    .h(px(g.thumb))
                    .rounded_full()
                    .bg(rgba(if g.history > 0 {
                        0xaab4cb99
                    } else {
                        0xaab4cb33
                    })),
            )
            .child(
                canvas(
                    |_, _, _| {},
                    move |_, _, window, _| {
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                            if phase == DispatchPhase::Capture {
                                let _ = owner.update(cx, |view, cx| {
                                    if view.scrollbar_drag.is_some()
                                        && event.pressed_button == Some(MouseButton::Left)
                                    {
                                        view.drag_scrollbar(f32::from(event.position.y), cx);
                                        cx.stop_propagation();
                                    }
                                });
                            }
                        });
                    },
                )
                .absolute()
                .size_full(),
            )
            .into_any_element()
    }
}
