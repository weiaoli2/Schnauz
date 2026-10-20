//! Schnauz-only extension associations. Never writes LaunchServices defaults.
use super::*;
use std::collections::BTreeMap;

#[derive(Default)]
struct Associations {
    apps: BTreeMap<String, PathBuf>,
}
impl Associations {
    fn load(file: &Path) -> Result<Self, String> {
        match fs::read(file) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(|apps| Self { apps })
                .map_err(|e| format!("Couldn't read app choices: {e}")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("Couldn't read app choices: {e}")),
        }
    }
    fn save(&mut self, file: &Path, extension: &str, app: Option<&Path>) -> Result<(), String> {
        let mut apps = self.apps.clone();
        if let Some(app) = app {
            apps.insert(extension.into(), app.into());
        } else {
            apps.remove(extension);
        }
        let bytes = serde_json::to_vec_pretty(&apps).map_err(|e| e.to_string())?;
        projects::atomic_write(file, &bytes)
            .map_err(|e| format!("Couldn't save app choice: {e}"))?;
        self.apps = apps;
        Ok(())
    }
}
thread_local! {
    static ASSOCIATIONS: RefCell<Option<Result<Associations, String>>> = const { RefCell::new(None) };
}
fn with_store<T>(
    f: impl FnOnce(&mut Associations, &Path) -> Result<T, String>,
) -> Result<T, String> {
    let file = config_file("file-apps.json").ok_or("Settings folder unavailable")?;
    ASSOCIATIONS.with(|store| {
        let mut store = store.borrow_mut();
        let loaded = store.get_or_insert_with(|| Associations::load(&file));
        match loaded {
            Ok(store) => f(store, &file),
            Err(e) => Err(e.clone()),
        }
    })
}
fn extension(path: &Path) -> Option<String> {
    path.extension()?
        .to_str()
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase)
}
fn preferred(path: &Path) -> Result<Option<PathBuf>, String> {
    // Applications and folders always retain their normal launch/navigation behavior.
    if path.is_dir() {
        return Ok(None);
    }
    let Some(ext) = extension(path) else {
        return Ok(None);
    };
    with_store(|store, _| Ok(store.apps.get(&ext).filter(|app| app.is_dir()).cloned()))
}
fn system_app(path: &Path) -> Option<PathBuf> {
    let url = NSURL::fileURLWithPath(&NSString::from_str(path.to_str()?));
    NSWorkspace::sharedWorkspace()
        .URLForApplicationToOpenURL(&url)?
        .path()
        .map(|p| PathBuf::from(p.to_string()))
}
fn app_name(app: &Path) -> String {
    app.file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}
pub(super) struct Choice {
    path: PathBuf,
    app: PathBuf,
    extension: String,
    error: Option<String>,
}
#[cfg(feature = "ui-checks")]
thread_local! { pub(super) static OPEN_CAPTURE: RefCell<Option<Vec<PathBuf>>> = const { RefCell::new(None) }; }

