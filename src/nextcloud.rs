//! Native Nextcloud bridge. Credentials remain inside the signed Swift helper
//! and macOS Keychain; Rust sees only account routing and operation results.
use super::*;

#[derive(Clone, Debug, serde::Deserialize)]
pub(super) struct Account {
    id: String,
    pub root: PathBuf,
    pub server: String,
    pub login: String,
    #[serde(rename = "remoteRoot")]
    pub remote_root: String,
    #[serde(default)]
    locking: bool,
}
#[derive(serde::Deserialize)]
struct Configuration {
    version: u32,
    accounts: Vec<Account>,
}
#[derive(Clone, Default)]
pub(super) struct State {
    pub accounts: Vec<Account>,
    busy: HashSet<String>,
    pub notice: Option<String>,
}
impl gpui::Global for State {}
impl State {
    pub fn load() -> Self {
        match read_accounts() {
            Ok(accounts) => Self {
                accounts,
                ..Self::default()
            },
            Err(error) => Self {
                notice: Some(error),
                ..Self::default()
            },
        }
    }
}
fn read_accounts() -> Result<Vec<Account>, String> {
    let Some(file) = config_file("nextcloud.json") else {
        return Err("Cannot find Nextcloud settings.".into());
    };
    let bytes = match fs::read(file) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => {
            return Err(
                "Cannot read Nextcloud settings. The existing file has been preserved.".into(),
            )
        }
    };
    if bytes.len() > 1024 * 1024 {
        return Err(
            "Nextcloud settings are too large. The existing file has been preserved.".into(),
        );
    }
    let config: Configuration = serde_json::from_slice(&bytes)
        .map_err(|_| "Nextcloud settings are invalid. The existing file has been preserved.")?;
    let mut roots = HashSet::new();
    if config.version != 1
        || config
            .accounts
            .iter()
            .any(|a| !a.root.is_absolute() || a.id.is_empty() || !roots.insert(a.root.clone()))
    {
        return Err("Nextcloud settings are invalid. The existing file has been preserved.".into());
    }
    Ok(config.accounts)
}
fn matching<'a>(accounts: &'a [Account], path: &Path) -> Option<&'a Account> {
    let physical = cloud_locations::physical_path(path);
    accounts
        .iter()
        .filter(|a| physical.starts_with(cloud_locations::physical_path(&a.root)))
        .max_by_key(|a| cloud_locations::physical_path(&a.root).components().count())
}
pub(super) fn location(path: &Path, cx: &App) -> Option<cloud_locations::Location> {
    matching(&cx.global::<State>().accounts, path).map(|a| cloud_locations::Location {
        root: cloud_locations::display_root(&a.root, path),
        name: "Nextcloud".into(),
    })
}
#[derive(serde::Serialize)]
struct Request {
    operation: String,
    config: PathBuf,
    root: Option<PathBuf>,
    path: Option<PathBuf>,
}
#[derive(serde::Deserialize)]
struct Response {
    success: bool,
    message: String,
    #[serde(default)]
    cancelled: bool,
}

pub(super) fn launch<T: 'static>(
    operation: &str,
    root: Option<PathBuf>,
    path: Option<PathBuf>,
    cx: &mut Context<T>,
) {
    let Some(config) = config_file("nextcloud.json") else {
        return;
    };
    let path = path.map(|path| {
        if let Some(root) = root.as_ref() {
            let physical = cloud_locations::physical_path(&path);
            let physical_root = cloud_locations::physical_path(root);
            if let Ok(relative) = physical.strip_prefix(physical_root) { return root.join(relative); }
        }
        path
    });
    let request = Request {
        operation: operation.into(),
        config,
        root,
        path,
    };
    let Ok(input) = serde_json::to_vec(&request) else {
        return;
    };
    let key = String::from_utf8_lossy(&input).into_owned();
    let mut state = cx.global::<State>().clone();
    if !state.busy.insert(key.clone()) {
        return;
    }
    state.notice = Some("Nextcloud: working…".into());
    cx.set_global(state);
    cx.spawn(async move |_, cx| {
        let result = cx
            .background_spawn(async move {
                use std::io::Write;
                let helper = std::env::current_exe()
                    .map_err(|_| "Cannot locate the Nextcloud helper.".to_string())?
                    .parent()
                    .ok_or("Cannot locate the Nextcloud helper.")?
                    .join("nextcloudctl");
                let mut child = Command::new(helper)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()
                    .map_err(|_| {
                        "The Nextcloud helper is unavailable. Build the packaged Schnauz app."
                            .to_string()
                    })?;
                let sent = child.stdin.take().map(|mut stdin| stdin.write_all(&input));
                if !matches!(sent, Some(Ok(()))) {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("Cannot send the Nextcloud request.".into());
                }
                let output = child
                    .wait_with_output()
                    .map_err(|_| "The Nextcloud helper stopped unexpectedly.".to_string())?;
                let response: Response = serde_json::from_slice(&output.stdout)
                    .map_err(|_| "The Nextcloud helper stopped unexpectedly.".to_string())?;
                if !response.success && !response.cancelled {
                    return Err(response.message);
                }
                Ok(response.message)
            })
            .await;
        let _ = cx.update(|cx| {
            let mut state = cx.global::<State>().clone();
            state.busy.remove(&key);
            if let Ok(accounts) = read_accounts() {
                state.accounts = accounts;
            }
            state.notice = Some(match result {
                Ok(message) | Err(message) => message,
            });
            cx.set_global(state);
        });
    })
    .detach();
}

