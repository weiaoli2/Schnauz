//! Project-scoped tasks below the shared navigation/filter bars.
use super::*;
use todoist::{Global, Row};

fn button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex_none()
        .whitespace_nowrap()
        .px_2()
        .py_1()
        .rounded_md()
        .cursor_pointer()
        .text_size(px(UI_SMALL_FONT_SIZE))
        .text_color(rgb(theme().accent))
        .hover(|s| s.bg(rgb(theme().hover)))
        .child(label.into())
}
pub(super) fn settings_rows<T: 'static>(cx: &Context<T>) -> Vec<AnyElement> {
    let state = cx.global::<Global>();
    let mut row = div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .child(
            div().flex_1().min_w_0().child(
                state
                    .account
                    .as_ref()
                    .map_or_else(|| "Todoist".into(), |a| format!("Todoist · {}", a.name)),
            ),
        )
        .child(
            button(
                "todoist-connect",
                if state.busy {
                    "Working…"
                } else if state.account.is_some() {
                    "Reconnect…"
                } else {
                    "Connect…"
                },
            )
            .on_click(
                cx.listener(|_, _: &ClickEvent, _, cx| todoist::account_action("connect", cx)),
            ),
        );
    if state.account.is_some() {
        row = row.child(
            button("todoist-show-tasks", "Show tasks")
                .on_click(cx.listener(|_, _: &ClickEvent, _, cx| todoist::show_tasks(cx))),
        );
        row = row.child(button("todoist-disconnect", "Disconnect…").on_click(
            cx.listener(|_, _: &ClickEvent, _, cx| todoist::account_action("disconnect", cx)),
        ));
    }
    let mut rows = vec![row.into_any_element(), div().text_size(px(UI_SMALL_FONT_SIZE)).text_color(rgb(theme().text_muted))
        .child("Project names match Todoist. Existing projects are linked; newly created Schnauz projects are also created in Todoist when needed. API token stays in Keychain.").into_any_element()];
    if let Some(message) = &state.notice {
        rows.push(
            div()
                .text_size(px(UI_SMALL_FONT_SIZE))
                .child(message.clone())
                .into_any_element(),
        );
    }
    rows
}
impl Schnauz {
    pub(super) fn show_file_preview(&mut self, cx: &mut Context<Self>) {
        schnauz::toggle_inspector(cx);
        cx.notify();
    }
    pub(super) fn render_todoist(&self, width: f32, cx: &Context<Self>) -> AnyElement {
        let t = theme();
        let link = self.todoist_link(cx);
        let cache = link.as_ref().and_then(|l| {
            self.todoist
                .caches
                .get(&(l.account_id.clone(), l.project_id.clone()))
        });
        let local_name = self
            .projects
            .active
            .and_then(|id| self.projects.items.iter().find(|p| p.id == id))
            .map_or("Personal", |p| p.name.as_str());
        let name = cache
            .and_then(|c| c.data.as_ref())
            .map(|d| d.project.name.as_str())
            .or_else(|| link.as_ref().map(|l| l.project_name.as_str()))
            .unwrap_or(local_name)
            .to_string();
        let mut header = div()
            .w(px(width))
            .min_w_0()
            .flex_none()
            .flex()
            .items_center()
            .gap_1()
            .px_3()
            .h(px(36.))
            .border_b_1()
            .border_color(rgb(t.border))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Todoist"),
            );
        if link.is_some() {
            header = header
                .child(
                    button("todoist-relink", "Relink…")
                        .tooltip(tip("Find or create the matching Todoist project"))
                        .on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| this.link_todoist(true, cx)),
                        ),
                )
                .child(
                    button(
                        "todoist-refresh",
                        if cache.is_some_and(|c| c.busy) {
                            "Loading…"
                        } else {
                            "Refresh"
                        },
                    )
                    .on_click(
                        cx.listener(|this, _: &ClickEvent, _, cx| this.ensure_todoist(true, cx)),
                    ),
                );
        }
        header = header.child(
            button("todoist-close", "×")
                .tooltip(tip("Close Todoist tasks"))
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.todoist.open = false;
                    cx.notify();
                })),
        );
        let mut panel = div()
            .id("todoist-pane")
            .relative()
            .w(px(width))
            .min_w_0()
            .h_full()
            .min_h_0()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(rgb(t.bg))
            .text_color(rgb(t.text))
            .children(ui_checks::probe("todoist-pane"))
            .child(header);
        if cx.global::<Global>().account.is_none() {
            return panel
                .child(status(
                    width,
                    "Connect Todoist to see this project's tasks here.",
                ))
                .child(
                    button("todoist-panel-connect", "Connect Todoist…").on_click(cx.listener(
                        |_, _: &ClickEvent, _, cx| todoist::account_action("connect", cx),
                    )),
                )
                .into_any_element();
        }
        let Some(link) = link else {
            let pending = self
                .todoist
                .pending
                .contains(&format!("link:{:?}", self.projects.active));
            let message = if pending {
                "Finding the matching Todoist project…".to_string()
            } else {
                self.todoist.link_errors.get(&self.projects.active).cloned()
                    .unwrap_or_else(|| format!("No linked Todoist project yet. Schnauz uses the same project name: {local_name}."))
            };
            return panel
                .child(status(width, message))
                .child(
                    button(
                        "todoist-match",
                        if pending {
                            "Working…"
                        } else {
                            "Find or create matching project"
                        },
                    )
                    .on_click(
                        cx.listener(|this, _: &ClickEvent, _, cx| this.link_todoist(true, cx)),
                    ),
                )
                .into_any_element();
        };
        let project_id = link.project_id.clone();
        panel = panel.child(
            div()
                .w(px(width))
                .flex_none()
                .min_w_0()
                .px_4()
                .pt_3()
                .pb_2()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(18.))
                        .font_weight(FontWeight::BOLD)
                        .child(name.to_string()),
                )
                .child(
                    button("todoist-project-web", "Open in Todoist").on_click(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            if let Err(error) = open_target("project", &project_id) {
                                this.set_notice(error, cx);
                            }
                        },
                    )),
                ),
        );
        let key = (link.account_id.clone(), link.project_id.clone());
        let Some(cache) = self.todoist.caches.get(&key) else {
            return panel
                .child(status(width, "Loading tasks…"))
                .child(self.todoist_add_row(width, cx))
                .into_any_element();
        };
        if let Some(error) = &cache.error {
            panel = panel.child(status(
                width,
                format!(
                    "{error}{}",
                    if cache.data.is_some() {
                        " Showing cached tasks."
                    } else {
                        ""
                    }
                ),
            ));
        } else if cache.busy {
            panel = panel.child(
                div()
                    .w(px(width))
                    .flex_none()
                    .px_4()
                    .py_1()
                    .text_size(px(UI_SMALL_FONT_SIZE))
                    .child("Refreshing…"),
            );
        }
        if cache.data.is_none() {
            return panel
                .child(self.todoist_add_row(width, cx))
                .into_any_element();
        }
        // The add row is part of the list: directly below tasks, also when empty.
        let scroll = self.todoist.scroll.clone();
        let base = scroll.0.borrow().base_handle.clone();
        let count = cache.rows.len() + 1;
        panel
            .child(
                div()
                    .relative()
                    .w(px(width))
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .children(ui_checks::probe("todoist-list"))
                    .child(
                        uniform_list(
                            "todoist-tasks",
                            count,
                            cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                                this.todoist_rows(&key, range, width, cx)
                            }),
                        )
                        .size_full()
                        .w(px(width))
                        .track_scroll(scroll),
                    )
                    .children(static_scrollbar_thumb(&base)),
            )
            .into_any_element()
    }

    fn todoist_add_row(&self, width: f32, cx: &Context<Self>) -> AnyElement {
        let pending = self.todoist_link(cx).is_some_and(|link| {
            self.todoist
                .pending
                .contains(&format!("add:{}", link.project_id))
        });
        div()
            .id("todoist-add")
            .relative()
            .w(px(width))
            .h(px(60.))
            .flex_none()
            .min_w_0()
            .flex()
            .items_center()
            .gap_3()
            .px_4()
            .cursor_pointer()
            .text_color(rgb(theme().accent))
            .hover(|s| s.bg(rgb(theme().hover)))
            .children(ui_checks::probe("todoist-add"))
            .child(ui_icon(Icon::Plus, 18., rgb(theme().accent)))
            .child(if pending {
                "Adding task…"
            } else {
                "Add task"
            })
            .on_click(
                cx.listener(|this, _: &ClickEvent, _, cx| this.todoist_mutation("add", None, cx)),
            )
            .into_any_element()
    }

    fn todoist_rows(
        &self,
        key: &todoist::Key,
        range: std::ops::Range<usize>,
        width: f32,
        cx: &Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(cache) = self.todoist.caches.get(key) else {
            return Vec::new();
        };
        let Some(data) = cache.data.as_ref() else {
            return Vec::new();
        };
        let t = theme();
        range
            .filter_map(|row| {
                if row == cache.rows.len() {
                    return Some(self.todoist_add_row(width, cx));
                }
                Some(match cache.rows.get(row)? {
                    Row::Section(name) => div()
                        .id(("todoist-section", row))
                        .w(px(width))
                        .h(px(60.))
                        .min_w_0()
                        .flex()
                        .items_end()
                        .px_4()
                        .pb_2()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(name.clone())
                        .into_any_element(),
                    Row::Task(index, depth) => {
                        let task = &data.tasks[*index];
                        let busy = self
                            .todoist
                            .pending
                            .contains(&format!("complete:{}", task.id));
                        let task_id = task.id.clone();
                        let open_id = task.id.clone();
                        let color = match task.priority.unwrap_or(1) {
                            4 => 0xe25555,
                            3 => 0xdf9417,
                            2 => 0x4b89dc,
                            _ => t.border_strong,
                        };
                        let left = 16. + (*depth as f32).min(8.) * 12.;
                        let text_width = (width - left - 16. - 18. - 12.).max(1.);
                        let due = task.due.as_ref().map(due_label);
                        let secondary =
                            due.as_ref().map(|(label, _)| label.clone()).or_else(|| {
                                task.description.as_ref().filter(|s| !s.is_empty()).cloned()
                            });
                        let date_color = due.map_or(t.text_muted, |(_, color)| color);
                        div()
                            .id(SharedString::from(format!("todoist-task:{}", task.id)))
                            .relative()
                            .w(px(width))
                            .h(px(60.))
                            .min_w_0()
                            .flex()
                            .items_start()
                            .gap_3()
                            .pl(px(left))
                            .pr_4()
                            .py_2()
                            .children(ui_checks::probe("todoist-task"))
                            .hover(|s| s.bg(rgb(t.hover)))
                            .child(
                                div()
                                    .id("complete-task")
                                    .flex_none()
                                    .mt(px(3.))
                                    .size(px(18.))
                                    .rounded_full()
                                    .border_1()
                                    .border_color(rgb(color))
                                    .cursor_pointer()
                                    .when(busy, |s| s.bg(rgb(t.hover)))
                                    .tooltip(tip(if busy {
                                        "Completing…"
                                    } else {
                                        "Complete task in Todoist"
                                    }))
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        this.todoist_mutation("complete", Some(task_id.clone()), cx)
                                    })),
                            )
                            .child(
                                div()
                                    .id("task-details")
                                    .w(px(text_width))
                                    .min_w_0()
                                    .h_full()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .border_b_1()
                                    .border_color(rgb(t.border))
                                    .cursor_pointer()
                                    .child(
                                        div()
                                            .relative()
                                            .w(px(text_width))
                                            .truncate()
                                            .children(ui_checks::probe("todoist-task-title"))
                                            .child(task.content.clone()),
                                    )
                                    .children(secondary.map(|label| {
                                        div()
                                            .w(px(text_width))
                                            .min_w_0()
                                            .flex()
                                            .items_center()
                                            .gap_1()
                                            .text_size(px(UI_SMALL_FONT_SIZE))
                                            .text_color(rgb(date_color))
                                            .when(task.due.is_some(), |s| {
                                                s.child(ui_icon(
                                                    Icon::Calendar,
                                                    12.,
                                                    rgb(date_color),
                                                ))
                                            })
                                            .child(div().flex_1().min_w_0().truncate().child(label))
                                    }))
                                    .tooltip(tip(task
                                        .description
                                        .as_ref()
                                        .filter(|s| !s.is_empty())
                                        .cloned()
                                        .unwrap_or_else(|| "Open task in Todoist".into())))
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        if let Err(error) = open_target("task", &open_id) {
                                            this.set_notice(error, cx);
                                        }
                                    })),
                            )
                            .into_any_element()
                    }
                })
            })
            .collect()
    }
}
fn status(width: f32, message: impl Into<SharedString>) -> AnyElement {
    div()
        .relative()
        .w(px(width))
        .min_w_0()
        .flex_none()
        .p_4()
        .flex()
        .flex_col()
        .children(ui_checks::probe("todoist-status"))
        .child(
            div()
                .relative()
                .w(px((width - 32.).max(1.)))
                .min_w_0()
                .children(ui_checks::probe("todoist-status-text"))
                .child(message.into()),
        )
        .into_any_element()
}

