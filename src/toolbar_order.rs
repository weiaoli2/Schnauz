//! Persisted, shared order of file-action controls; drag payloads contain no files.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Control {
    Folder,
    File,
    Share,
    Favourite,
    Trash,
    Network,
    Hidden,
    Search,
    Views,
    Options,
    Sort,
}
use Control::*;
pub(super) const DEFAULT: [Control; 11] = [
    Folder, File, Share, Favourite, Trash, Network, Hidden, Search, Views, Options, Sort,
];
impl Control {
    pub(super) fn key(self) -> &'static str {
        match self {
            Folder => "new-folder",
            File => "new-file",
            Share => "share",
            Favourite => "favourite",
            Trash => "trash",
            Network => "network",
            Hidden => "hidden",
            Search => "search",
            Views => "views",
            Options => "options",
            Sort => "sort",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Folder => "New folder",
            File => "New text file",
            Share => "Share",
            Favourite => "Favourites",
            Trash => "Trash",
            Network => "Network drive",
            Hidden => "Hidden items",
            Search => "Search",
            Views => "List / Icons",
            Options => "View options",
            Sort => "Sort by",
        }
    }
}
pub(super) fn encode(order: [Control; 11]) -> String {
    order.map(Control::key).join(",")
}
pub(super) fn decode(value: &str) -> [Control; 11] {
    let mut result = Vec::new();
    for key in value.trim().split(',') {
        if let Some(control) = DEFAULT.into_iter().find(|control| control.key() == key) {
            if !result.contains(&control) {
                result.push(control);
            }
        }
    }
    if result.is_empty() {
        return DEFAULT;
    }
    // A newly introduced File control belongs beside Folder in older saved orders.
    if !result.contains(&File) {
        let index = result
            .iter()
            .position(|control| *control == Folder)
            .map_or(0, |i| i + 1);
        result.insert(index, File);
    }
    for control in DEFAULT {
        if !result.contains(&control) {
            result.push(control);
        }
    }
    result.try_into().unwrap()
}
fn moved(order: [Control; 11], source: Control, target: Control, after: bool) -> [Control; 11] {
    if source == target {
        return order;
    }
    let mut result = order.to_vec();
    result.retain(|control| *control != source);
    let index = result
        .iter()
        .position(|control| *control == target)
        .unwrap()
        + usize::from(after);
    result.insert(index, source);
    result.try_into().unwrap()
}
#[derive(Clone)]
struct ToolbarDrag(Control);

impl Schnauz {
    fn sortable_toolbar_control(
        &self,
        control: Control,
        pane: usize,
        content: AnyElement,
        cx: &Context<Self>,
    ) -> AnyElement {
        let bounds = Rc::new(RefCell::new(None::<Bounds<gpui::Pixels>>));
        let paint_bounds = bounds.clone();
        let hover_bounds = bounds.clone();
        let marker = Rc::new(std::cell::Cell::new(None::<bool>));
        let paint_marker = marker.clone();
        div()
            .id((
                SharedString::from(format!("toolbar-order-{}", control.key())),
                pane,
            ))
            .relative()
            .flex_none()
            .flex()
            .items_center()
            .child(content)
            .children(
                (pane == 0)
                    .then(|| ui_checks::probe(control.key()))
                    .flatten(),
            )
            .child(
                gpui::canvas(
                    move |b, _, _| {
                        *paint_bounds.borrow_mut() = Some(b);
                        b
                    },
                    move |_, b, w, _| {
                        if let Some(after) = paint_marker.take() {
                            let x = if after { b.right() - px(2.) } else { b.left() };
                            w.paint_quad(gpui::fill(
                                Bounds::new(point(x, b.top()), size(px(2.), b.size.height)),
                                rgb(theme().accent),
                            ));
                        }
                    },
                )
                .absolute()
                .size_full(),
            )
            .on_drag(ToolbarDrag(control), move |_, _, _, cx| {
                cx.new(|_| TabDragPreview {
                    label: control.label().into(),
                })
            })
            .drag_over::<ToolbarDrag>(move |style, source, window, _| {
                if source.0 == control {
                    return style;
                }
                let after = hover_bounds
                    .borrow()
                    .is_some_and(|b| window.mouse_position().x >= b.center().x);
                // An inset marker leaves the toolbar geometry unchanged during dragging.
                marker.set(Some(after));
                style.bg(Theme::alpha(theme().accent, 0x20))
            })
            .on_drop(cx.listener(move |_, source: &ToolbarDrag, window, cx| {
                cx.stop_propagation();
                let after = bounds
                    .borrow()
                    .is_some_and(|b| window.mouse_position().x >= b.center().x);
                let mut p = prefs();
                let next = moved(p.toolbar_order, source.0, control, after);
                if next != p.toolbar_order {
                    p.toolbar_order = next;
                    apply_prefs(p, cx);
                }
            }))
            .into_any_element()
    }

