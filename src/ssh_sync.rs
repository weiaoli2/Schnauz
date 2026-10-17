//! One-way SSH folder following. Terminal output can select a directory only
//! within the connection described by our private local launch record.
use super::*;

pub(super) enum Event {
    Start(String),
    Ready(String, PathBuf),
    End(String),
    Unsupported(String),
}
fn token_ok(token: &str) -> bool {
    !token.is_empty()
        && token.len() <= 80
        && token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}
pub(super) fn parse(payload: &str) -> Option<Event> {
    let fields: Vec<_> = payload.split(';').collect();
    let token = *fields.get(1)?;
    if !token_ok(token) {
        return None;
    }
    match fields.as_slice() {
        ["start", _] => Some(Event::Start(token.into())),
        ["end", _] => Some(Event::End(token.into())),
        ["unsupported", _] => Some(Event::Unsupported(token.into())),
        ["ready", _, hex] if hex.len() <= 16384 && hex.len() % 2 == 0 => {
            let bytes: Option<Vec<u8>> = hex
                .as_bytes()
                .chunks_exact(2)
                .map(|pair| {
                    Some(
                        (char::from(pair[0]).to_digit(16)? * 16
                            + char::from(pair[1]).to_digit(16)?) as u8,
                    )
                })
                .collect();
            let bytes = bytes?;
            if bytes.contains(&0) {
                return None;
            }
            use std::os::unix::ffi::OsStringExt;
            let folder = PathBuf::from(std::ffi::OsString::from_vec(bytes));
            folder
                .is_absolute()
                .then(|| Event::Ready(token.into(), folder))
        }
        _ => None,
    }
}

#[derive(Clone)]
struct LocalView {
    folder: PathBuf,
    history: Vec<PathBuf>,
    hist_pos: usize,
    style: folder_views::FolderView,
    filter: EntryFilter,
    show_hidden: bool,
}
impl LocalView {
    fn capture(tab: &Tab, folder: PathBuf) -> Self {
        let same = tab.current_dir == folder && tab.remote.is_none();
        Self {
            history: if same {
                tab.history.clone()
            } else {
                vec![folder.clone()]
            },
            hist_pos: if same { tab.hist_pos } else { 0 },
            folder,
            style: folder_views::FolderView {
                view: tab.view,
                list: tab.list_layout.clone(),
                preview_width: tab.preview_width,
                sort: sort_key_name(tab.sort_key).into(),
                ascending: tab.sort_asc,
            },
            filter: tab.entry_filter,
            show_hidden: tab.show_hidden,
        }
    }
    fn restore(self, tab: &mut Tab) {
        tab.remote = None;
        tab.current_dir = self.folder;
        tab.history = self.history;
        tab.hist_pos = self.hist_pos;
        list_ui::apply_style(tab, &self.style);
        tab.entry_filter = self.filter;
        tab.show_hidden = self.show_hidden;
        clear_contents(tab);
    }
}

