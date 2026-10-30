//! Fallible local listings: denied access is never mistaken for an empty folder.
use super::*;

#[derive(Clone, Debug)]
pub(super) struct ListingError {
    pub permission_denied: bool,
    pub message: String,
}

pub(super) fn unpack(result: std::io::Result<Vec<Entry>>) -> (Vec<Entry>, Option<ListingError>) {
    match result {
        Ok(entries) => (entries, None),
        Err(error) => {
            let denied = error.kind() == std::io::ErrorKind::PermissionDenied;
            let message = if denied {
                "macOS denied access to this folder. Allow Schnauz in Privacy & Security, then retry.".into()
            } else {
                format!("Couldn't read this folder: {error}")
            };
            (
                Vec::new(),
                Some(ListingError {
                    permission_denied: denied,
                    message,
                }),
            )
        }
    }
}

pub(super) fn error_panel(error: &ListingError, pane: usize, cx: &Context<Schnauz>) -> AnyElement {
    let t = theme();
    let button = |id, label: &str| {
        div()
            .id((id, pane))
            .px_3()
            .py_1()
            .rounded_md()
            .bg(rgb(t.hover))
            .cursor_pointer()
            .child(label.to_string())
    };
    div()
        .w_full()
        .min_w_0()
        .flex_1()
        .p_4()
        .flex()
        .flex_col()
        .items_start()
        .gap_3()
        .child(
            div()
                .w_full()
                .font_weight(FontWeight::SEMIBOLD)
                .child(if error.permission_denied {
                    "Folder access denied"
                } else {
                    "Folder unavailable"
                }),
        )
        .child(
            div()
                .w_full()
                .text_size(px(UI_FONT_SIZE))
                .child(error.message.clone()),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .child(button("retry-folder", "Retry").on_click(
                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if this.tab(pane).network.is_some()&&!this.tab(pane).network_connected {this.reconnect_network_tab(pane,cx);}else{this.reload_pane(pane,cx);}
                    }),
                ))
                .when(error.permission_denied, |d| {
                    d.child(
                        button("folder-access-settings", "File Access Settings…").on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                if let Err(error) = file_access::open_settings() {
                                    this.set_notice(error, cx);
                                }
                            }),
                        ),
                    )
                }),
        )
        .into_any_element()
}

/// Listing reads, including the first names-only pass, run on owned workers.
pub(super) fn read_entries(dir: &Path) -> std::io::Result<Vec<Entry>> {
    read_cancellable(dir, true, &AtomicBool::new(false))
}
#[cfg(any(test,feature="ui-checks"))]
pub(super) fn read_entries_fast(dir: &Path) -> std::io::Result<Vec<Entry>> {
    read_cancellable(dir, false, &AtomicBool::new(false))
}
pub(super) fn read_cancellable(dir: &Path, full: bool, cancel: &AtomicBool) -> std::io::Result<Vec<Entry>> {
    let mut entries=Vec::new();
    for item in fs::read_dir(dir)? {
        if cancel.load(Ordering::Acquire) { return Err(std::io::Error::new(std::io::ErrorKind::Interrupted,"Listing cancelled")); }
        let entry=item?;
        let name=entry.file_name().to_string_lossy().into_owned();
        let kind=entry.file_type()?;
        let md=if full || kind.is_symlink() {fs::metadata(entry.path()).ok()} else {None};
        let is_dir=if kind.is_symlink() {md.as_ref().is_some_and(|m|m.is_dir())} else {kind.is_dir()};
        entries.push(Entry {
            name,is_dir,
            size:md.as_ref().map_or(0,|m|m.len()),
            modified:md.as_ref().and_then(|m|m.modified().ok()),
            created:md.as_ref().and_then(|m|m.created().ok()),
            mode:md.as_ref().map(|m|{use std::os::unix::fs::PermissionsExt;m.permissions().mode()}),
            loaded:full,
            hidden_flag:md.as_ref().is_some_and(|m|{use std::os::macos::fs::MetadataExt;m.st_flags()&libc::UF_HIDDEN!=0}),
        });
    }
    if cancel.load(Ordering::Acquire) {return Err(std::io::Error::new(std::io::ErrorKind::Interrupted,"Listing cancelled"));}
    sort_default(&mut entries);Ok(entries)
}

pub(super) fn loading_panel() -> AnyElement {
    div().w_full().min_w_0().flex_1().p_4().text_color(rgb(theme().text_muted))
        .child("Loading folder…").into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distinguishes_empty_denied_and_failed_listings_and_allows_retry() {
        let folder = crate::tests::scratch("fallible-listing");
        let mut tab = Tab::fixture(folder.clone());
        assert!(tab.entries.is_empty() && tab.listing_error.is_none());
        tab.install_listing(Err(std::io::Error::from_raw_os_error(libc::EPERM)));
        assert!(tab.listing_error.as_ref().unwrap().permission_denied);
        fs::write(folder.join("visible.txt"), "available").unwrap();
        tab.install_listing(read_entries_fast(&folder));
        assert!(tab.listing_error.is_none());
        assert_eq!(tab.visible_entries().next().unwrap().name, "visible.txt");
        assert!(read_entries(&folder.join("visible.txt")).is_err());
        assert!(read_entries_fast(&folder.join("missing")).is_err());
        fs::remove_dir_all(folder).unwrap();
    }
    #[test]
    fn ancestor_navigation_clears_text_filter_and_rebuilds_hidden_results() {
        let root = crate::tests::scratch("ancestor-listing");
        fs::create_dir(root.join("child")).unwrap();
        fs::write(root.join("parent.txt"), "parent").unwrap();
        fs::write(root.join(".hidden"), "hidden").unwrap();
        let mut tab = Tab::fixture(root.join("child"));
        tab.find_query = Some("unmatched".into());
        tab.change_directory(root.clone());
        tab.install_listing(read_entries_fast(&root));
        assert!(tab.find_query.is_none());
        let visible: Vec<_> = tab.visible_entries().map(|e| e.name.as_str()).collect();
        assert_eq!(visible, ["child", "parent.txt"]);
        fs::remove_dir_all(root).unwrap();
    }
}
