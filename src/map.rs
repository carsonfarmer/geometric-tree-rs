use std::cmp::Ordering;
use std::hash::Hash;
use std::sync::Arc;

use crate::rank::{Rank, hashed};
use crate::tree::{ArrayItems, Entry, Items, Tree, entries, find, insert, remove, single, size};

/// How a map or set assigns ranks and stores the entries of a node.
pub struct Options<K, S> {
    pub rank: Rank<K>,
    pub empty: S,
}

impl<K, S> Options<K, S> {
    /// Hashed ranks with about `k` keys per node, over the given empty inner set.
    pub fn new(k: u32, empty: S) -> Self
    where
        K: Hash + 'static,
    {
        Options {
            rank: hashed(k),
            empty,
        }
    }
}

impl<K: Hash + 'static, V: Clone> Default for Options<K, ArrayItems<K, V>>
where
    K: Clone,
{
    fn default() -> Self {
        Options::new(8, ArrayItems::default())
    }
}

/// An immutable, ordered map. Every update returns a new map that shares
/// structure with the old one.
pub struct GMap<K, V, S = ArrayItems<K, V>> {
    pub root: Tree<K, V, S>,
    options: Arc<Options<K, S>>,
}

impl<K, V, S> Clone for GMap<K, V, S> {
    fn clone(&self) -> Self {
        GMap {
            root: self.root.clone(),
            options: self.options.clone(),
        }
    }
}

impl<K: Ord + Clone + Hash + 'static, V: Clone> Default for GMap<K, V> {
    fn default() -> Self {
        Self::with(Options::default())
    }
}

impl<K: Ord + Clone + Hash + 'static, V: Clone> GMap<K, V> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<K: Ord + Clone, V: Clone, S: Items<K, V>> GMap<K, V, S> {
    pub fn with(options: Options<K, S>) -> Self {
        GMap {
            root: None,
            options: Arc::new(options),
        }
    }

    pub fn len(&self) -> usize {
        size(&self.root)
    }

    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    fn at<'k>(key: &'k K) -> impl Fn(&Entry<K, V, S>) -> Ordering + 'k {
        move |e| e.key.cmp(key)
    }

    fn entry(&self, key: &K) -> Option<&Entry<K, V, S>> {
        find(&self.root, &Self::at(key)).filter(|e| e.key == *key)
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.entry(key).is_some()
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.entry(key).map(Entry::value)
    }

    /// A map with `value` at `key`, replacing any previous value.
    pub fn insert(&self, key: K, value: V) -> Self {
        let rank = (self.options.rank)(&key, 0);
        let one = single(key.clone(), value, rank, &self.options.empty);
        GMap {
            root: insert(&self.root, &Self::at(&key), &one),
            options: self.options.clone(),
        }
    }

    /// A map without `key`. Returns a clone of this map if the key is absent.
    pub fn remove(&self, key: &K) -> Self {
        if !self.contains_key(key) {
            return self.clone();
        }
        GMap {
            root: remove(&self.root, &Self::at(key)),
            options: self.options.clone(),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        entries(&self.root).map(|e| (&e.key, e.value()))
    }

    pub fn keys(&self) -> impl Iterator<Item = &K> {
        entries(&self.root).map(|e| &e.key)
    }

    pub fn values(&self) -> impl Iterator<Item = &V> {
        entries(&self.root).map(Entry::value)
    }
}

impl<K: Ord + Clone, V: Clone, S: Items<K, V>> Extend<(K, V)> for GMap<K, V, S> {
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        for (k, v) in iter {
            *self = self.insert(k, v);
        }
    }
}

impl<K: Ord + Clone + Hash + 'static, V: Clone> FromIterator<(K, V)> for GMap<K, V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut map = GMap::new();
        map.extend(iter);
        map
    }
}
