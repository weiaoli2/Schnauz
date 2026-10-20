//! Mounted SMB/AFP references. Passwords never leave the native Keychain helper.
use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Location {
    pub url: String,
    pub username: String,
    pub guest: bool,
    pub read_only: bool,
    pub mount_root: PathBuf,
    pub relative: PathBuf,
}
impl Location {
    pub fn at_path(&self, path: &Path) -> Option<Self> {
        let mut location = self.clone();
        location.relative = path.strip_prefix(&self.mount_root).ok()?.to_path_buf();
        location.valid().then_some(location)
    }
    pub fn same_place(&self, other: &Self) -> bool {
        self.url == other.url
            && self.username == other.username
            && self.guest == other.guest
            && self.read_only == other.read_only
            && self.relative == other.relative
    }
    fn valid(&self) -> bool {
        let authority = self
            .url
            .split_once("://")
            .map(|(_, r)| r.split('/').next().unwrap_or(""));
        (self.url.starts_with("smb://") || self.url.starts_with("afp://"))
            && authority.is_some_and(|a| !a.is_empty() && !a.contains('@'))
            && !self.url.chars().any(char::is_control)
            && self.mount_root.is_absolute()
            && self.mount_root != Path::new("/")
            && !self.relative.is_absolute()
            && self
                .relative
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_)))
    }
}
#[derive(Clone, Serialize)]
struct Request {
    mode: String,
    location: Option<Location>,
}
#[derive(Clone, Deserialize)]
struct Folder {
    location: Location,
    path: PathBuf,
}
#[derive(Clone, Default, Deserialize)]
struct Response {
    #[serde(default)]
    cancelled: bool,
    #[serde(default)]
    folders: Vec<Folder>,
    error: Option<String>,
    warning: Option<String>,
}
#[derive(Clone)]
enum Target {
    NewTab,
    NewWindow,
    Existing(u64),
}
fn helper(request: Request) -> Result<Response, String> {
    use std::io::{Read, Write};
    let helper = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("networkctl")))
        .ok_or("Network connection helper is unavailable.")?;
    let input = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
    let mut child = Command::new(helper)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| {
            "Could not open the network connection dialog. Rebuild Schnauz and try again."
        })?;
    if child
        .stdin
        .take()
        .is_none_or(|mut pipe| pipe.write_all(&input).is_err())
    {
        let _ = child.kill();
        let _ = child.wait();
        return Err("The network connection helper could not start.".into());
    }
    let mut output = Vec::new();
    if let Some(pipe) = child.stdout.take() {
        pipe.take(1024 * 1024 + 1)
            .read_to_end(&mut output)
            .map_err(|_| "Could not read the network connection result.")?;
    }
    if output.len() > 1024 * 1024 {
        let _ = child.kill();
        let _ = child.wait();
        return Err("The network connection result was too large.".into());
    }
    let status = child
        .wait()
        .map_err(|_| "The network connection helper did not finish.")?;
    if !status.success() || output.len() > 1024 * 1024 {
        return Err("The network connection helper did not finish successfully.".into());
    }
    let response: Response = serde_json::from_slice(&output)
        .map_err(|_| "The network connection result was invalid.")?;
    if let Some(error) = &response.error {
        return Err(error.clone());
    }
    if response
        .folders
        .iter()
        .any(|f| !f.location.valid() || f.path != f.location.mount_root.join(&f.location.relative))
    {
        return Err("The network connection returned a folder outside its share.".into());
    }
    Ok(response)
}
impl Schnauz {
    pub(super) fn open_network_dialog(&mut self, cx: &mut Context<Self>) {
        self.begin_network(None, Target::NewTab, None, false, cx);
    }
    pub(super) fn open_network_favourite(
        &mut self,
        shortcut: projects::Shortcut,
        open_file: bool,
        new_window: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(location) = shortcut.network else {
            return;
        };
        self.begin_network(
            Some(location),
            if new_window {
                Target::NewWindow
            } else {
                Target::NewTab
            },
            Some(shortcut.kind),
            open_file,
            cx,
        );
    }
    pub(super) fn reconnect_network_tab(&mut self, pane: usize, cx: &mut Context<Self>) {
        let Some(location) = self.tab(pane).network.clone() else {
            return;
        };
        self.begin_network(
            Some(location),
            Target::Existing(self.tab(pane).id),
            None,
            false,
            cx,
        );
    }
    fn begin_network(
        &mut self,
        location: Option<Location>,
        target: Target,
        kind: Option<projects::Section>,
        open_file: bool,
        cx: &mut Context<Self>,
    ) {
        if self.network_connecting {
            return;
        }
        self.network_connecting = true;
        self.network_status = Some("Connecting to network drive…".into());
        let project = self.projects.active;
        let pane = self.active_pane;
        let request = Request {
            mode: if location.is_some() {
                "reconnect"
            } else {
                "dialog"
            }
            .into(),
            location,
        };
        cx.notify();
        cx.spawn(async move |this, cx| {
            // The native form and NetAuth waits must not occupy the shared file-work pool.
            let (send, receive) = futures::channel::oneshot::channel();
            let worker = std::thread::Builder::new()
                .name("schnauz-network-connect".into())
                .spawn(move || {
                    let _ = send.send(helper(request));
                });
            let result = if worker.is_ok() {
                receive
                    .await
                    .unwrap_or_else(|_| Err("The network connection helper stopped.".into()))
            } else {
                Err("Could not start the network connection worker.".into())
            };
            let _ = this.update(cx, |b, cx| {
                b.network_connecting = false;
                b.network_status = None;
                match result {
                    Ok(response) if response.cancelled => {}
                    Ok(response) => {
                        if project.is_some_and(|id| !b.projects.items.iter().any(|p| p.id == id)) {
                            cx.notify();
                            return;
                        }
                        for folder in response.folders {
                            let file = kind == Some(projects::Section::Files);
                            let path = if file {
                                folder.path.parent().unwrap_or(&folder.path).to_path_buf()
                            } else {
                                folder.path.clone()
                            };
                            let location = folder.location;
                            let Some(browser_location) = location.at_path(&path) else {
                                continue;
                            };
                            if matches!(target, Target::NewWindow) && b.projects.active!=project {
                                b.set_notice("The project changed while connecting. Open the network favourite again in the intended project.".into(),cx);
                                continue;
                            }
                            if matches!(target, Target::NewWindow) {
                                let mut shortcut =
                                    projects::Shortcut::local(path, projects::Section::Folders);
                                shortcut.network = Some(browser_location);
                                if let Err(error) = browser_windows::open(Some(shortcut), cx) {
                                    b.set_notice(error, cx);
                                }
                                continue;
                            }
                            if let Target::Existing(id) = target {
                                let Some((p, t)) =
                                    b.panes.iter().enumerate().find_map(|(p, pane)| {
                                        pane.tabs.iter().position(|t| t.id == id).map(|t| (p, t))
                                    })
                                else {
                                    if let Some(tab)=b.project_sessions.get_mut(&project).and_then(|s|s.panes.iter_mut().flat_map(|p|p.tabs.iter_mut()).find(|t|t.id==id)) {
                                        tab.history=vec![path.clone()];tab.hist_pos=0;tab.selection.clear();tab.anchor=None;
                                        tab.current_dir=path;tab.network=Some(browser_location);tab.network_connected=true;
                                    }
                                    continue;
                                };
                                let tab = &mut b.panes[p].tabs[t];
                                tab.history = vec![path.clone()];
                                tab.hist_pos = 0;
                                tab.selection.clear();
                                tab.anchor = None;
                                tab.current_dir = path;
                                tab.network = Some(browser_location);
                                tab.network_connected = true;
                                if b.panes[p].active == t {
                                    b.reload_pane(p, cx);
                                }
                                continue;
                            }
                            let mut tab =
                                Tab::at_network_location(path, None, Some(browser_location));
                            tab.network_connected = true;
                            if file {
                                tab.selection.insert(folder.path.clone());
                                tab.anchor = Some(folder.path.clone());
                            }
                            if b.projects.active == project {
                                let p = pane.min(b.panes.len() - 1);
                                b.panes[p].tabs.push(tab);
                                b.panes[p].active = b.panes[p].tabs.len() - 1;
                                b.active_pane = p;
                                b.reload_pane(p, cx);
                                if open_file && file {
                                    b.open_local_file(folder.path, cx);
                                }
                            } else if let Some(session) = b.project_sessions.get_mut(&project) {
                                let p = pane.min(session.panes.len() - 1);
                                session.panes[p].tabs.push(tab);
                                session.panes[p].active = session.panes[p].tabs.len() - 1;
                            }
                        }
                        if let Some(warning) = response.warning {
                            b.set_notice(warning, cx);
                        }
                        b.save_session(cx);
                    }
                    Err(error) => b.set_notice(error, cx),
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn reload_network_pane(&mut self, pane: usize, cx: &mut Context<Self>) {
        if !self.tab(pane).network_connected {
            self.tab_mut(pane).listing_error = Some(directory_listing::ListingError {
                permission_denied: false,
                message: "This network share is disconnected. Click Retry to reconnect.".into(),
            });
            cx.notify();
            return;
        }
        self.next_load_gen += 1;
        let gen = self.next_load_gen;
        let tab = self.tab_mut(pane);
        tab.load_gen = gen;
        tab.network_loading = true;
        tab.listing_error = None;
        let id = tab.id;
        let path = tab.current_dir.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let (send,receive)=futures::channel::oneshot::channel();
            let worker=std::thread::Builder::new().name("schnauz-network-list".into())
                .spawn(move||{let _=send.send(read_entries(&path));});
            let result=if worker.is_ok() {receive.await.unwrap_or_else(|_|Err(std::io::Error::other("Network listing stopped")))}
                else {Err(std::io::Error::other("Could not start network listing"))};
            let _ = this.update(cx, |b, cx| {
                let Some((p, t)) = b.panes.iter().enumerate().find_map(|(p, pane)| {
                    pane.tabs
                        .iter()
                        .position(|t| t.id == id && t.load_gen == gen)
                        .map(|t| (p, t))
                }) else {
                    return;
                };
                let tab = &mut b.panes[p].tabs[t];
                tab.network_loading = false;
                match result {
                    Ok(entries)=>tab.install_listing(Ok(entries)),
                    Err(error)=> {
                        tab.network_connected=false;
                        tab.install_listing(Ok(Vec::new()));
                        tab.listing_error=Some(directory_listing::ListingError{permission_denied:false,
                            message:format!("Could not read the network folder: {error}. Click Retry to reconnect.")});
                    }
                }
                if b.panes[p].active == t {
                    b.sort_tab(p);
                    if b.tab(p).is_filtered() {
                        b.recompute_find(p);
                    }
                }
                if b.panes[p].active == t {
                    b.prewarm_icons(cx);
                    if let Some(path) = b.tab(p).anchor.clone() {
                        b.reveal_in_pane(p, path, cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn network_references_keep_routing_without_secret_fields_and_ignore_mount_names_for_identity() {
        let a = Location {
            url: "smb://server/Share".into(),
            username: "DOMAIN\\user".into(),
            guest: false,
            read_only: false,
            mount_root: "/Volumes/Share".into(),
            relative: "Folder/file.txt".into(),
        };
        assert!(a.valid());
        let mut b = a.clone();
        b.mount_root = "/Volumes/Share-1".into();
        assert!(a.same_place(&b));
        let encoded = serde_json::to_string(&a).unwrap();
        assert!(!encoded.contains("password"));
        let child = a.at_path(Path::new("/Volumes/Share/Child")).unwrap();
        assert_eq!(child.relative, PathBuf::from("Child"));
        assert!(a.at_path(Path::new("/Volumes/Share-other/Child")).is_none());
        b.relative = "../outside".into();
        assert!(!b.valid());
        b = a.clone();
        b.url = "smb://user:password@server/Share".into();
        assert!(!b.valid());
        let mut favourite =
            projects::Shortcut::local(a.mount_root.join(&a.relative), projects::Section::Files);
        favourite.network = Some(a.clone());
        let mut remounted = favourite.clone();
        remounted.path = b.mount_root.join(&a.relative);
        remounted.network = Some(a.clone());
        remounted.network.as_mut().unwrap().mount_root = "/Volumes/Share-1".into();
        remounted.path = PathBuf::from("/Volumes/Share-1/Folder/file.txt");
        assert!(favourite.same_place(&remounted));
        assert!(!favourite.same_place(&projects::Shortcut::local(
            favourite.path.clone(),
            projects::Section::Files
        )));
        let mut sidebar = projects::Sidebar::default();
        sidebar.set_shortcut(favourite.clone(), true);
        sidebar.set_shortcut(remounted, true);
        assert_eq!(sidebar.shortcuts.len(), 1);
        let roundtrip: projects::Sidebar =
            serde_json::from_str(&serde_json::to_string(&sidebar).unwrap()).unwrap();
        assert_eq!(
            roundtrip.shortcuts[0].network.as_ref().unwrap().username,
            "DOMAIN\\user"
        );
        assert!(!serde_json::to_string(&roundtrip)
            .unwrap()
            .contains("password"));
    }
}
