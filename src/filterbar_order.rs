//! Shared order of the Show / Filter / Mark groups. Drag labels, never files.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Group {
    Show,
    Filter,
    Mark,
}
pub(super) const DEFAULT: [Group; 3] = [Group::Show, Group::Filter, Group::Mark];
impl Group {
    fn key(self) -> &'static str {
        match self {
            Self::Show => "show",
            Self::Filter => "filter",
            Self::Mark => "mark",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Show => "Show",
            Self::Filter => "Filter:",
            Self::Mark => "Mark:",
        }
    }
}
pub(super) fn encode(order: [Group; 3]) -> String {
    order.map(Group::key).join(",")
}
pub(super) fn decode(value: &str) -> [Group; 3] {
    let mut groups = Vec::new();
    for key in value.split(',').map(str::trim) {
        if let Some(group) = DEFAULT.into_iter().find(|g| g.key() == key) {
            if !groups.contains(&group) {
                groups.push(group);
            }
        }
    }
    for group in DEFAULT {
        if !groups.contains(&group) {
            groups.push(group);
        }
    }
    groups.try_into().unwrap()
}
fn moved(order: [Group; 3], from: Group, to: Group, after: bool) -> [Group; 3] {
    if from == to {
        return order;
    }
    let mut next = order.to_vec();
    next.retain(|g| *g != from);
    let at = next.iter().position(|g| *g == to).unwrap() + usize::from(after);
    next.insert(at, from);
    next.try_into().unwrap()
}
#[derive(Clone)]
struct GroupDrag(Group);

impl Schnauz {
    pub(super) fn filterbar_label(
        &self,
        group: Group,
        pane: usize,
        _: &Context<Self>,
    ) -> AnyElement {
        div()
            .id((
                SharedString::from(format!("filterbar-label-{}", group.key())),
                pane,
            ))
            .flex_none()
            .cursor(gpui::CursorStyle::OpenHand)
            .tooltip(tip("Drag this label to rearrange Show, Filter and Mark"))
            .child(group.label())
            .on_drag(GroupDrag(group), move |_, _, _, cx| {
                cx.new(|_| TabDragPreview {
                    label: group.label().trim_end_matches(':').into(),
                })
            })
            .into_any_element()
    }

    pub(super) fn filterbar_group(
        &self,
        group: Group,
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
                SharedString::from(format!("filterbar-group-{}", group.key())),
                pane,
            ))
            .relative()
            .flex_none()
            .flex()
            .items_center()
            .rounded_sm()
            .child(content)
            .child(
                gpui::canvas(
                    move |b, _, _| {
                        *paint_bounds.borrow_mut() = Some(b);
                        b
                    },
                    move |_, b, window, _| {
                        if let Some(after) = paint_marker.take() {
                            let x = if after { b.right() - px(2.) } else { b.left() };
                            window.paint_quad(gpui::fill(
                                Bounds::new(point(x, b.top()), size(px(2.), b.size.height)),
                                rgb(theme().accent),
                            ));
                        }
                    },
                )
                .absolute()
                .size_full(),
            )
            .drag_over::<GroupDrag>(move |style, source, window, _| {
                if source.0 == group {
                    return style;
                }
                marker.set(Some(
                    hover_bounds
                        .borrow()
                        .is_some_and(|b| window.mouse_position().x >= b.center().x),
                ));
                style.bg(Theme::alpha(theme().accent, 0x18))
            })
            .on_drop(cx.listener(move |_, source: &GroupDrag, window, cx| {
                cx.stop_propagation();
                let after = bounds
                    .borrow()
                    .is_some_and(|b| window.mouse_position().x >= b.center().x);
                let mut p = prefs();
                let next = moved(p.filterbar_order, source.0, group, after);
                if next != p.filterbar_order {
                    p.filterbar_order = next;
                    apply_prefs(p, cx);
                }
            }))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn older_or_invalid_orders_keep_all_groups_and_saved_reordering_roundtrips() {
        assert_eq!(decode(""), DEFAULT);
        assert_eq!(
            decode("unknown,mark,mark, show"),
            [Group::Mark, Group::Show, Group::Filter]
        );
        let order = moved(DEFAULT, Group::Mark, Group::Show, false);
        assert_eq!(
            decode(&encode(order)),
            [Group::Mark, Group::Show, Group::Filter]
        );
        assert_eq!(moved(order, Group::Mark, Group::Filter, true), DEFAULT);
    }
}
