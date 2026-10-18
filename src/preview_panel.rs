//! Independently sized file preview and auxiliary (Todoist / Disk Usage) panes.
use super::*;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Kind {
    Preview,
    Tools,
}

#[derive(Clone, Copy, Debug, Default)]
struct Layout {
    preview: f32,
    tools: f32,
    stacked: bool,
}
impl Layout {
    fn width(self, kind: Kind) -> f32 {
        match kind {
            Kind::Preview => self.preview,
            Kind::Tools => self.tools,
        }
    }
}

pub(super) struct Drag {
    pane: usize,
    owner: usize,
    kind: Kind,
    start_x: f32,
    start: Layout,
    width: f32,
}

fn bounded_width(requested: f32, available: f32) -> f32 {
    let max = available.clamp(0., 1200.);
    requested.clamp(220_f32.min(max), max)
}

fn allocate(preview: Option<f32>, tools: Option<f32>, pane_width: f32) -> Layout {
    let available = (pane_width - 220.).max(0.);
    match (preview, tools) {
        (Some(p), Some(t)) if available >= 440. => {
            let p = p.clamp(220., 1200.);
            let t = t.clamp(220., 720.);
            let total = (p + t).min(available);
            let preview = (total * p / (p + t)).clamp(220., total - 220.);
            Layout {
                preview,
                tools: total - preview,
                stacked: false,
            }
        }
        (Some(p), Some(t)) => {
            let width = bounded_width(p.max(t), available);
            Layout {
                preview: width,
                tools: width,
                stacked: true,
            }
        }
        (Some(p), None) => Layout {
            preview: bounded_width(p, available),
            ..Layout::default()
        },
        (None, Some(t)) => Layout {
            tools: bounded_width(t, available),
            ..Layout::default()
        },
        _ => Layout::default(),
    }
}

fn layout(this: &Schnauz, pane: usize, preview: bool, tools: bool) -> Layout {
    if let Some(drag) = &this.preview_resize {
        if drag.pane == pane {
            let mut current = drag.start;
            if current.stacked {
                current.preview = drag.width;
                current.tools = drag.width;
            } else {
                match drag.kind {
                    Kind::Preview => current.preview = drag.width,
                    Kind::Tools => current.tools = drag.width,
                }
            }
            return current;
        }
    }
    allocate(
        preview.then_some(this.active_tab().preview_width as f32),
        tools.then_some(prefs().tools_width as f32),
        pane_width(pane),
    )
}

pub(super) fn preview_width(this: &Schnauz, pane: usize) -> f32 {
    layout(
        this,
        pane,
        true,
        this.space_map.is_some() || this.todoist.open,
    )
    .preview
}

impl Schnauz {
    pub(super) fn resize_preview(&mut self, x: f32, cx: &mut Context<Self>) {
        if let Some(drag) = &mut self.preview_resize {
            let other = if drag.start.stacked {
                0.
            } else {
                match drag.kind {
                    Kind::Preview => drag.start.tools,
                    Kind::Tools => drag.start.preview,
                }
            };
            drag.width = bounded_width(
                drag.start.width(drag.kind) + drag.start_x - x,
                pane_width(drag.pane) - 220. - other,
            );
            if drag.kind == Kind::Tools {
                drag.width = drag.width.min(720.);
            }
            cx.notify();
        }
    }

    pub(super) fn finish_preview_resize(&mut self, cx: &mut Context<Self>) {
        if let Some(drag) = self.preview_resize.take() {
            let owner = drag.owner.min(self.panes.len() - 1);
            let width = (drag.width.round() as u16).clamp(220, 1200);
            let mut next = prefs();
            match drag.kind {
                Kind::Preview => self.tab_mut(owner).preview_width = width,
                Kind::Tools => next.tools_width = width.clamp(220, 720),
            }
            // Preserve the other pane's displayed width when both had to shrink
            // to fit. Otherwise the divider would jump on mouse-up.
            if !drag.start.stacked && drag.start.preview > 0. && drag.start.tools > 0. {
                match drag.kind {
                    Kind::Preview => {
                        next.tools_width = (drag.start.tools.round() as u16).clamp(220, 720)
                    }
                    Kind::Tools => {
                        self.tab_mut(owner).preview_width =
                            (drag.start.preview.round() as u16).clamp(220, 1200)
                    }
                }
            }
            if next.tools_width != prefs().tools_width {
                apply_prefs(next, cx);
            }
            if drag.kind == Kind::Preview || (!drag.start.stacked && drag.start.preview > 0.) {
                self.remember_folder_view(owner, cx);
            }
            cx.notify();
        }
    }

