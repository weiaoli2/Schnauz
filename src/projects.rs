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
                if tab.path.is_absolute() && tab.path.is_dir() {
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

/// Explicit project shortcuts and history. No automatically discovered roots.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub(crate) struct Sidebar {
    pub locations: Vec<PathBuf>,
    pub cloud: Vec<PathBuf>,
    pub server_folders: Vec<PathBuf>,
    pub servers: Vec<SftpServer>,
    pub tagged: Vec<PathBuf>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Section {
    Locations,
    Cloud,
    ServerFolders,
    Tagged,
}
impl Sidebar {
    pub fn paths(&self, section: Section) -> &[PathBuf] {
        match section {
            Section::Locations => &self.locations,
            Section::Cloud => &self.cloud,
            Section::ServerFolders => &self.server_folders,
            Section::Tagged => &self.tagged,
        }
    }
    pub fn set_path(&mut self, section: Section, path: PathBuf, add: bool) {
        let paths = match section {
            Section::Locations => &mut self.locations,
            Section::Cloud => &mut self.cloud,
            Section::ServerFolders => &mut self.server_folders,
            Section::Tagged => &mut self.tagged,
        };
        if add {
            if !paths.contains(&path) {
                paths.push(path);
            }
        } else {
            paths.retain(|p| p != &path);
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct Project {
    pub id: u64,
    pub name: String,
    pub layout: Layout,
    #[serde(default)]
    pub favorites: Vec<PathBuf>,
    #[serde(default)]
    pub sidebar: Sidebar,
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
    pub personal_favorites: Vec<PathBuf>,
    #[serde(default)]
    pub personal_sidebar: Sidebar,
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
            personal_favorites: Vec::new(),
            personal_sidebar: Sidebar::default(),
            sidebar_migrated: false,
        }
    }
}

impl Library {
    pub fn load(path: &Path) -> io::Result<Self> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(error),
        };
        let mut library: Self = serde_json::from_slice(&bytes)
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
        library.personal_favorites = favorites;
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

    pub fn favorites(&self) -> &[PathBuf] {
        self.active
            .and_then(|id| self.items.iter().find(|p| p.id == id))
            .map(|p| p.favorites.as_slice())
            .unwrap_or(&self.personal_favorites)
    }

    pub fn set_favorite(&mut self, path: PathBuf, pinned: bool) {
        let favorites = self
            .active
            .and_then(|id| self.items.iter_mut().find(|p| p.id == id))
            .map(|p| &mut p.favorites)
            .unwrap_or(&mut self.personal_favorites);
        if pinned {
            if !favorites.contains(&path) {
                favorites.push(path);
            }
        } else {
            favorites.retain(|p| p != &path);
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
            favorites: Vec::new(),
            sidebar: Sidebar::default(),
        });
        Ok(id)
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        let bytes = serde_json::to_vec_pretty(self)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        atomic_write(path, &bytes)
    }
}

