use std::hash::Hash;

use crate::map::{GMap, Options};
use crate::tree::{ArrayItems, Items, Tree};

/// An immutable, ordered set. Every update returns a new set that shares
/// structure with the old one.
pub struct GSet<K, S = ArrayItems<K, ()>>(GMap<K, (), S>);

impl<K, S> Clone for GSet<K, S> {
    fn clone(&self) -> Self {
        GSet(self.0.clone())
    }
}

impl<K: Ord + Clone + Hash + 'static> Default for GSet<K> {
    fn default() -> Self {
        GSet(GMap::default())
    }
}

impl<K: Ord + Clone + Hash + 'static> GSet<K> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<K: Ord + Clone, S: Items<K, ()>> GSet<K, S> {
    pub fn with(options: Options<K, S>) -> Self {
        GSet(GMap::with(options))
    }

    pub fn root(&self) -> &Tree<K, (), S> {
        &self.0.root
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn contains(&self, key: &K) -> bool {
        self.0.contains_key(key)
    }

    /// A set with `key`. Returns a clone of this set if the key is present.
    pub fn insert(&self, key: K) -> Self {
        if self.contains(&key) {
            return self.clone();
        }
        GSet(self.0.insert(key, ()))
    }

    /// A set without `key`. Returns a clone of this set if the key is absent.
    pub fn remove(&self, key: &K) -> Self {
        GSet(self.0.remove(key))
    }

    pub fn iter(&self) -> impl Iterator<Item = &K> {
        self.0.keys()
    }
}

impl<K: Ord + Clone, S: Items<K, ()>> Extend<K> for GSet<K, S> {
    fn extend<I: IntoIterator<Item = K>>(&mut self, iter: I) {
        for k in iter {
            *self = self.insert(k);
        }
    }
}

impl<K: Ord + Clone + Hash + 'static> FromIterator<K> for GSet<K> {
    fn from_iter<I: IntoIterator<Item = K>>(iter: I) -> Self {
        let mut set = GSet::new();
        set.extend(iter);
        set
    }
}
