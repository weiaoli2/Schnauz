//! Inspect is explicit and independent of ordinary folder browsing.
use super::*;
use crate::space_map::{self, Rect, Tile, Tree};
static NEXT_INSPECTION: AtomicU64 = AtomicU64::new(1);
#[derive(Default)]
struct Geometry {
    key: Option<(usize, f32, f32, f32, f32)>,
    tiles: Arc<Vec<Tile>>,
}
pub(super) struct State {
    id: u64,
    root: PathBuf,
    pane: usize,
    cancel: Arc<AtomicBool>,
    tree: Option<Arc<Tree>>,
    view_root: usize,
    progress: space_map::Progress,
    error: Option<String>,
    hovered: Option<usize>,
    selected: Option<usize>,
    geometry: Rc<RefCell<Geometry>>,
    expanded_from: Option<(f32, f32)>,
}
impl Drop for State {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}
enum Update {
    Progress(space_map::Progress),
    Done(Option<Tree>),
}

impl Shuffle {
    pub(super) fn inspect_folder(
        &mut self,
        pane: usize,
        root: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_context_menu(cx);
        self.close_sidebar_menu(cx);
        self.close_palette(cx);
        if !root.is_dir() {
            self.set_notice("Choose a local folder to inspect.".into(), cx);
            return;
        }
        let mut expanded_from = self.space_map.as_ref().and_then(|s| s.expanded_from);
        if self.space_map.is_none() && !window.is_fullscreen() {
            if let Some(display) = window.display(cx) {
                let bounds = window.bounds();
                let width = f32::from(bounds.size.width);
                let available = f32::from(display.bounds().right() - bounds.left());
                let expanded = (width + 420.).min(available).max(width);
                if expanded - width > 8. {
                    expanded_from = Some((width, expanded));
                    window.resize(size(px(expanded), window.viewport_size().height));
                }
            }
        }
        let id = NEXT_INSPECTION.fetch_add(1, Ordering::Relaxed);
        let cancel = Arc::new(AtomicBool::new(false));
        self.space_map = Some(State {
            id,
            root: root.clone(),
            pane,
            cancel: cancel.clone(),
            tree: None,
            view_root: 0,
            progress: Default::default(),
            error: None,
            hovered: None,
            selected: None,
            geometry: Default::default(),
            expanded_from,
        });
        window.focus(&self.focus);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let (tx, rx) = std::sync::mpsc::channel();
            let worker_cancel = cancel.clone();
            cx.background_spawn(async move {
                let tree = space_map::scan(&root, &worker_cancel, |p| {
                    let _ = tx.send(Update::Progress(p));
                });
                let _ = tx.send(Update::Done(tree));
            })
            .detach();
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                if cancel.load(Ordering::Acquire) {
                    return;
                }
                let mut updates = Vec::new();
                let disconnected = loop {
                    match rx.try_recv() {
                        Ok(update) => updates.push(update),
                        Err(std::sync::mpsc::TryRecvError::Empty) => break false,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => break true,
                    }
                };
                if updates.is_empty() && !disconnected {
                    continue;
                }
                let complete = updates.iter().any(|u| matches!(u, Update::Done(_)));
                let done = complete || disconnected;
                let active = this
                    .update(cx, |this, cx| {
                        let Some(state) = this.space_map.as_mut().filter(|s| s.id == id) else {
                            return false;
                        };
                        for update in updates {
                            match update {
                                Update::Progress(p) => state.progress = p,
                                Update::Done(Some(tree)) => state.tree = Some(Arc::new(tree)),
                                Update::Done(None) => {
                                    state.error = Some("This folder could not be inspected.".into())
                                }
                            }
                        }
                        if disconnected && !complete {
                            state.error = Some("This folder could not be inspected.".into());
                        }
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !active {
                    cancel.store(true, Ordering::Release);
                    return;
                }
                if done {
                    return;
                }
            }
        })
        .detach();
    }
    fn close_space_map(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(state) = self.space_map.take() {
            if let Some((old, expanded)) = state.expanded_from {
                if (f32::from(window.viewport_size().width) - expanded).abs() < 8. {
                    window.resize(size(px(old), window.viewport_size().height));
                }
            }
        }
        cx.notify();
    }
    fn select_map_node(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = self.space_map.as_mut() else {
            return;
        };
        let Some(tree) = state.tree.as_ref() else {
            return;
        };
        let Some(node) = tree.nodes.get(index) else {
            return;
        };
        let path = node.path.clone();
        let directory = node.directory;
        let pane = state.pane.min(self.panes.len() - 1);
        state.selected = Some(index);
        if directory {
            state.view_root = index;
            state.hovered = None;
            state.geometry.borrow_mut().key = None;
        }
        self.active_pane = pane;
        if self.tab(pane).remote.is_some() {
            let folder = if directory {
                path.clone()
            } else {
                path.parent().unwrap_or(&path).to_path_buf()
            };
            self.panes[pane].tabs.push(Tab::new(folder));
            self.panes[pane].active = self.panes[pane].tabs.len() - 1;
        }
        self.tab_mut(pane).find_query = None;
        self.tab_mut(pane).recompute_results();
        if directory {
            self.navigate_in(pane, path, cx);
        } else if let Some(parent) = path.parent() {
            let folder = parent.to_path_buf();
            use std::os::macos::fs::MetadataExt;
            let hidden = path
                .file_name()
                .is_some_and(|n| n.as_encoded_bytes().starts_with(b"."))
                || fs::metadata(&path).is_ok_and(|m| m.st_flags() & libc::UF_HIDDEN != 0);
            self.tab_mut(pane).show_hidden |= hidden;
            if self.tab(pane).entry_filter == EntryFilter::Folders {
                self.tab_mut(pane).set_entry_filter(EntryFilter::All);
            }
            self.navigate_in(pane, folder, cx);
            self.reveal_in_pane(pane, path, cx);
        }
        window.focus(&self.focus);
        cx.notify();
    }
    pub(super) fn render_space_map(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let state = self.space_map.as_ref()?;
        let t = theme();
        let current = state
            .tree
            .as_ref()
            .map(|tree| &tree.nodes[state.view_root].path)
            .unwrap_or(&state.root);
        let root_label = path_label(current);
        let mut panel = div()
            .w(px(420.))
            .flex_none()
            .min_h_0()
            .flex()
            .flex_col()
            .border_l_1()
            .border_color(rgb(t.border))
            .bg(rgb(t.surface))
            .child(
                div()
                    .px_3()
                    .py_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Inspect"),
                    )
                    .child(
                        map_button("inspect-refresh", "Refresh").on_click(cx.listener(
                            |this, _: &ClickEvent, window, cx| {
                                let Some(state) = &this.space_map else {
                                    return;
                                };
                                let root = state
                                    .tree
                                    .as_ref()
                                    .map(|t| t.nodes[state.view_root].path.clone())
                                    .unwrap_or_else(|| state.root.clone());
                                let pane = state.pane;
                                this.inspect_folder(pane, root, window, cx);
                            },
                        )),
                    )
                    .child(
                        map_button("inspect-close", "×")
                            .tooltip(tip("Close folder inspection"))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.close_space_map(window, cx)
                            })),
                    ),
            )
            .child(
                div()
                    .px_3()
                    .pb_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        map_button("inspect-up", "↑")
                            .tooltip(tip("Zoom out one folder"))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                let parent = this.space_map.as_ref().and_then(|s| {
                                    s.tree.as_ref().and_then(|t| t.nodes[s.view_root].parent)
                                });
                                if let Some(parent) = parent {
                                    this.select_map_node(parent, window, cx);
                                }
                            })),
                    )
                    .child(
                        div()
                            .id("inspect-folder-title")
                            .min_w_0()
                            .flex_1()
                            .truncate()
                            .cursor_pointer()
                            .child(root_label)
                            .tooltip(tip(current.to_string_lossy().into_owned()))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                if let Some(state) = &this.space_map {
                                    let index = state.view_root;
                                    this.select_map_node(index, window, cx);
                                }
                            })),
                    ),
            );
        if let Some(tree) = &state.tree {
            let root = state.view_root;
            panel = panel.child(
                div()
                    .px_3()
                    .pb_2()
                    .text_size(px(11.))
                    .text_color(rgb(t.text_muted))
                    .child(format!(
                        "{} · {} {}",
                        format_size(false, tree.nodes[root].bytes),
                        tree.counts[root],
                        if tree.counts[root] == 1 {
                            "file"
                        } else {
                            "files"
                        }
                    )),
            );
            if tree.nodes[root].bytes == 0 {
                panel = panel.child(div().flex_1().p_4().text_color(rgb(t.text_muted)).child(
                    "No file data to display. This folder is empty or its files have zero size.",
                ));
            } else {
                let geometry = state.geometry.clone();
                let paint_tree = tree.clone();
                let layout_tree = tree.clone();
                let pre_geometry = geometry.clone();
                let selected = state.hovered.or(state.selected);
                panel = panel.child(
                    div()
                        .id("inspect-map")
                        .flex_1()
                        .min_h_0()
                        .mx_2()
                        .overflow_hidden()
                        .cursor_pointer()
                        .child(
                            gpui::canvas(
                                move |bounds, _, _| {
                                    let rect = Rect {
                                        x: f32::from(bounds.origin.x),
                                        y: f32::from(bounds.origin.y),
                                        w: f32::from(bounds.size.width),
                                        h: f32::from(bounds.size.height),
                                    };
                                    let key = Some((root, rect.x, rect.y, rect.w, rect.h));
                                    let mut geometry = pre_geometry.borrow_mut();
                                    if geometry.key != key {
                                        geometry.tiles =
                                            Arc::new(space_map::layout(&layout_tree, root, rect));
                                        geometry.key = key;
                                    }
                                    geometry.tiles.clone()
                                },
                                move |_, tiles, window, cx| {
                                    for tile in tiles.iter() {
                                        let r = tile.rect;
                                        let bounds = Bounds::new(
                                            point(px(r.x), px(r.y)),
                                            size(px((r.w - 1.).max(0.)), px((r.h - 1.).max(0.))),
                                        );
                                        let node = &paint_tree.nodes[tile.node];
                                        let color = if tile.folder {
                                            0x293649
                                        } else {
                                            file_color(&node.path)
                                        };
                                        window.paint_quad(gpui::fill(
                                            bounds,
                                            rgb(if selected == Some(tile.node) {
                                                0xf4f7ff
                                            } else {
                                                color
                                            }),
                                        ));
                                        if selected == Some(tile.node) {
                                            window.paint_quad(gpui::fill(
                                                Bounds::new(
                                                    point(px(r.x + 2.), px(r.y + 2.)),
                                                    size(
                                                        px((r.w - 5.).max(0.)),
                                                        px((r.h - 5.).max(0.)),
                                                    ),
                                                ),
                                                rgb(color),
                                            ));
                                        }
                                        if r.w > 48. && r.h > 14. {
                                            let name = format!(
                                                "{}{}",
                                                if tile.folder { "▸ " } else { "" },
                                                path_label(&node.path)
                                            );
                                            paint_label(
                                                name,
                                                point(px(r.x + 4.), px(r.y + 2.)),
                                                bounds,
                                                if tile.folder { 10. } else { 9. },
                                                window,
                                                cx,
                                            );
                                            if !tile.folder && r.h > 34. {
                                                paint_label(
                                                    format_size(false, node.bytes),
                                                    point(px(r.x + 4.), px(r.y + 16.)),
                                                    bounds,
                                                    9.,
                                                    window,
                                                    cx,
                                                );
                                            }
                                        }
                                    }
                                },
                            )
                            .size_full(),
                        )
                        .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                            let hovered = space_map::hit(
                                &geometry.borrow().tiles,
                                f32::from(event.position.x),
                                f32::from(event.position.y),
                            );
                            if let Some(state) = this.space_map.as_mut() {
                                if state.hovered != hovered {
                                    state.hovered = hovered;
                                    cx.notify();
                                }
                            }
                        }))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, event: &MouseDownEvent, window, cx| {
                                let index = this.space_map.as_ref().and_then(|s| {
                                    space_map::hit(
                                        &s.geometry.borrow().tiles,
                                        f32::from(event.position.x),
                                        f32::from(event.position.y),
                                    )
                                });
                                if let Some(index) = index {
                                    this.select_map_node(index, window, cx);
                                }
                                cx.stop_propagation();
                            }),
                        ),
                );
            }
            if tree.unreadable > 0 || tree.limited {
                panel = panel.child(
                    div()
                        .px_3()
                        .py_1()
                        .text_size(px(10.))
                        .text_color(rgb(t.text_muted))
                        .child(if tree.limited {
                            "Partial map: inspect a smaller folder to see every item."
                        } else {
                            "Partial map: some folders could not be read."
                        }),
                );
            }
            let node = &tree.nodes[state.hovered.or(state.selected).unwrap_or(root)];
            panel = panel.child(
                div()
                    .px_3()
                    .py_2()
                    .flex_none()
                    .h(px(70.))
                    .border_t_1()
                    .border_color(rgb(t.border))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().truncate().text_size(px(11.)).child(format!(
                        "{} · {}",
                        if node.directory { "Folder" } else { "File" },
                        path_label(&node.path)
                    )))
                    .child(
                        div()
                            .truncate()
                            .text_size(px(10.))
                            .text_color(rgb(t.text_muted))
                            .child(display_path(&node.path)),
                    )
                    .child(div().text_size(px(10.)).child(format!(
                        "{} · click to {}",
                        format_size(false, node.bytes),
                        if node.directory {
                            "open folder and zoom"
                        } else {
                            "show file on the left"
                        }
                    ))),
            );
        } else {
            panel = panel.child(div().flex_1().p_4().text_color(rgb(t.text_muted)).child(
                if let Some(error) = &state.error {
                    error.clone()
                } else {
                    format!(
                        "Scanning folder…\n{} files · {}",
                        state.progress.files,
                        format_size(false, state.progress.bytes)
                    )
                },
            ));
        }
        panel = panel.child(
            div()
                .px_3()
                .py_2()
                .text_size(px(10.))
                .text_color(rgb(t.text_dim))
                .child("Area = file size · color = file type · includes hidden items"),
        );
        Some(panel.into_any_element())
    }
}
fn map_button(id: &'static str, label: &'static str) -> gpui::Stateful<gpui::Div> {
    let t = theme();
    div()
        .id(id)
        .px_2()
        .py_1()
        .rounded_md()
        .cursor_pointer()
        .text_size(px(11.))
        .text_color(rgb(t.text_muted))
        .hover(|s| s.bg(rgb(t.hover)))
        .child(label)
}
fn file_color(path: &Path) -> u32 {
    const COLORS: [u32; 8] = [
        0x416e99, 0x537650, 0x8c6553, 0x6e5894, 0x927039, 0x3c7c7a, 0x8d5075, 0x586b83,
    ];
    let ext = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    COLORS[ext
        .bytes()
        .fold(0usize, |h, b| h.wrapping_mul(31).wrapping_add(b as usize))
        % COLORS.len()]
}
fn paint_label(
    text: String,
    origin: gpui::Point<gpui::Pixels>,
    bounds: Bounds<gpui::Pixels>,
    font_size: f32,
    window: &mut Window,
    cx: &mut App,
) {
    let text: SharedString = text.into();
    let run = gpui::TextRun {
        len: text.len(),
        font: window.text_style().font(),
        color: rgb(0xffffff).into(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = window
        .text_system()
        .shape_line(text, px(font_size), &[run], None);
    window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
        let _ = line.paint(origin, px(font_size + 3.), window, cx);
    });
}
