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

/// Keep the failing operation in the footer error. A numeric errno alone cannot
/// distinguish SMB data/metadata failures from unsupported exclusive rename.
/// Diagnostics contain only fixed operation labels, never paths or credentials.
fn step<T>(label: &'static str, diagnostic: &'static str, result: io::Result<T>) -> io::Result<T> {
    result.map_err(|error| {
        diagnostics::event(diagnostic);
        io::Error::new(error.kind(), format!("{label}: {error}"))
    })
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
    let md = step(
        "Reading source metadata",
        "transfer failed: source metadata",
        fs::symlink_metadata(src),
    )?;
    if md.file_type().is_symlink() {
        let target = step(
            "Reading source link",
            "transfer failed: read link",
            fs::read_link(src),
        )?;
        step(
            "Creating destination link",
            "transfer failed: create link",
            std::os::unix::fs::symlink(target, dest),
        )?;
    } else if md.is_dir() {
        step(
            "Creating destination folder",
            "transfer failed: create folder",
            fs::create_dir(dest),
        )?;
        for entry in step(
            "Reading source folder",
            "transfer failed: read folder",
            fs::read_dir(src),
        )? {
            check(&progress.cancel)?;
            let entry = step(
                "Reading source folder entry",
                "transfer failed: read entry",
                entry,
            )?;
            copy(&entry.path(), &dest.join(entry.file_name()), progress)?;
        }
        step(
            "Preserving folder permissions",
            "transfer failed: folder permissions",
            fs::set_permissions(dest, md.permissions()),
        )?;
    } else if md.is_file() {
        // All copying happens inside this job's private stage.
        let reserved = step(
            "Creating temporary destination file",
            "transfer failed: create file",
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(dest),
        )?;
        drop(reserved);
        step(
            "Copying file data and permissions",
            "transfer failed: copy data",
            fs::copy(src, dest),
        )?;
        if let Ok(time) = md.modified() {
            let destination = step(
                "Opening copied file to preserve its date",
                "transfer failed: reopen copied file",
                fs::File::options().write(true).open(dest),
            )?;
            let _ = destination.set_modified(time);
        }
        progress.done.fetch_add(md.len(), Ordering::Relaxed);
    } else {
        return Err(io::Error::other("Unsupported file type."));
    }
    Ok(())
}

fn unsupported_rename(error: &io::Error) -> bool {
    matches!(
        error.raw_os_error(),
        Some(libc::ENOTSUP) | Some(libc::ENOSYS)
    )
}

/// SMB servers need not implement RENAME_EXCL. Reserve each destination with
/// create_new/mkdir/symlink instead. Never reopen a reserved file by path: another
/// client creating the same name must neither be overwritten nor cleaned up.
struct ReservedDestination {
    path: PathBuf,
    identity: (u64, u64),
    directory: bool,
    complete: bool,
}
impl ReservedDestination {
    fn new(path: &Path, metadata: &fs::Metadata) -> Self {
        Self {
            path: path.to_path_buf(),
            identity: (metadata.dev(), metadata.ino()),
            directory: metadata.is_dir(),
            complete: false,
        }
    }
}
impl Drop for ReservedDestination {
    fn drop(&mut self) {
        if !self.complete
            && fs::symlink_metadata(&self.path).is_ok_and(|m| (m.dev(), m.ino()) == self.identity)
        {
            if self.directory {
                // A public destination may gain another client's files. Remove
                // only an empty directory; never recursively delete their work.
                let _ = fs::remove_dir(&self.path);
            } else {
                let _ = fs::remove_file(&self.path);
            }
        }
    }
}

#[derive(Default)]
struct Reservations(Vec<ReservedDestination>);
impl Drop for Reservations {
    fn drop(&mut self) {
        // Children before parents, checking every object's identity separately.
        while self.0.pop().is_some() {}
    }
}

fn copy_reserved(src: &Path, dest: &Path, progress: &JobProgress) -> io::Result<()> {
    let mut reservations = Reservations::default();
    copy_reserved_inner(src, dest, progress, &mut reservations)?;
    check(&progress.cancel)?;
    for owned in &mut reservations.0 {
        owned.complete = true;
    }
    Ok(())
}

