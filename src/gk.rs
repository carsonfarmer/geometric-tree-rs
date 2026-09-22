//! G-trees whose inner sets are G-trees.
//!
//! [`GkItems`] keeps a node's entries in a sorted array until the node outgrows
//! `threshold`, then stores them as a G-tree of their own, ranked by the next
//! seed, whose nodes are again `GkItems` one dimension further down. With a
//! threshold of 1 this is the zip-zip tree of the paper's section 4.2; with a
//! larger one it is the Gk-tree of Hehemann (2025, section 4.1). An adversary
//! who crafts keys of equal rank (which costs O(n) work for one dimension)
//! must repeat the feat once per dimension, so node sizes stay bounded and
//! operations stay O(log^2 n) even then. The conversion happens in both
//! directions at the same threshold, so the representation remains a function
//! of the stored set alone.
//!
//! Inner trees store the outer entries they stand for as [`Value::Outer`], so
//! every dimension uses the same entry type and the core algorithms apply
//! unchanged.

use std::cmp::Ordering;
use std::hash::Hash;
use std::sync::Arc;

use crate::map::Options;
use crate::rank::{Rank, hashed};
use crate::tree::{
    Entry, Items, Slice, Tree, Value, entries, find, node, remove, size, unzip, zip,
};

/// One dimension of a Gk-tree: the seed that ranks its inner trees, and the
/// node size at which entries move between an array and a tree.
pub struct Dimension<K> {
    pub rank: Rank<K>,
    pub seed: u32,
    pub threshold: usize,
}

impl<K> Clone for Dimension<K> {
    fn clone(&self) -> Self {
        Dimension {
            rank: self.rank.clone(),
            seed: self.seed,
            threshold: self.threshold,
        }
    }
}

impl<K> Dimension<K> {
    fn next(&self) -> Self {
        Dimension {
            seed: self.seed + 1,
            ..self.clone()
        }
    }
}

/// [`Items`] that are an array up to the threshold and a G-tree beyond it.
pub enum GkItems<K, V> {
    Array(Slice<K, V, GkItems<K, V>>, Dimension<K>),
    Tree(Tree<K, V, GkItems<K, V>>, Dimension<K>),
}

impl<K, V> Clone for GkItems<K, V> {
    fn clone(&self) -> Self {
        match self {
            GkItems::Array(s, d) => GkItems::Array(s.clone(), d.clone()),
            GkItems::Tree(t, d) => GkItems::Tree(t.clone(), d.clone()),
        }
    }
}

/// The outer entry an inner entry stands for.
fn outer<K, V>(inner: &Entry<K, V, GkItems<K, V>>) -> &Entry<K, V, GkItems<K, V>> {
    match &inner.value {
        Value::Outer(e) => e,
        Value::Own(_) => unreachable!("inner trees hold outer entries"),
    }
}

impl<K: Ord + Clone, V: Clone> GkItems<K, V> {
    pub fn empty(dim: Dimension<K>) -> Self {
        GkItems::Array(Slice::new(vec![]), dim)
    }

    fn dim(&self) -> &Dimension<K> {
        match self {
            GkItems::Array(_, d) | GkItems::Tree(_, d) => d,
        }
    }

    /// A single-entry inner tree, ranked by this dimension's seed.
    fn one(e: Entry<K, V, Self>, dim: &Dimension<K>) -> Tree<K, V, Self> {
        let key = e.key.clone();
        let rank = (dim.rank)(&key, dim.seed);
        let entry = Entry {
            key,
            value: Value::Outer(Arc::new(e)),
            left: None,
        };
        node(rank, Self::empty(dim.next()).unshift(entry), None)
    }

    fn as_tree(&self) -> Tree<K, V, Self> {
        match self {
            GkItems::Tree(t, _) => t.clone(),
            GkItems::Array(s, dim) => s
                .iter()
                .fold(None, |t, e| zip(&t, &Self::one(e.clone(), dim))),
        }
    }

    /// Canonical form of an array: a tree once it exceeds the threshold.
    fn from_slice(s: Slice<K, V, Self>, dim: &Dimension<K>) -> Self {
        if s.len() > dim.threshold {
            GkItems::Tree(GkItems::Array(s, dim.clone()).as_tree(), dim.clone())
        } else {
            GkItems::Array(s, dim.clone())
        }
    }

