//! Exact-folder sticky ordering. The private drag payload can never transfer files.
use super::*;

pub(super) fn folder_key(tab: &Tab) -> String {
    // Keep local and SSH paths (and distinct hosts/users) separate. No live mux,
    // credentials or connection activity is needed to identify this folder.
    let server = tab.remote.as_ref().map(|s| (&s.host, &s.user, s.port));
    serde_json::to_string(&(server, &tab.current_dir)).unwrap_or_default()
}

pub(super) fn rank(order: &[String], name: &str) -> usize {
    order.iter().position(|n| n == name).unwrap_or(usize::MAX)
}
pub(super) fn prioritize(entries: &mut [Entry], order: &[String]) {
    if order.is_empty() {
        return;
    }
    let ranks: HashMap<&str, usize> = order
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i))
        .collect();
    // Stable partition: the chosen normal sort is retained for every other item.
    entries.sort_by_key(|e| ranks.get(e.name.as_str()).copied().unwrap_or(usize::MAX));
}
fn moved(mut order: Vec<String>, from: &str, target: &str, after: bool) -> Vec<String> {
    if from == target || !order.iter().any(|n| n == from) || !order.iter().any(|n| n == target) {
        return order;
    }
    let item = order.remove(order.iter().position(|n| n == from).unwrap());
    let index = order.iter().position(|n| n == target).unwrap() + usize::from(after);
    order.insert(index, item);
    order
}

impl Tab {
    pub(super) fn visible_sticky_count(&self) -> usize {
        self.visible_entries()
            .take_while(|e| self.sticky_order.contains(&e.name))
            .count()
    }
    pub(super) fn sticky_icon_row(&self, index: usize, cols: usize) -> usize {
        let pinned = self.visible_sticky_count();
        if index < pinned {
            index / cols
        } else {
            pinned.div_ceil(cols) + (index - pinned) / cols
        }
    }
    pub(super) fn sticky_icon_vertical(&self, index: usize, dy: i32, cols: usize) -> usize {
        let row = self.sticky_icon_row(index, cols);
        let pinned = self.visible_sticky_count();
        let col = if index < pinned {
            index % cols
        } else {
            (index - pinned) % cols
        };
        let max_row = self.sticky_icon_row(self.visible_count().saturating_sub(1), cols);
        let next_row = (row as i32 + dy).clamp(0, max_row as i32) as usize;
        if row == next_row {
            return index;
        }
        (0..=col)
            .rev()
            .find_map(|c| self.sticky_icon_index(next_row, c, cols))
            .unwrap_or(index)
    }
    pub(super) fn sticky_icon_index(&self, row: usize, col: usize, cols: usize) -> Option<usize> {
        let pinned = self.visible_sticky_count();
        let sticky_rows = pinned.div_ceil(cols);
        let index = if row < sticky_rows {
            let index = row * cols + col;
            if index >= pinned {
                return None;
            }
            index
        } else {
            pinned + (row - sticky_rows) * cols + col
        };
        (index < self.visible_count()).then_some(index)
    }
}

pub(super) fn divider<T: ParentElement + Styled>(element: T, shown: bool) -> T {
    if !shown {
        return element;
    }
    element.child(
        gpui::canvas(
            |bounds, _, _| bounds,
            |_, bounds, window, _| {
                window.paint_quad(gpui::fill(
                    Bounds::new(
                        point(bounds.left() + px(8.), bounds.bottom() - px(1.)),
                        size((bounds.size.width - px(16.)).max(px(0.)), px(1.)),
                    ),
                    rgb(CHROME_HIGHLIGHT),
                ));
            },
        )
        .absolute()
        // Without explicit insets, the canvas starts after the row's content
        // and its bottom edge lands one row below the pinned group.
        .top_0()
        .left_0()
        .size_full(),
    )
}

