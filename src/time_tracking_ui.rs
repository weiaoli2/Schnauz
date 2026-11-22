//! One clock for every browser window and parked project session.
use super::*;
use gpui::{Entity, WeakEntity};
use time_tracking::{Clock, Saved, Status};

#[derive(Clone)]
struct Shared {
    model: Entity<Tracker>,
    browsers: Vec<WeakEntity<Schnauz>>,
    floating: Option<gpui::WindowHandle<Floating>>,
}
impl gpui::Global for Shared {}

pub(super) struct Tracker {
    pub clock: Clock,
    path: Option<PathBuf>,
    pub notice: Option<String>,
    load_error: bool,
    last_tick: Instant,
    last_wall: u64,
    last_saved: Instant,
    pub reminder: bool,
    reminder_ack: Instant,
    active_project: Option<u64>,
    project_ids: HashSet<u64>,
    last_bytes: Vec<u8>,
}
impl Tracker {
    fn new(cx: &mut Context<Self>) -> Self {
        cx.observe_global::<ProjectLibraryGlobal>(|this, cx| {
            this.account();
            let library = cx.global::<ProjectLibraryGlobal>().0.clone();
            let active = library.active;
            let ids = library.items.iter().map(|p| p.id).collect::<HashSet<_>>();
            for id in this.project_ids.difference(&ids) {
                let key = time_tracking::key(Some(*id));
                this.clock.retire(
                    &key,
                    &format!("archived-{id}-{}", time_tracking_bridge::now_ms()),
                );
            }
            this.project_ids = ids;
            for project in &library.items {
                if let Some(saved) = this
                    .clock
                    .saved
                    .projects
                    .get_mut(&time_tracking::key(Some(project.id)))
                {
                    saved.name = project.name.clone();
                }
            }
            if active != this.active_project {
                this.active_project = active;
                this.clock.activity(&time_tracking::key(active));
            }
            this.queue_save();
            cx.notify();
        })
        .detach();
        let path = config_file("time-tracking.json");
        let loaded = path.as_ref().map(|p| fs::read(p)).transpose();
        let (saved, notice, load_error) = match loaded {
            Ok(Some(bytes)) => match serde_json::from_slice::<Saved>(&bytes) {
                Ok(saved) if saved.version == 1 => (saved, None, false),
                _ => (
                    Saved::default(),
                    Some("Couldn't read time tracking. The existing file was preserved.".into()),
                    true,
                ),
            },
            Ok(None) => (
                Saved::default(),
                Some("Cannot find the settings folder.".into()),
                true,
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Saved::default(), None, false),
            Err(e) => (
                Saved::default(),
                Some(format!("Couldn't read time tracking: {e}")),
                true,
            ),
        };
        cx.on_app_quit(|this, _| {
            this.account();
            this.queue_save();
            let path = this.path.clone().filter(|_| !this.load_error);
            async move {
                if let Some(path) = path {
                    let _ = workers::run("schnauz-save-time-on-quit", move || {
                        persistence::flush(&path)
                    })
                    .await;
                }
            }
        })
        .detach();
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            let bindings = match cx.update(|cx| {
                let Some(shared) = cx.try_global::<Shared>() else {
                    return Vec::new();
                };
                let clock = &shared.model.read(cx).clock;
                shared
                    .browsers
                    .iter()
                    .filter_map(|b| b.upgrade())
                    .flat_map(|b| b.read(cx).time_tracking_bindings(cx))
                    .map(|mut binding| {
                        binding.enabled = clock.enabled(&binding.project);
                        binding
                    })
                    .collect()
            }) {
                Ok(b) => b,
                Err(_) => break,
            };
            let result = workers::run("schnauz-time-status", move || {
                time_tracking_bridge::collect(bindings)
            })
            .await;
            let Ok(events) = result else {
                continue;
            };
            if this.update(cx, |this, cx| this.tick(events, cx)).is_err() {
                break;
            }
        })
        .detach();
        let last_bytes = serde_json::to_vec_pretty(&saved).unwrap_or_default();
        Self {
            clock: Clock::new(saved),
            path,
            notice,
            load_error,
            last_tick: Instant::now(),
            last_wall: time_tracking_bridge::now_ms(),
            last_saved: Instant::now(),
            reminder: false,
            reminder_ack: Instant::now(),
            active_project: cx.global::<ProjectLibraryGlobal>().0.active,
            project_ids: cx
                .global::<ProjectLibraryGlobal>()
                .0
                .items
                .iter()
                .map(|p| p.id)
                .collect(),
            last_bytes,
        }
    }
    fn account(&mut self) {
        let now = Instant::now();
        let wall = time_tracking_bridge::now_ms();
        let ms = now
            .duration_since(self.last_tick)
            .as_millis()
            .min(u64::MAX as u128) as u64;
        let wall_delta = wall.abs_diff(self.last_wall);
        if wall >= self.last_wall && wall_delta.abs_diff(ms) <= 2000 {
            self.clock
                .advance(ms, &chrono::Local::now().format("%Y-%m-%d").to_string());
        } else {
            // Sleep/clock jumps are not charged. A native menu or a busy
            // renderer alone must not pause a deliberately running timer.
            self.clock.suspend_personal();
        }
        self.last_tick = now;
        self.last_wall = wall;
    }
    fn tick(
        &mut self,
        bindings: Vec<(time_tracking_bridge::Binding, Vec<time_tracking::Event>)>,
        cx: &mut Context<Self>,
    ) {
        let now = Instant::now();
        let wall = time_tracking_bridge::now_ms();
        let elapsed = now.duration_since(self.last_tick).as_millis() as u64;
        let continuous =
            wall >= self.last_wall && (wall - self.last_wall).abs_diff(elapsed) <= 2000;
        if !continuous {
            self.clock.suspend_personal();
        }
        let mut cursor = if continuous { self.last_wall } else { wall };
        let day = chrono::Local::now().format("%Y-%m-%d").to_string();
        let mut live = HashSet::new();
        let mut timeline = Vec::new();
        for (binding, events) in bindings {
            let channel = binding.channel.to_string_lossy().into_owned();
            if binding.running {
                live.insert(channel.clone());
            }
            for event in events {
                timeline.push((channel.clone(), binding.project.clone(), event));
            }
        }
        timeline.sort_by_key(|(_, _, event)| event.at_ms);
        for (channel, project, event) in timeline {
            // Honor start/stop timestamps, including work that finishes between
            // two polls. Late events are clamped; downtime is never backfilled.
            let at = event.at_ms.clamp(cursor, wall);
            self.clock.advance(at - cursor, &day);
            cursor = at;
            self.clock.event(&channel, &project, event);
        }
        self.clock.advance(wall - cursor, &day);
        self.last_tick = now;
        self.last_wall = wall;
        self.clock.retain_channels(&live);
        if session_locked() {
            self.clock.suspend_personal();
        }
        self.reminder = self.clock.personal.is_some()
            && self.reminder_ack.elapsed() > Duration::from_secs(1800)
            && idle_seconds() >= 1800.;
        if self.last_saved.elapsed() >= Duration::from_secs(5) {
            self.queue_save();
            if let Some(path) = self.path.clone().filter(|_| !self.load_error) {
                let model = cx.weak_entity();
                cx.spawn(async move |_, cx| {
                    let result =
                        workers::run("schnauz-save-time", move || persistence::flush(&path)).await;
                    if let Ok(Err(error)) = result {
                        let _ = model.update(cx, |this, cx| {
                            this.notice = Some(format!("Couldn't save tracked time: {error}"));
                            cx.notify();
                        });
                    }
                })
                .detach();
            }
            self.last_saved = Instant::now();
        }
        if self.clock.saved.projects.values().any(|p| p.enabled) {
            cx.notify();
        }
    }
    fn queue_save(&mut self) {
        if self.load_error || self.clock.saved.projects.is_empty() {
            return;
        }
        if let (Some(path), Ok(bytes)) = (&self.path, serde_json::to_vec_pretty(&self.clock.saved))
        {
            if bytes != self.last_bytes {
                self.last_bytes = bytes.clone();
                persistence::queue(path.clone(), bytes);
            }
        }
    }
    fn enable(&mut self, project: &str, enabled: bool, cx: &mut Context<Self>) {
        if self.load_error {
            cx.notify();
            return;
        }
        self.account();
        self.clock.enable(project, enabled);
        if let Some(data) = self.clock.saved.projects.get_mut(project) {
            data.name = if project == "personal" {
                "Personal".into()
            } else {
                cx.global::<ProjectLibraryGlobal>()
                    .0
                    .items
                    .iter()
                    .find(|p| p.id.to_string() == project)
                    .map(|p| p.name.clone())
                    .unwrap_or_default()
            };
        }
        if enabled && time_tracking::key(cx.global::<ProjectLibraryGlobal>().0.active) == project {
            self.clock.activity(project);
        }
        self.queue_save();
        cx.notify();
    }
    fn pause(&mut self, project: &str, paused: bool, cx: &mut Context<Self>) {
        self.account();
        self.clock.pause(project, paused);
        self.reminder = false;
        self.reminder_ack = Instant::now();
        self.queue_save();
        cx.notify();
    }
}

