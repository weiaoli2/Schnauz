//! Explicit, cancellable folder-size scans and a squarified treemap.
use std::{
    collections::HashSet,
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

pub const MAX_NODES: usize = 500_000;
#[derive(Clone)]
pub struct Node {
    pub path: PathBuf,
    pub parent: Option<usize>,
    pub directory: bool,
    pub bytes: u64,
    pub children: Vec<usize>,
}
pub struct Tree {
    pub nodes: Vec<Node>,
    pub counts: Vec<usize>,
    pub unreadable: usize,
    pub limited: bool,
    pub skipped_volumes: usize,
    pub capacity: Option<(u64, u64)>,
}
#[derive(Default, Clone, Copy)]
pub struct Progress {
    pub files: usize,
    pub bytes: u64,
    pub unreadable: usize,
}
pub fn scan(root: &Path, cancel: &AtomicBool, report: impl FnMut(Progress)) -> Option<Tree> {
    scan_with_limit(root, cancel, report, MAX_NODES)
}

fn scan_with_limit(
    root: &Path,
    cancel: &AtomicBool,
    mut report: impl FnMut(Progress),
    max_nodes: usize,
) -> Option<Tree> {
    // Resolve an explicitly chosen volume alias (e.g. /Volumes/Macintosh HD).
    // Links encountered inside the scan are still never followed.
    let resolved = fs::canonicalize(root).ok()?;
    let root = resolved.as_path();
    let root_meta = fs::symlink_metadata(root).ok()?;
    if !root_meta.is_dir() {
        return None;
    }
    let device = root_meta.dev();
    // The boot namespace crosses APFS firmlinks into its paired Data volume.
    // Other mounts must remain outside this explicit scan.
    let boot_data = if root == Path::new("/") {
        fs::metadata("/System/Volumes/Data").ok().map(|m| m.dev())
    } else {
        None
    };
    let mut directories = HashSet::from([(device, root_meta.ino())]);
    let mut tree = Tree {
        nodes: vec![Node {
            path: root.into(),
            parent: None,
            directory: true,
            bytes: 0,
            children: Vec::new(),
        }],
        counts: Vec::new(),
        unreadable: 0,
        limited: false,
        skipped_volumes: 0,
        capacity: volume_capacity(root),
    };
    let mut stack = vec![0];
    let mut progress = Progress::default();
    let mut last = Instant::now();
    'walk: while let Some(parent) = stack.pop() {
        if cancel.load(Ordering::Acquire) {
            return None;
        }
        let entries = match fs::read_dir(&tree.nodes[parent].path) {
            Ok(e) => e,
            Err(_) => {
                progress.unreadable += 1;
                continue;
            }
        };
        for entry in entries {
            if cancel.load(Ordering::Acquire) {
                return None;
            }
            if tree.nodes.len() >= max_nodes {
                tree.limited = true;
                break 'walk;
            }
            let Ok(entry) = entry else {
                progress.unreadable += 1;
                continue;
            };
            let Ok(md) = entry.metadata() else {
                progress.unreadable += 1;
                continue;
            };
            // Data is already exposed through the merged boot namespace. Do not
            // scan its backing mount again or include unrelated mounted volumes.
            if boot_data.is_some() && entry.path() == Path::new("/System/Volumes/Data") {
                continue;
            }
            if !allowed_device(md.dev(), device, boot_data) {
                tree.skipped_volumes += 1;
                continue;
            }
            let directory = md.is_dir();
            if directory && !directories.insert((md.dev(), md.ino())) {
                continue;
            }
            if !directory && !md.is_file() && !md.file_type().is_symlink() {
                continue;
            }
            let bytes = if directory { 0 } else { md.len() };
            let index = tree.nodes.len();
            tree.nodes.push(Node {
                path: entry.path(),
                parent: Some(parent),
                directory,
                bytes,
                children: Vec::new(),
            });
            tree.nodes[parent].children.push(index);
            if directory {
                stack.push(index);
            } else {
                progress.files += 1;
                progress.bytes = progress.bytes.saturating_add(bytes);
            }
            if last.elapsed() >= Duration::from_millis(150) {
                report(progress);
                last = Instant::now();
            }
        }
    }
    for i in (1..tree.nodes.len()).rev() {
        if cancel.load(Ordering::Acquire) {
            return None;
        }
        let parent = tree.nodes[i].parent.unwrap();
        tree.nodes[parent].bytes = tree.nodes[parent].bytes.saturating_add(tree.nodes[i].bytes);
    }
    let sizes: Vec<u64> = tree.nodes.iter().map(|n| n.bytes).collect();
    for node in &mut tree.nodes {
        node.children
            .sort_unstable_by(|a, b| sizes[*b].cmp(&sizes[*a]).then_with(|| a.cmp(b)));
    }
    let mut counts: Vec<usize> = tree
        .nodes
        .iter()
        .map(|n| usize::from(!n.directory))
        .collect();
    for i in (1..tree.nodes.len()).rev() {
        counts[tree.nodes[i].parent.unwrap()] += counts[i];
    }
    tree.counts = counts;
    tree.unreadable = progress.unreadable;
    report(progress);
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
            if rect.w < 60. || rect.h < 42. || depth >= 8 {
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
    fn boot_scope_includes_only_system_and_paired_data_device() {
        assert!(allowed_device(1, 1, Some(2)));
        assert!(allowed_device(2, 1, Some(2)));
        assert!(!allowed_device(3, 1, Some(2)));
        assert!(!allowed_device(2, 1, None));
    }

    #[test]
    fn explicit_root_alias_and_safety_limit_have_honest_totals() {
        let root = crate::tests::scratch("space-map-limit");
        fs::write(root.join("a"), vec![0; 100]).unwrap();
        fs::write(root.join("b"), vec![0; 200]).unwrap();
        let alias = root.with_extension("alias");
        std::os::unix::fs::symlink(&root, &alias).unwrap();
        let cancel = AtomicBool::new(false);
        let complete = scan(&alias, &cancel, |_| {}).unwrap();
        assert_eq!(complete.nodes[0].bytes, 300);
        assert!(!complete.limited);
        assert!(complete
            .capacity
            .is_some_and(|(total, free)| total > 0 && free <= total));
        let partial = scan_with_limit(&root, &cancel, |_| {}, 2).unwrap();
        assert!(partial.limited);
        assert_eq!(partial.nodes.len(), 2);
        assert_eq!(partial.counts[0], 1);
        assert_eq!(partial.nodes[0].bytes, partial.nodes[1].bytes);
        fs::remove_file(alias).unwrap();
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
            },
            Node {
                path: "/root/a".into(),
                parent: Some(0),
                directory: false,
                bytes: 50,
                children: vec![],
            },
            Node {
                path: "/root/b".into(),
                parent: Some(0),
                directory: false,
                bytes: 30,
                children: vec![],
            },
            Node {
                path: "/root/c".into(),
                parent: Some(0),
                directory: true,
                bytes: 20,
                children: vec![],
            },
        ];
        let tree = Tree {
            nodes,
            counts: vec![2, 1, 1, 0],
            unreadable: 0,
            limited: false,
            skipped_volumes: 0,
            capacity: None,
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
