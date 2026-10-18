//! Preview takes browser space; auxiliary tools extend the window to the right.
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

/// One expansion shared by Todoist and Disk Usage in this window.
#[derive(Default)]
pub(super) struct Expansion {
    visible: bool,
    requested: f32,
    added: f32,
}
impl Expansion {
    fn update(
        &mut self,
        visible: bool,
        requested: f32,
        current: f32,
        room: f32,
        fullscreen: bool,
    ) -> Option<f32> {
        let delta = if !visible {
            -self.added
        } else if !self.visible {
            requested
        } else {
            requested - self.requested
        };
        self.visible = visible;
        self.requested = requested;
        if fullscreen {
            return None;
        }
        let change = if delta > 0. {
            delta.min(room.max(0.))
        } else {
            delta.max(-self.added).max(980. - current)
        };
        self.added = (self.added + change).max(0.);
        if !visible {
            self.added = 0.;
        }
        (change.abs() > 0.5).then_some(current + change)
    }
}

fn bounded_width(requested: f32, available: f32) -> f32 {
    let max = available.clamp(0., 1200.);
    requested.clamp(220_f32.min(max), max)
}

fn allocate(preview: Option<f32>, tools: Option<f32>, pane_width: f32) -> Layout {
    let available = (pane_width - 220.).max(0.);
    let tools = tools.map(|w| w.clamp(220., 720.)).unwrap_or(0.);
    if let Some(preview) = preview {
        let preview = preview.clamp(220., 1200.);
        if tools > 0. && available <= tools {
            // At a screen edge or in full screen, keep both beside the files;
            // never stack them or replace the preview with the tool.
            let fraction = preview / (preview + tools);
            Layout {
                preview: available * fraction,
                tools: available * (1. - fraction),
            }
        } else {
            Layout {
                preview: bounded_width(preview, available - tools),
                tools,
            }
        }
    } else {
        Layout {
            preview: 0.,
            tools: tools.min(available),
        }
    }
}

pub(super) fn tools_width(this: &Schnauz) -> f32 {
    if this.space_map.is_none() && !this.todoist.open {
        return 0.;
    }
    this.preview_resize
        .as_ref()
        .filter(|d| d.kind == Kind::Tools)
        .map(|d| d.width)
        .unwrap_or(prefs().tools_width as f32)
}