fn copy_reserved_inner(
    src: &Path,
    dest: &Path,
    progress: &JobProgress,
    reservations: &mut Reservations,
) -> io::Result<()> {
    use std::io::{Read, Write};
    check(&progress.cancel)?;
    let md = fs::symlink_metadata(src)?;
    if md.is_file() {
        let mut source = fs::File::open(src)?;
        let mut target = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dest)?;
        reservations
            .0
            .push(ReservedDestination::new(dest, &target.metadata()?));
        let mut buffer = vec![0; 1024 * 1024];
        loop {
            check(&progress.cancel)?;
            let n = source.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            target.write_all(&buffer[..n])?;
            progress.done.fetch_add(n as u64, Ordering::Relaxed);
        }
        target.flush()?;
        target.set_permissions(md.permissions())?;
        if let Ok(time) = md.modified() {
            let _ = target.set_modified(time);
        }
        check(&progress.cancel)?;
    } else if md.is_dir() {
        fs::DirBuilder::new().mode(0o700).create(dest)?;
        reservations
            .0
            .push(ReservedDestination::new(dest, &fs::symlink_metadata(dest)?));
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            copy_reserved_inner(
                &entry.path(),
                &dest.join(entry.file_name()),
                progress,
                reservations,
            )?;
        }
        check(&progress.cancel)?;
        fs::set_permissions(dest, md.permissions())?;
    } else if md.file_type().is_symlink() {
        std::os::unix::fs::symlink(fs::read_link(src)?, dest)?;
        reservations
            .0
            .push(ReservedDestination::new(dest, &fs::symlink_metadata(dest)?));
    } else {
        return Err(io::Error::other("Unsupported file type."));
    }
    Ok(())
}

fn publish(
    partial: &Path,
    dest: &Path,
    progress: &JobProgress,
    bytes: u64,
    rename: fn(&Path, &Path) -> io::Result<()>,
) -> io::Result<()> {
    match rename(partial, dest) {
        Ok(()) => Ok(()),
        Err(e) if unsupported_rename(&e) => {
            diagnostics::event(
                "transfer: exclusive rename unsupported; exclusive creation fallback",
            );
            progress.total.fetch_add(bytes, Ordering::Relaxed);
            step(
                "Saving copy with exclusive creation",
                "transfer failed: exclusive creation fallback",
                copy_reserved(partial, dest, progress),
            )
        }
        Err(e) => step(
            "Finalizing copy without overwriting",
            "transfer failed: exclusive copy finalization",
            Err(e),
        ),
    }
}

pub(super) fn run(
    pairs: &[(PathBuf, PathBuf)],
    kind: TransferKind,
    progress: &JobProgress,
) -> (Vec<(PathBuf, PathBuf)>, Option<String>) {
    run_with_rename(pairs, kind, progress, rename_exclusive)
}

