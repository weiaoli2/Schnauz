//! Explicit, cancellable folder-size scans and a squarified treemap.
use std::{
    collections::{HashMap, HashSet},
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

// Bound retained detail, never the number of filesystem entries counted.
// At most one additional aggregate tile is retained per detailed directory.
const DETAIL_NODES: usize = 50_000;
#[derive(Clone)]
pub struct Node {
    pub path: PathBuf,
    pub parent: Option<usize>,
    pub directory: bool,
    pub bytes: u64,
    pub children: Vec<usize>,
    /// A synthetic tile representing counted files omitted from individual detail.
    pub grouped_files: usize,
}
#[derive(Clone)]
pub struct Tree {
    pub nodes: Vec<Node>,
    pub counts: Vec<usize>,
    pub unreadable: usize,
    pub grouped_files: usize,
    pub skipped_volumes: usize,
    pub capacity: Option<(u64, u64)>,
    pub scanning: bool,
    pub pending_folders: usize,
}
#[derive(Default, Clone, Copy)]
pub struct Progress {
    pub files: usize,
    pub bytes: u64,
    pub unreadable: usize,
}
struct Work {
    path: PathBuf,
    parent: usize,
    entries: Option<fs::ReadDir>,
}
struct Branch {
    path: PathBuf,
    work: std::collections::VecDeque<Work>,
    bytes: u64,
    generation: usize,
}
impl Branch {
    fn next(&mut self) -> Option<Result<(usize, fs::DirEntry), std::io::Error>> {
        let work = self.work.front_mut()?;
        if work.entries.is_none() {
            match fs::read_dir(&work.path) {
                Ok(entries) => work.entries = Some(entries),
                Err(error) => {
                    self.work.pop_front();
                    return Some(Err(error));
                }
            }
        }
        match work.entries.as_mut().unwrap().next() {
            Some(entry) => Some(entry.map(|entry| (work.parent, entry))),
            None => {
                self.work.pop_front();
                None
            }
        }
    }
    fn prioritise(&mut self, path: &Path) {
        if let Some(index) = self
            .work
            .iter()
            .position(|work| path.starts_with(&work.path) || work.path.starts_with(path))
        {
            self.work.swap(0, index);
        }
    }
}

struct Scan {
    tree: Tree,
    grouped: Vec<(u64, usize)>,
    directories: HashSet<(u64, u64)>,
    progress: Progress,
    device: u64,
    boot_data: Option<u64>,
    detail_budget: usize,
    checked: usize,
    files: HashMap<usize, std::collections::BinaryHeap<std::cmp::Reverse<(u64, usize)>>>,
}
impl Scan {
    fn record(&mut self, parent: usize, entry: fs::DirEntry) -> Option<Work> {
        self.checked += 1;
        let path = entry.path();
        let md = match fs::symlink_metadata(&path) {
            Ok(md) => md,
            Err(_) => {
                self.progress.unreadable += 1;
                return None;
            }
        };
        if self.boot_data.is_some() && path == Path::new("/System/Volumes/Data") {
            return None;
        }
        if !allowed_device(md.dev(), self.device, self.boot_data) {
            self.tree.skipped_volumes += 1;
            return None;
        }
        let directory = md.is_dir();
        if directory && !self.directories.insert((md.dev(), md.ino())) {
            return None;
        }
        if !directory && !md.is_file() && !md.file_type().is_symlink() {
            return None;
        }
        let bytes = if directory { 0 } else { md.len() };
        let child = if self.tree.nodes.len() < self.detail_budget.max(1) {
            let index = self.tree.nodes.len();
            self.tree.nodes.push(Node {
                path: path.clone(),
                parent: Some(parent),
                directory,
                bytes,
                children: Vec::new(),
                grouped_files: 0,
            });
            self.tree.nodes[parent].children.push(index);
            self.grouped.push((0, 0));
            if !directory {
                self.files
                    .entry(parent)
                    .or_default()
                    .push(std::cmp::Reverse((bytes, index)));
            }
            index
        } else {
            if !directory {
                // Keep the largest files visible even if discovered late.
                // Smaller file detail is folded into its containing group.
                let replace = self
                    .files
                    .get(&parent)
                    .and_then(|files| files.peek())
                    .copied()
                    .filter(|smallest| smallest.0 .0 < bytes);
                let grouped_bytes = if let Some(std::cmp::Reverse((old_bytes, index))) = replace {
                    let files = self.files.get_mut(&parent).unwrap();
                    files.pop();
                    files.push(std::cmp::Reverse((bytes, index)));
                    self.tree.nodes[index].path = path.clone();
                    self.tree.nodes[index].bytes = bytes;
                    old_bytes
                } else {
                    bytes
                };
                self.grouped[parent].0 = self.grouped[parent].0.saturating_add(grouped_bytes);
                self.grouped[parent].1 = self.grouped[parent].1.saturating_add(1);
            }
            parent
        };
        if !directory {
            self.progress.files += 1;
            self.progress.bytes = self.progress.bytes.saturating_add(bytes);
        }
        directory.then_some(Work {
            path,
            parent: child,
            entries: None,
        })
    }
    fn snapshot(&self, cancel: &AtomicBool) -> Option<Tree> {
        let mut tree = self.tree.clone();
        tree.unreadable = self.progress.unreadable;
        finish_tree(tree, &self.grouped, cancel)
    }
}

#[cfg(test)]
pub fn scan(root: &Path, cancel: &AtomicBool, report: impl FnMut(Progress)) -> Option<Tree> {
    scan_core(
        root,
        cancel,
        report,
        |_, _| {},
        &std::sync::Mutex::new(None),
        DETAIL_NODES,
        false,
        50_000,
    )
}

pub fn scan_progressive(
    root: &Path,
    cancel: &AtomicBool,
    report: impl FnMut(Progress),
    snapshot: impl FnMut(Tree, Progress),
    priority: &std::sync::Mutex<Option<PathBuf>>,
) -> Option<Tree> {
    scan_core(
        root,
        cancel,
        report,
        snapshot,
        priority,
        DETAIL_NODES,
        true,
        50_000,
    )
}

#[cfg(test)]
fn scan_with_detail_budget(
    root: &Path,
    cancel: &AtomicBool,
    report: impl FnMut(Progress),
    detail_budget: usize,
) -> Option<Tree> {
    scan_core(
        root,
        cancel,
        report,
        |_, _| {},
        &std::sync::Mutex::new(None),
        detail_budget,
        false,
        50_000,
    )
}

fn scan_core(
    root: &Path,
    cancel: &AtomicBool,
    mut report: impl FnMut(Progress),
    mut snapshot: impl FnMut(Tree, Progress),
    priority: &std::sync::Mutex<Option<PathBuf>>,
    detail_budget: usize,
    progressive: bool,
    snapshot_every: usize,
) -> Option<Tree> {
    let resolved = fs::canonicalize(root).ok()?;
    let root = resolved.as_path();
    let root_meta = fs::symlink_metadata(root).ok()?;
    if !root_meta.is_dir() {
        return None;
    }
    let device = root_meta.dev();
    let boot_data = if root == Path::new("/") {
        fs::metadata("/System/Volumes/Data").ok().map(|m| m.dev())
    } else {
        None
    };
    let mut scan = Scan {
        tree: Tree {
            nodes: vec![Node {
                path: root.into(),
                parent: None,
                directory: true,
                bytes: 0,
                children: Vec::new(),
                grouped_files: 0,
            }],
            counts: Vec::new(),
            unreadable: 0,
            grouped_files: 0,
            skipped_volumes: 0,
            capacity: volume_capacity(root),
            scanning: true,
            pending_folders: 0,
        },
        grouped: vec![(0, 0)],
        directories: HashSet::from([(device, root_meta.ino())]),
        progress: Progress::default(),
        device,
        boot_data,
        detail_budget,
        checked: 0,
        files: HashMap::new(),
    };
    let mut branches = Vec::new();
    let mut last_report = Instant::now();
    let mut last_snapshot = Instant::now();
    let mut last_checked = 0;
    let mut has_data_snapshot = false;
    // Root names are discovered before descending. Directory sizes are not
    // guessed: all in-progress sizes are explicitly bytes counted so far.
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries {
            if cancel.load(Ordering::Acquire) {
                return None;
            }
            match entry {
                Ok(entry) => {
                    if let Some(work) = scan.record(0, entry) {
                        branches.push(Branch {
                            path: work.path.clone(),
                            work: std::collections::VecDeque::from([work]),
                            bytes: 0,
                            generation: 0,
                        });
                    }
                }
                Err(_) => scan.progress.unreadable += 1,
            }
            if progressive
                && (scan.checked - last_checked >= snapshot_every.max(1)
                    || last_snapshot.elapsed() >= Duration::from_millis(250))
            {
                scan.tree.pending_folders = branches.len();
                snapshot(scan.snapshot(cancel)?, scan.progress);
                has_data_snapshot |= scan.progress.bytes > 0;
                last_snapshot = Instant::now();
                last_checked = scan.checked;
            }
        }
    } else {
        scan.progress.unreadable += 1;
    }
    scan.tree.pending_folders = branches.len();
    if progressive {
        snapshot(scan.snapshot(cancel)?, scan.progress);
        has_data_snapshot |= scan.progress.bytes > 0;
        last_snapshot = Instant::now();
        last_checked = scan.checked;
    }
    let mut heap = std::collections::BinaryHeap::new();
    for index in 0..branches.len() {
        heap.push((0u64, std::cmp::Reverse(index), 0usize));
    }
    let mut first_pass = 0;
    let mut fair_cursor = 0;
    let mut batches = 0;
    loop {
        if cancel.load(Ordering::Acquire) {
            return None;
        }
        let requested = priority.lock().unwrap().clone();
        let selected = requested.as_ref().and_then(|path| {
            branches
                .iter()
                .position(|b| !b.work.is_empty() && path.starts_with(&b.path))
        });
        while first_pass < branches.len() && branches[first_pass].work.is_empty() {
            first_pass += 1;
        }
        let fair = if batches % 16 == 15 && !branches.is_empty() {
            let found = (0..branches.len())
                .map(|i| (fair_cursor + i) % branches.len())
                .find(|i| !branches[*i].work.is_empty());
            if let Some(index) = found {
                fair_cursor = (index + 1) % branches.len();
            }
            found
        } else {
            None
        };
        let index = if let Some(index) = selected
            .or_else(|| (first_pass < branches.len()).then_some(first_pass))
            .or(fair)
        {
            index
        } else {
            let mut next = None;
            while let Some((_, std::cmp::Reverse(index), generation)) = heap.pop() {
                if !branches[index].work.is_empty() && generation == branches[index].generation {
                    next = Some(index);
                    break;
                }
            }
            let Some(index) = next else {
                break;
            };
            index
        };
        if index == first_pass {
            first_pass += 1;
        }
        let branch = &mut branches[index];
        if let Some(path) = requested
            .as_ref()
            .filter(|path| path.starts_with(&branch.path))
        {
            branch.prioritise(path);
        }
        let before = scan.progress.bytes;
        let slice_start = Instant::now();
        // A short slice keeps click-priority and cancellation responsive.
        for _ in 0..256 {
            if slice_start.elapsed() >= Duration::from_millis(25) {
                break;
            }
            if cancel.load(Ordering::Acquire) {
                return None;
            }
            match branch.next() {
                Some(Ok((parent, entry))) => {
                    if let Some(work) = scan.record(parent, entry) {
                        branch.work.push_back(work);
                    }
                }
                Some(Err(_)) => scan.progress.unreadable += 1,
                None => {
                    if branch.work.is_empty() {
                        break;
                    }
                }
            }
        }
        branch.bytes = branch
            .bytes
            .saturating_add(scan.progress.bytes.saturating_sub(before));
        branch.generation += 1;
        if !branch.work.is_empty() {
            heap.push((branch.bytes, std::cmp::Reverse(index), branch.generation));
        }
        batches += 1;
        if last_report.elapsed() >= Duration::from_millis(150) {
            report(scan.progress);
            last_report = Instant::now();
        }
        if progressive
            && (scan.checked - last_checked >= snapshot_every.max(1)
                || last_snapshot.elapsed()
                    >= if has_data_snapshot {
                        Duration::from_secs(1)
                    } else {
                        Duration::from_millis(250)
                    })
        {
            scan.tree.pending_folders = branches.iter().map(|branch| branch.work.len()).sum();
            snapshot(scan.snapshot(cancel)?, scan.progress);
            has_data_snapshot |= scan.progress.bytes > 0;
            last_snapshot = Instant::now();
            last_checked = scan.checked;
        }
    }
    scan.tree.scanning = false;
    scan.tree.pending_folders = 0;
    scan.tree.unreadable = scan.progress.unreadable;
    report(scan.progress);
    finish_tree(scan.tree, &scan.grouped, cancel)
}

