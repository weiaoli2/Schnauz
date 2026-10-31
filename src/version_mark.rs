//! Filename-only version grouping. No filesystem reads, scans or file changes.
use super::*;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering as Cmp;

pub(super) const PALETTE: [(&str, u32); 6] = [
    ("Yellow", 0xffeb80),
    ("Green", 0xb9ed9c),
    ("Pink", 0xffbddb),
    ("Blue", 0xb2dfff),
    ("Orange", 0xffd09b),
    ("Lavender", 0xd9c5ff),
];
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Target {
    #[default]
    Files,
    Folders,
}
impl Target {
    fn accepts(self, folder: bool) -> bool {
        folder == (self == Self::Folders)
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Kind {
    #[default]
    NewestVersion,
    VersionMatch,
    FilenameEnding,
}
impl Kind {
    fn label(self) -> &'static str {
        match self {
            Self::NewestVersion => "Newest version",
            Self::VersionMatch => "Version match",
            Self::FilenameEnding => "Name ending",
        }
    }
}
const KINDS: [Kind; 3] = [
    Kind::NewestVersion,
    Kind::VersionMatch,
    Kind::FilenameEnding,
];
const MAX_RULES: usize = 16;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct MarkRow {
    pub target: Target,
    pub kind: Kind,
    pub patterns: Vec<String>,
    pub color: u32,
    pub mark_single_version: bool,
}
impl Default for MarkRow {
    fn default() -> Self {
        Self {
            target: Target::Files,
            kind: Kind::NewestVersion,
            patterns: vec!["vX".into(), "vX.X".into(), "vX.X.X".into()],
            color: PALETTE[0].1,
            mark_single_version: false,
        }
    }
}
impl MarkRow {
    fn parse(target: Target, kind: Kind, input: &str, color: u32) -> Result<Self, String> {
        if input.len() > 512 {
            return Err("Use at most 512 characters per rule.".into());
        }
        let mut patterns = Vec::new();
        for value in input
            .split([',', ';', '\n'])
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let normalized = match kind {
                Kind::NewestVersion => {
                    if Pattern::parse(value).is_none() {
                        return Err("Newest version uses patterns such as vX, vX.X or revX; X represents a whole number.".into());
                    }
                    value.to_ascii_lowercase().replace('x', "X")
                }
                Kind::VersionMatch => {
                    if Pattern::parse(value).is_none() && literal_version(value).is_none() {
                        return Err("Version match accepts vX, vX.X or specific versions such as v1 and v2.1.".into());
                    }
                    value.to_ascii_lowercase().replace('x', "X")
                }
                Kind::FilenameEnding => {
                    let value = value.strip_prefix('*').unwrap_or(value);
                    if value.is_empty()
                        || value.len() > 128
                        || value
                            .chars()
                            .any(|c| c.is_control() || "/\\*?[]{}".contains(c))
                    {
                        return Err("Enter literal endings such as .pdf, .ai or _final.pdf. Only an optional leading * is supported.".into());
                    }
                    value.to_lowercase()
                }
            };
            if !patterns.contains(&normalized) {
                patterns.push(normalized);
            }
        }
        if patterns.is_empty() || patterns.len() > 8 {
            return Err("Enter between 1 and 8 comma-separated patterns per rule.".into());
        }
        if !PALETTE.iter().any(|(_, c)| *c == color) {
            return Err("Choose one of the six marker colors.".into());
        }
        Ok(Self {
            target,
            kind,
            patterns,
            color,
            mark_single_version: false,
        })
    }
    fn marker_color(&self) -> u32 {
        if PALETTE.iter().any(|(_, c)| *c == self.color) {
            self.color
        } else {
            PALETTE[0].1
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Rule {
    pub enabled: bool,
    pub rows: Vec<MarkRow>,
}
impl Default for Rule {
    fn default() -> Self {
        Self {
            enabled: false,
            rows: vec![MarkRow::default()],
        }
    }
}
impl<'de> Deserialize<'de> for Rule {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Default, Deserialize)]
        #[serde(default)]
        struct Stored {
            enabled: bool,
            rows: Option<Vec<MarkRow>>,
            patterns: Option<Vec<String>>,
            color: Option<u32>,
        }
        let stored = Stored::deserialize(deserializer)?;
        let rows = stored.rows.unwrap_or_else(|| {
            vec![MarkRow {
                patterns: stored
                    .patterns
                    .unwrap_or_else(|| MarkRow::default().patterns),
                color: stored.color.unwrap_or(PALETTE[0].1),
                ..MarkRow::default()
            }]
        });
        Ok(Self {
            enabled: stored.enabled,
            rows,
        })
    }
}
impl Rule {
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
    #[cfg(test)]
    fn parse(input: &str, color: u32, enabled: bool) -> Result<Self, String> {
        Ok(Self {
            enabled,
            rows: vec![MarkRow::parse(
                Target::Files,
                Kind::NewestVersion,
                input,
                color,
            )?],
        })
    }
}
struct Pattern {
    prefix: String,
    components: usize,
}
impl Pattern {
    fn parse(text: &str) -> Option<Self> {
        let text = text.to_ascii_lowercase();
        let at = text.find('x')?;
        let prefix = &text[..at];
        let parts: Vec<_> = text[at..].split('.').collect();
        if prefix.is_empty()
            || prefix.len() > 16
            || !prefix.bytes().all(|b| b.is_ascii_alphabetic())
            || parts.len() > 8
            || parts.iter().any(|p| *p != "x")
        {
            return None;
        }
        Some(Self {
            prefix: prefix.into(),
            components: parts.len(),
        })
    }
    fn split(&self, stem: &str) -> Option<(String, Version)> {
        let lower = stem.to_ascii_lowercase();
        let at = lower.rfind(&self.prefix)?;
        if at == 0 || !stem[..at].chars().next_back().is_some_and(separator) {
            return None;
        }
        let base = stem[..at].trim_end_matches(separator);
        if base.is_empty() {
            return None;
        }
        let numbers: Vec<_> = stem[at + self.prefix.len()..].split('.').collect();
        if numbers.len() != self.components
            || numbers
                .iter()
                .any(|n| n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()))
        {
            return None;
        }
        Some((
            base.to_lowercase(),
            Version(
                numbers
                    .into_iter()
                    .map(|n| {
                        let n = n.trim_start_matches('0');
                        if n.is_empty() {
                            "0".into()
                        } else {
                            n.into()
                        }
                    })
                    .collect(),
            ),
        ))
    }
}
fn separator(c: char) -> bool {
    c.is_whitespace() || matches!(c, '_' | '-' | '.')
}
#[derive(Clone, Debug)]
struct Version(Vec<String>);
impl Version {
    fn compare(&self, other: &Self) -> Cmp {
        for i in 0..self.0.len().max(other.0.len()) {
            let a = self.0.get(i).map(String::as_str).unwrap_or("0");
            let b = other.0.get(i).map(String::as_str).unwrap_or("0");
            let order = a.len().cmp(&b.len()).then_with(|| a.cmp(b));
            if order != Cmp::Equal {
                return order;
            }
        }
        Cmp::Equal
    }
}

