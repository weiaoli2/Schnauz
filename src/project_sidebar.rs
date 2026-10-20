//! Compact project favourites. Local and remote drops create references only.
use super::*;
use projects::{Section, Shortcut};

/// A small exit tolerance avoids native drag jitter at the sidebar boundary.
pub(super) fn drop_hover(
    bounds: Bounds<gpui::Pixels>,
    position: gpui::Point<gpui::Pixels>,
    already_inside: bool,
) -> bool {
    if already_inside {
        bounds.dilate(px(2.)).contains(&position)
    } else {
        bounds.contains(&position)
    }
}

pub(super) struct RemoteDrag {
    pub items: Vec<Shortcut>,
    pub position: gpui::Point<gpui::Pixels>,
}

/// Sidebar gestures carry references and captured ownership, never file URLs.
#[derive(Clone)]
pub(super) enum SidebarOrder {
    Project(u64),
    WebLink { project: Option<u64>, common: bool, id: String },
    Shortcut {
        project: Option<u64>,
        common: bool,
        item: Shortcut,
    },
}
impl SidebarOrder {
    fn accepts(&self, source: &Self) -> bool {
        match (self, source) {
            (Self::Project(a), Self::Project(b)) => a != b,
            (Self::WebLink { project: a, common: ac, id: ai }, Self::WebLink { project: b, common: bc, id: bi }) =>
                ac == bc && (*ac || a == b) && ai != bi,
            (
                Self::Shortcut {
                    project: a,
                    common: ac,
                    item: ai,
                },
                Self::Shortcut {
                    project: b,
                    common: bc,
                    item: bi,
                },
            ) => {
                ac == bc
                    && (*ac || a == b)
                    && ai.kind == bi.kind
                    && !ai.is_home()
                    && !bi.is_home()
                    && !ai.same_place(bi)
            }
            _ => false,
        }
    }
}

/// Fixed row geometry keeps the insertion line from shifting neighbouring items.
#[derive(serde::Serialize, serde::Deserialize)]
struct FolderLabel {
    #[serde(default)]
    alias: String,
    #[serde(default)]
    add_on: String,
}
#[derive(serde::Deserialize)]
struct FolderLabelResponse {
    cancelled: bool,
    label: Option<FolderLabel>,
}

