//! Exclusive copy/move publication. Partial work stays in an owned private folder.
use super::*;
use std::{
    ffi::CString,
    io,
    os::unix::{
        ffi::OsStrExt,
        fs::{DirBuilderExt, MetadataExt},
    },
};

fn check(cancel: &AtomicBool) -> io::Result<()> {
    if cancel.load(Ordering::Acquire) {
        Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"))
    } else {
        Ok(())
    }
}

pub(super) fn rename_exclusive(src: &Path, dest: &Path) -> io::Result<()> {
    let src = CString::new(src.as_os_str().as_bytes())?;
    let dest = CString::new(dest.as_os_str().as_bytes())?;
    if unsafe { libc::renamex_np(src.as_ptr(), dest.as_ptr(), libc::RENAME_EXCL) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

struct Stage {
    path: PathBuf,
    identity: (u64, u64),
}
impl Stage {
    fn new(parent: &Path) -> io::Result<Self> {
        for _ in 0..16 {
            let path = parent.join(format!(
                ".schnauz-transfer-{}",
                objc2_foundation::NSUUID::UUID().UUIDString()
            ));
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => {
                    let md = fs::symlink_metadata(&path)?;
                    return Ok(Self {
                        path,
                        identity: (md.dev(), md.ino()),
                    });
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        Err(io::Error::other(
            "Could not reserve a private transfer folder.",
        ))
    }
}
impl Drop for Stage {
    fn drop(&mut self) {
        if fs::symlink_metadata(&self.path)
            .is_ok_and(|m| m.is_dir() && (m.dev(), m.ino()) == self.identity)
        {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn size(path: &Path, cancel: &AtomicBool) -> io::Result<u64> {
    let mut stack = vec![path.to_path_buf()];
    let mut bytes = 0u64;
    while let Some(path) = stack.pop() {
        check(cancel)?;
        let md = fs::symlink_metadata(&path)?;
        if md.is_dir() {
            for entry in fs::read_dir(path)? {
                check(cancel)?;
                stack.push(entry?.path());
            }
        } else if md.is_file() {
            bytes = bytes.saturating_add(md.len());
        }
    }
    Ok(bytes)
}

fn copy(src: &Path, dest: &Path, progress: &JobProgress) -> io::Result<()> {
    check(&progress.cancel)?;
    let md = fs::symlink_metadata(src)?;
    if md.file_type().is_symlink() {
        std::os::unix::fs::symlink(fs::read_link(src)?, dest)?;
    } else if md.is_dir() {
        fs::create_dir(dest)?;
        for entry in fs::read_dir(src)? {
            check(&progress.cancel)?;
            let entry = entry?;
            copy(&entry.path(), &dest.join(entry.file_name()), progress)?;
        }
        fs::set_permissions(dest, md.permissions())?;
    } else if md.is_file() {
        // All copying happens inside this job's private stage.
        let reserved = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dest)?;
        drop(reserved);
        fs::copy(src, dest)?;
        if let Ok(time) = md.modified() {
            let _ = fs::File::options()
                .write(true)
                .open(dest)?
                .set_modified(time);
        }
        progress.done.fetch_add(md.len(), Ordering::Relaxed);
    } else {
        return Err(io::Error::other("Unsupported file type."));
    }
    Ok(())
}

pub(super) fn run(
    pairs: &[(PathBuf, PathBuf)],
    kind: TransferKind,
    progress: &JobProgress,
) -> (Vec<(PathBuf, PathBuf)>, Option<String>) {
    let mut done = Vec::new();
    let mut error = None;
    let mut copies = Vec::new();
    for (src, dest) in pairs {
        if check(&progress.cancel).is_err() {
            return (done, error);
        }
        if kind == TransferKind::Move {
            match rename_exclusive(src, dest) {
                Ok(()) => {
                    done.push((src.clone(), dest.clone()));
                    continue;
                }
                Err(e) if e.raw_os_error() == Some(libc::EXDEV) => {}
                Err(e) => {
                    error = Some(format!("Couldn't move “{}”: {e}", path_label(src)));
                    continue;
                }
            }
        }
        copies.push((src, dest));
    }
    let mut bytes = 0u64;
    let mut ready = Vec::new();
    for (src, dest) in copies {
        if fs::symlink_metadata(dest).is_ok() {
            error = Some(format!("“{}” already exists there", path_label(dest)));
            continue;
        }
        match size(src, &progress.cancel) {
            Ok(n) => {
                bytes = bytes.saturating_add(n);
                ready.push((src, dest));
            }
            Err(e) => {
                if progress.cancel.load(Ordering::Acquire) {
                    return (done, error);
                }
                error = Some(format!("Couldn't prepare “{}”: {e}", path_label(src)));
            }
        }
    }
    progress.total.store(bytes, Ordering::Relaxed);
    for (src, dest) in ready {
        let result = (|| -> io::Result<()> {
            check(&progress.cancel)?;
            match fs::symlink_metadata(dest) {
                Ok(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "Destination already exists",
                    ))
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
            let identity = fs::symlink_metadata(src)?;
            let stage = Stage::new(
                dest.parent()
                    .ok_or_else(|| io::Error::other("Missing destination folder"))?,
            )?;
            let partial = stage.path.join("item");
            copy(src, &partial, progress)?;
            check(&progress.cancel)?;
            rename_exclusive(&partial, dest)?;
            if kind == TransferKind::Move {
                let current = fs::symlink_metadata(src)?;
                if (current.dev(), current.ino()) != (identity.dev(), identity.ino()) {
                    return Err(io::Error::other(
                        "Copy saved; the original was replaced and has been left untouched.",
                    ));
                }
                let removal = if identity.is_dir() {
                    fs::remove_dir_all(src)
                } else {
                    fs::remove_file(src)
                };
                removal.map_err(|e| {
                    io::Error::other(format!("Copy saved, but couldn't remove the original: {e}"))
                })?;
            }
            Ok(())
        })();
        match result {
            Ok(()) => done.push((src.clone(), dest.clone())),
            Err(e) => {
                if progress.cancel.load(Ordering::Acquire) {
                    break;
                }
                error = Some(format!("Couldn't finish “{}”: {e}", path_label(src)));
            }
        }
    }
    (done, error)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_destination_created_during_copy_survives_failed_publication() {
        let root = crate::tests::scratch("publication-race");
        let source = root.join("source");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("data"), "source").unwrap();
        let dest = root.join("destination");
        {
            let stage = Stage::new(&root).unwrap();
            let partial = stage.path.join("item");
            copy(&source, &partial, &JobProgress::default()).unwrap();
            fs::create_dir(&dest).unwrap();
            fs::write(dest.join("keep"), "winner").unwrap();
            assert!(rename_exclusive(&partial, &dest).is_err());
        }
        assert_eq!(fs::read_to_string(dest.join("keep")).unwrap(), "winner");
        assert_eq!(fs::read_to_string(source.join("data")).unwrap(), "source");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 2);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn concurrent_publication_never_clobbers_or_removes_the_winner() {
        let root = crate::tests::scratch("exclusive-transfer");
        let a = root.join("a");
        let b = root.join("b");
        let dest = root.join("dest");
        fs::write(&a, "a").unwrap();
        fs::write(&b, "b").unwrap();
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let jobs = [a, b]
            .into_iter()
            .map(|src| {
                let dest = dest.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    rename_exclusive(&src, &dest)
                })
            })
            .collect::<Vec<_>>();
        let results = jobs
            .into_iter()
            .map(|j| j.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        assert!(matches!(
            fs::read_to_string(&dest).unwrap().as_str(),
            "a" | "b"
        ));
        let source = root.join("folder");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("new"), "new").unwrap();
        let (done, error) = run(
            &[(source, dest.clone())],
            TransferKind::Copy,
            &JobProgress::default(),
        );
        assert!(done.is_empty() && error.is_some());
        assert!(dest.is_file());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn cancelled_moves_leave_sources_and_destinations_intact() {
        let root = crate::tests::scratch("cancelled-move");
        let src = root.join("src");
        let dest = root.join("dest");
        fs::write(&src, "keep").unwrap();
        let progress = JobProgress::default();
        progress.cancel.store(true, Ordering::Release);
        assert!(run(
            &[(src.clone(), dest.clone())],
            TransferKind::Move,
            &progress
        )
        .0
        .is_empty());
        assert_eq!(fs::read_to_string(src).unwrap(), "keep");
        assert!(!dest.exists());
        fs::remove_dir_all(root).unwrap();
    }
}
