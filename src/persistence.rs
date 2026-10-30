//! Ordered latest-value writes shared by autosaves and explicit project saves.
use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, OnceLock,
    },
};
#[derive(Default)]
struct Slot {
    pending: Mutex<Option<(u64, Vec<u8>)>>,
    revision: AtomicU64,
    commit: Mutex<u64>,
}
type Shared = Arc<Slot>;
fn stores() -> &'static Mutex<HashMap<PathBuf, Shared>> {
    static STORES: OnceLock<Mutex<HashMap<PathBuf, Shared>>> = OnceLock::new();
    STORES.get_or_init(Default::default)
}
fn slot(path: &Path) -> Shared {
    stores()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .entry(path.into())
        .or_default()
        .clone()
}
/// Called on a UI mutation; a later explicit save supersedes this pending value.
pub(super) fn queue(path: PathBuf, bytes: Vec<u8>) {
    let shared = slot(&path);
    let revision = shared.revision.fetch_add(1, Ordering::AcqRel) + 1;
    let mut pending = shared.pending.lock().unwrap_or_else(|e| e.into_inner());
    if pending.as_ref().is_none_or(|(old, _)| *old < revision) {
        *pending = Some((revision, bytes));
    }
}
pub(super) fn flush(path: &Path) -> io::Result<()> {
    let shared = slot(path);
    let mut committed = shared.commit.lock().unwrap_or_else(|e| e.into_inner());
    let pending = shared
        .pending
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();
    if let Some((revision, bytes)) = pending {
        if revision <= *committed {
            return Ok(());
        }
        if let Err(error) = crate::projects::atomic_write(path, &bytes) {
            let mut pending = shared.pending.lock().unwrap_or_else(|e| e.into_inner());
            if pending.as_ref().is_none_or(|(newer, _)| *newer < revision) {
                *pending = Some((revision, bytes));
            }
            return Err(error);
        }
        *committed = revision;
    }
    Ok(())
}
/// Queuing never waits on disk. Explicit transactions and background commits
/// share a generation order, so an older snapshot cannot overwrite a newer one.
pub(super) fn write_now(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let shared = slot(path);
    let revision = shared.revision.fetch_add(1, Ordering::AcqRel) + 1;
    let mut committed = shared.commit.lock().unwrap_or_else(|e| e.into_inner());
    if revision > *committed {
        crate::projects::atomic_write(path, bytes)?;
        *committed = revision;
    }
    let mut pending = shared.pending.lock().unwrap_or_else(|e| e.into_inner());
    if pending.as_ref().is_some_and(|(old, _)| *old <= revision) {
        pending.take();
    }
    Ok(())
}
pub(super) fn flush_all() -> io::Result<()> {
    let paths = stores()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    for path in paths {
        flush(&path)?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn newest_autosave_and_explicit_transactions_win() {
        let root = crate::tests::scratch("ordered-autosave");
        let path = root.join("state.json");
        queue(path.clone(), b"old".to_vec());
        queue(path.clone(), b"new".to_vec());
        flush(&path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
        queue(path.clone(), b"stale".to_vec());
        write_now(&path, b"explicit").unwrap();
        flush(&path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"explicit");
        std::fs::remove_dir_all(root).unwrap();
    }
}
