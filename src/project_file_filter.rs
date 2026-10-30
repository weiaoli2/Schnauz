//! Per-project file-ending exclusions; no filesystem writes or folder pruning.
use super::*;
use objc2_app_kit::{NSAlert, NSAppearanceCustomization, NSApplication, NSTextField};

// Scope standard edit shortcuts to this native field; the main GPUI file
// browser intentionally owns Cmd+C/X/V itself and has no global native Edit menu.
define_class!(
    #[unsafe(super(NSTextField))]
    #[thread_kind = MainThreadOnly]
    #[name = "SchnauzProjectFilterTextField"]
    struct EditorField;
    unsafe impl NSObjectProtocol for EditorField {}
    impl EditorField {
        #[unsafe(method(performKeyEquivalent:))]
        fn edit_shortcut(&self, event: &objc2_app_kit::NSEvent) -> objc2::runtime::Bool {
            use objc2_app_kit::NSEventModifierFlags as Flags;
            let flags=event.modifierFlags();
            if flags.contains(Flags::Command) && !flags.intersects(Flags::Control | Flags::Option) {
                if let (Some(key),Some(editor))=(event.charactersIgnoringModifiers(),self.currentEditor()) {
                    unsafe {
                        match key.to_string().to_lowercase().as_str() {
                            "a"=>{editor.selectAll(None);return true.into();},
                            "c"=>{editor.copy(None);return true.into();},
                            "x"=>{editor.cut(None);return true.into();},
                            "v"=>{editor.paste(None);return true.into();},
                            _=>{},
                        }
                    }
                }
            }
            unsafe { objc2::msg_send![super(self), performKeyEquivalent: event] }
        }
    }
);
fn editor_field(input: &str, mtm: objc2::MainThreadMarker) -> objc2::rc::Retained<NSTextField> {
    let frame = objc2_foundation::NSRect::new(
        objc2_foundation::NSPoint::new(0., 0.),
        objc2_foundation::NSSize::new(480., 28.),
    );
    let field: objc2::rc::Retained<EditorField> =
        unsafe { objc2::msg_send![EditorField::alloc(mtm), initWithFrame: frame] };
    field.setStringValue(&NSString::from_str(input));
    field.setBezeled(true);
    field.setDrawsBackground(true);
    field.setEditable(true);
    field.setSelectable(true);
    field.setFont(Some(&objc2_app_kit::NSFont::systemFontOfSize(
        UI_FONT_SIZE as f64,
    )));
    field.into_super()
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(crate) struct Rule {
    pub enabled: bool,
    pub extensions: Vec<String>,
}
impl Rule {
    pub fn parse(input: &str, enabled: bool) -> Result<Self, String> {
        if input.len() > 8192 {
            return Err("Please shorten the list of file endings.".into());
        }
        let mut extensions = Vec::new();
        for value in input
            .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
            .filter(|s| !s.is_empty())
        {
            let value = value
                .strip_prefix("*.")
                .map(|s| format!(".{s}"))
                .unwrap_or_else(|| value.to_string());
            let extension = if value.starts_with('.') {
                value.to_lowercase()
            } else {
                format!(".{}", value.to_lowercase())
            };
            if extension.len() > 128
                || extension[1..].split('.').any(str::is_empty)
                || extension
                    .chars()
                    .any(|c| c.is_control() || "/\\:*?[]{}".contains(c))
            {
                return Err("Enter file endings such as .blend1,.bak. Paths and wildcard patterns are not supported.".into());
            }
            if !extensions.contains(&extension) {
                extensions.push(extension);
            }
            if extensions.len() > 128 {
                return Err("Use at most 128 file endings.".into());
            }
        }
        Ok(Self {
            enabled,
            extensions,
        })
    }
    pub fn is_default(&self) -> bool {
        !self.enabled && self.extensions.is_empty()
    }

    pub fn active(&self) -> bool {
        self.enabled && !self.extensions.is_empty()
    }
    pub fn allows(&self, name: &str, is_folder: bool) -> bool {
        !self.active() || is_folder || {
            let name = name.to_lowercase();
            !self
                .extensions
                .iter()
                .any(|extension| name.ends_with(&extension.to_lowercase()))
        }
    }
}

impl Tab {
    pub(super) fn apply_project_file_filter(&mut self, rule: &Rule) -> bool {
        if self.project_file_filter == *rule {
            return false;
        }
        self.project_file_filter = rule.clone();
        self.recompute_results();
        self.retain_visible_selection();
        self.scroll_handle = UniformListScrollHandle::new();
        true
    }
}

impl Schnauz {
    /// In-memory only. Also covers fresh, restored and moved tabs before painting.
    pub(super) fn apply_project_file_filters(&mut self) -> bool {
        let rule = self.projects.sidebar().file_filter.clone();
        let mut changed = false;
        for pane in &mut self.panes {
            for tab in &mut pane.tabs {
                changed |= tab.apply_project_file_filter(&rule);
            }
        }
        for (id, session) in &mut self.project_sessions {
            let rule = self
                .projects
                .sidebar_for(*id)
                .map(|s| s.file_filter.clone())
                .unwrap_or_default();
            for pane in &mut session.panes {
                for tab in &mut pane.tabs {
                    tab.apply_project_file_filter(&rule);
                }
            }
        }
        changed
    }

    pub(super) fn save_project_file_filter(
        &mut self,
        project: Option<u64>,
        expected: &Rule,
        rule: Rule,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.observe_project_library(cx);
        let mut library = cx.global::<ProjectLibraryGlobal>().0.clone();
        let sidebar = library
            .sidebar_for_mut(project)
            .ok_or("This project no longer exists.")?;
        if &sidebar.file_filter != expected {
            return Err("This project's filter changed. Cancel and open Modify again.".into());
        }
        sidebar.file_filter = rule;
        self.persist_library(library, cx)?;
        if self.apply_project_file_filters() && self.palette_open {
            self.refresh_file_search(cx);
        }
        self.pending_click_rename = None;
        self.rename = None;
        self.marquee = None;
        self.drag_candidate = None;
        self.cancel_unused_previews();
        cx.notify();
        Ok(())
    }

    pub(super) fn set_project_file_filter(
        &mut self,
        project: Option<u64>,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        self.observe_project_library(cx);
        let Some(sidebar) = self.projects.sidebar_for(project) else {
            return;
        };
        let previous = sidebar.file_filter.clone();
        let mut rule = previous.clone();
        rule.enabled = enabled;
        if rule == previous {
            return;
        }
        if let Err(error) = self.save_project_file_filter(project, &previous, rule, cx) {
            self.set_notice(error, cx);
        }
    }

    pub(super) fn render_project_file_filter(&self, pane: usize, cx: &Context<Self>) -> AnyElement {
        let t = theme();
        let project = self.projects.active;
        let rule = &self.projects.sidebar().file_filter;
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(6.))
            .whitespace_nowrap()
            .child("Filter:")
            .child(
                div()
                    .flex_none()
                    .flex()
                    .p(px(1.))
                    .rounded_lg()
                    .bg(rgb(t.surface))
                    .children(
                        [
                            (true, "On", "project-filter-on"),
                            (false, "Off", "project-filter-off"),
                        ]
                        .into_iter()
                        .map(|(enabled, label, probe)| {
                            let selected = enabled == rule.enabled;
                            div()
                                .id((probe, pane))
                                .relative()
                                .flex_none()
                                .h(px(20.))
                                .px_2()
                                .rounded_md()
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .font_weight(if selected {
                                    FontWeight::SEMIBOLD
                                } else {
                                    FontWeight::NORMAL
                                })
                                .text_color(rgb(if selected { t.accent } else { t.text_muted }))
                                .when(selected, |s| s.bg(rgb(t.bg)).shadow_sm())
                                .hover(|s| s.bg(rgb(t.hover)))
                                .children((pane == 0).then(|| ui_checks::probe(probe)).flatten())
                                .child(label)
                                .tooltip(tip(if enabled {
                                    "Hide the file types configured for this project"
                                } else {
                                    "Show files without applying this project's exclusion list"
                                }))
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.set_project_file_filter(project, enabled, cx)
                                }))
                                .into_any_element()
                        }),
                    ),
            )
            .child(
                div()
                    .id(("project-filter-modify", pane))
                    .relative()
                    .flex_none()
                    .h(px(20.))
                    .px(px(6.))
                    .rounded_md()
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(t.hover)))
                    .children(
                        (pane == 0)
                            .then(|| ui_checks::probe("project-filter-modify"))
                            .flatten(),
                    )
                    .child("Modify…")
                    .tooltip(tip(format!(
                        "Edit this project's hidden file endings: {}",
                        rule.extensions.join(",")
                    )))
                    .on_click(
                        cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.open_project_file_filter(cx)
                        }),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn open_project_file_filter(&mut self, cx: &mut Context<Self>) {
        if self.project_filter_editor {
            return;
        }
        self.observe_project_library(cx);
        let project = self.projects.active;
        let previous = self.projects.sidebar().file_filter.clone();
        let name = project
            .and_then(|id| {
                self.projects
                    .items
                    .iter()
                    .find(|p| p.id == id)
                    .map(|p| p.name.clone())
            })
            .unwrap_or_else(|| "Personal".into());
        self.project_filter_editor = true;
        self.pending_click_rename = None;
        self.rename = None;
        self.context_menu = None;
        self.sidebar_menu = None;
        self.sort_menu = None;
        cx.notify();
        cx.spawn(async move|this,cx| {
            let mtm = objc2::MainThreadMarker::new().expect("Project filter editor on main thread");
            let app = NSApplication::sharedApplication(mtm);
            let mut input = previous.extensions.join(",");
            let mut error: Option<String> = None;
            loop {
                let alert = NSAlert::new(mtm);
                alert.setMessageText(&NSString::from_str(&format!("File type filter — {name}")));
                let help = "Hide these file endings in this project, e.g. .blend1,.bak. Separate entries with commas. Folders stay visible. The list is used while Filter is On.";
                alert.setInformativeText(&NSString::from_str(&error.as_ref().map(|e|format!("{e}\n\n{help}")).unwrap_or_else(||help.into())));
                let field = editor_field(&input, mtm);
                field.setFrame(objc2_foundation::NSRect::new(objc2_foundation::NSPoint::new(0.,0.),objc2_foundation::NSSize::new(480.,28.)));
                field.setPlaceholderString(Some(&NSString::from_str(".blend1,.bak")));
                alert.setAccessoryView(Some(&field));
                alert.addButtonWithTitle(&NSString::from_str("Save"));
                let cancel = alert.addButtonWithTitle(&NSString::from_str("Cancel"));
                cancel.setKeyEquivalent(&NSString::from_str("\u{1b}"));
                let appearance = objc2_app_kit::NSAppearance::appearanceNamed(unsafe { if theme().is_light() { objc2_app_kit::NSAppearanceNameAqua } else { objc2_app_kit::NSAppearanceNameDarkAqua } });
                alert.window().setAppearance(appearance.as_deref());
                alert.window().setInitialFirstResponder(Some(&field));
                alert.layout(); alert.window().center();
                #[cfg(feature="ui-checks")]
                if TEST_HIDDEN.with(|s|s.get()) {
                    alert.window().setFrameOrigin(objc2_foundation::NSPoint::new(-20000.,-20000.));
                    TEST_EDITOR.with(|slot|*slot.borrow_mut()=Some((alert.clone(),field.clone())));
                }
                let session = app.beginModalSessionForWindow(&alert.window());
                unsafe { field.selectText(None); }
                let response = loop {
                    let response = unsafe { app.runModalSession(session) };
                    if response != objc2_app_kit::NSModalResponseContinue { break response; }
                    cx.background_executor().timer(Duration::from_millis(16)).await;
                };
                unsafe { app.endModalSession(session); }
                input = field.stringValue().to_string();
                alert.window().orderOut(None);
                #[cfg(feature="ui-checks")]
                TEST_EDITOR.with(|slot|slot.borrow_mut().take());
                if response != objc2_app_kit::NSAlertFirstButtonReturn { break; }
                match Rule::parse(&input, previous.enabled) {
                    Err(message) => error = Some(message),
                    Ok(rule) => match this.update(cx,|this,cx|this.save_project_file_filter(project,&previous,rule,cx)) {
                        Ok(Ok(())) => break,
                        Ok(Err(message)) => error = Some(message),
                        Err(_) => break,
                    },
                }
            }
            let _ = this.update(cx,|this,cx|{ this.project_filter_editor=false; cx.notify(); });
        }).detach();
    }
}

