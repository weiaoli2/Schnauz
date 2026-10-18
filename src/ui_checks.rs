//! Opt-in native layout checks with disposable data; excluded from normal builds.
use super::*;

#[cfg(not(feature = "ui-checks"))]
#[inline]
pub(super) fn probe(_: &'static str) -> Option<AnyElement> {
    None
}

#[cfg(feature = "ui-checks")]
thread_local! { static BOUNDS: RefCell<HashMap<&'static str, Bounds<gpui::Pixels>>> = RefCell::new(HashMap::new()); }
#[cfg(feature = "ui-checks")]
pub(super) fn probe(name: &'static str) -> Option<AnyElement> {
    Some(
        gpui::canvas(
            move |bounds, _, _| {
                BOUNDS.with(|all| {
                    all.borrow_mut().insert(name, bounds);
                });
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full()
        .into_any_element(),
    )
}

#[cfg(feature = "ui-checks")]
#[path = "drag_checks.rs"]
mod drag_checks;

#[cfg(feature = "ui-checks")]
struct Fixture {
    browser: gpui::Entity<Schnauz>,
}
#[cfg(feature = "ui-checks")]
impl Render for Fixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.browser.clone()
    }
}

#[cfg(feature = "ui-checks")]
fn check_geometry(phase: usize) {
    BOUNDS.with(|all| {
        let all = all.borrow();
        for (name, bounds) in all.iter() {
            eprintln!("layout {name}: {:?}", bounds);
        }
        let panel = all.get("todoist-pane").expect("Todoist panel was painted");
        assert!(
            f32::from(panel.size.width) > 300.,
            "Panel must retain its full width"
        );
        let footer = all.get("footer").expect("Footer was painted");
        assert_eq!(
            f32::from(footer.size.height),
            29.,
            "Footer content stays fixed below its one-pixel border"
        );
        let frame = all.get("tools-frame").expect("Tools frame was painted");
        let divider = all.get("tools-divider").expect("Tools divider was painted");
        assert_eq!(
            divider.origin.x, frame.origin.x,
            "Tools resize ONLY at the left edge"
        );
        assert_eq!(f32::from(divider.size.width), 7.);
        let list = all.get("todoist-list").expect("Task viewport was painted");
        assert!(
            f32::from(list.size.width) > 300. && f32::from(list.size.height) > 200.,
            "Tasks need a usable viewport"
        );
        let add = all
            .get("todoist-add")
            .expect("Add task stays visible, including an empty list");
        assert!(f32::from(add.size.width) > 300. && f32::from(add.size.height) == 60.);
        if phase > 0 {
            let task = all
                .get("todoist-task")
                .expect("A real virtual task row was painted");
            let title = all
                .get("todoist-task-title")
                .expect("Task title was painted");
            assert!(f32::from(task.size.width) > 300. && f32::from(title.size.width) > 250.);
            assert!(
                f32::from(title.size.height) < 30.,
                "Task text remains one horizontal line"
            );
        }
        if phase == 2 {
            let text = all
                .get("todoist-status-text")
                .expect("Status text was painted");
            assert!(
                f32::from(text.size.width) > 250. && f32::from(text.size.height) < 90.,
                "Status must not wrap one letter per line"
            );
        }
    });
}

#[cfg(feature = "ui-checks")]
pub(super) fn run() {
    std::panic::set_hook(Box::new(|error| {
        eprintln!("Layout check failed: {error}");
        std::process::exit(1);
    }));
    let visible = std::env::var_os("SCHNAUZ_UI_CHECK_VISIBLE").is_some();
    let directory = std::env::temp_dir().join(format!(
        "schnauz-ui-check-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&directory).unwrap();
    std::env::set_var("SCHNAUZ_CONFIG_DIR", &directory);
    std::env::set_var("SCHNAUZ_START_DIR", &directory);
    // A hung/hidden platform window must fail, never masquerade as a passing check.
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(if visible { 600 } else { 15 }));
        std::process::exit(2);
    });
    let cleanup = directory.clone();
    Application::new().run(move |cx| {
        cx.set_global(ThemeGlobal(theme()));
        cx.set_global(PrefsGlobal(prefs()));
        cx.set_global(nextcloud::State::default());
        cx.set_global(MenuStyleGlobal(MenuStyle::default()));
        cx.set_global(KeymapGlobal(Keymap::defaults()));
        cx.set_global(SftpServersGlobal(Vec::new(), None));
        cx.set_global(FolderViewsGlobal(Rc::new(RefCell::new(folder_views::Library::open(None)))));
        cx.set_global(todoist::Global { busy: true, account: Some(todoist::Account { id:"fixture-account".into(), user_id:"fixture-user".into(), name:"Fixture".into() }), ..Default::default() });
        let mut projects = projects::Library::default();
        projects.sidebar_mut().todoist = Some(projects::TodoistLink { account_id:"fixture-account".into(), project_id:"fixture-project".into(), project_name:"Fixture".into() });
        cx.set_global(ProjectLibraryGlobal(projects, None));
        let bounds = Bounds::new(if visible { point(px(100.), px(100.)) } else { point(px(-4000.), px(-4000.)) }, size(px(980.), px(600.)));
        let window = cx.open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), focus: visible, show: visible, titlebar: Some(TitlebarOptions { title: None, appears_transparent: true, ..Default::default() }), ..Default::default() }, move |_, cx| {
            let browser = cx.new(|cx| {
                let mut browser = Schnauz::new(projects::Shortcut::local(directory.clone(), projects::Section::Folders), cx);
                browser.projects.sidebar_mut().todoist = Some(projects::TodoistLink { account_id:"fixture-account".into(), project_id:"fixture-project".into(), project_name:"Fixture".into() });
                // Start unmeasured, exactly like a newly opened real window.
                browser.todoist.open = true;
                let data: todoist::Snapshot = serde_json::from_value(serde_json::json!({"project":{"id":"fixture-project","name":"Fixture"},"tasks":[],"sections":[]})).unwrap();
                browser.todoist.caches.insert(("fixture-account".into(), "fixture-project".into()), todoist::Cache { data:Some(data), ..Default::default() });
                browser
            });
            cx.new(|_| Fixture { browser })
        }).unwrap();
        for phase in 0..3 {
            BOUNDS.with(|all| all.borrow_mut().clear());
            for _ in 0..4 {
                window.update(cx, |fixture, window, cx| { fixture.browser.update(cx, |_, cx| cx.notify()); window.refresh(); }).unwrap();
                cx.update_window(*window, |_, window, cx| { window.draw(cx).clear(); }).unwrap();
            }
            check_geometry(phase);
            window.update(cx, |fixture, window, cx| {
                let browser = fixture.browser.clone();
                browser.update(cx, |browser, cx| {
                    let data: todoist::Snapshot = serde_json::from_value(serde_json::json!({
                        "project":{"id":"fixture-project","name":"Fixture"}, "sections":[],
                        "tasks":[{"id":"one","project_id":"fixture-project","content":"Upgrade the website","due":{"date":chrono::Local::now().date_naive().to_string()}},
                                 {"id":"two","project_id":"fixture-project","content":"Review the app"}]
                    })).unwrap();
                    let cache = browser.todoist.caches.get_mut(&("fixture-account".into(), "fixture-project".into())).unwrap();
                    // Exercise the same decoded-helper-response -> cache path as production.
                    let mut response = todoist::Response::default(); response.success = true; response.snapshot = Some(data); cache.finish_read(response, "fixture-project");
                    if phase == 1 { cache.error = Some("Cannot reach Todoist. Cached tasks may be out of date.".into()); }
                    browser.notice = Some("Nothing to undo — a long status message stays on one line".into());
                    cx.notify();
                });
                window.refresh();
            }).unwrap();
        }
        // Progress lives inside the existing footer, never above the files.
        let body_before = BOUNDS.with(|all| *all.borrow().get("drag-file-body").unwrap());
        window.update(cx, |fixture, _, cx| fixture.browser.update(cx, |b,cx| {
            let progress=Arc::new(JobProgress::default());
            progress.total.store(100,Ordering::Relaxed);progress.done.store(42,Ordering::Relaxed);
            b.jobs.push(FileJob{id:999,label:"Copying 3 items to SSH".into(),prog:progress});cx.notify();
        })).unwrap();
        for _ in 0..3 { cx.update_window(*window, |_,window,cx| {window.refresh();window.draw(cx).clear();}).unwrap(); }
        BOUNDS.with(|all| {
            let all=all.borrow();let footer=all.get("footer").unwrap();let progress=all.get("footer-transfer").unwrap();
            assert!(f32::from(progress.size.width)>150. && f32::from(progress.size.height)==23.);
            assert!(footer.contains(&progress.origin) && footer.contains(&progress.bottom_right()));
            assert_eq!(*all.get("drag-file-body").unwrap(),body_before,"Transfer status cannot shift file content");
        });
        window.update(cx,|fixture,_,cx|fixture.browser.update(cx,|b,cx|{b.jobs.clear();cx.notify();})).unwrap();
        println!("PASS real Todoist empty/populated/error layouts, horizontal task/status text, always-visible Add task, left-only tools divider and fixed footer; no windows shown or accounts accessed");
        if !visible {
            drag_checks::run(window, cleanup, cx);
        }
    });
}