pub(super) struct Link {
    token: String,
    pub server: SftpServer,
    pub folder: Option<PathBuf>,
    pub following: bool,
    pub unsupported: bool,
    local: LocalView,
}
impl Link {
    fn from_record(token: String, record: &Path, tab: &Tab) -> Option<Self> {
        use std::io::Read;
        let mut bytes = Vec::new();
        fs::File::open(record)
            .ok()?
            .take(32769)
            .read_to_end(&mut bytes)
            .ok()?;
        if bytes.len() > 32768 {
            return None;
        }
        let fields: Vec<_> = bytes.split(|&b| b == 0).collect();
        if fields.len() != 7 || !fields[6].is_empty() || fields[0] != token.as_bytes() {
            return None;
        }
        let string = |i: usize| std::str::from_utf8(fields[i]).ok().map(str::to_owned);
        let socket = PathBuf::from(string(1)?);
        // Only reuse the socket launched by our wrapper. An OSC escape cannot
        // choose a different server, control socket or private metadata file.
        if socket.file_name()? != "mux"
            || socket.parent()?.file_name()? != token.as_str()
            || !token.starts_with("schnauz-ssh.")
            || socket.parent()?.parent()? != Path::new("/tmp")
        {
            return None;
        }
        let host = string(2)?;
        let user = string(3)?;
        if [&host, &user]
            .iter()
            .any(|s| s.is_empty() || s.starts_with('-') || s.chars().any(char::is_control))
        {
            return None;
        }
        let port: u16 = string(4)?.parse().ok().filter(|p| *p > 0)?;
        use std::os::unix::ffi::OsStrExt;
        let origin = PathBuf::from(std::ffi::OsStr::from_bytes(fields[5]));
        if !origin.is_absolute() {
            return None;
        }
        Some(Self {
            local: LocalView::capture(tab, origin),
            token,
            following: true,
            unsupported: false,
            folder: None,
            server: SftpServer {
                name: format!("{user}@{host}"),
                host,
                user,
                port,
                control_path: Some(socket),
                ..Default::default()
            },
        })
    }
    pub fn origin(&self) -> &Path {
        &self.local.folder
    }
    pub fn saved_local(&self) -> projects::SavedTab {
        projects::SavedTab {
            path: self.local.folder.clone(),
            view: view_mode_key(self.local.style.view).into(),
            sort: self.local.style.sort.clone(),
            ascending: self.local.style.ascending,
            filter: self.local.filter,
            show_hidden: self.local.show_hidden,
        }
    }
}
fn clear_contents(tab: &mut Tab) {
    tab.entries.clear();
    tab.selection.clear();
    tab.anchor = None;
    tab.editing_path = None;
    tab.find_query = None;
    tab.find_results.clear();
    tab.content_hits = None;
    tab.content_for = None;
    tab.content_gen += 1;
    tab.load_gen = 0; // Discard listings started before a mode change.
    tab.scroll_handle = UniformListScrollHandle::new();
    tab.h_scroll = ScrollHandle::new();
}
fn follow(tab: &mut Tab) -> bool {
    let Some(link) = tab.terminal.as_ref().and_then(|s| s.ssh.as_ref()) else {
        return false;
    };
    let Some(folder) = link
        .folder
        .clone()
        .filter(|_| link.following && tab.terminal_open)
    else {
        return false;
    };
    let server = link.server.clone();
    if tab.remote.as_ref() == Some(&server) && tab.current_dir == folder {
        return false;
    }
    if tab.remote.as_ref() != Some(&server) {
        tab.history.clear();
    }
    tab.remote = Some(server);
    tab.current_dir = folder.clone();
    tab.history.truncate(tab.hist_pos.saturating_add(1));
    if tab.history.last() != Some(&folder) {
        tab.history.push(folder);
    }
    tab.hist_pos = tab.history.len().saturating_sub(1);
    clear_contents(tab);
    true
}
fn restore(tab: &mut Tab) -> bool {
    let Some(mut link) = tab.terminal.as_mut().and_then(|s| s.ssh.take()) else {
        return false;
    };
    link.following = false;
    let remote = tab.remote.as_ref() == Some(&link.server);
    if remote {
        let local = link.local.clone();
        local.restore(tab);
    }
    let session = tab.terminal.as_mut().unwrap();
    session.ssh = Some(link);
    session.pending = None;
    session.browser_moved_during_ssh();
    remote
}
fn receive(tab: &mut Tab, event: Event) -> (bool, bool) {
    if let Event::Start(token) = event {
        if tab.terminal.as_ref().is_none_or(|s| s.ssh.is_some()) {
            return (false, false);
        }
        let record = tab.terminal.as_ref().unwrap().ssh_record().to_path_buf();
        let link = Link::from_record(token, &record, tab);
        tab.terminal.as_mut().unwrap().ssh = link;
        return (false, false);
    }
    let Some(link) = tab.terminal.as_mut().and_then(|s| s.ssh.as_mut()) else {
        return (false, false);
    };
    match event {
        Event::Ready(token, folder) if token == link.token => {
            link.folder = Some(folder);
            (follow(tab), false)
        }
        Event::End(token) if token == link.token => {
            let changed = restore(tab);
            tab.terminal.as_mut().unwrap().ssh = None;
            (changed, false)
        }
        Event::Unsupported(token) if token == link.token => {
            link.unsupported = true;
            link.following = false;
            (false, true)
        }
        _ => (false, false),
    }
}

impl Shuffle {
    pub(super) fn ssh_shell_event(&mut self, id: u64, payload: &str, cx: &mut Context<Self>) {
        let Some(event) = parse(payload) else {
            return;
        };
        let mut changed = None;
        for (pane, p) in self.panes.iter_mut().enumerate() {
            for (index, tab) in p.tabs.iter_mut().enumerate() {
                if tab.terminal.as_ref().is_some_and(|s| s.id == id) {
                    let result = receive(tab, event);
                    if result.0 {
                        changed = Some((pane, index));
                    }
                    if let Some((pane, index)) = changed {
                        self.reload_ssh_tab(pane, index, cx);
                    }
                    if result.1 {
                        self.set_notice("SSH works normally. Folder following needs Bash or Zsh on the remote machine.".into(), cx);
                    }
                    cx.notify();
                    return;
                }
            }
        }
        // Parked projects keep their shells, but never move the visible browser.
        for live in self.project_sessions.values_mut() {
            for tab in live.panes.iter_mut().flat_map(|p| &mut p.tabs) {
                if tab.terminal.as_ref().is_some_and(|s| s.id == id) {
                    receive(tab, event);
                    cx.notify();
                    return;
                }
            }
        }
    }

