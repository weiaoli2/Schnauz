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
        if visible && std::env::var_os("SCHNAUZ_PREVIEW_CHECK_VISIBLE").is_some() {
            check_previews(window, cx);
            return;
        }
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
        window.update(cx, |fixture, _, cx| fixture.browser.update(cx, |browser, cx| {
            file_apps::prepare_fixture(browser, cx);
        })).unwrap();
        for _ in 0..3 { cx.update_window(*window, |_,window,cx| {window.refresh();window.draw(cx).clear();}).unwrap(); }
        BOUNDS.with(|all| {
            let all = all.borrow();
            let dialog = all.get("file-app-dialog").expect("App-choice dialog painted");
            let title = all.get("file-app-title").expect("App-choice title painted");
            assert!(f32::from(title.size.width) > 350. && f32::from(title.size.height) < 85.);
            for name in ["file-app-once", "file-app-always"] {
                let button = all.get(name).expect("App-choice button painted");
                assert!(f32::from(button.size.width) >= 35. && f32::from(button.size.height) >= 28.);
                assert!(dialog.contains(&button.origin) && dialog.contains(&button.bottom_right()));
            }
        });
        window.update(cx, |fixture, _, cx| fixture.browser.update(cx, |browser, cx| {
            browser.file_app_choice = None; cx.notify();
        })).unwrap();
        if !visible {
            settings::check_quick_keybinds(cx);
            check_previews(window, cx);
            check_cloud_alias(window, cx);
            drag_checks::run(window, cleanup, cx);
        }
    });
}

