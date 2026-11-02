use std::collections::HashMap;
use std::hash::Hash;

pub(crate) struct Lru<K, V> {
    cap: usize,
    tick: u64,
    map: HashMap<K, (u64, V)>,
}

impl<K: Hash + Eq + Clone, V: Clone> Lru<K, V> {
    pub(crate) fn new(cap: usize) -> Self {
        Self {
            cap,
            tick: 0,
            map: HashMap::new(),
        }
    }

    pub(crate) fn get(&mut self, key: &K) -> Option<V> {
        self.tick += 1;
        let tick = self.tick;
        self.map.get_mut(key).map(|(t, v)| {
            *t = tick;
            v.clone()
        })
    }

    pub(crate) fn put(&mut self, key: K, value: V) {
        self.tick += 1;
        if self.map.len() >= self.cap && !self.map.contains_key(&key) {
            let oldest = self
                .map
                .iter()
                .min_by_key(|(_, (t, _))| *t)
                .map(|(k, _)| k.clone());
            if let Some(k) = oldest {
                self.map.remove(&k);
            }
        }
        self.map.insert(key, (self.tick, value));
    }

    pub(crate) fn len(&self) -> usize {
        self.map.len()
    }
}
