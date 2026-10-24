//! Cache-only cloud availability lookups; metadata waits stay on an owned worker.
use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Mutex, OnceLock,
    },
    time::{Duration, Instant},
};

const TTL: Duration = Duration::from_secs(3);
const MAX_CACHE: usize = 4096;
const MAX_PENDING: usize = 1024;
const BATCH_SIZE: usize = 64;
const POLL_INTERVAL: Duration = Duration::from_millis(40);

struct Entry {
    value: Option<bool>,
    checked: Option<Instant>,
    generation: u64,
    pending: bool,
}
#[derive(Clone)]
struct Job {
    path: PathBuf,
    generation: u64,
}
struct ResultItem {
    job: Job,
    value: Option<bool>,
}

struct Cache {
    entries: HashMap<PathBuf, Entry>,
    order: VecDeque<PathBuf>,
    queue: VecDeque<Job>,
    generation: u64,
    max_cache: usize,
    max_pending: usize,
}
impl Cache {
    fn new(max_cache: usize, max_pending: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            queue: VecDeque::new(),
            generation: 0,
            max_cache,
            max_pending,
        }
    }
    fn generation(&mut self) -> u64 {
        self.generation = self
            .generation
            .checked_add(1)
            .expect("Cloud metadata generation exhausted");
        self.generation
    }
    // This path never calls a reader, starts a thread, or recursively visits files.
    fn read_at(&mut self, path: &Path, now: Instant) -> Option<bool> {
        if let Some(entry) = self.entries.get(path) {
            if entry
                .checked
                .is_some_and(|at| now.saturating_duration_since(at) < TTL)
            {
                return entry.value;
            }
            if entry.pending {
                return None;
            }
        }
        if self.queue.len() >= self.max_pending || self.max_cache == 0 {
            return None;
        }
        let generation = self.generation();
        if let Some(entry) = self.entries.get_mut(path) {
            entry.generation = generation;
            entry.pending = true;
        } else {
            self.make_room();
            self.order.push_back(path.into());
            self.entries.insert(
                path.into(),
                Entry {
                    value: None,
                    checked: None,
                    generation,
                    pending: true,
                },
            );
        }
        self.queue.push_back(Job {
            path: path.into(),
            generation,
        });
        None
    }
    fn make_room(&mut self) {
        while self.entries.len() >= self.max_cache {
            if let Some(path) = self.order.pop_front() {
                self.entries.remove(&path);
            } else {
                break;
            }
        }
    }
    fn invalidate(&mut self, path: &Path) {
        self.entries.remove(path);
        self.order.retain(|p| p != path);
        self.queue.retain(|job| job.path != path);
        // In-flight jobs retain an older generation. A subsequent read gets a
        // new generation, so their results cannot overwrite fresh state.
    }
    fn take_batch(&mut self) -> Vec<Job> {
        let mut jobs = Vec::new();
        while jobs.len() < BATCH_SIZE {
            let Some(job) = self.queue.pop_front() else {
                break;
            };
            if self
                .entries
                .get(&job.path)
                .is_some_and(|entry| entry.pending && entry.generation == job.generation)
            {
                jobs.push(job);
            }
        }
        jobs
    }
    fn apply(&mut self, results: Vec<ResultItem>, now: Instant) -> usize {
        let mut accepted = 0;
        for ResultItem { job, value } in results {
            if let Some(entry) = self
                .entries
                .get_mut(&job.path)
                .filter(|entry| entry.pending && entry.generation == job.generation)
            {
                entry.value = value;
                entry.checked = Some(now);
                entry.pending = false;
                accepted += 1;
            }
        }
        accepted
    }
    #[cfg(any(test, feature = "ui-checks"))]
    fn seed_at(&mut self, path: &Path, value: Option<bool>, now: Instant) {
        self.invalidate(path);
        if self.max_cache == 0 {
            return;
        }
        self.make_room();
        let generation = self.generation();
        self.order.push_back(path.into());
        self.entries.insert(
            path.into(),
            Entry {
                value,
                checked: Some(now),
                generation,
                pending: false,
            },
        );
    }
}

fn read_batch(jobs: Vec<Job>, reader: impl Fn(&Path) -> Option<bool>) -> Vec<ResultItem> {
    jobs.into_iter()
        .map(|job| {
            let value = reader(&job.path);
            ResultItem { job, value }
        })
        .collect()
}

