//! Named bookmarks; project-scoped storage and explicit browser choices.
use super::*;
use projects::WebLink;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Scope {
    pub project: Option<u64>,
    pub common: bool,
}
#[derive(Clone)]
pub(super) struct Draft {
    scope: Scope,
    original: Option<WebLink>,
    link: WebLink,
}
#[derive(serde::Deserialize, Default)]
struct Response {
    #[serde(default)]
    cancelled: bool,
    link: Option<WebLink>,
}

fn validate(link: &WebLink) -> Result<(), String> {
    if link.name.trim().is_empty()
        || link.name.chars().any(char::is_control)
        || link.name.chars().count() > 200
    {
        return Err("Enter a weblink name (up to 200 characters).".into());
    }
    let url = NSURL::URLWithString(&NSString::from_str(&link.url));
    if link.url.len() > 8192
        || link
            .url
            .chars()
            .any(|c| c.is_control() || c.is_whitespace())
        || !url.as_ref().is_some_and(|u| {
            matches!(
                u.scheme()
                    .as_ref()
                    .map(|s| s.to_string().to_lowercase())
                    .as_deref(),
                Some("http" | "https")
            ) && u.host().is_some_and(|s| !s.is_empty())
        })
    {
        return Err("Enter a complete http:// or https:// web address.".into());
    }
    if link
        .browser
        .as_ref()
        .is_some_and(|p| !p.is_absolute() || p.extension().is_none_or(|s| s != "app"))
    {
        return Err("Choose a browser application or macOS Default.".into());
    }
    Ok(())
}
fn apply(library: &mut projects::Library, draft: &Draft) -> Result<(), String> {
    validate(&draft.link)?;
    let links = library
        .web_links_mut(draft.scope.project, draft.scope.common)
        .ok_or("This project no longer exists.")?;
    if let Some(original) = &draft.original {
        let link = links
            .iter_mut()
            .find(|l| l.id == original.id)
            .ok_or("This weblink was removed in another window.")?;
        if link != original {
            return Err("This weblink changed in another window. Cancel and edit it again.".into());
        }
        *link = draft.link.clone();
    } else {
        if links.iter().any(|l| l.id == draft.link.id) {
            return Err("This weblink was already saved.".into());
        }
        links.push(draft.link.clone());
    }
    Ok(())
}
fn launch_args(link: &WebLink) -> Result<Vec<OsString>, String> {
    validate(link)?;
    let mut args = Vec::new();
    if let Some(browser) = &link.browser {
        args.push("-a".into());
        args.push(browser.as_os_str().into());
    }
    args.push("--".into());
    args.push(link.url.as_str().into());
    Ok(args)
}

#[cfg(feature = "ui-checks")]
thread_local! { static OPEN_CAPTURE: RefCell<Option<Vec<Vec<OsString>>>> = const { RefCell::new(None) }; }