pub(super) fn menu_items(path: &Path, is_dir: bool, cx: &Context<Schnauz>) -> Vec<MenuNode> {
    let state = cx.global::<State>();
    let account = matching(&state.accounts, path);
    let detected = cloud_locations::identify(&home_dir(), path).filter(|c| c.name == "Nextcloud");
    let Some(root) = account
        .map(|a| a.root.clone())
        .or_else(|| detected.map(|c| c.root))
    else {
        return Vec::new();
    };
    let mut items = Vec::new();
    if let Some(account) = account {
        items.push(MenuNode::Disabled(format!("Connected · {}", account.login)));
        for (operation, label) in [
            ("shares", "Share options…"),
            ("open", "Open in browser"),
            ("copy-link", "Copy internal link"),
        ] {
            let root = root.clone();
            let path = path.to_path_buf();
            items.push(mi(label, move |this, _, cx| {
                this.close_context_menu(cx);
                launch(operation, Some(root.clone()), Some(path.clone()), cx);
            }));
        }
        if !is_dir {
            if account.locking {
                for (operation, label) in [("lock", "Lock file"), ("unlock", "Unlock file")] {
                    let root = root.clone();
                    let path = path.to_path_buf();
                    items.push(mi(label, move |this, _, cx| {
                        this.close_context_menu(cx);
                        launch(operation, Some(root.clone()), Some(path.clone()), cx);
                    }));
                }
            } else {
                items.push(MenuNode::Disabled(
                    "File locking is not enabled on this server".into(),
                ));
            }
        }
        items.push(MenuNode::Sep);
    }
    let connected = account.is_some();
    let connect_root = root.clone();
    items.push(mi(
        if connected {
            "Reconnect / folder mapping…"
        } else {
            "Connect this folder to Nextcloud…"
        },
        move |this, _, cx| {
            this.close_context_menu(cx);
            launch("connect", Some(connect_root.clone()), None, cx);
        },
    ));
    if connected {
        let refresh_root = root.clone();
        items.push(mi("Refresh server features", move |this, _, cx| {
            this.close_context_menu(cx);
            launch("refresh", Some(refresh_root.clone()), None, cx);
        }));
        items.push(mi("Disconnect from Schnauz…", move |this, _, cx| {
            this.close_context_menu(cx);
            launch("disconnect", Some(root.clone()), None, cx);
        }));
    }
    items
}

pub(super) fn settings_rows<T: 'static>(cx: &Context<T>) -> Vec<AnyElement> {
    let mut rows = vec![div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .child(div().child("Nextcloud accounts"))
        .child(
            div()
                .id("connect-nextcloud")
                .px_3()
                .py_1()
                .rounded_md()
                .cursor_pointer()
                .bg(rgb(theme().surface))
                .hover(|d| d.bg(rgb(theme().hover)))
                .child("Connect folder…")
                .on_click(
                    cx.listener(|_, _: &ClickEvent, _, cx| launch("connect", None, None, cx)),
                ),
        )
        .into_any_element()];
    for (index, account) in cx.global::<State>().accounts.iter().enumerate() {
        let root = account.root.clone();
        let disconnect = root.clone();
        rows.push(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(format!("{} · {}", account.login, account.server))
                        .child(
                            div()
                                .text_size(px(UI_SMALL_FONT_SIZE))
                                .text_color(rgb(theme().text_muted))
                                .child(format!("{} → {}", root.display(), account.remote_root)),
                        ),
                )
                .child(
                    div()
                        .id(("nextcloud-account", index))
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .cursor_pointer()
                        .child("Manage…")
                        .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                            launch("connect", Some(root.clone()), None, cx)
                        })),
                )
                .child(
                    div()
                        .id(("nextcloud-disconnect", index))
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .cursor_pointer()
                        .child("Disconnect…")
                        .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                            launch("disconnect", Some(disconnect.clone()), None, cx)
                        })),
                )
                .into_any_element(),
        );
    }
    rows.push(div().text_size(px(UI_SMALL_FONT_SIZE)).text_color(rgb(theme().text_muted))
        .child("Sign in through your browser. Schnauz keeps its own app credential in macOS Keychain; your Nextcloud desktop account stays unchanged.").into_any_element());
    if let Some(notice) = &cx.global::<State>().notice {
        rows.push(
            div()
                .text_size(px(UI_SMALL_FONT_SIZE))
                .child(notice.clone())
                .into_any_element(),
        );
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_folder_links_use_component_boundaries_and_longest_root() {
        let account = |root: &str| Account {
            id: root.into(),
            root: root.into(),
            server: "https://cloud.example".into(),
            login: "user".into(),
            remote_root: "/".into(),
            locking: false,
        };
        let accounts = vec![account("/cloud"), account("/cloud/Projects")];
        assert_eq!(
            matching(&accounts, Path::new("/cloud/Projects/file.txt"))
                .unwrap()
                .root,
            Path::new("/cloud/Projects")
        );
        assert!(matching(&accounts, Path::new("/cloud-copy/file.txt")).is_none());
    }
}
