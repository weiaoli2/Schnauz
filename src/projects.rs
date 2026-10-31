//! Named, local-folder projects. Storage is independent of the GPUI views.

use crate::entry_filter::EntryFilter;
use crate::SftpServer;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct SavedTab {
    pub path: PathBuf,
    pub view: String,
    pub sort: String,
    pub ascending: bool,
    #[serde(default)]
    pub filter: EntryFilter,
    #[serde(default)]
    pub show_hidden: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<crate::network::Location>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct SavedPane {
    pub tabs: Vec<SavedTab>,
    pub active: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct Layout {
    pub panes: Vec<SavedPane>,
    pub active_pane: usize,
    pub split_ratio: f32,
}

impl Layout {
    /// Keep existing folders and preserve the selected tab/pane when earlier
    /// entries disappeared. No directory or file is ever created by restoring.
    pub fn available(&self) -> Option<Self> {
        let mut panes = Vec::new();
        let mut active_pane = 0;
        for (pane_index, pane) in self.panes.iter().take(2).enumerate() {
            let mut tabs = Vec::new();
            let mut active = 0;
            for (tab_index, tab) in pane.tabs.iter().enumerate() {
                if tab.path.is_absolute() && (tab.network.is_some() || tab.path.is_dir()) {
                    if tab_index <= pane.active {
                        active = tabs.len();
                    }
                    tabs.push(tab.clone());
                }
            }
            if !tabs.is_empty() {
                if pane_index <= self.active_pane {
                    active_pane = panes.len();
                }
                panes.push(SavedPane { tabs, active });
            }
        }
        if panes.is_empty() {
            return None;
        }
        Some(Self {
            panes,
            active_pane,
            split_ratio: if self.split_ratio.is_finite() {
                self.split_ratio.clamp(0.2, 0.8)
            } else {
                0.5
            },
        })
    }

    pub fn tab_count(&self) -> usize {
        self.panes.iter().map(|pane| pane.tabs.len()).sum()
    }
}

/// Nonsecret binding to the corresponding Todoist project.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct TodoistLink {
    pub account_id: String,
    pub project_id: String,
    pub project_name: String,
}

/// The two kinds of project favourites, independent of their storage provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Section {
    #[serde(rename = "folder")]
    Folders,
    #[serde(rename = "file")]
    Files,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct Shortcut {
    pub path: PathBuf,
    pub kind: Section,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub alias: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub add_on: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<SftpServer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<crate::network::Location>,
}
impl Shortcut {
    pub fn local(path: PathBuf, kind: Section) -> Self {
        Self {
            path,
            kind,
            alias: String::new(),
            add_on: String::new(),
            server: None,
            network: None,
        }
    }

    pub fn display_name(&self) -> String {
        let name = if self.alias.trim().is_empty() {
            crate::path_label(&self.path)
        } else {
            self.alias.trim().to_string()
        };
        if self.add_on.trim().is_empty() {
            name
        } else {
            format!("{name} ({})", self.add_on.trim())
        }
    }

    pub fn is_home(&self) -> bool {
        self.server.is_none() && self.network.is_none() && self.kind == Section::Folders && self.path == crate::home_dir()
    }

    pub fn same_place(&self, other: &Self) -> bool {
        match (&self.network,&other.network) {
            (Some(a),Some(b))=>return a.same_place(b),
            (None,None)=>{},
            _=>return false,
        }

        self.path == other.path
            && match (&self.server, &other.server) {
                (None, None) => true,
                (Some(a), Some(b)) => same_server(a, b),
                _ => false,
            }
    }
}

pub(crate) fn same_server(a: &SftpServer, b: &SftpServer) -> bool {
    a.host.trim().eq_ignore_ascii_case(b.host.trim())
        && a.user.trim() == b.user.trim()
        && a.port == b.port
}

/// A named HTTP(S) bookmark with an optional per-link browser application.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct WebLink {
    pub id: String,
    pub name: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser: Option<PathBuf>,
}

/// Nonsecret project shortcuts and account routing. Connections remain in Settings.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub(crate) struct RecentSearch {
    pub query: String,
    pub content: bool,
    pub drive: bool,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub(crate) struct Sidebar {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub recent_searches: Vec<RecentSearch>,
    #[serde(skip_serializing_if = "crate::project_file_filter::Rule::is_default")]
    pub file_filter: crate::project_file_filter::Rule,
    #[serde(skip_serializing_if = "crate::version_mark::Rule::is_default")]
    pub version_mark: crate::version_mark::Rule,
    pub shortcuts: Vec<Shortcut>,
    pub web_links: Vec<WebLink>,
    pub servers: Vec<SftpServer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub todoist: Option<TodoistLink>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub todoist_create_request: Option<String>,
}
impl Sidebar {
    pub fn remember_search(&mut self, mut search: RecentSearch) -> bool {
        search.query = search.query.trim().to_string();
        if search.query.is_empty() || self.recent_searches.first() == Some(&search) { return false; }
        self.recent_searches.retain(|old| old != &search);
        self.recent_searches.insert(0, search);
        self.recent_searches.truncate(10);
        true
    }

    pub fn contains(&self, shortcut: &Shortcut) -> bool {
        shortcut.is_home() || self.shortcuts.iter().any(|s| s.same_place(shortcut))
    }

    pub fn set_shortcut(&mut self, shortcut: Shortcut, add: bool) {
        update_shortcuts(&mut self.shortcuts, shortcut, add);
    }

    pub fn set_path(&mut self, section: Section, path: PathBuf, add: bool) {
        self.set_shortcut(Shortcut::local(path, section), add);
    }
}

fn update_shortcuts(shortcuts: &mut Vec<Shortcut>, mut shortcut: Shortcut, add: bool) {
    // Home is shown once in every project without storing a duplicate shortcut.
    if shortcut.is_home() {
        return;
    }
    // Favourites describe destinations, never ownership of a live SSH session.
    if let Some(server) = &mut shortcut.server {
        server.control_path = None;
    }
    if let Some(index) = shortcuts.iter().position(|s| s.same_place(&shortcut)) {
        if add {
            // Re-adding a destination must preserve its label.
            shortcut.alias = shortcuts[index].alias.clone();
            shortcut.add_on = shortcuts[index].add_on.clone();
            shortcuts[index] = shortcut;
        } else {
            shortcuts.remove(index);
        }
    } else if add {
        shortcuts.push(shortcut);
    }
}