fn finish_tree(mut tree: Tree, grouped: &[(u64, usize)], cancel: &AtomicBool) -> Option<Tree> {
    for (parent, &(bytes, count)) in grouped.iter().enumerate() {
        if cancel.load(Ordering::Acquire) {
            return None;
        }
        if count == 0 {
            continue;
        }
        let index = tree.nodes.len();
        tree.nodes.push(Node {
            path: tree.nodes[parent].path.clone(),
            parent: Some(parent),
            directory: false,
            bytes,
            children: Vec::new(),
            grouped_files: count,
        });
        tree.nodes[parent].children.push(index);
        tree.grouped_files += count;
    }
    for i in (1..tree.nodes.len()).rev() {
        if cancel.load(Ordering::Acquire) {
            return None;
        }
        let parent = tree.nodes[i].parent.unwrap();
        tree.nodes[parent].bytes = tree.nodes[parent].bytes.saturating_add(tree.nodes[i].bytes);
    }
    let sizes: Vec<u64> = tree.nodes.iter().map(|node| node.bytes).collect();
    for node in &mut tree.nodes {
        node.children
            .sort_unstable_by(|a, b| sizes[*b].cmp(&sizes[*a]).then_with(|| a.cmp(b)));
    }
    let mut counts: Vec<usize> = tree
        .nodes
        .iter()
        .map(|node| {
            if node.grouped_files > 0 {
                node.grouped_files
            } else {
                usize::from(!node.directory)
            }
        })
        .collect();
    for i in (1..tree.nodes.len()).rev() {
        counts[tree.nodes[i].parent.unwrap()] += counts[i];
    }
    tree.counts = counts;
    Some(tree)
}

