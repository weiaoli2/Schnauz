//! Explicit, cancellable folder-size scans and a squarified treemap.
use std::{
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
}
#[derive(Default, Clone, Copy)]
pub struct Progress {
    pub files: usize,
    pub bytes: u64,
    pub unreadable: usize,
}
pub fn scan(root: &Path, cancel: &AtomicBool, mut report: impl FnMut(Progress)) -> Option<Tree> {
    let root_meta = fs::symlink_metadata(root).ok()?;
    if !root_meta.is_dir() {
        return None;
    }
    let device = root_meta.dev();
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
            if tree.nodes.len() >= MAX_NODES {
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
            // Metadata only: no file reads/downloads, directory symlink traversal or other volumes.
            if md.dev() != device {
                continue;
            }
            let directory = md.is_dir();
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