/// Validate the captured reference, then label the same destination everywhere.
pub(crate) fn edit_shortcut_label(
    library: &mut Library, project: Option<u64>, common: bool,
    original: &Shortcut, alias: &str, add_on: &str,
) -> Result<(), String> {
    if original.is_home() {
        return Err("Home cannot have an alias.".into());
    }
    let alias = alias.trim();
    let add_on = add_on.trim();
    if [alias, add_on].iter().any(|s| s.chars().count() > 200 || s.chars().any(char::is_control)) {
        return Err("Use up to 200 characters per field, without line breaks.".into());
    }
    let shortcuts = if common {
        &mut library.common_shortcuts
    } else {
        &mut library.sidebar_for_mut(project).ok_or("This project no longer exists.")?.shortcuts
    };
    let item = shortcuts.iter_mut().find(|s| s.same_place(original) && s.kind == original.kind)
        .ok_or("This favourite was removed. Add it again before editing its label.")?;
    if item.alias != original.alias || item.add_on != original.add_on {
        return Err("This label changed in another window. Open the editor again.".into());
    }
    for item in library.common_shortcuts.iter_mut()
        .chain(library.personal_sidebar.shortcuts.iter_mut())
        .chain(library.items.iter_mut().flat_map(|p| p.sidebar.shortcuts.iter_mut()))
        .filter(|s| s.same_place(original))
    {
        item.alias = alias.into();
        item.add_on = add_on.into();
    }
    Ok(())
}

/// Move an existing item relative to another without changing its identity.
pub(crate) fn reorder<T>(items: &mut Vec<T>, from: usize, target: usize, after: bool) -> bool {
    if from == target {
        return false;
    }
    let insertion = target + usize::from(after);
    let insertion = insertion - usize::from(from < insertion);
    if insertion == from {
        return false;
    }
    let item = items.remove(from);
    items.insert(insertion, item);
    true
}

pub(crate) fn reorder_shortcuts(
    items: &mut Vec<Shortcut>,
    source: &Shortcut,
    target: &Shortcut,
    after: bool,
) -> bool {
    if source.is_home() || target.is_home() || source.kind != target.kind {
        return false;
    }
    let Some(from) = items
        .iter()
        .position(|s| s.kind == source.kind && s.same_place(source))
    else {
        return false;
    };
    let Some(to) = items
        .iter()
        .position(|s| s.kind == target.kind && s.same_place(target))
    else {
        return false;
    };
    reorder(items, from, to, after)
}

// Read old sidebar sections once. New saves contain only the unified shortcuts.
// Metadata is needed only for legacy paths; new shortcuts keep their kind and
// SSH identity, so rendering and subsequent loads perform no filesystem checks.
impl<'de> Deserialize<'de> for Sidebar {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Default, Deserialize)]
        #[serde(default)]
        struct Stored {
            recent_searches: Vec<RecentSearch>,
            file_filter: crate::project_file_filter::Rule,
            version_mark: crate::version_mark::Rule,
            shortcuts: Vec<Shortcut>,
            web_links: Vec<WebLink>,
            locations: Vec<PathBuf>,
            cloud: Vec<PathBuf>,
            server_folders: Vec<PathBuf>,
            tagged: Vec<PathBuf>,
            servers: Vec<SftpServer>,
            todoist: Option<TodoistLink>,
            todoist_create_request: Option<String>,
        }
        let old = Stored::deserialize(deserializer)?;
        let mut sidebar = Self {
            recent_searches: Vec::new(),
            file_filter: old.file_filter,
            version_mark: old.version_mark,
            shortcuts: Vec::new(),
            web_links: old.web_links,
            servers: old.servers,
            todoist: old.todoist,
            todoist_create_request: old.todoist_create_request,
        };
        for search in old.recent_searches.into_iter().rev() {
            sidebar.remember_search(search);
        }
        for shortcut in old.shortcuts {
            sidebar.set_shortcut(shortcut, true);
        }
        for (paths, fallback) in [
            (old.locations, Section::Folders),
            (old.cloud, Section::Folders),
            (old.server_folders, Section::Folders),
            (old.tagged, Section::Files),
        ] {
            for path in paths {
                let kind = legacy_kind(&path, fallback);
                if !sidebar.contains(&Shortcut::local(path.clone(), kind)) {
                    sidebar.set_path(kind, path, true);
                }
            }
        }
        Ok(sidebar)
    }
}

