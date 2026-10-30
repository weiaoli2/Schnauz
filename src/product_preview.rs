//! Reproducible marketing captures. Synthetic files/tasks and an owned test PTY;
//! excluded from normal builds. No accounts, Keychain or live config are loaded.
use super::*;

pub(super) fn run() {
    let initial_scene = std::env::args()
        .find_map(|arg| {
            arg.strip_prefix("--website-preview-scene=")
                .and_then(|n| n.parse::<usize>().ok())
        })
        .unwrap_or(0)
        .min(3);
    let initial_project = usize::from(initial_scene == 1);
    let root = PathBuf::from("/private/tmp").join(format!("Schnauz Studio {}", std::process::id()));
    let profile = root.join(".preview-profile");
    fs::create_dir_all(&profile).unwrap();
    std::env::set_var("SCHNAUZ_CONFIG_DIR", &profile);
    std::env::set_var("SCHNAUZ_PREVIEW_SHELL", "1");
    std::env::set_var("SCHNAUZ_START_DIR", &root);
    let names = ["Studio Website", "Brand Refresh", "Product Launch"];
    let folders = ["Design", "Development", "Documents", "Releases"];
    let mut library = projects::Library::default();
    for name in names {
        let directory = root.join(name);
        for folder in folders {
            fs::create_dir_all(directory.join(folder)).unwrap();
        }
        fs::write(directory.join("Project brief.txt"),format!("{name}\n\nA clear workspace for the next good idea.\n\nFOCUS THIS WEEK\nRefine the visual direction.\nBuild and review the main pages.\nPrepare a thoughtful launch.\n\nFiles, terminal and tasks stay together.\nEvery project keeps its own favourite places,\ntabs and layout.\n")).unwrap();
        fs::write(directory.join("Launch checklist.md"),"# Launch checklist\n\n- Review the design\n- Check the website\n- Prepare the release notes\n").unwrap();
        fs::write(directory.join("Development/README.md"),format!("# {name}\n\nBuild something worth sharing.\n\nThis file was reached by pasting its path\ninto the Schnauz window. Its folder opens\nand the file is selected automatically.\n")).unwrap();
        fs::write(
            directory.join("Documents/Notes.txt"),
            "Keep your project notes close.\n",
        )
        .unwrap();
        let saved = |path: PathBuf| projects::SavedTab {
            path,
            view: "list".into(),
            sort: "name".into(),
            ascending: true,
            filter: EntryFilter::All,
            show_hidden: false, network:None,
        };
        let tabs = vec![
            saved(directory.clone()),
            saved(directory.join(if name == "Brand Refresh" {
                "Design"
            } else {
                "Development"
            })),
        ];
        let id = library
            .create(
                name,
                projects::Layout {
                    panes: vec![projects::SavedPane { tabs, active: 0 }],
                    active_pane: 0,
                    split_ratio: 0.5,
                },
            )
            .unwrap();
        let project = library.items.iter_mut().find(|p| p.id == id).unwrap();
        for folder in if name == "Brand Refresh" {
            vec!["Design", "Documents"]
        } else {
            vec!["Development", "Documents", "Releases"]
        } {
            project
                .sidebar
                .set_path(projects::Section::Folders, directory.join(folder), true);
        }
        project.sidebar.set_path(
            projects::Section::Files,
            directory.join("Project brief.txt"),
            true,
        );
        project.sidebar.todoist = Some(projects::TodoistLink {
            account_id: "preview-account".into(),
            project_id: format!("preview-{id}"),
            project_name: name.into(),
        });
        project.sidebar.todoist_create_request = None;
    }
    library.active = Some(library.items[initial_project].id);
    let studio = root.join(names[initial_project]);
    Application::new().run(move|cx| {
        set_active_theme(Theme::default());
        cx.set_global(ThemeGlobal(Theme::default()));
        cx.set_menus(vec![Menu { name: "Schnauz Website Preview".into(), items: vec![MenuItem::action("Quit Preview", Quit)] }]);
        cx.on_action(|_:&Quit,cx:&mut App|cx.quit());
        cx.bind_keys([KeyBinding::new("cmd-q",Quit,None)]);
        let mut p=prefs();p.file_access_intro_seen=true;p.sidebar_collapsed=false;p.preview=false;p.tools_width=350;
        set_active_prefs(p);cx.set_global(PrefsGlobal(p));
        cx.set_global(nextcloud::State::default());
        cx.set_global(KeymapGlobal(Keymap::defaults()));
        cx.set_global(SftpServersGlobal(Vec::new(),None));
        cx.set_global(FolderViewsGlobal(Rc::new(RefCell::new(folder_views::Library::open(None)))));
        // The real Tasks UI, fed sample data. Busy disables automatic refresh and account
        // actions; no token or account files are present in the isolated profile.
        cx.set_global(todoist::Global{busy:true,account:Some(todoist::Account{id:"preview-account".into(),user_id:"preview-user".into(),name:"Sample workspace".into()}),..Default::default()});
        cx.set_global(ProjectLibraryGlobal(library.clone(),None));
        let bounds=Bounds::centered(None,size(px(1280.),px(820.)),cx);
        cx.open_window(WindowOptions{window_bounds:Some(WindowBounds::Windowed(bounds)),window_min_size:Some(size(px(980.),px(600.))),titlebar:Some(TitlebarOptions{title:Some("Schnauz — Website Preview".into()),appears_transparent:true,..Default::default()}),..Default::default()},move |window,cx| {
            let browser=cx.new(|cx| {
                let mut b=Schnauz::new(projects::Shortcut::local(studio.clone(),projects::Section::Folders),cx);
                let layout=library.items[initial_project].layout.clone();b.apply_layout(&layout);
                b.projects=library;
                b.apply_all_folder_views();
                b.todoist.open=true;
                for project in &b.projects.items {
                    let id=project.id;
                    let titles=if project.name=="Brand Refresh" {vec!["Explore the visual direction","Review the colour palette","Prepare the brand guidelines"]} else {vec!["Refine the home page","Review the project brief","Prepare the launch checklist"]};
                    let tasks:Vec<_>=titles.into_iter().enumerate().map(|(i,title)|serde_json::json!({"id":format!("task-{id}-{i}"),"project_id":format!("preview-{id}"),"content":title,"description":"Sample task for the Schnauz product preview.","priority":if i==0{3}else{1},"due":if i==0{serde_json::json!({"date":chrono::Local::now().date_naive().to_string()})}else{serde_json::Value::Null}})).collect();
                    let data:todoist::Snapshot=serde_json::from_value(serde_json::json!({"project":{"id":format!("preview-{id}"),"name":project.name},"tasks":tasks,"sections":[]})).unwrap();
                    let mut cache=todoist::Cache::default();
                    let mut response=todoist::Response::default();response.success=true;response.snapshot=Some(data);
                    cache.finish_read(response,&format!("preview-{id}"));
                    todoist_window::shared(cx).update(cx,|tasks,cx| {
                        tasks.todoist.caches.insert(("preview-account".into(),format!("preview-{id}")),cache);
                        cx.notify();
                    });
                }
                if initial_scene == 0 || initial_scene == 3 {
                    b.start_fixture_terminal(0,cx);
                    b.tab_mut(0).terminal_height=250.;
                }
                scene(&mut b,initial_scene,cx);
                b
            });
            window.focus(&browser.read(cx).focus);
            browser
        }).unwrap();

    });
}

