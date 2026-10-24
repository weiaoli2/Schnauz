//! Cached metadata from the installed Nextcloud File Provider, never credentials.
use super::*;
use std::collections::VecDeque;

const LIMIT: usize = 1024;
const QUEUE_LIMIT: usize = 256;
const TTL: Duration = Duration::from_secs(5);
const ERROR_TTL: Duration = Duration::from_secs(30);
// Display may retain a confirmed state during a bounded in-flight refresh.
// Menu eligibility always uses TTL and never consumes this grace period.
const DISPLAY_TTL: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Snapshot {
    pub path: PathBuf,
    provider: String,
    pub folder: bool,
    pub downloaded: bool,
    pub recursively_downloaded: bool,
    pub downloading: bool,
    pub keep_downloaded: bool,
    can_keep_downloaded: bool,
    error: Option<String>,
}
impl Snapshot {
    fn valid(&self) -> bool {
        self.error.is_none() && self.provider == "com.nextcloud.desktopclient.FileProviderExt"
    }
    pub fn can_keep(&self) -> bool {
        self.valid()
            && self.can_keep_downloaded
            && !self.downloading
            && !self.keep_downloaded
            && if self.folder {
                !self.recursively_downloaded
            } else {
                !self.downloaded
            }
    }
}
struct Entry {
    value: Option<Snapshot>,
    error: Option<String>,
    checked: Option<Instant>,
    generation: u64,
    queued: bool,
}
#[derive(Default)]
struct Cache {
    entries: HashMap<PathBuf, Entry>,
    queue: VecDeque<(PathBuf, u64)>,
    generation: u64,
    busy: bool,
    pins: HashSet<PathBuf>,
    #[cfg(feature = "ui-checks")]
    fixture: bool,
}
thread_local! { static CACHE: RefCell<Cache> = RefCell::new(Cache::default()); }

impl Cache {
    fn read_at(&mut self, path: &Path, now: Instant, display: bool) -> Option<Snapshot> {
        let previous = self.entries.get(path).and_then(|entry| {
            (display && entry.checked.is_some_and(|at| now.saturating_duration_since(at) < DISPLAY_TTL))
                .then(|| entry.value.clone()).flatten()
        });
        if let Some(entry) = self.entries.get(path) {
            let ttl = if entry.value.is_some() {
                TTL
            } else {
                ERROR_TTL
            };
            if entry.checked.is_some_and(|at| now.saturating_duration_since(at) < ttl) {
                return entry.value.clone();
            }
            if entry.queued {
                return previous;
            }
        }
        #[cfg(feature = "ui-checks")]
        if self.fixture {
            return previous;
        }
        if self.queue.len() >= QUEUE_LIMIT {
            return previous;
        }
        // Refresh an existing entry in place; do not evict unrelated rows or
        // erase its confirmed display value just because it has expired.
        if !self.entries.contains_key(path) && self.entries.len() >= LIMIT {
            let oldest = self
                .entries
                .iter()
                .filter(|(_, e)| !e.queued)
                .min_by_key(|(_, e)| e.checked)
                .map(|(p, _)| p.clone());
            if let Some(oldest) = oldest {
                self.entries.remove(&oldest);
            } else {
                return previous;
            }
        }
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let entry = self.entries.entry(path.into()).or_insert_with(|| Entry {
                value: None,
                error: None,
                checked: None,
                generation,
                queued: false,
            });
        entry.generation = generation;
        entry.queued = true;
        self.queue.push_back((path.into(), generation));
        previous
    }
}

/// Fresh provider metadata for action eligibility. No stale values are returned.
pub(super) fn read(path: &Path) -> Option<Snapshot> {
    CACHE.with(|cache| cache.borrow_mut().read_at(path, Instant::now(), false))
}

