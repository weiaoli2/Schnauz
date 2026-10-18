use super::*;

pub(super) struct Form {
    pane: usize,
    folder: PathBuf,
    name: String,
    cursor: usize,
    selected: bool,
    error: Option<String>,
}

fn filename(input: &str) -> Result<String, String> {
    let name = input.trim();
    if name.is_empty() || name == "." || name == ".." || name.eq_ignore_ascii_case(".txt") {
        return Err("Enter a name for your text file.".into());
    }
    if name.chars().any(|c| c.is_control() || c == '/' || c == ':') {
        return Err("Use a file name without slashes, colons or line breaks.".into());
    }
    let name = if name.to_lowercase().ends_with(".txt") {
        name.to_owned()
    } else {
        format!("{name}.txt")
    };
    if name.len() > 255 {
        return Err("That name is too long.".into());
    }
    Ok(name)
}

fn create(folder: &Path, input: &str) -> Result<PathBuf, String> {
    let path = folder.join(filename(input)?);
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                "An item with that name already exists. Choose another name.".into()
            } else {
                format!("Couldn't create the file: {error}")
            }
        })?;
    Ok(path)
}

impl Schnauz {
    pub(super) fn open_text_file_dialog(
        &mut self,
        pane: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_context_menu(cx);
        self.rename = None;
        self.project_dialog = None;
        self.new_text_file = Some(Form {
            pane,
            folder: self.tab(pane).current_dir.clone(),
            name: String::new(),
            cursor: 0,
            selected: false,
            error: None,
        });
        window.focus(&self.focus);
        cx.notify();
    }

    fn create_text_file(&mut self, cx: &mut Context<Self>) {
        let Some(form) = self.new_text_file.as_ref() else {
            return;
        };
        let pane = form.pane;
        let folder = form.folder.clone();
        match create(&folder, &form.name) {
            Ok(path) => {
                self.new_text_file = None;
                self.push_undo(UndoOp::Created(vec![path.clone()]));
                if pane < self.panes.len() {
                    self.active_pane = pane;
                    if self.tab(pane).entry_filter == EntryFilter::Folders {
                        self.tab_mut(pane).set_entry_filter(EntryFilter::All);
                    }
                    if path
                        .file_name()
                        .is_some_and(|s| s.as_encoded_bytes().starts_with(b"."))
                    {
                        self.tab_mut(pane).show_hidden = true;
                    }
                    self.load_dir_in(pane, folder, cx);
                    self.reveal_in_pane(pane, path, cx);
                }
                self.save_session(cx);
                cx.notify();
            }
            Err(error) => {
                self.new_text_file.as_mut().unwrap().error = Some(error);
                cx.notify();
            }
        }
    }

