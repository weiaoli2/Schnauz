//! Move owned live tabs between a window's project sessions without recreating PTYs.
use super::*;
use schnauz::{capture_panes_layout, ProjectSession};

pub(super) struct Dialog {
    pub source: Option<u64>,
    pub tab: u64,
    pub target: Option<u64>,
    pub activate: bool,
    pub error: Option<String>,
}

#[cfg(feature = "ui-checks")]
pub(super) fn check_moves(b: &mut Schnauz, window: &mut Window, cx: &mut Context<Schnauz>) {
    let old = b.projects.active;
    let source = b.capture_layout();
    let original_projects = b.projects.clone();
    let id = b.tab(0).id;
    let terminal = b.tab(0).terminal.as_ref().unwrap();
    let session_id = terminal.id;
    let view = terminal.view.entity_id();
    let pid = terminal.fixture_pid();
    let path = b.tab(0).current_dir.clone();
    let history = b.tab(0).history.clone();
    let selected = b.tab(0).selection.clone();
    let mut library = b.projects.clone();
    let project = library.items.iter().map(|p| p.id).max().unwrap_or(0) + 1;
    library.items.push(projects::Project {
        id: project,
        name: "Tab Move Fixture".into(),
        layout: source.clone(),
        sidebar: Default::default(),
    });
    b.persist_library(library, cx).unwrap();
    let assert_terminal = |tab: &Tab| {
        assert_eq!(tab.id, id);
        assert_eq!(tab.current_dir, path);
        assert_eq!(tab.history, history);
        assert_eq!(tab.selection, selected);
        let terminal = tab.terminal.as_ref().unwrap();
        assert_eq!(terminal.id, session_id);
        assert_eq!(terminal.view.entity_id(), view);
        assert_eq!(terminal.fixture_pid(), pid);
        assert_eq!(
            unsafe { libc::kill(pid as i32, 0) },
            0,
            "The same owned fixture shell is still alive"
        );
        assert!(
            terminal.ssh.is_some(),
            "Existing SSH identity stays attached without reconnecting"
        );
    };
    b.open_tab_project_move(id, window, cx);
    assert!(
        b.tab_project_move.as_ref().unwrap().activate,
        "Activate Project defaults on"
    );
    assert!(b.native_preview_blocked());
    b.tab_project_move = None;
    let menu = b.sidebar_menu_nodes(SidebarTarget::Tab(0, b.panes[0].active), cx);
    assert!(menu
        .iter()
        .any(|n| matches!(n,MenuNode::Item {label,..} if label=="Move to Project…")));
    let favourites = menu
        .iter()
        .find_map(|n| match n {
            MenuNode::Sub { label, items } if label == "Add to Favourites…" => Some(items),
            _ => None,
        })
        .unwrap();
    let add = favourites
        .iter()
        .find_map(|n| match n {
            MenuNode::Item { label, on, .. } if label == "Tab Move Fixture" => Some(on.clone()),
            _ => None,
        })
        .unwrap();
    add(b, window, cx);
    assert!(b
        .projects
        .sidebar_for(Some(project))
        .unwrap()
        .contains(&projects::Shortcut::local(
            path.clone(),
            projects::Section::Folders
        )));
    assert_eq!(
        b.projects.active, old,
        "Favourites routing does not move the tab or switch projects"
    );
    assert!(b.notice.as_ref().unwrap().contains("Tab Move Fixture"));
    b.project_load_error = Some("fixture save failure".into());
    assert!(b.move_tab_to_project(id, Some(project), true, cx).is_err());
    assert_terminal(b.tab(0));
    assert!(!b.project_sessions.contains_key(&Some(project)));
    b.project_load_error = None;
    b.panes[0].tabs.push(Tab::new(path.clone()));
    b.panes[0].active = b.panes[0].tabs.len() - 1;
    let kept = b.tab(0).id;
    b.move_tab_to_project(id, Some(project), false, cx).unwrap();
    assert_eq!(
        b.tab(0).id,
        kept,
        "Moving a background tab preserves the active source tab"
    );
    assert_eq!(b.projects.active, old);
    assert!(!b.panes.iter().flat_map(|p| &p.tabs).any(|t| t.id == id));
    let parked = b.project_sessions.get(&Some(project)).unwrap();
    assert_terminal(parked.panes[parked.active_pane].active_tab());
    b.switch_project(Some(project), cx).unwrap();
    assert_terminal(b.active_tab());
    b.move_tab_to_project(id, old, true, cx).unwrap();
    assert_eq!(b.projects.active, old);
    assert_terminal(b.active_tab());
    let saved = projects::Library::load(&config_file("projects.json").unwrap()).unwrap();
    assert_eq!(saved.active, old);
    assert_eq!(
        saved, b.projects,
        "Both project layouts are persisted together"
    );
    // Cover removing the last tab of the secondary pane and Personal routing.
    let active = b.panes[b.active_pane].active;
    let terminal_tab = b.panes[b.active_pane].tabs.remove(active);
    b.panes = vec![
        Pane::new(path.clone()),
        Pane {
            tabs: vec![terminal_tab],
            active: 0,
        },
    ];
    b.active_pane = 1;
    b.move_tab_to_project(id, Some(project), true, cx).unwrap();
    assert_terminal(b.active_tab());
    assert_eq!(b.project_sessions.get(&old).unwrap().panes.len(), 1);
    b.move_tab_to_project(id, old, true, cx).unwrap();
    assert_terminal(b.active_tab());
    // Restore fixture metadata only; retain the moved owned shell until the
    // enclosing fixture explicitly closes it. No live user state is involved.
    b.persist_library(original_projects, cx).unwrap();
    b.project_sessions.remove(&Some(project));
    println!("PASS tab-project move with/without activation, Personal/split handling, atomic persistence failure, meaningful Favourites action, and identical live shell PID/terminal/tab/SSH identity; no reconnect or terminal restart");
}