impl Schnauz {
    fn open_weblink(&mut self, link: WebLink, cx: &mut Context<Self>) {
        let args = match launch_args(&link) {
            Ok(args) => args,
            Err(error) => {
                self.set_notice(error, cx);
                return;
            }
        };
        #[cfg(feature = "ui-checks")]
        if OPEN_CAPTURE.with(|capture| {
            let mut capture = capture.borrow_mut();
            if let Some(commands) = capture.as_mut() {
                commands.push(args.clone());
                true
            } else {
                false
            }
        }) {
            return;
        }
        if link.browser.as_ref().is_some_and(|app| !app.is_dir()) {
            self.set_notice("This weblink’s browser is unavailable. Edit the weblink to choose another browser.".into(),cx);
            return;
        }
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { Command::new("open").args(args).output() })
                .await;
            let _ = this.update(cx, |this, cx| {
                if !result.is_ok_and(|output| output.status.success()) {
                    this.set_notice(
                        "Couldn’t open the weblink. Check its browser choice and URL.".into(),
                        cx,
                    );
                }
            });
        })
        .detach();
    }
    fn edit_weblink(&mut self, scope: Scope, original: Option<WebLink>, cx: &mut Context<Self>) {
        if self.weblink_editor_open {
            return;
        }
        let draft = self
            .weblink_draft
            .as_ref()
            .filter(|d| {
                d.scope == scope
                    && d.original.as_ref().map(|l| &l.id) == original.as_ref().map(|l| &l.id)
            })
            .cloned()
            .unwrap_or_else(|| Draft {
                scope,
                original: original.clone(),
                link: original.unwrap_or_else(|| WebLink {
                    id: todoist::request_id(),
                    name: String::new(),
                    url: "https://".into(),
                    browser: None,
                }),
            });
        self.weblink_draft = Some(draft.clone());
        self.weblink_editor_open = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let (send, receive) = futures::channel::oneshot::channel();
            let input = serde_json::to_vec(&draft.link).unwrap();
            let worker = std::thread::Builder::new()
                .name("schnauz-weblink-editor".into())
                .spawn(move || {
                    let response = (|| -> Option<Response> {
                        use std::io::Write;
                        let helper = std::env::current_exe().ok()?.parent()?.join("weblinkctl");
                        let mut child = Command::new(helper)
                            .stdin(Stdio::piped())
                            .stdout(Stdio::piped())
                            .stderr(Stdio::null())
                            .spawn()
                            .ok()?;
                        if child
                            .stdin
                            .take()
                            .is_none_or(|mut stdin| stdin.write_all(&input).is_err())
                        {
                            let _ = child.kill();
                            let _ = child.wait();
                            return None;
                        }
                        let output = child.wait_with_output().ok()?;
                        serde_json::from_slice(&output.stdout).ok()
                    })();
                    let _ = send.send(response);
                });
            let response = if worker.is_ok() {
                receive.await.ok().flatten()
            } else {
                None
            };
            let _ = this.update(cx, |this, cx| {
                this.weblink_editor_open = false;
                match response {
                    Some(response) if response.cancelled => this.weblink_draft = None,
                    Some(Response {
                        link: Some(mut link),
                        ..
                    }) => {
                        link.id = draft.link.id.clone();
                        let mut saved = draft;
                        saved.link = link;
                        this.weblink_draft = Some(saved.clone());
                        let mut candidate = this.projects.clone();
                        match apply(&mut candidate, &saved)
                            .and_then(|_| this.persist_library(candidate, cx))
                        {
                            Ok(()) => {
                                this.weblink_draft = None;
                                this.set_notice("Weblink saved".into(), cx);
                            }
                            Err(error) => this.set_notice(
                                format!(
                                    "{error} Your form is kept; use Add/Edit weblink to retry."
                                ),
                                cx,
                            ),
                        }
                    }
                    _ => this.set_notice(
                        "The weblink editor could not open. Your form is kept for retry.".into(),
                        cx,
                    ),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn remove_weblink(&mut self, scope: Scope, id: &str, cx: &mut Context<Self>) {
        let mut candidate = self.projects.clone();
        let Some(links) = candidate.web_links_mut(scope.project, scope.common) else {
            return;
        };
        links.retain(|l| l.id != id);
        if let Err(error) = self.persist_library(candidate, cx) {
            self.set_notice(error, cx);
        }
        cx.notify();
    }
    pub(super) fn web_link_section_menu(
        &mut self,
        common: bool,
        position: gpui::Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) {
        let scope = Scope { project: self.projects.active, common };
        self.context_menu = Some(ContextMenu {
            x: f32::from(position.x),
            y: f32::from(position.y),
            target: None,
            nodes: vec![mi("Add Weblink…", move |this, _, cx| {
                this.edit_weblink(scope, None, cx);
            })],
        });
        cx.stop_propagation();
        cx.notify();
    }

    pub(super) fn web_links_section(
        &self,
        rows: &mut Vec<AnyElement>,
        key: &mut usize,
        common: bool,
        cx: &Context<Self>,
    ) {
        let scope = Scope {
            project: self.projects.active,
            common,
        };
        if self.projects.web_links(scope.project, common).is_none_or(|links| links.is_empty()) {
            return;
        }
        rows.push(
            div()
                .px_4()
                .pt_1()
                .pb_0p5()
                .text_size(px(UI_SMALL_FONT_SIZE))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(theme().text))
                .child("Weblinks")
                .into_any_element(),
        );
        for link in self
            .projects
            .web_links(scope.project, common)
            .into_iter()
            .flatten()
        {
            *key += 1;
            let open = link.clone();
            let context = link.clone();
            let browser = link
                .browser
                .as_ref()
                .map(|p| {
                    p.file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                })
                .unwrap_or("macOS Default".into());
            let row = nav_item(
                link.name.clone(),
                format!("{}\n{}\nBrowser: {browser}", link.name, link.url),
                ui_icon(Icon::Globe, 16., rgb(theme().text)),
                None,
                *key,
                false,
                cx.listener(move |this, event: &ClickEvent, _, cx| {
                    if event.click_count() == 1 {
                        this.open_weblink(open.clone(), cx);
                    }
                }),
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    let edit = context.clone();
                    let open = context.clone();
                    let id = context.id.clone();
                    let url = context.url.clone();
                    this.context_menu = Some(ContextMenu {
                        x: f32::from(event.position.x),
                        y: f32::from(event.position.y),
                        target: None,
                        nodes: vec![
                            mi("Open weblink", move |this, _, cx| {
                                this.open_weblink(open.clone(), cx)
                            }),
                            mi("Edit weblink…", move |this, _, cx| {
                                this.edit_weblink(scope, Some(edit.clone()), cx)
                            }),
                            mi("Copy link", move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(url.clone()))
                            }),
                            MenuNode::Sep,
                            mi("Remove weblink", move |this, _, cx| {
                                this.remove_weblink(scope, &id, cx)
                            }),
                        ],
                    });
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .into_any_element();
            rows.push(self.sortable_row(
                row,
                project_sidebar::SidebarOrder::WebLink {
                    project: scope.project,
                    common,
                    id: link.id.clone(),
                },
                link.name.clone(),
                format!("weblink-order-{common}-{}", link.id),
                cx,
            ));
        }

    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn link() -> WebLink {
        WebLink {
            id: "one".into(),
            name: "Example".into(),
            url: "https://example.org/a?q=hello&next=%22quoted%22#part".into(),
            browser: None,
        }
    }
    #[test]
    fn named_bookmarks_keep_browser_choices_scoped_and_preserve_legacy_data() {
        let mut library = projects::Library::default();
        let scope = Scope {
            project: None,
            common: false,
        };
        let draft = Draft {
            scope,
            original: None,
            link: link(),
        };
        apply(&mut library, &draft).unwrap();
        assert!(library.common_web_links.is_empty());
        let mut edited = draft.clone();
        edited.original = Some(draft.link.clone());
        edited.link.name = "Other name".into();
        edited.link.browser = Some("/Applications/Fixture Browser.app".into());
        apply(&mut library, &edited).unwrap();
        assert!(
            apply(&mut library, &edited).is_err(),
            "Stale editor cannot overwrite a newer value"
        );
        let restored: projects::Library =
            serde_json::from_str(&serde_json::to_string(&library).unwrap()).unwrap();
        assert_eq!(restored, library);
        assert_eq!(
            launch_args(&edited.link).unwrap(),
            vec![
                OsString::from("-a"),
                OsString::from("/Applications/Fixture Browser.app"),
                OsString::from("--"),
                OsString::from(edited.link.url)
            ]
        );
        let old: projects::Sidebar =
            serde_json::from_str(r#"{"shortcuts":[],"servers":[]}"#).unwrap();
        assert!(old.web_links.is_empty());
        for url in [
            "file:///etc/hosts",
            "javascript:alert(1)",
            "https://",
            "https://example.org/\n--other",
        ] {
            let mut bad = link();
            bad.url = url.into();
            assert!(validate(&bad).is_err());
        }
    }
}

#[cfg(feature = "ui-checks")]
pub(super) fn check(
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
    let original = window
        .update(cx, |f, _, cx| {
            f.browser.update(cx, |b, cx| {
                let original = b.projects.clone();
                b.projects.common_web_links = vec![
                    WebLink {
                        id: "ui-one".into(),
                        name: "Example bookmark".into(),
                        url: "https://example.org/one?q=hello&b=%22quoted%22".into(),
                        browser: Some("/Applications/Fixture Browser.app".into()),
                    },
                    WebLink {
                        id: "ui-two".into(),
                        name: "Default browser bookmark".into(),
                        url: "https://example.org/two".into(),
                        browser: None,
                    },
                ];
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
    let (first, second) = window
        .update(cx, |f, _, cx| {
            let b = f.browser.read(cx);
            let bounds = b.sidebar_order_bounds.borrow();
            (
                *bounds.get("weblink-order-true-ui-one").unwrap(),
                *bounds.get("weblink-order-true-ui-two").unwrap(),
            )
        })
        .unwrap();
    let (ptr, height) = cx
        .update_window(*window, |_, w, _| {
            (
                ns_view_ptr(w).unwrap() as usize,
                f32::from(w.viewport_size().height),
            )
        })
        .unwrap();
    OPEN_CAPTURE.with(|capture| *capture.borrow_mut() = Some(Vec::new()));
    cx.spawn(async move|cx| {
        use objc2_app_kit::{NSEvent,NSEventType,NSEventModifierFlags,NSView};
        cx.background_executor().timer(Duration::from_millis(10)).await;
        let view=unsafe{&*(ptr as *const NSView)};
        let number:isize=unsafe{objc2::msg_send![&*view.window().unwrap(),windowNumber]};
        let destination=point(second.center().x,second.bottom_right().y-px(3.));
        for (kind,position) in [
            (NSEventType::MouseMoved,first.center()),
            (NSEventType::LeftMouseDown,first.center()),
            (NSEventType::LeftMouseUp,first.center()),
            (NSEventType::LeftMouseDown,second.center()),
            (NSEventType::LeftMouseUp,second.center()),
            (NSEventType::LeftMouseDown,first.center()),
            (NSEventType::LeftMouseDragged,first.center()+point(px(0.),px(8.))),
            (NSEventType::LeftMouseDragged,destination),
            (NSEventType::LeftMouseUp,destination),
        ] {
            let event=NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(kind,objc2_foundation::NSPoint::new(f32::from(position.x) as f64,(height-f32::from(position.y)) as f64),NSEventModifierFlags::empty(),0.,number,None,0,1,0.).unwrap();
            match kind {
                NSEventType::LeftMouseDown=>view.mouseDown(&event),
                NSEventType::LeftMouseUp=>view.mouseUp(&event),
                NSEventType::LeftMouseDragged=>view.mouseDragged(&event),
                _=>view.mouseMoved(&event),
            }
            cx.update(|cx|paint(window,cx)).unwrap();
            cx.background_executor().timer(Duration::from_millis(15)).await;
        }
        cx.update(|cx| {
            OPEN_CAPTURE.with(|capture| {
                let commands=capture.borrow_mut().take().unwrap();
                assert_eq!(commands.len(),2,"Click opens once; drag never opens");
                assert_eq!(commands[0][0],OsString::from("-a"));
                assert_eq!(commands[0][1],OsString::from("/Applications/Fixture Browser.app"));
                assert_eq!(commands[1],vec![OsString::from("--"),OsString::from("https://example.org/two")]);
            });
            window.update(cx,|f,_,cx|f.browser.update(cx,|b,cx| {
                assert_eq!(b.projects.common_web_links[0].id,"ui-two");
                assert_eq!(b.projects.common_web_links[1].id,"ui-one");
                assert_eq!(b.active_tab().id,original.1);
                assert_eq!(b.active_tab().current_dir,original.2);
                let saved=projects::Library::load(&config_file("projects.json").unwrap()).unwrap();
                assert_eq!(saved.common_web_links,b.projects.common_web_links);
                b.persist_library(original.0,cx).unwrap();cx.notify();
            })).unwrap();
            paint(window,cx);
            println!("PASS actual weblink row clicks select default/individual browser launch arguments, drag ordering persists, drag never launches and browser tab/path stays unchanged; external opening intercepted");
            done(cx);
        }).unwrap();
    }).detach();
}
