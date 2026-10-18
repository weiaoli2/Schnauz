//! Compact project favourites. Local and remote drops create references only.
use super::*;
use projects::{Section, Shortcut};

pub(super) struct RemoteDrag {
    pub items: Vec<Shortcut>,
    pub position: gpui::Point<gpui::Pixels>,
}

pub(super) fn text_width(window: &Window, text: &str, font_size: f32) -> f32 {
    let text: SharedString = text.to_string().into();
    let run = gpui::TextRun {
        len: text.len(),
        font: window.text_style().font(),
        color: rgb(theme().text).into(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    f32::from(
        window
            .text_system()
            .shape_line(text, px(font_size), &[run], None)
            .width,
    )
}

fn fitted_width(content: f32, window_width: f32) -> f32 {
    content
        .ceil()
        .clamp(170., (window_width * 0.3).clamp(170., 320.))
}

impl Schnauz {
    pub(super) fn activate_project_sidebar(&mut self, cx: &mut Context<Self>) {
        let servers = self.projects.sidebar().servers.clone();
        set_active_sftp_servers(servers.clone());
        cx.set_global(SftpServersGlobal(servers));
    }

    pub(super) fn set_project_shortcut(
        &mut self,
        shortcut: Shortcut,
        add: bool,
        cx: &mut Context<Self>,
    ) {
        let mut candidate = self.projects.clone();
        candidate.sidebar_mut().set_shortcut(shortcut, add);
        match self.persist_library(candidate) {
            Ok(()) if add => self.expand_favourites(),
            Err(error) => self.set_notice(error, cx),
            _ => {}
        }
        cx.notify();
    }

    fn share_project_shortcut(
        &mut self,
        shortcut: Shortcut,
        targets: &[Option<u64>],
        add: bool,
        cx: &mut Context<Self>,
    ) {
        let mut candidate = self.projects.clone();
        for id in targets {
            if let Some(sidebar) = candidate.sidebar_for_mut(*id) {
                sidebar.set_shortcut(shortcut.clone(), add);
            }
        }
        if let Err(error) = self.persist_library(candidate) {
            self.set_notice(error, cx);
        }
        cx.notify();
    }

    pub(super) fn project_visibility_menu(&self, shortcut: Shortcut) -> MenuNode {
        let mut projects = vec![(None, "Personal".to_string())];
        projects.extend(
            self.projects
                .items
                .iter()
                .map(|p| (Some(p.id), p.name.clone())),
        );
        let targets: Vec<_> = projects.iter().map(|(id, _)| *id).collect();
        let all = targets.iter().all(|id| {
            self.projects
                .sidebar_for(*id)
                .is_some_and(|s| s.contains(&shortcut))
        });
        let all_shortcut = shortcut.clone();
        let mut items = vec![
            mi_check("All projects", all, move |this, _, cx| {
                this.share_project_shortcut(all_shortcut.clone(), &targets, !all, cx)
            }),
            MenuNode::Sep,
        ];
        for (id, name) in projects {
            let checked = self
                .projects
                .sidebar_for(id)
                .is_some_and(|s| s.contains(&shortcut));
            let shortcut = shortcut.clone();
            items.push(mi_check(name, checked, move |this, _, cx| {
                this.share_project_shortcut(shortcut.clone(), &[id], !checked, cx)
            }));
        }
        MenuNode::Sub {
            label: "View in…".into(),
            items,
        }
    }

    pub(super) fn shortcut_for_path(&self, pane: usize, path: PathBuf) -> Shortcut {
        let tab = self.tab(pane);
        let folder = path == tab.current_dir
            || tab
                .entries
                .iter()
                .any(|e| tab.current_dir.join(&e.name) == path && e.is_folder())
            || (self.search_in_pane(pane)
                && self.palette_items.iter().any(|item| {
                    matches!(&item.action, Action::Open(p, _) if p == &path)
                        && is_browsable_folder(&path, item.is_dir)
                }));
        Shortcut {
            path,
            kind: if folder {
                Section::Folders
            } else {
                Section::Files
            },
            server: tab.remote.clone(),
        }
    }

    pub(super) fn selection_is_favourite(&self, pane: usize) -> bool {
        !self.tab(pane).selection.is_empty()
            && self.tab(pane).selection.iter().all(|p| {
                self.projects
                    .sidebar()
                    .contains(&self.shortcut_for_path(pane, p.clone()))
            })
    }

    pub(super) fn toggle_project_tags(&mut self, pane: usize, cx: &mut Context<Self>) {
        let shortcuts: Vec<_> = self
            .action_targets(pane)
            .into_iter()
            .map(|p| self.shortcut_for_path(pane, p))
            .collect();
        if shortcuts.is_empty() {
            return;
        }
        let add = !shortcuts
            .iter()
            .all(|s| self.projects.sidebar().contains(s));
        let count = shortcuts.len();
        let mut candidate = self.projects.clone();
        for shortcut in shortcuts {
            candidate.sidebar_mut().set_shortcut(shortcut, add);
        }
        match self.persist_library(candidate) {
            Ok(()) => {
                self.expand_favourites();
                self.set_notice(
                    format!(
                        "{} {count} item{} {} Favourites",
                        if add { "Added" } else { "Removed" },
                        if count == 1 { "" } else { "s" },
                        if add { "to" } else { "from" }
                    ),
                    cx,
                );
            }
            Err(error) => self.set_notice(error, cx),
        }
    }

    fn expand_favourites(&mut self) {
        for title in ["FAVOURITES", "FOLDERS", "FILES"] {
            self.collapsed_sections.remove(title);
        }
    }

    pub(super) fn add_dropped_favourites(
        &mut self,
        project: Option<u64>,
        paths: Vec<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let shortcuts = cx
                .background_spawn(async move {
                    paths
                        .into_iter()
                        .filter_map(|path| {
                            let metadata = fs::metadata(&path).ok()?;
                            let kind = if is_browsable_folder(&path, metadata.is_dir()) {
                                Section::Folders
                            } else {
                                Section::Files
                            };
                            Some(Shortcut::local(path, kind))
                        })
                        .collect::<Vec<_>>()
                })
                .await;
            let _ = this.update(cx, |this, cx| this.add_favourites(project, shortcuts, cx));
        })
        .detach();
    }

    fn add_favourites(
        &mut self,
        project: Option<u64>,
        shortcuts: Vec<Shortcut>,
        cx: &mut Context<Self>,
    ) {
        let mut candidate = self.projects.clone();
        let Some(sidebar) = candidate.sidebar_for_mut(project) else {
            return;
        };
        let before = sidebar.shortcuts.len();
        for shortcut in shortcuts {
            sidebar.set_shortcut(shortcut, true);
        }
        let added = sidebar.shortcuts.len() - before;
        if candidate == self.projects {
            self.set_notice("No new favourites to add.".into(), cx);
        } else if let Err(error) = self.persist_library(candidate) {
            self.set_notice(error, cx);
        } else {
            if self.projects.active == project {
                self.expand_favourites();
            }
            self.set_notice(
                if added == 0 {
                    "Favourites updated".into()
                } else {
                    format!(
                        "Added {added} favourite{}",
                        if added == 1 { "" } else { "s" }
                    )
                },
                cx,
            );
        }
        cx.notify();
    }

    pub(super) fn remote_drop_hover(&self) -> bool {
        !prefs().sidebar_collapsed
            && self.favourite_drag.as_ref().is_some_and(|drag| {
                self.sidebar_bounds
                    .borrow()
                    .is_some_and(|bounds| bounds.contains(&drag.position))
            })
    }

    pub(super) fn finish_favourite_drag(
        &mut self,
        position: gpui::Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = self.favourite_drag.take() else {
            return;
        };
        if !prefs().sidebar_collapsed
            && self
                .sidebar_bounds
                .borrow()
                .is_some_and(|bounds| bounds.contains(&position))
        {
            self.add_favourites(self.projects.active, drag.items, cx);
        }
        cx.notify();
    }

    pub(super) fn render_favourite_drag(&self) -> Option<AnyElement> {
        let drag = self.favourite_drag.as_ref()?;
        let label = if drag.items.len() == 1 {
            path_label(&drag.items[0].path)
        } else {
            format!("{} items", drag.items.len())
        };
        Some(
            div()
                .absolute()
                .left(drag.position.x + px(14.))
                .top(drag.position.y + px(14.))
                .px_2()
                .py_1()
                .rounded_md()
                .bg(rgb(theme().surface))
                .border_1()
                .border_color(rgb(theme().accent))
                .shadow_md()
                .flex()
                .items_center()
                .gap_2()
                .child(ui_icon(Icon::Globe, 14., rgb(theme().accent)))
                .child(label)
                .into_any_element(),
        )
    }

    pub(super) fn favourites_section(
        &self,
        items: &mut Vec<AnyElement>,
        key: &mut usize,
        cx: &Context<Self>,
    ) {
        items.push(self.section_header_el("FAVOURITES", None, cx));
        if self.collapsed_sections.contains("FAVOURITES") {
            return;
        }
        for (kind, key_name, label) in [
            (Section::Folders, "FOLDERS", "Folders"),
            (Section::Files, "FILES", "Files"),
        ] {
            if !self
                .projects
                .sidebar()
                .shortcuts
                .iter()
                .any(|s| s.kind == kind)
            {
                continue;
            }
            items.push(
                div()
                    .id(key_name)
                    .px_4()
                    .pt_2()
                    .pb_0p5()
                    .text_size(px(UI_SMALL_FONT_SIZE))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme().text))
                    .cursor_pointer()
                    .child(label)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.toggle_section(key_name.into(), cx)
                    }))
                    .into_any_element(),
            );
            if self.collapsed_sections.contains(key_name) {
                continue;
            }
            for shortcut in self
                .projects
                .sidebar()
                .shortcuts
                .iter()
                .filter(|s| s.kind == kind)
            {
                self.project_shortcut_row(items, key, shortcut, cx);
            }
        }
    }

    fn favourite_icon(&self, shortcut: &Shortcut, cx: &Context<Self>) -> AnyElement {
        let network = shortcut.server.is_some() || self.network_volume_path(&shortcut.path);
        let cloud = !network
            && (nextcloud::location(&shortcut.path, cx).is_some()
                || cloud_locations::identify(&home_dir(), &shortcut.path).is_some());
        let icon = icon_element(&shortcut.path, shortcut.kind == Section::Folders);
        if !network && !cloud {
            return icon;
        }
        div()
            .relative()
            .flex_none()
            .w(px(19.))
            .h(px(19.))
            .child(icon)
            .child(
                div()
                    .absolute()
                    .right(px(-2.))
                    .bottom(px(-2.))
                    .w(px(12.))
                    .h(px(12.))
                    .rounded_full()
                    .bg(rgb(theme().sidebar))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(ui_icon(
                        if network { Icon::Globe } else { Icon::Cloud },
                        11.,
                        rgb(theme().accent),
                    )),
            )
            .into_any_element()
    }

    fn project_shortcut_row(
        &self,
        items: &mut Vec<AnyElement>,
        key: &mut usize,
        shortcut: &Shortcut,
        cx: &Context<Self>,
    ) {
        *key += 1;
        let nav = shortcut.clone();
        let target = SidebarTarget::Favourite(shortcut.clone());
        let same_server = match (&self.active_tab().remote, &shortcut.server) {
            (None, None) => true,
            (Some(a), Some(b)) => projects::same_server(a, b),
            _ => false,
        };
        let active = same_server
            && (self.active_tab().current_dir == shortcut.path
                || self.active_tab().selection.contains(&shortcut.path));
        let tooltip = shortcut.server.as_ref().map_or_else(
            || display_path(&shortcut.path),
            |s| format!("{}\n{}", s.display(), shortcut.path.display()),
        );
        items.push(
            nav_item(
                path_label(&shortcut.path),
                tooltip,
                self.favourite_icon(shortcut, cx),
                *key,
                active,
                cx.listener(move |this, _: &ClickEvent, window, cx| {
                    window.focus(&this.focus);
                    this.open_favourite(nav.clone(), cx);
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

    pub(super) fn open_favourite(&mut self, shortcut: Shortcut, cx: &mut Context<Self>) {
        let pane = self.active_pane;
        let folder = if shortcut.kind == Section::Folders {
            shortcut.path.clone()
        } else {
            shortcut
                .path
                .parent()
                .unwrap_or(Path::new("/"))
                .to_path_buf()
        };
        if let Some(server) = shortcut.server {
            let live = self
                .panes
                .iter()
                .flat_map(|p| p.tabs.iter())
                .filter_map(|t| t.terminal.as_ref()?.ssh.as_ref().map(|link| &link.server))
                .find(|s| projects::same_server(s, &server))
                .cloned()
                .or_else(|| {
                    self.projects
                        .sidebar()
                        .servers
                        .iter()
                        .find(|s| projects::same_server(s, &server))
                        .cloned()
                })
                .unwrap_or(server);
            // Keep a live terminal tab intact. Browser-only remote tabs can be
            // reused, with no commands or directory changes sent to any terminal.
            let reuse = self.tab(pane).terminal.is_none()
                && self
                    .tab(pane)
                    .remote
                    .as_ref()
                    .is_some_and(|s| projects::same_server(s, &live));
            if reuse {
                self.tab_mut(pane).remote = Some(live);
                if self.tab(pane).current_dir == folder {
                    self.load_dir_in(pane, folder, cx);
                } else {
                    self.navigate_in(pane, folder, cx);
                }
            } else {
                let mut tab = Tab::new(folder.clone());
                tab.remote = Some(live);
                tab.current_dir = folder.clone();
                tab.history = vec![folder];
                self.panes[pane].tabs.push(tab);
                self.panes[pane].active = self.panes[pane].tabs.len() - 1;
                self.reload_pane(pane, cx);
            }
            if shortcut.kind == Section::Files {
                self.tab_mut(pane).set_entry_filter(EntryFilter::All);
                self.tab_mut(pane).selection.insert(shortcut.path.clone());
                if shortcut
                    .path
                    .file_name()
                    .is_some_and(|name| name.as_encoded_bytes().starts_with(b"."))
                {
                    self.tab_mut(pane).show_hidden = true;
                }
                self.tab_mut(pane).anchor = Some(shortcut.path);
            }
            self.remote_error = None;
            return;
        }
        if !shortcut.path.exists() {
            self.set_notice(
                "This item is unavailable. Reconnect its drive or remove the favourite.".into(),
                cx,
            );
            return;
        }
        self.unsync_ssh_browser(pane, cx);
        if self.tab(pane).remote.is_some() {
            self.panes[pane].tabs.push(Tab::new(folder.clone()));
            self.panes[pane].active = self.panes[pane].tabs.len() - 1;
        }
        if shortcut.kind == Section::Files {
            self.tab_mut(pane).set_entry_filter(EntryFilter::All);
            use std::os::macos::fs::MetadataExt;
            if shortcut
                .path
                .file_name()
                .is_some_and(|n| n.as_encoded_bytes().starts_with(b"."))
                || fs::metadata(&shortcut.path).is_ok_and(|m| m.st_flags() & libc::UF_HIDDEN != 0)
            {
                self.tab_mut(pane).show_hidden = true;
            }
        }
        if self.tab(pane).current_dir == folder {
            self.load_dir_in(pane, folder, cx);
        } else {
            self.navigate_in(pane, folder, cx);
        }
        if shortcut.kind == Section::Files {
            self.reveal_in_pane(pane, shortcut.path, cx);
        }
    }

    pub(super) fn sidebar_width(&self, window: &Window) -> f32 {
        if prefs().sidebar_collapsed {
            return SIDEBAR_COLLAPSED_W;
        }
        let mut width = self.volume_sidebar_width(window).max(170.);
        if !self.collapsed_sections.contains("PROJECTS") {
            for project in &self.projects.items {
                width = width.max(text_width(window, &project.name, UI_FONT_SIZE) + 54.);
            }
        }
        if !self.collapsed_sections.contains("FAVOURITES") {
            for shortcut in &self.projects.sidebar().shortcuts {
                let section = if shortcut.kind == Section::Folders {
                    "FOLDERS"
                } else {
                    "FILES"
                };
                if !self.collapsed_sections.contains(section) {
                    width = width
                        .max(text_width(window, &path_label(&shortcut.path), UI_FONT_SIZE) + 62.);
                }
            }
        }
        if prefs().groups_enabled {
            for group in &self.groups {
                width = width.max(text_width(window, &group.name, UI_SMALL_FONT_SIZE) + 50.);
                for path in &group.paths {
                    width = width.max(text_width(window, &path_label(path), UI_FONT_SIZE) + 62.);
                }
            }
        }
        fitted_width(width, f32::from(window.viewport_size().width))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sidebar_fits_content_without_growing_over_the_browser() {
        assert_eq!(fitted_width(120., 1000.), 170.);
        assert_eq!(fitted_width(194.2, 1000.), 195.);
        assert_eq!(fitted_width(1000., 1000.), 300.);
        assert_eq!(fitted_width(1000., 2000.), 320.);
    }
}
