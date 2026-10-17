//! Small weighted LRU caches: bounded decoded memory, shared values, no dependencies.
use std::{borrow::Borrow, cell::Cell, collections::HashMap, hash::Hash};
struct Item<V> {
    value: V,
    bytes: usize,
    touched: Cell<u64>,
}
pub struct Cache<K, V> {
    items: HashMap<K, Item<V>>,
    clock: Cell<u64>,
    bytes: usize,
    max_items: usize,
    max_bytes: usize,
    weigh: fn(&V) -> usize,
    pinned: fn(&K) -> bool,
}
impl<K: Eq + Hash + Clone, V> Cache<K, V> {
    pub fn new(
        max_items: usize,
        max_bytes: usize,
        weigh: fn(&V) -> usize,
        pinned: fn(&K) -> bool,
    ) -> Self {
        Self {
            items: HashMap::new(),
            clock: Cell::new(0),
            bytes: 0,
            max_items,
            max_bytes,
            weigh,
            pinned,
        }
    }
    fn tick(&self) -> u64 {
        let tick = self.clock.get().wrapping_add(1);
        self.clock.set(tick);
        tick
    }
    pub fn get<Q: Eq + Hash + ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
    {
        let item = self.items.get(key)?;
        item.touched.set(self.tick());
        Some(&item.value)
    }
    pub fn contains_key<Q: Eq + Hash + ?Sized>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
    {
        self.get(key).is_some()
    }
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        let bytes = (self.weigh)(&value);
        if bytes > self.max_bytes && !(self.pinned)(&key) {
            return None;
        }
        let tick = self.tick();
        let old = self.items.insert(
            key,
            Item {
                value,
                bytes,
                touched: Cell::new(tick),
            },
        );
        if let Some(old) = &old {
            self.bytes = self.bytes.saturating_sub(old.bytes);
        }
        self.bytes = self.bytes.saturating_add(bytes);
        while self.items.len() > self.max_items || self.bytes > self.max_bytes {
            let oldest = self
                .items
                .iter()
                .filter(|(k, _)| !(self.pinned)(k))
                .min_by_key(|(_, v)| v.touched.get())
                .map(|(k, _)| k.clone());
            let Some(oldest) = oldest else {
                break;
            };
            self.remove(&oldest);
        }
        old.map(|item| item.value)
    }
    pub fn remove<Q: Eq + Hash + ?Sized>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
    {
        let item = self.items.remove(key)?;
        self.bytes = self.bytes.saturating_sub(item.bytes);
        Some(item.value)
    }
    pub fn clear(&mut self) {
        self.items.clear();
        self.bytes = 0;
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn retain(&mut self, mut keep: impl FnMut(&K, &mut V) -> bool) {
        self.items.retain(|k, item| keep(k, &mut item.value));
        self.bytes = self.items.values().map(|v| v.bytes).sum();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn enforces_weight_count_and_lru_without_evicting_pinned_values() {
        let mut cache = Cache::new(3, 10, |v: &String| v.len(), |k: &String| k == "base");
        cache.insert("base".into(), "xx".into());
        cache.insert("a".into(), "aaa".into());
        cache.insert("b".into(), "bbb".into());
        assert_eq!(cache.get("a").map(String::as_str), Some("aaa"));
        cache.insert("c".into(), "ccc".into());
        assert!(cache.get("b").is_none());
        assert!(cache.contains_key("base"));
        cache.insert("huge".into(), "x".repeat(20));
        assert!(!cache.contains_key("huge"));
        assert_eq!(cache.len(), 3);
        cache.remove("a");
        cache.insert("d".into(), "ddddd".into());
        assert!(cache.bytes <= 10);
        cache.retain(|k, _| k == "base");
        assert_eq!(cache.len(), 1);
        cache.clear();
        assert_eq!(cache.bytes, 0);
    }
}