/// Compare sibling files of the same base name and extension. A lone versioned
/// file is optional; all numerically tied highest versions are marked.
fn newest(
    patterns: &[String],
    files: impl IntoIterator<Item = PathBuf>,
    folders: bool,
    mark_single_version: bool,
) -> HashSet<PathBuf> {
    let patterns: Vec<_> = patterns
        .iter()
        .take(8)
        .filter_map(|p| Pattern::parse(p))
        .collect();
    let mut groups: HashMap<(PathBuf, String, String), (usize, Version, Vec<PathBuf>)> =
        HashMap::new();
    let mut seen = HashSet::new();
    for path in files {
        if !seen.insert(path.clone()) {
            continue;
        }
        let Some(name) = path
            .file_name()
            .and_then(|n| n.to_str())
            .filter(|n| n.len() <= 4096)
        else {
            continue;
        };
        let full = patterns
            .iter()
            .find_map(|p| p.split(name))
            .map(|pair| (pair, String::new()));
        let found = full.or_else(|| {
            if folders {
                return None;
            }
            let stem = path.file_stem()?.to_str()?;
            let extension = path.extension()?.to_str()?.to_lowercase();
            patterns
                .iter()
                .find_map(|p| p.split(stem))
                .map(|pair| (pair, extension))
        });
        let Some(((base, version), extension)) = found else {
            continue;
        };
        let key = (
            path.parent().unwrap_or(Path::new("")).to_path_buf(),
            base,
            extension,
        );
        if let Some((count, best, winners)) = groups.get_mut(&key) {
            *count += 1;
            match version.compare(best) {
                Cmp::Greater => {
                    *best = version;
                    *winners = vec![path];
                }
                Cmp::Equal => winners.push(path),
                Cmp::Less => {}
            }
        } else {
            groups.insert(key, (1, version, vec![path]));
        }
    }
    groups
        .into_values()
        .filter(|(count, _, _)| mark_single_version || *count > 1)
        .flat_map(|(_, _, paths)| paths)
        .collect()
}

