//! macOS vnode notifications for visible folders and selected previews; no directory hashing.
use std::{
    collections::{HashMap, HashSet},
    ffi::CString,
    fs::File,
    io,
    os::unix::{
        ffi::OsStrExt,
        io::{AsRawFd, FromRawFd},
    },
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime},
};
struct Watched {
    file: Option<File>,
    mtime: Option<SystemTime>,
    checked: Instant,
    interval: Duration,
}
pub struct Watcher {
    queue: Option<File>,
    folders: HashMap<PathBuf, Watched>,
}
impl Watcher {
    pub fn new() -> Self {
        let fd = unsafe { libc::kqueue() };
        Self {
            queue: (fd >= 0).then(|| unsafe { File::from_raw_fd(fd) }),
            folders: HashMap::new(),
        }
    }
    fn attach(&self, path: &Path) -> io::Result<File> {
        let queue = self
            .queue
            .as_ref()
            .ok_or_else(|| io::Error::other("No event queue"))?;
        let name = CString::new(path.as_os_str().as_bytes())?;
        let fd = unsafe { libc::open(name.as_ptr(), libc::O_EVTONLY | libc::O_CLOEXEC) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let file = unsafe { File::from_raw_fd(fd) };
        let event = libc::kevent {
            ident: fd as _,
            filter: libc::EVFILT_VNODE,
            flags: libc::EV_ADD | libc::EV_CLEAR,
            fflags: libc::NOTE_WRITE
                | libc::NOTE_DELETE
                | libc::NOTE_RENAME
                | libc::NOTE_ATTRIB
                | libc::NOTE_REVOKE,
            data: 0,
            udata: std::ptr::null_mut(),
        };
        if unsafe {
            libc::kevent(
                queue.as_raw_fd(),
                &event,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        } < 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(file)
    }
    /// Called off the UI thread. Dropping unused file descriptors unregisters them.
    pub fn changes(&mut self, wanted: Vec<PathBuf>) -> HashSet<PathBuf> {
        let wanted: HashSet<_> = wanted.into_iter().collect();
        self.folders.retain(|p, _| wanted.contains(p));
        let mut changed = HashSet::new();
        for path in wanted {
            if !self.folders.contains_key(&path) {
                let file = self.attach(&path).ok();
                let mtime = modified(&path);
                self.folders.insert(
                    path.clone(),
                    Watched {
                        file,
                        mtime,
                        checked: Instant::now(),
                        interval: Duration::from_secs(1),
                    },
                );
                // Close the gap between initial listing and watcher registration.
                changed.insert(path);
            }
        }
        if let Some(queue) = &self.queue {
            let mut events: [libc::kevent; 64] = unsafe { std::mem::zeroed() };
            let timeout = libc::timespec {
                tv_sec: 0,
                tv_nsec: 0,
            };
            loop {
                let n = unsafe {
                    libc::kevent(
                        queue.as_raw_fd(),
                        std::ptr::null(),
                        0,
                        events.as_mut_ptr(),
                        64,
                        &timeout,
                    )
                };
                if n <= 0 {
                    break;
                }
                for event in &events[..n as usize] {
                    for (path, watch) in &mut self.folders {
                        if watch
                            .file
                            .as_ref()
                            .is_some_and(|f| f.as_raw_fd() as usize == event.ident)
                        {
                            changed.insert(path.clone());
                            watch.mtime = modified(path);
                            if event.fflags
                                & (libc::NOTE_DELETE | libc::NOTE_RENAME | libc::NOTE_REVOKE)
                                != 0
                            {
                                watch.file = None;
                                watch.interval = Duration::from_secs(1);
                            }
                        }
                    }
                }
                if n < 64 {
                    break;
                }
            }
        }
        // Unsupported/network filesystems retry with backoff; ordinary folders
        // incur no per-file work or polling while idle.
        let retry: Vec<_> = self
            .folders
            .iter()
            .filter(|(_, w)| w.file.is_none() && w.checked.elapsed() >= w.interval)
            .map(|(p, _)| p.clone())
            .collect();
        for path in retry {
            let file = self.attach(&path).ok();
            let now = modified(&path);
            let watch = self.folders.get_mut(&path).unwrap();
            if watch.mtime != now {
                changed.insert(path);
                watch.interval = Duration::from_secs(1);
            } else {
                watch.interval = (watch.interval * 2).min(Duration::from_secs(15));
            }
            watch.file = file;
            watch.mtime = now;
            watch.checked = Instant::now();
        }
        changed
    }
}
fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reacts_to_create_rename_delete_and_releases_old_folders() {
        let root = crate::tests::scratch("folder-watch");
        std::fs::create_dir_all(&root).unwrap();
        let mut watcher = Watcher::new();
        assert!(watcher.changes(vec![root.clone()]).contains(&root));
        assert!(watcher.changes(vec![root.clone()]).is_empty());
        for action in 0..3 {
            match action {
                0 => std::fs::write(root.join("first"), "file").unwrap(),
                1 => std::fs::rename(root.join("first"), root.join("second")).unwrap(),
                _ => std::fs::remove_file(root.join("second")).unwrap(),
            }
            let deadline = Instant::now() + Duration::from_secs(2);
            let mut detected = false;
            while Instant::now() < deadline {
                if watcher.changes(vec![root.clone()]).contains(&root) {
                    detected = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(detected, "filesystem event was lost");
        }
        watcher.changes(Vec::new());
        assert!(watcher.folders.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}
