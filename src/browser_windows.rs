//! Additional browser windows share the active project, never terminal sessions.
use super::*;
use projects::Shortcut;

pub(super) struct ProjectLibraryGlobal(pub projects::Library, pub Option<String>);
impl gpui::Global for ProjectLibraryGlobal {}

pub(super) fn initialize_project_library(cx:&mut App) {
    if cx.try_global::<ProjectLibraryGlobal>().is_some() {return;}
    let loaded=config_dir().ok_or_else(||"Cannot find the settings folder.".to_string()).and_then(|dir|
        projects::Library::load_migrating(&dir.join("projects.json"),&dir.join("workspaces.json"),read_path_list("bookmarks.txt"))
            .map_err(|e|format!("Couldn't read projects: {e}. The existing file has been preserved.")));
    let (mut library,mut error)=match loaded {Ok(l)=>(l,None),Err(e)=>(projects::Library::default(),Some(e))};
    if error.is_none() && library.migrate_sidebar(load_sftp_servers()) {
        if let Some(path)=config_file("projects.json") {
            if let Err(e)=library.save(&path) {error=Some(format!("Couldn't migrate project sidebar: {e}"));}
        }
    }
    cx.set_global(ProjectLibraryGlobal(library,error));
}

pub(super) struct FolderViewsGlobal(pub Rc<RefCell<folder_views::Library>>);
impl gpui::Global for FolderViewsGlobal {}

/// Runtime activity only: saved layouts do not imply an open project.
#[derive(Clone, Default)]
pub(super) struct ProjectActivity(pub HashMap<gpui::EntityId, HashSet<Option<u64>>>);
impl gpui::Global for ProjectActivity {}

impl Schnauz {
    pub(super) fn live_projects(&self) -> HashSet<Option<u64>> {
        let mut live = HashSet::new();
        if self.panes.iter().any(|p| !p.tabs.is_empty()) {
            live.insert(self.projects.active);
        }
        for (id, session) in &self.project_sessions {
            if session.panes.iter().any(|p| !p.tabs.is_empty()) {
                live.insert(*id);
            }
        }
        live
    }

    pub(super) fn publish_project_activity(&self, cx: &mut Context<Self>) {
        let live = self.live_projects();
        let id = cx.entity_id();
        if cx.global::<ProjectActivity>().0.get(&id) != Some(&live) {
            let mut activity = cx.global::<ProjectActivity>().clone();
            activity.0.insert(id, live);
            cx.set_global(activity);
        }
    }

    pub(super) fn project_is_open(&self, project: Option<u64>, cx: &App) -> bool {
        self.live_projects().contains(&project)
            || cx
                .try_global::<ProjectActivity>()
                .is_some_and(|a| a.0.values().any(|ids| ids.contains(&project)))
    }

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
            if this.palette_open {
                this.close_palette(cx);
            }
            if tab_folder.network.is_some() {this.active_pane=pane;this.open_network_favourite(tab_folder.clone(),false,false,cx);return;}
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
            if folder.network.is_some() {this.open_network_favourite(folder.clone(),false,true,cx);return;}
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
        if self.apply_project_file_filters() && self.palette_open { self.refresh_file_search(cx); }
        cx.notify();
    }
}

pub(super) fn open(folder: Option<Shortcut>, cx: &mut App) -> Result<(), String> {
    open_with_setup(folder,None,false,cx)
}
pub(super) fn open_restored(setup:startup_setup::SavedWindow,cx:&mut App)->Result<(),String> {
    open_with_setup(None,Some(setup),false,cx)
}
pub(super) fn open_fresh(cx:&mut App)->Result<(),String> { open_with_setup(None,None,true,cx) }
fn open_with_setup(folder:Option<Shortcut>,setup:Option<startup_setup::SavedWindow>,fresh:bool,cx:&mut App)->Result<(),String> {
    let bounds = setup.as_ref().map(|s|s.bounds(cx)).unwrap_or_else(||Bounds::centered(None, size(px(1280.), px(800.)), cx));
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
            window.on_window_should_close(cx, |_,cx| {
                let count=cx.windows().iter().filter(|w|w.downcast::<Schnauz>().is_some()).count();
                if count>1 {true} else {quit_guard::request(cx);false}
            });
            let view = cx.new(|cx| {
                let startup = folder.is_none();
                let location = folder.unwrap_or_else(|| {
                    Shortcut::local(if fresh {home_dir()} else {load_last_dir()}, projects::Section::Folders)
                });
                let connected=location.network.is_some();
                let mut finder = Schnauz::new(location, cx);
                if connected { finder.panes[0].tabs[0].network_connected=true; }
                if startup {
                    if !prefs().file_access_intro_seen {
                        finder.file_access_intro = Some(file_access::Intro::default());
                    }
                    if let Some(setup)=&setup { setup.apply(&mut finder,cx); }
                    else if !fresh { finder.restore_session(); }
                    cx.set_global(ProjectLibraryGlobal(
                        finder.projects.clone(),
                        finder.project_load_error.clone(),
                    ));
                }
                finder.activate_project_sidebar(cx);
                if setup.is_none() { finder.apply_all_folder_views(); }
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
                if startup && setup.is_none() && !fresh {
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
