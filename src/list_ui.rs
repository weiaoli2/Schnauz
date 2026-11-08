//! Configurable list columns and folder presentation controls.
use super::*;
use folder_views::Scope;

#[derive(Clone)]
pub(super) struct ViewChange { scope: Scope, root: PathBuf, reset: bool }
impl gpui::Global for ViewChange {}

#[derive(Clone, Copy)]
struct ColumnDrag {
    pane: usize,
    column: Column,
}

fn style_of(tab: &Tab) -> folder_views::FolderView {
    folder_views::FolderView {
        view: tab.view,
        list: tab.list_layout.clone(),
        preview_width: tab.preview_width,
        sort: sort_key_name(tab.sort_key).into(),
        ascending: tab.sort_asc,
    }
}
pub(super) fn apply_style(tab: &mut Tab, style: &folder_views::FolderView) {
    tab.view = style.view;
    tab.list_layout = style.list.clone();
    tab.preview_width = style.preview_width;
    let sort = sort_key_from_name(&style.sort).unwrap_or(SortKey::None);
    let resort = tab.sort_key != sort || tab.sort_asc != style.ascending;
    tab.sort_key = sort;
    tab.sort_asc = style.ascending;
    if resort {
        sort_entries(&mut tab.entries, tab.sort_key, tab.sort_asc);
        tab.recompute_results();
    }
}
impl Schnauz {
    pub(super) fn apply_folder_view(&mut self, pane: usize, only_saved: bool) {
        let tab = self.tab(pane);
        if tab.remote.is_some() {
            return;
        }
        let style = self
            .folder_views
            .borrow()
            .resolve_project(self.projects.active, &tab.current_dir)
            .cloned()
            .or_else(|| (!only_saved).then(folder_views::FolderView::default));
        if let Some(style) = style {
            apply_style(self.tab_mut(pane), &style);
        }
    }
    pub(super) fn apply_all_folder_views(&mut self) {
        let views = self.folder_views.borrow();
        for (project, panes) in std::iter::once((self.projects.active, &mut self.panes))
            .chain(self.project_sessions.iter_mut().map(|(id, session)| (*id, &mut session.panes))) {
            for pane in panes { for tab in &mut pane.tabs {
                if tab.remote.is_none() {
                    if let Some(style) = views.resolve_project(project, &tab.current_dir) { apply_style(tab, style); }
                }
            } }
        }
    }
    pub(super) fn observe_view_change(&mut self, cx: &mut Context<Self>) {
        let change = cx.global::<ViewChange>().clone();
        self.apply_all_folder_views();
        if change.reset {
            let views = self.folder_views.borrow();
            for (project, panes) in std::iter::once((self.projects.active, &mut self.panes))
                .chain(self.project_sessions.iter_mut().map(|(id, session)| (*id, &mut session.panes))) {
                if change.scope != Scope::AllProjects && change.scope != Scope::Project(project) { continue; }
                for pane in panes { for tab in &mut pane.tabs {
                    if tab.remote.is_none() && tab.current_dir == change.root {
                        apply_style(tab, &views.resolve_project(project, &tab.current_dir).cloned().unwrap_or_default());
                    }
                } }
            }
        }
        for pane in 0..self.panes.len() { self.scroll_selected_in_pane(pane); }
        cx.notify();
    }
    fn publish_view_change(&mut self, scope: Scope, root: PathBuf, reset: bool, cx: &mut Context<Self>) {
        cx.set_global(ViewChange { scope, root, reset });
        self.observe_view_change(cx);
        self.schedule_view_save(cx);
    }
    pub(super) fn schedule_view_save(&mut self, cx: &mut Context<Self>) {
        let error = self.folder_views.borrow().error.clone();
        if let Some(error) = error {
            self.set_notice(
                format!("Couldn't read saved views: {error}. Existing settings preserved."),
                cx,
            );
            return;
        }
        let Some(job) = self.folder_views.borrow().write_job() else {
            return;
        };
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(250))
                .await;
            let result = cx.background_spawn(async move { job.run() }).await;
            if let Err(error) = result {
                let _ = this.update(cx, |this, cx| {
                    this.set_notice(format!("Couldn't save folder view: {error}"), cx)
                });
            }
        })
        .detach();
    }
    pub(super) fn remember_folder_view(&mut self, pane: usize, cx: &mut Context<Self>) {
        let tab = self.tab(pane);
        if tab.remote.is_some() {
            return;
        }
        let (path, style) = (tab.current_dir.clone(), style_of(tab));
        let scope = Scope::Project(self.projects.active);
        self.folder_views.borrow_mut().remember_scoped(scope, path.clone(), style, false);
        self.publish_view_change(scope, path, false, cx);
    }
    pub(super) fn view_rule_nodes(&self, pane: usize, root: PathBuf) -> Vec<MenuNode> {
        let project = self.projects.active;
        let name = project.and_then(|id| self.projects.items.iter().find(|p| p.id == id).map(|p| p.name.as_str())).unwrap_or("Personal");
        let style = style_of(self.tab(pane));
        [
            ("Save Current View & Sorting for This Folder", false, false),
            ("Apply Current View & Sorting to This Folder & Subfolders", true, false),
            ("Use inherited / default view", false, true),
        ].into_iter().map(|(label, descendants, reset)| {
            let items = [(Scope::Project(project), format!("This project ({name})")), (Scope::AllProjects, "All projects".into())]
                .into_iter().map(|(scope, label)| {
                    let root = root.clone();
                    let style = style.clone();
                    mi(label, move |this, _, cx| {
                        let error = this.folder_views.borrow().error.clone();
                        if let Some(error) = error {
                            this.set_notice(format!("Couldn't read saved views: {error}. Existing settings preserved."), cx);
                            return;
                        }
                        if let Scope::Project(Some(id)) = scope {
                            if !this.projects.items.iter().any(|p| p.id == id) { return; }
                        }
                        if reset { this.folder_views.borrow_mut().reset_scoped(scope, &root); }
                        else { this.folder_views.borrow_mut().remember_scoped(scope, root.clone(), style.clone(), descendants); }
                        this.publish_view_change(scope, root.clone(), reset, cx);
                        this.close_context_menu(cx);
                        this.close_sidebar_menu(cx);
                    })
                }).collect();
            MenuNode::Sub { label: label.into(), items }
        }).collect()
    }
    pub(super) fn scroll_selected_in_pane(&self, pane: usize) {
        let tab = self.tab(pane);
        let selected = tab
            .anchor
            .as_ref()
            .filter(|p| tab.selection.contains(*p))
            .or_else(|| tab.selection.iter().next());
        let Some(selected) = selected else { return };
        let Some(index) = tab
            .visible_entries()
            .position(|e| tab.current_dir.join(&e.name) == *selected)
        else {
            return;
        };
        let row = match tab.view {
            ViewMode::Icons => index / self.icon_cols(pane),
            ViewMode::List => {
                index
                    + usize::from(
                        !tab.is_filtered()
                            && prefs().show_parent
                            && tab.current_dir.parent().is_some(),
                    )
            }
        };
        tab.scroll_handle
            .scroll_to_item(row, ScrollStrategy::Center);
    }
    pub(super) fn set_view(&mut self, pane: usize, mode: ViewMode, cx: &mut Context<Self>) {
        self.active_pane = pane;
        if self.tab(pane).view != mode {
            let tab = self.tab_mut(pane);
            tab.view = mode;
            tab.scroll_handle = UniformListScrollHandle::new();
            tab.h_scroll = ScrollHandle::new();
        }
        self.remember_folder_view(pane, cx);
        self.scroll_selected_in_pane(pane);
        self.prewarm_icons(cx);
        cx.notify();
    }
    pub(super) fn toggle_hidden_preserving_selection(
        &mut self,
        pane: usize,
        cx: &mut Context<Self>,
    ) {
        let searching = self.search_in_pane(pane);
        let tab = self.tab_mut(pane);
        tab.show_hidden = !tab.show_hidden;
        tab.recompute_results();
        if !searching {
            tab.retain_visible_selection();
        }
        if self.search_in_pane(pane) {
            self.refresh_file_search_preserving_selection(cx);
        }
        self.scroll_selected_in_pane(pane);
        self.save_session(cx);
        self.prewarm_icons(cx);
        cx.notify();
    }
    fn toggle_column(&mut self, pane: usize, column: Column, cx: &mut Context<Self>) {
        self.tab_mut(pane).list_layout.toggle(column);
        self.remember_folder_view(pane, cx);
        self.close_context_menu(cx);
        cx.notify();
    }
    fn move_column(
        &mut self,
        drag: ColumnDrag,
        pane: usize,
        before: Option<Column>,
        cx: &mut Context<Self>,
    ) {
        if drag.pane != pane {
            return;
        }
        self.tab_mut(pane)
            .list_layout
            .move_before(drag.column, before);
        self.remember_folder_view(pane, cx);
        cx.notify();
    }
    pub(super) fn begin_resize(&mut self, pane: usize, col: Column, x: f32) {
        self.resize = Some(Resize {
            pane,
            col,
            start_x: x,
            start_w: self.tab(pane).list_layout.width(col),
        });
    }
    pub(super) fn update_resize(&mut self, x: f32, cx: &mut Context<Self>) {
        if let Some(r) = self.resize.filter(|r| r.pane < self.panes.len()) {
            self.tab_mut(r.pane)
                .list_layout
                .set_width(r.col, r.start_w + x - r.start_x);
            cx.notify();
        }
    }
    pub(super) fn end_resize(&mut self, cx: &mut Context<Self>) {
        if let Some(r) = self.resize.take().filter(|r| r.pane < self.panes.len()) {
            self.remember_folder_view(r.pane, cx);
        }
    }
    fn column_nodes(&self, pane: usize) -> Vec<MenuNode> {
        Column::ALL
            .into_iter()
            .map(|column| {
                if column == Column::Name {
                    MenuNode::Disabled("Name (always shown)".into())
                } else {
                    mi_check(
                        column.label(),
                        self.tab(pane).list_layout.contains(column),
                        move |this, _, cx| this.toggle_column(pane, column, cx),
                    )
                }
            })
            .collect()
    }
    pub(super) fn open_column_menu(&mut self, pane: usize, x: f32, y: f32, cx: &mut Context<Self>) {
        let mut nodes = self.column_nodes(pane);
        if self.tab(pane).remote.is_none() {
            nodes.push(MenuNode::Sep);
            let root = self.tab(pane).current_dir.clone();
            nodes.extend(self.view_rule_nodes(pane, root));
        }
        self.context_menu = Some(ContextMenu {
            x,
            y,
            target: None,
            nodes,
        });
        cx.notify();
    }
    pub(super) fn open_view_menu(&mut self, pane: usize, x: f32, y: f32, cx: &mut Context<Self>) {
        self.active_pane = pane;
        let mut nodes: Vec<MenuNode> = [(ViewMode::List, "List"), (ViewMode::Icons, "Icons")]
            .into_iter()
            .map(|(mode, label)| {
                mi_check(label, self.tab(pane).view == mode, move |this, _, cx| {
                    this.close_context_menu(cx);
                    this.set_view(pane, mode, cx);
                })
            })
            .collect();
        nodes.push(MenuNode::Sep);
        nodes.push(MenuNode::Sub {
            label: "List columns".into(),
            items: self.column_nodes(pane),
        });
        if self.tab(pane).remote.is_none() {
            nodes.push(MenuNode::Sep);
            let root = self.tab(pane).current_dir.clone();
            nodes.extend(self.view_rule_nodes(pane, root));
        }
        self.context_menu = Some(ContextMenu {
            x,
            y,
            target: None,
            nodes,
        });
        cx.notify();
    }
    pub(super) fn column_header(&self, pane: usize, cx: &Context<Self>) -> AnyElement {
        let tab = self.tab(pane);
        div()
            .flex()
            .items_center()
            .flex_none()
            .px_3()
            .h(px(28.))
            .text_size(px(UI_SMALL_FONT_SIZE))
            .text_color(rgb(theme().text_muted))
            .bg(rgb(theme().sidebar))
            .border_b_1()
            .border_color(rgb(theme().border))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                    this.open_column_menu(
                        pane,
                        f32::from(ev.position.x),
                        f32::from(ev.position.y),
                        cx,
                    );
                    cx.stop_propagation();
                }),
            )
            .children(
                tab.list_layout
                    .columns
                    .iter()
                    .map(|c| header_cell(pane, c.column, c.width, tab.sort_key, tab.sort_asc, cx)),
            )
            .child(
                div()
                    .id(("column-drop-end", pane))
                    .flex_1()
                    .min_w(px(20.))
                    .h_full()
                    .drag_over::<ColumnDrag>(|s, _, _, _| s.bg(rgb(theme().selected)))
                    .on_drop(cx.listener(move |this, drag: &ColumnDrag, _, cx| {
                        this.move_column(*drag, pane, None, cx)
                    })),
            )
            .into_any_element()
    }
}