impl Schnauz {
    pub(super) fn sortable_row(
        &self,
        row: AnyElement,
        target: SidebarOrder,
        label: String,
        id: String,
        cx: &Context<Schnauz>,
    ) -> AnyElement {
        let bounds = self.sidebar_order_bounds.clone();
        let paint_id = id.clone();
        let hover_id = id.clone();
        let paint_bounds = bounds.clone();
        let hover_bounds = bounds.clone();
        let hover_target = target.clone();
        let source = target.clone();
        div()
            .id(SharedString::from(id.clone()))
            .relative()
            .h(px(24.))
            .flex_none()
            .child(div().absolute().inset_0().child(row))
            .child(
                gpui::canvas(
                    move |b, _, _| {
                        paint_bounds.borrow_mut().insert(paint_id.clone(), b);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .on_drag(source, move |_, _, _, cx| {
                cx.new(|_| TabDragPreview {
                    label: label.clone(),
                })
            })
            .drag_over::<SidebarOrder>(move |s, source, window, _| {
                if !hover_target.accepts(source) {
                    return s;
                }
                let after = hover_bounds
                    .borrow()
                    .get(&hover_id)
                    .is_some_and(|b| window.mouse_position().y >= b.center().y);
                let s = if after {
                    s.border_b_2()
                } else {
                    s.border_t_2()
                };
                s.border_color(rgb(theme().accent))
            })
            .on_drop(cx.listener(move |this, source: &SidebarOrder, window, cx| {
                cx.stop_propagation();
                if !target.accepts(source) {
                    return;
                }
                let after = bounds
                    .borrow()
                    .get(&id)
                    .is_some_and(|b| window.mouse_position().y >= b.center().y);
                this.reorder_sidebar(source, &target, after, cx);
            }))
            .into_any_element()
    }
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

/// Ignore legacy subsection collapse flags; retain whole-group Projects/Common state.
pub(super) fn restored_sections(saved: Vec<String>) -> HashSet<String> {
    saved
        .into_iter()
        .filter(|title| {
            !matches!(
                title.as_str(),
                "FAVOURITES" | "COMMON_FOLDERS" | "COMMON_FILES" | "FOLDERS" | "FILES"
            )
        })
        .collect()
}

/// Only Common pins Home. All shortcut rows remain visible.
fn shortcuts_in_section(
    shortcuts: &[Shortcut],
    kind: Section,
    pin_home: bool,
) -> impl Iterator<Item = Shortcut> + '_ {
    (pin_home && kind == Section::Folders)
        .then(|| Shortcut::local(home_dir(), Section::Folders))
        .into_iter()
        .chain(
            shortcuts
                .iter()
                .filter(move |s| s.kind == kind && !s.is_home())
                .cloned(),
        )
}

impl Schnauz {
    pub(super) fn edit_favourite_label(
        &mut self, project: Option<u64>, common: bool, original: Shortcut,
        cx: &mut Context<Self>,
    ) {
        if self.weblink_editor_open { return; }
        self.weblink_editor_open = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let input = serde_json::to_vec(&FolderLabel {
                alias: original.alias.clone(), add_on: original.add_on.clone(),
            }).unwrap();
            let (send, receive) = futures::channel::oneshot::channel();
            let worker = std::thread::Builder::new().name("schnauz-folder-label".into()).spawn(move || {
                let result = (|| -> Option<FolderLabelResponse> {
                    use std::io::Write;
                    let helper = std::env::current_exe().ok()?.parent()?.join("weblinkctl");
                    let mut child = Command::new(helper).arg("--folder-label")
                        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
                    if child.stdin.take().is_none_or(|mut stdin| stdin.write_all(&input).is_err()) {
                        let _ = child.kill(); let _ = child.wait(); return None;
                    }
                    let output = child.wait_with_output().ok()?;
                    serde_json::from_slice(&output.stdout).ok()
                })();
                let _ = send.send(result);
            });
            let response = if worker.is_ok() { receive.await.ok().flatten() } else { None };
            let _ = this.update(cx, |this, cx| {
                this.weblink_editor_open = false;
                match response {
                    Some(response) if response.cancelled => {},
                    Some(FolderLabelResponse { label: Some(label), .. }) => {
                        let mut candidate = this.projects.clone();
                        if let Err(error) = projects::edit_shortcut_label(&mut candidate, project, common,
                            &original, &label.alias, &label.add_on)
                            .and_then(|_| this.persist_library(candidate, cx)) {
                            this.set_notice(error, cx);
                        }
                    },
                    _ => this.set_notice("The label editor could not open. Please try again.".into(), cx),
                }
                cx.notify();
            });
        }).detach();
    }

    pub(super) fn tab_folder_drag(&self, pane: usize, index: usize, cx: &Context<Self>) -> TabDrag {
        let tab = &self.panes[pane].tabs[index];
        TabDrag {
            pane,
            tab: index,
            owner: cx.entity_id(),
            project: self.projects.active,
            tab_id: tab.id,
            folder: Shortcut {
                path: tab.current_dir.clone(),
                kind: Section::Folders,
                server: tab.remote.clone(),
                network: tab.network.as_ref().and_then(|n|n.at_path(&tab.current_dir)),
                alias: String::new(),
                add_on: String::new(),
            },
        }
    }

    pub(super) fn track_sidebar_drop(
        &mut self,
        position: gpui::Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) {
        let hovered = !prefs().sidebar_collapsed
            && self
                .sidebar_bounds
                .borrow()
                .is_some_and(|bounds| drop_hover(bounds, position, self.favourites_drop_hover));
        let common = hovered
            && self
                .common_bounds
                .borrow()
                .is_some_and(|bounds| bounds.contains(&position));
        if self.favourites_drop_hover != hovered || self.common_drop_hover != common {
            self.favourites_drop_hover = hovered;
            self.common_drop_hover = common;
            cx.notify();
        }
    }

    pub(super) fn drop_tab_shortcut(
        &mut self,
        drag: &TabDrag,
        common: bool,
        cx: &mut Context<Self>,
    ) {
        self.favourites_drop_hover = false;
        self.common_drop_hover = false;
        TAB_DRAG_LIVE.store(false, Ordering::Relaxed);
        if prefs().sidebar_collapsed {
            cx.notify();
            return;
        }
        // Adding a reference consumes only the frozen folder data. It never
        // takes the Tab/PTY, reads a remote path or reconnects a server.
        self.add_favourites(drag.project, common, vec![drag.folder.clone()], cx);
    }

    pub(super) fn reorder_sidebar(
        &mut self,
        source: &SidebarOrder,
        target: &SidebarOrder,
        after: bool,
        cx: &mut Context<Self>,
    ) {
        if !target.accepts(source) {
            return;
        }
        let mut candidate = self.projects.clone();
        let changed = match (source, target) {
            (SidebarOrder::Project(from), SidebarOrder::Project(to)) => {
                candidate.reorder_project(*from, *to, after)
            }
            (SidebarOrder::WebLink { project, common, id: from }, SidebarOrder::WebLink { id: to, .. }) => {
                let Some(links) = candidate.web_links_mut(*project, *common) else { return; };
                let Some(a) = links.iter().position(|l| &l.id == from) else { return; };
                let Some(b) = links.iter().position(|l| &l.id == to) else { return; };
                projects::reorder(links, a, b, after)
            }
            (
                SidebarOrder::Shortcut {
                    project,
                    common,
                    item,
                },
                SidebarOrder::Shortcut { item: to, .. },
            ) => {
                let shortcuts = if *common {
                    &mut candidate.common_shortcuts
                } else if let Some(sidebar) = candidate.sidebar_for_mut(*project) {
                    &mut sidebar.shortcuts
                } else {
                    return;
                };
                projects::reorder_shortcuts(shortcuts, item, to, after)
            }
            _ => false,
        };
        if changed {
            if let Err(error) = self.persist_library(candidate, cx) {
                self.set_notice(error, cx);
            }
            cx.notify();
        }
    }

    pub(super) fn activate_project_sidebar(&mut self, cx: &mut Context<Self>) {
        let servers = self.projects.sidebar().servers.clone();
        set_active_sftp_servers(servers.clone());
        cx.set_global(SftpServersGlobal(servers, self.projects.active));
    }

    pub(super) fn set_project_shortcut(
        &mut self,
        shortcut: Shortcut,
        add: bool,
        cx: &mut Context<Self>,
    ) {
        let mut candidate = self.projects.clone();
        candidate.sidebar_mut().set_shortcut(shortcut, add);
        match self.persist_library(candidate, cx) {
            Ok(()) if add => self.expand_favourites(),
            Err(error) => self.set_notice(error, cx),
            _ => {}
        }
        cx.notify();
    }

    pub(super) fn set_common_shortcut(
        &mut self,
        shortcut: Shortcut,
        add: bool,
        cx: &mut Context<Self>,
    ) {
        let mut candidate = self.projects.clone();
        candidate.set_common_shortcut(shortcut, add);
        if let Err(error) = self.persist_library(candidate, cx) {
            self.set_notice(error, cx);
        }
        if add {
            self.collapsed_sections.remove("COMMON_FOLDERS");
            self.collapsed_sections.remove("COMMON_FILES");
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
        if let Err(error) = self.persist_library(candidate, cx) {
            self.set_notice(error, cx);
            return;
        }
        let destination = if targets.len() > 1 { "all projects".into() }
            else if let Some(Some(id)) = targets.first() {
                self.projects.items.iter().find(|p|p.id==*id).map(|p|p.name.clone()).unwrap_or_default()
            } else { "Personal".into() };
        self.set_notice(format!("{} project Favourites · {destination}",if add {"Added to"} else {"Removed from"}),cx);
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
        let shared = self.projects.common_contains(&shortcut);
        let common_shortcut = shortcut.clone();
        let mut items = vec![
            mi_check("Common · all projects", shared, move |this, _, cx| {
                this.set_common_shortcut(common_shortcut.clone(), !shared, cx)
            }),
            MenuNode::Sep,
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
            network: tab.network.as_ref().and_then(|n|n.at_path(&path)),
            path,
            kind: if folder {
                Section::Folders
            } else {
                Section::Files
            },
            server: tab.remote.clone(),
            alias: String::new(),
            add_on: String::new(),
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
            .filter(|s| !s.is_home())
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
        match self.persist_library(candidate, cx) {
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
        for title in ["FOLDERS", "FILES"] {
            self.collapsed_sections.remove(title);
        }
    }

    pub(super) fn add_dropped_favourites(
        &mut self,
        project: Option<u64>,
        common: bool,
        paths: Vec<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        self.clear_file_drag(cx);
        let networks:Vec<_>=self.panes.iter().flat_map(|p|p.tabs.iter()).filter_map(|t|t.network.clone()).collect();
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
                            {let mut shortcut=Shortcut::local(path,kind); shortcut.network=networks.iter().find_map(|n|n.at_path(&shortcut.path));Some(shortcut)}
                        })
                        .collect::<Vec<_>>()
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.add_favourites(project, common, shortcuts, cx)
            });
        })
        .detach();
    }

    fn add_favourites(
        &mut self,
        project: Option<u64>,
        common: bool,
        shortcuts: Vec<Shortcut>,
        cx: &mut Context<Self>,
    ) {
        if common && self.collapsed_sections.contains("COMMON") {
            self.toggle_section("COMMON".into(), cx);
        }
        let mut candidate = self.projects.clone();
        let before;
        let after;
        if common {
            before = candidate.common_shortcuts.len();
            for shortcut in shortcuts {
                candidate.set_common_shortcut(shortcut, true);
            }
            after = candidate.common_shortcuts.len();
        } else {
            let Some(sidebar) = candidate.sidebar_for_mut(project) else {
                return;
            };
            before = sidebar.shortcuts.len();
            for shortcut in shortcuts {
                sidebar.set_shortcut(shortcut, true);
            }
            after = sidebar.shortcuts.len();
        }
        let added = after - before;
        if candidate == self.projects {
            self.set_notice("No new favourites to add.".into(), cx);
        } else if let Err(error) = self.persist_library(candidate, cx) {
            self.set_notice(error, cx);
        } else {
            if common {
                self.collapsed_sections.remove("COMMON_FOLDERS");
                self.collapsed_sections.remove("COMMON_FILES");
            } else if self.projects.active == project {
                self.expand_favourites();
            }
            self.set_notice(
                if added == 0 {
                    if common {
                        "Common updated"
                    } else {
                        "Favourites updated"
                    }
                    .into()
                } else {
                    format!(
                        "Added {added} item{} to {}",
                        if added == 1 { "" } else { "s" },
                        if common { "Common" } else { "Favourites" }
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
            let common = self
                .common_bounds
                .borrow()
                .is_some_and(|bounds| bounds.contains(&position));
            self.add_favourites(self.projects.active, common, drag.items, cx);
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
        self.shortcut_section(items, key, false, cx);
    }
    pub(super) fn common_section(&self, key: &mut usize, cx: &Context<Self>) -> AnyElement {
        let mut rows = Vec::new();
        self.shortcut_section(&mut rows, key, true, cx);
        div()
            .id("common-shortcuts")
            .relative()
            .flex_none()
            .flex()
            .flex_col()
            .children(rows)
            .child(
                gpui::canvas(
                    {
                        let store = self.common_bounds.clone();
                        move |bounds, _, _| {
                            *store.borrow_mut() = Some(bounds);
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .on_drop(cx.listener(|this, drag: &TabDrag, _, cx| {
                this.drop_tab_shortcut(drag, true, cx);
                cx.stop_propagation();
            }))
            .on_drop(cx.listener(|this, drag: &ExternalPaths, _, cx| {
                this.favourites_drop_hover = false;
                this.common_drop_hover = false;
                this.add_dropped_favourites(this.projects.active, true, drag.paths().to_vec(), cx);
                cx.stop_propagation();
            }))
            .into_any_element()
    }
    fn shortcut_section(
        &self,
        items: &mut Vec<AnyElement>,
        key: &mut usize,
        common: bool,
        cx: &Context<Self>,
    ) {
        let shortcuts = if common {
            &self.projects.common_shortcuts
        } else {
            &self.projects.sidebar().shortcuts
        };
        items.push(
            div()
                .id(if common {
                    "common-heading"
                } else {
                    "favourites-heading"
                })
                .relative()
                .when(common, |d| {
                    d.cursor_pointer()
                        .children(ui_checks::probe("common-heading"))
                        .tooltip(tip(if self.collapsed_sections.contains("COMMON") {
                            "Click to expand Common"
                        } else {
                            "Click to collapse Common"
                        }))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.toggle_section("COMMON".into(), cx);
                            cx.stop_propagation();
                        }))
                })
                .px_3()
                .pt_2()
                .pb_0()
                .text_size(px(UI_SMALL_FONT_SIZE))
                .text_color(rgb(theme().text_dim))
                .child(if common { "COMMON" } else { "FAVOURITES" })
                .on_mouse_down(MouseButton::Right, cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.web_link_section_menu(common, event.position, cx);
                }))
                .into_any_element(),
        );
        if common && self.collapsed_sections.contains("COMMON") {
            return;
        }
        for (kind, label) in [(Section::Folders, "Folders"), (Section::Files, "Files")] {
            if !(common && kind == Section::Folders)
                && !shortcuts.iter().any(|s| s.kind == kind && !s.is_home())
            {
                continue;
            }
            let key_name = match (common, kind) {
                (true, Section::Folders) => "COMMON_FOLDERS",
                (true, Section::Files) => "COMMON_FILES",
                (false, Section::Folders) => "FOLDERS",
                (false, Section::Files) => "FILES",
            };
            items.push(
                div()
                    .id(key_name)
                    .px_4()
                    .pt_1()
                    .pb_0p5()
                    .text_size(px(UI_SMALL_FONT_SIZE))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme().text))
                    .child(label)
                    .into_any_element(),
            );
            for shortcut in shortcuts_in_section(shortcuts, kind, common) {
                self.project_shortcut_row(items, key, &shortcut, common, cx);
            }
        }
        self.web_links_section(items, key, common, cx);
    }

    fn favourite_icon(&self, shortcut: &Shortcut, cx: &Context<Self>) -> AnyElement {
        let network = shortcut.network.is_some() || shortcut.server.is_some() || self.network_volume_path(&shortcut.path);
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
        common: bool,
        cx: &Context<Self>,
    ) {
        *key += 1;
        let nav = shortcut.clone();
        let target = if common {
            SidebarTarget::Common(shortcut.clone())
        } else {
            SidebarTarget::Favourite(shortcut.clone())
        };
        let same_server = match (&self.active_tab().remote, &shortcut.server) {
            (None, None) => true,
            (Some(a), Some(b)) => projects::same_server(a, b),
            _ => false,
        };
        let network_matches=match (&self.active_tab().network,&shortcut.network) {
            (Some(a),Some(b))=>a.url==b.url&&a.username==b.username&&a.guest==b.guest,
            (None,None)=>true,_=>false,
        };
        let active = network_matches && same_server
            && (self.active_tab().current_dir == shortcut.path
                || self.active_tab().selection.contains(&shortcut.path));
        let tooltip = shortcut.network.as_ref().map(|n|format!("{}\n{}",n.url,n.relative.display())).unwrap_or_else(||shortcut.server.as_ref().map_or_else(
            || display_path(&shortcut.path),
            |s| format!("{}\n{}", s.display(), shortcut.path.display()),
        ));
        let row = nav_item(
                shortcut.display_name(),
                tooltip,
                self.favourite_icon(shortcut, cx),
                shortcut
                    .is_home()
                    .then(|| sidebar_badge("Home".into(), 0x3478c5).into_any_element()),
                *key,
                active,
                cx.listener(move |this, event: &ClickEvent, window, cx| {
                    window.focus(&this.focus);
                    if nav.kind == Section::Files && event.click_count() >= 2 && !event.modifiers().modified() {
                        if nav.network.is_some() {
                            this.open_network_favourite(nav.clone(),true,false,cx);
                        } else if nav.server.is_none() {
                            this.open_local_file(nav.path.clone(), cx);
                        } else {
                            this.open_favourite(nav.clone(), cx);
                            this.open_path(this.active_pane, nav.path.clone(), false, cx);
                        }
                    } else {
                        this.open_favourite(nav.clone(), cx);
                    }
                }),
                cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                    this.open_sidebar_menu(
                        f32::from(ev.position.x),
                        f32::from(ev.position.y),
                        target.clone(),
                        ev.modifiers.platform,
                        cx,
                    );
                    cx.stop_propagation();
                }),
            )
            .into_any_element();
        items.push(if shortcut.is_home() { row } else {
            self.sortable_row(row, SidebarOrder::Shortcut { project: self.projects.active, common, item: shortcut.clone() }, shortcut.display_name(), format!("sidebar-order-{key}"), cx)
        });
    }

    pub(super) fn favourite_server(&self, server: SftpServer) -> SftpServer {
        std::iter::once(self.active_tab())
            .chain(self.panes.iter().flat_map(|p| p.tabs.iter()))
            .filter_map(|t| t.terminal.as_ref()?.ssh.as_ref().map(|link| &link.server))
            .find(|s| projects::same_server(s, &server))
            .cloned()
            .or_else(|| server.control_path.is_some().then(|| server.clone()))
            .or_else(|| {
                self.projects
                    .sidebar()
                    .servers
                    .iter()
                    .find(|s| projects::same_server(s, &server))
                    .cloned()
            })
            .unwrap_or(server)
    }

    pub(super) fn open_favourite(&mut self, shortcut: Shortcut, cx: &mut Context<Self>) {
        if shortcut.network.is_some() {self.open_network_favourite(shortcut,false,false,cx);return;}
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
            let live = self.favourite_server(server);
            // Navigation changes only the browser. Keep the current terminal
            // visible and its PTY untouched, including foreground SSH/CLI work.
            let reuse = self.tab(pane).terminal.is_some()
                || self
                    .tab(pane)
                    .remote
                    .as_ref()
                    .is_some_and(|s| projects::same_server(s, &live));
            if reuse {
                ssh_sync::browse_remote(self.tab_mut(pane), live, &folder);
                if self.tab(pane).current_dir == folder {
                    self.load_dir_in(pane, folder, cx);
                } else {
                    self.navigate_in(pane, folder, cx);
                }
            } else {
                let tab = Tab::at_location(folder, Some(live));
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
            let tab = self.tab_mut(pane);
            tab.remote = None;
            tab.history = vec![folder.clone()];
            tab.hist_pos = 0;
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
                width = width.max(text_width(window, &project.name, UI_FONT_SIZE) + 67.);
            }
        }
        width = width.max(
            text_width(window, &path_label(&home_dir()), UI_FONT_SIZE)
                + text_width(window, "Home", 10.)
                + 65.,
        );
        for shortcuts in std::iter::once(&self.projects.sidebar().shortcuts)
            .chain((!self.collapsed_sections.contains("COMMON")).then_some(&self.projects.common_shortcuts)) {
            for shortcut in shortcuts.iter().filter(|s| !s.is_home()) {
                width =
                    width.max(text_width(window, &shortcut.display_name(), UI_FONT_SIZE) + 62.);
            }
        }
        for common in [false, true] {
            if common && self.collapsed_sections.contains("COMMON") { continue; }
            if let Some(links) = self.projects.web_links(self.projects.active, common) {
                for link in links { width = width.max(text_width(window, &link.name, UI_FONT_SIZE) + 62.); }
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
    fn ordering_rejects_different_owners_sections_and_pinned_home() {
        let a = Shortcut::local("/a".into(), Section::Folders);
        let b = Shortcut::local("/b".into(), Section::Folders);
        let source = SidebarOrder::Shortcut { project: Some(1), common: false, item: a.clone() };
        let target = |project, common, item| SidebarOrder::Shortcut { project, common, item };
        assert!(target(Some(1), false, b.clone()).accepts(&source));
        assert!(!target(Some(2), false, b.clone()).accepts(&source));
        assert!(!target(Some(1), true, b.clone()).accepts(&source));
        assert!(!target(Some(1), false, Shortcut::local("/f".into(), Section::Files)).accepts(&source));
        assert!(!target(Some(1), false, Shortcut::local(home_dir(), Section::Folders)).accepts(&source));
        assert!(!SidebarOrder::Project(1).accepts(&source));
        assert!(target(Some(2), true, b).accepts(&target(Some(1), true, a)));
    }

    #[test]
    fn common_pins_home_while_project_favourites_do_not_duplicate_it() {
        let home = Shortcut::local(home_dir(), Section::Folders);
        let mut library = projects::Library::default();
        library.set_common_shortcut(
            Shortcut::local(PathBuf::from("/shared"), Section::Folders),
            true,
        );
        library.set_common_shortcut(
            Shortcut::local(PathBuf::from("/shared.txt"), Section::Files),
            true,
        );
        library.set_common_shortcut(home.clone(), true);
        assert_eq!(library.common_shortcuts.len(), 4);
        let rows: Vec<_> =
            shortcuts_in_section(&library.common_shortcuts, Section::Folders, true).collect();
        assert_eq!(rows[0], home);
        assert_eq!(rows.iter().filter(|s| s.is_home()).count(), 1);
        assert!(rows.iter().any(|s| s.path == Path::new("/shared")));
        assert!(
            shortcuts_in_section(&library.sidebar().shortcuts, Section::Folders, false)
                .next()
                .is_none()
        );
        let saved = [
            "FAVOURITES",
            "COMMON_FOLDERS",
            "COMMON_FILES",
            "FOLDERS",
            "FILES",
            "PROJECTS",
        ];
        let sections = restored_sections(saved.iter().map(|s| (*s).into()).collect());
        assert_eq!(sections, HashSet::from(["PROJECTS".into()]));
        let restored: projects::Library =
            serde_json::from_str(&serde_json::to_string(&library).unwrap()).unwrap();
        assert_eq!(restored.common_shortcuts, library.common_shortcuts);
        assert!(restored.common_contains(&home));
        assert!(!restored_sections(vec!["FAVOURITES".into()]).contains("FAVOURITES"));
    }

    #[test]
    fn native_drag_hover_is_stable_at_sidebar_edges_and_clears_outside() {
        let bounds = Bounds::new(point(px(0.), px(56.)), size(px(180.), px(500.)));
        assert!(drop_hover(bounds, point(px(179.), px(100.)), false));
        assert!(drop_hover(bounds, point(px(180.5), px(100.)), true));
        assert!(!drop_hover(bounds, point(px(180.5), px(100.)), false));
        assert!(!drop_hover(bounds, point(px(190.), px(100.)), true));
        assert!(!drop_hover(bounds, point(px(100.), px(600.)), true));
    }

    #[test]
    fn sidebar_fits_content_without_growing_over_the_browser() {
        assert_eq!(fitted_width(120., 1000.), 170.);
        assert_eq!(fitted_width(194.2, 1000.), 195.);
        assert_eq!(fitted_width(1000., 1000.), 300.);
        assert_eq!(fitted_width(1000., 2000.), 320.);
    }
}

#[cfg(feature = "ui-checks")]
pub(super) fn check_order(
    window: gpui::WindowHandle<ui_checks::Fixture>,
    cx: &mut App,
    done: impl FnOnce(&mut App) + 'static,
) {
    fn paint(window: gpui::WindowHandle<ui_checks::Fixture>, cx: &mut App) {
        for _ in 0..3 {
            cx.update_window(*window, |_, w, cx| {
                w.refresh();
                w.draw(cx).clear();
            })
            .unwrap();
        }
    }
    let file = config_file("sidebar-double-click.txt").unwrap();
    fs::write(&file, "sidebar fixture").unwrap();
    let fixture_file = file.clone();
    let original = window
        .update(cx, |f, _, cx| {
            f.browser.update(cx, |b, cx| {
                let original = b.projects.clone();
                b.projects.common_shortcuts = vec![
                    Shortcut::local("/sidebar-order-a".into(), Section::Folders),
                    Shortcut::local("/sidebar-order-b".into(), Section::Folders),
                    Shortcut::local(fixture_file, Section::Files),
                ];
                b.collapsed_sections.remove("PROJECTS");
                b.persist_library(b.projects.clone(), cx).unwrap();
                cx.notify();
                (
                    original,
                    b.active_tab().id,
                    b.active_tab().current_dir.clone(),
                )
            })
        })
        .unwrap();
    paint(window, cx);
    let bounds = window
        .update(cx, |f, _, cx| {
            f.browser.read(cx).sidebar_order_bounds.borrow().clone()
        })
        .unwrap();
    let a = bounds
        .get("sidebar-order-2")
        .expect("first sortable shortcut")
        .center();
    let b = *bounds
        .get("sidebar-order-3")
        .expect("second sortable shortcut");
    let destination = point(b.center().x, b.bottom_right().y - px(3.));
    let file_position = bounds
        .get("sidebar-order-4")
        .expect("sidebar file")
        .center();
    let (ptr, height) = cx
        .update_window(*window, |_, w, _| {
            (
                ns_view_ptr(w).unwrap() as usize,
                f32::from(w.viewport_size().height),
            )
        })
        .unwrap();
    cx.spawn(async move |cx| {
        use objc2_app_kit::{NSEvent, NSEventType, NSEventModifierFlags, NSView};
        cx.background_executor().timer(Duration::from_millis(10)).await;
        let view = unsafe { &*(ptr as *const NSView) };
        let native = view.window().unwrap();
        let number: isize = unsafe { objc2::msg_send![&*native, windowNumber] };
        for (kind, position) in [
            (NSEventType::MouseMoved, a),
            (NSEventType::LeftMouseDown, a),
            (NSEventType::LeftMouseDragged, a + point(px(0.), px(8.))),
            (NSEventType::LeftMouseDragged, destination),
            (NSEventType::LeftMouseUp, destination),
        ] {
            let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(kind, objc2_foundation::NSPoint::new(f32::from(position.x) as f64, (height - f32::from(position.y)) as f64), NSEventModifierFlags::empty(), 0., number, None, 0, 1, 0.).unwrap();
            {
                match kind {
                    NSEventType::LeftMouseDown => view.mouseDown(&event),
                    NSEventType::LeftMouseDragged => view.mouseDragged(&event),
                    NSEventType::LeftMouseUp => view.mouseUp(&event),
                    _ => view.mouseMoved(&event),
                }
            }
            cx.update(|cx| paint(window, cx)).unwrap();
            cx.background_executor().timer(Duration::from_millis(15)).await;
        }
        cx.update(|cx| window.update(cx, |f, _, cx| {
            assert_eq!(f.browser.read(cx).active_tab().current_dir, original.2, "Sorting must not navigate");
        }).unwrap()).unwrap();
        // Use the real sidebar callback, intercepting only external app launch.
        file_apps::OPEN_CAPTURE.with(|capture| *capture.borrow_mut() = Some(Vec::new()));
        for count in [1, 2] {
            for kind in [NSEventType::LeftMouseDown, NSEventType::LeftMouseUp] {
                let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(kind, objc2_foundation::NSPoint::new(f32::from(file_position.x) as f64, (height - f32::from(file_position.y)) as f64), NSEventModifierFlags::empty(), 0., number, None, 0, count, 0.).unwrap();
                if kind == NSEventType::LeftMouseDown { view.mouseDown(&event); } else { view.mouseUp(&event); }
                cx.update(|cx| paint(window, cx)).unwrap();
                cx.background_executor().timer(Duration::from_millis(15)).await;
            }
            file_apps::OPEN_CAPTURE.with(|capture| {
                let capture = capture.borrow();
                if count == 1 { assert!(capture.as_ref().unwrap().is_empty(), "Single click only reveals the file"); }
                else { assert_eq!(capture.as_ref().unwrap(), &vec![file.clone()], "Double click opens the captured file once"); }
            });
        }
        file_apps::OPEN_CAPTURE.with(|capture| *capture.borrow_mut() = None);
        cx.update(|cx| {
            window.update(cx, |f, _, cx| f.browser.update(cx, |b, cx| {
                assert_eq!(b.projects.common_shortcuts[0].path, Path::new("/sidebar-order-b"));
                assert_eq!(b.projects.common_shortcuts[1].path, Path::new("/sidebar-order-a"));
                assert_eq!(b.active_tab().id, original.1);

                let saved = projects::Library::load(&config_file("projects.json").unwrap()).unwrap();
                assert_eq!(saved.common_shortcuts, b.projects.common_shortcuts);
                let stored_only = b.projects.create("Sidebar stored only", b.capture_layout()).unwrap();
                assert!(!b.project_is_open(Some(stored_only), cx), "Saved layout alone is not runtime activity");
                assert!(b.project_is_open(b.projects.active, cx));
                b.project_sessions.insert(Some(stored_only), schnauz::ProjectSession { panes: vec![Pane { tabs: vec![Tab::new(std::env::temp_dir())], active: 0 }], active_pane: 0, split_ratio: 0.5 });
                assert!(b.project_is_open(Some(stored_only), cx), "Parked tabs remain active");
                b.project_sessions.remove(&Some(stored_only));
                assert!(!b.project_is_open(Some(stored_only), cx));
                b.persist_library(original.0, cx).unwrap();
                cx.notify();
            })).unwrap();
            paint(window, cx);
            println!("PASS native sidebar pointer drag, lower-half insertion, saved order, unchanged tab/path during drag and file single/double-click routing without launching apps");
            done(cx);
        }).unwrap();
    }).detach();
}