fn session(layout: &projects::Layout) -> ProjectSession {
    let available = layout.available();
    let Some(layout) = available else {
        return ProjectSession {
            panes: vec![Pane::new(home_dir())],
            active_pane: 0,
            split_ratio: 0.5,
        };
    };
    ProjectSession {
        panes: layout
            .panes
            .iter()
            .map(|pane| Pane {
                active: pane.active,
                tabs: pane
                    .tabs
                    .iter()
                    .map(|saved| {
                        let mut tab = Tab::new(saved.path.clone());
                        tab.view = view_mode_from_key(&saved.view).unwrap_or(ViewMode::List);
                        tab.sort_key = sort_key_from_name(&saved.sort).unwrap_or(SortKey::None);
                        tab.sort_asc = saved.ascending;
                        tab.entry_filter = saved.filter;
                        tab.show_hidden = saved.show_hidden;
                        sort_entries(&mut tab.entries, tab.sort_key, tab.sort_asc);
                        tab.recompute_results();
                        tab
                    })
                    .collect(),
            })
            .collect(),
        active_pane: layout.active_pane,
        split_ratio: layout.split_ratio,
    }
}

impl Schnauz {
    fn tab_project_targets(&self) -> Vec<(Option<u64>, String)> {
        let mut targets = vec![(None, "Personal".into())];
        targets.extend(
            self.projects
                .items
                .iter()
                .map(|p| (Some(p.id), p.name.clone())),
        );
        targets.retain(|(id, _)| *id != self.projects.active);
        targets
    }

    pub(super) fn open_tab_project_move(
        &mut self,
        tab: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_sidebar_menu(cx);
        let targets = self.tab_project_targets();
        let Some((target, _)) = targets.first() else {
            self.set_notice("Create another project before moving a tab.".into(), cx);
            return;
        };
        self.tab_project_move = Some(Dialog {
            source: self.projects.active,
            tab,
            target: *target,
            activate: true,
            error: None,
        });
        window.focus(&self.focus);
        cx.notify();
    }

