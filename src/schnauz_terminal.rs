//! Persistent per-tab terminal sessions, native PTYs and two-way folder sync.
use super::*;
use gpui::{Edges, Entity};
use gpui_terminal::{shell::ShellEvent, ColorPalette, TerminalConfig, TerminalView};
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};

static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);
const CD_WIDGET: &[u8] = b"\x1b[99;1~";

#[derive(Clone)]
struct SharedWriter(Arc<Mutex<Box<dyn Write + Send>>>);
impl Write for SharedWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().write(bytes)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}

struct Process {
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    writer: SharedWriter,
    killer: Box<dyn ChildKiller + Send + Sync>,
    exited: Arc<AtomicBool>,
    request_file: PathBuf,
    ssh_request: PathBuf,
    shell_pid: Option<u32>,
}
impl Process {
    fn input(&self, bytes: &[u8]) -> std::io::Result<()> {
        let mut writer = self.writer.clone();
        writer.write_all(bytes).and_then(|_| writer.flush())
    }

    fn change_directory(&self, folder: &Path) -> std::io::Result<()> {
        if self.has_foreground_app() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "A terminal app is running",
            ));
        }
        use std::os::unix::ffi::OsStrExt;
        let mut bytes = folder.as_os_str().as_bytes().to_vec();
        bytes.push(0);
        crate::projects::atomic_write(&self.request_file, &bytes)?;
        self.input(CD_WIDGET)
    }

    fn has_foreground_app(&self) -> bool {
        match (
            self.shell_pid,
            self.master.lock().unwrap().process_group_leader(),
        ) {
            (Some(shell), Some(foreground)) => foreground as u32 != shell,
            _ => false,
        }
    }

    fn is_idle_shell(&self) -> bool {
        let Some(shell) = self.shell_pid else {
            return false;
        };
        if self.exited.load(Ordering::Acquire)
            || self.master.lock().unwrap().process_group_leader() != Some(shell as i32)
        {
            return false;
        }
        // A prompt alone isn't enough: a background or suspended job must also
        // survive. Check only on an explicit recovery click, never every frame.
        let mut children = [0 as libc::pid_t; 128];
        unsafe {
            libc::proc_listchildpids(
                shell as libc::pid_t,
                children.as_mut_ptr().cast(),
                std::mem::size_of_val(&children) as i32,
            ) == 0
        }
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        if !self.exited.load(Ordering::Acquire) {
            // Only the process group belonging to this PTY is signalled.
            if let Some(group) = self.master.lock().unwrap().process_group_leader() {
                if group > 1 {
                    unsafe {
                        libc::kill(-group, libc::SIGHUP);
                    }
                }
            }
            let _ = self.killer.kill();
        }
        let _ = fs::remove_file(&self.request_file);
        let _ = fs::remove_file(&self.ssh_request);
    }
}