    fn reload_ssh_tab(&mut self, pane: usize, index: usize, cx: &mut Context<Self>) {
        // Inactive tabs reload when selected; their SSH events must not close
        // an unrelated search or inspect panel in the visible tab.
        if self.panes[pane].active != index {
            return;
        }
        self.dismiss_space_map |= self.space_map.as_ref().is_some_and(|map| map.pane == pane);
        if self.search_in_pane(pane) {
            self.close_palette(cx);
        }
        self.reload_pane(pane, cx);
    }

    pub(super) fn unsync_ssh_browser(&mut self, pane: usize, cx: &mut Context<Self>) -> bool {
        if self
            .tab(pane)
            .terminal
            .as_ref()
            .is_none_or(|s| s.ssh.is_none())
        {
            return false;
        }
        let changed = restore(self.tab_mut(pane));
        if changed {
            self.reload_ssh_tab(pane, self.panes[pane].active, cx);
        }
        cx.notify();
        true
    }

    pub(super) fn toggle_ssh_follow(&mut self, pane: usize, cx: &mut Context<Self>) {
        let Some(link) = self
            .tab(pane)
            .terminal
            .as_ref()
            .and_then(|s| s.ssh.as_ref())
        else {
            return;
        };
        if link.following || self.tab(pane).remote.is_some() {
            self.unsync_ssh_browser(pane, cx);
        } else if !link.unsupported {
            self.tab_mut(pane)
                .terminal
                .as_mut()
                .unwrap()
                .ssh
                .as_mut()
                .unwrap()
                .following = true;
            if follow(self.tab_mut(pane)) {
                self.reload_ssh_tab(pane, self.panes[pane].active, cx);
            }
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sftp_listing_paths_are_literal_and_cannot_inject_batch_commands() {
        assert!(sftp_path_arg("/remote/folder\n!danger").is_err());
        assert!(sftp_path_arg("/remote/folder\r").is_err());
        assert!(sftp_path_arg("/remote/\0folder").is_err());
        let quoted = sftp_path_arg("/remote/Folder \"quoted\" [name]*?\\").unwrap();
        assert!(quoted.starts_with('"') && quoted.ends_with('"'));
        assert!(quoted.contains("\\\"quoted\\\""));
        assert!(quoted.contains("[name]*?\\\\"));
    }
    #[test]
    fn protocol_requires_matching_private_launch_record_and_roundtrips_unusual_paths() {
        let root = crate::tests::scratch("ssh-link");
        let record = root.join("launch");
        let origin = root.join("Original Mac folder");
        fs::create_dir(&origin).unwrap();
        let token = "schnauz-ssh.ABCdef12";
        let socket = format!("/tmp/{token}/mux");
        let mut bytes = Vec::new();
        for field in [
            token,
            &socket,
            "remote.example",
            "oliver",
            "2222",
            origin.to_str().unwrap(),
        ] {
            bytes.extend(field.as_bytes());
            bytes.push(0);
        }
        fs::write(&record, &bytes).unwrap();
        let mut tab = Tab::new(origin.clone());
        tab.view = ViewMode::Icons;
        tab.preview_width = 610;
        let link = Link::from_record(token.into(), &record, &tab).unwrap();
        assert!(Link::from_record("forged".into(), &record, &tab).is_none());
        assert_eq!(link.server.host, "remote.example");
        assert_eq!(link.server.port, 2222);
        assert_eq!(link.saved_local().path, origin);
        let options = ssh_options(&link.server, true);
        assert!(options.contains(&"ProxyCommand=false".into()));
        assert!(options.contains(&format!("ControlPath={socket}")));
        assert!(!serde_json::to_string(&link.server).unwrap().contains("mux"));
        let remote_folder = "/home/oliver/Folder 'quoted' $name 素材 #?%;file\nname";
        let hex: String = remote_folder
            .as_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert!(
            matches!(parse(&format!("ready;{token};{hex}")), Some(Event::Ready(t, path)) if t == token && path == Path::new(remote_folder))
        );
        for invalid in [
            "ready;token;00",
            "ready;token;2e2e",
            "ready;token;f",
            "ready;token;gg",
            "start;../bad",
            "end;token;extra",
        ] {
            assert!(parse(invalid).is_none(), "{invalid}");
        }
        tab.remote = Some(link.server.clone());
        tab.current_dir = PathBuf::from("/remote/path");
        tab.view = ViewMode::List;
        link.local.restore(&mut tab);
        assert_eq!(tab.current_dir, origin);
        assert!(tab.remote.is_none());
        assert_eq!(tab.view, ViewMode::Icons);
        assert_eq!(tab.preview_width, 610);
        fs::remove_dir_all(root).unwrap();
    }
}