struct Running {
    receiver: mpsc::Receiver<Vec<ResultItem>>,
    jobs: Vec<Job>,
}
struct State {
    cache: Cache,
    running: Option<Running>,
}
#[derive(Default)]
struct Drain {
    active: bool,
    changed: bool,
}
impl State {
    fn new() -> Self {
        Self {
            cache: Cache::new(MAX_CACHE, MAX_PENDING),
            running: None,
        }
    }
    fn drive(&mut self, reader: fn(&Path) -> Option<bool>) -> Drain {
        let completed =
            self.running
                .as_ref()
                .and_then(|running| match running.receiver.try_recv() {
                    Ok(results) => Some(results),
                    Err(mpsc::TryRecvError::Disconnected) => {
                        Some(read_batch(running.jobs.clone(), |_| None))
                    }
                    Err(mpsc::TryRecvError::Empty) => None,
                });
        let mut changed = false;
        if let Some(results) = completed {
            self.running = None;
            changed = self.cache.apply(results, Instant::now()) > 0;
        }
        if self.running.is_none() {
            let jobs = self.cache.take_batch();
            if !jobs.is_empty() {
                let (sender, receiver) = mpsc::channel();
                let work = jobs.clone();
                match std::thread::Builder::new()
                    .name("schnauz-cloud-status".into())
                    .spawn(move || {
                        let _ = sender.send(read_batch(work, reader));
                    }) {
                    Ok(_) => self.running = Some(Running { receiver, jobs }),
                    Err(_) => {
                        changed |= self.cache.apply(read_batch(jobs, |_| None), Instant::now()) > 0;
                    }
                }
            }
        }
        Drain {
            active: self.running.is_some() || !self.cache.queue.is_empty(),
            changed,
        }
    }
}

static STATE: OnceLock<Mutex<State>> = OnceLock::new();
static POLLING: AtomicBool = AtomicBool::new(false);
fn state() -> &'static Mutex<State> {
    STATE.get_or_init(|| Mutex::new(State::new()))
}

/// `Some(true)` is online-only, `Some(false)` has local data, and `None` is
/// pending/unavailable. Lookup itself performs no filesystem access.
pub(super) fn read(path: &Path) -> Option<bool> {
    state().lock().unwrap().cache.read_at(path, Instant::now())
}
pub(super) fn invalidate(path: &Path) {
    state().lock().unwrap().cache.invalidate(path);
}

#[cfg(any(test, feature = "ui-checks"))]
pub(super) fn seed(path: &Path, dataless: Option<bool>) {
    state()
        .lock()
        .unwrap()
        .cache
        .seed_at(path, dataless, Instant::now());
}