fn legacy_kind(path: &Path, fallback: Section) -> Section {
    fs::metadata(path)
        .map(|m| {
            if crate::is_browsable_folder(path, m.is_dir()) {
                Section::Folders
            } else {
                Section::Files
            }
        })
        .unwrap_or(fallback)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct Project {
    pub id: u64,
    pub name: String,
    pub layout: Layout,
    #[serde(default)]
    pub sidebar: Sidebar,
}

fn default_common_shortcuts() -> Vec<Shortcut> {
    ["Desktop", "Downloads"]
        .into_iter()
        .map(|name| Shortcut::local(crate::home_dir().join(name), Section::Folders))
        .collect()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct Library {
    version: u32,
    pub items: Vec<Project>,
    #[serde(default)]
    pub active: Option<u64>,
    #[serde(default)]
    pub personal_layout: Option<Layout>,
    #[serde(default)]
    pub personal_sidebar: Sidebar,
    #[serde(default = "default_common_shortcuts")]
    pub common_shortcuts: Vec<Shortcut>,
    #[serde(default)]
    pub common_web_links: Vec<WebLink>,
    #[serde(default)]
    sidebar_migrated: bool,
}

impl Default for Library {
    fn default() -> Self {
        Self {
            version: 1,
            items: Vec::new(),
            active: None,
            personal_layout: None,
            personal_sidebar: Sidebar::default(),
            common_shortcuts: default_common_shortcuts(),
            common_web_links: Vec::new(),
            sidebar_migrated: false,
        }
    }
}

impl Library {
    pub fn web_links(&self, project: Option<u64>, common: bool) -> Option<&Vec<WebLink>> {
        if common { Some(&self.common_web_links) } else { self.sidebar_for(project).map(|s| &s.web_links) }
    }
    pub fn web_links_mut(&mut self, project: Option<u64>, common: bool) -> Option<&mut Vec<WebLink>> {
        if common { Some(&mut self.common_web_links) } else { self.sidebar_for_mut(project).map(|s| &mut s.web_links) }
    }
    pub fn reorder_project(&mut self, source: u64, target: u64, after: bool) -> bool {
        let Some(from) = self.items.iter().position(|p| p.id == source) else {
            return false;
        };
        let Some(to) = self.items.iter().position(|p| p.id == target) else {
            return false;
        };
        reorder(&mut self.items, from, to, after)
    }

    pub fn set_common_shortcut(&mut self, shortcut: Shortcut, add: bool) {
        update_shortcuts(&mut self.common_shortcuts, shortcut, add);
    }
    pub fn common_contains(&self, shortcut: &Shortcut) -> bool {
        shortcut.is_home() || self.common_shortcuts.iter().any(|s| s.same_place(shortcut))
    }
    pub fn load(path: &Path) -> io::Result<Self> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(error),
        };
        let mut stored: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if let Some(root) = stored.as_object_mut() {
            merge_old_favorites(root, "personal_favorites", "personal_sidebar")?;
            if let Some(items) = root.get_mut("items").and_then(|v| v.as_array_mut()) {
                for project in items.iter_mut().filter_map(|v| v.as_object_mut()) {
                    merge_old_favorites(project, "favorites", "sidebar")?;
                }
            }
        }
        let mut library: Self = serde_json::from_value(stored)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let mut ids = HashSet::new();
        let mut names = HashSet::new();
        if library.version != 1
            || library.items.iter().any(|project| {
                project.id == 0
                    || !ids.insert(project.id)
                    || valid_name(&project.name).is_err()
                    || !names.insert(project.name.trim().to_lowercase())
            })
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid project library",
            ));
        }
        let common = std::mem::take(&mut library.common_shortcuts);
        for shortcut in common {
            library.set_common_shortcut(shortcut, true);
        }
        library.active = library
            .active
            .filter(|id| library.items.iter().any(|p| p.id == *id));
        Ok(library)
    }

    pub fn load_migrating(path: &Path, legacy: &Path, favorites: Vec<PathBuf>) -> io::Result<Self> {
        if path.exists() {
            return Self::load(path);
        }
        let mut library = Self::load(legacy)?;
        for path in favorites {
            library
                .personal_sidebar
                .set_path(legacy_kind(&path, Section::Folders), path, true);
        }
        Ok(library)
    }

    pub fn migrate_sidebar(&mut self, servers: Vec<SftpServer>) -> bool {
        if self.sidebar_migrated {
            return false;
        }
        self.personal_sidebar.servers = servers;
        self.sidebar_migrated = true;
        true
    }

    pub fn sidebar(&self) -> &Sidebar {
        self.sidebar_for(self.active)
            .unwrap_or(&self.personal_sidebar)
    }
    pub fn sidebar_for(&self, id: Option<u64>) -> Option<&Sidebar> {
        match id {
            Some(id) => self.items.iter().find(|p| p.id == id).map(|p| &p.sidebar),
            None => Some(&self.personal_sidebar),
        }
    }
    pub fn sidebar_mut(&mut self) -> &mut Sidebar {
        self.sidebar_for_mut(self.active)
            .expect("active project exists")
    }
    pub fn sidebar_for_mut(&mut self, id: Option<u64>) -> Option<&mut Sidebar> {
        match id {
            Some(id) => self
                .items
                .iter_mut()
                .find(|p| p.id == id)
                .map(|p| &mut p.sidebar),
            None => Some(&mut self.personal_sidebar),
        }
    }

    pub fn remember_layout(&mut self, layout: Layout) {
        if layout.tab_count() == 0 {
            return;
        }
        if let Some(project) = self
            .active
            .and_then(|id| self.items.iter_mut().find(|p| p.id == id))
        {
            project.layout = layout;
        } else {
            self.personal_layout = Some(layout);
        }
    }

    pub fn create(&mut self, name: &str, layout: Layout) -> Result<u64, String> {
        let name = valid_name(name)?;
        if name.eq_ignore_ascii_case("Personal") {
            return Err(
                "Personal is reserved for your general browsing. Choose another name.".into(),
            );
        }
        if self
            .items
            .iter()
            .any(|project| project.name.to_lowercase() == name.to_lowercase())
        {
            return Err("That name is already used. Choose another name.".into());
        }
        if layout.tab_count() == 0 {
            return Err("Open a local folder before saving a project.".into());
        }
        let id = self
            .items
            .iter()
            .map(|project| project.id)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("Cannot create another project")?;
        self.items.push(Project {
            id,
            name,
            layout,
            sidebar: Sidebar::default(),
        });
        Ok(id)
    }

    pub fn set_name(&mut self, id: u64, name: &str) -> Result<(), String> {
        let name = valid_name(name)?;
        if name.eq_ignore_ascii_case("Personal")
            || self
                .items
                .iter()
                .any(|p| p.id != id && p.name.to_lowercase() == name.to_lowercase())
        {
            return Err("Another Schnauz project already uses this Todoist name.".into());
        }
        self.items
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or("Project no longer exists")?
            .name = name;
        Ok(())
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        let bytes = serde_json::to_vec_pretty(self)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        crate::persistence::write_now(path, &bytes)
    }
}

