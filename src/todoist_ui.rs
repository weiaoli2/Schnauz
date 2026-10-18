//! Project-scoped tasks below the shared navigation/filter bars.
use super::*;
use todoist::{Global, Row};

fn button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
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
        if self.todoist.open {
            self.todoist.open = false;
            if !prefs().preview {
                schnauz::toggle_inspector(cx);
            }
        } else {
            schnauz::toggle_inspector(cx);
        }
        cx.notify();
    }
    pub(super) fn render_todoist(&self, cx: &Context<Self>) -> AnyElement {
        let t = theme();
        let local_name = self
            .projects
            .active
            .and_then(|id| self.projects.items.iter().find(|p| p.id == id))
            .map_or("Personal", |p| p.name.as_str());
        let link = self.todoist_link(cx);
        let cache = link.as_ref().and_then(|l| {
            self.todoist
                .caches
                .get(&(l.account_id.clone(), l.project_id.clone()))
        });
        let linked_name = cache
            .and_then(|c| c.data.as_ref())
            .map(|d| d.project.name.as_str())
            .or_else(|| link.as_ref().map(|l| l.project_name.as_str()))
            .unwrap_or(local_name);
        let mut header = div()
            .flex_none()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .h(px(38.))
            .border_b_1()
            .border_color(rgb(t.border))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format!("Todoist · {linked_name}")),
            );
        if link.is_some() {
            header = header.child(
                button("todoist-relink", "Relink…")
                    .tooltip(tip(
                        "Find or create the Todoist project with this Schnauz project's name",
                    ))
                    .on_click(
                        cx.listener(|this, _: &ClickEvent, _, cx| this.link_todoist(true, cx)),
                    ),
            );
            header = header.child(
                button(
                    "todoist-refresh",
                    if cache.is_some_and(|c| c.busy) {
                        "Loading…"
                    } else {
                        "Refresh"
                    },
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.ensure_todoist(true, cx))),
            );
        }
        header = header.child(
            button("todoist-close", "×")
                .tooltip(tip("Close Tasks and return to the file preview"))
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.todoist.open = false;
                    cx.notify();
                })),
        );
        let mut panel = div()
            .id("todoist-pane")
            .flex()
            .flex_col()
            .h_full()
            .min_h_0()
            .w_full()
            .bg(rgb(t.bg))
            .text_color(rgb(t.text))
            .child(header);
        let global = cx.global::<Global>();
        if global.account.is_none() {
            panel = panel.child(div().p_4().flex().flex_col().gap_3()
                .child("Connect Todoist to see this project's tasks here.")
                .child(button("todoist-panel-connect", if global.busy { "Connecting…" } else { "Connect Todoist…" })
                    .on_click(cx.listener(|_, _: &ClickEvent, _, cx| todoist::account_action("connect", cx))))
                .child(div().text_size(px(UI_SMALL_FONT_SIZE)).text_color(rgb(t.text_muted))
                    .child("Your API token is entered in a secure macOS dialog and stored in Keychain.")));
            if let Some(message) = &global.notice {
                panel = panel.child(div().px_4().child(message.clone()));
            }
            return panel.into_any_element();
        }
        let Some(link) = link else {
            let pending = self
                .todoist
                .pending
                .contains(&format!("link:{:?}", self.projects.active));
            return panel
                .child(
                    div()
                        .p_4()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(if pending {
                            "Finding the matching Todoist project…".to_string()
                        } else {
                            self.todoist
                                .link_errors
                                .get(&self.projects.active)
                                .cloned()
                                .unwrap_or_else(|| "No linked Todoist project yet.".into())
                        })
                        .child(
                            div()
                                .text_size(px(UI_SMALL_FONT_SIZE))
                                .text_color(rgb(t.text_muted))
                                .child(format!(
                                    "Schnauz uses the same project name: {local_name}."
                                )),
                        )
                        .child(
                            button(
                                "todoist-match",
                                if pending {
                                    "Working…"
                                } else {
                                    "Find or create matching project"
                                },
                            )
                            .on_click(cx.listener(
                                |this, _: &ClickEvent, _, cx| this.link_todoist(true, cx),
                            )),
                        ),
                )
                .into_any_element();
        };
        let key = (link.account_id.clone(), link.project_id.clone());
        let add_pending = self
            .todoist
            .pending
            .contains(&format!("add:{}", link.project_id));
        panel = panel.child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .justify_between()
                .px_3()
                .py_2()
                .child(
                    button(
                        "todoist-add",
                        if add_pending {
                            "Adding…"
                        } else {
                            "+ Add task…"
                        },
                    )
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.todoist_mutation("add", None, cx)
                    })),
                )
                .child(button("todoist-project-web", "Open in Todoist").on_click(
                    move |_, _, _| {
                        let _ = Command::new("open")
                            .arg(format!(
                                "https://app.todoist.com/app/project/{}",
                                url_component(&link.project_id)
                            ))
                            .spawn();
                    },
                )),
        );
        let Some(cache) = self.todoist.caches.get(&key) else {
            return panel
                .child(
                    div()
                        .p_4()
                        .text_color(rgb(t.text_muted))
                        .child("Loading tasks…"),
                )
                .into_any_element();
        };
        if let Some(error) = &cache.error {
            panel = panel.child(
                div()
                    .flex_none()
                    .px_3()
                    .py_2()
                    .text_size(px(UI_SMALL_FONT_SIZE))
                    .text_color(rgb(t.text_muted))
                    .child(format!(
                        "{error}{}",
                        if cache.data.is_some() {
                            " Showing cached tasks."
                        } else {
                            ""
                        }
                    )),
            );
        } else if cache.busy {
            panel = panel.child(
                div()
                    .flex_none()
                    .px_3()
                    .text_size(px(UI_SMALL_FONT_SIZE))
                    .text_color(rgb(t.text_muted))
                    .child("Refreshing…"),
            );
        }
        let Some(data) = cache.data.as_ref() else {
            return panel
                .child(
                    div()
                        .p_4()
                        .text_color(rgb(t.text_muted))
                        .child("Tasks will appear here when Todoist is available."),
                )
                .into_any_element();
        };
        if data.tasks.is_empty() {
            panel = panel.child(
                div()
                    .p_4()
                    .text_color(rgb(t.text_muted))
                    .child("No open tasks in this project."),
            );
        }
        let scroll = self.todoist.scroll.clone();
        let base = scroll.0.borrow().base_handle.clone();
        let count = cache.rows.len();
        panel
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(
                        uniform_list(
                            "todoist-tasks",
                            count,
                            cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                                this.todoist_rows(&key, range, cx)
                            }),
                        )
                        .size_full()
                        .track_scroll(scroll),
                    )
                    .children(static_scrollbar_thumb(&base)),
            )
            .into_any_element()
    }
    fn todoist_rows(
        &self,
        key: &todoist::Key,
        range: std::ops::Range<usize>,
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
                let item = cache.rows.get(row)?;
                Some(match item {
                    Row::Section(name) => div()
                        .id(("todoist-section", row))
                        .h(px(48.))
                        .flex()
                        .items_end()
                        .px_3()
                        .pt_3()
                        .pb_1()
                        .text_size(px(UI_SMALL_FONT_SIZE))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(t.text_muted))
                        .child(name.clone())
                        .into_any_element(),
                    Row::Task(index, depth) => {
                        let task = &data.tasks[*index];
                        let busy = self
                            .todoist
                            .pending
                            .contains(&format!("complete:{}", task.id));
                        let task_id = task.id.clone();
                        let web_id = task.id.clone();
                        let priority = match task.priority.unwrap_or(1) {
                            4 => 0xe25555,
                            3 => 0xdf9417,
                            2 => 0x4b89dc,
                            _ => t.text_muted,
                        };
                        let due = task
                            .due
                            .as_ref()
                            .and_then(|d| d.datetime.as_ref().or(d.date.as_ref()))
                            .cloned()
                            .unwrap_or_default();
                        let due = if task
                            .due
                            .as_ref()
                            .is_some_and(|d| d.is_recurring == Some(true))
                        {
                            format!("{due} · recurring")
                        } else {
                            due
                        };
                        div()
                            .id(SharedString::from(format!("todoist-task:{}", task.id)))
                            .h(px(48.))
                            .flex()
                            .items_start()
                            .gap_2()
                            .pl(px(12. + (*depth as f32).min(8.) * 12.))
                            .pr_3()
                            .py_2()
                            .hover(|s| s.bg(rgb(t.hover)))
                            .child(
                                div()
                                    .id("complete-task")
                                    .mt(px(2.))
                                    .flex_none()
                                    .size(px(15.))
                                    .rounded_full()
                                    .border_1()
                                    .border_color(rgb(priority))
                                    .cursor_pointer()
                                    .when(busy, |d| d.bg(rgb(t.hover)))
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
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .cursor_pointer()
                                    .child(div().truncate().child(task.content.clone()))
                                    .when(!due.is_empty(), |d| {
                                        d.child(
                                            div()
                                                .text_size(px(UI_SMALL_FONT_SIZE))
                                                .text_color(rgb(t.text_muted))
                                                .child(due),
                                        )
                                    })
                                    .tooltip(tip(task
                                        .description
                                        .as_ref()
                                        .filter(|s| !s.is_empty())
                                        .cloned()
                                        .unwrap_or_else(|| "Open task in Todoist".into())))
                                    .on_click(move |_, _, _| {
                                        let _ = Command::new("open")
                                            .arg(format!(
                                                "https://app.todoist.com/app/task/{}",
                                                url_component(&web_id)
                                            ))
                                            .spawn();
                                    }),
                            )
                            .into_any_element()
                    }
                })
            })
            .collect()
    }
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