fn scene(b: &mut Schnauz, scene: usize, cx: &mut Context<Schnauz>) {
    let project = if scene == 1 {
        b.projects.items[1].id
    } else {
        b.projects.items[0].id
    };
    b.switch_project(Some(project), cx).unwrap();
    if scene == 0 || scene == 3 {
        b.select_tab(0, 0, cx);
        let root = config_dir()
            .unwrap()
            .parent()
            .unwrap()
            .join("Studio Website");
        b.navigate_in(0, root, cx);
        b.tab_mut(0).selection.clear();
        b.tab_mut(0).anchor = None;
    } else if scene == 1 {
        let root = config_dir()
            .unwrap()
            .parent()
            .unwrap()
            .join("Brand Refresh");
        b.select_tab(0, 0, cx);
        b.navigate_in(0, root.clone(), cx);
        b.reveal_in_pane(0, root.join("Project brief.txt"), cx);
    } else {
        let root = config_dir()
            .unwrap()
            .parent()
            .unwrap()
            .join("Studio Website");
        b.navigate_pasted_path(root.join("Development/README.md").to_str().unwrap(), cx);
    }
    b.todoist.open = true;
    let mut p = prefs();
    p.preview = scene == 1 || scene == 2;
    apply_prefs(p, cx);
    theme::apply_theme(
        if scene == 3 {
            theme::dark_theme(0x171923, 0xe9ecf5, 0x8d96ff)
        } else {
            Theme::default()
        },
        cx,
    );
    cx.notify();
}