    pub(super) fn move_tab_to_project(
        &mut self,
        tab_id: u64,
        target: Option<u64>,
        activate: bool,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let old = self.projects.active;
        if target == old {
            return Err("Choose another project.".into());
        }
        let stored = match target {
            Some(id) => self
                .projects
                .items
                .iter()
                .find(|p| p.id == id)
                .map(|p| p.layout.clone())
                .ok_or("Project no longer exists.")?,
            None => self.projects.personal_layout.clone().unwrap_or_else(|| {
                capture_panes_layout(&[Pane::new(home_dir())], 0, 0.5, None, None)
            }),
        };
        let (pane, index) = self
            .panes
            .iter()
            .enumerate()
            .find_map(|(p, pane)| {
                pane.tabs
                    .iter()
                    .position(|t| t.id == tab_id)
                    .map(|t| (p, t))
            })
            .ok_or("The tab is no longer in this project.")?;
        let prepared = if self.project_sessions.contains_key(&target) {
            None
        } else {
            Some(session(&stored))
        };
        let dest = self
            .project_sessions
            .get(&target)
            .or(prepared.as_ref())
            .unwrap();
        let mut destination = capture_panes_layout(
            &dest.panes,
            dest.active_pane,
            dest.split_ratio,
            None,
            Some((dest.active_pane, &self.panes[pane].tabs[index])),
        );
        // Unrestorable standalone remote tabs stay live only, following the
        // existing session policy; keep a local cold-start fallback if needed.
        if destination.panes.is_empty() {
            destination = capture_panes_layout(&[Pane::new(home_dir())], 0, 0.5, None, None);
        }
        let mut outgoing = capture_panes_layout(
            &self.panes,
            self.active_pane,
            self.split_ratio,
            Some(tab_id),
            None,
        );
        if outgoing.panes.is_empty() {
            outgoing = capture_panes_layout(&[Pane::new(home_dir())], 0, 0.5, None, None);
        }
        if self.panes.len() == 2 && self.panes[pane].tabs.len() == 1 {
            outgoing.split_ratio = 0.5;
        }
        let mut library = self.projects.clone();
        library.remember_layout(outgoing);
        match target {
            Some(id) => {
                library
                    .items
                    .iter_mut()
                    .find(|p| p.id == id)
                    .unwrap()
                    .layout = destination.clone()
            }
            None => library.personal_layout = Some(destination.clone()),
        }
        if activate {
            library.active = target;
        }
        // Commit both project layouts before touching any live tab. Failure
        // leaves selection, ownership and every terminal exactly where they were.
        self.persist_library(library, cx)?;
        let moved = self.panes[pane].tabs.remove(index);
        let p = &mut self.panes[pane];
        if p.active > index {
            p.active -= 1;
        }
        p.active = p.active.min(p.tabs.len().saturating_sub(1));
        if p.tabs.is_empty() {
            if self.panes.len() == 1 {
                self.panes[0] = Pane::new(home_dir());
            } else {
                self.panes.remove(pane);
                if self.active_pane > pane {
                    self.active_pane -= 1;
                }
                self.active_pane = self.active_pane.min(self.panes.len() - 1);
                self.split_ratio = 0.5;
                self.split_epoch += 1;
            }
        }
        let mut dest = self.project_sessions.remove(&target).or(prepared).unwrap();
        let p = &mut dest.panes[dest.active_pane];
        p.tabs.push(moved);
        p.active = p.tabs.len() - 1;
        self.project_sessions.insert(target, dest);
        self.tab_project_move = None;
        self.clear_file_drag(cx);
        self.close_palette(cx);
        self.rename = None;
        self.pending_click_rename = None;
        self.sidebar_menu = None;
        self.context_menu = None;
        self.terminal_resize = None;
        if activate {
            self.apply_project_switch(old, &destination, cx);
        } else {
            self.dismiss_space_map |= self.space_map.is_some();
        }
        self.saved_session = self.session_string();
        cx.notify();
        Ok(())
    }

    fn confirm_tab_project_move(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = &self.tab_project_move else {
            return;
        };
        let result = if dialog.source != self.projects.active {
            Err("The active project changed. Open Move to Project again.".into())
        } else {
            self.move_tab_to_project(dialog.tab, dialog.target, dialog.activate, cx)
        };
        if let Err(error) = result {
            if let Some(dialog) = &mut self.tab_project_move {
                dialog.error = Some(error);
            }
        }
        window.focus(&self.focus);
        cx.notify();
    }

