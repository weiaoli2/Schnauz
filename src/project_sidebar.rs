//! Explicit shortcuts, tags and connections belonging to the active project.
use super::*;
use projects::Section;

impl Shuffle {
    pub(super) fn activate_project_sidebar(&mut self, cx: &mut Context<Self>) {
        let servers = self.projects.sidebar().servers.clone();
        set_active_sftp_servers(servers.clone());
        cx.set_global(SftpServersGlobal(servers));
    }

    pub(super) fn set_project_path(
        &mut self,
        section: Section,
        path: PathBuf,
        add: bool,
        cx: &mut Context<Self>,
    ) {
        let mut candidate = self.projects.clone();
        candidate.sidebar_mut().set_path(section, path, add);
        if let Err(error) = self.persist_library(candidate) {
            self.set_notice(error, cx);
        }
        cx.notify();
    }

    fn share_project_path(
        &mut self,
        section: Section,
        path: PathBuf,
        targets: &[Option<u64>],
        add: bool,
        cx: &mut Context<Self>,
    ) {
        let mut candidate = self.projects.clone();
        for id in targets {
            if let Some(sidebar) = candidate.sidebar_for_mut(*id) {
                sidebar.set_path(section, path.clone(), add);
            }
        }
        if let Err(error) = self.persist_library(candidate) {
            self.set_notice(error, cx);
        }
        cx.notify();
    }

    pub(super) fn project_visibility_menu(&self, section: Section, path: PathBuf) -> MenuNode {
        let mut projects = vec![(None, "Personal".to_string())];
        projects.extend(
            self.projects
                .items
                .iter()
                .map(|p| (Some(p.id), p.name.clone())),
        );
        let targets: Vec<Option<u64>> = projects.iter().map(|(id, _)| *id).collect();
        let all = targets.iter().all(|id| {
            self.projects
                .sidebar_for(*id)
                .is_some_and(|s| s.paths(section).contains(&path))
        });
        let all_path = path.clone();
        let mut items = vec![
            mi_check("All projects", all, move |this, _, cx| {
                this.share_project_path(section, all_path.clone(), &targets, !all, cx)
            }),
            MenuNode::Sep,
        ];
        for (id, name) in projects {
            let checked = self
                .projects
                .sidebar_for(id)
                .is_some_and(|s| s.paths(section).contains(&path));
            let path = path.clone();
            items.push(mi_check(name, checked, move |this, _, cx| {
                this.share_project_path(section, path.clone(), &[id], !checked, cx)
            }));
        }
        MenuNode::Sub {
            label: "View in…".into(),
            items,
        }
    }

    pub(super) fn toggle_project_tags(&mut self, pane: usize, cx: &mut Context<Self>) {
        if self.tab(pane).remote.is_some() {
            return;
        }
        let paths = self.action_targets(pane);
        if paths.is_empty() {
            return;
        }
        let add = !paths
            .iter()
            .all(|p| self.projects.sidebar().tagged.contains(p));
        let mut candidate = self.projects.clone();
        for path in &paths {
            candidate
                .sidebar_mut()
                .set_path(Section::Tagged, path.clone(), add);
        }
        match self.persist_library(candidate) {
            Ok(()) => {
                self.collapsed_sections.remove("TAGGED ITEMS");
                self.set_notice(
                    format!(
                        "{} {} item{} {} this project",
                        if add { "Tagged" } else { "Removed" },
                        paths.len(),
                        if paths.len() == 1 { "" } else { "s" },
                        if add { "in" } else { "from" }
                    ),
                    cx,
                );
            }
            Err(error) => self.set_notice(error, cx),
        }
    }

