//! Cloud identity is a path boundary, not a substring of an arbitrary filename.
use super::*;

#[derive(Debug, PartialEq)]
pub(super) struct Location {
    pub root: PathBuf,
    pub name: String,
}

pub(super) fn identify(home: &Path, path: &Path) -> Option<Location> {
    let storage = home.join("Library/CloudStorage");
    if let Ok(relative) = path.strip_prefix(&storage) {
        let account = relative.components().next()?;
        if let std::path::Component::Normal(account) = account {
            let raw = account.to_string_lossy();
            let name = match raw
                .split('-')
                .next()
                .unwrap_or(&raw)
                .to_ascii_lowercase()
                .as_str()
            {
                "nextcloud" => "Nextcloud".into(),
                "googledrive" => "Google Drive".into(),
                "onedrive" => "OneDrive".into(),
                "icloud" => "iCloud".into(),
                _ => pretty_cloud_name(&raw),
            };
            return Some(Location {
                root: storage.join(account),
                name,
            });
        }
    }
    let root = home.join("Library/Mobile Documents/com~apple~CloudDocs");
    path.starts_with(&root).then(|| Location {
        root,
        name: "iCloud".into(),
    })
}

impl Shuffle {
    pub(super) fn cloud_breadcrumbs(
        &self,
        pane: usize,
        cx: &Context<Self>,
    ) -> Option<(AnyElement, Vec<AnyElement>)> {
        let tab = self.tab(pane);
        if tab.remote.is_some() {
            return None;
        }
        let location = identify(&home_dir(), &tab.current_dir)?;
        let relative = tab.current_dir.strip_prefix(&location.root).ok()?;
        let root = location.root.clone();
        let badge = div()
            .id(("cloud-root", pane))
            .flex_none()
            .px_2()
            .py(px(1.))
            .rounded_md()
            .bg(rgb(0x0078c9))
            .text_color(rgb(0xffffff))
            .text_size(px(UI_SMALL_FONT_SIZE))
            .cursor_pointer()
            .child(location.name)
            .tooltip(tip(format!("Cloud storage\n{}", root.display())))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.navigate_in(pane, root.clone(), cx)
            }))
            .into_any_element();
        let mut crumbs = Vec::new();
        let mut path = location.root;
        for (index, component) in relative.components().enumerate() {
            if let std::path::Component::Normal(name) = component {
                path.push(name);
                crumbs.push(breadcrumb_sep());
                crumbs.push(breadcrumb_seg(
                    pane * 4096 + index + 1,
                    pane,
                    name.to_string_lossy().into_owned(),
                    path.clone(),
                    true,
                    path == tab.current_dir,
                    cx,
                ));
            }
        }
        Some((badge, crumbs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cloud_root_and_relative_path_do_not_match_similar_names_or_another_home() {
        let home = Path::new("/Users/example");
        let folder = home.join("Library/CloudStorage/Nextcloud-server-account/Projekte/Design");
        let cloud = identify(home, &folder).unwrap();
        assert_eq!(cloud.name, "Nextcloud");
        assert_eq!(
            folder.strip_prefix(&cloud.root).unwrap(),
            Path::new("Projekte/Design")
        );
        let icloud = home.join("Library/Mobile Documents/com~apple~CloudDocs");
        assert_eq!(identify(home, &icloud).unwrap().name, "iCloud");
        for path in [
            home.join("Library/CloudStorage"),
            home.join("Library/CloudStorage-copy/Nextcloud-test"),
            home.join("Nextcloud-not-a-provider"),
            PathBuf::from("/Users/other/Library/CloudStorage/Nextcloud-test"),
        ] {
            assert!(identify(home, &path).is_none(), "{}", path.display());
        }
    }
}