fn layout(this: &Schnauz, pane: usize, preview: bool, tools: bool) -> Layout {
    let preview_request = if let Some(drag) = this
        .preview_resize
        .as_ref()
        .filter(|d| d.pane == pane && d.kind == Kind::Preview)
    {
        drag.width
    } else {
        this.active_tab().preview_width as f32
    };
    allocate(
        preview.then_some(preview_request),
        tools.then_some(tools_width(this)),
        this.pane_width(pane),
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
    pub(super) fn sync_tools_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let visible = self.space_map.is_some() || self.todoist.open;
        let requested = tools_width(self);
        let width = f32::from(window.viewport_size().width);
        let room = window
            .display(cx)
            .map(|display| f32::from(display.bounds().right() - window.bounds().right()))
            .unwrap_or(0.);
        if let Some(width) =
            self.tools_expansion
                .update(visible, requested, width, room, window.is_fullscreen())
        {
            window.resize(size(px(width), window.viewport_size().height));
        }
    }

    pub(super) fn resize_preview(&mut self, x: f32, cx: &mut Context<Self>) {
        let pane_width = self
            .preview_resize
            .as_ref()
            .map(|d| self.pane_width(d.pane))
            .unwrap_or(0.);
        if let Some(drag) = &mut self.preview_resize {
            drag.width = match drag.kind {
                Kind::Preview => bounded_width(
                    drag.start.preview + drag.start_x - x,
                    pane_width - 220. - drag.start.tools,
                ),
                // Tools resize from their outer right edge, growing the window.
                Kind::Tools => (drag.start.tools + x - drag.start_x).clamp(220., 720.),
            };
            cx.notify();
        }
    }

    pub(super) fn finish_preview_resize(&mut self, cx: &mut Context<Self>) {
        if let Some(drag) = self.preview_resize.take() {
            let owner = drag.owner.min(self.panes.len() - 1);
            let width = (drag.width.round() as u16).clamp(220, 1200);
            match drag.kind {
                Kind::Preview => {
                    self.tab_mut(owner).preview_width = width;
                    self.remember_folder_view(owner, cx);
                }
                Kind::Tools => {
                    let mut next = prefs();
                    next.tools_width = width.clamp(220, 720);
                    apply_prefs(next, cx);
                }
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
        // Fixed order: files → preview → Todoist / Disk Usage.
        if let Some(content) = preview {
            row = row.child(self.render_side_panel(content, Kind::Preview, sizes, pane, cx));
        }
        if let Some(content) = tools {
            row = row.child(self.render_side_panel(content, Kind::Tools, sizes, pane, cx));
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
                .min_w_0()
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
            .flex_none()
            .min_h_0()
            .min_w_0()
            .w(px(width))
            .h_full()
            .overflow_hidden()
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
                    .when(kind == Kind::Preview, |d| d.left_0())
                    .when(kind == Kind::Tools, |d| d.right_0())
                    .top_0()
                    .bottom_0()
                    .w(px(7.))
                    .cursor(CursorStyle::ResizeLeftRight)
                    .hover(|s| s.bg(Theme::alpha(t.accent, 0x33)))
                    .when(
                        self.preview_resize.as_ref().is_some_and(|d| d.kind == kind),
                        |s| s.bg(Theme::alpha(t.accent, 0x55)),
                    )
                    .tooltip(tip(if kind == Kind::Preview {
                        "Drag left edge to resize preview · double-click to reset"
                    } else {
                        "Drag right edge to resize tools · double-click to reset"
                    }))
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
    fn preview_uses_browser_width_tools_use_the_added_space() {
        let before = allocate(Some(420.), None, 1000.);
        let after = allocate(Some(420.), Some(360.), 1360.);
        assert_eq!(before.preview, after.preview);
        assert_eq!(after.tools, 360.);
        assert_eq!(1000. - before.preview, 1360. - after.preview - after.tools);
        let split_before = allocate(Some(420.), None, 370.);
        let split_after = allocate(Some(420.), Some(360.), 730.);
        assert_eq!(split_before.preview, split_after.preview);
        assert_eq!(split_after.tools, 360.);
        let tight = allocate(Some(420.), Some(360.), 500.);
        assert!(tight.preview > 0. && tight.tools > 0.);
        assert!((tight.preview + tight.tools - 280.).abs() < 0.01);
        assert_eq!(allocate(Some(640.), None, 700.).preview, 480.);
        assert_eq!(allocate(None, Some(360.), 700.).tools, 360.);
        assert_eq!(allocate(Some(420.), None, 200.).preview, 0.);
    }
    #[test]
    fn tools_expand_once_switch_without_jumping_and_close_without_undoing_manual_resize() {
        let mut state = Expansion::default();
        assert_eq!(state.update(false, 0., 1280., 1000., false), None);
        assert_eq!(state.update(true, 360., 1280., 1000., false), Some(1640.));
        // Preview selection or Todoist/Disk Usage replacement does not grow again.
        assert_eq!(state.update(true, 360., 1640., 640., false), None);
        assert_eq!(state.update(true, 500., 1640., 640., false), Some(1780.));
        assert_eq!(state.update(false, 0., 1880., 400., false), Some(1380.));
        assert_eq!(state.update(false, 0., 1380., 900., false), None);
    }
    #[test]
    fn expansion_respects_screen_edges_and_fullscreen() {
        let mut state = Expansion::default();
        assert_eq!(state.update(true, 360., 1280., 100., false), Some(1380.));
        assert_eq!(state.update(false, 0., 1380., 0., false), Some(1280.));
        assert_eq!(state.update(true, 360., 1280., 1000., true), None);
        assert_eq!(state.update(false, 0., 1280., 1000., true), None);
        assert_eq!(state.update(false, 0., 1280., 1000., false), None);
    }
}