pub(super) fn initialize(cx: &mut Context<Schnauz>) {
    if cx.try_global::<Shared>().is_none() {
        let model = cx.new(Tracker::new);
        cx.set_global(Shared {
            model,
            browsers: Vec::new(),
            floating: None,
        });
    }
    let mut state = cx.global::<Shared>().clone();
    state.browsers.retain(|b| b.upgrade().is_some());
    state.browsers.push(cx.weak_entity());
    let model = state.model.clone();
    cx.set_global(state);
    cx.observe(&model, |_, _, cx| cx.notify()).detach();
    cx.observe_global::<Shared>(|_, cx| cx.notify()).detach();
}
fn model(cx: &App) -> Entity<Tracker> {
    cx.global::<Shared>().model.clone()
}
pub(super) fn enabled(project: Option<u64>, cx: &App) -> bool {
    cx.try_global::<Shared>()
        .is_some_and(|s| s.model.read(cx).clock.enabled(&time_tracking::key(project)))
}
pub(super) fn set_enabled(project: Option<u64>, value: bool, cx: &mut App) -> Result<(), String> {
    let key = time_tracking::key(project);
    model(cx).update(cx, |this, cx| {
        if this.load_error {
            return Err(this
                .notice
                .clone()
                .unwrap_or_else(|| "Time tracking is unavailable.".into()));
        }
        this.enable(&key, value, cx);
        Ok(())
    })
}
pub(super) fn activity(project: Option<u64>, cx: &mut App) {
    let Some(shared) = cx.try_global::<Shared>() else {
        return;
    };
    let model = shared.model.clone();
    let project = time_tracking::key(project);
    model.update(cx, |this, cx| {
        this.account();
        let old = this.clock.personal.clone();
        this.clock.activity(&project);
        this.reminder_ack = Instant::now();
        this.reminder = false;
        if old != this.clock.personal {
            this.queue_save();
            cx.notify();
        }
    });
}
pub(super) fn docked(project: Option<u64>, cx: &App) -> bool {
    enabled(project, cx) && cx.global::<Shared>().floating.is_none()
}
pub(super) fn panel(project: Option<u64>, cx: &App) -> AnyElement {
    Panel {
        model: model(cx),
        project,
        floating: false,
    }
    .into_any_element()
}
pub(super) fn show(project: Option<u64>, window: &mut Window, cx: &mut App) -> Result<(), String> {
    if !enabled(project, cx) {
        set_enabled(project, true, cx)?;
    }
    if let Some(floating) = cx.global::<Shared>().floating {
        let _ = floating.update(cx, |_, window, _| window.activate_window());
    } else {
        let _ = window;
    }
    Ok(())
}

