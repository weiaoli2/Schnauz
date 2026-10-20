//! Explicit next-launch setup, separate from durable projects and live PTYs.
use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Terminal {
    pub cwd: PathBuf,
    pub server: Option<SftpServer>,
    pub folder: PathBuf,
    pub visible: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct SavedTab {
    location: PathBuf,
    server: Option<SftpServer>,
    #[serde(default)]
    network:Option<network::Location>,
    style: folder_views::FolderView,
    filter: EntryFilter,
    hidden: bool,
    selection: HashSet<PathBuf>,
    anchor: Option<PathBuf>,
    terminal: Option<Terminal>,
    terminal_height: f32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct SavedPane {
    tabs: Vec<SavedTab>,
    active: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Layout {
    panes: Vec<SavedPane>,
    active: usize,
    ratio: f32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Project {
    id: Option<u64>,
    layout: Layout,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct SavedWindow {
    current: Layout,
    parked: Vec<Project>,
    frame: [f32; 4],
    tasks: bool,
    tools_added: f32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Setup {
    version: u8,
    pub(super) restore: bool,
    project: Option<u64>,
    pub windows: Vec<SavedWindow>,
}
fn saved_server(server: &SftpServer) -> SftpServer {
    let mut server = server.clone();
    server.control_path = None;
    server
}
fn capture(panes: &[Pane], active: usize, ratio: f32) -> Layout {
    Layout {
        active,
        ratio,
        panes: panes
            .iter()
            .map(|p| SavedPane {
                active: p.active,
                tabs: p
                    .tabs
                    .iter()
                    .map(|t| SavedTab {
                        location: t.current_dir.clone(),
                        server: t.remote.as_ref().map(saved_server),
                    network:t.network.clone(),
                        style: folder_views::FolderView {
                            view: t.view,
                            list: t.list_layout.clone(),
                            preview_width: t.preview_width,
                            sort: sort_key_name(t.sort_key).into(),
                            ascending: t.sort_asc,
                        },
                        filter: t.entry_filter,
                        hidden: t.show_hidden,
                        selection: t.selection.clone(),
                        anchor: t.anchor.clone(),
                        terminal_height: t.terminal_height,
                        terminal: t
                            .terminal
                            .as_ref()
                            .map(|s| Terminal {
                                cwd: s.cwd.clone(),
                                server: s.ssh.as_ref().map(|s| saved_server(&s.server)),
                                folder: s
                                    .ssh
                                    .as_ref()
                                    .and_then(|s| s.folder.clone())
                                    .unwrap_or_else(|| t.current_dir.clone()),
                                visible: t.terminal_open,
                            })
                            .or_else(|| t.terminal_restore.clone()),
                    })
                    .collect(),
            })
            .collect(),
    }
}
fn restore(layout: &Layout) -> schnauz::ProjectSession {
    let mut panes = Vec::new();
    for p in layout.panes.iter().take(2) {
        let mut tabs = Vec::new();
        for saved in p.tabs.iter().take(256) {
            if !saved.location.is_absolute() {
                continue;
            }
            // Remote namespaces must never be validated against local files.
            // Missing local folders remain visible with the usual read error.
            let mut tab = Tab::at_network_location(
                saved.location.clone(),
                saved.server.as_ref().map(saved_server),saved.network.clone(),
            );
            list_ui::apply_style(&mut tab, &saved.style);
            tab.entry_filter = saved.filter;
            tab.show_hidden = saved.hidden;
            tab.selection = saved.selection.clone();
            tab.anchor = saved.anchor.clone();
            tab.terminal_height = if saved.terminal_height.is_finite() {
                saved.terminal_height.clamp(150., 1200.)
            } else {
                290.
            };
            tab.terminal_open = saved.terminal.as_ref().is_some_and(|t| t.visible);
            tab.terminal_restore = saved.terminal.clone();
            tab.recompute_results();
            tabs.push(tab);
        }
        if !tabs.is_empty() {
            panes.push(Pane {
                active: p.active.min(tabs.len() - 1),
                tabs,
            });
        }
    }
    if panes.is_empty() {
        panes.push(Pane::new(home_dir()));
    }
    schnauz::ProjectSession {
        active_pane: layout.active.min(panes.len() - 1),
        panes,
        split_ratio: if layout.ratio.is_finite() {
            layout.ratio.clamp(0.2, 0.8)
        } else {
            0.5
        },
    }
}
impl SavedWindow {
    pub(super) fn capture(b: &Schnauz, frame: [f32; 4]) -> Self {
        Self {
            current: capture(&b.panes, b.active_pane, b.split_ratio),
            parked: b
                .project_sessions
                .iter()
                .map(|(id, s)| Project {
                    id: *id,
                    layout: capture(&s.panes, s.active_pane, s.split_ratio),
                })
                .collect(),
            frame,
            tasks: b.todoist.open,
            tools_added: b.tools_expansion.saved_added(),
        }
    }
    pub(super) fn apply(&self, b: &mut Schnauz, cx: &mut Context<Schnauz>) {
        let current = restore(&self.current);
        b.panes = current.panes;
        b.active_pane = current.active_pane;
        b.split_ratio = current.split_ratio;
        b.project_sessions = self
            .parked
            .iter()
            .filter(|p| {
                p.id != b.projects.active
                    && p.id
                        .is_none_or(|id| b.projects.items.iter().any(|p| p.id == id))
            })
            .take(128)
            .map(|p| (p.id, restore(&p.layout)))
            .collect();
        if self.tasks && cx.global::<todoist::Global>().account.is_some() {
            b.open_todoist(cx);
        }
        b.tools_expansion = preview_panel::Expansion::restored(b.todoist.open, self.tools_added);
        b.saved_session = b.session_string();
    }
    pub(super) fn bounds(&self, cx: &App) -> Bounds<gpui::Pixels> {
        let fallback = Bounds::centered(None, size(px(1280.), px(800.)), cx);
        if self.frame.iter().any(|v| !v.is_finite()) {
            return fallback;
        }
        let origin = gpui::point(px(self.frame[0]), px(self.frame[1]));
        let display = cx
            .displays()
            .into_iter()
            .find(|d| d.bounds().contains(&origin))
            .or_else(|| cx.primary_display());
        let Some(display) = display else {
            return fallback;
        };
        let screen = display.bounds();
        let w = self.frame[2].max(980.).min(f32::from(screen.size.width));
        let h = self.frame[3].max(600.).min(f32::from(screen.size.height));
        Bounds::new(
            gpui::point(
                px(self.frame[0].clamp(f32::from(screen.left()), f32::from(screen.right()) - w)),
                px(self.frame[1].clamp(f32::from(screen.top()), f32::from(screen.bottom()) - h)),
            ),
            size(px(w), px(h)),
        )
    }
}
pub(super) fn save(keep: bool, cx: &mut App) -> Result<(), String> {
    let mut windows = Vec::new();
    if keep {
        for handle in cx.windows() {
            if let Some(handle) = handle.downcast::<Schnauz>() {
                if let Ok(saved) = handle.update(cx, |b, w, _| {
                    let frame = w.bounds();
                    SavedWindow::capture(
                        b,
                        [
                            f32::from(frame.origin.x),
                            f32::from(frame.origin.y),
                            f32::from(frame.size.width),
                            f32::from(frame.size.height),
                        ],
                    )
                }) {
                    windows.push(saved);
                }
            }
        }
        // Hidden QA embeds the browser in a fixture root rather than a normal window.
        #[cfg(feature = "ui-checks")]
        if windows.is_empty() {
            for b in quit_guard::browsers(cx) {
                windows.push(SavedWindow::capture(b.read(cx), [0., 0., 1280., 800.]));
            }
        }
    }
    let setup = Setup {
        version: 1,
        restore: keep,
        project: cx.global::<ProjectLibraryGlobal>().0.active,
        windows,
    };
    let bytes = serde_json::to_vec_pretty(&setup).map_err(|e| e.to_string())?;
    let path = config_file("startup-setup.json").ok_or("Cannot find the settings folder.")?;
    projects::atomic_write(&path, &bytes)
        .map_err(|e| format!("Couldn't save the next-start setup: {e}"))
}
pub(super) fn load() -> Result<Option<Setup>, String> {
    use std::io::Read;
    let Some(path) = config_file("startup-setup.json") else {
        return Ok(None);
    };
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("Couldn't read the saved setup: {e}")),
    };
    let mut bytes = Vec::new();
    file.take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("The saved setup is too large; it has been preserved.".into());
    }
    let setup: Setup = serde_json::from_slice(&bytes)
        .map_err(|_| "The saved setup is invalid; it has been preserved.".to_string())?;
    if setup.version != 1 || setup.windows.len() > 32 {
        return Err(
            "The saved setup version or window count is invalid; it has been preserved.".into(),
        );
    }
    Ok(Some(setup))
}
/// None means legacy startup; an empty vector explicitly requests a fresh start.
pub(super) fn startup(cx: &mut App) -> Result<Option<Vec<SavedWindow>>, String> {
    let Some(setup) = load()? else {
        return Ok(None);
    };
    browser_windows::initialize_project_library(cx);
    let current = cx.global::<ProjectLibraryGlobal>();
    let mut shared = ProjectLibraryGlobal(current.0.clone(), current.1.clone());
    shared.0.active = setup
        .restore
        .then_some(setup.project)
        .flatten()
        .filter(|id| shared.0.items.iter().any(|p| p.id == *id));
    cx.set_global(shared);
    Ok(Some(if setup.restore {
        setup.windows
    } else {
        Vec::new()
    }))
}

impl Schnauz {
    pub(super) fn restore_terminals(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for pane in 0..self.panes.len() {
            for index in 0..self.panes[pane].tabs.len() {
                if self.panes[pane].tabs[index].terminal_restore_attempted {
                    continue;
                }
                let Some(saved) = self.panes[pane].tabs[index].terminal_restore.clone() else {
                    continue;
                };
                self.panes[pane].tabs[index].terminal_restore_attempted = true;
                if self.panes[pane].tabs[index].terminal.is_some() {
                    continue;
                }
                if let Some(server) = saved.server {
                    self.start_remote_terminal_at(
                        pane,
                        index,
                        server,
                        saved.folder,
                        Some(saved.visible),
                        window,
                        cx,
                    );
                } else {
                    let cwd = if saved.cwd.is_dir() {
                        saved.cwd
                    } else {
                        home_dir()
                    };
                    self.start_terminal_at(pane, index, cwd, cx);
                    self.panes[pane].tabs[index].terminal_open = saved.visible;
                }
            }
        }
    }
}

#[cfg(feature = "ui-checks")]
thread_local! {static FIXTURE_SHELL:std::cell::Cell<bool>=const{std::cell::Cell::new(false)};}
#[cfg(feature = "ui-checks")]
pub(super) fn fixture_shell() -> bool {
    FIXTURE_SHELL.with(|f| f.get())
}

#[cfg(feature = "ui-checks")]
pub(super) fn check(cx: &mut App, done: impl FnOnce(&mut App) + 'static) {
    let root = config_dir().unwrap().join("setup-restore-fixture");
    fs::create_dir_all(root.join("browser")).unwrap();
    fs::create_dir_all(root.join("shell")).unwrap();
    let layout = Layout {
        active: 0,
        ratio: 0.63,
        panes: vec![SavedPane {
            active: 1,
            tabs: vec![
                SavedTab {
                    location: root.join("browser"),
                    network:None,
                    server: None,
                    style: Default::default(),
                    filter: EntryFilter::All,
                    hidden: true,
                    selection: HashSet::new(),
                    anchor: None,
                    terminal: Some(Terminal {
                        cwd: root.join("shell"),
                        server: None,
                        folder: root.join("shell"),
                        visible: false,
                    }),
                    terminal_height: 211.,
                },
                SavedTab {
                    location: root.join("browser"),
                    network:None,
                    server: None,
                    style: Default::default(),
                    filter: EntryFilter::All,
                    hidden: false,
                    selection: HashSet::new(),
                    anchor: None,
                    terminal: Some(Terminal {
                        cwd: root.join("browser"),
                        server: None,
                        folder: root.join("browser"),
                        visible: true,
                    }),
                    terminal_height: 237.,
                },
            ],
        }],
    };
    let saved = SavedWindow {
        current: layout.clone(),
        parked: vec![Project { id: None, layout }],
        frame: [-20000., -20000., 1100., 760.],
        tasks: false,
        tools_added: 0.,
    };
    FIXTURE_SHELL.with(|f| f.set(true));
    let window = cx
        .open_window(
            WindowOptions {
                show: false,
                focus: false,
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    gpui::point(px(-20000.), px(-20000.)),
                    size(px(1100.), px(760.)),
                ))),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|cx| {
                    let mut b = Schnauz::new(
                        projects::Shortcut::local(root.join("browser"), projects::Section::Folders),
                        cx,
                    );
                    saved.apply(&mut b, cx);
                    b
                })
            },
        )
        .unwrap();
    FIXTURE_SHELL.with(|f| f.set(true));
    for _ in 0..3 {
        cx.update_window(*window, |_, w, cx| {
            w.refresh();
            w.draw(cx).clear();
        })
        .unwrap();
    }
    let ids = window
        .update(cx, |b, _, _| {
            assert_eq!(b.panes[0].active, 1);
            assert_eq!(b.split_ratio, 0.63);
            assert!(!b.panes[0].tabs[0].terminal_open);
            assert!(b.panes[0].tabs[1].terminal_open);
            assert_eq!(b.panes[0].tabs[0].terminal_height, 211.);
            b.panes[0]
                .tabs
                .iter()
                .map(|t| t.terminal.as_ref().unwrap().fixture_pid())
                .collect::<Vec<_>>()
        })
        .unwrap();
    cx.spawn(async move|cx| {
        cx.background_executor().timer(Duration::from_millis(150)).await;
        cx.update(|cx| {
            window.update(cx,|b,_,_| {
                assert_eq!(b.panes[0].tabs[0].current_dir,root.join("browser"));
                assert_eq!(b.panes[0].tabs[0].terminal.as_ref().unwrap().cwd,root.join("shell"));
                assert_ne!(ids[0],ids[1],"Each restored terminal owns a fresh separate shell");
                for pid in &ids {assert_eq!(unsafe{libc::kill(*pid as i32,0)},0);}
                let saved=SavedWindow::capture(b,[0.,0.,1100.,760.]);
                let decoded:SavedWindow=serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
                assert_eq!(decoded.current.panes[0].tabs.len(),2);
                assert!(!decoded.current.panes[0].tabs[0].terminal.as_ref().unwrap().visible);
            }).unwrap();
            window.update(cx,|_,w,_|w.remove_window()).unwrap();
            FIXTURE_SHELL.with(|f|f.set(false));
            println!("PASS setup roundtrip starts distinct fresh fixture shells, restores hidden/visible terminal panels, heights, selected tab and unsynced browser/shell folders; no live sessions or SSH connections");
            done(cx);
        }).unwrap();
    }).detach();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn setup_roundtrip_preserves_remote_tabs_without_local_reads_or_live_sockets() {
        let root = crate::tests::scratch("startup-setup");
        fs::write(root.join("local-only.txt"), "not a remote file").unwrap();
        let server = SftpServer {
            host: "fixture.invalid".into(),
            user: "fixture".into(),
            ssh_alias: "saved-host".into(),
            control_path: Some("/tmp/private-live-mux".into()),
            ..Default::default()
        };
        let mut tab = Tab::at_location(root.clone(), Some(server.clone()));
        tab.terminal_restore = Some(Terminal {
            cwd: root.clone(),
            server: Some(server),
            folder: "/remote/with spaces".into(),
            visible: true,
        });
        tab.terminal_open = true;
        tab.terminal_height = 243.;
        tab.show_hidden = true;
        tab.selection.insert(root.join("selected.txt"));
        tab.anchor = Some(root.join("selected.txt"));
        let captured = capture(
            &[Pane {
                tabs: vec![tab],
                active: 0,
            }],
            0,
            0.62,
        );
        let bytes = serde_json::to_vec(&captured).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("private-live-mux"));
        let decoded: Layout = serde_json::from_slice(&bytes).unwrap();
        let restored = restore(&decoded);
        let tab = &restored.panes[0].tabs[0];
        assert!(
            tab.entries.is_empty(),
            "A restored SSH path must not list its local namesake"
        );
        assert_eq!(tab.remote.as_ref().unwrap().ssh_alias, "saved-host");
        assert!(tab.remote.as_ref().unwrap().control_path.is_none());
        assert!(
            tab.terminal.is_none(),
            "Deserialization never starts a process or connects"
        );
        assert!(tab.terminal_open && tab.show_hidden);
        assert_eq!(tab.terminal_height, 243.);
        assert_eq!(tab.anchor, Some(root.join("selected.txt")));
        assert_eq!(
            tab.terminal_restore.as_ref().unwrap().folder,
            PathBuf::from("/remote/with spaces")
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn unavailable_local_tabs_stay_visible_and_invalid_layout_sizes_are_bounded() {
        let root = crate::tests::scratch("setup-missing");
        let tab = Tab::new(root.join("missing"));
        let mut layout = capture(
            &[Pane {
                tabs: vec![tab],
                active: 0,
            }],
            99,
            f32::NAN,
        );
        layout.panes[0].active = 99;
        layout.panes[0].tabs[0].terminal_height = f32::NAN;
        let restored = restore(&layout);
        let tab = &restored.panes[0].tabs[0];
        assert_eq!(restored.active_pane, 0);
        assert_eq!(restored.split_ratio, 0.5);
        assert_eq!(restored.panes[0].active, 0);
        assert_eq!(tab.terminal_height, 290.);
        assert!(tab.listing_error.is_some());
        assert_eq!(tab.current_dir, root.join("missing"));
        fs::remove_dir_all(root).unwrap();
    }
}
