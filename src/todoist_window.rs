//! One shared task controller, with no ownership of browser tabs or PTYs.
use super::*;
use gpui::{Entity, Pixels, Size, WeakEntity};

#[derive(Clone)]
struct Shared {
    model: Entity<Tasks>,
    floating: Option<gpui::WindowHandle<Floating>>,
    browsers: Vec<WeakEntity<Schnauz>>,
    dock_target: Option<WeakEntity<Schnauz>>,
}
impl gpui::Global for Shared {}

pub(super) struct Tasks {
    pub projects: projects::Library,
    pub todoist: todoist::State,
    pub floating: bool,
    pub compact: bool,
    pub host: Option<WeakEntity<Schnauz>>,
    pub notice: Option<String>,
    notice_serial: u64,
    scrolls: HashMap<gpui::WindowId, UniformListScrollHandle>,
}
impl Tasks {
    fn new(cx: &mut Context<Self>) -> Self {
        cx.observe_global::<ProjectLibraryGlobal>(|this, cx| {
            let library = cx.global::<ProjectLibraryGlobal>().0.clone();
            if this.projects.active != library.active {
                for scroll in this.scrolls.values() {
                    scroll.scroll_to_item(0, ScrollStrategy::Top);
                }
            }
            this.projects = library;
            this.todoist.open = visible(cx);
            this.ensure_todoist(false, cx);
            cx.notify();
        })
        .detach();
        cx.observe_global::<todoist::Global>(|this, cx| {
            let global = cx.global::<todoist::Global>().clone();
            if this.todoist.observe_account(&global).0 {
                this.notice = None;
            }
            if global.account.is_none() {
                close_floating(cx);
            } else {
                this.todoist.open = visible(cx);
                this.ensure_todoist(false, cx);
            }
            cx.notify();
        })
        .detach();
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_secs(30))
                .await;
            if this
                .update(cx, |this, cx| {
                    this.todoist.open = visible(cx);
                    let active = objc2::MainThreadMarker::new().is_some_and(|m| {
                        objc2_app_kit::NSApplication::sharedApplication(m).isActive()
                    });
                    if this.todoist.open && (active || is_floating(cx)) {
                        this.ensure_todoist(false, cx);
                    }
                })
                .is_err()
            {
                break;
            }
        })
        .detach();
        Self {
            projects: cx.global::<ProjectLibraryGlobal>().0.clone(),
            todoist: todoist::State::default(),
            floating: false,
            compact: false,
            host: None,
            notice: None,
            notice_serial: 0,
            scrolls: HashMap::new(),
        }
    }
    pub(super) fn persist_library(
        &mut self,
        candidate: projects::Library,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if let Some(error) = &cx.global::<ProjectLibraryGlobal>().1 {
            return Err(error.clone());
        }
        let path = config_file("projects.json").ok_or("Cannot find the settings folder.")?;
        candidate
            .save(&path)
            .map_err(|e| format!("Couldn't save the project: {e}"))?;
        self.projects = candidate.clone();
        cx.set_global(ProjectLibraryGlobal(candidate, None));
        Ok(())
    }
    pub(super) fn set_notice(&mut self, message: String, cx: &mut Context<Self>) {
        self.notice = Some(message.clone());
        self.notice_serial = self.notice_serial.wrapping_add(1);
        let serial = self.notice_serial;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(8)).await;
            let _ = this.update(cx, |this, cx| {
                if this.notice_serial == serial {
                    this.notice = None;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
        let mut state = cx.global::<todoist::Global>().clone();
        state.notice = Some(message);
        cx.set_global(state);
    }
}
pub(super) fn initialize(cx: &mut Context<Schnauz>) {
    cx.on_release(|_, cx| visibility_changed(cx)).detach();
    if cx.try_global::<Shared>().is_none() {
        let model = cx.new(Tasks::new);
        cx.set_global(Shared {
            model,
            floating: None,
            browsers: Vec::new(),
            dock_target: None,
        });
    }
    let model = shared(cx);
    cx.observe(&model, |_, _, cx| cx.notify()).detach();
    cx.observe_global::<Shared>(|this, cx| {
        if is_floating(cx) {
            this.todoist.open = false;
        }
        cx.notify();
    })
    .detach();
    let mut state = cx.global::<Shared>().clone();
    state.browsers.retain(|b| b.upgrade().is_some());
    state.browsers.push(cx.weak_entity());
    cx.set_global(state);
}
pub(super) fn shared(cx: &App) -> Entity<Tasks> {
    cx.global::<Shared>().model.clone()
}
pub(super) fn is_floating(cx: &App) -> bool {
    cx.try_global::<Shared>()
        .is_some_and(|s| s.floating.is_some())
}
fn visible(cx: &App) -> bool {
    cx.try_global::<Shared>().is_some_and(|s| {
        s.floating.is_some()
            || s.browsers
                .iter()
                .filter_map(|b| b.upgrade())
                .any(|b| b.read(cx).todoist.open)
    })
}

// Run after the originating browser/controller borrow and close effects finish.
pub(super) fn visibility_changed(cx: &mut App) {
    cx.defer(|cx| {
        if cx.try_global::<Shared>().is_some() {
            let open = visible(cx);
            shared(cx).update(cx, |tasks, _| tasks.todoist.open = open);
        }
    });
}

#[derive(IntoElement)]
struct Panel {
    model: Entity<Tasks>,
    width: f32,
    floating: bool,
    compact: bool,
    host: Option<WeakEntity<Schnauz>>,
}
impl RenderOnce for Panel {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = window.window_handle().window_id();
        self.model.update(cx, |tasks, cx| {
            tasks.projects = cx.global::<ProjectLibraryGlobal>().0.clone();
            tasks
                .scrolls
                .retain(|id, _| cx.windows().iter().any(|w| w.window_id() == *id));
            tasks.todoist.scroll = tasks.scrolls.entry(id).or_default().clone();
            tasks.floating = self.floating;
            tasks.compact = self.compact;
            tasks.host = self.host;
            tasks.render_todoist(self.width, cx)
        })
    }
}
pub(super) fn panel(width: f32, cx: &Context<Schnauz>) -> AnyElement {
    Panel {
        model: shared(cx),
        width,
        floating: false,
        compact: false,
        host: Some(cx.weak_entity()),
    }
    .into_any_element()
}
pub(super) fn close_panel(host: Option<WeakEntity<Schnauz>>, window: &mut Window, cx: &mut App) {
    if let Some(browser) = host.and_then(|b| b.upgrade()) {
        browser.update(cx, |b, cx| {
            b.todoist.open = false;
            cx.notify();
        });
    } else {
        window.remove_window();
    }
    visibility_changed(cx);
}
pub(super) fn show_floating(cx: &mut App) {
    if let Some(handle) = cx.try_global::<Shared>().and_then(|s| s.floating) {
        let _ = handle.update(cx, |_, window, _| window.activate_window());
    }
}
fn close_floating(cx: &mut App) {
    if let Some(handle) = cx.try_global::<Shared>().and_then(|s| s.floating) {
        let mut state = cx.global::<Shared>().clone();
        state.floating = None;
        cx.set_global(state);
        visibility_changed(cx);
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        });
    }
}
pub(super) fn dock_here(browser: WeakEntity<Schnauz>, cx: &mut App) {
    let Some(browser) = browser.upgrade() else {
        return;
    };
    close_floating(cx);
    browser.update(cx, |b, cx| {
        b.open_todoist(cx);
        cx.notify();
    });
}
pub(super) fn detach(host: Option<WeakEntity<Schnauz>>, window: &mut Window, cx: &mut App) {
    if is_floating(cx) {
        if window.is_window_active() {
            show_floating(cx);
        }
        return;
    }
    let bounds = Bounds::centered(None, size(px(400.), px(560.)), cx);
    let model = shared(cx);
    let result = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(320.), px(150.))),
            titlebar: Some(TitlebarOptions {
                title: Some("Schnauz · Todoist".into()),
                appears_transparent: true,
                ..Default::default()
            }),
            show: window.is_window_active(),
            focus: window.is_window_active(),
            ..Default::default()
        },
        move |window, cx| {
            configure_native(window);
            let window_id = window.window_handle().window_id();
            window.on_window_should_close(cx, |_, cx| {
                let mut state = cx.global::<Shared>().clone();
                state.floating = None;
                cx.set_global(state);
                visibility_changed(cx);
                true
            });
            cx.new(|cx| {
                cx.observe(&model, |_, _, cx| cx.notify()).detach();
                cx.observe_global::<ThemeGlobal>(|_, cx| cx.notify())
                    .detach();
                cx.on_release(move |_, cx| {
                    if let Some(state) = cx.try_global::<Shared>() {
                        if state.floating.is_some_and(|w| w.window_id() == window_id) {
                            let mut state = state.clone();
                            state.floating = None;
                            cx.set_global(state);
                            visibility_changed(cx);
                        }
                    }
                })
                .detach();
                Floating {
                    model,
                    compact: false,
                    full_size: size(px(400.), px(560.)),
                }
            })
        },
    );
    match result {
        Ok(handle) => {
            let mut state = cx.global::<Shared>().clone();
            state.floating = Some(handle);
            state.dock_target = host;
            // Do this before notifying; no duplicate embedded list in any browser.
            for browser in state.browsers.iter().filter_map(|b| b.upgrade()) {
                browser.update(cx, |b, cx| {
                    b.todoist.open = false;
                    cx.notify();
                });
            }
            cx.set_global(state);
            cx.defer(|cx| {
                shared(cx).update(cx, |tasks, cx| {
                    tasks.todoist.open = true;
                    tasks.ensure_todoist(false, cx);
                })
            });
        }
        Err(error) => {
            if let Some(browser) = host.and_then(|b| b.upgrade()) {
                browser.update(cx, |b, cx| {
                    b.set_notice(format!("Couldn't detach Todoist: {error}"), cx)
                });
            }
        }
    }
}
fn configure_native(window: &Window) {
    use objc2_app_kit::{NSFloatingWindowLevel, NSView, NSWindowCollectionBehavior};
    let Some(pointer) = ns_view_ptr(window) else {
        return;
    };
    // GPUI's WindowKind::Floating does not set AppKit's floating window level.
    let view = unsafe { &*pointer.cast::<NSView>() };
    if let Some(native) = view.window() {
        native.setLevel(NSFloatingWindowLevel);
        native.setHidesOnDeactivate(false);
        native.setTabbingMode(objc2_app_kit::NSWindowTabbingMode::Disallowed);
        let mut behavior = NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary;
        if objc2_foundation::NSProcessInfo::processInfo().isOperatingSystemAtLeastVersion(
            objc2_foundation::NSOperatingSystemVersion {
                majorVersion: 13,
                minorVersion: 0,
                patchVersion: 0,
            },
        ) {
            behavior |= NSWindowCollectionBehavior::CanJoinAllApplications;
        }
        native.setCollectionBehavior(behavior);
    }
}
struct Floating {
    model: Entity<Tasks>,
    compact: bool,
    full_size: Size<Pixels>,
}
impl Floating {
    fn toggle_compact(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.compact {
            window.resize(self.full_size);
        } else {
            self.full_size = window.viewport_size();
            window.resize(size(self.full_size.width, px(160.)));
        }
        self.compact = !self.compact;
        cx.notify();
    }
}
impl Render for Floating {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme();
        let width = f32::from(window.viewport_size().width);
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(t.bg))
            .text_color(rgb(t.text))
            .text_size(px(UI_FONT_SIZE))
            .child(
                div()
                    .h(px(48.))
                    .window_control_area(gpui::WindowControlArea::Drag)
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_1()
                    .pl(px(82.))
                    .pr_2()
                    .child(div().flex_1().min_w_0().truncate().child("Todoist"))
                    .when(!self.compact, |header| {
                        header
                            .child(
                                div()
                                    .id("todoist-floating-relink")
                                    .px_1()
                                    .py_1()
                                    .text_size(px(UI_SMALL_FONT_SIZE))
                                    .cursor_pointer()
                                    .text_color(rgb(t.accent))
                                    .child("Relink…")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.model
                                            .update(cx, |tasks, cx| tasks.link_todoist(true, cx))
                                    })),
                            )
                            .child(
                                div()
                                    .id("todoist-floating-refresh")
                                    .size(px(24.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .child(ui_icon(Icon::Reload, 14., rgb(t.accent)))
                                    .tooltip(tip("Refresh Todoist tasks"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.model
                                            .update(cx, |tasks, cx| tasks.ensure_todoist(true, cx))
                                    })),
                            )
                    })
                    .child(
                        div()
                            .id("todoist-compact")
                            .relative()
                            .children(ui_checks::probe("todoist-compact-toggle"))
                            .px_2()
                            .py_1()
                            .cursor_pointer()
                            .text_color(rgb(t.accent))
                            .child(if self.compact { "Expand" } else { "Compact" })
                            .on_click(
                                cx.listener(|this, _, window, cx| this.toggle_compact(window, cx)),
                            ),
                    )
                    .child(
                        div()
                            .id("todoist-dock")
                            .relative()
                            .children(ui_checks::probe("todoist-dock-button"))
                            .size(px(26.))
                            .rounded_md()
                            .hover(|d|d.bg(rgb(t.hover)))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .child(ui_icon(Icon::PinFloating, 16., rgb(t.text_muted)))
                            .tooltip(tip("Floating · Dock Todoist"))
                            .on_click(|_, _, cx| {
                                let state = cx.global::<Shared>().clone();
                                if let Some(target) = state
                                    .dock_target
                                    .filter(|b| b.upgrade().is_some())
                                    .or_else(|| {
                                        state
                                            .browsers
                                            .iter()
                                            .find(|b| b.upgrade().is_some())
                                            .cloned()
                                    })
                                {
                                    dock_here(target, cx);
                                } else if browser_windows::open(None, cx).is_ok() {
                                    let target = cx.global::<Shared>().browsers.last().cloned();
                                    if let Some(target) = target {
                                        dock_here(target, cx);
                                    }
                                }
                            }),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_hidden()
                    .child(Panel {
                        model: self.model.clone(),
                        width,
                        floating: true,
                        compact: self.compact,
                        host: None,
                    }),
            )
    }
}

#[cfg(feature = "ui-checks")]
#[path = "todoist_window_checks.rs"]
mod checks;
#[cfg(feature = "ui-checks")]
pub(super) use checks::check;
