//! A new interactive SSH PTY, started only by the explicit Terminal button.
//! No keystrokes/commands are ever written to an existing shell or SSH session.
use super::*;

pub(super) struct SocketLease {
    directory: PathBuf,
}
impl SocketLease {
    pub(super) fn new(shared: Option<&Path>) -> Result<Arc<Self>, String> {
        use std::os::unix::fs::DirBuilderExt;
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let directory = PathBuf::from(format!(
            "/tmp/schnauz-ssh.{}.{}.{nonce:x}",
            std::process::id(),
            NEXT_SESSION.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .map_err(|e| e.to_string())?;
        let lease = Arc::new(Self { directory });
        if let Some(shared) = shared {
            // An owned alias to the authenticated transport. Cleanup unlinks
            // only our alias, never the original master socket or its session.
            std::os::unix::fs::symlink(shared, lease.socket()).map_err(|e| e.to_string())?;
        }
        Ok(lease)
    }
    pub(super) fn socket(&self) -> PathBuf {
        self.directory.join("mux")
    }
    pub(super) fn token(&self) -> String {
        self.directory
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned()
    }
    pub(super) fn clean(&self) {
        let _ = fs::remove_file(self.socket());
        let _ = fs::remove_dir(&self.directory);
    }
}
impl Drop for SocketLease {
    fn drop(&mut self) {
        self.clean();
    }
}

pub(super) fn arguments(
    server: &SftpServer,
    folder: &Path,
    socket: &Path,
    token: &str,
    shared: bool,
    use_system: bool,
) -> Result<Vec<String>, String> {
    let folder = folder
        .to_str()
        .filter(|f| f.starts_with('/') && !f.contains('\0'))
        .ok_or("Cannot start an SSH terminal for this folder path.")?;
    if server.host.trim().is_empty()
        || [&server.host, &server.user]
            .iter()
            .any(|s| s.starts_with('-') || s.chars().any(char::is_control))
    {
        return Err("Invalid SSH server settings.".into());
    }
    let bootstrap = include_str!("terminal-shell/ssh-bootstrap.sh");
    let remote = format!(
        "sh -c {} sh {} {}",
        shell_quote(bootstrap),
        shell_quote(token),
        shell_quote(folder)
    );
    let mut args = vec![
        "-tt".into(),
        "-o".into(),
        format!("ControlPath={}", socket.display()),
        "-o".into(),
        format!("ControlMaster={}", if shared { "no" } else { "auto" }),
        "-o".into(),
        "ControlPersist=no".into(),
        "-o".into(),
        "ConnectTimeout=12".into(),
        "-o".into(),
        format!("RemoteCommand={}", remote.replace('%', "%%")),
    ];
    if shared {
        args.extend([
            "-o".into(),
            "ProxyCommand=false".into(),
            "-o".into(),
            "BatchMode=yes".into(),
            "-o".into(),
            "StrictHostKeyChecking=yes".into(),
        ]);
    } else {
        // This is a fresh PTY, so normal SSH password/passphrase prompts can be
        // answered here. Saved password connections reuse their existing master.
        args.extend([
            "-o".into(),
            "BatchMode=no".into(),
            "-o".into(),
            "StrictHostKeyChecking=accept-new".into(),
        ]);
        if !use_system && !server.key.trim().is_empty() {
            args.extend([
                "-o".into(),
                "IdentitiesOnly=yes".into(),
                "-i".into(),
                server.key.trim().into(),
            ]);
        }
    }
    if server.port != 0 {
        args.extend(["-p".into(), server.port.to_string()]);
    }
    args.push(server.connection_target());
    Ok(args)
}
pub(super) struct Launch {
    pub process: Process,
    pub reader: Box<dyn Read + Send>,
    pub server: SftpServer,
    pub token: String,
    pub origin: PathBuf,
}
pub(super) fn launch(
    server: SftpServer,
    folder: PathBuf,
    settings: PathBuf,
    use_system: bool,
) -> Result<Launch, String> {
    let mut server = ssh_connection::for_reconnect(&server, use_system);
    ensure_master(&server)?;
    let shared = server.control_path.clone().or_else(|| {
        let path = ssh_control_path(&server);
        (server.use_password || (path.exists() && password_master_alive(&server))).then_some(path)
    });
    let socket = SocketLease::new(shared.as_deref())?;
    let token = socket.token();
    let args = arguments(
        &server,
        &folder,
        &socket.socket(),
        &token,
        shared.is_some(),
        use_system,
    )?;
    let origin = home_dir();
    let requests = settings.join("terminal/requests");
    use std::os::unix::fs::PermissionsExt;
    fs::create_dir_all(&requests).map_err(|e| e.to_string())?;
    fs::set_permissions(&requests, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    let mut command = CommandBuilder::new("/usr/bin/ssh");
    command.args(args);
    command.cwd(&origin);
    let request = requests.join(format!("{token}.cwd"));
    let record = requests.join(format!("{token}.ssh"));
    let (process, reader) = spawn_pty(command, request, record, Some(socket.clone()))?;
    // A session owns its private endpoint even when it aliases another master.
    if server.ssh_alias.is_empty() {
        server.ssh_alias = server.host.clone();
    }
    server.control_path = Some(socket.socket());
    Ok(Launch {
        process,
        reader,
        server,
        token,
        origin,
    })
}

impl Schnauz {
    pub(super) fn start_remote_terminal(
        &mut self,
        pane: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(server) = self.tab(pane).remote.clone() else {
            return;
        };
        let folder = self.tab(pane).current_dir.clone();
        self.start_remote_terminal_at(pane,self.panes[pane].active,server,folder,None,window,cx);
    }

    pub(crate) fn start_remote_terminal_at(&mut self,pane:usize,index:usize,server:SftpServer,folder:PathBuf,visible:Option<bool>,window:&mut Window,cx:&mut Context<Self>) {
        let tab_id = self.panes[pane].tabs[index].id;
        let request = NEXT_SESSION.fetch_add(1, Ordering::Relaxed);
        let project = self.projects.active;
        self.panes[pane].tabs[index].terminal_starting = Some(request);
        self.set_notice("Opening SSH terminal…".into(), cx);
        let use_system = prefs().ssh_use_system;
        let settings = config_dir().unwrap_or_else(std::env::temp_dir);
        cx.spawn_in(window, async move |this, cx| {
            let original = server.clone();
            let start_folder = folder.clone();
            let prepared = cx
                .background_spawn(async move { launch(server, folder, settings, use_system) })
                .await;
            let _ = cx.update(|window, cx| {
                this.update(cx, |this, cx| {
                    let location = this.panes.iter().enumerate().find_map(|(p, pane)| {
                        pane.tabs
                            .iter()
                            .position(|t| t.id == tab_id)
                            .map(|i| (p, i))
                    });
                    let Some((p, index)) = location else {
                        for live in this.project_sessions.values_mut() {
                            for t in live.panes.iter_mut().flat_map(|p| &mut p.tabs) {
                                if t.id == tab_id && t.terminal_starting == Some(request) {
                                    t.terminal_starting = None;
                                }
                            }
                        }
                        return;
                    };
                    let tab = &mut this.panes[p].tabs[index];
                    if tab.terminal_starting != Some(request) {
                        return;
                    }
                    tab.terminal_starting = None;
                    if this.projects.active != project
                        || tab.terminal.as_ref().is_some_and(|s| !s.exited())
                    {
                        return;
                    }
                    match prepared {
                        Ok(launch) => {
                            let following = tab.current_dir == start_folder
                                && tab
                                    .remote
                                    .as_ref()
                                    .is_some_and(|s| projects::same_server(s, &original));
                            let link = ssh_sync::Link::remote_start(
                                launch.token,
                                launch.server,
                                launch.origin.clone(),
                                tab,
                                following,
                            );
                            if this.notice.as_deref() == Some("Opening SSH terminal…") {
                                this.notice = None;
                                this.notice_gen += 1;
                            }
                            this.install_terminal(
                                p,
                                index,
                                launch.origin,
                                launch.process,
                                launch.reader,
                                Some(link),
                                cx,
                            );
                            if let Some(visible)=visible { this.panes[p].tabs[index].terminal_open=visible; }
                            if visible.is_none() && this.active_pane == p && this.panes[p].active == index {
                                window.focus(
                                    &this.panes[p].tabs[index]
                                        .terminal
                                        .as_ref()
                                        .unwrap()
                                        .view
                                        .read(cx)
                                        .focus_handle(),
                                );
                            }
                        }
                        Err(error) => {
                            this.set_notice(format!("Couldn't open SSH terminal: {error}"), cx)
                        }
                    }
                    cx.notify();
                })
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn initial_folder_is_literal_and_existing_transport_never_reauthenticates() {
        let server = SftpServer {
            host: "192.0.2.1".into(),
            user: "fixture".into(),
            port: 2222,
            ssh_alias: "my-alias".into(),
            ..Default::default()
        };
        let folder = Path::new("/remote/Folder 'quote' $(touch nope) 素材 %?\nlast");
        let args = arguments(
            &server,
            folder,
            Path::new("/tmp/fixture/mux"),
            "fixture-token",
            false,
            true,
        )
        .unwrap();
        assert_eq!(args.last().unwrap(), "fixture@my-alias");
        assert!(args.contains(&"BatchMode=no".into()));
        let remote = args
            .iter()
            .find_map(|a| a.strip_prefix("RemoteCommand="))
            .unwrap();
        let unexpanded = remote.replace("%%", "%");
        assert!(unexpanded.ends_with(&shell_quote(folder.to_str().unwrap())));
        let args = arguments(
            &server,
            folder,
            Path::new("/tmp/fixture/mux"),
            "fixture-token",
            true,
            true,
        )
        .unwrap();
        assert!(
            args.contains(&"ProxyCommand=false".into())
                && args.contains(&"ControlMaster=no".into())
                && args.contains(&"BatchMode=yes".into())
        );
        assert!(arguments(
            &server,
            Path::new("relative"),
            Path::new("/tmp/f"),
            "t",
            false,
            true
        )
        .is_err());
    }
    #[test]
    fn owned_socket_alias_connects_without_removing_the_original_master() {
        use std::os::unix::net::{UnixListener, UnixStream};
        let root = crate::tests::scratch("ssh-socket-alias");
        let master = root.join("master");
        let listener = UnixListener::bind(&master).unwrap();
        let lease = SocketLease::new(Some(&master)).unwrap();
        let stream = UnixStream::connect(lease.socket()).unwrap();
        let (_accepted, _) = listener.accept().unwrap();
        let directory = lease.directory.clone();
        drop(stream);
        drop(lease);
        assert!(!directory.exists());
        assert!(master.exists());
        drop(listener);
        fs::remove_dir_all(root).unwrap();
    }
}
