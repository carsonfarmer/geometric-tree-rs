//! Tests of Hehemann (2025), section 4.1: Gk-trees keep operations
//! polylogarithmic against an adversary and stay history independent across
//! conversions. Plus the cross-implementation check: the same keys give the
//! same tree here and in the TypeScript package.
mod common;

use std::cell::Cell;
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::sync::Arc;

use common::*;
use geometric_tree::gk::{GkItems, gk_by};
use geometric_tree::rank::{hash_utf16, hashed, hashed_by};
use geometric_tree::tree::{Node, Tree};
use geometric_tree::{GMap, GSet, Options};

/// Every node at every dimension, with its dimension.
#[allow(clippy::type_complexity)]
fn everywhere<K: Ord + Clone, V: Clone>(
    t: &Tree<K, V, GkItems<K, V>>,
    dim: usize,
) -> Vec<(&Node<K, V, GkItems<K, V>>, usize)> {
    let mut out = Vec::new();
    for n in nodes(t) {
        out.push((n, dim));
        if let GkItems::Tree(inner, _) = &n.items {
            out.extend(everywhere(inner, dim + 1));
        }
    }
    out
}

/// A key type whose comparisons are counted, to measure work per operation.
#[derive(Clone, PartialEq, Eq, Hash)]
struct Counted(u64);
thread_local!(static COMPARES: Cell<usize> = const { Cell::new(0) });
impl PartialOrd for Counted {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Counted {
    fn cmp(&self, other: &Self) -> Ordering {
        COMPARES.with(|c| c.set(c.get() + 1));
        self.0.cmp(&other.0)
    }
}

#[test]
fn operations_stay_polylogarithmic_when_every_key_shares_a_rank() {
    let threshold = 32;
    let honest = hashed::<Counted>(8);
    // Every key has rank 1 in the first dimension: a plain G-tree is one node.
    let adversarial =
        Arc::new(move |key: &Counted, seed: u32| if seed == 0 { 1 } else { honest(key, seed) });
    let cost = |n: u64| {
        let mut map = GMap::with(gk_by::<Counted, u64>(adversarial.clone(), threshold));
        map.extend((0..n).map(|i| (Counted(i), i)));
        COMPARES.with(|c| c.set(0));
        for i in 0..200 {
            map.get(&Counted(i * n / 200));
        }
        let get = COMPARES.with(|c| c.get()) as f64 / 200.0;
        COMPARES.with(|c| c.set(0));
        // Spread positions: the cost of one spine node would dominate at the end.
        for i in 0..100 {
            map.insert(Counted(i * n / 100), i);
        }
        let set = COMPARES.with(|c| c.get()) as f64 / 100.0;
        let mut dims = 0;
        for (node, dim) in everywhere(&map.root, 0) {
            dims = dims.max(dim);
            if let GkItems::Array(s, _) = &node.items {
                assert!(s.len() <= threshold);
            }
        }
        (get, set, dims)
    };
    let (small, large) = (cost(1000), cost(16_000));
    assert!(large.0 / small.0 < 4.0, "get {small:?} {large:?}"); // a plain G-tree would give 16
    assert!(large.1 / small.1 < 4.0, "set {small:?} {large:?}");
    assert!(large.2 <= 3);
}

#[test]
fn representation_stays_a_function_of_the_set_across_many_conversions() {
    let options = || gk_by::<u64, ()>(hashed(4), 4);
    let mut rand = Prng::new(30);
    let mut set = GSet::with(options());
    let mut present = BTreeSet::new();
    for i in 1..=3000 {
        let key = rand.below(60) as u64;
        if present.contains(&key) && rand.next_f64() < 0.5 {
            set = set.remove(&key);
            present.remove(&key);
        } else {
            set = set.insert(key);
            present.insert(key);
        }
        if i % 100 == 0 {
            let mut fresh = GSet::with(options());
            fresh.extend(shuffle(
                &present.iter().copied().collect::<Vec<_>>(),
                &mut rand,
            ));
            assert_eq!(fingerprint(set.root()), fingerprint(fresh.root()));
        }
    }
}

/// Same as `hashString` in the TypeScript tests.
fn hash_string(s: &str) -> u32 {
    let mut h: i32 = 0;
    for c in s.encode_utf16() {
        h = h.wrapping_mul(31).wrapping_add(c as i32);
    }
    h as u32
}

#[test]
fn a_fixed_set_has_the_same_fingerprint_as_in_the_typescript_package() {
    // The TypeScript package hashes `String(key)`; keys stay numbers for ordering.
    let rank = hashed_by::<u64>(4, |key, seed| hash_utf16(&key.to_string(), seed));
    let mut set = GSet::with(Options {
        rank,
        empty: geometric_tree::ArrayItems::default(),
    });
    set.extend((0..1000u64).map(|i| i * 7919));
    assert_eq!(hash_string(&fingerprint(set.root())), 1801069496);
}