#[cfg(feature = "ui-checks")]
fn check_previews(window: gpui::WindowHandle<Fixture>, cx: &mut App) {
    let select = |name: &str, dimensions: (u32, u32), cx: &mut App| {
        window.update(cx, |fixture, _, cx| fixture.browser.update(cx, |browser, cx| {
            browser.todoist.open = false;
            let path = browser.active_tab().current_dir.join(name);
            INFO_CACHE.with(|cache| cache.borrow_mut().insert(path.clone(), FileInfo {
                kind: "PNG image".into(), size: "1.2 MB".into(), created: "18 Oct 2026".into(),
                modified: "18 Oct 2026".into(), accessed: "18 Oct 2026".into(),
                date_times: std::array::from_fn(|_| ("18.10.26".into(), "18:48".into())),
                dimensions: Some(format!("{} × {}", dimensions.0, dimensions.1)), color: Some("RGB".into()), signed: None,
            }));
            let image = image::RgbaImage::from_pixel(dimensions.0, dimensions.1, image::Rgba([80, 120, 180, 255]));
            PREVIEW_CACHE.with(|cache| cache.borrow_mut().insert(path.clone(), Some(Arc::new(RenderImage::new(vec![image::Frame::new(image)])))));
            browser.tab_mut(browser.active_pane).selection = HashSet::from([path.clone()]);
            browser.tab_mut(browser.active_pane).anchor = Some(path);
            cx.notify();
        })).unwrap();
    };
    let paint = |cx: &mut App| {
        for _ in 0..4 { cx.update_window(*window, |_, window, cx| { window.refresh(); window.draw(cx).clear(); }).unwrap(); }
    };
    for (name, dimensions, width) in [("landscape-min.png", (1600, 800), 253), ("landscape.png", (1600, 800), 360), ("portrait.png", (800, 1600), 420)] {
        window.update(cx, |fixture, _, cx| fixture.browser.update(cx, |browser, _| {
            browser.tab_mut(browser.active_pane).preview_width = width;
        })).unwrap();
        select(name, dimensions, cx);
        paint(cx);
        let (frame, image) = BOUNDS.with(|all| {
            let all = all.borrow(); (*all.get("preview-frame").unwrap(), *all.get("preview-image").unwrap())
        });
        if dimensions.0 > dimensions.1 {
            BOUNDS.with(|all| {
                let all = all.borrow();
                let info = all.get("preview-frame").unwrap();
                TEXT_LAYOUTS.with(|layouts| {
                    let layouts = layouts.borrow();
                    for (key, expected) in [("section-heading", "INFORMATION"), ("preview-kind-text", "PNG image"),
                        ("preview-size-text", "1.2 MB"), ("preview-created-date-text", "18.10.26"),
                        ("preview-created-time-text", "18:48"),
                        ("preview-modified-date-text", "18.10.26"), ("preview-modified-time-text", "18:48"),
                        ("preview-accessed-date-text", "18.10.26"), ("preview-accessed-time-text", "18:48"),
                        ("preview-dimensions-text", "1600 × 800"), ("preview-color-text", "RGB")] {
                        let layout = layouts.get(key).expect("Actual text was measured");
                        assert_eq!(layout.text(), expected, "Visible glyph content disappeared for {key}");
                        assert!(layout.bounds().size.width > px(0.), "Zero-width glyph bounds for {key}: {:?}", layout.bounds());
                    }
                });
                let heading = all.get("preview-info-heading").unwrap();
                assert!(f32::from(heading.size.width) >= 200. && f32::from(heading.size.height) <= 18., "Information heading must remain a full-width single line: {heading:?}");
                let row = all.get("preview-info-first-row").unwrap();
                assert!(row.origin.y >= heading.bottom(), "Heading cannot overlap fields");
                assert_eq!(f32::from(row.size.height), 13., "Single-line fields remain compact");
                let date = all.get("preview-created-date").unwrap();
                let time = all.get("preview-created-time").unwrap();
                let label = all.get("preview-created-label").unwrap();
                assert_eq!(date.origin.x, time.origin.x);
                assert_eq!(time.origin.y, date.bottom(), "Time follows date without extra row padding");
                assert_eq!(f32::from(label.size.height), 13.);
                assert_eq!(label.origin.y, date.origin.y);
                let label1 = all.get("preview-info-label-1").unwrap();
                let value1 = all.get("preview-info-value-1").unwrap();
                let label2 = all.get("preview-info-label-2").unwrap();
                let value2 = all.get("preview-info-value-2").unwrap();
                assert_eq!(label1.origin.y, label2.origin.y, "Two label/value pairs share one row");
                assert!(label1.origin.x >= info.origin.x + px(12.));
                assert!(label1.right() < value1.origin.x && value1.right() < label2.origin.x && label2.right() < value2.origin.x);
                assert!(value2.right() <= info.right() - px(12.));
            });
        }
        let aspect = dimensions.1 as f32 / dimensions.0 as f32;
        assert!((f32::from(image.size.height) - f32::from(image.size.width) * aspect).abs() < 1.);
        assert!(f32::from(image.origin.y - frame.origin.y) < 50., "Image starts directly below its title");
        assert!((f32::from(image.size.width) - f32::from(frame.size.width) + 1.).abs() < 2.);
        window.update(cx, |fixture, _, cx| fixture.browser.update(cx, |browser, cx| {
            browser.zoom_visual_preview(20., cx);
        })).unwrap();
        paint(cx);
        window.update(cx, |fixture, _, cx| fixture.browser.update(cx, |browser, _| {
            assert!(browser.preview_zoom.scale > 1., "Wheel delta zooms image");
            assert_eq!(browser.preview_zoom.scroll.offset(), point(px(0.), px(0.)), "Zoom wheel must not also scroll");
        })).unwrap();
        let zoomed = BOUNDS.with(|all| *all.borrow().get("preview-image").unwrap());
        assert!(zoomed.size.width > image.size.width && zoomed.size.height > image.size.height);
    }
    select("next.png", (600, 600), cx); paint(cx);
    window.update(cx, |fixture, _, cx| fixture.browser.update(cx, |browser, _| {
        assert_eq!(browser.preview_zoom.scale, 1., "New selection resets zoom");
    })).unwrap();
    window.update(cx, |fixture, _, cx| fixture.browser.update(cx, |browser, cx| {
        let pane = browser.active_pane;
        let path = browser.active_tab().anchor.clone().unwrap();
        browser.open_context_menu(pane, 20., 20., Some((path.clone(), false)), true, cx);
        let menu = browser.context_menu.as_ref().unwrap();
        assert!(menu.nodes.iter().any(|node| matches!(node, MenuNode::Item { label, .. } if label == "Rotate Left")));
        assert!(!menu.nodes.iter().any(|node| matches!(node, MenuNode::Sub { label, .. } if label == "Quick Actions")));
        browser.context_menu = None;
        browser.open_context_menu(pane, 20., 20., Some((path, false)), false, cx);
        assert!(browser.context_menu.as_ref().unwrap().nodes.iter().any(|node| matches!(node, MenuNode::Sub { label, .. } if label == "Quick Actions")));
        browser.context_menu = None;
    })).unwrap();
    window.update(cx, |fixture, window, cx| fixture.browser.update(cx, |browser, cx| {
        let root = browser.active_tab().current_dir.join("sidebar-view-root");
        fs::create_dir_all(&root).unwrap();
        let child = root.join("unvisited-child");
        let unrelated = root.with_file_name("unrelated-folder");
        let old = folder_views::FolderView::default();
        browser.folder_views.borrow_mut().remember(child.clone(), old.clone(), false);
        browser.folder_views.borrow_mut().remember(unrelated.clone(), old.clone(), false);
        let pane = browser.active_pane;
        browser.tab_mut(pane).sort_key = SortKey::Modified;
        browser.tab_mut(pane).sort_asc = false;
        let directory_before = browser.active_tab().current_dir.clone();
        let shortcuts_before = browser.projects.sidebar().shortcuts.clone();
        let menu = browser.sidebar_menu_nodes(SidebarTarget::Favourite(projects::Shortcut::local(root.clone(), projects::Section::Folders)), cx);
        let apply = menu.into_iter().find_map(|node| match node {
            MenuNode::Item { label, on, .. } if label == "Apply Current View & Sorting to This Folder & Subfolders" => Some(on), _ => None,
        }).expect("Sidebar folder menu offers applying the main view/sort to its tree");
        apply(browser, window, cx);
        let views = browser.folder_views.borrow();
        assert_eq!(views.resolve(&root).unwrap().sort, "modified");
        assert!(!views.resolve(&child).unwrap().ascending);
        assert_eq!(views.resolve(&root), views.resolve(&child));
        assert_eq!(views.resolve(&unrelated), Some(&old));
        assert_eq!(browser.active_tab().current_dir, directory_before);
        assert_eq!(browser.projects.sidebar().shortcuts, shortcuts_before);
    })).unwrap();
    println!("PASS single-line information heading, compact date/time lines at 253/360/420 pt, persistent sidebar visibility and sidebar tree view/sort scope");
    println!("PASS preview aspect, zoom/reset, padded four-column information and Command direct-action menu routing");
    println!("{}", text_view::smoke_test().unwrap());
    if std::env::var_os("SCHNAUZ_PREVIEW_CHECK_VISIBLE").is_some() {
        window.update(cx, |fixture, _, cx| fixture.browser.update(cx, |browser, _| browser.tab_mut(browser.active_pane).preview_width = 360)).unwrap();
        select("sample.png", (1600, 800), cx); paint(cx);
    } else {
        window.update(cx, |fixture, _, cx| fixture.browser.update(cx, |browser, cx| {
            browser.tab_mut(browser.active_pane).selection.clear(); cx.notify();
        })).unwrap();
    }
}