fn allowed_device(device: u64, root: u64, boot_data: Option<u64>) -> bool {
    device == root || boot_data == Some(device)
}

// Capacity is filesystem metadata, not a recursive scan or a TCC access probe.
fn volume_capacity(root: &Path) -> Option<(u64, u64)> {
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(root.as_os_str().as_bytes()).ok()?;
    let mut info = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    if unsafe { libc::statvfs(path.as_ptr(), info.as_mut_ptr()) } != 0 {
        return None;
    }
    let info = unsafe { info.assume_init() };
    let block = info.f_frsize as u64;
    Some((
        (info.f_blocks as u64).saturating_mul(block),
        (info.f_bavail as u64).saturating_mul(block),
    ))
}

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}
impl Rect {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
}
#[derive(Clone)]
pub struct Tile {
    pub node: usize,
    pub rect: Rect,
    pub folder: bool,
}

/// Squarify siblings by their byte weights. Folder header bands distinguish
/// navigation targets from file tiles; small folders collapse into zoomable tiles.
pub fn layout(tree: &Tree, root: usize, bounds: Rect) -> Vec<Tile> {
    let mut tiles = Vec::new();
    let mut pending = vec![(root, bounds, 0)];
    while let Some((index, mut rect, depth)) = pending.pop() {
        let node = &tree.nodes[index];
        if rect.w < 2. || rect.h < 2. || node.bytes == 0 {
            continue;
        }
        if !node.directory {
            tiles.push(Tile {
                node: index,
                rect,
                folder: false,
            });
            continue;
        }
        if index != root {
            if node.children.is_empty() || rect.w < 60. || rect.h < 42. || depth >= 8 {
                tiles.push(Tile {
                    node: index,
                    rect,
                    folder: true,
                });
                continue;
            }
            tiles.push(Tile {
                node: index,
                rect: Rect { h: 18., ..rect },
                folder: true,
            });
            rect = Rect {
                x: rect.x + 1.,
                y: rect.y + 19.,
                w: (rect.w - 2.).max(0.),
                h: (rect.h - 20.).max(0.),
            };
        }
        let total = node.bytes as f64;
        let children: Vec<(usize, f64)> = node
            .children
            .iter()
            .filter(|i| tree.nodes[**i].bytes > 0)
            .map(|i| {
                (
                    *i,
                    tree.nodes[*i].bytes as f64 / total * rect.w as f64 * rect.h as f64,
                )
            })
            .collect();
        let mut start = 0;
        while start < children.len() && rect.w > 0. && rect.h > 0. {
            let side = rect.w.min(rect.h) as f64;
            let mut end = start + 1;
            let mut sum = children[start].1;
            let mut worst = ratio(sum, children[start].1, children[start].1, side);
            while end < children.len() {
                let candidate = sum + children[end].1;
                let score = ratio(candidate, children[start].1, children[end].1, side);
                if score > worst {
                    break;
                }
                sum = candidate;
                worst = score;
                end += 1;
            }
            if rect.w >= rect.h {
                let width = (sum / rect.h as f64) as f32;
                let mut y = rect.y;
                for &(child, area) in &children[start..end] {
                    let h = (area / width as f64) as f32;
                    pending.push((
                        child,
                        Rect {
                            x: rect.x,
                            y,
                            w: width,
                            h,
                        },
                        depth + 1,
                    ));
                    y += h;
                }
                rect.x += width;
                rect.w = (rect.w - width).max(0.);
            } else {
                let height = (sum / rect.w as f64) as f32;
                let mut x = rect.x;
                for &(child, area) in &children[start..end] {
                    let w = (area / height as f64) as f32;
                    pending.push((
                        child,
                        Rect {
                            x,
                            y: rect.y,
                            w,
                            h: height,
                        },
                        depth + 1,
                    ));
                    x += w;
                }
                rect.y += height;
                rect.h = (rect.h - height).max(0.);
            }
            start = end;
        }
    }
    tiles
}
fn ratio(sum: f64, max: f64, min: f64, side: f64) -> f64 {
    let square = sum * sum;
    let side = side * side;
    (side * max / square).max(square / (side * min))
}
pub fn hit(tiles: &[Tile], x: f32, y: f32) -> Option<usize> {
    tiles
        .iter()
        .rev()
        .find(|t| t.rect.contains(x, y))
        .map(|t| t.node)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn early_map_and_clicked_branch_priority_precede_other_descendants() {
        let root =
            fs::canonicalize(crate::tests::scratch("space-map-progressive-priority")).unwrap();
        for name in ["first", "second"] {
            fs::create_dir(root.join(name)).unwrap();
            fs::write(root.join(name).join("data"), vec![0; 1024]).unwrap();
        }
        let priority = std::sync::Mutex::new(None);
        let cancel = AtomicBool::new(false);
        let mut target = None;
        let mut first_data = None;
        let mut snapshots = 0;
        let final_tree = scan_core(
            &root,
            &cancel,
            |_| {},
            |tree, progress| {
                snapshots += 1;
                assert!(tree.scanning);
                assert_eq!(tree.nodes[0].bytes, progress.bytes);
                assert_eq!(tree.counts[0], progress.files);
                if target.is_none() && tree.nodes[0].children.len() == 2 {
                    // Select the last discovered sibling, so ordinary first-pass
                    // order would read the other one first without click priority.
                    let last = *tree.nodes[0].children.last().unwrap();
                    let path = tree.nodes[last].path.clone();
                    *priority.lock().unwrap() = Some(path.clone());
                    target = Some(path);
                    assert_eq!(
                        progress.files, 0,
                        "An overview arrives before subtree scanning"
                    );
                }
                if progress.files > 0 && first_data.is_none() {
                    first_data = tree
                        .nodes
                        .iter()
                        .find(|node| node.directory && node.path != root && node.bytes > 0)
                        .map(|node| node.path.clone());
                }
            },
            &priority,
            100,
            true,
            1,
        )
        .unwrap();
        assert!(snapshots >= 3);
        assert_eq!(
            first_data, target,
            "A clicked branch is counted before its sibling"
        );
        assert_eq!(final_tree.nodes[0].bytes, 2048);
        assert_eq!(final_tree.counts[0], 2);
        assert!(!final_tree.scanning);
        assert_eq!(final_tree.pending_folders, 0);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn largest_files_stay_visible_when_detail_storage_is_full() {
        let root = fs::canonicalize(crate::tests::scratch("space-map-largest-files")).unwrap();
        for index in 0..40 {
            fs::write(root.join(format!("small-{index}")), [0; 10]).unwrap();
        }
        fs::write(root.join("large"), vec![0; 10000]).unwrap();
        let tree = scan_with_detail_budget(&root, &AtomicBool::new(false), |_| {}, 2).unwrap();
        assert_eq!(tree.nodes[0].bytes, 10400);
        assert_eq!(tree.counts[0], 41);
        assert!(tree.nodes.iter().any(|node| node.grouped_files == 0
            && !node.directory
            && node.path == root.join("large")));
        assert_eq!(tree.grouped_files, 40);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn boot_scope_includes_only_system_and_paired_data_device() {
        assert!(allowed_device(1, 1, Some(2)));
        assert!(allowed_device(2, 1, Some(2)));
        assert!(!allowed_device(3, 1, Some(2)));
        assert!(!allowed_device(2, 1, None));
    }

    #[test]
    fn detail_budget_never_truncates_scan_totals() {
        let root = crate::tests::scratch("space-map-limit");
        fs::write(root.join("a"), vec![0; 100]).unwrap();
        fs::write(root.join("b"), vec![0; 200]).unwrap();
        let alias = root.with_extension("alias");
        std::os::unix::fs::symlink(&root, &alias).unwrap();
        let cancel = AtomicBool::new(false);
        let complete = scan(&alias, &cancel, |_| {}).unwrap();
        assert_eq!(complete.nodes[0].bytes, 300);
        assert_eq!(complete.grouped_files, 0);
        assert!(complete
            .capacity
            .is_some_and(|(total, free)| total > 0 && free <= total));
        let grouped = scan_with_detail_budget(&root, &cancel, |_| {}, 2).unwrap();
        assert_eq!(
            grouped.nodes[0].bytes, 300,
            "Detail budget must never stop counting bytes"
        );
        assert_eq!(grouped.counts[0], 2, "Every file still counts");
        assert_eq!(grouped.grouped_files, 1);
        assert!(grouped.nodes.len() <= 4);
        fs::remove_file(alias).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn grouped_subtrees_preserve_all_files_bytes_and_map_area_without_following_links() {
        let root = crate::tests::scratch("space-map-complete-grouping");
        let mut expected_bytes = 0;
        for directory in 0..8 {
            let folder = root.join(format!("folder-{directory}"));
            fs::create_dir_all(&folder).unwrap();
            for file in 0..12 {
                let bytes = (directory * 12 + file + 1) as u64;
                fs::write(folder.join(format!("file-{file}")), vec![0; bytes as usize]).unwrap();
                expected_bytes += bytes;
            }
        }
        std::os::unix::fs::symlink(&root, root.join("loop")).unwrap();
        expected_bytes += fs::symlink_metadata(root.join("loop")).unwrap().len();
        let cancel = AtomicBool::new(false);
        for budget in [1, 2, 5, 20, 200] {
            let mut final_progress = Progress::default();
            let tree =
                scan_with_detail_budget(&root, &cancel, |p| final_progress = p, budget).unwrap();
            assert_eq!(
                tree.nodes[0].bytes, expected_bytes,
                "Every subtree is scanned even with budget {budget}"
            );
            assert_eq!(tree.counts[0], 97);
            assert_eq!(final_progress.files, 97);
            assert_eq!(final_progress.bytes, expected_bytes);
            assert_eq!(tree.unreadable, 0);
            assert!(
                tree.nodes.len() <= budget * 2,
                "Retained details stay bounded"
            );
            let tiles = layout(
                &tree,
                0,
                Rect {
                    x: 0.,
                    y: 0.,
                    w: 1000.,
                    h: 1000.,
                },
            );
            assert!(
                tiles
                    .iter()
                    .any(|tile| tree.nodes[tile.node].grouped_files > 0)
                    || budget == 200
            );
            for (index, node) in tree
                .nodes
                .iter()
                .enumerate()
                .filter(|(_, node)| node.grouped_files > 0)
            {
                assert_eq!(node.path, tree.nodes[node.parent.unwrap()].path);
                assert_eq!(tree.counts[index], node.grouped_files);
            }
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn scans_exact_sizes_hidden_files_and_cancels_without_following_links() {
        let root = crate::tests::scratch("space-map");
        fs::create_dir_all(root.join("inside")).unwrap();
        fs::write(root.join("one.txt"), vec![0; 100]).unwrap();
        fs::write(root.join("inside/.hidden"), vec![0; 300]).unwrap();
        std::os::unix::fs::symlink(&root, root.join("inside/loop")).unwrap();
        let cancel = AtomicBool::new(false);
        let tree = scan(&root, &cancel, |_| {}).unwrap();
        let link_size = fs::symlink_metadata(root.join("inside/loop"))
            .unwrap()
            .len();
        assert_eq!(tree.nodes[0].bytes, 400 + link_size);
        assert_eq!(tree.counts[0], 3);
        assert_eq!(tree.nodes.len(), 5);
        cancel.store(true, Ordering::Release);
        assert!(scan(&root, &cancel, |_| {}).is_none());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn tiles_preserve_area_proportions_bounds_and_file_folder_hit_targets() {
        let nodes = vec![
            Node {
                path: "/root".into(),
                parent: None,
                directory: true,
                bytes: 100,
                children: vec![1, 2, 3],
                grouped_files: 0,
            },
            Node {
                path: "/root/a".into(),
                parent: Some(0),
                directory: false,
                bytes: 50,
                children: vec![],
                grouped_files: 0,
            },
            Node {
                path: "/root/b".into(),
                parent: Some(0),
                directory: false,
                bytes: 30,
                children: vec![],
                grouped_files: 0,
            },
            Node {
                path: "/root/c".into(),
                parent: Some(0),
                directory: true,
                bytes: 20,
                children: vec![],
                grouped_files: 0,
            },
        ];
        let tree = Tree {
            nodes,
            counts: vec![2, 1, 1, 0],
            unreadable: 0,
            grouped_files: 0,
            skipped_volumes: 0,
            capacity: None,
            scanning: false,
            pending_folders: 0,
        };
        let tiles = layout(
            &tree,
            0,
            Rect {
                x: 0.,
                y: 0.,
                w: 100.,
                h: 100.,
            },
        );
        assert_eq!(tiles.len(), 3);
        for tile in &tiles {
            let r = tile.rect;
            assert!(r.x >= 0. && r.y >= 0. && r.x + r.w <= 100.01 && r.y + r.h <= 100.01);
            assert!((r.w * r.h - tree.nodes[tile.node].bytes as f32 * 100.).abs() < 0.05);
            assert_eq!(hit(&tiles, r.x + r.w / 2., r.y + r.h / 2.), Some(tile.node));
            assert_eq!(tile.folder, tree.nodes[tile.node].directory);
        }
    }
}