fn column_sort(column: Column) -> Option<SortKey> {
    Some(match column {
        Column::Name => SortKey::Name,
        Column::Kind => SortKey::Kind,
        Column::Modified => SortKey::Modified,
        Column::Created => SortKey::Created,
        Column::Size => SortKey::Size,
        Column::Extension => SortKey::Extension,
        Column::Permissions => SortKey::Permissions,
        Column::Location => return None,
    })
}
fn header_cell(
    pane: usize,
    column: Column,
    width: f32,
    sort: SortKey,
    ascending: bool,
    cx: &Context<Schnauz>,
) -> AnyElement {
    let key = column_sort(column);
    let active = key == Some(sort);
    let label = format!(
        "{}{}",
        column.label(),
        if active {
            if ascending {
                "  ↑"
            } else {
                "  ↓"
            }
        } else {
            ""
        }
    );
    let drag = ColumnDrag { pane, column };
    div()
        .id(("column-header", pane * 100 + column.key()))
        .flex_none()
        .w(px(width))
        .h_full()
        .flex()
        .items_center()
        .drag_over::<ColumnDrag>(|s, _, _, _| {
            s.bg(rgb(theme().selected))
                .border_l_2()
                .border_color(rgb(theme().accent))
        })
        .on_drop(cx.listener(move |this, drag: &ColumnDrag, _, cx| {
            this.move_column(*drag, pane, Some(column), cx)
        }))
        .child(
            div()
                .id(("column-label", pane * 100 + column.key()))
                .flex_1()
                .min_w_0()
                .h_full()
                .flex()
                .items_center()
                .truncate()
                .when(column == Column::Name, |s| s.pl(px(ICON_W + 8.)))
                .when(active, |s| s.text_color(rgb(theme().text)))
                .cursor_pointer()
                .hover(|s| s.bg(rgb(theme().hover)))
                .tooltip(tip(
                    "Drag to reorder · drag the edge to resize · right-click to choose columns",
                ))
                .on_drag(drag, move |_, _, _, cx| {
                    cx.new(|_| TabDragPreview {
                        label: column.label().into(),
                    })
                })
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(key) = key {
                        this.set_sort(pane, key, cx);
                    }
                }))
                .child(label),
        )
        .child(
            div()
                .id(("column-resize", pane * 100 + column.key()))
                .flex_none()
                .w(px(9.))
                .h_full()
                .flex()
                .justify_center()
                .cursor(CursorStyle::ResizeLeftRight)
                .hover(|s| s.bg(rgb(theme().selected)))
                .child(
                    div()
                        .w(px(1.))
                        .h_full()
                        .my_1()
                        .bg(rgb(theme().border_strong)),
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                        this.begin_resize(pane, column, f32::from(ev.position.x));
                        cx.stop_propagation();
                        cx.notify();
                    }),
                ),
        )
        .into_any_element()
}

pub(super) fn column_text(entry: &Entry, column: Column, location: &Path) -> String {
    if entry.name == ".." {
        return String::new();
    }
    match column {
        Column::Name => entry.name.clone(),
        Column::Kind => kind_label(&entry.name, entry.is_dir),
        Column::Modified => {
            if entry.loaded {
                format_date(entry.modified)
            } else {
                "—".into()
            }
        }
        Column::Created => {
            if entry.loaded {
                format_date(entry.created)
            } else {
                "—".into()
            }
        }
        Column::Size => {
            if !entry.loaded || entry.is_dir {
                "—".into()
            } else {
                format_size(false, entry.size)
            }
        }
        Column::Extension => {
            if entry.is_folder() {
                "—".into()
            } else {
                Path::new(&entry.name)
                    .extension()
                    .map(|e| e.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "—".into())
            }
        }
        Column::Permissions => entry
            .mode
            .map(|m| mode_string(m, entry.is_dir))
            .unwrap_or_else(|| "—".into()),
        Column::Location => display_path(location),
    }
}
