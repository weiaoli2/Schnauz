//! User-directed macOS file-access setup. Never probes protected folders or TCC.
use super::*;

pub(super) const HELP: &str = "To browse protected folders, enable Schnauz in System Settings → Privacy & Security → Full Disk Access. This allows access to files across your Mac.";
pub(super) const STEPS: &str = "If Schnauz is missing, use + to add the current Schnauz.app. If macOS asks again after an update, remove its old entry and add the current app again. Restart Schnauz when convenient. You can continue without it; some folders will remain restricted.";

#[derive(Default)]
pub(super) struct Intro {
    error: Option<String>,
}

pub(super) fn open_settings() -> Result<(), String> {
    // Apple's macOS 13+ link, documented in the SDK's EndpointSecurity/ESClient.h.
    let url = NSURL::URLWithString(&NSString::from_str(
        "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Privacy_AllFiles",
    ))
    .ok_or("Couldn't open System Settings. Open Privacy & Security → Full Disk Access manually.")?;
    if NSWorkspace::sharedWorkspace().openURL(&url) {
        Ok(())
    } else {
        Err(
            "Couldn't open System Settings. Open Privacy & Security → Full Disk Access manually."
                .into(),
        )
    }
}

impl Schnauz {
    pub(super) fn dismiss_file_access_intro(&mut self, cx: &mut Context<Self>) {
        if self.file_access_intro.take().is_some() {
            let mut p = prefs();
            // This records acknowledgement only, never a claimed permission grant.
            p.file_access_intro_seen = true;
            apply_prefs(p, cx);
            cx.notify();
        }
    }

    pub(super) fn open_file_access_settings(&mut self, cx: &mut Context<Self>) {
        match open_settings() {
            Ok(()) => self.dismiss_file_access_intro(cx),
            Err(error) => {
                if let Some(intro) = &mut self.file_access_intro {
                    intro.error = Some(error);
                }
                cx.notify();
            }
        }
    }

    pub(super) fn render_file_access_intro(&self, cx: &Context<Self>) -> impl IntoElement {
        let t = theme();
        let button = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .px_3()
                .py_2()
                .rounded_md()
                .cursor_pointer()
                .text_size(px(UI_FONT_SIZE))
                .bg(rgb(t.hover))
                .text_color(rgb(t.text))
                .hover(|s| s.bg(rgb(t.selected)))
                .child(label)
        };
        let error = self
            .file_access_intro
            .as_ref()
            .and_then(|intro| intro.error.clone());
        modal_backdrop(cx.listener(|this, _, _, cx| this.dismiss_file_access_intro(cx))).child(
            div()
                .w(px(480.))
                .p_5()
                .rounded_lg()
                .bg(rgb(t.surface))
                .border_1()
                .border_color(rgb(t.border_strong))
                .shadow_lg()
                .flex()
                .flex_col()
                .gap_4()
                .on_mouse_down(MouseButton::Left, |_, _, cx: &mut App| {
                    cx.stop_propagation()
                })
                .child(
                    div()
                        .text_size(px(16.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(t.text))
                        .child("File access for Schnauz"),
                )
                .child(
                    div()
                        .text_size(px(UI_FONT_SIZE))
                        .text_color(rgb(t.text))
                        .child(HELP),
                )
                .child(
                    div()
                        .text_size(px(UI_FONT_SIZE))
                        .text_color(rgb(t.text))
                        .child(STEPS),
                )
                .children(error.map(|error| {
                    div()
                        .text_size(px(UI_FONT_SIZE))
                        .text_color(rgb(0xc23a35))
                        .child(error)
                }))
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child(button("file-access-later", "Not Now").on_click(cx.listener(
                            |this, _: &ClickEvent, _, cx| this.dismiss_file_access_intro(cx),
                        )))
                        .child(
                            button("file-access-settings", "Open System Settings")
                                .bg(rgb(t.accent))
                                .text_color(rgb(0xffffff))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.open_file_access_settings(cx)
                                })),
                        ),
                ),
        )
    }
}
