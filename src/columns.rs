//! Shared column listings and cached visibility masks; rows borrow their entries.
use super::*;
struct Listing {
    entries: Arc<Vec<Entry>>,
    full: bool,
    masks: RefCell<HashMap<(EntryFilter, bool), Arc<Vec<usize>>>>,
}
impl Listing {
    fn new(entries: Vec<Entry>, full: bool) -> Self {
        Self {
            entries: Arc::new(entries),
            full,
            masks: Default::default(),
        }
    }
    fn visible(&self, filter: EntryFilter, show_hidden: bool) -> Entries<'static> {
        let key = (filter, show_hidden);
        let cached = self.masks.borrow().get(&key).cloned();
        let indices = cached.unwrap_or_else(|| {
            let indices = Arc::new(
                self.entries
                    .iter()
                    .enumerate()
                    .filter_map(|(i, e)| {
                        (filter.accepts(e.is_folder()) && (show_hidden || !e.is_hidden()))
                            .then_some(i)
                    })
                    .collect(),
            );
            self.masks.borrow_mut().insert(key, Arc::clone(&indices));
            indices
        });
        Entries::Shared {
            entries: Arc::clone(&self.entries),
            indices,
        }
    }
}
thread_local! {
    static LISTINGS:RefCell<Cache<PathBuf,Arc<Listing>>> = RefCell::new(Cache::new(128,64*1024*1024,
        |l| l.entries.iter().map(|e|std::mem::size_of::<Entry>()+e.name.len()+6*std::mem::size_of::<usize>()).sum(), |_|false));
    static PENDING:RefCell<HashSet<PathBuf>> = RefCell::new(HashSet::new());
}
pub(super) enum Entries<'a> {
    Root(&'a Tab),
    Shared {
        entries: Arc<Vec<Entry>>,
        indices: Arc<Vec<usize>>,
    },
}
impl Entries<'_> {
    pub fn len(&self) -> usize {
        match self {
            Self::Root(tab) => tab.visible_count(),
            Self::Shared { indices, .. } => indices.len(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn get(&self, row: usize) -> Option<&Entry> {
        match self {
            Self::Root(tab) => {
                let index = if tab.is_filtered() {
                    *tab.find_results.get(row)?
                } else {
                    row
                };
                tab.entries.get(index)
            }
            Self::Shared { entries, indices } => entries.get(*indices.get(row)?),
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = &Entry> {
        (0..self.len()).filter_map(|i| self.get(i))
    }
}
fn listing(dir: &Path) -> Arc<Listing> {
    if let Some(entries) = LISTINGS.with(|c| c.borrow().get(dir).cloned()) {
        return entries;
    }
    let entries = Arc::new(Listing::new(read_entries_fast(dir), false));
    LISTINGS.with(|c| c.borrow_mut().insert(dir.to_path_buf(), entries.clone()));
    entries
}
pub(super) fn clear_column_cache() {
    LISTINGS.with(|c| c.borrow_mut().clear());
}
pub(super) fn forget_column_listing(dir: &Path) {
    LISTINGS.with(|c| {
        c.borrow_mut().remove(dir);
    });
}
impl Shuffle {
    pub(super) fn visible_column_entries<'a>(&'a self, pane: usize, dir: &Path) -> Entries<'a> {
        let tab = self.tab(pane);
        if dir == tab.current_dir {
            return Entries::Root(tab);
        }
        listing(dir).visible(tab.entry_filter, tab.show_hidden)
    }
    pub(super) fn ensure_column_metadata(&self, cx: &mut Context<Self>) {
        let dirs: HashSet<PathBuf> = self
            .panes
            .iter()
            .map(Pane::active_tab)
            .filter(|t| t.remote.is_none() && t.view == ViewMode::Columns)
            .flat_map(|t| t.col_chain.iter().cloned())
            .collect();
        for dir in dirs {
            let current = listing(&dir);
            if current.full || !PENDING.with(|p| p.borrow_mut().insert(dir.clone())) {
                continue;
            }
            cx.spawn(async move |this, cx| {
                let path = dir.clone();
                let entries = cx
                    .background_spawn(async move { read_entries(&path) })
                    .await;
                PENDING.with(|p| {
                    p.borrow_mut().remove(&dir);
                });
                let unchanged = LISTINGS.with(|c| {
                    c.borrow()
                        .get(&dir)
                        .is_some_and(|l| Arc::ptr_eq(l, &current))
                });
                if unchanged {
                    LISTINGS.with(|c| {
                        c.borrow_mut()
                            .insert(dir, Arc::new(Listing::new(entries, true)))
                    });
                    let _ = this.update(cx, |_, cx| cx.notify());
                }
            })
            .detach();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn columns_share_entries_and_masks_without_losing_hidden_or_application_filters() {
        let entries = [
            ("Folder", true, false),
            ("Tool.app", true, false),
            (".secret", false, false),
            ("hidden.txt", false, true),
            ("notes.txt", false, false),
        ]
        .into_iter()
        .map(|(name, is_dir, hidden_flag)| Entry {
            name: name.into(),
            is_dir,
            size: 0,
            modified: None,
            created: None,
            loaded: true,
            hidden_flag,
        })
        .collect();
        let listing = Listing::new(entries, true);
        let names = |e: Entries<'_>| e.iter().map(|e| e.name.clone()).collect::<Vec<_>>();
        assert_eq!(
            names(listing.visible(EntryFilter::All, false)),
            ["Folder", "Tool.app", "notes.txt"]
        );
        assert_eq!(
            names(listing.visible(EntryFilter::Folders, true)),
            ["Folder"]
        );
        assert_eq!(
            names(listing.visible(EntryFilter::Files, false)),
            ["Tool.app", "notes.txt"]
        );
        assert_eq!(listing.visible(EntryFilter::Files, true).len(), 4);
        let Entries::Shared {
            entries: a,
            indices: ai,
        } = listing.visible(EntryFilter::All, false)
        else {
            panic!()
        };
        let Entries::Shared {
            entries: b,
            indices: bi,
        } = listing.visible(EntryFilter::All, false)
        else {
            panic!()
        };
        assert!(Arc::ptr_eq(&a, &b));
        assert!(Arc::ptr_eq(&ai, &bi));
    }
}