    pub(super) fn render_side_panels(
        &self,
        preview: Option<AnyElement>,
        tools: Option<AnyElement>,
        pane: usize,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        if preview.is_none() && tools.is_none() {
            return None;
        }
        let sizes = layout(self, pane, preview.is_some(), tools.is_some());
        let mut row = div().flex().flex_none().h_full().min_h_0().min_w_0();
        if sizes.stacked {
            row = row.flex_col().w(px(sizes.preview));
        }
        // The file preview stays on the far right, or above the tools in a
        // narrow split, and never disappears when a tool is opened.
        if sizes.stacked {
            if let Some(content) = preview {
                row = row.child(self.render_side_panel(content, Kind::Preview, sizes, pane, cx));
            }
            if let Some(content) = tools {
                row = row.child(self.render_side_panel(content, Kind::Tools, sizes, pane, cx));
            }
        } else {
            if let Some(content) = tools {
                row = row.child(self.render_side_panel(content, Kind::Tools, sizes, pane, cx));
            }
            if let Some(content) = preview {
                row = row.child(self.render_side_panel(content, Kind::Preview, sizes, pane, cx));
            }
        }
        Some(row.into_any_element())
    }

    fn render_side_panel(
        &self,
        content: AnyElement,
        kind: Kind,
        sizes: Layout,
        pane: usize,
        cx: &Context<Self>,
    ) -> AnyElement {
        let width = sizes.width(kind);
        let t = theme();
        let framed = if kind == Kind::Tools {
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .m_1p5()
                .border_1()
                .border_color(rgb(t.border))
                .rounded_md()
                .overflow_hidden()
                .child(content)
                .into_any_element()
        } else {
            content
        };
        div()
            .relative()
            .flex()
            .flex_col()
            .min_h_0()
            .min_w_0()
            .overflow_hidden()
            .when(sizes.stacked, |d| d.flex_1().w_full())
            .when(!sizes.stacked, |d| d.flex_none().w(px(width)).h_full())
            .bg(rgb(t.sidebar))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(framed)
            .child(
                div()
                    .id(if kind == Kind::Preview {
                        "preview-resize"
                    } else {
                        "tools-resize"
                    })
                    .absolute()
                    .left_0()
                    .top_0()
                    .bottom_0()
                    .w(px(7.))
                    .cursor(CursorStyle::ResizeLeftRight)
                    .hover(|s| s.bg(Theme::alpha(t.accent, 0x33)))
                    .when(
                        self.preview_resize.as_ref().is_some_and(|d| d.kind == kind),
                        |s| s.bg(Theme::alpha(t.accent, 0x55)),
                    )
                    .tooltip(tip("Drag to resize panel · double-click to reset"))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                            if ev.click_count >= 2 {
                                this.preview_resize = None;
                                match kind {
                                    Kind::Preview => {
                                        this.tab_mut(this.active_pane).preview_width = 420;
                                        this.remember_folder_view(this.active_pane, cx);
                                    }
                                    Kind::Tools => {
                                        let mut next = prefs();
                                        next.tools_width = 360;
                                        apply_prefs(next, cx);
                                    }
                                }
                            } else {
                                this.preview_resize = Some(Drag {
                                    pane,
                                    owner: this.active_pane,
                                    kind,
                                    start_x: f32::from(ev.position.x),
                                    start: sizes,
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
    fn panes_keep_browser_space_and_remain_independent() {
        let wide = allocate(Some(420.), Some(360.), 1280.);
        assert_eq!(
            (wide.preview, wide.tools, wide.stacked),
            (420., 360., false)
        );
        let small = allocate(Some(420.), Some(360.), 760.);
        assert!(small.preview >= 220. && small.tools >= 220.);
        assert_eq!(small.preview + small.tools, 540.);
        let split = allocate(Some(420.), Some(360.), 500.);
        assert_eq!(
            (split.preview, split.tools, split.stacked),
            (280., 280., true)
        );
        assert_eq!(allocate(Some(640.), None, 700.).preview, 480.);
        assert_eq!(allocate(None, Some(360.), 700.).tools, 360.);
        assert_eq!(allocate(Some(420.), None, 200.).preview, 0.);
        assert_eq!(bounded_width(5000., 3000.), 1200.);
    }
}