impl Choice {
    fn remember(&self, always: bool) -> Result<(), String> {
        if always {
            with_store(|store, file| store.save(file, &self.extension, Some(&self.app)))?;
        }
        Ok(())
    }
}
impl Schnauz {
    fn browse_file_app(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.close_context_menu(cx);
        let selected = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose Application".into()),
        });
        cx.spawn(async move |this, cx| {
            let result = selected.await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(Ok(Some(paths))) => {
                    if let Some(app) = paths.first() {
                        if is_app_bundle(app, app.is_dir()) {
                            this.choose_file_app(app, &path, cx);
                        } else {
                            this.set_notice("Choose an application (.app).".into(), cx);
                        }
                    }
                }
                Ok(Ok(None)) => {}
                _ => this.set_notice(
                    "Couldn't open the application chooser. Please try again.".into(),
                    cx,
                ),
            });
        })
        .detach();
    }
    pub(super) fn open_local_file(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        match preferred(&path) {
            Ok(app) => self.launch_file(path, app, cx),
            Err(error) => {
                self.set_notice(error, cx);
            }
        }
    }
    fn launch_file(&mut self, path: PathBuf, app: Option<PathBuf>, cx: &mut Context<Self>) {
        #[cfg(feature = "ui-checks")]
        if OPEN_CAPTURE.with(|capture| {
            let mut capture = capture.borrow_mut();
            if let Some(paths) = capture.as_mut() { paths.push(path.clone()); true } else { false }
        }) { return; }
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let mut command = Command::new("open");
                    if let Some(app) = app {
                        command.arg("-a").arg(app);
                    }
                    let output = command.arg(&path).output().map_err(|e| e.to_string())?;
                    if output.status.success() {
                        Ok(())
                    } else {
                        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
                    }
                })
                .await;
            if let Err(error) = result {
                let _ = this.update(cx, |this, cx| {
                    this.set_notice(format!("Couldn't open file: {error}"), cx)
                });
            }
        })
        .detach();
    }
    pub(super) fn choose_file_app(&mut self, app: &Path, path: &Path, cx: &mut Context<Self>) {
        self.close_context_menu(cx);
        let current = preferred(path).ok().flatten().or_else(|| system_app(path));
        if !path.is_dir() && current.as_deref() != Some(app) {
            if let Some(extension) = extension(path) {
                self.file_app_choice = Some(Choice {
                    path: path.into(),
                    app: app.into(),
                    extension,
                    error: None,
                });
                cx.notify();
                return;
            }
        }
        self.launch_file(path.into(), Some(app.into()), cx);
    }
    pub(super) fn finish_file_app_choice(&mut self, always: bool, cx: &mut Context<Self>) {
        let Some(choice) = self.file_app_choice.as_mut() else {
            return;
        };
        if let Err(error) = choice.remember(always) {
            choice.error = Some(error);
            cx.notify();
            return;
        }
        let choice = self.file_app_choice.take().unwrap();
        self.launch_file(choice.path, Some(choice.app), cx);
        cx.notify();
    }
    pub(super) fn render_file_app_choice(&self, cx: &Context<Self>) -> impl IntoElement {
        let choice = self.file_app_choice.as_ref().unwrap();
        let t = theme();
        let button = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .px_3()
                .py_2()
                .rounded_md()
                .cursor_pointer()
                .text_size(px(UI_FONT_SIZE))
                .bg(rgb(t.hover))
                .text_color(rgb(t.text))
                .hover(|s| s.bg(rgb(t.selected)))
                .child(label)
        };
        modal_backdrop(cx.listener(|this, _, _, cx| { this.file_app_choice = None; cx.notify(); })).child(
            div().w(px(460.)).p_5().rounded_lg().bg(rgb(t.surface)).border_1().border_color(rgb(t.border_strong))
                .shadow_lg().relative().children(ui_checks::probe("file-app-dialog")).flex().flex_col().gap_4()
                .on_mouse_down(MouseButton::Left, |_, _, cx: &mut App| cx.stop_propagation())
                .child(div().text_size(px(16.)).text_color(rgb(t.text)).font_weight(FontWeight::SEMIBOLD)
                    .relative().children(ui_checks::probe("file-app-title"))
                    .child(format!("Always open .{} with {}?", choice.extension, app_name(&choice.app))))
                .child(div().text_size(px(UI_FONT_SIZE)).text_color(rgb(t.text))
                    .child("OK saves this choice for Schnauz. Only Now opens this file once. Your macOS default stays unchanged."))
                .children(choice.error.clone().map(|e| div().text_size(px(UI_FONT_SIZE)).text_color(rgb(0xc23a35)).child(e)))
                .child(div().flex().justify_end().gap_2()
                    .child(button("file-app-cancel", "Cancel").on_click(cx.listener(|this, _, _, cx| { this.file_app_choice = None; cx.notify(); })))
                    .child(button("file-app-once", "Only Now").relative().children(ui_checks::probe("file-app-once")).on_click(cx.listener(|this, _, _, cx| this.finish_file_app_choice(false, cx))))
                    .child(button("file-app-always", "OK").relative().children(ui_checks::probe("file-app-always")).bg(rgb(t.accent)).text_color(rgb(0xffffff))
                        .on_click(cx.listener(|this, _, _, cx| this.finish_file_app_choice(true, cx)))))
        )
    }
}