fn project_name(project: Option<u64>, cx: &App) -> String {
    project
        .and_then(|id| {
            cx.global::<ProjectLibraryGlobal>()
                .0
                .items
                .iter()
                .find(|p| p.id == id)
                .map(|p| p.name.clone())
        })
        .unwrap_or_else(|| {
            if project.is_none() {
                "Personal".into()
            } else {
                "Removed project".into()
            }
        })
}
#[derive(IntoElement)]
struct Panel {
    model: Entity<Tracker>,
    project: Option<u64>,
    floating: bool,
}
impl RenderOnce for Panel {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme();
        let project = time_tracking::key(self.project);
        let state = self.model.read(cx);
        let data = state
            .clock
            .saved
            .projects
            .get(&project)
            .cloned()
            .unwrap_or_default();
        let personal = state.clock.personal.as_deref() == Some(project.as_str());
        let codex = state.clock.status(&project);
        let reminder = state.reminder && personal;
        let notice = state.notice.clone();
        let connected = state.clock.connected.contains(&project);
        let pause_model = self.model.clone();
        let pause_key = project.clone();
        let setup_model = self.model.clone();
        let reminder_model = self.model.clone();
        let floating = self.floating;
        let row = |label: &str, value: String, status: &str| {
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(div().w(px(68.)).child(label.to_string()))
                .child(
                    div()
                        .flex_1()
                        .text_color(rgb(t.text_muted))
                        .child(status.to_string()),
                )
                .child(div().text_color(rgb(t.text)).child(value))
        };
        div().id(SharedString::from(format!("time-project-{project}"))).flex_none().w_full().max_h(px(280.)).overflow_y_scroll().flex().flex_col().gap_1().p_2().text_size(px(12.))
            .border_t_1().border_color(rgb(t.border)).bg(rgb(t.sidebar)).text_color(rgb(t.text))
            .child(div().flex().items_center().gap_2()
                .child(ui_icon(Icon::Stopwatch, 16., rgb(0x239b56)))
                .child(div().flex_1().overflow_hidden().whitespace_nowrap().child(format!("{} · Time Tracking", project_name(self.project, cx))))
                .child(div().id("time-detach").cursor_pointer().child(ui_icon(if floating {Icon::PinFloating} else {Icon::PinDocked}, 15., rgb(t.text_muted)))
                    .tooltip(tip(if floating {"Dock time tracking"} else {"Float time tracking above other windows"}))
                    .on_click(move |_, window, cx|if floating {dock(cx)} else {detach(window,cx)})))
            .child(row("Total", time_tracking::duration(data.totals.total_ms), "overlap counted once"))
            .child(row("My time", time_tracking::duration(data.totals.personal_ms), if personal {"Running"} else {"Paused"}))
            .child(row("Codex", time_tracking::duration(data.totals.codex_ms), match codex {
                Status::Working => "Working", Status::Waiting => "Waiting for you",
                Status::Idle if connected => "Idle", _ => "Not connected",
            }))
            .child(div().flex().gap_2()
                .child(div().id("time-pause").px_2().py_1().rounded_md().bg(rgb(t.surface)).cursor_pointer()
                    .child(if personal {"Pause my time"} else {"Resume my time"})
                    .on_click(move |_, _, cx|pause_model.update(cx, |this, cx|this.pause(&pause_key, personal, cx))))
                .when(!connected, |d| d.child(div().id("time-connect").px_2().py_1().rounded_md().bg(rgb(t.surface)).cursor_pointer().child("Connect Codex…")
                    .tooltip(tip("Show how to connect Codex using session-specific status hooks."))
                    .on_click(move |_, _, cx| {
                        setup_model.update(cx, |this,cx| {
                            this.notice=Some(time_tracking_bridge::instructions());cx.notify();
                        });
                    }))))
            .when(reminder, |d|d.child(div().flex().items_center().gap_2()
                .child("Still working on this project?")
                .child(div().id("time-still-working").cursor_pointer().text_color(rgb(t.accent)).child("Yes")
                    .on_click(move|_,_,cx|reminder_model.update(cx,|this,cx| {
                        this.reminder=false;this.reminder_ack=Instant::now();cx.notify();
                    })))))
            .when_some(notice, |d, notice|d.child(div().text_size(px(11.)).child(notice)))
    }
}
fn detach(window: &mut Window, cx: &mut App) {
    if cx.global::<Shared>().floating.is_some() {
        return;
    }
    let tracker_model = model(cx);
    let result = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(410.), px(230.)),
                cx,
            ))),
            window_min_size: Some(size(px(350.), px(180.))),
            titlebar: Some(TitlebarOptions {
                title: Some("Schnauz · Time Tracking".into()),
                appears_transparent: true,
                ..Default::default()
            }),
            focus: window.is_window_active(),
            show: true,
            ..Default::default()
        },
        move |window, cx| {
            todoist_window::configure_native(window);
            window.on_window_should_close(cx, |_, cx| {
                let mut state = cx.global::<Shared>().clone();
                state.floating = None;
                cx.set_global(state);
                true
            });
            cx.new(|cx| {
                cx.observe(&tracker_model, |_, _, cx| cx.notify()).detach();
                cx.observe_global::<ProjectLibraryGlobal>(|_, cx| cx.notify())
                    .detach();
                cx.observe_global::<ThemeGlobal>(|_, cx| cx.notify())
                    .detach();
                Floating {
                    model: tracker_model,
                }
            })
        },
    );
    match result {
        Ok(handle) => {
            let mut state = cx.global::<Shared>().clone();
            state.floating = Some(handle);
            cx.set_global(state);
        }
        Err(e) => model(cx).update(cx, |this, cx| {
            this.notice = Some(format!("Couldn't open time tracking: {e}"));
            cx.notify();
        }),
    }
}
fn dock(cx: &mut App) {
    let mut state = cx.global::<Shared>().clone();
    let floating = state.floating.take();
    cx.set_global(state);
    if let Some(window) = floating {
        let _ = window.update(cx, |_, w, _| w.remove_window());
    }
}
struct Floating {
    model: Entity<Tracker>,
}
impl Render for Floating {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let projects = cx.global::<ProjectLibraryGlobal>().0.clone();
        let clock = &self.model.read(cx).clock;
        let mut ids = Vec::new();
        if clock.enabled("personal") {
            ids.push(None);
        }
        ids.extend(
            projects
                .items
                .iter()
                .filter(|p| clock.enabled(&time_tracking::key(Some(p.id))))
                .map(|p| Some(p.id)),
        );
        div()
            .id("floating-time-tracking")
            .size_full()
            .overflow_y_scroll()
            .bg(rgb(theme().sidebar))
            .children(ids.into_iter().map(|project| {
                Panel {
                    model: self.model.clone(),
                    project,
                    floating: true,
                }
                .into_any_element()
            }))
            .when(!clock.saved.projects.values().any(|p| p.enabled), |d| {
                d.child("Enable Time Tracking from a project's context menu.")
            })
    }
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
    fn CGSessionCopyCurrentDictionary() -> *mut std::ffi::c_void;
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(value: *const std::ffi::c_void);
}
fn idle_seconds() -> f64 {
    unsafe { CGEventSourceSecondsSinceLastEventType(0, u32::MAX) }
}
fn session_locked() -> bool {
    unsafe {
        let dictionary = CGSessionCopyCurrentDictionary();
        if dictionary.is_null() {
            return false;
        }
        let key = NSString::from_str("CGSSessionScreenIsLocked");
        let object: *mut objc2::runtime::AnyObject =
            objc2::msg_send![dictionary.cast::<objc2::runtime::AnyObject>(),objectForKey:&*key];
        let locked = if object.is_null() {
            false
        } else {
            objc2::msg_send![object, boolValue]
        };
        CFRelease(dictionary);
        locked
    }
}