#[cfg(feature = "ui-checks")]
thread_local! {
    pub(super) static TEST_HIDDEN: std::cell::Cell<bool> = const {std::cell::Cell::new(false)};
    pub(super) static TEST_EDITOR: RefCell<Option<(objc2::rc::Retained<NSAlert>,objc2::rc::Retained<NSTextField>)>> = const {RefCell::new(None)};
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn endings_are_literal_case_insensitive_and_folders_are_preserved() {
        let rule = Rule::parse(".blend1, BAK; *.bak\n.tar.gz, .备份", true).unwrap();
        assert_eq!(rule.extensions, [".blend1", ".bak", ".tar.gz", ".备份"]);
        for name in ["scene.BLEND1", "file.bak", "archive.TAR.GZ", "资料.备份"] {
            assert!(!rule.allows(name, false));
        }
        for name in ["scene.blend", "scene.blend11", "file.bak.txt", "backup"] {
            assert!(rule.allows(name, false));
        }
        assert!(rule.allows("versions.bak", true));
        assert!(Rule::parse("", true).unwrap().allows("file.bak", false));
        assert!(
            Rule::parse(".bak", false)
                .unwrap()
                .allows("file.bak", false)
        );
        for input in [
            ".",
            "..",
            ".tar..gz",
            "../private",
            "foo/bar",
            "*.b*k",
            ".a?",
            ".bak:",
        ] {
            assert!(Rule::parse(input, true).is_err(), "{input}");
        }
    }
    #[test]
    fn legacy_storage_defaults_to_off_and_each_project_retains_its_own_rule() {
        let mut library = projects::Library::default();
        let legacy: projects::Sidebar = serde_json::from_str("{}").unwrap();
        assert_eq!(legacy.file_filter, Rule::default());
        let dir = crate::tests::scratch("project-filter-store");
        let layout = projects::Layout {
            panes: vec![projects::SavedPane {
                tabs: vec![projects::SavedTab {
                    path: dir.clone(),
                    view: "list".into(),
                    sort: "none".into(),
                    ascending: true,
                    filter: EntryFilter::All,
                    show_hidden: false,
                    network: None,
                }],
                active: 0,
            }],
            active_pane: 0,
            split_ratio: 0.5,
        };
        let a = library.create("A", layout.clone()).unwrap();
        let b = library.create("B", layout).unwrap();
        library.sidebar_for_mut(Some(a)).unwrap().file_filter =
            Rule::parse(".blend1,.bak", true).unwrap();
        library.sidebar_for_mut(Some(b)).unwrap().file_filter = Rule::parse(".tmp", false).unwrap();
        let restored: projects::Library =
            serde_json::from_slice(&serde_json::to_vec(&library).unwrap()).unwrap();
        assert!(
            !restored
                .sidebar_for(Some(a))
                .unwrap()
                .file_filter
                .allows("scene.blend1", false)
        );
        assert!(
            restored
                .sidebar_for(Some(b))
                .unwrap()
                .file_filter
                .allows("scene.blend1", false)
        );
        assert_eq!(
            restored
                .sidebar_for(Some(b))
                .unwrap()
                .file_filter
                .extensions,
            [".tmp"]
        );
        assert_eq!(
            restored.sidebar_for(None).unwrap().file_filter,
            Rule::default()
        );
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn browser_visibility_selection_queries_and_reload_share_the_project_rule() {
        let dir = crate::tests::scratch("project-file-filter");
        fs::create_dir(dir.join("versions.bak")).unwrap();
        for name in ["scene.blend", "scene.blend1", "notes.bak", ".hidden.bak"] {
            fs::write(dir.join(name), "keep data").unwrap();
        }
        let mut tab = Tab::fixture(dir.clone());
        tab.selection = HashSet::from([dir.join("scene.blend"), dir.join("scene.blend1")]);
        tab.anchor = Some(dir.join("scene.blend1"));
        tab.apply_project_file_filter(&Rule::parse(".blend1,.bak", true).unwrap());
        let names = |t: &Tab| {
            t.visible_entries()
                .map(|e| e.name.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&tab), ["versions.bak", "scene.blend"]);
        assert_eq!(tab.selection, HashSet::from([dir.join("scene.blend")]));
        tab.show_hidden = true;
        tab.recompute_results();
        assert_eq!(tab.visible_count(), 2);
        tab.find_query = Some("scene".into());
        tab.recompute_results();
        assert_eq!(names(&tab), ["scene.blend"]);
        tab.find_query = None;
        tab.set_entry_filter(EntryFilter::Folders);
        assert_eq!(names(&tab), ["versions.bak"]);
        tab.set_entry_filter(EntryFilter::Files);
        assert_eq!(names(&tab), ["scene.blend"]);
        fs::write(dir.join("new.bak"), "keep too").unwrap();
        tab.install_listing(read_entries_fast(&dir));
        assert_eq!(names(&tab), ["scene.blend"]);
        tab.apply_project_file_filter(&Rule::parse(".blend1,.bak", false).unwrap());
        assert_eq!(tab.visible_count(), 5);
        assert_eq!(
            fs::read_to_string(dir.join("scene.blend1")).unwrap(),
            "keep data"
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