    pub(super) fn handle_tab_project_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let targets = self.tab_project_targets();
        match event.keystroke.key.as_str() {
            "escape" => self.tab_project_move = None,
            "enter" => self.confirm_tab_project_move(window, cx),
            "space" => {
                if let Some(d) = &mut self.tab_project_move {
                    d.activate = !d.activate;
                }
            }
            "up" | "down" => {
                if let Some(d) = &mut self.tab_project_move {
                    let i = targets
                        .iter()
                        .position(|(id, _)| *id == d.target)
                        .unwrap_or(0);
                    let i = if event.keystroke.key == "up" {
                        i.saturating_sub(1)
                    } else {
                        (i + 1).min(targets.len().saturating_sub(1))
                    };
                    if let Some((id, _)) = targets.get(i) {
                        d.target = *id;
                    }
                }
            }
            _ => {}
        }
        cx.notify();
    }

    pub(super) fn render_tab_project_move(&self, cx: &Context<Self>) -> AnyElement {
        let t = theme();
        let dialog = self.tab_project_move.as_ref().unwrap();
        let mut choices = div()
            .id("move-project-choices")
            .relative()
            .children(ui_checks::probe("move-project-choices"))
            .w(px(382.))
            .max_h(px(240.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        for (id, name) in self.tab_project_targets() {
            choices = choices.child(
                div()
                    .id(("move-project", id.unwrap_or(0)))
                    .w(px(382.))
                    .h(px(30.))
                    .flex_none()
                    .px_2()
                    .flex()
                    .items_center()
                    .rounded_md()
                    .bg(rgb(if id == dialog.target {
                        t.selected
                    } else {
                        t.surface
                    }))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(t.hover)))
                    .child(div().w(px(360.)).truncate().child(name))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(d) = &mut this.tab_project_move {
                            d.target = id;
                            d.error = None;
                        }
                        cx.notify();
                    })),
            );
        }
        modal_backdrop(cx.listener(|this, _, _, cx| {
            this.tab_project_move = None;
            cx.notify();
        }))
        .child(
            div()
                .w(px(420.))
                .p(px(18.))
                .rounded_lg()
                .bg(rgb(t.surface))
                .text_color(rgb(t.text))
                .text_size(px(UI_FONT_SIZE))
                .relative()
                .children(ui_checks::probe("move-project-dialog"))
                .border_1()
                .border_color(rgb(t.border))
                .shadow_lg()
                .flex()
                .flex_col()
                .gap_3()
                .on_mouse_down(MouseButton::Left, |_, _, cx: &mut App| {
                    cx.stop_propagation()
                })
                .child(
                    div()
                        .w(px(382.))
                        .text_size(px(17.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(ui_checks::text(
                            "move-project-heading",
                            "Move to Project…".into(),
                        )),
                )
                .child(choices)
                .child(
                    div()
                        .id("activate-moved-project")
                        .relative()
                        .children(ui_checks::probe("move-project-activate"))
                        .w(px(382.))
                        .h(px(26.))
                        .flex()
                        .items_center()
                        .gap_2()
                        .cursor_pointer()
                        .child(
                            div()
                                .w(px(14.))
                                .h(px(14.))
                                .border_1()
                                .border_color(rgb(t.accent))
                                .rounded_sm()
                                .when(dialog.activate, |d| {
                                    d.child(ui_icon(Icon::Check, 12., rgb(t.accent)))
                                }),
                        )
                        .child("Activate Project")
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(d) = &mut this.tab_project_move {
                                d.activate = !d.activate;
                            }
                            cx.notify();
                        })),
                )
                .children(dialog.error.clone().map(|e| div().w(px(382.)).child(e)))
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            div()
                                .id("move-project-cancel")
                                .px_3()
                                .py_1()
                                .rounded_md()
                                .bg(rgb(t.hover))
                                .cursor_pointer()
                                .child("Cancel")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.tab_project_move = None;
                                    cx.notify();
                                })),
                        )
                        .child(
                            div()
                                .id("move-project-confirm")
                                .px_3()
                                .py_1()
                                .rounded_md()
                                .bg(rgb(t.accent))
                                .text_color(rgb(0xffffff))
                                .cursor_pointer()
                                .child("Move")
                                .on_click(cx.listener(|this, _, w, cx| {
                                    this.confirm_tab_project_move(w, cx)
                                })),
                        ),
                ),
        )
        .into_any_element()
    }
}
