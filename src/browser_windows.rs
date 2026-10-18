//! Additional browser windows share the active project, never terminal sessions.
use super::*;
use projects::Shortcut;

pub(super) struct ProjectLibraryGlobal(pub projects::Library, pub Option<String>);
impl gpui::Global for ProjectLibraryGlobal {}

pub(super) struct FolderViewsGlobal(pub Rc<RefCell<folder_views::Library>>);
impl gpui::Global for FolderViewsGlobal {}

impl Schnauz {
    pub(super) fn folder_open_nodes(
        &self,
        pane: usize,
        folder: Shortcut,
        include_open: bool,
    ) -> Vec<MenuNode> {
        let mut folder = folder;
        if let Some(server) = folder.server.take() {
            folder.server = Some(self.favourite_server(server));
        }
        let mut items = Vec::new();
        if include_open {
            let folder = folder.clone();
            items.push(mi("Open", move |this, window, cx| {
                this.close_context_menu(cx);
                this.close_sidebar_menu(cx);
                window.focus(&this.focus);
                this.active_pane = pane;
                this.open_favourite(folder.clone(), cx);
            }));
        }
        let tab_folder = folder.clone();
        items.push(mi("Open in New Tab", move |this, window, cx| {
            this.close_context_menu(cx);
            this.close_sidebar_menu(cx);
            window.focus(&this.focus);
            if this.palette_open && this.file_search.inline {
                this.close_palette(cx);
            }
            let tab = Tab::at_location(tab_folder.path.clone(), tab_folder.server.clone());
            let target = this.pane_mut(pane);
            target.tabs.push(tab);
            target.active = target.tabs.len() - 1;
            this.active_pane = pane;
            this.apply_folder_view(pane, false);
            this.reload_pane(pane, cx);
        }));
        items.push(mi("Open in New Window", move |this, _, cx| {
            this.close_context_menu(cx);
            this.close_sidebar_menu(cx);
            if let Err(error) = open(Some(folder.clone()), cx) {
                this.set_notice(error, cx);
            }
        }));
        items
    }

    pub(super) fn observe_project_library(&mut self, cx: &mut Context<Self>) {
        let shared = cx.global::<ProjectLibraryGlobal>().0.clone();
        if shared == self.projects {
            return;
        }
        let old = self.projects.active;
        let changed_project = old != shared.active;
        let changed_link = self.projects.sidebar().todoist != shared.sidebar().todoist;
        self.projects = shared;
        if changed_project {
            let layout = self
                .projects
                .active
                .and_then(|id| {
                    self.projects
                        .items
                        .iter()
                        .find(|p| p.id == id)
                        .map(|p| p.layout.clone())
                })
                .or_else(|| self.projects.personal_layout.clone())
                .unwrap_or_else(|| self.capture_layout());
            if let Some(available) = layout.available() {
                self.apply_project_switch(old, &available, cx);
            } else {
                self.set_notice(
                    "This project's folders are unavailable. Reconnect the drive and try again."
                        .into(),
                    cx,
                );
            }
        } else if changed_link {
            self.ensure_todoist(false, cx);
        }
        cx.notify();
    }
}

pub(super) fn open(folder: Option<Shortcut>, cx: &mut App) -> Result<(), String> {
    let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(980.), px(600.))),
            titlebar: Some(TitlebarOptions {
                title: None,
                appears_transparent: true,
                ..Default::default()
            }),
            ..Default::default()
        },
        move |window, cx| {
            let view = cx.new(|cx| {
                let startup = folder.is_none();
                let location = folder.unwrap_or_else(|| {
                    Shortcut::local(load_last_dir(), projects::Section::Folders)
                });
                let mut finder = Schnauz::new(location, cx);
                if startup {
                    finder.restore_session();
                    cx.set_global(ProjectLibraryGlobal(
                        finder.projects.clone(),
                        finder.project_load_error.clone(),
                    ));
                }
                finder.activate_project_sidebar(cx);
                finder.apply_all_folder_views();
                if !startup {
                    // Opening a window must not replace the existing saved layout.
                    finder.saved_session = finder.session_string();
                }
                for pane in 0..finder.panes.len() {
                    finder.reload_pane(pane, cx);
                }
                finder.prewarm_icons(cx);
                // Additional windows browse only the requested folder. Do not
                // duplicate startup connections or the original window's PTYs.
                if startup {
                    for server in sftp_servers().into_iter().filter(|s| s.auto_reopen) {
                        finder.connect_sftp(server, cx);
                    }
                }
                finder
            });
            window.focus(&view.read(cx).focus);
            view
        },
    )
    .map(|_| ())
    .map_err(|error| format!("Couldn't open a Schnauz window: {error}"))
}