    pub(super) fn choose_project_folder(&mut self, section: Section, cx: &mut Context<Self>) {
        let project = self.projects.active;
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: true,
            prompt: Some("Add to Project".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await {
                let _ = this.update(cx, |this, cx| {
                    let mut candidate = this.projects.clone();
                    let Some(sidebar) = candidate.sidebar_for_mut(project) else {
                        return;
                    };
                    for path in paths {
                        sidebar.set_path(section, path, true);
                    }
                    if let Err(error) = this.persist_library(candidate) {
                        this.set_notice(error, cx);
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub(super) fn project_folder_section(
        &self,
        items: &mut Vec<AnyElement>,
        key: &mut usize,
        title: &'static str,
        section: Section,
        cx: &Context<Self>,
    ) {
        let add = div()
            .id(SharedString::from(format!("project-add-{title}")))
            .px_1()
            .cursor_pointer()
            .child(toolbar_icons::ui_icon(
                Icon::Plus,
                13.,
                rgb(theme().text_muted),
            ))
            .tooltip(tip("Add folders to this project"))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.choose_project_folder(section, cx)
            }))
            .into_any_element();
        items.push(self.section_header_el(title, Some(add), cx));
        if self.collapsed_sections.contains(title) {
            return;
        }
        let paths = self.projects.sidebar().paths(section);
        if paths.is_empty() {
            items.push(empty_hint("Click + to add a folder").into_any_element());
        }
        for path in paths {
            self.project_path_row(items, key, path, section, cx);
        }
    }

    pub(super) fn project_tagged_section(
        &self,
        items: &mut Vec<AnyElement>,
        key: &mut usize,
        cx: &Context<Self>,
    ) {
        if !self.begin_section(items, "TAGGED ITEMS", cx) {
            return;
        }
        if self.projects.sidebar().tagged.is_empty() {
            items.push(empty_hint("Select items and click the tag icon").into_any_element());
        }
        for path in &self.projects.sidebar().tagged {
            self.project_path_row(items, key, path, Section::Tagged, cx);
        }
    }

    pub(super) fn project_path_row(
        &self,
        items: &mut Vec<AnyElement>,
        key: &mut usize,
        path: &PathBuf,
        section: Section,
        cx: &Context<Self>,
    ) {
        *key += 1;
        let path = path.clone();
        let nav = path.clone();
        let target = SidebarTarget::ProjectPath(section, path.clone());
        let is_dir = cached_is_dir(&path);
        let active = self.active_tab().remote.is_none()
            && (self.active_tab().current_dir == path
                || self.active_tab().selection.contains(&path));
        items.push(
            nav_item(
                path_label(&path),
                display_path(&path),
                icon_element(&path, is_dir),
                *key,
                active,
                cx.listener(move |this, _: &ClickEvent, window, cx| {
                    window.focus(&this.focus);
                    if !nav.exists() {
                        this.set_notice(
                            "This item is unavailable. Reconnect its drive or remove the shortcut."
                                .into(),
                            cx,
                        );
                        return;
                    }
                    let pane = this.active_pane;
                this.unsync_ssh_browser(pane, cx);
                    let is_dir = nav.is_dir();
                    let folder = if is_dir {
                        nav.clone()
                    } else {
                        nav.parent().unwrap_or(Path::new("/")).to_path_buf()
                    };
                    if this.tab(pane).remote.is_some() {
                        let p = this.pane_mut(pane);
                        p.tabs.push(Tab::new(folder.clone()));
                        p.active = p.tabs.len() - 1;
                    }
                    if !is_dir {
                        this.tab_mut(pane).set_entry_filter(EntryFilter::All);
                        use std::os::macos::fs::MetadataExt;
                        if nav
                            .file_name()
                            .is_some_and(|n| n.as_encoded_bytes().starts_with(b"."))
                            || fs::metadata(&nav).is_ok_and(|m| m.st_flags() & libc::UF_HIDDEN != 0)
                        {
                            this.tab_mut(pane).show_hidden = true;
                        }
                    }
                    if this.tab(pane).current_dir == folder {
                        this.load_dir_in(pane, folder, cx);
                    } else {
                        this.navigate_in(pane, folder, cx);
                    }
                    if !is_dir {
                        this.reveal_in_pane(pane, nav.clone(), cx);
                    }
                }),
                cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                    this.open_sidebar_menu(
                        f32::from(ev.position.x),
                        f32::from(ev.position.y),
                        target.clone(),
                        cx,
                    );
                    cx.stop_propagation();
                }),
            )
            .into_any_element(),
        );
    }
}