fn due_label(due: &todoist::Due) -> (String, u32) {
    let value = due
        .datetime
        .as_ref()
        .or(due.date.as_ref())
        .cloned()
        .unwrap_or_default();
    let date = chrono::DateTime::parse_from_rfc3339(&value)
        .ok()
        .map(|d| d.with_timezone(&chrono::Local).date_naive())
        .or_else(|| chrono::NaiveDate::parse_from_str(&value, "%Y-%m-%d").ok());
    let today = chrono::Local::now().date_naive();
    let mut label = match date {
        Some(d) if d == today => "Today".into(),
        Some(d) if Some(d) == today.succ_opt() => "Tomorrow".into(),
        _ => value,
    };
    if due.is_recurring == Some(true) {
        label.push_str(" · recurring");
    }
    let color = match date {
        Some(d) if d < today => 0xc23a35,
        Some(d) if d == today => 0x168139,
        _ => theme().accent,
    };
    (label, color)
}

fn url_component(value: &str) -> String {
    let mut result = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            result.push(byte as char);
        } else {
            use std::fmt::Write;
            let _ = write!(result, "%{byte:02X}");
        }
    }
    result
}

fn target_urls(kind: &str, id: &str) -> (String, String) {
    let id = url_component(id);
    (
        format!("todoist://{kind}?id={id}"),
        format!("https://app.todoist.com/app/{kind}/{id}"),
    )
}

fn open_target(kind: &str, id: &str) -> Result<(), String> {
    let (native, web) = target_urls(kind, id);
    let workspace = NSWorkspace::sharedWorkspace();
    if let Some(url) = NSURL::URLWithString(&NSString::from_str(&native)) {
        if workspace.URLForApplicationToOpenURL(&url).is_some() && workspace.openURL(&url) {
            return Ok(());
        }
    }
    if let Some(url) = NSURL::URLWithString(&NSString::from_str(&web)) {
        if workspace.openURL(&url) {
            return Ok(());
        }
    }
    Err("Couldn't open Todoist. Check that Todoist or a browser is installed.".into())
}

#[cfg(test)]
mod link_tests {
    use super::*;
    #[test]
    fn native_and_browser_links_keep_opaque_ids_in_the_same_destination() {
        let (native, web) = target_urls("project", "6XGgm6PHrGgMpCFX");
        assert_eq!(native, "todoist://project?id=6XGgm6PHrGgMpCFX");
        assert_eq!(web, "https://app.todoist.com/app/project/6XGgm6PHrGgMpCFX");
        assert_eq!(
            target_urls("task", "id&other=value").0,
            "todoist://task?id=id%26other%3Dvalue"
        );
    }
}
