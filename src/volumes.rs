//! A cached, global drive list. All filesystem queries run outside rendering.
use super::*;

#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub(super) struct Volume {
    name: String,
    path: PathBuf,
    #[serde(default = "default_local")]
    local: bool,
    total: Option<u64>,
    free: Option<u64>,
}

fn default_local() -> bool {
    true
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

fn free_space_label(total: Option<u64>, free: Option<u64>, percent: bool) -> String {
    let Some(free) = free else {
        return "—".into();
    };
    if percent {
        match total.filter(|total| *total > 0) {
            Some(total) => format!("{:.1}%", free.min(total) as f64 * 100. / total as f64),
            None => "—".into(),
        }
    } else {
        format!("{:.1} GB", free as f64 / 1_000_000_000.)
    }
}

impl Schnauz {
    pub(super) fn network_volume_path(&self, path: &Path) -> bool {
        self.volumes
            .iter()
            .any(|v| !v.local && path.starts_with(&v.path))
    }

    pub(super) fn volume_sidebar_width(&self, window: &Window) -> f32 {
        self.volumes
            .iter()
            .filter(|v| v.local)
            .map(|volume| {
                let free = free_space_label(volume.total, volume.free, prefs().drive_free_percent);
                project_sidebar::text_width(window, &volume.name, UI_FONT_SIZE)
                    + project_sidebar::text_width(window, &free, 10.)
                    + 65.
            })
            .fold(0., f32::max)
    }

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
        // Global devices stay above every project's Favourites.
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
        for (index, volume) in self.volumes.iter().filter(|v| v.local).enumerate() {
            let p = prefs();
            let color = match usage(volume.total, volume.free, p.drive_warning, p.drive_critical) {
                Usage::Healthy => 0x237a49,
                Usage::Warning => 0x9a6700,
                Usage::Critical => 0xc23a35,
                Usage::Unknown => theme().text_dim,
            };
            let free = free_space_label(volume.total, volume.free, p.drive_free_percent);
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
                    .h(px(24.))
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
                        sidebar_badge(free.clone(), color)
                            .id(("device-free", index))
                            .cursor_pointer()
                            .tooltip(tip(if p.drive_free_percent {
                                "Free space · Click to show GB"
                            } else {
                                "Free space · Click to show percent"
                            }))
                            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                let mut p = prefs();
                                p.drive_free_percent = !p.drive_free_percent;
                                apply_prefs(p, cx);
                            })),
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
    fn free_capacity_formats_gb_and_percent_without_inventing_unknown_values() {
        assert_eq!(
            free_space_label(Some(1_000_000_000_000), Some(250_000_000_000), false),
            "250.0 GB"
        );
        assert_eq!(
            free_space_label(Some(1_000_000_000_000), Some(250_000_000_000), true),
            "25.0%"
        );
        assert_eq!(free_space_label(Some(100), Some(0), true), "0.0%");
        assert_eq!(free_space_label(Some(100), Some(150), true), "100.0%");
        assert_eq!(free_space_label(Some(0), Some(0), true), "—");
        assert_eq!(free_space_label(None, Some(1_000_000_000), false), "1.0 GB");
        assert_eq!(free_space_label(None, Some(1_000_000_000), true), "—");
        assert_eq!(free_space_label(Some(100), None, true), "—");
        assert_eq!(free_space_label(Some(100), None, false), "—");
    }

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
