//! Folder presentation rules, loaded once. Inheritance never walks the disk.
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ViewMode {
    #[default]
    List,
    Icons,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Column {
    Name,
    Kind,
    Modified,
    Size,
    Created,
    Extension,
    Permissions,
    Location,
}
impl Column {
    pub const ALL: [Self; 8] = [
        Self::Name,
        Self::Kind,
        Self::Modified,
        Self::Size,
        Self::Created,
        Self::Extension,
        Self::Permissions,
        Self::Location,
    ];
    pub fn key(self) -> usize {
        Self::ALL.iter().position(|c| *c == self).unwrap()
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Kind => "Kind",
            Self::Modified => "Date Modified",
            Self::Size => "Size",
            Self::Created => "Date Created",
            Self::Extension => "Extension",
            Self::Permissions => "Permissions",
            Self::Location => "Location",
        }
    }
    pub fn default_width(self) -> f32 {
        match self {
            Self::Name => 320.,
            Self::Kind => 165.,
            Self::Modified | Self::Created => 185.,
            Self::Size => 90.,
            Self::Extension => 90.,
            Self::Permissions => 160.,
            Self::Location => 300.,
        }
    }
    fn minimum(self) -> f32 {
        if self == Self::Name {
            140.
        } else {
            60.
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct ColumnSpec {
    pub column: Column,
    pub width: f32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct ListLayout {
    pub columns: Vec<ColumnSpec>,
}
impl Default for ListLayout {
    fn default() -> Self {
        Self {
            columns: [Column::Name, Column::Kind, Column::Modified, Column::Size]
                .into_iter()
                .map(|column| ColumnSpec {
                    column,
                    width: column.default_width(),
                })
                .collect(),
        }
    }
}
impl ListLayout {
    pub fn normalize(&mut self) {
        let mut seen = HashSet::new();
        self.columns.retain(|c| seen.insert(c.column));
        if !seen.contains(&Column::Name) {
            self.columns.insert(
                0,
                ColumnSpec {
                    column: Column::Name,
                    width: Column::Name.default_width(),
                },
            );
        }
        for c in &mut self.columns {
            c.width = if c.width.is_finite() {
                c.width.clamp(c.column.minimum(), 2000.)
            } else {
                c.column.default_width()
            };
        }
    }
    pub fn contains(&self, column: Column) -> bool {
        self.columns.iter().any(|c| c.column == column)
    }
    pub fn width(&self, column: Column) -> f32 {
        self.columns
            .iter()
            .find(|c| c.column == column)
            .map_or(0., |c| c.width)
    }
    pub fn offset(&self, column: Column) -> f32 {
        self.columns
            .iter()
            .take_while(|c| c.column != column)
            .map(|c| c.width)
            .sum()
    }
    pub fn total_width(&self) -> f32 {
        self.columns.iter().map(|c| c.width).sum::<f32>() + 44.
    }
    pub fn set_width(&mut self, column: Column, width: f32) {
        if let Some(c) = self.columns.iter_mut().find(|c| c.column == column) {
            c.width = width.clamp(column.minimum(), 2000.);
        }
    }
    pub fn toggle(&mut self, column: Column) {
        if column == Column::Name {
            return;
        }
        if self.contains(column) {
            self.columns.retain(|c| c.column != column);
        } else {
            self.columns.push(ColumnSpec {
                column,
                width: column.default_width(),
            });
        }
    }
    pub fn move_before(&mut self, column: Column, before: Option<Column>) {
        if before == Some(column) {
            return;
        }
        let Some(from) = self.columns.iter().position(|c| c.column == column) else {
            return;
        };
        let c = self.columns.remove(from);
        let to = before
            .and_then(|before| self.columns.iter().position(|c| c.column == before))
            .unwrap_or(self.columns.len());
        self.columns.insert(to, c);
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct FolderView {
    pub view: ViewMode,
    pub list: ListLayout,
    pub preview_width: u16,
    pub sort: String,
    pub ascending: bool,
}
impl Default for FolderView {
    fn default() -> Self {
        Self {
            view: ViewMode::List,
            list: ListLayout::default(),
            preview_width: 420,
            sort: "none".into(),
            ascending: true,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Rule {
    descendants: bool,
    style: FolderView,
}
#[derive(Clone, Serialize, Deserialize)]
struct Data {
    version: u32,
    folders: HashMap<PathBuf, Rule>,
}
impl Default for Data {
    fn default() -> Self {
        Self {
            version: 1,
            folders: HashMap::new(),
        }
    }
}

fn normalized(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            c => out.push(c.as_os_str()),
        }
    }
    out
}

pub(crate) struct Library {
    data: Data,
    path: Option<PathBuf>,
    pub error: Option<String>,
    revision: Arc<AtomicU64>,
    gate: Arc<Mutex<()>>,
}
impl Library {
    pub fn open(path: Option<PathBuf>) -> Self {
        let loaded = path
            .as_ref()
            .map_or(Ok(Data::default()), |p| match std::fs::read(p) {
                Ok(bytes) => serde_json::from_slice::<Data>(&bytes)
                    .map_err(|e| e.to_string())
                    .and_then(|d| {
                        if d.version == 1 {
                            Ok(d)
                        } else {
                            Err("Unsupported folder view version".into())
                        }
                    }),
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Data::default()),
                Err(e) => Err(e.to_string()),
            });
        let (mut data, error) = match loaded {
            Ok(d) => (d, None),
            Err(e) => (Data::default(), Some(e)),
        };
        data.folders.retain(|p, _| p.is_absolute());
        data.folders = data
            .folders
            .into_iter()
            .map(|(p, mut rule)| {
                rule.style.list.normalize();
                rule.style.preview_width = rule.style.preview_width.clamp(220, 1200);
                (normalized(&p), rule)
            })
            .collect();
        Self {
            data,
            path,
            error,
            revision: Arc::new(AtomicU64::new(0)),
            gate: Arc::new(Mutex::new(())),
        }
    }
    pub fn resolve(&self, path: &Path) -> Option<&FolderView> {
        let path = normalized(path);
        path.ancestors().find_map(|ancestor| {
            self.data
                .folders
                .get(ancestor)
                .filter(|r| ancestor == path || r.descendants)
                .map(|r| &r.style)
        })
    }
    pub fn remember(&mut self, path: PathBuf, mut style: FolderView, descendants: bool) {
        let path = normalized(&path);
        if !path.is_absolute() {
            return;
        }
        style.list.normalize();
        style.preview_width = style.preview_width.clamp(220, 1200);
        let inherited = self.data.folders.get(&path).is_some_and(|r| r.descendants);
        if descendants {
            self.data
                .folders
                .retain(|p, _| p == &path || !p.starts_with(&path));
        }
        self.data.folders.insert(
            path,
            Rule {
                descendants: descendants || inherited,
                style,
            },
        );
        self.revision.fetch_add(1, Ordering::AcqRel);
    }
    pub fn reset(&mut self, path: &Path) {
        self.data.folders.remove(&normalized(path));
        self.revision.fetch_add(1, Ordering::AcqRel);
    }
    pub fn write_job(&self) -> Option<WriteJob> {
        if self.error.is_some() || self.revision.load(Ordering::Acquire) == 0 {
            return None;
        }
        Some(WriteJob {
            data: self.data.clone(),
            path: self.path.clone()?,
            revision: self.revision.clone(),
            version: self.revision.load(Ordering::Acquire),
            gate: self.gate.clone(),
        })
    }
}
pub(crate) struct WriteJob {
    data: Data,
    path: PathBuf,
    revision: Arc<AtomicU64>,
    version: u64,
    gate: Arc<Mutex<()>>,
}
impl WriteJob {
    pub fn run(self) -> io::Result<()> {
        let _lock = self
            .gate
            .lock()
            .map_err(|_| io::Error::other("Folder view writer unavailable"))?;
        if self.version != self.revision.load(Ordering::Acquire) {
            return Ok(());
        }
        let bytes = serde_json::to_vec(&self.data).map_err(io::Error::other)?;
        crate::projects::atomic_write(&self.path, &bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inheritance_overrides_and_apply_all_are_component_based() {
        let mut lib = Library::open(None);
        let icons = FolderView {
            view: ViewMode::Icons,
            ..Default::default()
        };
        lib.remember("/work/project".into(), icons.clone(), true);
        assert_eq!(lib.resolve(Path::new("/work/project/a/b")), Some(&icons));
        assert_eq!(lib.resolve(Path::new("/work/project-other")), None);
        lib.remember("/work/project/a".into(), FolderView::default(), false);
        assert_eq!(
            lib.resolve(Path::new("/work/project/a")).unwrap().view,
            ViewMode::List
        );
        assert_eq!(
            lib.resolve(Path::new("/work/project/a/b")).unwrap().view,
            ViewMode::Icons
        );
        lib.remember("/work/project".into(), icons.clone(), true);
        assert_eq!(lib.data.folders.len(), 1);
        assert_eq!(lib.resolve(Path::new("/work/project/a")), Some(&icons));
    }
    #[test]
    fn exact_rules_do_not_visit_subfolders_and_reset_inherits() {
        let mut lib = Library::open(None);
        lib.remember("/work".into(), FolderView::default(), true);
        lib.remember(
            "/work/a".into(),
            FolderView {
                view: ViewMode::Icons,
                ..Default::default()
            },
            false,
        );
        assert_eq!(
            lib.resolve(Path::new("/work/a/b")).unwrap().view,
            ViewMode::List
        );
        lib.reset(Path::new("/work/a"));
        assert_eq!(
            lib.resolve(Path::new("/work/a")).unwrap().view,
            ViewMode::List
        );
    }
    #[test]
    fn editing_a_parent_preserves_individual_child_overrides() {
        let mut lib = Library::open(None);
        lib.remember("/work".into(), FolderView::default(), true);
        let icons = FolderView {
            view: ViewMode::Icons,
            ..Default::default()
        };
        lib.remember("/work/custom".into(), icons.clone(), false);
        let mut wider = FolderView::default();
        wider.list.set_width(Column::Name, 500.);
        wider.preview_width = 640;
        lib.remember("/work".into(), wider.clone(), false);
        assert_eq!(lib.resolve(Path::new("/work/new/child")), Some(&wider));
        assert_eq!(lib.resolve(Path::new("/work/custom")), Some(&icons));
        lib.remember("/work".into(), wider.clone(), true);
        assert_eq!(lib.resolve(Path::new("/work/custom")), Some(&wider));
    }
    #[test]
    fn reordered_columns_keep_widths_and_name_cannot_be_hidden() {
        let mut layout = ListLayout::default();
        layout.set_width(Column::Name, 450.);
        layout.toggle(Column::Created);
        layout.move_before(Column::Created, Some(Column::Name));
        layout.toggle(Column::Name);
        assert_eq!(layout.columns[0].column, Column::Created);
        assert_eq!(layout.offset(Column::Name), 185.);
        assert_eq!(layout.width(Column::Name), 450.);
        assert_eq!(layout.total_width(), 1119.);
        layout.columns.push(ColumnSpec {
            column: Column::Name,
            width: -100.,
        });
        layout.normalize();
        assert_eq!(layout.columns.len(), 5);
    }
    #[test]
    fn central_storage_roundtrips_and_stale_writes_cannot_replace_newer_rules() {
        let dir = crate::tests::scratch("folder-views");
        let path = dir.join("views.json");
        let mut lib = Library::open(Some(path.clone()));
        lib.remember("/no/disk/scan".into(), FolderView::default(), true);
        let stale = lib.write_job().unwrap();
        let style = FolderView {
            view: ViewMode::Icons,
            preview_width: 680,
            ..Default::default()
        };
        lib.remember("/no/disk/scan".into(), style.clone(), true);
        lib.write_job().unwrap().run().unwrap();
        stale.run().unwrap();
        assert_eq!(
            Library::open(Some(path.clone())).resolve(Path::new("/no/disk/scan/new/folder")),
            Some(&style)
        );
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        std::fs::write(&path, b"broken existing settings").unwrap();
        let mut broken = Library::open(Some(path.clone()));
        broken.remember("/test".into(), FolderView::default(), false);
        assert!(broken.write_job().is_none());
        assert_eq!(std::fs::read(&path).unwrap(), b"broken existing settings");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