#[derive(Clone)]
struct StickyDrag {
    owner: gpui::EntityId,
    tab: u64,
    project: Option<u64>,
    folder: String,
    name: String,
}
impl Schnauz {
    pub(super) fn load_sticky(&mut self, pane: usize) {
        let order = self
            .folder_views
            .borrow()
            .sticky(self.projects.active, &folder_key(self.tab(pane)));
        if self.tab(pane).sticky_order != order {
            let tab = self.tab_mut(pane);
            tab.sticky_order = order;
            sort_entries(&mut tab.entries, tab.sort_key, tab.sort_asc);
            tab.recompute_results();
        }
    }
    pub(super) fn rename_sticky(
        &mut self,
        folder: String,
        pairs: &[(PathBuf, PathBuf)],
        cx: &mut Context<Self>,
    ) {
        // Search/batch rename can contain children from several directories.
        // Update each actual parent, retaining the captured provider identity.
        let Ok((server, _)) =
            serde_json::from_str::<(Option<(String, String, u16)>, PathBuf)>(&folder)
        else {
            return;
        };
        let mut groups = HashMap::<PathBuf, Vec<(String, String)>>::new();
        for (from, to) in pairs {
            if from.parent() != to.parent() {
                continue;
            }
            let (Some(parent), Some(old), Some(new)) = (
                from.parent(),
                from.file_name().and_then(|n| n.to_str()),
                to.file_name().and_then(|n| n.to_str()),
            ) else {
                continue;
            };
            groups
                .entry(parent.into())
                .or_default()
                .push((old.into(), new.into()));
        }
        let mut changed = false;
        for (parent, names) in groups {
            let key = serde_json::to_string(&(&server, parent)).unwrap_or_default();
            changed |= self.folder_views.borrow_mut().rename_sticky(&key, &names);
        }
        if changed {
            self.publish_view_change(
                folder_views::Scope::AllProjects,
                pairs
                    .first()
                    .and_then(|(p, _)| p.parent())
                    .unwrap_or(Path::new("/"))
                    .into(),
                false,
                cx,
            );
        }
    }
    fn save_sticky(&mut self, pane: usize, order: Vec<String>, cx: &mut Context<Self>) {
        let error = self.folder_views.borrow().error.clone();
        if let Some(error) = error {
            self.set_notice(
                format!("Couldn't read saved views: {error}. Existing settings preserved."),
                cx,
            );
            return;
        }
        let key = folder_key(self.tab(pane));
        self.folder_views
            .borrow_mut()
            .set_sticky(self.projects.active, key, order);
        self.pending_click_rename = None;
        self.drag_candidate = None;
        self.publish_view_change(
            folder_views::Scope::Project(self.projects.active),
            self.tab(pane).current_dir.clone(),
            false,
            cx,
        );
    }
    pub(super) fn sticky_menu_node(&self, pane: usize, path: &Path) -> Option<MenuNode> {
        // Recursive search results belong to different directories; folder order
        // is edited while browsing that actual folder, rather than the search.
        if self.search_in_pane(pane) || path.parent() != Some(self.tab(pane).current_dir.as_path())
        {
            return None;
        }
        let tab = self.tab(pane);
        let names: Vec<String> = tab
            .visible_entries()
            .filter(|e| {
                let p = tab.current_dir.join(&e.name);
                p == path || tab.selection.contains(&p)
            })
            .map(|e| e.name.clone())
            .collect();
        if names.is_empty() {
            return None;
        }
        let remove = names.iter().all(|n| tab.sticky_order.contains(n));
        let captured = (self.projects.active, tab.id, folder_key(tab));
        Some(mi(
            if remove {
                "Unstick from Top"
            } else {
                "Stick to Top"
            },
            move |this, _, cx| {
                this.close_context_menu(cx);
                if pane >= this.panes.len()
                    || this.projects.active != captured.0
                    || this.tab(pane).id != captured.1
                    || folder_key(this.tab(pane)) != captured.2
                {
                    return;
                }
                let mut order = this.tab(pane).sticky_order.clone();
                if remove {
                    order.retain(|n| !names.contains(n));
                } else {
                    for name in &names {
                        if !order.contains(name) {
                            order.push(name.clone());
                        }
                    }
                }
                this.save_sticky(pane, order, cx);
            },
        ))
    }
    pub(super) fn sticky_handle(
        &self,
        pane: usize,
        path: &Path,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let tab = self.tab(pane);
        let name = path.file_name()?.to_str()?.to_owned();
        if !tab.sticky_order.contains(&name)
            || self
                .rename
                .as_ref()
                .is_some_and(|r| r.pane == pane && r.path == path)
        {
            return None;
        }
        let drag = StickyDrag {
            owner: cx.entity_id(),
            tab: tab.id,
            project: self.projects.active,
            folder: folder_key(tab),
            name,
        };
        Some(
            div()
                .id("sticky-handle")
                .flex_none()
                .size(px(18.))
                .flex()
                .items_center()
                .justify_center()
                .cursor(CursorStyle::OpenHand)
                .tooltip(tip(
                    "Sticky · drag this pin to reorder; right-click to unstick",
                ))
                .child(ui_icon(Icon::PinDocked, 12., rgb(CHROME_HIGHLIGHT)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseDownEvent, _, cx| {
                        this.pending_click_rename = None;
                        this.drag_candidate = None;
                        cx.stop_propagation();
                    }),
                )
                .on_click(|_, _, cx| cx.stop_propagation())
                .on_drag(drag, |source, _, _, cx| {
                    cx.new(|_| TabDragPreview {
                        label: source.name.clone(),
                    })
                })
                .into_any_element(),
        )
    }
    pub(super) fn sticky_target(
        &self,
        pane: usize,
        path: PathBuf,
        horizontal: bool,
        content: AnyElement,
        cx: &Context<Self>,
    ) -> AnyElement {
        let tab = self.tab(pane);
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let pinned = tab.sticky_order.contains(&name);
        let count = tab.visible_sticky_count();
        let last = !horizontal
            && count > 0
            && count < tab.visible_count()
            && tab
                .visible_entries()
                .nth(count - 1)
                .is_some_and(|e| e.name == name);
        if !pinned {
            return content;
        }
        let bounds = Rc::new(RefCell::new(None::<Bounds<gpui::Pixels>>));
        let paint_bounds = bounds.clone();
        let hover_bounds = bounds.clone();
        let marker = Rc::new(std::cell::Cell::new(None::<bool>));
        let paint_marker = marker.clone();
        let owner = cx.entity_id();
        let project = self.projects.active;
        let folder = folder_key(tab);
        let hover_folder = folder.clone();
        let tab_id = tab.id;
        let hover_name = name.clone();
        let target = div()
            .id(SharedString::from(format!("sticky-target:{pane}:{name}")))
            .relative()
            .when(horizontal, |d| d.flex_none())
            .child(content)
            .child(
                gpui::canvas(
                    move |b, _, _| {
                        *paint_bounds.borrow_mut() = Some(b);
                        b
                    },
                    move |_, b, window, _| {
                        if let Some(after) = paint_marker.take() {
                            let rect = if horizontal {
                                Bounds::new(
                                    point(
                                        if after { b.right() - px(2.) } else { b.left() },
                                        b.top(),
                                    ),
                                    size(px(2.), b.size.height),
                                )
                            } else {
                                Bounds::new(
                                    point(
                                        b.left(),
                                        if after { b.bottom() - px(2.) } else { b.top() },
                                    ),
                                    size(b.size.width, px(2.)),
                                )
                            };
                            window.paint_quad(gpui::fill(rect, rgb(CHROME_HIGHLIGHT)));
                        }
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .drag_over::<StickyDrag>(move |style, source, window, _| {
                if source.owner == owner
                    && source.project == project
                    && source.folder == hover_folder
                    && source.tab == tab_id
                    && source.name != hover_name
                {
                    marker.set(Some(hover_bounds.borrow().is_some_and(|b| {
                        if horizontal {
                            window.mouse_position().x >= b.center().x
                        } else {
                            window.mouse_position().y >= b.center().y
                        }
                    })));
                }
                style
            })
            .on_drop(cx.listener(move |this, source: &StickyDrag, window, cx| {
                cx.stop_propagation();
                if source.owner != cx.entity_id()
                    || source.project != this.projects.active
                    || pane >= this.panes.len()
                    || source.tab != this.tab(pane).id
                    || source.folder != folder_key(this.tab(pane))
                    || source.folder != folder
                {
                    return;
                }
                let after = bounds.borrow().is_some_and(|b| {
                    if horizontal {
                        window.mouse_position().x >= b.center().x
                    } else {
                        window.mouse_position().y >= b.center().y
                    }
                });
                let order = this.tab(pane).sticky_order.clone();
                let next = moved(order.clone(), &source.name, &name, after);
                if next != order {
                    this.save_sticky(pane, next, cx);
                }
            }));
        divider(target, last).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manual_order_is_independent_of_sort_direction_and_filters() {
        let mut tab = Tab::new("/fixture".into());
        tab.entries = ["b", "c", "a", ".hidden"]
            .into_iter()
            .map(|name| Entry {
                name: name.into(),
                is_dir: false,
                link: None,
                size: 0,
                modified: None,
                modified_date_only: false,
                created: None,
                mode: None,
                loaded: true,
                hidden_flag: false,
            })
            .collect();
        tab.sticky_order = vec!["c".into(), ".hidden".into(), "a".into()];
        for ascending in [true, false] {
            sort_entries(&mut tab.entries, SortKey::Name, ascending);
            tab.recompute_results();
            assert_eq!(
                tab.visible_entries()
                    .map(|e| e.name.as_str())
                    .collect::<Vec<_>>(),
                ["c", "a", "b"]
            );
            assert_eq!(tab.visible_sticky_count(), 2);
            assert_eq!(tab.sticky_icon_row(2, 3), 1);
            assert_eq!(tab.sticky_icon_index(0, 2, 3), None);
            assert_eq!(tab.sticky_icon_index(1, 0, 3), Some(2));
        }
        let order = vec!["a".into(), "b".into(), "c".into()];
        assert_eq!(moved(order.clone(), "a", "c", true), ["b", "c", "a"]);
        assert_eq!(moved(order.clone(), "c", "a", false), ["c", "a", "b"]);
        assert_eq!(moved(order.clone(), "gone", "a", false), order);
    }
}