fn literal_version(text: &str) -> Option<(Pattern, Version)> {
    let at = text.find(|c: char| c.is_ascii_digit())?;
    let prefix = &text[..at];
    if prefix.is_empty() || prefix.len() > 16 || !prefix.bytes().all(|b| b.is_ascii_alphabetic()) {
        return None;
    }
    let parts: Vec<_> = text[at..].split('.').collect();
    if parts.len() > 8
        || parts
            .iter()
            .any(|n| n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    Some((
        Pattern {
            prefix: prefix.to_ascii_lowercase(),
            components: parts.len(),
        },
        Version(
            parts
                .into_iter()
                .map(|n| {
                    let n = n.trim_start_matches('0');
                    if n.is_empty() {
                        "0".into()
                    } else {
                        n.into()
                    }
                })
                .collect(),
        ),
    ))
}
fn version_matches(path: &Path, patterns: &[(Pattern, Option<Version>)], folders: bool) -> bool {
    [
        path.file_name(),
        if folders { None } else { path.file_stem() },
    ]
    .into_iter()
    .flatten()
    .filter_map(|n| n.to_str())
    .any(|name| {
        patterns.iter().any(|(pattern, wanted)| {
            pattern.split(name).is_some_and(|(_, version)| {
                wanted
                    .as_ref()
                    .is_none_or(|wanted| version.compare(wanted) == Cmp::Equal)
            })
        })
    })
}
/// Topmost matching row wins. Only already-loaded file names participate.
pub(super) fn marks(
    rule: &Rule,
    items: impl IntoIterator<Item = (PathBuf, bool)>,
) -> HashMap<PathBuf, u32> {
    let mut result = HashMap::new();
    if !rule.enabled {
        return result;
    }
    let items: Vec<_> = items.into_iter().collect();
    for row in rule.rows.iter().take(MAX_RULES) {
        let files: Vec<_> = items
            .iter()
            .filter(|(_, folder)| row.target.accepts(*folder))
            .map(|(path, _)| path.clone())
            .collect();
        let matching: Vec<_> = match row.kind {
            Kind::NewestVersion => newest(
                &row.patterns,
                files.iter().cloned(),
                row.target == Target::Folders,
                row.mark_single_version,
            )
            .into_iter()
            .collect(),
            Kind::VersionMatch => {
                let patterns: Vec<_> = row
                    .patterns
                    .iter()
                    .take(8)
                    .filter_map(|p| {
                        Pattern::parse(p)
                            .map(|p| (p, None))
                            .or_else(|| literal_version(p).map(|(p, v)| (p, Some(v))))
                    })
                    .collect();
                files
                    .iter()
                    .filter(|p| version_matches(p, &patterns, row.target == Target::Folders))
                    .cloned()
                    .collect()
            }
            Kind::FilenameEnding => {
                let patterns: Vec<_> = row
                    .patterns
                    .iter()
                    .take(8)
                    .map(|p| p.to_lowercase())
                    .collect();
                files
                    .iter()
                    .filter(|p| {
                        p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                            let name = n.to_lowercase();
                            patterns.iter().any(|p| !p.is_empty() && name.ends_with(p))
                        })
                    })
                    .cloned()
                    .collect()
            }
        };
        for path in matching {
            result.entry(path).or_insert_with(|| row.marker_color());
        }
    }
    result
}

