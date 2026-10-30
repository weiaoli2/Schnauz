//! Local Trash and directly confirmed permanent deletion on network drives.
use super::*;
use std::os::unix::fs::MetadataExt;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Identity {
    device: u64,
    inode: u64,
    changed: i64,
    changed_ns: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!("schnauz-delete-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
        fn target(&self, name: &str) -> Target {
            let path = self.0.join(name);
            Target { identity: identity(&path).unwrap(), path }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
    }
    #[test]
    fn permanent_delete_removes_files_and_folders_but_preserves_symlink_targets() {
        let fixture = Fixture::new();
        fs::write(fixture.0.join("file.txt"), "fixture").unwrap();
        delete_target(&fixture.target("file.txt")).unwrap();
        assert!(!fixture.0.join("file.txt").exists());
        fs::create_dir(fixture.0.join("folder")).unwrap();
        fs::write(fixture.0.join("preserved.txt"), "outside the selected folder").unwrap();
        std::os::unix::fs::symlink(fixture.0.join("preserved.txt"), fixture.0.join("folder/link")).unwrap();
        delete_target(&fixture.target("folder")).unwrap();
        assert!(!fixture.0.join("folder").exists());
        assert_eq!(fs::read_to_string(fixture.0.join("preserved.txt")).unwrap(), "outside the selected folder");
        std::os::unix::fs::symlink(fixture.0.join("preserved.txt"), fixture.0.join("link")).unwrap();
        delete_target(&fixture.target("link")).unwrap();
        assert!(fs::symlink_metadata(fixture.0.join("link")).is_err());
        assert!(fixture.0.join("preserved.txt").exists());
    }
    #[test]
    fn permanent_delete_rejects_a_replacement_at_the_confirmed_path() {
        let fixture = Fixture::new();
        fs::write(fixture.0.join("file.txt"), "original").unwrap();
        let target = fixture.target("file.txt");
        fs::rename(&target.path, fixture.0.join("original.txt")).unwrap();
        fs::write(&target.path, "replacement").unwrap();
        assert!(delete_target(&target).unwrap_err().contains("changed"));
        assert_eq!(fs::read_to_string(&target.path).unwrap(), "replacement");
    }
    #[test]
    fn partial_failures_keep_counts_and_the_actual_error() {
        let report = Report {
            succeeded: 2,
            failures: vec![Failure { path: "/fixture/denied.txt".into(), error: "Permission denied".into() }],
            ..Default::default()
        };
        let error = report.error(true).unwrap();
        assert!(error.contains("2 completed"));
        assert!(error.contains("denied.txt"));
        assert!(error.contains("Permission denied"));
    }
}
fn identity(path: &Path) -> std::io::Result<Identity> {
    let metadata = fs::symlink_metadata(path)?;
    Ok(Identity { device: metadata.dev(), inode: metadata.ino(), changed: metadata.ctime(), changed_ns: metadata.ctime_nsec() })
}

#[derive(Clone)]
pub(super) struct Target {
    pub path: PathBuf,
    identity: Identity,
}
pub(super) struct Confirmation {
    pub targets: Vec<Target>,
}
struct Failure {
    path: PathBuf,
    error: String,
}
#[derive(Default)]
struct Report {
    succeeded: usize,
    undo: Vec<(PathBuf, PathBuf)>,
    failures: Vec<Failure>,
    cancelled: bool,
}
impl Report {
    fn error(&self, permanent: bool) -> Option<String> {
        let failure = self.failures.first()?;
        Some(format!("{}{} “{}”: {}{}",
            if self.succeeded > 0 { format!("{} completed; ", self.succeeded) } else { String::new() },
            if permanent { "Couldn't delete" } else { "Couldn't move to Trash" },
            path_label(&failure.path), failure.error,
            if self.failures.len() > 1 { format!(" ({} items failed)", self.failures.len()) } else { String::new() }))
    }
}

fn network_parent(path: &Path) -> Result<bool, String> {
    let parent = path.parent().ok_or("Cannot delete a filesystem root.")?;
    let parent = std::ffi::CString::new(parent.as_os_str().as_encoded_bytes()).map_err(|e| e.to_string())?;
    let mut info = std::mem::MaybeUninit::<libc::statfs>::uninit();
    if unsafe { libc::statfs(parent.as_ptr(), info.as_mut_ptr()) } != 0 { return Err(std::io::Error::last_os_error().to_string()); }
    Ok(unsafe { info.assume_init().f_flags & libc::MNT_LOCAL as u32 == 0 })
}

fn run_trash(paths: &[PathBuf], progress: &JobProgress) -> Report {
    let mut report = Report::default();
    progress.total.store(paths.len() as u64, Ordering::Relaxed);
    for path in paths {
        if progress.cancel.load(Ordering::Relaxed) { report.cancelled = true; break; }
        match trash_path(path) {
            Ok(destination) => {
                report.succeeded += 1;
                if !destination.as_os_str().is_empty() { report.undo.push((path.clone(), destination)); }
            }
            Err(error) => {
                report.failures.push(Failure { path: path.clone(), error });
            }
        }
        progress.done.fetch_add(1, Ordering::Relaxed);
    }
    report
}

fn delete_target(target: &Target) -> Result<(), String> {
    let now = identity(&target.path).map_err(|e| e.to_string())?;
    if now != target.identity { return Err("The item changed since confirmation was requested. Select it again to delete it.".into()); }
    let metadata = fs::symlink_metadata(&target.path).map_err(|e| e.to_string())?;
    // symlink_metadata ensures a link removes only the link, never its target.
    if metadata.is_dir() { fs::remove_dir_all(&target.path) } else { fs::remove_file(&target.path) }
        .map_err(|e| e.to_string())
}

impl Schnauz {
    pub(super) fn request_local_delete(&mut self, pane: usize, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if paths.is_empty() { return; }
        self.delete_generation = self.delete_generation.wrapping_add(1);
        let generation = self.delete_generation;
        let tab = self.tab(pane).id;
        let progress = Arc::new(JobProgress::default());
        let id = self.begin_file_job("Preparing deletion…".into(), progress.clone(), cx);
        let cancelled = progress.clone();
        let (send, receive) = futures::channel::oneshot::channel();
        std::thread::spawn(move || {
            let result = (|| -> Result<_, String> {
                let mut local = Vec::new();
                let mut network = Vec::new();
                for path in paths {
                    if progress.cancel.load(Ordering::Relaxed) { break; }
                    if network_parent(&path).map_err(|e| format!("{}: {e}", path_label(&path)))? {
                        let identity = identity(&path).map_err(|e| format!("{}: {e}", path_label(&path)))?;
                        network.push(Target { path, identity });
                    } else { local.push(path); }
                }
                Ok((local, network))
            })();
            let _ = send.send(result);
        });
        cx.spawn(async move |this, cx| {
            let result = receive.await;
            let _ = this.update(cx, |this, cx| {
                this.jobs.retain(|job| job.id != id);
                if this.delete_generation != generation || cancelled.cancel.load(Ordering::Relaxed) {
                    cx.notify();
                    return;
                }
                if pane >= this.panes.len() || this.tab(pane).id != tab {
                    this.set_notice("Delete cancelled because the source tab changed.".into(), cx);
                    return;
                }
                match result {
                    Ok(Ok((local, network))) => {
                        this.confirm_delete = (!local.is_empty()).then_some((pane, local));
                        this.permanent_delete = (!network.is_empty()).then_some(Confirmation { targets: network });
                    }
                    _ => {
                        this.notice = None;
                        this.remote_error = Some(match result {
                            Ok(Err(error)) => format!("Couldn't prepare deletion: {error}"),
                            _ => "Couldn't prepare deletion: the worker stopped unexpectedly.".into(),
                        });
                    }
                }
                cx.notify();
            });
        }).detach();
    }
    pub(super) fn start_trash(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        self.start_trash_job(paths, None, cx);
    }
    pub(super) fn start_permanent_delete(&mut self, targets: Vec<Target>, cx: &mut Context<Self>) {
        let paths = targets.iter().map(|t| t.path.clone()).collect();
        self.start_trash_job(paths, Some(targets), cx);
    }
    fn start_trash_job(&mut self, paths: Vec<PathBuf>, permanent: Option<Vec<Target>>, cx: &mut Context<Self>) {
        if paths.is_empty() { return; }
        let is_permanent = permanent.is_some();
        let label = format!("{} {}", if is_permanent { "Deleting permanently" } else { "Moving to Trash" }, items_label(paths.len()));
        let progress = Arc::new(JobProgress::default());
        let id = self.begin_file_job(label, progress.clone(), cx);
        let refresh_paths = paths.clone();
        let (send, receive) = futures::channel::oneshot::channel();
        std::thread::spawn(move || {
            let report = objc2::rc::autoreleasepool(|_| {
                if let Some(targets) = permanent {
                    let mut report = Report::default();
                    progress.total.store(targets.len() as u64, Ordering::Relaxed);
                    for target in targets {
                        if progress.cancel.load(Ordering::Relaxed) { report.cancelled = true; break; }
                        match delete_target(&target) {
                            Ok(()) => report.succeeded += 1,
                            Err(error) => report.failures.push(Failure { path: target.path, error }),
                        }
                        progress.done.fetch_add(1, Ordering::Relaxed);
                    }
                    report
                } else { run_trash(&paths, &progress) }
            });
            let _ = send.send(report);
        });
        cx.spawn(async move |this, cx| {
            let report = receive.await;
            let _ = this.update(cx, |this, cx| {
                this.jobs.retain(|job| job.id != id);
                let Ok(mut report) = report else {
                    this.notice = None;
                    this.remote_error = Some("The file operation stopped unexpectedly. Refresh the folder to check its contents.".into());
                    cx.notify();
                    return;
                };
                if !report.undo.is_empty() { this.push_undo(UndoOp::Trashed(std::mem::take(&mut report.undo))); }
                if let Some(error) = report.error(is_permanent) {
                    this.notice = None;
                    this.remote_error = Some(error.clone());
                } else {
                    this.set_notice(if report.cancelled { format!("Operation cancelled · {} completed", report.succeeded) }
                        else { format!("{} {}", if is_permanent { "Deleted permanently" } else { "Moved to Trash" }, items_label(report.succeeded)) }, cx);
                }
                if report.succeeded > 0 {
                    for pane in 0..this.panes.len() {
                        if this.tab(pane).remote.is_none() && (this.search_in_pane(pane) || refresh_paths.iter().any(|p| p.parent() == Some(this.tab(pane).current_dir.as_path()))) {
                            this.refresh_pane(pane, cx);
                        }
                    }
                }
                cx.notify();
            });
        }).detach();
    }
}
