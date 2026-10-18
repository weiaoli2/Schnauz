//! Cloud identity is a path boundary, not a substring of an arbitrary filename.
use super::*;

#[derive(Debug, PartialEq)]
pub(super) struct Location {
    pub root: PathBuf,
    pub name: String,
}

fn direct_identify(home: &Path, path: &Path) -> Option<Location> {
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

// Discover symlink routing once at startup. Never canonicalize/probe mounts or
// read desktop-client credentials while rendering a path or opening a menu.
static ALIASES: OnceLock<Vec<(PathBuf, PathBuf)>> = OnceLock::new();
fn discover_aliases(home: &Path) -> Vec<(PathBuf, PathBuf)> {
    fs::read_dir(home)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            if !entry.file_type().ok()?.is_symlink() {
                return None;
            }
            let target = fs::read_link(entry.path()).ok()?;
            let target = if target.is_absolute() {
                target
            } else {
                home.join(target)
            };
            direct_identify(home, &target)?;
            Some((entry.path(), target))
        })
        .collect()
}
pub(super) fn initialize(home: &Path) {
    ALIASES.get_or_init(|| discover_aliases(home));
}

fn resolve_alias(path: &Path, aliases: &[(PathBuf, PathBuf)]) -> PathBuf {
    aliases
        .iter()
        .filter_map(|(alias, target)| {
            path.strip_prefix(alias)
                .ok()
                .map(|relative| (alias, target.join(relative)))
        })
        .max_by_key(|(alias, _)| alias.components().count())
        .map(|(_, path)| path)
        .unwrap_or_else(|| path.into())
}
pub(super) fn physical_path(path: &Path) -> PathBuf {
    resolve_alias(path, ALIASES.get().map(Vec::as_slice).unwrap_or(&[]))
}
fn identify_with_aliases(
    home: &Path,
    path: &Path,
    aliases: &[(PathBuf, PathBuf)],
) -> Option<Location> {
    if let Some(location) = direct_identify(home, path) {
        return Some(location);
    }
    let (alias, target) = aliases.iter().find(|(alias, _)| path.starts_with(alias))?;
    let location = direct_identify(home, target)?;
    Some(Location {
        root: alias.clone(),
        name: location.name,
    })
}

pub(super) fn identify(home: &Path, path: &Path) -> Option<Location> {
    identify_with_aliases(home, path, ALIASES.get().map(Vec::as_slice).unwrap_or(&[]))
}
pub(super) fn display_root(root: &Path, path: &Path) -> PathBuf {
    let physical_root = physical_path(root);
    if let Some(aliases) = ALIASES.get() {
        for (alias, target) in aliases {
            if !path.starts_with(alias) {
                continue;
            }
            if let Ok(relative) = physical_root.strip_prefix(target) {
                return alias.join(relative);
            }
            if target.starts_with(&physical_root) {
                return alias.clone();
            }
        }
    }
    root.into()
}

impl Schnauz {
    pub(super) fn cloud_breadcrumbs(
        &self,
        pane: usize,
        cx: &Context<Self>,
    ) -> Option<(AnyElement, Vec<AnyElement>)> {
        let tab = self.tab(pane);
        if tab.remote.is_some() {
            return None;
        }
        let location = nextcloud::location(&tab.current_dir, cx)
            .or_else(|| identify(&home_dir(), &tab.current_dir))?;
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
    fn real_nextcloud_symlink_is_detected_without_reading_credentials_or_target_files() {
        let home = std::env::temp_dir().join(format!("schnauz-cloud-alias-{}", std::process::id()));
        fs::create_dir_all(&home).unwrap();
        let physical = home.join("Library/CloudStorage/Nextcloud-test");
        std::os::unix::fs::symlink(&physical, home.join("Nextcloud")).unwrap();
        let aliases = discover_aliases(&home);
        assert_eq!(aliases.len(), 1);
        let path = home.join("Nextcloud/Folder/file.png");
        let location = identify_with_aliases(&home, &path, &aliases).unwrap();
        assert_eq!(location.name, "Nextcloud");
        assert_eq!(location.root, home.join("Nextcloud"));
        assert_eq!(
            resolve_alias(&path, &aliases),
            physical.join("Folder/file.png")
        );
        assert!(
            identify_with_aliases(&home, &home.join("Nextcloud-copy/file.png"), &aliases).is_none()
        );
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn cloud_alias_paths_resolve_at_component_boundaries_without_io() {
        let aliases = vec![(
            PathBuf::from("/Users/example/Nextcloud"),
            PathBuf::from("/Users/example/Library/CloudStorage/Nextcloud-account"),
        )];
        assert_eq!(
            resolve_alias(
                Path::new("/Users/example/Nextcloud/Folder/file.png"),
                &aliases
            ),
            PathBuf::from("/Users/example/Library/CloudStorage/Nextcloud-account/Folder/file.png")
        );
        assert_eq!(
            resolve_alias(
                Path::new("/Users/example/Nextcloud-copy/file.png"),
                &aliases
            ),
            PathBuf::from("/Users/example/Nextcloud-copy/file.png")
        );
    }
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

#[cfg(feature = "ui-checks")]
pub(super) fn fixture_alias(alias: PathBuf, target: PathBuf) {
    ALIASES
        .set(vec![(alias, target)])
        .expect("isolated cloud aliases initialized once");
}