fn metadata(path: &Path) -> Option<bool> {
    use std::os::macos::fs::MetadataExt;
    std::fs::metadata(path)
        .ok()
        .map(|metadata| metadata.st_flags() & 0x4000_0000 != 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn cached_lookup_never_invokes_reader_and_deduplicates_expired_work() {
        let mut cache = Cache::new(8, 8);
        let path = Path::new("/fixture/file");
        let now = Instant::now();
        let calls = AtomicUsize::new(0);
        for _ in 0..1000 {
            assert_eq!(cache.read_at(path, now), None);
        }
        assert_eq!(calls.load(Ordering::Relaxed), 0);
        assert_eq!(cache.queue.len(), 1);
        let results = read_batch(cache.take_batch(), |_| {
            calls.fetch_add(1, Ordering::Relaxed);
            Some(true)
        });
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert_eq!(cache.apply(results, now), 1);
        assert_eq!(
            cache.read_at(path, now + Duration::from_secs(2)),
            Some(true)
        );
        for _ in 0..1000 {
            assert_eq!(cache.read_at(path, now + TTL), None);
        }
        assert_eq!(cache.queue.len(), 1);
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn invalidation_rejects_inflight_results_even_after_the_same_path_is_requested() {
        let mut cache = Cache::new(8, 8);
        let path = Path::new("/fixture/file");
        let now = Instant::now();
        cache.read_at(path, now);
        let old = cache.take_batch();
        cache.invalidate(path);
        cache.read_at(path, now);
        assert_eq!(cache.apply(read_batch(old, |_| Some(true)), now), 0);
        assert_eq!(cache.read_at(path, now), None);
        let fresh = cache.take_batch();
        assert_eq!(cache.apply(read_batch(fresh, |_| Some(false)), now), 1);
        assert_eq!(cache.read_at(path, now), Some(false));
    }

    #[test]
    fn eviction_and_fixture_replacement_reject_old_worker_results() {
        let mut cache = Cache::new(1, 8);
        let path = Path::new("/fixture/evicted");
        let now = Instant::now();
        cache.read_at(path, now);
        let old = cache.take_batch();
        cache.read_at(Path::new("/fixture/replacement"), now);
        cache.read_at(path, now);
        assert_eq!(cache.apply(read_batch(old, |_| Some(true)), now), 0);
        let fresh = cache.take_batch();
        assert_eq!(fresh.len(), 1, "Evicted pending work is skipped");
        cache.seed_at(path, Some(false), now);
        assert_eq!(cache.apply(read_batch(fresh, |_| Some(true)), now), 0);
        assert_eq!(cache.read_at(path, now), Some(false));
    }

    #[test]
    fn queue_cache_and_fifo_are_bounded_under_repeated_path_churn() {
        let mut cache = Cache::new(4, 2);
        let now = Instant::now();
        for index in 0..1000 {
            cache.read_at(&PathBuf::from(format!("/fixture/{index}")), now);
            assert!(cache.entries.len() <= 4 && cache.queue.len() <= 2 && cache.order.len() <= 4);
            if index % 3 == 0 {
                let jobs = cache.take_batch();
                cache.apply(read_batch(jobs, |_| Some(false)), now);
            }
        }
    }

    #[test]
    fn read_failures_stay_unknown_and_are_cached_instead_of_retried_each_frame() {
        let mut cache = Cache::new(2, 2);
        let path = Path::new("/fixture/unreadable");
        let now = Instant::now();
        cache.read_at(path, now);
        let jobs = cache.take_batch();
        cache.apply(read_batch(jobs, |_| None), now);
        for _ in 0..1000 {
            assert_eq!(cache.read_at(path, now + Duration::from_secs(1)), None);
        }
        assert!(cache.queue.is_empty());
        cache.read_at(path, now + TTL);
        assert_eq!(cache.queue.len(), 1);
    }

    #[test]
    fn worker_metadata_reader_runs_on_a_dedicated_thread() {
        fn fake(_: &Path) -> Option<bool> {
            assert_eq!(std::thread::current().name(), Some("schnauz-cloud-status"));
            Some(false)
        }
        let mut state = State::new();
        let path = Path::new("/fixture/worker");
        state.cache.read_at(path, Instant::now());
        assert!(state.drive(fake).active);
        let deadline = Instant::now() + Duration::from_secs(2);
        while state.drive(fake).active && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert_eq!(state.cache.read_at(path, Instant::now()), Some(false));
    }

    #[test]
    fn a_completed_batch_publishes_one_change_signal_for_all_its_items() {
        fn fake(_: &Path) -> Option<bool> {
            Some(false)
        }
        let mut state = State::new();
        let now = Instant::now();
        for index in 0..BATCH_SIZE {
            state
                .cache
                .read_at(Path::new(&format!("/fixture/{index}")), now);
        }
        assert!(!state.drive(fake).changed);
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut changes = 0;
        loop {
            let drain = state.drive(fake);
            changes += usize::from(drain.changed);
            if !drain.active || Instant::now() >= deadline {
                break;
            }
            std::thread::yield_now();
        }
        assert_eq!(changes, 1);
        assert!(!state.drive(fake).changed);
        assert!(state
            .cache
            .entries
            .values()
            .all(|entry| entry.checked.is_some() && !entry.pending));
    }

    #[test]
    fn benchmark_one_hundred_thousand_warm_lookups_schedule_no_work() {
        let now = Instant::now();
        let path = Path::new("/fixture/warm");
        let mut cache = Cache::new(8, 8);
        cache.seed_at(path, Some(false), now);
        let cache = Mutex::new(cache);
        let start = Instant::now();
        for _ in 0..100_000 {
            assert_eq!(cache.lock().unwrap().read_at(path, now), Some(false));
        }
        eprintln!(
            "100,000 warm cloud-status lookups: {:?}; no I/O or queued jobs",
            start.elapsed()
        );
        assert!(cache.lock().unwrap().queue.is_empty());
    }
}

// GPUI integration: call after lazy row layout has queued its paths (e.g. in
// the browser's existing on_next_frame callback). Local/cache-hit-only frames
// do not start a timer or worker.
use crate::Schnauz;
impl Schnauz {
    pub(super) fn pump_cloud_status(&mut self, cx: &mut gpui::Context<Self>) {
        let drain = state().lock().unwrap().drive(metadata);
        if drain.changed {
            cx.refresh_windows();
        }
        if !drain.active {
            return;
        }
        if POLLING
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        struct PollGuard;
        impl Drop for PollGuard {
            fn drop(&mut self) {
                POLLING.store(false, Ordering::Release);
            }
        }
        cx.spawn(async move |this, cx| {
            let _guard = PollGuard;
            loop {
                cx.background_executor().timer(POLL_INTERVAL).await;
                let active = this.update(cx, |_, cx| {
                    let drain = state().lock().unwrap().drive(metadata);
                    if drain.changed {
                        cx.refresh_windows();
                    }
                    drain.active
                });
                let Ok(active) = active else {
                    break;
                };
                if !active {
                    break;
                }
            }
        })
        .detach();
    }
}