fn valid_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Give your project a name.".into());
    }
    if name.chars().count() > 80 || name.chars().any(char::is_control) {
        return Err("Use up to 80 characters without line breaks.".into());
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
    let temporary = parent.join(format!(".studio-{}-{nonce}.tmp", std::process::id()));
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
    use super::*;

    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path =
                std::env::temp_dir().join(format!("studio-project-{}-{nonce}", std::process::id()));
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
            show_hidden: false,
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
    fn projects_isolate_favorites_and_remember_each_layout_after_restart() {
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
        library.set_favorite(scratch.0.join("personal pin"), true);
        let a = library.create("Design", make_layout("design")).unwrap();
        let b = library.create("Writing", make_layout("writing")).unwrap();
        library.active = Some(a);
        assert!(library.favorites().is_empty());
        let pin = scratch.0.join("素材\nnotes");
        library.set_favorite(pin.clone(), true);
        library.set_favorite(pin.clone(), true);
        library.remember_layout(make_layout("design new folder"));
        library.active = Some(b);
        assert!(library.favorites().is_empty());
        library.set_favorite(scratch.0.join("writing pin"), true);
        library.active = Some(a);
        assert_eq!(library.favorites(), &[pin.clone()]);
        let path = scratch.0.join("projects.json");
        library.save(&path).unwrap();
        let mut restored = Library::load(&path).unwrap();
        assert_eq!(restored, library);
        assert_eq!(restored.items[0].layout, make_layout("design new folder"));
        restored.set_favorite(pin, false);
        assert!(restored.favorites().is_empty());
        restored.active = None;
        assert_eq!(restored.favorites(), &[scratch.0.join("personal pin")]);
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
            .set_path(Section::Locations, location.clone(), true);
        library
            .sidebar_mut()
            .set_path(Section::Cloud, location.clone(), true);
        library
            .sidebar_mut()
            .set_path(Section::Tagged, file.clone(), true);
        library
            .sidebar_mut()
            .set_path(Section::Tagged, file.clone(), true);
        library.sidebar_mut().servers.push(SftpServer {
            host: "design-host".into(),
            name: "Design server".into(),
            ..Default::default()
        });
        assert_eq!(library.sidebar().tagged, vec![file.clone()]);
        library.active = Some(b);
        assert_eq!(library.sidebar(), &Sidebar::default());
        // Sharing from a sidebar menu updates its destination without switching.
        library.sidebar_for_mut(Some(a)).unwrap().set_path(
            Section::Locations,
            location.clone(),
            true,
        );
        library.sidebar_for_mut(Some(b)).unwrap().set_path(
            Section::Locations,
            location.clone(),
            true,
        );
        library
            .sidebar_for_mut(None)
            .unwrap()
            .set_path(Section::Locations, location.clone(), true);
        assert_eq!(library.active, Some(b));
        let path = scratch.0.join("projects.json");
        library.save(&path).unwrap();
        let mut restored = Library::load(&path).unwrap();
        assert_eq!(restored, library);
        for id in [None, Some(a), Some(b)] {
            assert_eq!(
                restored.sidebar_for(id).unwrap().locations,
                vec![location.clone()]
            );
        }
        assert!(restored.sidebar().tagged.is_empty());
        assert!(restored.sidebar().servers.is_empty());
        restored.active = Some(a);
        assert_eq!(restored.sidebar().servers[0].host, "design-host");
        restored
            .sidebar_mut()
            .set_path(Section::Tagged, file, false);
        assert!(restored.sidebar().tagged.is_empty());
        restored
            .sidebar_mut()
            .set_path(Section::Locations, location, false);
        assert!(restored.sidebar().locations.is_empty());
        assert_eq!(restored.sidebar_for(Some(b)).unwrap().locations.len(), 1);
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
    fn migrates_legacy_workspaces_without_modifying_original_or_reimporting_pins() {
        let scratch = Scratch::new();
        let legacy = scratch.0.join("workspaces.json");
        let bytes = br#"{"version":1,"items":[{"id":1,"name":"Existing","layout":{"panes":[{"tabs":[{"path":"/tmp","view":"list","sort":"none","ascending":true}],"active":0}],"active_pane":0,"split_ratio":0.5}}]}"#;
        fs::write(&legacy, bytes).unwrap();
        let path = scratch.0.join("projects.json");
        let pin = scratch.0.join("old bookmark");
        let mut library = Library::load_migrating(&path, &legacy, vec![pin.clone()]).unwrap();
        assert_eq!(library.items[0].name, "Existing");
        assert!(library.items[0].favorites.is_empty());
        assert_eq!(library.favorites(), &[pin.clone()]);
        library.set_favorite(pin, false);
        library.save(&path).unwrap();
        let restored =
            Library::load_migrating(&path, &legacy, vec![scratch.0.join("stale")]).unwrap();
        assert!(restored.favorites().is_empty());
        assert_eq!(fs::read(legacy).unwrap(), bytes);
        assert!(library
            .create("Personal", library.items[0].layout.clone())
            .is_err());
    }
}
