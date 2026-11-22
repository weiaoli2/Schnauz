//! File-level macOS notifications supplement directory vnode events. No scanning.
use objc2_foundation::{NSArray, NSString};
use std::{
    collections::HashSet,
    ffi::{c_char, c_void, CStr},
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    ptr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

// ABI/constants from the SDK's CoreServices/FSEvents.h. FileEvents reports
// in-place writes/metadata changes which do not modify the parent directory.
const NO_DEFER: u32 = 0x2;
const WATCH_ROOT: u32 = 0x4;
const FILE_EVENTS: u32 = 0x10;
const RECOVER: u32 = 0x1 | 0x2 | 0x4 | 0x8; // coalesced/dropped/wrapped events
const HISTORY_DONE: u32 = 0x10;
#[repr(C)]
struct Context {
    version: isize,
    info: *mut c_void,
    retain: Option<unsafe extern "C" fn(*const c_void) -> *const c_void>,
    release: Option<unsafe extern "C" fn(*const c_void)>,
    description: Option<unsafe extern "C" fn(*const c_void) -> *const c_void>,
}
type Callback =
    unsafe extern "C" fn(*const c_void, *mut c_void, usize, *mut c_void, *const u32, *const u64);
#[link(name = "CoreServices", kind = "framework")]
extern "C" {
    fn FSEventStreamCreate(
        allocator: *const c_void,
        callback: Callback,
        context: *mut Context,
        paths: *const c_void,
        since: u64,
        latency: f64,
        flags: u32,
    ) -> *mut c_void;
    fn FSEventStreamSetDispatchQueue(stream: *mut c_void, queue: *mut c_void);
    fn FSEventStreamStart(stream: *mut c_void) -> u8;
    fn FSEventStreamStop(stream: *mut c_void);
    fn FSEventStreamInvalidate(stream: *mut c_void);
    fn FSEventStreamRelease(stream: *mut c_void);
}
extern "C" {
    fn dispatch_queue_create(label: *const c_char, attr: *const c_void) -> *mut c_void;
    fn dispatch_release(object: *mut c_void);
}
struct State {
    roots: Vec<(PathBuf, PathBuf)>, // displayed path -> physical watched root
    dirty: Mutex<HashSet<PathBuf>>,
}
unsafe extern "C" fn retain(info: *const c_void) -> *const c_void {
    unsafe {
        Arc::increment_strong_count(info.cast::<State>());
    }
    info
}
unsafe extern "C" fn release(info: *const c_void) {
    unsafe {
        Arc::decrement_strong_count(info.cast::<State>());
    }
}
fn affects(root: &Path, event: &Path, flags: u32) -> bool {
    // Recovery reconciles only the current listing, never scans descendants.
    flags & RECOVER != 0 || event == root || event.parent() == Some(root)
}
unsafe extern "C" fn events(
    _: *const c_void,
    info: *mut c_void,
    count: usize,
    paths: *mut c_void,
    flags: *const u32,
    _: *const u64,
) {
    if info.is_null() || paths.is_null() || flags.is_null() {
        return;
    }
    // The stream retains this Arc until after its callbacks finish. Store only
    // affected displayed roots, so a huge event batch cannot grow a path queue.
    let state = unsafe { &*info.cast::<State>() };
    let Ok(mut dirty) = state.dirty.lock() else {
        return;
    };
    for i in 0..count {
        let flag = unsafe { *flags.add(i) };
        if flag & HISTORY_DONE != 0 {
            continue;
        }
        let name = unsafe { *paths.cast::<*const c_char>().add(i) };
        if name.is_null() {
            continue;
        }
        let name = unsafe { CStr::from_ptr(name) };
        let event = Path::new(std::ffi::OsStr::from_bytes(name.to_bytes()));
        for (displayed, physical) in &state.roots {
            if affects(physical, event, flag) {
                dirty.insert(displayed.clone());
            }
        }
    }
}
struct Stream {
    stream: *mut c_void,
    queue: *mut c_void,
    state: Arc<State>,
}
// Creation/configuration/draining/destruction are serialized by the owned
// folder worker. The only concurrent callback touches immutable roots + Mutex.
unsafe impl Send for Stream {}
impl Stream {
    fn new(folders: &HashSet<PathBuf>) -> Option<Self> {
        let roots: Vec<_> = folders
            .iter()
            .filter_map(|path| {
                let physical = path.canonicalize().ok()?;
                if !physical.is_dir() || physical.to_str().is_none() {
                    return None;
                }
                Some((path.clone(), physical))
            })
            .collect();
        if roots.is_empty() {
            return None;
        }
        let names: Vec<_> = roots
            .iter()
            .map(|(_, path)| NSString::from_str(path.to_str().unwrap()))
            .collect();
        let names = NSArray::from_retained_slice(&names);
        let state = Arc::new(State {
            roots,
            dirty: Mutex::new(HashSet::new()),
        });
        let mut context = Context {
            version: 0,
            info: Arc::as_ptr(&state) as *mut c_void,
            retain: Some(retain),
            release: Some(release),
            description: None,
        };
        unsafe {
            let queue = dispatch_queue_create(c"local.schnauz.file-events".as_ptr(), ptr::null());
            if queue.is_null() {
                return None;
            }
            // NSArray<NSString> and CFArray<CFString> are toll-free bridged.
            let stream = FSEventStreamCreate(
                ptr::null(),
                events,
                &mut context,
                (&*names as *const NSArray<NSString>).cast(),
                u64::MAX,
                0.15,
                NO_DEFER | WATCH_ROOT | FILE_EVENTS,
            );
            if stream.is_null() {
                dispatch_release(queue);
                return None;
            }
            FSEventStreamSetDispatchQueue(stream, queue);
            if FSEventStreamStart(stream) == 0 {
                FSEventStreamInvalidate(stream);
                FSEventStreamRelease(stream);
                dispatch_release(queue);
                return None;
            }
            Some(Self {
                stream,
                queue,
                state,
            })
        }
    }
    fn drain(&self) -> HashSet<PathBuf> {
        self.state
            .dirty
            .lock()
            .map(|mut dirty| std::mem::take(&mut *dirty))
            .unwrap_or_default()
    }
}
impl Drop for Stream {
    fn drop(&mut self) {
        unsafe {
            FSEventStreamStop(self.stream);
            FSEventStreamInvalidate(self.stream);
            FSEventStreamRelease(self.stream);
            dispatch_release(self.queue);
        }
    }
}
pub(super) struct Watcher {
    folders: HashSet<PathBuf>,
    stream: Option<Stream>,
    attempted: Instant,
}
impl Watcher {
    pub(super) fn new() -> Self {
        Self {
            folders: HashSet::new(),
            stream: None,
            attempted: Instant::now(),
        }
    }
    pub(super) fn changes(&mut self, folders: Vec<PathBuf>) -> HashSet<PathBuf> {
        let folders: HashSet<_> = folders.into_iter().collect();
        let changed_roots = self.folders != folders;
        let retry = self.stream.is_none()
            && !folders.is_empty()
            && self.attempted.elapsed() >= Duration::from_secs(5);
        if changed_roots || retry {
            self.stream = None; // stop old scope before attaching the new one
            self.folders = folders;
            self.attempted = Instant::now();
            self.stream = Stream::new(&self.folders);
            // Close the listing/registration gap without periodic full refresh.
            if self.stream.is_some() {
                return self.folders.clone();
            }
        }
        self.stream.as_ref().map(Stream::drain).unwrap_or_default()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn in_place_child_events_refresh_only_the_displayed_listing() {
        let root = Path::new("/real/project");
        assert!(affects(root, Path::new("/real/project/file.txt"), 0x1000)); // ItemModified
        assert!(affects(root, Path::new("/real/project/file.txt"), 0x400)); // InodeMetaMod
        assert!(affects(root, root, 0));
        assert!(!affects(
            root,
            Path::new("/real/project/sub/file.txt"),
            0x1000
        ));
        assert!(!affects(
            root,
            Path::new("/real/project-other/file.txt"),
            0x1000
        ));
        assert!(affects(root, Path::new("/real"), RECOVER));
    }
}
