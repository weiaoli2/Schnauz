//! Spring-loaded tabs/folders. One cancellable dwell timer, with stable tab and
//! drag identities; changing selection or navigating never changes the payload.
use super::*;
use native_file_drag::Operation;
const DWELL: Duration = Duration::from_millis(650);

#[derive(Clone, Debug, PartialEq)]
enum Target {
    Tab {
        pane: usize,
        id: u64,
    },
    Folder {
        pane: usize,
        id: u64,
        parent: PathBuf,
        path: PathBuf,
    },
}
impl Target {
    fn pane(&self) -> usize {
        match self {
            Self::Tab { pane, .. } | Self::Folder { pane, .. } => *pane,
        }
    }
    fn id(&self) -> u64 {
        match self {
            Self::Tab { id, .. } | Self::Folder { id, .. } => *id,
        }
    }
}
#[derive(Default)]
pub(super) struct State {
    drag: usize,
    view: usize,
    count: usize,
    operation: Option<Operation>,
    copy_only: bool,
    pending: Option<(Target, Bounds<gpui::Pixels>)>,
    timer: Option<gpui::Task<()>>,
    generation: u64,
}
impl State {
    fn cancel_hover(&mut self) {
        self.pending = None;
        self.timer = None;
        self.generation += 1;
    }
    pub fn label(&self) -> Option<String> {
        self.operation
            .filter(|_| self.count > 0)
            .map(|op| op.label(self.count))
    }
    fn follow_pointer(&mut self, drag: usize, position: gpui::Point<gpui::Pixels>) {
        if self.drag != drag {
            self.cancel_hover();
            self.drag = drag;
        }
        if self
            .pending
            .as_ref()
            .is_some_and(|(_, b)| !b.contains(&position))
        {
            self.cancel_hover();
        }
    }
}
impl Schnauz {
    pub(super) fn clear_file_drag(&mut self, cx: &mut Context<Self>) {
        let was_active = self.file_drag.operation.is_some();
        self.file_drag = State {
            generation: self.file_drag.generation + 1,
            ..Default::default()
        };
        if was_active {
            cx.notify();
        }
    }
    pub(super) fn track_file_drag(
        &mut self,
        ev: &gpui::DragMoveEvent<ExternalPaths>,
        cx: &mut Context<Self>,
    ) {
        let drag = ev.dragged_item() as *const dyn std::any::Any as *const () as usize;
        let fresh = self.file_drag.drag != drag || self.file_drag.operation.is_none();
        self.file_drag.follow_pointer(drag, ev.event.position);
        let view = native_file_drag::destination_id();
        self.file_drag.view = view;
        // Count the payload, never the destination's selection after activation.
        self.file_drag.count = ev
            .dragged_item()
            .downcast_ref::<ExternalPaths>()
            .map_or(0, |p| p.paths().len());
        self.file_drag.copy_only = false;
        let op = native_file_drag::destination(false);
        if self.file_drag.operation != Some(op) {
            self.file_drag.operation = Some(op);
            cx.notify();
        }
        if fresh {
            cx.spawn(async move |this, cx| loop {
                cx.background_executor()
                    .timer(Duration::from_millis(60))
                    .await;
                let keep = this
                    .update(cx, |this, cx| {
                        if this.file_drag.drag != drag {
                            return false;
                        }
                        if !native_file_drag::active_in(view) || !cx.has_active_drag() {
                            this.clear_file_drag(cx);
                            return false;
                        }
                        let op = native_file_drag::destination(this.file_drag.copy_only);
                        if this.file_drag.operation != Some(op) {
                            this.file_drag.operation = Some(op);
                            cx.notify();
                        }
                        true
                    })
                    .unwrap_or(false);
                if !keep {
                    break;
                }
            })
            .detach();
        }
    }
    pub(super) fn drag_over_browser(
        &mut self,
        pane: usize,
        ev: &gpui::DragMoveEvent<ExternalPaths>,
        cx: &mut Context<Self>,
    ) {
        if !ev.bounds.contains(&ev.event.position) {
            return;
        }
        self.file_drag.copy_only = self.tab(pane).remote.is_some();
        let op = native_file_drag::destination(self.file_drag.copy_only);
        if self.file_drag.operation != Some(op) {
            self.file_drag.operation = Some(op);
            cx.notify();
        }
    }
    fn arm_spring(
        &mut self,
        target: Target,
        ev: &gpui::DragMoveEvent<ExternalPaths>,
        cx: &mut Context<Self>,
    ) {
        if !ev.bounds.contains(&ev.event.position) {
            return;
        }
        if self
            .file_drag
            .pending
            .as_ref()
            .is_some_and(|(t, _)| t == &target)
        {
            return;
        }
        self.file_drag.cancel_hover();
        self.file_drag.pending = Some((target.clone(), ev.bounds));
        let generation = self.file_drag.generation;
        let drag = self.file_drag.drag;
        let view = self.file_drag.view;
        self.file_drag.timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DWELL).await;
            this.update(cx, |this, cx| {
                if this.file_drag.generation != generation
                    || this.file_drag.drag != drag
                    || !native_file_drag::active_in(view)
                    || !cx.has_active_drag()
                {
                    return;
                }
                if this
                    .file_drag
                    .pending
                    .as_ref()
                    .is_none_or(|(t, _)| t != &target)
                {
                    return;
                }
                let pane = target.pane();
                let Some(p) = this.panes.get(pane) else {
                    return;
                };
                let Some(index) = p.tabs.iter().position(|t| t.id == target.id()) else {
                    return;
                };
                match &target {
                    Target::Tab { .. } => {
                        this.file_drag.cancel_hover();
                        this.select_tab(pane, index, cx);
                    }
                    Target::Folder { parent, path, .. } => {
                        if p.active != index || &p.tabs[index].current_dir != parent {
                            return;
                        }
                        let path = path.clone();
                        this.file_drag.cancel_hover();
                        this.navigate_in(pane, path, cx);
                    }
                }
            })
            .ok();
        }));
    }
    pub(super) fn spring_tab(
        &mut self,
        pane: usize,
        id: u64,
        ev: &gpui::DragMoveEvent<ExternalPaths>,
        cx: &mut Context<Self>,
    ) {
        if !ev.bounds.contains(&ev.event.position) {
            return;
        }
        let Some(tab) = self
            .panes
            .get(pane)
            .and_then(|p| p.tabs.iter().find(|t| t.id == id))
        else {
            return;
        };
        self.file_drag.copy_only = tab.remote.is_some();
        let op = native_file_drag::destination(self.file_drag.copy_only);
        if self.file_drag.operation != Some(op) {
            self.file_drag.operation = Some(op);
            cx.notify();
        }
        if self.tab(pane).id == id && self.active_pane == pane {
            return;
        }
        self.arm_spring(Target::Tab { pane, id }, ev, cx);
    }
    pub(super) fn spring_folder(
        &mut self,
        pane: usize,
        path: PathBuf,
        ev: &gpui::DragMoveEvent<ExternalPaths>,
        cx: &mut Context<Self>,
    ) {
        let tab = self.tab(pane);
        let target = Target::Folder {
            pane,
            id: tab.id,
            parent: tab.current_dir.clone(),
            path,
        };
        self.arm_spring(target, ev, cx);
    }
    pub(super) fn drop_on_tab(
        &mut self,
        pane: usize,
        id: u64,
        paths: Vec<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self
            .panes
            .get(pane)
            .and_then(|p| p.tabs.iter().position(|t| t.id == id))
        else {
            return;
        };
        self.select_tab(pane, index, cx);
        self.drop_files(pane, self.tab(pane).current_dir.clone(), paths, cx);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dwell_survives_moving_inside_target_and_cancels_on_exit_or_new_drag() {
        let bounds = Bounds::new(point(px(10.), px(10.)), size(px(120.), px(30.)));
        let target = Target::Tab { pane: 0, id: 42 };
        let mut state = State {
            drag: 7,
            count: 3,
            operation: Some(Operation::Copy),
            pending: Some((target, bounds)),
            ..Default::default()
        };
        state.follow_pointer(7, point(px(20.), px(20.)));
        assert!(state.pending.is_some());
        assert_eq!(state.generation, 0);
        assert_eq!(state.label().as_deref(), Some("Copy 3 items"));
        state.follow_pointer(7, point(px(200.), px(20.)));
        assert!(state.pending.is_none());
        let generation = state.generation;
        state.follow_pointer(8, point(px(20.), px(20.)));
        assert!(state.generation > generation);
        assert_ne!(Tab::new("/tmp".into()).id, Tab::new("/tmp".into()).id);
    }
}