    pub(super) fn handle_text_file_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = &ev.keystroke;
        match key.key.as_str() {
            "escape" => {
                self.new_text_file = None;
                cx.notify();
                return;
            }
            "enter" => {
                self.create_text_file(cx);
                return;
            }
            _ => {}
        }
        let form = self.new_text_file.as_mut().unwrap();
        match key.key.as_str() {
            "a" if key.modifiers.platform => form.selected = true,
            "left" | "right" => {
                form.cursor = if key.key == "left" {
                    form.cursor.saturating_sub(1)
                } else {
                    (form.cursor + 1).min(form.name.chars().count())
                };
                form.selected = false;
            }
            "backspace" | "delete" => {
                if form.selected {
                    form.name.clear();
                    form.cursor = 0;
                    form.selected = false;
                } else {
                    let len = form.name.chars().count();
                    let (start, end) = if key.key == "backspace" {
                        (form.cursor.saturating_sub(1), form.cursor)
                    } else {
                        (form.cursor, (form.cursor + 1).min(len))
                    };
                    let (a, b) = (char_byte(&form.name, start), char_byte(&form.name, end));
                    form.name.replace_range(a..b, "");
                    form.cursor = start;
                }
            }
            _ => {
                let text = if key.modifiers.platform && key.key == "v" {
                    cx.read_from_clipboard().and_then(|i| i.text())
                } else if !key.modifiers.platform && !key.modifiers.control {
                    key.key_char.clone()
                } else {
                    None
                };
                if let Some(text) = text {
                    if form.selected {
                        form.name.clear();
                        form.cursor = 0;
                        form.selected = false;
                    }
                    let text: String = text.chars().filter(|c| !c.is_control()).take(256).collect();
                    form.name
                        .insert_str(char_byte(&form.name, form.cursor), &text);
                    form.cursor += text.chars().count();
                }
            }
        }
        form.error = None;
        cx.notify();
    }

    pub(super) fn render_text_file_dialog(&self, cx: &Context<Self>) -> AnyElement {
        let t = theme();
        let form = self.new_text_file.as_ref().unwrap();
        let b = char_byte(&form.name, form.cursor);
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x11182766))
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.new_text_file = None;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .w(px(430.0))
                    .p_6()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .rounded_xl()
                    .bg(rgb(t.surface))
                    .border_1()
                    .border_color(rgb(t.border))
                    .shadow_lg()
                    .on_mouse_down(MouseButton::Left, |_, _, cx: &mut App| {
                        cx.stop_propagation()
                    })
                    .child(
                        div()
                            .text_size(px(19.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("New text file"),
                    )
                    .child(div().text_color(rgb(t.text_muted)).child(format!(
                        "Create an empty .txt file in {}",
                        path_label(&form.folder)
                    )))
                    .child(
                        div()
                            .h(px(42.0))
                            .px_3()
                            .flex()
                            .items_center()
                            .rounded_lg()
                            .bg(rgb(t.bg))
                            .border_1()
                            .border_color(rgb(t.accent))
                            .overflow_hidden()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .items_center()
                                    .whitespace_nowrap()
                                    .child(
                                        div()
                                            .flex_none()
                                            .when(form.selected, |d| {
                                                d.bg(Theme::alpha(t.accent, 0x44))
                                            })
                                            .child(form.name[..b].to_string()),
                                    )
                                    .child(div().w(px(1.0)).h(px(17.0)).bg(rgb(t.accent)))
                                    .child(
                                        div()
                                            .flex_none()
                                            .when(form.selected, |d| {
                                                d.bg(Theme::alpha(t.accent, 0x44))
                                            })
                                            .child(form.name[b..].to_string()),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .pl_2()
                                    .text_color(rgb(t.text_dim))
                                    .child(".txt"),
                            ),
                    )
                    .children(
                        form.error
                            .as_ref()
                            .map(|e| div().text_color(rgb(RENAME_ERR_COLOR)).child(e.clone())),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                schnauz::schnauz_button("text-cancel", "Cancel", false).on_click(
                                    cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.new_text_file = None;
                                        cx.notify();
                                    }),
                                ),
                            )
                            .child(
                                schnauz::schnauz_button("text-create", "Create file", true).on_click(
                                    cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.create_text_file(cx)
                                    }),
                                ),
                            ),
                    ),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn creates_empty_text_files_only_after_validation_and_never_overwrites() {
        let dir = crate::tests::scratch("new-text-file");
        let path = create(&dir, "  Notes 素材  ").unwrap();
        assert_eq!(path.file_name().unwrap(), "Notes 素材.txt");
        assert_eq!(fs::metadata(&path).unwrap().len(), 0);
        fs::write(&path, "Keep this content").unwrap();
        assert!(create(&dir, "Notes 素材.txt").is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "Keep this content");
        assert_eq!(filename("Notes.TXT").unwrap(), "Notes.TXT");
        for name in ["", ".", "..", ".txt", "../escape", "a/b", "a:b", "a\nb"] {
            assert!(filename(name).is_err(), "{name}");
        }
        assert!(filename(&"a".repeat(253)).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