    pub(super) fn render_ordered_toolbar(&self, pane: usize, cx: &Context<Self>) -> AnyElement {
        use toolbar_icons::toolbar_button as button;
        let view = self.tab(pane).view;
        let local = self.tab(pane).remote.is_none();
        let selected = !self.tab(pane).selection.is_empty();
        let tiny = self.pane_width(pane) > 0. && self.pane_width(pane) < 470.;
        let mut row = div().flex_none().flex().items_center().gap_1().pl_2();
        for control in prefs().toolbar_order {
            if (matches!(control, Folder | File | Share) && (!local || tiny))
                || (control == Trash && !local)
                || (control == Search && tiny)
            {
                continue;
            }
            let content = match control {
                Folder => button(
                    "tb-new-folder",
                    pane,
                    Icon::FolderPlus,
                    false,
                    true,
                    "New folder · ⇧⌘N",
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, w, cx| {
                    this.active_pane = pane;
                    this.new_folder(pane, w, cx);
                }))
                .into_any_element(),
                File => button(
                    "tb-new-file",
                    pane,
                    Icon::FilePlus,
                    false,
                    true,
                    "New text file…",
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, w, cx| {
                    this.active_pane = pane;
                    this.open_text_file_dialog(pane, w, cx);
                }))
                .into_any_element(),
                Share => button(
                    "tb-share",
                    pane,
                    Icon::Share,
                    false,
                    selected,
                    "Share selection · ⇧⌘S",
                )
                .when(selected, |d| {
                    d.on_click(cx.listener(move |this, ev: &ClickEvent, w, cx| {
                        this.active_pane = pane;
                        if let Some(view) = ns_view_ptr(w) {
                            show_share_picker(
                                view,
                                (f32::from(ev.position().x), f32::from(ev.position().y)),
                                &this.action_targets(pane),
                            );
                        }
                        cx.stop_propagation();
                    }))
                })
                .into_any_element(),
                Favourite => button(
                    "tb-project-tag",
                    pane,
                    Icon::Tag,
                    self.selection_is_favourite(pane),
                    selected,
                    "Add to Favourites",
                )
                .when(selected, |d| {
                    d.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.toggle_project_tags(pane, cx)
                    }))
                })
                .into_any_element(),
                Trash => button(
                    "tb-delete",
                    pane,
                    Icon::Trash,
                    false,
                    selected,
                    "Move selection to Trash · ⌘⌫",
                )
                .when(selected, |d| {
                    d.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.active_pane = pane;
                        this.request_delete(pane, cx);
                    }))
                })
                .into_any_element(),
                Network => button(
                    "connect-network",
                    pane,
                    Icon::Globe,
                    false,
                    !self.network_connecting,
                    "Connect to network drive…",
                )
                .relative()
                .children(ui_checks::probe("connect-network-button"))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.active_pane = pane;
                    this.open_network_dialog(cx);
                }))
                .into_any_element(),
                Hidden => self.render_hidden_button(pane, cx),
                Search => button(
                    "palette-search",
                    pane,
                    Icon::Search,
                    false,
                    true,
                    "Search files · ⌘P",
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, w, cx| {
                    this.active_pane = pane;
                    this.focus_file_search(false, w, cx);
                }))
                .into_any_element(),
                Views => div()
                    .flex()
                    .items_center()
                    .gap(px(1.))
                    .p(px(2.))
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(theme().border))
                    .bg(rgb(theme().sidebar))
                    .children(
                        [
                            (ViewMode::List, Icon::List, "view-list", "List view"),
                            (ViewMode::Icons, Icon::Grid, "view-icons", "Icon view"),
                        ]
                        .into_iter()
                        .map(|(mode, icon, id, label)| {
                            button(id, pane, icon, view == mode, true, label).on_click(cx.listener(
                                move |this, _: &ClickEvent, _, cx| this.set_view(pane, mode, cx),
                            ))
                        }),
                    )
                    .into_any_element(),
                Options => button(
                    "view-options",
                    pane,
                    Icon::Columns,
                    false,
                    true,
                    "View options · columns and saved folder views",
                )
                .on_click(cx.listener(move |this, ev: &ClickEvent, _, cx| {
                    this.active_pane = pane;
                    this.open_view_menu(
                        pane,
                        f32::from(ev.position().x),
                        f32::from(ev.position().y),
                        cx,
                    );
                    cx.stop_propagation();
                }))
                .into_any_element(),
                Sort => button("sort-by", pane, Icon::Sort, false, true, "Sort by")
                    .on_click(cx.listener(move |this, ev: &ClickEvent, _, cx| {
                        this.active_pane = pane;
                        this.sort_menu =
                            Some((pane, f32::from(ev.position().x), f32::from(ev.position().y)));
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .into_any_element(),
            };
            row = row.child(self.sortable_toolbar_control(control, pane, content, cx));
        }
        row.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_order_survives_missing_duplicate_and_unknown_controls() {
        assert_eq!(decode(&encode(DEFAULT)), DEFAULT);
        assert_eq!(decode("unknown,unknown"), DEFAULT);
        let restored = decode("sort,new-folder,new-folder,unknown,hidden");
        assert_eq!(&restored[..4], &[Sort, Folder, File, Hidden]);
        for control in DEFAULT {
            assert_eq!(restored.iter().filter(|c| **c == control).count(), 1);
        }
        let order = moved(DEFAULT, Folder, Sort, true);
        assert_eq!(order[10], Folder);
        assert_eq!(decode(&encode(order)), order);
        assert_eq!(moved(order, Folder, Folder, false), order);
        assert_eq!(moved(order, Folder, File, false), DEFAULT);
    }
}