    /// Canonical form of a tree: an array once it is within the threshold.
    fn from_tree(t: Tree<K, V, Self>, dim: &Dimension<K>) -> Self {
        if entries(&t).nth(dim.threshold).is_some() {
            GkItems::Tree(t, dim.clone())
        } else {
            GkItems::Array(
                Slice::new(entries(&t).map(|ie| outer(ie).clone()).collect()),
                dim.clone(),
            )
        }
    }
}

impl<K: Ord + Clone, V: Clone> Items<K, V> for GkItems<K, V> {
    fn weight(&self) -> usize {
        match self {
            GkItems::Array(s, _) => s.weight,
            GkItems::Tree(t, _) => size(t),
        }
    }

    fn split(
        &self,
        at: &impl Fn(&Entry<K, V, Self>) -> Ordering,
    ) -> (Self, Option<Entry<K, V, Self>>, Self) {
        match self {
            GkItems::Array(s, dim) => {
                let (lo, hit, hi) = s.split(at);
                (Self::from_slice(lo, dim), hit, Self::from_slice(hi, dim))
            }
            GkItems::Tree(t, dim) => {
                // Erase the closure's type so deeper dimensions reuse this instantiation.
                let inner: &dyn Fn(&Entry<K, V, Self>) -> Ordering = &|ie| at(outer(ie));
                let (l, hit, r) = unzip(t, &inner);
                (
                    Self::from_tree(l, dim),
                    hit.map(|ie| outer(&ie).clone()),
                    Self::from_tree(r, dim),
                )
            }
        }
    }

    fn find(&self, at: &impl Fn(&Entry<K, V, Self>) -> Ordering) -> Option<&Entry<K, V, Self>> {
        match self {
            GkItems::Array(s, _) => s.find(at),
            GkItems::Tree(t, _) => {
                let inner: &dyn Fn(&Entry<K, V, Self>) -> Ordering = &|ie| at(outer(ie));
                find(t, &inner).map(outer)
            }
        }
    }

    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (GkItems::Array(a, dim), GkItems::Array(b, _)) => Self::from_slice(a.join(b), dim),
            _ => Self::from_tree(zip(&self.as_tree(), &other.as_tree()), self.dim()),
        }
    }

    fn shift(&self) -> (Entry<K, V, Self>, Self) {
        match self {
            GkItems::Array(s, dim) => {
                let (e, rest) = s.shift();
                (e, Self::from_slice(rest, dim))
            }
            GkItems::Tree(t, dim) => {
                let min = find(t, &|_| Ordering::Greater).expect("shift of empty items");
                let key = min.key.clone();
                let rest = remove(t, &|ie| ie.key.cmp(&key));
                (outer(min).clone(), Self::from_tree(rest, dim))
            }
        }
    }

    fn unshift(&self, e: Entry<K, V, Self>) -> Self {
        match self {
            GkItems::Array(s, dim) => Self::from_slice(s.unshift(e), dim),
            GkItems::Tree(t, dim) => Self::from_tree(zip(&Self::one(e, dim), t), dim),
        }
    }

    fn iter(&self) -> Box<dyn Iterator<Item = &Entry<K, V, Self>> + '_> {
        match self {
            GkItems::Array(s, _) => Box::new(s.iter()),
            GkItems::Tree(t, _) => Box::new(entries(t).map(outer)),
        }
    }
}

/// Options for a Gk-tree with hashed ranks: about `k` keys per node and a
/// threshold of `12k`, past which an honest node lands with probability
/// around e^-12.
pub fn gk<K: Hash + Clone + 'static, V: Clone>(k: u32) -> Options<K, GkItems<K, V>> {
    gk_by(hashed(k), 12 * k as usize)
}

/// Options for a Gk-tree with the given rank function, reseeded once per
/// dimension, and threshold. A threshold of 1 gives zip-zip trees.
pub fn gk_by<K: Clone, V: Clone>(rank: Rank<K>, threshold: usize) -> Options<K, GkItems<K, V>> {
    let dim = Dimension {
        rank: rank.clone(),
        seed: 1,
        threshold: threshold.max(1),
    };
    Options {
        rank,
        empty: GkItems::Array(Slice::new(vec![]), dim),
    }
}