fn spawn_process(
    cwd: &Path,
    settings: &Path,
    test_shell: bool,
) -> Result<(Process, Box<dyn Read + Send>), String> {
    let id = NEXT_SESSION.fetch_add(1, Ordering::Relaxed);
    let integration = settings.join("terminal/zsh");
    fs::create_dir_all(&integration).map_err(|e| e.to_string())?;
    for (name, source) in [
        (".zshenv", include_str!("terminal-shell/zshenv")),
        (".zprofile", include_str!("terminal-shell/zprofile")),
        (".zshrc", include_str!("terminal-shell/zshrc")),
        ("ssh.zsh", include_str!("terminal-shell/ssh.zsh")),
        ("ssh-bootstrap.sh", include_str!("terminal-shell/ssh-bootstrap.sh")),
    ] {
        crate::projects::atomic_write(&integration.join(name), source.as_bytes())
            .map_err(|e| e.to_string())?;
    }
    let requests = settings.join("terminal/requests");
    fs::create_dir_all(&requests).map_err(|e| e.to_string())?;
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&requests, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    let request_file = requests.join(format!("{}-{id}.cwd", std::process::id()));
    let ssh_request = requests.join(format!("{}-{id}.ssh", std::process::id()));
    let pair = native_pty_system()
        .openpty(PtySize {
            rows: 18,
            cols: 100,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())?;
    let mut command = CommandBuilder::new("/bin/zsh");
    command.args(["-l", "-i"]);
    command.cwd(cwd);
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
    command.env("TERM_PROGRAM", "Schnauz");
    command.env("ZDOTDIR", &integration);
    command.env("SCHNAUZ_INTEGRATION_DIR", &integration);
    command.env(
        "SCHNAUZ_USER_ZDOTDIR",
        std::env::var_os("ZDOTDIR").unwrap_or_else(|| home_dir().into_os_string()),
    );
    command.env("SCHNAUZ_CWD_REQUEST", &request_file);
    command.env("SCHNAUZ_SSH_REQUEST", &ssh_request);
    command.env(
        "SCHNAUZ_TEST_SHELL",
        if test_shell { "1" } else { "0" },
    );
    if std::env::var_os("LANG").is_none() {
        command.env("LANG", "en_US.UTF-8");
    }
    let mut child = pair
        .slave
        .spawn_command(command)
        .map_err(|e| e.to_string())?;
    drop(pair.slave);
    let killer = child.clone_killer();
    let shell_pid = child.process_id();
    let reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = SharedWriter(Arc::new(Mutex::new(
        pair.master.take_writer().map_err(|e| e.to_string())?,
    )));
    let exited = Arc::new(AtomicBool::new(false));
    let finished = exited.clone();
    std::thread::spawn(move || {
        let _ = child.wait();
        finished.store(true, Ordering::Release);
    });
    Ok((
        Process {
            master: Arc::new(Mutex::new(pair.master)),
            writer,
            killer,
            exited,
            request_file,
            ssh_request,
            shell_pid,
        },
        reader,
    ))
}

#[derive(Default)]
struct ForegroundSync {
    running: bool,
    browser_moved: bool,
}
impl ForegroundSync {
    fn started(&mut self) {
        // Enter is also delivered by editors, Codex, REPLs and other CLIs.
        // Only the transition out of the shell prompt begins a new command.
        if !self.running {
            self.browser_moved = false;
        }
        self.running = true;
    }
    fn prompt_ready(&mut self) -> bool {
        let follow = !self.browser_moved;
        self.running = false;
        self.browser_moved = false;
        follow
    }
}

pub(super) struct Session {
    pub id: u64,
    pub view: Entity<TerminalView>,
    pub cwd: PathBuf,
    pub ready: bool,
    guard: ForegroundSync,
    pub pending: Option<PathBuf>,
    process: Process,
    pub ssh: Option<crate::ssh_sync::Link>,
}
impl Session {
    pub(super) fn ssh_record(&self) -> &Path { &self.process.ssh_request }
    pub(super) fn browser_moved_during_ssh(&mut self) { self.guard.browser_moved = true; }
    fn exited(&self) -> bool {
        self.process.exited.load(Ordering::Acquire)
    }
    fn request_directory(&mut self, folder: PathBuf) {
        if self.cwd == folder && (self.ready || self.pending.is_none()) {
            self.pending = None;
            return;
        }
        if self.guard.running || self.process.has_foreground_app() {
            self.guard.running = true;
            self.guard.browser_moved = true;
            self.pending = None;
            return;
        }
        self.pending = Some(folder);
        self.flush_directory();
    }
    fn flush_directory(&mut self) {
        if !self.ready || self.guard.running || self.exited() {
            return;
        }
        if self.pending.as_ref() == Some(&self.cwd) {
            self.pending = None;
            return;
        }
        let Some(folder) = self.pending.as_ref() else {
            return;
        };
        if self.process.change_directory(folder).is_ok() {
            self.ready = false;
        }
    }
}

impl Schnauz {
    pub(super) fn toggle_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pane = self.active_pane;
        if self.tab(pane).remote.is_some() && self.tab(pane).terminal.is_none() {
            self.set_notice(
                "Open a local folder to start its terminal. You can run ssh inside the terminal."
                    .into(),
                cx,
            );
            return;
        }
        let restart = self
            .tab(pane)
            .terminal
            .as_ref()
            .is_none_or(|session| session.exited());
        if restart {
            if !self.start_terminal(pane, cx) {
                return;
            }
        } else {
            self.tab_mut(pane).terminal_open = !self.tab(pane).terminal_open;
        }
        if self.tab(pane).terminal_open {
            self.sync_terminal_directory(pane);
            self.tab(pane)
                .terminal
                .as_ref()
                .unwrap()
                .view
                .read(cx)
                .focus_handle()
                .focus(window);
        } else {
            window.focus(&self.focus);
        }
        cx.notify();
    }

    fn start_terminal(&mut self, pane: usize, cx: &mut Context<Self>) -> bool {
        let cwd = self.tab(pane).current_dir.clone();
        let settings = config_dir().unwrap_or_else(std::env::temp_dir);
        let (process, reader) = match spawn_process(&cwd, &settings, false) {
            Ok(pair) => pair,
            Err(error) => {
                self.set_notice(format!("Couldn't start terminal: {error}"), cx);
                return false;
            }
        };
        let id = NEXT_SESSION.fetch_add(1, Ordering::Relaxed);
        let parent = cx.entity().downgrade();
        let parent_exit = parent.clone();
        let resize_master = process.master.clone();
        let writer = process.writer.clone();
        let config = TerminalConfig {
            font_family: "Menlo".into(),
            font_size: px(12.0),
            cols: 100,
            rows: 18,
            scrollback: 10000,
            line_height_multiplier: 1.15,
            padding: Edges::all(px(8.0)),
            colors: ColorPalette::builder()
                .background(0x15, 0x18, 0x22)
                .foreground(0xdc, 0xe1, 0xec)
                .cursor(0xa5, 0xb4, 0xfc)
                .build(),
        };
        let view = cx.new(|cx| {
            TerminalView::new(writer, reader, config, cx)
                .with_resize_callback(move |cols, rows| {
                    let _ = resize_master.lock().unwrap().resize(PtySize {
                        cols: cols.min(u16::MAX as usize) as u16,
                        rows: rows.min(u16::MAX as usize) as u16,
                        pixel_width: 0,
                        pixel_height: 0,
                    });
                })
                .with_shell_callback(move |event, window, cx| {
                    if matches!(event, ShellEvent::Focused | ShellEvent::CommandStarted) {
                        let _ =
                            parent.update(cx, |this, cx| this.terminal_shell_event(id, event, cx));
                    } else {
                        // Protocol events are drained while drawing the terminal.
                        // Update its parent after this frame so the new cwd/status
                        // is guaranteed another frame without waiting for input.
                        let parent = parent.clone();
                        let event = event.clone();
                        window.on_next_frame(move |_, cx| {
                            let _ = parent
                                .update(cx, |this, cx| this.terminal_shell_event(id, &event, cx));
                        });
                    }
                })
                .with_exit_callback(move |window, _| {
                    let parent = parent_exit.clone();
                    window.on_next_frame(move |_, cx| {
                        let _ = parent.update(cx, |_, cx| cx.notify());
                    });
                })
        });
        self.tab_mut(pane).terminal = Some(Session {
            id,
            view,
            cwd,
            ready: false,
            guard: ForegroundSync::default(),
            pending: None,
            process,
            ssh: None,
        });
        self.tab_mut(pane).terminal_open = true;
        true
    }

    fn recover_terminal(&mut self, pane: usize, window: &mut Window, cx: &mut Context<Self>) {
        let tab = self.tab(pane);
        let Some(session) = tab.terminal.as_ref() else {
            return;
        };
        if tab.remote.is_some() || session.cwd == tab.current_dir || session.exited() {
            return;
        }
        if !session.ready
            || session.guard.running
            || session.pending.is_some()
            || !session.process.is_idle_shell()
        {
            self.set_notice(
                "Finish or stop terminal jobs before recovering folder sync.".into(),
                cx,
            );
            return;
        }
        // Spawn successfully before replacing the old session. The panel stays
        // open, and recovery leaves keyboard focus in the file browser.
        if self.start_terminal(pane, cx) {
            self.active_pane = pane;
            window.focus(&self.focus);
            cx.notify();
        }
    }

    pub(super) fn sync_terminal_directory(&mut self, pane: usize) {
        let tab = self.tab_mut(pane);
        if tab.terminal.as_ref().is_some_and(|s| s.ssh.is_some()) { return; }
        if tab.terminal_open && tab.remote.is_none() {
            if let Some(session) = tab.terminal.as_mut() {
                session.request_directory(tab.current_dir.clone());
            }
        }
    }

    fn terminal_shell_event(&mut self, id: u64, event: &ShellEvent, cx: &mut Context<Self>) {
        if let ShellEvent::SshIntegration(payload) = event {
            self.ssh_shell_event(id, payload, cx);
            return;
        }
        let target = self.panes.iter().enumerate().find_map(|(p, pane)| {
            pane.tabs.iter().enumerate().find_map(|(t, tab)| {
                tab.terminal
                    .as_ref()
                    .is_some_and(|session| session.id == id)
                    .then_some((p, t))
            })
        });
        let Some((pane, tab_index)) = target else {
            // A project switch parks its live tabs rather than ending their
            // shells. Record protocol events without changing the visible project.
            for live in self.project_sessions.values_mut() {
                for tab in live.panes.iter_mut().flat_map(|p| &mut p.tabs) {
                    let Some(session) = tab.terminal.as_mut().filter(|s| s.id == id) else {
                        continue;
                    };
                    if session.ssh.is_some() { return; }
                    match event {
                        ShellEvent::SshIntegration(_) => unreachable!(),
                        ShellEvent::WorkingDirectory(url) => {
                            if let Some(folder) =
                                crate::clipboard_path::parse_path(url, &home_dir())
                                    .filter(|p| p.is_dir())
                            {
                                session.cwd = folder.clone();
                                if session.pending.as_ref() == Some(&folder) {
                                    session.pending = None;
                                }
                                if tab.terminal_open
                                    && !session.guard.running
                                    && session.pending.is_none()
                                {
                                    tab.current_dir = folder;
                                    tab.install_listing(read_entries_fast(&tab.current_dir));
                                    tab.find_query = None;
                                    tab.selection.clear();
                                    tab.anchor = None;

                                    tab.recompute_results();
                                    let style = self.folder_views.borrow().resolve(&tab.current_dir).cloned().unwrap_or_default();
                                    list_ui::apply_style(tab, &style);
                                }
                            }
                        }
                        ShellEvent::PromptReady => {
                            session.ready = true;
                            let follow = session.guard.prompt_ready()
                                && tab.terminal_open
                                && session.pending.is_none();
                            if follow {
                                tab.current_dir = session.cwd.clone();
                                tab.install_listing(read_entries_fast(&tab.current_dir));
                                tab.recompute_results();
                                let style = self.folder_views.borrow().resolve(&tab.current_dir).cloned().unwrap_or_default();
                                list_ui::apply_style(tab, &style);
                            }
                        }
                        ShellEvent::CommandStarted => {
                            session.ready = false;
                            session.guard.started();
                            session.pending = None;
                        }
                        ShellEvent::DirectoryChangeFailed => session.pending = None,
                        ShellEvent::Focused => {}
                    }
                    return;
                }
            }
            return;
        };
        let tab = &mut self.panes[pane].tabs[tab_index];
        let session = tab.terminal.as_mut().unwrap();
        if session.ssh.is_some() {
            if matches!(event, ShellEvent::Focused) { self.active_pane = pane; }
            cx.notify(); return;
        }
        match event {
            ShellEvent::SshIntegration(_) => unreachable!(),
            ShellEvent::Focused => {
                self.active_pane = pane;
            }
            ShellEvent::WorkingDirectory(url) => {
                let Some(folder) = crate::clipboard_path::parse_path(url, &home_dir())
                    .filter(|path| path.is_dir())
                else {
                    return;
                };
                session.cwd = folder.clone();
                if session.pending.as_ref() == Some(&folder) {
                    session.pending = None;
                }
                // A foreground app may emit its own cwd reports. Record them
                // without moving the browser while the app owns the terminal.
                if !session.guard.running {
                    self.follow_terminal_folder(pane, tab_index, cx);
                }
            }
            ShellEvent::PromptReady => {
                session.ready = true;
                let follow = session.guard.prompt_ready();
                if tab.terminal_open {
                    session.flush_directory();
                }
                if follow {
                    self.follow_terminal_folder(pane, tab_index, cx);
                }
            }
            ShellEvent::DirectoryChangeFailed => {
                session.pending = None;
                self.set_notice("The terminal couldn't enter that folder.".into(), cx);
            }
            ShellEvent::CommandStarted => {
                session.ready = false;
                session.guard.started();
                session.pending = None;
            }
        }
        cx.notify();
    }

    fn follow_terminal_folder(&mut self, pane: usize, index: usize, cx: &mut Context<Self>) {
        let tab = &self.panes[pane].tabs[index];
        let session = tab.terminal.as_ref().unwrap();
        if !tab.terminal_open || session.pending.is_some() || tab.current_dir == session.cwd {
            return;
        }
        let folder = session.cwd.clone();
        if self.panes[pane].active == index {
            let active = self.active_pane;
            self.navigate_in(pane, folder, cx);
            self.active_pane = active;
        } else {
            let tab = &mut self.panes[pane].tabs[index];
            tab.current_dir = folder.clone();
            tab.install_listing(read_entries_fast(&folder));
            sort_entries(&mut tab.entries, tab.sort_key, tab.sort_asc);
            tab.find_query = None;
            tab.recompute_results();
            tab.selection.clear();
            tab.anchor = None;
            let style = self.folder_views.borrow().resolve(&tab.current_dir).cloned().unwrap_or_default();
            list_ui::apply_style(tab, &style);
        }
    }

    pub(super) fn render_terminal_panel(
        &self,
        pane: usize,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let tab = self.tab(pane);
        if !tab.terminal_open {
            return None;
        }
        let session = tab.terminal.as_ref()?;
        let t = theme();
        let ssh = session.ssh.as_ref();
        let synced = if let Some(link) = ssh {
            link.following && link.folder.as_ref() == Some(&tab.current_dir) && tab.remote.as_ref() == Some(&link.server)
        } else { session.cwd == tab.current_dir && session.pending.is_none() };
        let sync_tip = if ssh.is_some() {
            "Click to toggle SSH folder following. Unsync shows your original local folder; SSH keeps running."
        } else { "Restart in this folder when idle. Clears scrollback, shell variables and unfinished input." };
        let terminal_path = ssh.map(|link| format!("{}:{}", link.server.name, link.folder.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "connecting…".into())))
            .unwrap_or_else(|| display_path(&session.cwd));
        let label = if session.exited() {
            "Session ended"
        } else if synced {
            "Synced"
        } else {
            "Unsynced"
        };
        Some(
            div()
                .flex_none()
                .h(px(tab.terminal_height))
                .min_h(px(150.0))
                .flex()
                .flex_col()
                .bg(rgb(0x151822))
                .child(
                    div()
                        .id(("terminal-divider", pane))
                        .h(px(5.0))
                        .flex_none()
                        .cursor(CursorStyle::ResizeUpDown)
                        .bg(rgb(t.border))
                        .hover(|s| s.bg(rgb(t.accent)))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                this.terminal_resize = Some((
                                    pane,
                                    f32::from(event.position.y),
                                    this.tab(pane).terminal_height,
                                ));
                                cx.stop_propagation();
                            }),
                        ),
                )
                .child(
                    div()
                        .flex_none()
                        .h(px(30.0))
                        .px_3()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_size(px(11.0))
                        .text_color(rgb(0xaab4cb))
                        .child(div().font_weight(FontWeight::SEMIBOLD).child("Terminal"))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(terminal_path),
                        )
                        .child(
                            div()
                                .id(("terminal-sync", pane))
                                .text_color(rgb(if synced { 0x83d7ac } else { 0xf0c577 }))
                                .child(label)
                                .when((ssh.is_some() || !synced) && !session.exited(), |d| {
                                    d.cursor_pointer()
                                        .hover(|s| s.text_color(rgb(0xffffff)))
                                        .tooltip(tip(sync_tip))
                                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                            cx.stop_propagation();
                                            if this.tab(pane).terminal.as_ref().is_some_and(|s| s.ssh.is_some()) {
                                                this.toggle_ssh_follow(pane, cx);
                                            } else { this.recover_terminal(pane, window, cx); }
                                        }))
                                }),
                        )
                        .when(ssh.is_none() && session.guard.running, |d| d.child("· Folder sync paused"))
                        .when(ssh.is_some(), |d| d.child("· SSH → browser"))
                        .child(
                            div()
                                .id(("terminal-hide", pane))
                                .px_2()
                                .cursor_pointer()
                                .hover(|s| s.text_color(rgb(0xffffff)))
                                .child("Hide")
                                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                    this.tab_mut(pane).terminal_open = false;
                                    window.focus(&this.focus);
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .overflow_hidden()
                        .child(session.view.clone()),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    struct Probe {
        process: Process,
        rx: mpsc::Receiver<Vec<u8>>,
        parser: gpui_terminal::shell::ShellParser,
        output: Vec<u8>,
        events: Vec<ShellEvent>,
        cwd: Option<PathBuf>,
    }
    impl Probe {
        fn start(folder: &Path, settings: &Path) -> Self {
            let (process, mut reader) = spawn_process(folder, settings, true).unwrap();
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let mut buffer = [0; 4096];
                while let Ok(count) = reader.read(&mut buffer) {
                    if count == 0 || tx.send(buffer[..count].to_vec()).is_err() {
                        break;
                    }
                }
            });
            Self {
                process,
                rx,
                parser: Default::default(),
                output: Vec::new(),
                events: Vec::new(),
                cwd: None,
            }
        }
        fn wait(&mut self, condition: impl Fn(&[ShellEvent], &str) -> bool) {
            let deadline = Instant::now() + Duration::from_secs(8);
            let mut events = Vec::new();
            loop {
                let bytes = self
                    .rx
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    .unwrap_or_else(|error| panic!("terminal did not reach the expected state: {error}; output: {}", String::from_utf8_lossy(&self.output)));
                let received = self.parser.feed(&bytes);
                for event in &received {
                    if let ShellEvent::WorkingDirectory(url) = event {
                        self.cwd = crate::clipboard_path::parse_path(url, &home_dir());
                    }
                }
                self.events.extend(received.iter().cloned());
                events.extend(received);
                self.output.extend(bytes);
                if condition(&events, &String::from_utf8_lossy(&self.output)) {
                    return;
                }
            }
        }
        fn ready(&mut self) {
            self.wait(|events, _| events.contains(&ShellEvent::PromptReady));
        }
        fn command(&mut self, command: &str) {
            self.output.clear();
            self.process
                .input(format!("{command}\r").as_bytes())
                .unwrap();
            self.ready();
        }
    }

    #[test]
    fn enter_inside_foreground_app_never_reenables_directory_sync() {
        let mut guard = ForegroundSync::default();
        guard.started();
        guard.browser_moved = true;
        // A Codex prompt or an editor's quit command can contain many Enters.
        guard.started();
        guard.started();
        assert!(!guard.prompt_ready());
        assert!(!guard.running);
        // A later explicit shell cd can follow the terminal normally.
        guard.started();
        assert!(guard.prompt_ready());
    }

    #[test]
    fn ssh_wrapper_reports_remote_folders_and_preserves_the_live_remote_cli() {
        use std::os::unix::fs::PermissionsExt;
        let root = crate::tests::scratch("ssh-terminal");
        let bin = root.join("bin");
        fs::create_dir(&bin).unwrap();
        let fake_ssh = bin.join("ssh");
        fs::write(&fake_ssh, r#"#!/bin/sh
if [ "$1" = -G ]; then
  printf 'hostname test.invalid\nuser tester\nport 2222\nremotecommand none\nsessiontype default\n'
  exit 0
fi
remote=''
for arg in "$@"; do
  case "$arg" in RemoteCommand=*) remote=${arg#RemoteCommand=} ;; esac
done
if [ -z "$remote" ]; then printf 'NORMAL_SSH_PASSTHROUGH\n'; exit 0; fi
# Simulate OpenSSH's documented %% expansion without making a network connection.
remote=$(printf '%s' "$remote" | sed 's/%%/%/g')
SNZ_SSH_TEST_SHELL=1 SHELL="$SNZ_SSH_TEST_REMOTE_SHELL" /bin/sh -c "$remote"
"#).unwrap();
        fs::set_permissions(&fake_ssh, fs::Permissions::from_mode(0o755)).unwrap();
        let destination = root.join("Remote 'quoted' $name 素材 #?%;folder");
        fs::create_dir(&destination).unwrap();
        for shell in ["/bin/bash", "/bin/zsh"] {
            let mut probe = Probe::start(&root, &root.join(if shell.ends_with("bash") {"bash-profile"} else {"zsh-profile"}));
            probe.ready();
            probe.command(&format!("export PATH={}:$PATH SNZ_SSH_TEST_REMOTE_SHELL={shell}", shell_quote(&bin.to_string_lossy())));
            probe.events.clear();
            probe.output.clear();
            probe.process.input(b"ssh -p 2222 test-alias\r").unwrap();
            probe.wait(|events, _| events.iter().any(|e| matches!(e, ShellEvent::SshIntegration(p) if matches!(crate::ssh_sync::parse(p), Some(crate::ssh_sync::Event::Ready(_, _))))));
            assert!(probe.process.has_foreground_app());
            assert!(probe.events.iter().any(|e| matches!(e, ShellEvent::SshIntegration(p) if p.starts_with("start;"))));
            assert!(probe.process.ssh_request.is_file());
            probe.output.clear();
            probe.events.clear();
            probe.process.input(format!("cd -- {}\r", shell_quote(&destination.to_string_lossy())).as_bytes()).unwrap();
            probe.wait(|events, _| events.iter().any(|e| matches!(e, ShellEvent::SshIntegration(p) if matches!(crate::ssh_sync::parse(p), Some(crate::ssh_sync::Event::Ready(_, folder)) if folder == destination))));
            // A live remote CLI still owns the terminal; browsing must never
            // inject the local directory-change widget into it.
            probe.process.input(b"/usr/bin/vim -Nu NONE -n -i NONE ssh-test.txt\r").unwrap();
            probe.wait(|_, text| text.contains("\x1b[?1049h"));
            assert_eq!(probe.process.change_directory(&root).unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
            probe.process.input(b"iSSH editor survived\x1b:wq\r").unwrap();
            probe.wait(|events, _| events.iter().any(|e| matches!(e, ShellEvent::SshIntegration(p) if p.starts_with("ready;"))));
            assert_eq!(fs::read_to_string(destination.join("ssh-test.txt")).unwrap(), "SSH editor survived\n");
            fs::remove_file(destination.join("ssh-test.txt")).unwrap();
            probe.events.clear();
            probe.output.clear();
            probe.process.input(b"exit\r").unwrap();
            probe.wait(|events, _| events.contains(&ShellEvent::PromptReady));
            assert!(probe.events.iter().any(|e| matches!(e, ShellEvent::SshIntegration(p) if p.starts_with("end;"))));
            assert!(!probe.process.ssh_request.exists());
            assert!(probe.process.is_idle_shell());
            probe.command("ssh test-alias printf hello");
            assert!(String::from_utf8_lossy(&probe.output).contains("NORMAL_SSH_PASSTHROUGH"));
            drop(probe);
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn real_pty_preserves_shell_state_resizes_runs_vim_and_syncs_paths() {
        let root = crate::tests::scratch("real-terminal");
        let start = root.join("start");
        let destination = root.join("Folder 'quoted' $name 素材 #?%");
        fs::create_dir_all(&start).unwrap();
        fs::create_dir_all(&destination).unwrap();
        let mut probe = Probe::start(&start, &root.join("profile"));
        probe.ready();
        assert!(probe.process.is_idle_shell());
        assert_eq!(
            probe
                .cwd
                .as_ref()
                .map(|path| fs::canonicalize(path).unwrap()),
            Some(fs::canonicalize(&start).unwrap())
        );
        probe.command("typeset -g SCHNAUZ_TEST_STATE=preserved");
        // Keep a partially typed command while the browser changes directories.
        probe.output.clear();
        probe
            .process
            .input(b"printf '\\nSYNC_STATE:%s\\n' $SCHNAUZ_TEST_STATE")
            .unwrap();
        probe.process.change_directory(&destination).unwrap();
        probe.ready();
        assert_eq!(
            probe
                .cwd
                .as_ref()
                .map(|path| fs::canonicalize(path).unwrap()),
            Some(fs::canonicalize(&destination).unwrap())
        );
        probe.process.input(b"\r").unwrap();
        probe.wait(|events, text| {
            events.contains(&ShellEvent::PromptReady) && text.contains("SYNC_STATE:preserved")
        });
        probe.command("printf '\\nCWD:%s\\n' $PWD");
        assert!(String::from_utf8_lossy(&probe.output).contains("Folder 'quoted' $name 素材"));
        probe
            .process
            .master
            .lock()
            .unwrap()
            .resize(PtySize {
                rows: 30,
                cols: 110,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        probe.command("stty size");
        assert!(String::from_utf8_lossy(&probe.output).contains("30 110"));
        // A real full-screen editor must own the same persistent TTY.
        probe.output.clear();
        probe
            .process
            .input(b"/usr/bin/vim -Nu NONE -n -i NONE terminal-test.txt\r")
            .unwrap();
        probe.wait(|_, text| text.contains("\x1b[?1049h"));
        let request_before = fs::read(&probe.process.request_file).unwrap();
        assert!(probe.process.has_foreground_app());
        assert!(!probe.process.is_idle_shell());
        assert_eq!(
            probe.process.change_directory(&start).unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        assert_eq!(
            fs::read(&probe.process.request_file).unwrap(),
            request_before
        );
        probe
            .process
            .input(b"iInteractive terminal works\x1b:wq\r")
            .unwrap();
        probe.ready();
        assert!(probe.process.is_idle_shell());
        assert_eq!(
            fs::read_to_string(destination.join("terminal-test.txt")).unwrap(),
            "Interactive terminal works\n"
        );
        probe.output.clear();
        probe.process.input(b"cd ..\r").unwrap();
        probe.wait(|events, _| events.iter().any(|event| matches!(event, ShellEvent::WorkingDirectory(url) if url.ends_with("real-terminal-".trim_end_matches('-')))) || events.contains(&ShellEvent::PromptReady));
        let cwd_reported = String::from_utf8_lossy(&probe.output);
        assert!(cwd_reported.contains("\x1b]7;file://localhost"));
        // A disappeared destination fails once rather than flooding queued input.
        probe.output.clear();
        probe
            .process
            .change_directory(&root.join("missing"))
            .unwrap();
        probe.wait(|events, _| {
            events.contains(&ShellEvent::DirectoryChangeFailed)
                && events.contains(&ShellEvent::PromptReady)
        });
        // Background jobs can leave a ready prompt but must block recovery.
        probe.command("sleep 30 & SCHNAUZ_TEST_JOB=$!");
        assert!(!probe.process.has_foreground_app());
        assert!(!probe.process.is_idle_shell());
        probe.command(
            "kill $SCHNAUZ_TEST_JOB; wait $SCHNAUZ_TEST_JOB 2>/dev/null; unset SCHNAUZ_TEST_JOB",
        );
        assert!(probe.process.is_idle_shell());
        drop(probe);
        // An idle restart uses the browser's folder and a fresh shell.
        let mut restarted = Probe::start(&start, &root.join("profile"));
        restarted.ready();
        assert_eq!(
            restarted.cwd.as_ref().map(|p| fs::canonicalize(p).unwrap()),
            Some(fs::canonicalize(&start).unwrap())
        );
        restarted.command("printf '\\nRESTART_STATE:%s\\n' ${SCHNAUZ_TEST_STATE-unset}");
        assert!(String::from_utf8_lossy(&restarted.output).contains("RESTART_STATE:unset"));
        drop(restarted);
        fs::remove_dir_all(root).unwrap();
    }
}