pub(super) fn nodes(path: &Path) -> Vec<MenuNode> {
    let saved = preferred(path).ok().flatten();
    let current = saved.clone().or_else(|| system_app(path));
    let mut apps = apps_for_file(path);
    if let Some(app) = &saved {
        if !apps.iter().any(|(_, p)| p == app) {
            apps.insert(0, (app_name(app), app.clone()));
        }
    }
    let mut nodes: Vec<_> = apps
        .into_iter()
        .map(|(name, app)| {
            let p = path.to_path_buf();
            let selected = current.as_ref() == Some(&app);
            let icon = app.clone();
            let mut node = mi_check(name, selected, move |this, _, cx| {
                this.choose_file_app(&app, &p, cx)
            });
            if let MenuNode::Item { app_icon, .. } = &mut node {
                *app_icon = Some(icon);
            }
            node
        })
        .collect();
    if saved.is_some() {
        if let Some(ext) = extension(path) {
            nodes.push(MenuNode::Sep);
            nodes.push(mi("Use macOS Default Again", move |this, _, cx| {
                this.close_context_menu(cx);
                let result = with_store(|store, file| store.save(file, &ext, None));
                match result {
                    Ok(()) => this.set_notice(format!(".{} now uses the macOS default", ext), cx),
                    Err(e) => this.set_notice(e, cx),
                }
            }));
        }
    }
    if nodes.is_empty() {
        nodes.push(MenuNode::Disabled("No suggested applications".into()));
    }
    nodes.push(MenuNode::Sep);
    let path = path.to_path_buf();
    nodes.push(mi("Other Application…", move |this, _, cx| {
        this.browse_file_app(path.clone(), cx)
    }));
    nodes
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extension_choices_persist_replace_and_reset_without_touching_system_defaults() {
        let dir = std::env::temp_dir().join(format!("schnauz-app-choice-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("file-apps.json");
        let mut store = Associations::load(&file).unwrap();
        store
            .save(&file, "txt", Some(Path::new("/Applications/First.app")))
            .unwrap();
        store
            .save(&file, "pdf", Some(Path::new("/Applications/PDF.app")))
            .unwrap();
        store
            .save(&file, "txt", Some(Path::new("/Applications/Second.app")))
            .unwrap();
        let mut loaded = Associations::load(&file).unwrap();
        assert_eq!(loaded.apps["txt"], Path::new("/Applications/Second.app"));
        loaded.save(&file, "txt", None).unwrap();
        let loaded = Associations::load(&file).unwrap();
        assert!(!loaded.apps.contains_key("txt"));
        assert!(loaded.apps.contains_key("pdf"));
        fs::write(&file, b"broken").unwrap();
        assert!(Associations::load(&file).is_err());
        fs::remove_dir_all(dir).unwrap();
        assert_eq!(extension(Path::new("a.TXT")).as_deref(), Some("txt"));
        assert_eq!(extension(Path::new(".gitignore")), None);
    }
    #[test]
    fn failed_save_keeps_previous_choice() {
        let mut store = Associations::default();
        store
            .apps
            .insert("txt".into(), PathBuf::from("original.app"));
        // A file cannot be a parent directory, even when running as root.
        let file =
            std::env::temp_dir().join(format!("schnauz-app-choice-failure-{}", std::process::id()));
        fs::write(&file, b"fixture").unwrap();
        assert!(store
            .save(
                &file.join("settings.json"),
                "txt",
                Some(Path::new("other.app"))
            )
            .is_err());
        assert_eq!(store.apps["txt"], Path::new("original.app"));
        fs::remove_file(file).unwrap();
    }
}

// Called only by the hidden UI fixture, with its disposable settings directory.
#[cfg(feature = "ui-checks")]
pub(super) fn prepare_fixture(browser: &mut Schnauz, cx: &mut Context<Schnauz>) {
    let dir = config_dir().unwrap();
    let app = dir.join("Fixture Viewer.app");
    fs::create_dir_all(&app).unwrap();
    native_menus::check_application_icon(&app);
    let path = dir.join("example.schnauzqa");
    fs::write(&path, b"fixture").unwrap();
    browser.choose_file_app(&app, &path, cx);
    let choice = browser
        .file_app_choice
        .as_ref()
        .expect("Different app asks before remembering");
    let file = dir.join("file-apps.json");
    assert!(!file.exists());
    choice.remember(false).unwrap();
    assert!(!file.exists(), "Only Now must not create preferences");
    choice.remember(true).unwrap();
    assert_eq!(preferred(&path).unwrap(), Some(app.clone()));
    assert_eq!(preferred(&dir.join("other.SCHNAUZQA")).unwrap(), Some(app));
    let before = fs::read(&file).unwrap();
    choice.remember(false).unwrap();
    assert_eq!(
        fs::read(&file).unwrap(),
        before,
        "Only Now preserves existing defaults"
    );
    let menu = nodes(&path);
    assert!(menu
        .iter()
        .any(|n| matches!(n, MenuNode::Item { label, .. } if label == "Other Application…")));
    assert!(menu.iter().any(|n| matches!(
        n,
        MenuNode::Item {
            checked: true,
            app_icon: Some(_),
            ..
        }
    )));
    with_store(|store, file| store.save(file, "schnauzqa", None)).unwrap();
    assert_eq!(preferred(&path).unwrap(), None);
    assert!(browser.native_preview_blocked());
    println!("PASS Open With confirmation, Only Now preserves choices, Always persists case-insensitive extensions, menu icons/checkmark, reset and preview blocking; no apps launched");
}