/// Read-only compatibility for the earliest flat favourites. Preserve missing paths.
fn merge_old_favorites(
    object: &mut serde_json::Map<String, serde_json::Value>,
    old: &str,
    target: &str,
) -> io::Result<()> {
    let Some(value) = object.remove(old) else {
        return Ok(());
    };
    let decode = |error| io::Error::new(io::ErrorKind::InvalidData, error);
    let paths: Vec<PathBuf> = serde_json::from_value(value).map_err(decode)?;
    let mut sidebar: Sidebar = serde_json::from_value(
        object
            .get(target)
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})),
    )
    .map_err(decode)?;
    for path in paths {
        let kind = legacy_kind(&path, Section::Folders);
        if !sidebar.contains(&Shortcut::local(path.clone(), kind)) {
            sidebar.set_path(kind, path, true);
        }
    }
    object.insert(
        target.into(),
        serde_json::to_value(sidebar).map_err(decode)?,
    );
    Ok(())
}

fn valid_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Give your project a name.".into());
    }
    if name.chars().count() > 120 || name.chars().any(char::is_control) {
        return Err("Use up to 120 characters without line breaks.".into());
    }
    Ok(name.to_string())
}

pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Missing settings folder"))?;
    fs::create_dir_all(parent)?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = parent.join(format!(".schnauz-{}-{nonce}.tmp", std::process::id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    #[test]
    fn shortcut_labels_apply_everywhere_and_preserve_destinations() {
        use super::*;
        let original: Shortcut = serde_json::from_str(r#"{"path":"/fixture/reports","kind":"folder"}"#).unwrap();
        assert_eq!(original.display_name(), "reports");
        let mut library = Library::default();
        library.personal_sidebar.shortcuts.push(original.clone());
        library.common_shortcuts = vec![original.clone()];
        edit_shortcut_label(&mut library, None, false, &original, "  Work  ", "  2026  ").unwrap();
        let edited = library.personal_sidebar.shortcuts[0].clone();
        assert_eq!(edited.display_name(), "Work (2026)");
        assert_eq!(edited.path, original.path);
        assert!(edited.same_place(&original));
        assert_eq!(library.common_shortcuts[0].display_name(), "Work (2026)");
        assert!(edit_shortcut_label(&mut library, None, false, &original, "stale", "").is_err());
        library.personal_sidebar.set_shortcut(original.clone(), true);
        assert_eq!(library.personal_sidebar.shortcuts[0].display_name(), "Work (2026)");
        let restored: Library = serde_json::from_str(&serde_json::to_string(&library).unwrap()).unwrap();
        assert_eq!(restored.personal_sidebar.shortcuts, library.personal_sidebar.shortcuts);
        edit_shortcut_label(&mut library, None, false, &edited, "", "Archive").unwrap();
        let no_alias = library.personal_sidebar.shortcuts[0].clone();
        assert_eq!(no_alias.display_name(), "reports (Archive)");
        edit_shortcut_label(&mut library, None, false, &no_alias, "", "").unwrap();
        assert_eq!(library.personal_sidebar.shortcuts[0].display_name(), "reports");
        edit_shortcut_label(&mut library, None, true, &original, "Shared", "").unwrap();
        assert_eq!(library.common_shortcuts[0].display_name(), "Shared");
        assert!(edit_shortcut_label(&mut library, Some(u64::MAX), false, &original, "", "").is_err());
        let mut remote = original.clone();
        remote.server = Some(SftpServer { host: "fixture.invalid".into(), ..Default::default() });
        library.personal_sidebar.shortcuts.push(remote.clone());
        edit_shortcut_label(&mut library, None, false, &remote, "Remote", "SSH").unwrap();
        assert_eq!(library.personal_sidebar.shortcuts[1].display_name(), "Remote (SSH)");
        assert_eq!(library.personal_sidebar.shortcuts[0].display_name(), "Shared");
        assert!(edit_shortcut_label(&mut library, None, false, &original, "bad\nlabel", "").is_err());
        library.personal_sidebar.shortcuts.clear();
        assert!(edit_shortcut_label(&mut library, None, false, &original, "", "").is_err());
    }

    #[test]
    fn shared_labels_cover_files_projects_and_provider_identity() {
        use super::*;
        for kind in [Section::Folders, Section::Files] {
            let original = Shortcut::local("/fixture/shared".into(), kind);
            let mut library = Library::default();
            let project = 1;
            library.items.push(Project { id: project, name: "Other".into(),
                layout: Layout { panes: Vec::new(), active_pane: 0, split_ratio: 0.5 },
                sidebar: Sidebar::default() });
            library.personal_sidebar.shortcuts = vec![original.clone()];
            library.common_shortcuts = vec![original.clone()];
            let mut remote = original.clone();
            remote.server = Some(SftpServer { host: "fixture.invalid".into(), ..Default::default() });
            library.items[0].sidebar.shortcuts = vec![original.clone(), remote.clone()];
            edit_shortcut_label(&mut library, Some(project), false, &original, "Shared", "Archive").unwrap();
            assert_eq!(library.personal_sidebar.shortcuts[0].display_name(), "Shared (Archive)");
            assert_eq!(library.common_shortcuts[0].display_name(), "Shared (Archive)");
            assert_eq!(library.items[0].sidebar.shortcuts[0].display_name(), "Shared (Archive)");
            assert_eq!(library.items[0].sidebar.shortcuts[1], remote);
            let before = library.clone();
            assert!(edit_shortcut_label(&mut library, None, true, &original, "Stale", "").is_err());
            assert_eq!(library, before, "Stale edits must not change any project");
            let edited = library.common_shortcuts[0].clone();
            edit_shortcut_label(&mut library, None, true, &edited, "", "").unwrap();
            assert_eq!(library.personal_sidebar.shortcuts[0], original);
            assert_eq!(library.items[0].sidebar.shortcuts[0], original);
            // Remote destinations share labels too, while keeping local paths separate.
            library.common_shortcuts.push(remote.clone());
            edit_shortcut_label(&mut library, Some(project), false, &remote, "Remote", "SSH").unwrap();
            assert_eq!(library.common_shortcuts[1].display_name(), "Remote (SSH)");
            assert_eq!(library.items[0].sidebar.shortcuts[1].display_name(), "Remote (SSH)");
            assert_eq!(library.personal_sidebar.shortcuts[0], original);
            let restored: Library = serde_json::from_slice(&serde_json::to_vec(&library).unwrap()).unwrap();
            assert_eq!(restored, library);
        }
    }

    #[test]
    fn manual_order_roundtrips_and_preserves_identity_and_project_state() {
        use super::*;
        let a = Shortcut::local("/a".into(), Section::Folders);
        let b = Shortcut::local("/b".into(), Section::Folders);
        let file = Shortcut::local("/f.txt".into(), Section::Files);
        let mut remote = a.clone();
        remote.server = Some(SftpServer { host: "fixture.invalid".into(), user: "fixture".into(), ..Default::default() });
        let mut items = vec![a.clone(), file.clone(), remote.clone(), b.clone()];
        assert!(reorder_shortcuts(&mut items, &b, &a, false));
        assert_eq!(items, vec![b.clone(), a.clone(), file.clone(), remote.clone()]);
        assert!(reorder_shortcuts(&mut items, &b, &remote, true));
        assert_eq!(items, vec![a.clone(), file.clone(), remote.clone(), b.clone()]);
        assert!(!reorder_shortcuts(&mut items, &file, &a, false));
        assert!(!reorder_shortcuts(&mut items, &a, &a, true));
        assert!(!reorder_shortcuts(&mut items, &a, &Shortcut::local(crate::home_dir(), Section::Folders), false));
        let mut library = Library::default();
        library.common_shortcuts = items;
        let layout = Layout { panes: vec![SavedPane { tabs: vec![SavedTab { path: std::env::temp_dir(), view: "List".into(), sort: "Name".into(), ascending: true, filter: EntryFilter::All, show_hidden: false, network:None }], active: 0 }], active_pane: 0, split_ratio: 0.5 };
        let first = library.create("First", layout.clone()).unwrap();
        let second = library.create("Second", layout).unwrap();
        let original = library.clone();
        assert!(library.reorder_project(second, first, false));
        assert_eq!(library.active, original.active);
        assert_eq!(library.items[1], original.items[0]);
        assert_eq!(library.items[0], original.items[1]);
        assert!(!library.reorder_project(second, second, true));
        assert!(!library.reorder_project(u64::MAX, first, true));
        let restored: Library = serde_json::from_str(&serde_json::to_string(&library).unwrap()).unwrap();
        assert_eq!(library, restored);
    }

    #[test]
    fn common_defaults_migrate_once_and_removals_survive_project_switch_and_reload() {
        let mut library = Library::default();
        let defaults = library.common_shortcuts.clone();
        assert_eq!(
            defaults
                .iter()
                .map(|s| s.path.file_name().unwrap().to_string_lossy())
                .collect::<Vec<_>>(),
            vec!["Desktop", "Downloads"]
        );
        for shortcut in defaults {
            library.set_common_shortcut(shortcut, false);
        }
        let home = Shortcut::local(crate::home_dir(), Section::Folders);
        library.set_common_shortcut(home.clone(), false);
        assert!(library.common_contains(&home));
        let id = library
            .create(
                "Work",
                Layout {
                    panes: vec![SavedPane {
                        tabs: vec![tab(PathBuf::from("/tmp"))],
                        active: 0,
                    }],
                    active_pane: 0,
                    split_ratio: 0.5,
                },
            )
            .unwrap();
        library.active = Some(id);
        assert!(library.common_shortcuts.is_empty());
        let restored: Library =
            serde_json::from_str(&serde_json::to_string(&library).unwrap()).unwrap();
        assert!(restored.common_shortcuts.is_empty());
        let mut legacy = serde_json::to_value(&library).unwrap();
        legacy.as_object_mut().unwrap().remove("common_shortcuts");
        let migrated: Library = serde_json::from_value(legacy).unwrap();
        assert_eq!(migrated.common_shortcuts.len(), 2);
    }

    #[test]
    fn common_keeps_local_and_remote_references_separate_without_live_transports() {
        let mut library = Library::default();
        let local = Shortcut::local(PathBuf::from("/shared"), Section::Folders);
        let mut remote = local.clone();
        remote.server = Some(SftpServer {
            host: "example.test".into(),
            user: "test".into(),
            control_path: Some(PathBuf::from("/tmp/live-session")),
            ..Default::default()
        });
        library.set_common_shortcut(local.clone(), true);
        library.set_common_shortcut(remote.clone(), true);
        library.set_common_shortcut(remote.clone(), true);
        assert_eq!(library.common_shortcuts.len(), 4);
        assert!(library.common_contains(&local) && library.common_contains(&remote));
        assert!(library
            .common_shortcuts
            .last()
            .unwrap()
            .server
            .as_ref()
            .unwrap()
            .control_path
            .is_none());
        library.set_common_shortcut(local, false);
        assert!(library.common_contains(&remote));
    }

    #[test]
    fn home_is_always_a_favourite_without_project_records() {
        let home = super::Shortcut::local(crate::home_dir(), super::Section::Folders);
        let mut sidebar = super::Sidebar::default();
        assert!(sidebar.contains(&home));
        sidebar.set_shortcut(home.clone(), true);
        sidebar.set_shortcut(home.clone(), false);
        assert!(sidebar.contains(&home));
        assert!(sidebar.shortcuts.is_empty());
        let restored: super::Sidebar =
            serde_json::from_str(&serde_json::to_string(&sidebar).unwrap()).unwrap();
        assert!(restored.contains(&home));
        let mut remote = home;
        remote.server = Some(crate::SftpServer {
            host: "example.test".into(),
            user: "test".into(),
            ..Default::default()
        });
        assert!(!restored.contains(&remote));
    }

    fn local_paths(sidebar: &super::Sidebar, kind: super::Section) -> Vec<std::path::PathBuf> {
        sidebar
            .shortcuts
            .iter()
            .filter(|s| s.kind == kind && s.server.is_none())
            .map(|s| s.path.clone())
            .collect()
    }
    use super::*;

    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir()
                .join(format!("schnauz-project-{}-{nonce}", std::process::id()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn tab(path: PathBuf) -> SavedTab {
        SavedTab {
            path,
            view: "columns".into(),
            sort: "name".into(),
            ascending: false,
            filter: EntryFilter::Folders,
            show_hidden: false, network:None,
        }
    }

    #[test]
    fn saves_and_restores_unicode_names_and_unusual_paths() {
        let scratch = Scratch::new();
        let folder = scratch.0.join("Design\t2026\n素材");
        fs::create_dir(&folder).unwrap();
        let layout = Layout {
            panes: vec![SavedPane {
                tabs: vec![tab(folder)],
                active: 0,
            }],
            active_pane: 0,
            split_ratio: 0.42,
        };
        let mut library = Library::default();
        library.create("  Design / 设计  ", layout.clone()).unwrap();
        let path = scratch.0.join("settings/projects.json");
        library.save(&path).unwrap();
        let restored = Library::load(&path).unwrap();
        assert_eq!(restored.items[0].name, "Design / 设计");
        assert_eq!(restored.items[0].layout.available().unwrap(), layout);
        assert!(library.create("design / 设计", layout).is_err());
    }

    #[test]
    fn missing_folders_preserve_the_selected_surviving_tab_and_pane() {
        let scratch = Scratch::new();
        let a = scratch.0.join("a");
        let b = scratch.0.join("b");
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();
        let missing = scratch.0.join("missing");
        let layout = Layout {
            panes: vec![
                SavedPane {
                    tabs: vec![tab(missing.clone())],
                    active: 0,
                },
                SavedPane {
                    tabs: vec![tab(missing), tab(a), tab(b.clone())],
                    active: 2,
                },
            ],
            active_pane: 1,
            split_ratio: 9.0,
        };
        let restored = layout.available().unwrap();
        assert_eq!(restored.panes.len(), 1);
        assert_eq!(restored.active_pane, 0);
        assert_eq!(restored.panes[0].active, 1);
        assert_eq!(restored.panes[0].tabs[1].path, b);
        assert_eq!(restored.split_ratio, 0.8);
    }

    #[test]
    fn corrupted_storage_is_reported_and_left_intact() {
        let scratch = Scratch::new();
        let path = scratch.0.join("projects.json");
        fs::write(&path, b"{broken").unwrap();
        assert!(Library::load(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"{broken");
    }

    #[test]
    fn old_sessions_default_to_all_and_new_sessions_preserve_each_filter() {
        let old = r#"{"panes":[{"tabs":[{"path":"/tmp","view":"list","sort":"none","ascending":true}],"active":0}],"active_pane":0,"split_ratio":0.5}"#;
        let mut layout: Layout = serde_json::from_str(old).unwrap();
        assert_eq!(layout.panes[0].tabs[0].filter, EntryFilter::All);
        for filter in [EntryFilter::All, EntryFilter::Folders, EntryFilter::Files] {
            let mut saved = layout.panes[0].tabs[0].clone();
            saved.filter = filter;
            layout.panes[0].tabs.push(saved);
        }
        let json = serde_json::to_vec(&layout).unwrap();
        let restored: Layout = serde_json::from_slice(&json).unwrap();
        assert_eq!(restored, layout);
    }

    #[test]
    fn projects_isolate_favourites_and_remember_each_layout_after_restart() {
        let scratch = Scratch::new();
        let make_layout = |name: &str| Layout {
            panes: vec![SavedPane {
                tabs: vec![tab(scratch.0.join(name))],
                active: 0,
            }],
            active_pane: 0,
            split_ratio: 0.5,
        };
        let mut library = Library::default();
        library.remember_layout(make_layout("personal"));
        library
            .sidebar_mut()
            .set_path(Section::Folders, scratch.0.join("personal pin"), true);
        let a = library.create("Design", make_layout("design")).unwrap();
        let b = library.create("Writing", make_layout("writing")).unwrap();
        library.active = Some(a);
        assert!(local_paths(library.sidebar(), Section::Folders)
            .as_slice()
            .is_empty());
        let pin = scratch.0.join("素材\nnotes");
        library
            .sidebar_mut()
            .set_path(Section::Folders, pin.clone(), true);
        library
            .sidebar_mut()
            .set_path(Section::Folders, pin.clone(), true);
        library.remember_layout(make_layout("design new folder"));
        library.active = Some(b);
        assert!(local_paths(library.sidebar(), Section::Folders)
            .as_slice()
            .is_empty());
        library
            .sidebar_mut()
            .set_path(Section::Folders, scratch.0.join("writing pin"), true);
        library.active = Some(a);
        assert_eq!(
            local_paths(library.sidebar(), Section::Folders).as_slice(),
            &[pin.clone()]
        );
        let path = scratch.0.join("projects.json");
        library.save(&path).unwrap();
        let mut restored = Library::load(&path).unwrap();
        assert_eq!(restored, library);
        assert_eq!(restored.items[0].layout, make_layout("design new folder"));
        restored
            .sidebar_mut()
            .set_path(Section::Folders, pin, false);
        assert!(local_paths(restored.sidebar(), Section::Folders)
            .as_slice()
            .is_empty());
        restored.active = None;
        assert_eq!(
            local_paths(restored.sidebar(), Section::Folders).as_slice(),
            &[scratch.0.join("personal pin")]
        );
        assert_eq!(restored.personal_layout, Some(make_layout("personal")));
    }

    #[test]
    fn project_sidebar_is_empty_then_isolated_and_shared_explicitly_after_restart() {
        let scratch = Scratch::new();
        let layout = Layout {
            panes: vec![SavedPane {
                tabs: vec![tab(scratch.0.clone())],
                active: 0,
            }],
            active_pane: 0,
            split_ratio: 0.5,
        };
        let mut library = Library::default();
        let a = library.create("Design", layout.clone()).unwrap();
        let b = library.create("Code", layout).unwrap();
        assert_eq!(library.sidebar_for(Some(a)).unwrap(), &Sidebar::default());
        let location = scratch.0.join("shared folder");
        let file = location.join("spec.txt");
        library.active = Some(a);
        library
            .sidebar_mut()
            .set_path(Section::Folders, location.clone(), true);
        library
            .sidebar_mut()
            .set_path(Section::Folders, location.clone(), true);
        library
            .sidebar_mut()
            .set_path(Section::Files, file.clone(), true);
        library
            .sidebar_mut()
            .set_path(Section::Files, file.clone(), true);
        library.sidebar_mut().servers.push(SftpServer {
            host: "design-host".into(),
            name: "Design server".into(),
            ..Default::default()
        });
        assert_eq!(
            local_paths(library.sidebar(), Section::Files),
            vec![file.clone()]
        );
        library.active = Some(b);
        assert_eq!(library.sidebar(), &Sidebar::default());
        // Sharing from a sidebar menu updates its destination without switching.
        library.sidebar_for_mut(Some(a)).unwrap().set_path(
            Section::Folders,
            location.clone(),
            true,
        );
        library.sidebar_for_mut(Some(b)).unwrap().set_path(
            Section::Folders,
            location.clone(),
            true,
        );
        library
            .sidebar_for_mut(None)
            .unwrap()
            .set_path(Section::Folders, location.clone(), true);
        assert_eq!(library.active, Some(b));
        let path = scratch.0.join("projects.json");
        library.save(&path).unwrap();
        let mut restored = Library::load(&path).unwrap();
        assert_eq!(restored, library);
        for id in [None, Some(a), Some(b)] {
            assert_eq!(
                local_paths(restored.sidebar_for(id).unwrap(), Section::Folders),
                vec![location.clone()]
            );
        }
        assert!(local_paths(restored.sidebar(), Section::Files).is_empty());
        assert!(restored.sidebar().servers.is_empty());
        restored.active = Some(a);
        assert_eq!(restored.sidebar().servers[0].host, "design-host");
        restored.sidebar_mut().set_path(Section::Files, file, false);
        assert!(local_paths(restored.sidebar(), Section::Files).is_empty());
        restored
            .sidebar_mut()
            .set_path(Section::Folders, location, false);
        assert!(local_paths(restored.sidebar(), Section::Folders).is_empty());
        assert_eq!(
            local_paths(restored.sidebar_for(Some(b)).unwrap(), Section::Folders).len(),
            1
        );
    }

    #[test]
    fn old_servers_migrate_once_to_personal_without_filling_new_projects() {
        let mut library = Library::default();
        let server = SftpServer {
            name: "Existing".into(),
            host: "legacy-host".into(),
            ..Default::default()
        };
        assert!(library.migrate_sidebar(vec![server.clone()]));
        assert!(!library.migrate_sidebar(vec![]));
        assert_eq!(library.personal_sidebar.servers, vec![server]);
        let json = serde_json::to_vec(&library).unwrap();
        let mut restored: Library = serde_json::from_slice(&json).unwrap();
        restored.personal_sidebar.servers.clear();
        assert!(!restored.migrate_sidebar(vec![SftpServer::default()]));
        assert!(restored.personal_sidebar.servers.is_empty());
    }

    #[test]
    fn merges_old_favorites_once_without_losing_files_or_missing_paths() {
        let scratch = Scratch::new();
        let path = scratch.0.join("projects.json");
        let layout = Layout {
            panes: vec![SavedPane {
                tabs: vec![tab(scratch.0.clone())],
                active: 0,
            }],
            active_pane: 0,
            split_ratio: 0.5,
        };
        let mut library = Library::default();
        let id = library.create("Design", layout).unwrap();
        library.active = Some(id);
        let existing = scratch.0.join("existing folder");
        let file = scratch.0.join("notes.txt");
        let missing = scratch.0.join("unavailable volume");
        library
            .sidebar_mut()
            .set_path(Section::Folders, existing.clone(), true);
        let mut old = serde_json::to_value(&library).unwrap();
        old["items"][0]["sidebar"]["cloud"] = serde_json::json!([scratch.0.join("cloud")]);
        old["personal_favorites"] = serde_json::json!([file]);
        old["items"][0]["favorites"] = serde_json::json!([existing, file, missing, file]);
        let original = serde_json::to_vec(&old).unwrap();
        fs::write(&path, &original).unwrap();
        let restored = Library::load(&path).unwrap();
        assert_eq!(
            local_paths(restored.sidebar(), Section::Folders),
            [existing, scratch.0.join("cloud"), file.clone(), missing]
        );
        assert_eq!(
            local_paths(&restored.personal_sidebar, Section::Folders),
            [file]
        );
        assert_eq!(fs::read(&path).unwrap(), original);
        restored.save(&path).unwrap();
        let saved = fs::read_to_string(&path).unwrap();
        assert!(!saved.contains("favorites"));
        assert_eq!(Library::load(&path).unwrap(), restored);
    }

    #[test]
    fn migrates_legacy_workspaces_without_modifying_original_or_reimporting_pins() {
        let scratch = Scratch::new();
        let legacy = scratch.0.join("workspaces.json");
        let bytes = br#"{"version":1,"items":[{"id":1,"name":"Existing","layout":{"panes":[{"tabs":[{"path":"/tmp","view":"list","sort":"none","ascending":true}],"active":0}],"active_pane":0,"split_ratio":0.5}}]}"#;
        fs::write(&legacy, bytes).unwrap();
        let path = scratch.0.join("projects.json");
        let pin = scratch.0.join("old bookmark");
        let mut library = Library::load_migrating(&path, &legacy, vec![pin.clone()]).unwrap();
        assert_eq!(library.items[0].name, "Existing");
        assert!(local_paths(&library.items[0].sidebar, Section::Folders).is_empty());
        assert_eq!(
            local_paths(library.sidebar(), Section::Folders).as_slice(),
            &[pin.clone()]
        );
        library.sidebar_mut().set_path(Section::Folders, pin, false);
        library.save(&path).unwrap();
        let restored =
            Library::load_migrating(&path, &legacy, vec![scratch.0.join("stale")]).unwrap();
        assert!(local_paths(restored.sidebar(), Section::Folders)
            .as_slice()
            .is_empty());
        assert_eq!(fs::read(legacy).unwrap(), bytes);
        assert!(library
            .create("Personal", library.items[0].layout.clone())
            .is_err());
    }
    #[test]
    fn todoist_links_and_pending_creations_persist_per_project_without_changing_legacy_data() {
        let scratch = Scratch::new();
        let layout = Layout {
            panes: vec![SavedPane {
                tabs: vec![tab(scratch.0.clone())],
                active: 0,
            }],
            active_pane: 0,
            split_ratio: 0.5,
        };
        let mut library = Library::default();
        let a = library.create("Design", layout.clone()).unwrap();
        let b = library.create("Admin", layout).unwrap();
        library.sidebar_for_mut(Some(a)).unwrap().todoist = Some(TodoistLink {
            account_id: "account-A".into(),
            project_id: "opaque-project-A".into(),
            project_name: "Design".into(),
        });
        library
            .sidebar_for_mut(Some(b))
            .unwrap()
            .todoist_create_request = Some("stable-idempotency-key".into());
        library
            .personal_sidebar
            .set_path(Section::Folders, scratch.0.join("unavailable"), true);
        let file = scratch.0.join("projects.json");
        library.save(&file).unwrap();
        let restored = Library::load(&file).unwrap();
        assert_eq!(restored, library);
        assert!(restored.personal_sidebar.todoist.is_none());
        assert!(restored.sidebar_for(Some(b)).unwrap().todoist.is_none());
        let legacy: Sidebar = serde_json::from_str(r#"{"locations":["/legacy-folder"]}"#).unwrap();
        assert!(legacy.todoist.is_none() && legacy.todoist_create_request.is_none());
        assert_eq!(
            local_paths(&legacy, Section::Folders),
            vec![PathBuf::from("/legacy-folder")]
        );
        library.set_name(a, "DESIGN").unwrap();
        assert!(library.set_name(a, "Admin").is_err());
        assert!(library.set_name(a, "Personal").is_err());
        assert!(library.set_name(a, "Bad\nName").is_err());
        assert_eq!(library.items[0].name, "DESIGN");
        assert_eq!(
            library
                .sidebar_for(Some(a))
                .unwrap()
                .todoist
                .as_ref()
                .unwrap()
                .project_id,
            "opaque-project-A"
        );
    }
    #[test]
    fn legacy_sections_merge_and_sort_actual_files_and_folders_without_touching_sources() {
        let scratch = Scratch::new();
        let folder = scratch.0.join("folder");
        let tagged_folder = scratch.0.join("tagged folder");
        let file = scratch.0.join("notes.txt");
        let cloud = scratch.0.join("cloud");
        let network = scratch.0.join("missing network mount");
        fs::create_dir(&folder).unwrap();
        fs::create_dir(&tagged_folder).unwrap();
        fs::create_dir(&cloud).unwrap();
        fs::write(&file, "keep this text").unwrap();
        let legacy = serde_json::json!({"locations":[folder, file], "cloud":[cloud, folder], "server_folders":[network], "tagged":[file, tagged_folder]});
        let sidebar: Sidebar = serde_json::from_value(legacy).unwrap();
        assert_eq!(
            local_paths(&sidebar, Section::Folders),
            [folder, cloud, network, tagged_folder]
        );
        assert_eq!(local_paths(&sidebar, Section::Files), [file.clone()]);
        assert_eq!(fs::read_to_string(&file).unwrap(), "keep this text");
        let encoded = serde_json::to_string(&sidebar).unwrap();
        for obsolete in [
            "\"locations\"",
            "\"cloud\"",
            "\"server_folders\"",
            "\"tagged\"",
        ] {
            assert!(!encoded.contains(obsolete));
        }
        assert_eq!(serde_json::from_str::<Sidebar>(&encoded).unwrap(), sidebar);
    }

    #[test]
    fn remote_favourites_keep_server_identity_without_persisting_live_ssh_transport() {
        let path = PathBuf::from("/srv/素材/reports");
        let server = SftpServer {
            host: "work-host".into(),
            user: "oliver".into(),
            port: 2222,
            control_path: Some(PathBuf::from("/private/tmp/live-terminal-mux")),
            ..Default::default()
        };
        let remote = Shortcut {
            path: path.clone(),
            kind: Section::Folders,
            server: Some(server.clone()),
            network:None,
            alias: String::new(),
            add_on: String::new(),
        };
        let mut sidebar = Sidebar::default();
        sidebar.set_shortcut(remote.clone(), true);
        let mut updated = remote.clone();
        updated.server.as_mut().unwrap().name = "Renamed host".into();
        sidebar.set_shortcut(updated, true);
        assert_eq!(
            sidebar.shortcuts.len(),
            1,
            "Changing a label must not duplicate a remote pin"
        );
        sidebar.set_path(Section::Folders, path.clone(), true);
        let mut other_host = remote.clone();
        other_host.server.as_mut().unwrap().host = "other-host".into();
        sidebar.set_shortcut(other_host, true);
        let mut other_user = remote.clone();
        other_user.server.as_mut().unwrap().user = "other-user".into();
        sidebar.set_shortcut(other_user, true);
        assert_eq!(
            sidebar.shortcuts.len(),
            4,
            "Local paths and different remote namespaces must stay separate"
        );
        let encoded = serde_json::to_string(&sidebar).unwrap();
        assert!(!encoded.contains("control_path") && !encoded.contains("live-terminal-mux"));
        let mut restored: Sidebar = serde_json::from_str(&encoded).unwrap();
        assert!(restored.contains(&remote));
        assert_eq!(restored.shortcuts[0].kind, Section::Folders);
        assert!(restored.shortcuts[0]
            .server
            .as_ref()
            .unwrap()
            .control_path
            .is_none());
        restored.set_shortcut(remote, false);
        assert_eq!(restored.shortcuts.len(), 3);
        assert!(restored.contains(&Shortcut::local(path, Section::Folders)));
    }
}
