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
    #[serde(default)]
    pub uploading: Option<bool>,
    #[serde(default)]
    pub uploaded: Option<bool>,
    #[serde(default)]
    pub upload_failed: bool,
    pub keep_downloaded: bool,
    can_keep_downloaded: bool,
    error: Option<String>,
}
impl Snapshot {
    pub fn syncing(&self) -> bool {
        self.downloading
            || self.uploading == Some(true)
            || (!self.folder && self.uploaded == Some(false))
    }
    pub fn rename_ready(&self) -> bool {
        self.valid()
            && !self.syncing()
            && !self.upload_failed
            && self.uploading == Some(false)
            && self.uploaded == Some(true)
    }
    fn valid(&self) -> bool {
        self.error.is_none() && self.provider == "com.nextcloud.desktopclient.FileProviderExt"
    }
    pub fn can_keep(&self) -> bool {
        self.valid()
            && self.can_keep_downloaded
            && !self.syncing()
            && !self.upload_failed
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
    polling: bool,
    last_display: Option<Instant>,
    pins: HashSet<PathBuf>,
    #[cfg(feature = "ui-checks")]
    fixture: bool,
}
thread_local! { static CACHE: RefCell<Cache> = RefCell::new(Cache::default()); }

impl Cache {
    fn read_at(&mut self, path: &Path, now: Instant, display: bool) -> Option<Snapshot> {
        if display {
            self.last_display = Some(now);
        }
        let previous = self.entries.get(path).and_then(|entry| {
            (display
                && entry
                    .checked
                    .is_some_and(|at| now.saturating_duration_since(at) < DISPLAY_TTL))
            .then(|| entry.value.clone())
            .flatten()
        });
        if let Some(entry) = self.entries.get(path) {
            let ttl = if let Some(value) = &entry.value {
                if value.syncing() {
                    Duration::from_secs(1)
                } else {
                    TTL
                }
            } else {
                Duration::from_secs(2)
            };
            if entry
                .checked
                .is_some_and(|at| now.saturating_duration_since(at) < ttl)
            {
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
pub(super) fn invalidate_folder(folder: &Path) {
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        c.entries.retain(|p, _| p.parent() != Some(folder));
        c.queue.retain(|(p, _)| p.parent() != Some(folder));
    });
}

fn validate_rename_reply(paths: &[PathBuf], reply: &serde_json::Value) -> Result<(), String> {
    let items: Vec<Snapshot> =
        serde_json::from_value(reply.get("items").cloned().unwrap_or_default()).map_err(|_| {
            "Couldn't confirm Nextcloud sync status. Try again shortly.".to_string()
        })?;
    if reply.get("success").and_then(|v| v.as_bool()) != Some(true)
        || paths.iter().any(|path| {
            !items
                .iter()
                .any(|item| item.path == *path && item.rename_ready())
        })
    {
        return Err(
            "Nextcloud has not confirmed syncing is finished. Rename was not performed.".into(),
        );
    }
    Ok(())
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
    pub(super) fn nextcloud_sync_item(&self, pane: usize, path: &Path, cx: &App) -> bool {
        self.tab(pane).remote.is_none()
            && cloud_kind(path) == Some(CloudKind::Provider)
            && (nextcloud::location(path, cx).is_some()
                || cloud_locations::identify(&home_dir(), path)
                    .is_some_and(|p| p.name == "Nextcloud"))
    }
    pub(super) fn nextcloud_rename_wait(
        &self,
        pane: usize,
        paths: &[PathBuf],
        cx: &App,
    ) -> Option<&'static str> {
        for path in paths
            .iter()
            .filter(|p| self.nextcloud_sync_item(pane, p, cx))
        {
            if self.cloud_busy.contains(path) || pin_busy(path) {
                return Some("Wait for Nextcloud to finish syncing before renaming.");
            }
            match read(path) {
                Some(item) if item.rename_ready() => {}
                Some(_) => return Some("Wait for Nextcloud to finish syncing before renaming."),
                None => return Some("Checking Nextcloud sync status. Try renaming again shortly."),
            }
        }
        None
    }
    /// Fresh read-only provider check immediately before a rename. Cache display
    /// grace must never authorize a write; changed/cancelled drafts are not run.
    pub(super) fn check_nextcloud_rename(
        &mut self,
        pane: usize,
        paths: &[PathBuf],
        cx: &mut Context<Self>,
        ready: impl FnOnce(&mut Self, &mut Context<Self>) + 'static,
    ) -> bool {
        let paths: Vec<_> = paths
            .iter()
            .filter(|p| self.nextcloud_sync_item(pane, p, cx))
            .cloned()
            .collect();
        if paths.is_empty() {
            return false;
        }
        if paths
            .iter()
            .any(|p| self.cloud_busy.contains(p) || pin_busy(p))
        {
            self.set_notice(
                "Wait for Nextcloud to finish syncing before renaming.".into(),
                cx,
            );
            return true;
        }
        #[cfg(feature = "ui-checks")]
        if CACHE.with(|c| c.borrow().fixture) {
            if paths
                .iter()
                .all(|p| read(p).is_some_and(|s| s.rename_ready()))
            {
                ready(self, cx);
            } else {
                self.set_notice(
                    "Wait for Nextcloud to finish syncing before renaming.".into(),
                    cx,
                );
            }
            return true;
        }
        if self.cloud_rename_check {
            return true;
        }
        self.cloud_rename_check = true;
        let tab = self.tab(pane).id;
        let directory = self.tab(pane).current_dir.clone();
        let (tx, rx) = futures::channel::oneshot::channel();
        let worker=std::thread::Builder::new().name("nextcloud-rename-check".into()).spawn(move || {
            let result=(|| -> Result<(),String> {
                for chunk in paths.chunks(32) {
                    let input=serde_json::to_vec(&serde_json::json!({"operation":"local-status","config":"","paths":chunk})).map_err(|e|e.to_string())?;
                    let reply=nextcloud::helper_request(&input)?;
                    validate_rename_reply(chunk,&reply)?;
                }
                Ok(())
            })();
            let _=tx.send(result);
        });
        let failed = worker.is_err();
        cx.spawn(async move |this, cx| {
            let result = if failed {
                Err("Couldn't start the Nextcloud sync check.".into())
            } else {
                rx.await.unwrap_or_else(|_| {
                    Err("Nextcloud sync check stopped. Rename was not performed.".into())
                })
            };
            let _ = this.update(cx, |this, cx| {
                this.cloud_rename_check = false;
                if pane >= this.panes.len()
                    || this.tab(pane).id != tab
                    || this.tab(pane).current_dir != directory
                {
                    return;
                }
                match result {
                    Ok(()) => ready(this, cx),
                    Err(error) => this.set_notice(error, cx),
                }
                cx.notify();
            });
        })
        .detach();
        true
    }
    pub(super) fn pump_nextcloud_local(&mut self, cx: &mut Context<Self>) {
        let poll = CACHE.with(|c| {
            let mut c = c.borrow_mut();
            if c.entries.is_empty()
                || c.polling
                || !c
                    .last_display
                    .is_some_and(|at| at.elapsed() < Duration::from_secs(2))
            {
                false
            } else {
                c.polling = true;
                true
            }
        });
        if poll {
            cx.spawn(async move |_, cx| {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let _ = cx.update(|cx| {
                    CACHE.with(|c| c.borrow_mut().polling = false);
                    cx.refresh_windows();
                });
            })
            .detach();
        }
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
                    uploading: Some(false),
                    uploaded: Some(true),
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
    fn upload_item(uploading: bool, uploaded: bool) -> serde_json::Value {
        serde_json::json!({"path":"/fixture/file","provider":"com.nextcloud.desktopclient.FileProviderExt",
            "folder":false,"downloaded":true,"recursivelyDownloaded":false,"downloading":false,
            "keepDownloaded":false,"canKeepDownloaded":false,"uploading":uploading,"uploaded":uploaded})
    }
    #[test]
    fn locally_present_files_remain_syncing_until_upload_is_confirmed() {
        for (uploading, uploaded) in [(true, false), (false, false), (true, true)] {
            let item: Snapshot = serde_json::from_value(upload_item(uploading, uploaded)).unwrap();
            assert!(item.downloaded && item.syncing());
            assert!(!item.rename_ready());
        }
        let item: Snapshot = serde_json::from_value(upload_item(false, true)).unwrap();
        assert!(!item.syncing() && item.rename_ready());
        let mut error = item.clone();
        error.upload_failed = true;
        assert!(!error.rename_ready());
    }
    #[test]
    fn rename_confirmation_rejects_missing_stale_failed_and_foreign_metadata() {
        let paths = [PathBuf::from("/fixture/file")];
        let reply = |item| serde_json::json!({"success":true,"items":[item]});
        assert!(validate_rename_reply(&paths, &reply(upload_item(false, true))).is_ok());
        for (key, value) in [
            ("uploaded", serde_json::Value::Null),
            ("uploading", serde_json::json!(1)),
            ("path", serde_json::json!("/other")),
            ("provider", serde_json::json!("foreign")),
            ("uploadFailed", serde_json::json!(true)),
        ] {
            let mut item = upload_item(false, true);
            item[key] = value;
            assert!(validate_rename_reply(&paths, &reply(item)).is_err());
        }
        assert!(validate_rename_reply(&paths, &reply(upload_item(false, false))).is_err());
        assert!(
            validate_rename_reply(&paths, &serde_json::json!({"success":false,"items":[]}))
                .is_err()
        );
    }
    fn confirmed_cache(now: Instant) -> (Cache, PathBuf) {
        let path = PathBuf::from("/fixture/kept-folder");
        let mut cache = Cache::default();
        cache.entries.insert(
            path.clone(),
            Entry {
                value: Some(Snapshot {
                    path: path.clone(),
                    provider: "com.nextcloud.desktopclient.FileProviderExt".into(),
                    folder: true,
                    downloaded: true,
                    recursively_downloaded: true,
                    keep_downloaded: true,
                    ..Default::default()
                }),
                error: None,
                checked: Some(now),
                generation: 0,
                queued: false,
            },
        );
        (cache, path)
    }
    #[test]
    fn pending_refresh_preserves_pin_for_display_but_not_for_actions() {
        let now = Instant::now();
        let (mut cache, path) = confirmed_cache(now);
        assert!(cache.read_at(&path, now, false).unwrap().keep_downloaded);
        // A menu can be the first reader to start the refresh.
        assert!(cache.read_at(&path, now + TTL, false).is_none());
        for _ in 0..100 {
            assert!(
                cache
                    .read_at(&path, now + TTL, true)
                    .unwrap()
                    .keep_downloaded
            );
            assert!(cache.read_at(&path, now + TTL, false).is_none());
        }
        assert_eq!(
            cache.queue.len(),
            1,
            "Repaints must not duplicate provider requests"
        );
        assert!(cache.entries[&path].value.as_ref().unwrap().keep_downloaded);
        assert!(
            cache.read_at(&path, now + DISPLAY_TTL, true).is_none(),
            "A hung refresh cannot keep displaying stale status forever"
        );
    }
    #[test]
    fn expired_display_starts_refresh_without_erasing_the_confirmed_state() {
        let now = Instant::now();
        let (mut cache, path) = confirmed_cache(now);
        assert!(
            cache
                .read_at(&path, now + TTL, true)
                .unwrap()
                .keep_downloaded
        );
        assert_eq!(cache.queue.len(), 1);
        assert!(cache.entries[&path].queued);
        assert_eq!(
            cache.entries[&path].checked,
            Some(now),
            "Display grace must never renew freshness for actions"
        );
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
