//! A small least-recently-used cache with a byte budget, for decoded
//! artwork thumbnails (textures are kept while they fit the budget).

use std::collections::HashMap;
use std::hash::Hash;

#[derive(Debug)]
struct Entry<V> {
    value: V,
    bytes: usize,
    used: u64,
}

/// Keeps the most recently used values within `budget` bytes.
#[derive(Debug)]
pub struct Lru<K, V> {
    entries: HashMap<K, Entry<V>>,
    budget: usize,
    total: usize,
    tick: u64,
}

impl<K: Eq + Hash + Clone, V: Clone> Lru<K, V> {
    pub fn new(budget: usize) -> Self {
        Lru {
            entries: HashMap::new(),
            budget,
            total: 0,
            tick: 0,
        }
    }

    /// The value for `key`, marking it used.
    pub fn get(&mut self, key: &K) -> Option<V> {
        self.tick += 1;
        let tick = self.tick;
        self.entries.get_mut(key).map(|e| {
            e.used = tick;
            e.value.clone()
        })
    }

    /// Stores `value` and evicts the least recently used entries beyond the
    /// budget (never the one just stored).
    pub fn put(&mut self, key: K, value: V, bytes: usize) {
        self.tick += 1;
        if let Some(old) = self.entries.insert(
            key.clone(),
            Entry {
                value,
                bytes,
                used: self.tick,
            },
        ) {
            self.total -= old.bytes;
        }
        self.total += bytes;
        while self.total > self.budget && self.entries.len() > 1 {
            let Some(oldest) = self
                .entries
                .iter()
                .filter(|(k, _)| **k != key)
                .min_by_key(|(_, e)| e.used)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            if let Some(e) = self.entries.remove(&oldest) {
                self.total -= e.bytes;
            }
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Bytes held.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn bytes(&self) -> usize {
        self.total
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn clear(&mut self) {
        self.entries.clear();
        self.total = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_least_recently_used_beyond_budget() {
        let mut c: Lru<&str, u32> = Lru::new(100);
        c.put("a", 1, 40);
        c.put("b", 2, 40);
        assert_eq!(c.get(&"a"), Some(1)); // a is now newer than b
        c.put("c", 3, 40);
        assert_eq!(c.get(&"b"), None, "b was the least recently used");
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.get(&"c"), Some(3));
        assert_eq!(c.bytes(), 80);
        assert_eq!(c.len(), 2);
    }

    #[test]
    fn replacing_counts_bytes_once_and_keeps_an_oversized_entry() {
        let mut c: Lru<u8, u8> = Lru::new(10);
        c.put(1, 1, 4);
        c.put(1, 2, 6);
        assert_eq!(c.bytes(), 6);
        c.put(2, 3, 50);
        assert_eq!(c.len(), 1, "everything else made room");
        assert_eq!(c.get(&2), Some(3));
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.bytes(), 0);
    }
}
