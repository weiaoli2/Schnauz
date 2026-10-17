//! A cached, global drive list. All filesystem queries run outside rendering.
use super::*;

#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub(super) struct Volume {
    name: String,
    path: PathBuf,
    total: Option<u64>,
    free: Option<u64>,
}

#[derive(Debug, PartialEq)]
enum Usage {
    Healthy,
    Warning,
    Critical,
    Unknown,
}

fn usage(total: Option<u64>, free: Option<u64>, warning: u8, critical: u8) -> Usage {
    let (Some(total), Some(free)) = (total.filter(|n| *n > 0), free) else {
        return Usage::Unknown;
    };
    let used = (total.saturating_sub(free) as f64 / total as f64) * 100.;
    if used > f64::from(critical) {
        Usage::Critical
    } else if used >= f64::from(warning) {
        Usage::Warning
    } else {
        Usage::Healthy
    }
}

impl Shuffle {
    pub(super) fn watch_volumes_start(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| loop {
            let result = cx
                .background_spawn(async move {
                    let helper = std::env::current_exe().ok()?.parent()?.join("volumectl");
                    let output = Command::new(helper).output().ok()?;
                    output
                        .status
                        .success()
                        .then(|| serde_json::from_slice::<Vec<Volume>>(&output.stdout).ok())
                        .flatten()
                })
                .await;
            if this
                .update(cx, |this, cx| {
                    if let Some(volumes) = result {
                        if this.volumes != volumes {
                            this.volumes = volumes;
                            cx.notify();
                        }
                    }
                })
                .is_err()
            {
                break;
            }
            cx.background_executor()
                .timer(Duration::from_secs(30))
                .await;
        })
        .detach();
    }

    pub(super) fn push_volumes(&self, items: &mut Vec<AnyElement>, cx: &Context<Self>) {
        // Global devices stay above every project's own Locations.
        items.push(
            div()
                .px_3()
                .pt_3()
                .pb_1()
                .text_size(px(UI_SMALL_FONT_SIZE))
                .text_color(rgb(theme().text_dim))
                .child("DEVICES")
                .into_any_element(),
        );
        for (index, volume) in self.volumes.iter().enumerate() {
            let p = prefs();
            let color = match usage(volume.total, volume.free, p.drive_warning, p.drive_critical) {
                Usage::Healthy => 0x237a49,
                Usage::Warning => 0x9a6700,
                Usage::Critical => 0xc23a35,
                Usage::Unknown => theme().text_dim,
            };
            let free = volume
                .free
                .map(|size| format_size(false, size))
                .unwrap_or_else(|| "—".into());
            let used = match (volume.total, volume.free) {
                (Some(total), Some(free)) if total > 0 => format!(
                    " · {:.0}% used",
                    total.saturating_sub(free) as f64 * 100. / total as f64
                ),
                _ => String::new(),
            };
            let path = volume.path.clone();
            let active =
                self.active_tab().remote.is_none() && self.active_tab().current_dir == path;
            items.push(
                div()
                    .id(("device", index))
                    .mx_2()
                    .px_2()
                    .py_1p5()
                    .rounded_md()
                    .flex()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .when(active, |d| d.bg(rgb(theme().selected)))
                    .hover(|d| d.bg(rgb(theme().hover)))
                    .child(toolbar_icons::ui_icon(
                        Icon::Drive,
                        17.,
                        rgb(theme().accent),
                    ))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_color(rgb(theme().text))
                            .child(volume.name.clone()),
                    )
                    .child(
                        div()
                            .flex_none()
                            .px_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .bg(rgb(color))
                            .text_color(rgb(0xffffff))
                            .child(free.clone()),
                    )
                    .tooltip(tip(format!(
                        "{}\n{} free{}",
                        volume.path.display(),
                        free,
                        used
                    )))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        window.focus(&this.focus);
                        let pane = this.active_pane;
                        this.unsync_ssh_browser(pane, cx);
                        if this.tab(pane).remote.is_some() {
                            let p = this.pane_mut(pane);
                            p.tabs.push(Tab::new(path.clone()));
                            p.active = p.tabs.len() - 1;
                        }
                        if this.tab(pane).current_dir == path {
                            this.load_dir_in(pane, path.clone(), cx);
                        } else {
                            this.navigate_in(pane, path.clone(), cx);
                        }
                    }))
                    .into_any_element(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn thresholds_use_used_space_and_unknown_capacity_is_not_reported_full() {
        assert_eq!(usage(Some(100), Some(21), 80, 90), Usage::Healthy);
        assert_eq!(usage(Some(100), Some(20), 80, 90), Usage::Warning);
        assert_eq!(usage(Some(100), Some(10), 80, 90), Usage::Warning);
        assert_eq!(usage(Some(100), Some(9), 80, 90), Usage::Critical);
        assert_eq!(usage(Some(100), Some(150), 60, 75), Usage::Healthy);
        assert_eq!(usage(Some(100), Some(30), 60, 75), Usage::Warning);
        assert_eq!(usage(Some(0), Some(0), 80, 90), Usage::Unknown);
        assert_eq!(usage(Some(100), None, 80, 90), Usage::Unknown);
    }
}