/// Keep a confirmed icon stable while its background refresh is pending.
pub(super) fn display(path: &Path) -> Option<Snapshot> {
    CACHE.with(|cache| cache.borrow_mut().read_at(path, Instant::now(), true))
}
pub(super) fn invalidate(path: &Path) {
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.entries.remove(path);
        cache.queue.retain(|(p, _)| p != path);
    });
}
pub(super) fn pin_started(path: &Path) {
    invalidate(path);
    CACHE.with(|c| {
        c.borrow_mut().pins.insert(path.into());
    });
}
pub(super) fn pin_finished(path: &Path) {
    invalidate(path);
    CACHE.with(|c| {
        c.borrow_mut().pins.remove(path);
    });
}
pub(super) fn pin_busy(path: &Path) -> bool {
    CACHE.with(|c| c.borrow().pins.contains(path))
}
pub(super) fn error(path: &Path) -> Option<String> {
    CACHE.with(|c| {
        c.borrow()
            .entries
            .get(path)
            .filter(|e| e.checked.is_some_and(|at| at.elapsed() < ERROR_TTL))
            .and_then(|e| e.error.clone())
    })
}

impl Schnauz {
    pub(super) fn pump_nextcloud_local(&mut self, cx: &mut Context<Self>) {
        let jobs = CACHE.with(|c| {
            let mut c = c.borrow_mut();
            if c.busy || c.queue.is_empty() {
                return Vec::new();
            }
            c.busy = true;
            let count = c.queue.len().min(32);
            c.queue.drain(..count).collect::<Vec<_>>()
        });
        if jobs.is_empty() {
            return;
        }
        let paths = jobs.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>();
        let (tx, rx) = futures::channel::oneshot::channel();
        // Owned worker: process startup and metadata IPC never occupy GPUI's
        // shared file-work pool or block a render/menu callback.
        let worker = std::thread::Builder::new()
            .name("nextcloud-local-status".into())
            .spawn(move || {
                let input = serde_json::to_vec(
                    &serde_json::json!({"operation":"local-status","config":"","paths":paths}),
                );
                let values = input
                    .map_err(|e| e.to_string())
                    .and_then(|input| nextcloud::helper_request(&input))
                    .and_then(|reply| {
                        if reply.get("success").and_then(|s| s.as_bool()) != Some(true) {
                            return Err(reply
                                .get("message")
                                .and_then(|s| s.as_str())
                                .unwrap_or("Nextcloud local status is unavailable.")
                                .to_string());
                        }
                        serde_json::from_value::<Vec<serde_json::Value>>(
                            reply.get("items").cloned().unwrap_or_default(),
                        )
                        .map_err(|_| "Nextcloud returned incomplete local metadata.".into())
                    });
                let _ = tx.send(values);
            });
        let failed = worker.is_err();
        cx.spawn(async move |this, cx| {
            let values = if failed {
                Err("Cannot start the Nextcloud status worker.".into())
            } else {
                rx.await
                    .unwrap_or_else(|_| Err("The Nextcloud status worker stopped.".into()))
            };
            let (values, error) = match values {
                Ok(values) => (values, None),
                Err(error) => (Vec::new(), Some(error)),
            };
            let values = values
                .into_iter()
                .filter_map(|value| {
                    value
                        .get("path")
                        .and_then(|s| s.as_str())
                        .map(|p| (PathBuf::from(p), value.clone()))
                })
                .collect::<HashMap<_, _>>();
            let _ = cx.update(|cx| {
                CACHE.with(|c| {
                    let mut c = c.borrow_mut();
                    c.busy = false;
                    for (path, generation) in jobs {
                        if let Some(entry) = c
                            .entries
                            .get_mut(&path)
                            .filter(|e| e.generation == generation)
                        {
                            entry.value = values
                                .get(&path)
                                .and_then(|s| serde_json::from_value::<Snapshot>(s.clone()).ok())
                                .filter(|s| s.valid());
                            entry.error = if entry.value.is_some() {
                                None
                            } else {
                                Some(
                                    values
                                        .get(&path)
                                        .and_then(|v| v.get("error"))
                                        .and_then(|s| s.as_str())
                                        .map(str::to_string)
                                        .or_else(|| error.clone())
                                        .unwrap_or_else(|| {
                                            "Nextcloud local metadata is unavailable.".into()
                                        }),
                                )
                            };
                            entry.checked = Some(Instant::now());
                            entry.queued = false;
                        }
                    }
                });
                cx.refresh_windows();
            });
            let _ = this.update(cx, |this, cx| this.pump_nextcloud_local(cx));
        })
        .detach();
    }
}