// Inspect the exact glyph input retained by GPUI after measurement/truncation.
// Container geometry alone cannot detect text truncated to an empty string.
#[cfg(feature = "ui-checks")]
thread_local! { static TEXT_LAYOUTS: RefCell<HashMap<&'static str, gpui::TextLayout>> = RefCell::new(HashMap::new()); }
pub(super) fn text(name: &'static str, value: String) -> AnyElement {
    let text = gpui::StyledText::new(value);
    #[cfg(feature = "ui-checks")]
    TEXT_LAYOUTS.with(|all| { all.borrow_mut().insert(name, text.layout().clone()); });
    #[cfg(not(feature = "ui-checks"))]
    let _ = name;
    text.into_any_element()
}

#[cfg(feature = "ui-checks")]
fn check_cloud_alias(window: gpui::WindowHandle<Fixture>, cx: &mut App) {
    let original = cx.global::<nextcloud::State>().clone();
    let alias = config_file("Nextcloud-fixture").unwrap();
    let physical = home_dir().join("Library/CloudStorage/Nextcloud-offline-fixture");
    // No files under the real CloudStorage root are accessed or created.
    cloud_locations::fixture_alias(alias.clone(), physical.clone());
    let account: nextcloud::Account = serde_json::from_value(serde_json::json!({
        "id":"offline-fixture", "root":physical, "server":"https://invalid.example", "login":"fixture", "remoteRoot":"/" })).unwrap();
    let mut state = nextcloud::State::default(); state.accounts = vec![account]; cx.set_global(state);
    window.update(cx, |fixture, _, cx| fixture.browser.update(cx, |browser, cx| {
        for (path, directory) in [(alias.join("Folder"),true), (alias.join("Folder/image.png"),false)] {
            assert_eq!(nextcloud::location(&path, cx).unwrap().root, alias);
            assert_eq!(browser.cloud_state(&path,directory,cx),CloudSync::Available);
            let menu = browser.cloud_menu(0,&path,directory,cx).unwrap();
            match menu { MenuNode::Sub { label, items } => {
                assert_eq!(label,"Nextcloud");
                assert!(items.iter().any(|node| matches!(node,MenuNode::Item {label,..} if label=="Share options…")));
            }, _ => panic!("Missing native Nextcloud submenu") }
        }
    })).unwrap();
    let nested: nextcloud::Account = serde_json::from_value(serde_json::json!({
        "id":"offline-nested", "root":physical.join("Folder"), "server":"https://invalid.example", "login":"fixture", "remoteRoot":"/nested" })).unwrap();
    let mut state = cx.global::<nextcloud::State>().clone(); state.accounts.push(nested); cx.set_global(state);
    assert_eq!(nextcloud::location(&alias.join("Folder/image.png"),cx).unwrap().root,alias.join("Folder"));
    cx.set_global(original);
    println!("PASS Nextcloud alias identity, folder/file cloud badges and connected menu routing; no network, desktop credentials or Keychain accessed");
}
