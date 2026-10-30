//! A cached, global drive list. All filesystem queries run outside rendering.
use super::*;

// NSWorkspace's own center reports mount/unmount/rename immediately. Callbacks
// only wake subscribers; filesystem work stays on the existing owned workers.
type MountSender = futures::channel::mpsc::UnboundedSender<()>;
static MOUNT_SUBSCRIBERS: Mutex<Vec<MountSender>> = Mutex::new(Vec::new());

fn wake_mount_subscribers(subscribers: &mut Vec<MountSender>) {
    subscribers.retain(|sender| sender.unbounded_send(()).is_ok());
}
define_class!(
    #[unsafe(super(NSObject))]
    #[name = "SchnauzVolumeObserver"]
    struct VolumeObserver;
    unsafe impl NSObjectProtocol for VolumeObserver {}
    impl VolumeObserver {
        #[unsafe(method(volumesChanged:))]
        fn volumes_changed(&self, _notification: &objc2_foundation::NSNotification) {
            wake_mount_subscribers(&mut MOUNT_SUBSCRIBERS.lock().unwrap_or_else(|e| e.into_inner()));
        }
    }
);
thread_local! {
    static MOUNT_OBSERVER: objc2::rc::Retained<VolumeObserver> = unsafe {
        let observer: objc2::rc::Retained<VolumeObserver> = objc2::msg_send![VolumeObserver::alloc(), init];
        let center = NSWorkspace::sharedWorkspace().notificationCenter();
        for name in [
            objc2_app_kit::NSWorkspaceDidMountNotification,
            objc2_app_kit::NSWorkspaceDidUnmountNotification,
            objc2_app_kit::NSWorkspaceDidRenameVolumeNotification,
            objc2_app_kit::NSWorkspaceDidWakeNotification,
        ] {
            center.addObserver_selector_name_object(&observer, objc2::sel!(volumesChanged:), Some(name), None);
        }
        observer
    };
}

