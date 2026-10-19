//! Geometry shared by native preview views embedded below GPUI's headers.
use super::*;
use gpui::{DevicePixels, Pixels, Point, Size};
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
        self.file_app_choice.is_some()
            || self.tab_project_move.is_some()
            || self.file_access_intro.is_some()
            || self.confirm_delete.is_some()
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

/// Transient per-window zoom: changing the selected file or tab starts fresh.
pub(super) struct Zoom {
    target: Option<(u64, PathBuf)>,
    pub scale: f32,
    pub scroll: ScrollHandle,
    pub pan: Option<(Point<Pixels>, Point<Pixels>)>,
}
impl Default for Zoom {
    fn default() -> Self {
        Self {
            target: None,
            scale: 1.,
            scroll: ScrollHandle::new(),
            pan: None,
        }
    }
}
pub(super) fn zoom_step(scale: f32, delta: f32) -> f32 {
    (scale * (delta.clamp(-120., 120.) * 0.01).exp()).clamp(0.25, 8.)
}
pub(super) fn image_size(
    size: Size<DevicePixels>,
    available: f32,
    picture: bool,
    zoom: f32,
) -> Size<Pixels> {
    let aspect = size.height.0.max(1) as f32 / size.width.0.max(1) as f32;
    let width = available.max(1.);
    let width = if picture {
        width
    } else {
        width.min(600. / aspect)
    } * zoom;
    gpui::size(px(width), px(width * aspect))
}
impl Schnauz {
    pub(super) fn zoom_visual_preview(&mut self, delta: f32, cx: &mut Context<Self>) {
        self.preview_zoom.scale = zoom_step(self.preview_zoom.scale, delta);
        cx.notify();
    }
    pub(super) fn sync_preview_zoom(&mut self) {
        let tab = self.active_tab();
        let target = (prefs().preview && tab.selection.len() == 1)
            .then(|| (tab.id, tab.selection.iter().next().unwrap().clone()));
        if self.preview_zoom.target != target {
            self.preview_zoom = Zoom {
                target,
                ..Zoom::default()
            };
        }
    }
}

/// Two label/value pairs per row; date values contain two explicit compact lines.
pub(super) fn information(path: &Path, video_dimensions: Option<String>, width: f32) -> AnyElement {
    let t = theme();
    let content_width = (width - 24.).max(1.);
    let pair_width = ((content_width - 8.) / 2.).max(1.);
    let value_width = (pair_width - 64.).max(1.);
    let mut panel = div()
        .relative()
        .flex_none()
        .w(px(width))
        .min_w_0()
        .px(px(12.))
        .pb(px(8.))
        .flex()
        .flex_col()
        .gap(px(2.))
        .children(ui_checks::probe("preview-information"))
        .child(settings_title("Information", content_width));
    let Some(info) = lookup_info(path) else {
        return panel
            .child(div().text_size(px(UI_SMALL_FONT_SIZE)).child("Loading…"))
            .into_any_element();
    };
    let [created, modified, accessed] = info.date_times;
    let mut fields = vec![
        ("Kind", vec![info.kind]),
        ("Size", vec![info.size]),
        ("Created", vec![created.0, created.1]),
        ("Modified", vec![modified.0, modified.1]),
        ("Last opened", vec![accessed.0, accessed.1]),
    ];
    if let Some(value) = video_dimensions.or(info.dimensions) {
        fields.push(("Dimensions", vec![value]));
    }
    if let Some(value) = info.color {
        fields.push(("Color", vec![value]));
    }
    if let Some(value) = info.signed {
        fields.push(("Signature", vec![value]));
    }
    for (row, fields) in fields.chunks(2).enumerate() {
        let mut line = div()
            .relative()
            .flex_none()
            .flex()
            .w(px(content_width))
            .min_w_0()
            .gap(px(8.))
            .when(row == 0, |d| {
                d.children(ui_checks::probe("preview-info-first-row"))
            });
        for (column, (label, values)) in fields.iter().enumerate() {
            let mut value = div()
                .id(SharedString::from(format!("preview-info-{row}-{column}")))
                .relative()
                .flex_none()
                .w(px(value_width))
                .min_w_0()
                .flex()
                .flex_col()
                .tooltip(tip(values.join("\n")))
                .when(row == 0, |d| {
                    d.children(ui_checks::probe(if column == 0 {
                        "preview-info-value-1"
                    } else {
                        "preview-info-value-2"
                    }))
                });
            for (index, text) in values.iter().enumerate() {
                value = value.child(
                    div()
                        .relative()
                        .w(px(value_width))
                        .min_w_0()
                        .flex_none()
                        .flex()
                        .h(px(13.))
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .when(*label == "Created", |d| {
                            d.children(ui_checks::probe(if index == 0 {
                                "preview-created-date"
                            } else {
                                "preview-created-time"
                            }))
                        })
                        .child(ui_checks::text(
                            match (*label, index) {
                                ("Kind", _) => "preview-kind-text",
                                ("Size", _) => "preview-size-text",
                                ("Created", 0) => "preview-created-date-text",
                                ("Created", _) => "preview-created-time-text",
                                ("Modified", 0) => "preview-modified-date-text",
                                ("Modified", _) => "preview-modified-time-text",
                                ("Last opened", 0) => "preview-accessed-date-text",
                                ("Last opened", _) => "preview-accessed-time-text",
                                ("Dimensions", _) => "preview-dimensions-text",
                                ("Color", _) => "preview-color-text",
                                _ => "preview-other-text",
                            },
                            text.clone(),
                        )),
                );
            }
            line = line.child(
                div()
                    .flex_none()
                    .w(px(pair_width))
                    .min_w_0()
                    .flex()
                    .items_start()
                    .gap(px(4.))
                    .text_size(px(UI_SMALL_FONT_SIZE))
                    .line_height(px(13.))
                    .child(
                        div()
                            .id(SharedString::from(format!(
                                "preview-info-label-{row}-{column}"
                            )))
                            .relative()
                            .flex_none()
                            .w(px(60.))
                            .h(px(13.))
                            .flex()
                            .whitespace_nowrap()
                            .overflow_hidden()
                            .text_color(rgb(t.text_dim))
                            .tooltip(tip((*label).to_string()))
                            .when(row == 0, |d| {
                                d.children(ui_checks::probe(if column == 0 {
                                    "preview-info-label-1"
                                } else {
                                    "preview-info-label-2"
                                }))
                            })
                            .when(*label == "Created", |d| {
                                d.children(ui_checks::probe("preview-created-label"))
                            })
                            .child((*label).to_string()),
                    )
                    .child(value.text_color(rgb(t.text))),
            );
        }
        if fields.len() == 1 {
            line = line.child(div().flex_1().min_w_0());
        }
        panel = panel.child(line);
    }
    panel.into_any_element()
}