impl Tab {
    pub(super) fn refresh_version_marks(&mut self) {
        self.marked_versions = marks(
            &self.version_mark_rule,
            self.entries
                .iter()
                .map(|e| (self.current_dir.join(&e.name), e.is_folder())),
        );
    }
    fn apply_version_mark(&mut self, rule: &Rule) {
        if self.version_mark_rule != *rule {
            self.version_mark_rule = rule.clone();
            self.refresh_version_marks();
        }
    }
}
impl Schnauz {
    pub(super) fn sync_version_marks(&mut self) {
        let rule = self.projects.sidebar().version_mark.clone();
        for tab in self.panes.iter_mut().flat_map(|p| &mut p.tabs) {
            tab.apply_version_mark(&rule);
        }
        for (id, session) in &mut self.project_sessions {
            let rule = self
                .projects
                .sidebar_for(*id)
                .map(|s| s.version_mark.clone())
                .unwrap_or_default();
            for tab in session.panes.iter_mut().flat_map(|p| &mut p.tabs) {
                tab.apply_version_mark(&rule);
            }
        }
        let revision = (self.search_gen, self.palette_items.len(), rule.clone());
        if self.file_search.mark_revision.as_ref() != Some(&revision) {
            self.file_search.marked_versions = marks(
                &rule,
                self.palette_items.iter().map(|item| {
                    let Action::Open(path, is_dir) = &item.action;
                    (path.clone(), is_browsable_folder(path, *is_dir))
                }),
            );
            self.file_search.mark_revision = Some(revision);
        }
    }
    pub(super) fn version_marker(&self, pane: usize, path: &Path) -> Option<u32> {
        let tab = self.tab(pane);
        tab.marked_versions.get(path).copied()
    }
    fn save_version_mark(
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
        if sidebar.version_mark != *expected {
            return Err("Mark settings changed. Cancel and open Settings again.".into());
        }
        sidebar.version_mark = rule;
        self.persist_library(library, cx)?;
        self.sync_version_marks();
        cx.notify();
        Ok(())
    }
    pub(super) fn render_version_mark(&self, pane: usize, cx: &Context<Self>) -> AnyElement {
        let t = theme();
        let project = self.projects.active;
        let rule = &self.projects.sidebar().version_mark;
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(6.))
            .whitespace_nowrap()
            .child("Mark:")
            .child(
                div()
                    .flex_none()
                    .flex()
                    .p(px(1.))
                    .rounded_lg()
                    .bg(rgb(t.surface))
                    .children(
                        [
                            (true, "On", "version-mark-on"),
                            (false, "Off", "version-mark-off"),
                        ]
                        .into_iter()
                        .map(|(enabled, label, id)| {
                            let selected = enabled == rule.enabled;
                            div()
                                .id((id, pane))
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
                                .when(selected, |d| d.bg(rgb(t.bg)).shadow_sm())
                                .hover(|d| d.bg(rgb(t.hover)))
                                .children(ui_checks::probe(id))
                                .child(label)
                                .tooltip(tip("Apply this project's filename marking rules"))
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.observe_project_library(cx);
                                    let Some(old) = this
                                        .projects
                                        .sidebar_for(project)
                                        .map(|s| s.version_mark.clone())
                                    else {
                                        return;
                                    };
                                    let mut rule = old.clone();
                                    rule.enabled = enabled;
                                    if let Err(error) =
                                        this.save_version_mark(project, &old, rule, cx)
                                    {
                                        this.set_notice(error, cx);
                                    }
                                }))
                                .into_any_element()
                        }),
                    ),
            )
            .child(
                div()
                    .id(("version-mark-settings", pane))
                    .relative()
                    .flex_none()
                    .h(px(20.))
                    .px(px(6.))
                    .rounded_md()
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .hover(|d| d.bg(rgb(t.hover)))
                    .children(ui_checks::probe("version-mark-settings"))
                    .child("Settings…")
                    .tooltip(tip(
                        "Add version/ending rules and choose a text-marker color for each",
                    ))
                    .on_click(
                        cx.listener(|this, _: &ClickEvent, _, cx| this.open_version_mark(cx)),
                    ),
            )
            .into_any_element()
    }
    fn open_version_mark(&mut self, cx: &mut Context<Self>) {
        editor::open(self, cx);
    }
}
#[path = "version_mark_editor.rs"]
mod editor;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rows_have_independent_file_folder_colors_and_topmost_priority() {
        let row = |target, kind, text, color| MarkRow::parse(target, kind, text, color).unwrap();
        let mut rule = Rule {
            enabled: true,
            rows: vec![
                row(Target::Files, Kind::NewestVersion, "vX", PALETTE[0].1),
                row(Target::Files, Kind::FilenameEnding, "*.PDF", PALETTE[3].1),
                row(
                    Target::Folders,
                    Kind::FilenameEnding,
                    "_archive",
                    PALETTE[1].1,
                ),
            ],
        };
        let items = vec![
            (PathBuf::from("/a/Plan_v1.pdf"), false),
            (PathBuf::from("/a/Plan_v2.pdf"), false),
            (PathBuf::from("/a/old_archive"), true),
            (PathBuf::from("/a/file_archive"), false),
            (PathBuf::from("/a/folder.pdf"), true),
        ];
        let result = marks(&rule, items.clone());
        assert_eq!(
            result,
            HashMap::from([
                (PathBuf::from("/a/Plan_v1.pdf"), PALETTE[3].1),
                (PathBuf::from("/a/Plan_v2.pdf"), PALETTE[0].1),
                (PathBuf::from("/a/old_archive"), PALETTE[1].1),
            ])
        );
        rule.rows.swap(0, 1);
        assert_eq!(
            marks(&rule, items)[Path::new("/a/Plan_v2.pdf")],
            PALETTE[3].1
        );
        let folder_rule = Rule {
            enabled: true,
            rows: vec![row(
                Target::Folders,
                Kind::NewestVersion,
                "vX.X",
                PALETTE[4].1,
            )],
        };
        assert_eq!(
            marks(
                &folder_rule,
                [
                    (PathBuf::from("/a/Build v1.9"), true),
                    (PathBuf::from("/a/Build v1.10"), true)
                ]
            ),
            HashMap::from([(PathBuf::from("/a/Build v1.10"), PALETTE[4].1)])
        );
    }

    #[test]
    fn matching_versions_and_legacy_settings_preserve_intent() {
        let row =
            MarkRow::parse(Target::Files, Kind::VersionMatch, "v1, v2.1", PALETTE[2].1).unwrap();
        let rule = Rule {
            enabled: true,
            rows: vec![row],
        };
        let result = marks(
            &rule,
            ["A_V01.ai", "A_v2.1.ai", "A_v10.ai", "A_v2.10.ai"]
                .into_iter()
                .map(|n| (PathBuf::from(n), false)),
        );
        assert_eq!(
            result.into_keys().collect::<HashSet<_>>(),
            HashSet::from([PathBuf::from("A_V01.ai"), PathBuf::from("A_v2.1.ai")])
        );
        let legacy: Rule =
            serde_json::from_str(r#"{"enabled":true,"patterns":["revX"],"color":16760283}"#)
                .unwrap();
        assert!(legacy.enabled);
        assert_eq!(legacy.rows.len(), 1);
        assert_eq!(legacy.rows[0].target, Target::Files);
        assert_eq!(legacy.rows[0].kind, Kind::NewestVersion);
        assert_eq!(legacy.rows[0].patterns, ["revX"]);
        assert_eq!(legacy.rows[0].color, 16760283);
        let roundtrip: Rule =
            serde_json::from_slice(&serde_json::to_vec(&legacy).unwrap()).unwrap();
        assert_eq!(roundtrip, legacy);
        let empty: Rule = serde_json::from_str(r#"{"enabled":true,"rows":[]}"#).unwrap();
        assert!(empty.rows.is_empty());
        assert!(marks(&empty, [(PathBuf::from("A_v1.ai"), false)]).is_empty());
        assert!(MarkRow::parse(Target::Files, Kind::FilenameEnding, "*", PALETTE[0].1).is_err());
        assert!(
            MarkRow::parse(Target::Folders, Kind::FilenameEnding, "../*", PALETTE[0].1).is_err()
        );
    }

    fn marked(patterns: &str, names: &[&str]) -> HashSet<PathBuf> {
        marks(
            &Rule::parse(patterns, PALETTE[0].1, true).unwrap(),
            names.iter().map(|name| (PathBuf::from(name), false)),
        )
        .into_keys()
        .collect()
    }
    #[test]
    fn listing_refresh_updates_marks_without_changing_selection_or_names() {
        let root = PathBuf::from("/version-mark-fixture");
        let mut tab = Tab::new(root.clone());
        let entry = |name: &str, folder: bool| Entry {
            name: name.into(),
            is_dir: folder,
            size: 0,
            modified: None,
            created: None,
            mode: None,
            loaded: true,
            hidden_flag: false,
        };
        tab.entries = vec![
            entry("Plan_v1.ai", false),
            entry("Plan_v2.ai", false),
            entry("Plan_v3.ai", true),
        ];
        tab.selection.insert(root.join("Plan_v1.ai"));
        tab.anchor = Some(root.join("Plan_v1.ai"));
        let rule = Rule::parse("vX", PALETTE[2].1, true).unwrap();
        tab.apply_version_mark(&rule);
        assert_eq!(
            tab.marked_versions,
            HashMap::from([(root.join("Plan_v2.ai"), PALETTE[2].1)])
        );
        tab.entries.push(entry("Plan_v10.ai", false));
        tab.recompute_results();
        assert_eq!(
            tab.marked_versions,
            HashMap::from([(root.join("Plan_v10.ai"), PALETTE[2].1)])
        );
        assert_eq!(tab.selection, HashSet::from([root.join("Plan_v1.ai")]));
        assert_eq!(tab.anchor, Some(root.join("Plan_v1.ai")));
        let names: Vec<_> = tab.entries.iter().map(|e| e.name.clone()).collect();
        let mut disabled = rule;
        disabled.enabled = false;
        tab.apply_version_mark(&disabled);
        assert!(tab.marked_versions.is_empty());
        assert_eq!(
            names,
            tab.entries
                .iter()
                .map(|e| e.name.clone())
                .collect::<Vec<_>>()
        );
        let mut sidebar = projects::Sidebar::default();
        sidebar.version_mark = disabled;
        let restored: projects::Sidebar =
            serde_json::from_slice(&serde_json::to_vec(&sidebar).unwrap()).unwrap();
        assert_eq!(restored.version_mark, sidebar.version_mark);
    }

    #[test]
    fn numeric_versions_normalize_separators_and_compare_components() {
        let result = marked(
            "vX, vX.X",
            &[
                "/a/Report v1.pdf",
                "/a/Report_v2.pdf",
                "/a/Report-v10.pdf",
                "/a/Report v2.10.pdf",
                "/a/Report v2.9.pdf",
            ],
        );
        assert_eq!(result, HashSet::from([PathBuf::from("/a/Report-v10.pdf")]));
        assert_eq!(
            marked("vX.X", &["/a/设计 v1.9.ai", "/a/设计_V1.10.ai"]),
            HashSet::from([PathBuf::from("/a/设计_V1.10.ai")])
        );
    }
    #[test]
    fn ties_extensionless_files_and_large_numbers_are_stable() {
        let paths = ["/a/Plan_v2", "/a/Plan V02.0", "/a/Plan-v1"];
        assert_eq!(
            marked("vX,vX.X", &paths),
            HashSet::from([PathBuf::from(paths[0]), PathBuf::from(paths[1])])
        );
        let paths = [
            "/a/A v99999999999999999999.txt",
            "/a/A v100000000000000000000.txt",
        ];
        assert_eq!(
            marked("vX", &paths),
            HashSet::from([PathBuf::from(paths[1])])
        );
    }
    #[test]
    fn unrelated_files_folders_types_and_off_do_not_get_marks() {
        assert!(marked(
            "vX",
            &["/a/A v1.pdf", "/b/A v2.pdf", "/a/A v2.ai", "/a/rev1.pdf"]
        )
        .is_empty());
        assert!(marks(
            &Rule::default(),
            [
                (PathBuf::from("A v1"), false),
                (PathBuf::from("A v2"), false)
            ]
        )
        .is_empty());
        assert_eq!(
            marked("revX", &["A_rev1.txt", "A rev2.txt"]),
            HashSet::from([PathBuf::from("A rev2.txt")])
        );
        for pattern in ["", "v*", "v1", "X", "vX..X", "vX/X"] {
            assert!(Rule::parse(pattern, PALETTE[0].1, true).is_err());
        }
        let sidebar: projects::Sidebar = serde_json::from_str("{}").unwrap();
        assert_eq!(sidebar.version_mark, Rule::default());
    }

    #[test]
    fn single_version_option_is_per_rule_and_keeps_only_highest_versions() {
        for target in [Target::Files, Target::Folders] {
            let mut row = MarkRow::parse(target, Kind::NewestVersion, "vX", PALETTE[0].1).unwrap();
            let folder = target == Target::Folders;
            let items = ["Only_v1", "Pair_v1", "Pair_v2", "ordinary"]
                .map(|name| (PathBuf::from(name), folder));
            let rule = |row| Rule {
                enabled: true,
                rows: vec![row],
            };
            assert_eq!(
                marks(&rule(row.clone()), items.clone())
                    .keys()
                    .cloned()
                    .collect::<HashSet<_>>(),
                HashSet::from([PathBuf::from("Pair_v2")])
            );
            row.mark_single_version = true;
            let enabled = rule(row);
            assert_eq!(
                marks(&enabled, items)
                    .keys()
                    .cloned()
                    .collect::<HashSet<_>>(),
                HashSet::from([PathBuf::from("Only_v1"), PathBuf::from("Pair_v2")])
            );
            let saved: Rule =
                serde_json::from_str(&serde_json::to_string(&enabled).unwrap()).unwrap();
            assert_eq!(saved, enabled);
        }
        let old: MarkRow = serde_json::from_str(r#"{"patterns":["vX"]}"#).unwrap();
        assert!(!old.mark_single_version);
        let mut first = MarkRow::default();
        first.mark_single_version = true;
        let mut second = MarkRow::default();
        second.target = Target::Folders;
        let rule = Rule {
            enabled: true,
            rows: vec![first, second],
        };
        let marked = marks(
            &rule,
            [
                (PathBuf::from("File_v1"), false),
                (PathBuf::from("Folder_v1"), true),
            ],
        );
        assert!(marked.contains_key(Path::new("File_v1")));
        assert!(!marked.contains_key(Path::new("Folder_v1")));
    }
}
