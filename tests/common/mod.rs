#![allow(dead_code)]
//! Shared test helpers: a seeded PRNG, the paper's fixture, invariant checks.

use std::fmt::Display;

use geometric_tree::rank::{Rank, hash, hashed_by};
use geometric_tree::tree::{Items, Node, Tree};

/// A small seeded PRNG (mulberry32) so failures reproduce.
pub struct Prng(u32);

impl Prng {
    pub fn new(seed: u32) -> Self {
        Prng(seed)
    }
    pub fn next_f64(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6d2b79f5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(1 | t);
        t = (t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t))) ^ t;
        (t ^ (t >> 14)) as f64 / 4294967296.0
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_f64() * n as f64) as usize
    }
}

pub fn shuffle<T: Clone>(xs: &[T], rand: &mut Prng) -> Vec<T> {
    let mut a = xs.to_vec();
    for i in (1..a.len()).rev() {
        let j = rand.below(i + 1);
        a.swap(i, j);
    }
    a
}

/// The fixture from the paper's figures: primes with hand-picked ranks.
pub const FIXTURE: [(u64, u32); 20] = [
    (2, 1),
    (3, 2),
    (5, 1),
    (7, 3),
    (11, 1),
    (13, 2),
    (17, 1),
    (19, 1),
    (23, 2),
    (29, 2),
    (31, 3),
    (37, 1),
    (41, 2),
    (43, 2),
    (47, 1),
    (53, 3),
    (59, 1),
    (61, 2),
    (67, 3),
    (71, 2),
];

pub fn primes() -> Vec<u64> {
    FIXTURE.iter().map(|&(k, _)| k).collect()
}

pub fn fixed(key: &u64) -> u32 {
    FIXTURE
        .iter()
        .find(|&&(k, _)| k == *key)
        .expect("fixture key")
        .1
}

pub fn fixed_rank() -> Rank<u64> {
    std::sync::Arc::new(|key: &u64, _| fixed(key))
}

/// A rank function independent of `hashed(k)` for repeated experiments.
pub fn trial(k: u32, i: u32) -> Rank<u64> {
    hashed_by(k, move |key: &u64, seed| hash(&format!("{i}/{key}"), seed))
}

/// Every node of a tree, in preorder.
pub fn nodes<K, V, S: Items<K, V>>(t: &Tree<K, V, S>) -> Vec<&Node<K, V, S>> {
    let mut out = Vec::new();
    fn walk<'a, K, V, S: Items<K, V>>(t: &'a Tree<K, V, S>, out: &mut Vec<&'a Node<K, V, S>>) {
        if let Some(n) = t {
            out.push(n);
            for e in n.items.iter() {
                walk(&e.left, out);
            }
            walk(&n.right, out);
        }
    }
    walk(t, &mut out);
    out
}

/// Height in nodes.
pub fn height<K, V, S: Items<K, V>>(t: &Tree<K, V, S>) -> usize {
    let Some(n) = t else { return 0 };
    let mut h = height(&n.right);
    for e in n.items.iter() {
        h = h.max(height(&e.left));
    }
    h + 1
}

/// A string that determines the tree's shape, keys and ranks.
pub fn fingerprint<K: Display, V, S: Items<K, V>>(t: &Tree<K, V, S>) -> String {
    let Some(n) = t else { return ".".into() };
    let items: Vec<String> = n
        .items
        .iter()
        .map(|e| format!("{}{}", fingerprint(&e.left), e.key))
        .collect();
    format!("{}({}){}", n.rank, items.join(","), fingerprint(&n.right))
}

/// Assert the G-tree invariants and return the keys in order: ranks strictly
/// decrease down every path, every key sits in a node of its own rank, keys
/// are sorted, nodes are non-empty and sizes are right.
pub fn check<K: Ord + Clone, V, S: Items<K, V>>(
    t: &Tree<K, V, S>,
    rank: &dyn Fn(&K) -> u32,
) -> Vec<K> {
    fn go<K: Ord + Clone, V, S: Items<K, V>>(
        t: &Tree<K, V, S>,
        rank: &dyn Fn(&K) -> u32,
        above: u32,
    ) -> Vec<K> {
        let Some(n) = t else { return vec![] };
        assert!(n.rank < above, "rank does not decrease");
        assert!(n.items.weight() > 0, "empty node");
        let mut keys = Vec::new();
        for e in n.items.iter() {
            keys.extend(go(&e.left, rank, n.rank));
            assert_eq!(rank(&e.key), n.rank, "key in a node of another rank");
            keys.push(e.key.clone());
        }
        keys.extend(go(&n.right, rank, n.rank));
        assert!(keys.windows(2).all(|w| w[0] < w[1]), "keys out of order");
        assert_eq!(n.size, keys.len(), "wrong size");
        keys
    }
    go(t, rank, u32::MAX)
}