fn mount_events() -> futures::channel::mpsc::UnboundedReceiver<()> {
    let (sender, receiver) = futures::channel::mpsc::unbounded();
    MOUNT_OBSERVER.with(|_| {});
    let mut subscribers = MOUNT_SUBSCRIBERS.lock().unwrap_or_else(|e| e.into_inner());
    subscribers.retain(|sender| !sender.is_closed());
    subscribers.push(sender);
    receiver
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub(super) struct Volume {
    name: String,
    path: PathBuf,
    #[serde(default = "default_local")]
    local: bool,
    total: Option<u64>,
    free: Option<u64>,
    identity: Option<String>,
    #[serde(default)]
    ejectable: bool,
}

#[derive(Clone, Default)]
struct DeviceActions {
    busy: HashSet<(PathBuf, String)>,
    revision: u64,
    completed: Option<(u64, PathBuf)>,
}
impl gpui::Global for DeviceActions {}

impl Volume {
    pub(super) fn info_name(&self) -> &str {
        &self.name
    }

    pub(super) fn info_rows(&self) -> Vec<(&'static str, String)> {
        let bytes = |value: Option<u64>| {
            value
                .map(|n| format!("{} ({} bytes)", format_size(false, n), group_digits(n)))
                .unwrap_or_else(|| "Unavailable".into())
        };
        vec![
            (
                "Kind",
                if self.path == Path::new("/") {
                    "Startup disk"
                } else {
                    "Volume"
                }
                .into(),
            ),
            ("Where", self.path.display().to_string()),
            ("Capacity", bytes(self.total)),
            ("Available", bytes(self.free)),
            (
                "Used",
                bytes(
                    self.total
                        .zip(self.free)
                        .map(|(total, free)| total.saturating_sub(free)),
                ),
            ),
        ]
    }

    fn can_eject(&self) -> bool {
        self.local
            && self.ejectable
            && self.path != Path::new("/")
            && self.path.parent() == Some(Path::new("/Volumes"))
            && self.identity.as_ref().is_some_and(|id| !id.is_empty())
    }
    fn request_key(&self) -> (PathBuf, String) {
        (self.path.clone(), self.identity.clone().unwrap_or_default())
    }
}

pub(super) fn is_ejecting(path: &Path, cx: &App) -> bool {
    cx.try_global::<DeviceActions>()
        .is_some_and(|state| state.busy.iter().any(|(root, _)| path.starts_with(root)))
}

fn snapshot() -> Result<Vec<Volume>, String> {
    let helper = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("Volume helper unavailable")?
        .join("volumectl");
    let output = workers::command(
        Command::new(helper),
        Vec::new(),
        Duration::from_secs(15),
        Arc::new(AtomicBool::new(false)),
    )?;
    if !output.status.success() {
        return Err("Couldn't refresh Devices.".into());
    }
    serde_json::from_slice(&output.stdout).map_err(|_| "Couldn't read the drive list.".into())
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
                    + if volume.can_eject() { 26. } else { 0. }
            })
            .fold(0., f32::max)
    }

    pub(super) fn refresh_volumes(&mut self, cx: &mut Context<Self>) {
        if self.volumes_loading {
            self.volumes_refresh_pending = true;
            return;
        }
        self.volumes_loading = true;
        self.volumes_generation = self.volumes_generation.wrapping_add(1);
        let generation = self.volumes_generation;
        cx.spawn(async move |this, cx| {
            let result = workers::result("schnauz-volumes", snapshot).await;
            let _ = this.update(cx, |this, cx| {
                if this.volumes_generation == generation {
                    this.volumes_loading = false;
                    if let Ok(volumes) = result {
                        if this.volumes != volumes {
                            this.volumes = volumes;
                            cx.notify();
                        }
                    }
                    if std::mem::take(&mut this.volumes_refresh_pending) {
                        this.refresh_volumes(cx);
                    }
                }
            });
        })
        .detach();
    }

    fn release_device_previews(&mut self, roots: &HashSet<(PathBuf, String)>) {
        let affected = |path: &Path| roots.iter().any(|(root, _)| path.starts_with(root));
        if self
            .video_preview
            .as_ref()
            .is_some_and(|preview| affected(&preview.path))
        {
            if let Some(preview) = self.video_preview.take() {
                preview.deactivate();
            }
        }
        if self
            .info_panel
            .as_ref()
            .is_some_and(|info| affected(&info.path))
        {
            self.info_panel = None;
        }
        self.preview_pending.borrow_mut().retain(|path, cancel| {
            if affected(path) {
                cancel.store(true, Ordering::Release);
                false
            } else {
                true
            }
        });
        self.info_pending.borrow_mut().retain(|path, cancel| {
            if affected(path) {
                cancel.store(true, Ordering::Release);
                false
            } else {
                true
            }
        });
        self.pdf_pending.borrow_mut().retain(|(path, _), cancel| {
            if affected(path) {
                cancel.store(true, Ordering::Release);
                false
            } else {
                true
            }
        });
    }

    fn device_ejected(&mut self, root: &Path) {
        for tab in self.panes.iter_mut().flat_map(|p| &mut p.tabs).chain(
            self.project_sessions
                .values_mut()
                .flat_map(|s| &mut s.panes)
                .flat_map(|p| &mut p.tabs),
        ) {
            if tab.remote.is_some() || !tab.current_dir.starts_with(root) {
                continue;
            }
            if let Some(cancel) = tab.listing_cancel.take() {
                cancel.store(true, Ordering::Release);
            }
            tab.entries.clear();
            tab.selection.clear();
            tab.anchor = None;
            tab.listing_dirty = false;
            tab.local_loading = false;
            tab.recompute_results();
            tab.listing_error = Some(directory_listing::ListingError {
                permission_denied: false,
                message: "This drive was ejected. Reconnect it, then choose Retry.".into(),
            });
        }
    }

    pub(super) fn watch_volumes_start(cx: &mut Context<Self>) {
        let mut events = mount_events();
        cx.spawn(async move |this, cx| {
            use futures::StreamExt;
            while events.next().await.is_some() {
                // Collapse a burst (for example several partitions mounting)
                // into one request. A running query queues one follow-up.
                while events.try_recv().is_ok() {}
                if this
                    .update(cx, |this, cx| this.refresh_volumes(cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        if cx.try_global::<DeviceActions>().is_none() {
            cx.set_global(DeviceActions::default());
        }
        let seen = std::cell::Cell::new(cx.global::<DeviceActions>().revision);
        cx.observe_global::<DeviceActions>(move |this, cx| {
            let state = cx.global::<DeviceActions>().clone();
            this.release_device_previews(&state.busy);
            if let Some((revision, path)) = &state.completed {
                if *revision > seen.get() {
                    seen.set(*revision);
                    this.device_ejected(path);
                }
            }
            this.refresh_volumes(cx);
            cx.notify();
        })
        .detach();
        cx.spawn(async move |this, cx| loop {
            if this
                .update(cx, |this, cx| this.refresh_volumes(cx))
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

    fn eject_volume(&mut self, volume: Volume, cx: &mut Context<Self>) {
        if !volume.can_eject() {
            return;
        }
        let key = volume.request_key();
        let mut state = cx.global::<DeviceActions>().clone();
        if !state.busy.insert(key.clone()) {
            return;
        }
        self.release_device_previews(&state.busy);
        cx.set_global(state);
        self.set_notice_sticky(format!("Ejecting “{}”…", volume.name), cx);
        cx.spawn(async move |this, cx| {
            // Let other windows release their native previews before macOS checks use.
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let request = key.clone();
            let result =
                workers::result("schnauz-eject-volume", move || -> Result<String, String> {
                    #[derive(serde::Deserialize)]
                    struct Response {
                        success: bool,
                        message: String,
                    }
                    let helper = std::env::current_exe()
                        .map_err(|e| e.to_string())?
                        .parent()
                        .ok_or("Volume helper unavailable")?
                        .join("volumectl");
                    let mut command = Command::new(helper);
                    command.arg("--eject").arg(&request.0).arg(&request.1);
                    let output = workers::command(
                        command,
                        Vec::new(),
                        Duration::from_secs(90),
                        Arc::new(AtomicBool::new(false)),
                    )?;
                    let response: Response = serde_json::from_slice(&output.stdout)
                        .map_err(|_| "The drive could not be ejected.".to_string())?;
                    if response.success && output.status.success() {
                        Ok(response.message)
                    } else {
                        Err(response.message)
                    }
                })
                .await;
            let _ = cx.update(|cx| {
                let mut state = cx.global::<DeviceActions>().clone();
                state.busy.remove(&key);
                if result.is_ok() {
                    state.revision = state.revision.wrapping_add(1);
                    state.completed = Some((state.revision, key.0));
                }
                cx.set_global(state);
                let _ = this.update(cx, |this, cx| match result {
                    Ok(message) => this.set_notice(message, cx),
                    Err(error) => this.set_notice(format!("Couldn't eject drive: {error}"), cx),
                });
            });
        })
        .detach();
    }

    fn device_menu(&mut self, volume: Volume, event: &MouseDownEvent, cx: &mut Context<Self>) {
        let info = volume.clone();
        let mut nodes = vec![mi("Info", move |this, _, cx| {
            this.open_volume_info(info.clone(), cx)
        })];
        if volume.can_eject() {
            let node = if cx
                .global::<DeviceActions>()
                .busy
                .contains(&volume.request_key())
            {
                MenuNode::Disabled("Ejecting…".into())
            } else {
                mi(
                    format!("Eject “{}”", volume.name),
                    move |this, _, cx| this.eject_volume(volume.clone(), cx),
                )
            };
            nodes.push(node);
        }
        self.sidebar_menu = None;
        self.sort_menu = None;
        self.context_menu = Some(ContextMenu {
            x: f32::from(event.position.x),
            y: f32::from(event.position.y),
            target: None,
            nodes,
        });
        cx.stop_propagation();
        cx.notify();
    }

    fn open_volume_info(&mut self, volume: Volume, cx: &mut Context<Self>) {
        self.close_context_menu(cx);
        self.info_panel = Some(InfoPanel {
            cancel: Arc::new(AtomicBool::new(false)),
            error: None,
            path: volume.path.clone(),
            is_dir: true,
            info: FileInfo::default(),
            perms: String::new(),
            owner: String::new(),
            total: None,
            volume: Some(volume),
        });
        cx.notify();
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
            let menu_volume = volume.clone();
            let eject_volume = volume.clone();
            let busy = cx
                .try_global::<DeviceActions>()
                .is_some_and(|state| state.busy.contains(&volume.request_key()));
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
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(rgb(theme().text))
                            .child(volume.name.clone()),
                    )
                    .when(volume.can_eject(), |d| {
                        d.child(
                            div()
                                .id(("device-eject", index))
                                .flex_none()
                                .size(px(18.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_sm()
                                .when(!busy, |d| {
                                    d.cursor_pointer().hover(|d| d.bg(rgb(theme().selected)))
                                })
                                .tooltip(tip(if busy {
                                    "Ejecting…".into()
                                } else {
                                    format!("Eject “{}”", volume.name)
                                }))
                                .child(toolbar_icons::ui_icon(
                                    Icon::Eject,
                                    13.,
                                    if busy {
                                        Theme::alpha(theme().text_muted, 0x66)
                                    } else {
                                        rgb(theme().text_muted)
                                    },
                                ))
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    cx.stop_propagation();
                                    if !busy {
                                        this.eject_volume(eject_volume.clone(), cx);
                                    }
                                })),
                        )
                    })
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
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            this.device_menu(menu_volume.clone(), event, cx)
                        }),
                    )
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
    fn mount_events_wake_all_windows_and_retire_closed_subscribers() {
        let (first, mut a) = futures::channel::mpsc::unbounded();
        let (second, mut b) = futures::channel::mpsc::unbounded();
        let (closed, gone) = futures::channel::mpsc::unbounded();
        drop(gone);
        let mut subscribers = vec![first, closed, second];
        wake_mount_subscribers(&mut subscribers);
        assert_eq!(a.try_recv(), Ok(()));
        assert_eq!(b.try_recv(), Ok(()));
        assert_eq!(subscribers.len(), 2);
    }

    #[test]
    fn drive_info_uses_cached_capacity_even_for_an_unavailable_path() {
        let volume: Volume = serde_json::from_str(r#"{"name":"Fixture","path":"/Volumes/nonexistent-fixture","local":true,"total":100,"free":25,"identity":null,"ejectable":false}"#).unwrap();
        let rows = volume.info_rows();
        assert!(rows
            .iter()
            .any(|(label, value)| *label == "Capacity" && value.contains("100 bytes")));
        assert!(rows
            .iter()
            .any(|(label, value)| *label == "Available" && value.contains("25 bytes")));
        assert!(rows
            .iter()
            .any(|(label, value)| *label == "Used" && value.contains("75 bytes")));
        assert!(!volume.can_eject());
    }

    #[test]
    fn eject_requires_a_current_external_mount_identity_and_never_the_startup_disk() {
        let mut external:Volume=serde_json::from_str(r#"{"name":"USB","path":"/Volumes/USB","local":true,"total":100,"free":50,"identity":"mount-A","ejectable":true}"#).unwrap();
        assert!(external.can_eject());
        assert_eq!(
            external.request_key(),
            (PathBuf::from("/Volumes/USB"), "mount-A".into())
        );
        external.path = PathBuf::from("/");
        assert!(!external.can_eject());
        external.path = PathBuf::from("/Volumes/USB");
        external.local = false;
        assert!(!external.can_eject());
        external.local = true;
        external.identity = None;
        assert!(!external.can_eject());
        let old: Volume = serde_json::from_str(
            r#"{"name":"Old helper","path":"/Volumes/USB","local":true,"total":100,"free":50}"#,
        )
        .unwrap();
        assert!(!old.can_eject());
    }

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