fn run_with_rename(
    pairs: &[(PathBuf, PathBuf)],
    kind: TransferKind,
    progress: &JobProgress,
    rename: fn(&Path, &Path) -> io::Result<()>,
) -> (Vec<(PathBuf, PathBuf)>, Option<String>) {
    let mut done = Vec::new();
    let mut error = None;
    let mut copies = Vec::new();
    for (src, dest) in pairs {
        if check(&progress.cancel).is_err() {
            return (done, error);
        }
        if kind == TransferKind::Move {
            match rename(src, dest) {
                Ok(()) => {
                    done.push((src.clone(), dest.clone()));
                    continue;
                }
                Err(e) if e.raw_os_error() == Some(libc::EXDEV) || unsupported_rename(&e) => {}
                Err(e) => {
                    diagnostics::event("transfer failed: exclusive move");
                    error = Some(format!(
                        "Couldn't move “{}” · Renaming without overwriting: {e}",
                        path_label(src)
                    ));
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
                Err(e) => {
                    return step(
                        "Checking destination",
                        "transfer failed: destination metadata",
                        Err(e),
                    )
                }
            }
            let identity = step(
                "Reading source identity",
                "transfer failed: source identity",
                fs::symlink_metadata(src),
            )?;
            let stage = step(
                "Creating temporary destination folder",
                "transfer failed: create staging folder",
                Stage::new(
                    dest.parent()
                        .ok_or_else(|| io::Error::other("Missing destination folder"))?,
                ),
            )?;
            let partial = stage.path.join("item");
            let before = progress.done.load(Ordering::Relaxed);
            copy(src, &partial, progress)?;
            check(&progress.cancel)?;
            let bytes = progress.done.load(Ordering::Relaxed).saturating_sub(before);
            publish(&partial, dest, progress, bytes, rename)?;
            if kind == TransferKind::Move {
                let current = step(
                    "Rechecking original before removal",
                    "transfer failed: recheck original",
                    fs::symlink_metadata(src),
                )?;
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
    fn no_exclusive_rename(_: &Path, _: &Path) -> io::Result<()> {
        Err(io::Error::from_raw_os_error(libc::ENOTSUP))
    }

    #[test]
    fn unsupported_rename_copies_and_moves_files_folders_and_links() {
        let root = crate::tests::scratch("smb-exclusive-fallback");
        for kind in [TransferKind::Copy, TransferKind::Move] {
            let source = root.join(if kind == TransferKind::Copy {
                "copy"
            } else {
                "move"
            });
            fs::create_dir(&source).unwrap();
            fs::create_dir(source.join("nested")).unwrap();
            fs::write(source.join("nested/data"), "complete contents").unwrap();
            std::os::unix::fs::symlink("missing", source.join("link")).unwrap();
            let dest = source.with_extension("saved");
            let progress = JobProgress::default();
            let (done, error) = run_with_rename(
                &[(source.clone(), dest.clone())],
                kind,
                &progress,
                no_exclusive_rename,
            );
            assert!(error.is_none(), "{error:?}");
            assert_eq!(done.len(), 1);
            assert_eq!(source.exists(), kind == TransferKind::Copy);
            assert_eq!(
                fs::read_to_string(dest.join("nested/data")).unwrap(),
                "complete contents"
            );
            assert_eq!(
                fs::read_link(dest.join("link")).unwrap(),
                PathBuf::from("missing")
            );
            assert_eq!(
                progress.done.load(Ordering::Relaxed),
                progress.total.load(Ordering::Relaxed)
            );
        }
        assert_eq!(fs::read_dir(&root).unwrap().count(), 3);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fallback_preserves_a_racing_file_folder_or_symlink() {
        let root = crate::tests::scratch("smb-fallback-collision");
        let source = root.join("source");
        fs::write(&source, "new").unwrap();
        let winner = root.join("winner");
        fs::write(&winner, "keep").unwrap();
        let file = root.join("file");
        fs::write(&file, "keep").unwrap();
        let folder = root.join("folder");
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join("child"), "keep").unwrap();
        let link = root.join("link");
        std::os::unix::fs::symlink(&winner, &link).unwrap();
        for dest in [&file, &folder, &link] {
            assert!(publish(
                &source,
                dest,
                &JobProgress::default(),
                3,
                no_exclusive_rename
            )
            .is_err());
        }
        assert_eq!(fs::read_to_string(&file).unwrap(), "keep");
        assert_eq!(fs::read_to_string(folder.join("child")).unwrap(), "keep");
        assert_eq!(fs::read_link(&link).unwrap(), winner);
        assert_eq!(fs::read_to_string(&winner).unwrap(), "keep");
        assert_eq!(fs::read_to_string(&source).unwrap(), "new");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_fallback_cleans_only_its_own_incomplete_destination() {
        let root = crate::tests::scratch("smb-fallback-cleanup");
        let source = root.join("source");
        fs::create_dir(&source).unwrap();
        let fifo = CString::new(source.join("unsupported").as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        let dest = root.join("dest");
        assert!(copy_reserved(&source, &dest, &JobProgress::default()).is_err());
        assert!(!dest.exists());
        assert!(source.exists());
        let progress = JobProgress::default();
        progress.cancel.store(true, Ordering::Release);
        assert!(copy_reserved(&source, &dest, &progress).is_err());
        assert!(!dest.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fallback_cleanup_keeps_other_clients_files_and_replaced_objects() {
        let root = crate::tests::scratch("smb-fallback-ownership");
        let dest = root.join("dest");
        fs::create_dir(&dest).unwrap();
        let mut reservations = Reservations::default();
        reservations.0.push(ReservedDestination::new(
            &dest,
            &fs::symlink_metadata(&dest).unwrap(),
        ));
        let file = dest.join("owned");
        fs::write(&file, "ours").unwrap();
        reservations.0.push(ReservedDestination::new(
            &file,
            &fs::symlink_metadata(&file).unwrap(),
        ));
        // Another client replaces our file while retaining our original inode.
        fs::rename(&file, root.join("old-inode")).unwrap();
        fs::write(&file, "theirs").unwrap();
        fs::write(dest.join("added-by-another-client"), "keep").unwrap();
        drop(reservations);
        assert_eq!(fs::read_to_string(&file).unwrap(), "theirs");
        assert_eq!(
            fs::read_to_string(dest.join("added-by-another-client")).unwrap(),
            "keep"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn simultaneous_fallbacks_do_not_overwrite_or_remove_the_winner() {
        let root = crate::tests::scratch("smb-fallback-race");
        let dest = root.join("dest");
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let jobs = ["a", "b"]
            .into_iter()
            .map(|name| {
                let source = root.join(name);
                fs::write(&source, name).unwrap();
                let dest = dest.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    publish(
                        &source,
                        &dest,
                        &JobProgress::default(),
                        1,
                        no_exclusive_rename,
                    )
                })
            })
            .collect::<Vec<_>>();
        let results = jobs
            .into_iter()
            .map(|j| j.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        assert!(matches!(
            fs::read_to_string(dest).unwrap().as_str(),
            "a" | "b"
        ));
        fs::remove_dir_all(root).unwrap();
    }
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