#[cfg(feature = "ui-checks")]
pub(super) fn fixture_mode() {
    CACHE.with(|c| c.borrow_mut().fixture = true);
}
#[cfg(feature = "ui-checks")]
pub(super) fn seed(path: &Path, folder: bool, downloaded: bool, recursively: bool) {
    CACHE.with(|c| {
        c.borrow_mut().entries.insert(
            path.into(),
            Entry {
                value: Some(Snapshot {
                    path: path.into(),
                    provider: "com.nextcloud.desktopclient.FileProviderExt".into(),
                    folder,
                    downloaded,
                    recursively_downloaded: recursively,
                    can_keep_downloaded: true,
                    ..Default::default()
                }),
                checked: Some(Instant::now()),
                error: None,
                generation: 0,
                queued: false,
            },
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn confirmed_cache(now: Instant) -> (Cache, PathBuf) {
        let path = PathBuf::from("/fixture/kept-folder");
        let mut cache = Cache::default();
        cache.entries.insert(path.clone(), Entry {
            value: Some(Snapshot { path:path.clone(), provider:"com.nextcloud.desktopclient.FileProviderExt".into(),
                folder:true, downloaded:true, recursively_downloaded:true, keep_downloaded:true, ..Default::default() }),
            error:None, checked:Some(now), generation:0, queued:false,
        });
        (cache,path)
    }
    #[test]
    fn pending_refresh_preserves_pin_for_display_but_not_for_actions() {
        let now=Instant::now();
        let (mut cache,path)=confirmed_cache(now);
        assert!(cache.read_at(&path,now,false).unwrap().keep_downloaded);
        // A menu can be the first reader to start the refresh.
        assert!(cache.read_at(&path,now+TTL,false).is_none());
        for _ in 0..100 {
            assert!(cache.read_at(&path,now+TTL,true).unwrap().keep_downloaded);
            assert!(cache.read_at(&path,now+TTL,false).is_none());
        }
        assert_eq!(cache.queue.len(),1,"Repaints must not duplicate provider requests");
        assert!(cache.entries[&path].value.as_ref().unwrap().keep_downloaded);
        assert!(cache.read_at(&path,now+DISPLAY_TTL,true).is_none(),"A hung refresh cannot keep displaying stale status forever");
    }
    #[test]
    fn expired_display_starts_refresh_without_erasing_the_confirmed_state() {
        let now=Instant::now();
        let (mut cache,path)=confirmed_cache(now);
        assert!(cache.read_at(&path,now+TTL,true).unwrap().keep_downloaded);
        assert_eq!(cache.queue.len(),1);
        assert!(cache.entries[&path].queued);
        assert_eq!(cache.entries[&path].checked,Some(now),"Display grace must never renew freshness for actions");
    }
    #[test]
    fn keep_is_only_offered_for_nonlocal_nextcloud_content() {
        let mut s = Snapshot {
            provider: "com.nextcloud.desktopclient.FileProviderExt".into(),
            can_keep_downloaded: true,
            ..Default::default()
        };
        assert!(s.can_keep());
        s.downloaded = true;
        assert!(!s.can_keep());
        s.folder = true;
        assert!(s.can_keep());
        s.recursively_downloaded = true;
        assert!(!s.can_keep());
        s.recursively_downloaded = false;
        s.keep_downloaded = true;
        assert!(!s.can_keep());
        s.keep_downloaded = false;
        s.provider = "another.provider".into();
        assert!(!s.can_keep());
    }
    #[test]
    fn incomplete_or_wrongly_typed_native_metadata_cannot_enable_keep() {
        let valid = serde_json::json!({"path":"/fixture/a","provider":"com.nextcloud.desktopclient.FileProviderExt",
            "folder":false,"downloaded":false,"recursivelyDownloaded":false,"downloading":false,
            "keepDownloaded":false,"canKeepDownloaded":true});
        assert!(serde_json::from_value::<Snapshot>(valid.clone())
            .unwrap()
            .can_keep());
        for key in [
            "downloaded",
            "folder",
            "recursivelyDownloaded",
            "downloading",
            "keepDownloaded",
            "canKeepDownloaded",
        ] {
            let mut missing = valid.clone();
            missing.as_object_mut().unwrap().remove(key);
            assert!(serde_json::from_value::<Snapshot>(missing).is_err());
            let mut wrong = valid.clone();
            wrong[key] = serde_json::json!("unknown");
            assert!(serde_json::from_value::<Snapshot>(wrong).is_err());
        }
    }
}
