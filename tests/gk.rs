mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use common::*;
use geometric_tree::gk::{GkItems, gk_by};
use geometric_tree::rank::hashed;
use geometric_tree::tree::{Items, Tree};
use geometric_tree::{GMap, GSet};

fn small<V: Clone>() -> geometric_tree::Options<u64, GkItems<u64, V>> {
    gk_by(hashed(4), 4)
}

#[test]
fn behaves_like_a_map_and_keeps_the_invariants() {
    let mut rand = Prng::new(8);
    let mut map = GMap::with(small::<u32>());
    let mut reference = BTreeMap::new();
    for i in 0..5000u32 {
        let key = rand.below(500) as u64;
        if rand.next_f64() < 0.3 {
            map = map.remove(&key);
            reference.remove(&key);
        } else {
            map = map.insert(key, i);
            reference.insert(key, i);
        }
        assert_eq!(map.get(&key), reference.get(&key));
    }
    let got: Vec<(u64, u32)> = map.iter().map(|(k, v)| (*k, *v)).collect();
    assert_eq!(got, reference.into_iter().collect::<Vec<_>>());
    let rank = hashed::<u64>(4);
    check(&map.root, &|k| rank(k, 0));
}

#[test]
fn is_history_independent_conversions_included() {
    let keys: Vec<u64> = (0..500).collect();
    let mut expected = GSet::with(small::<()>());
    expected.extend(keys.iter().copied());
    let mut rand = Prng::new(9);
    for _ in 0..10 {
        let mut set = GSet::with(small::<()>());
        set.extend(shuffle(&keys, &mut rand));
        for key in shuffle(&keys[..200], &mut rand) {
            set = set.remove(&key);
        }
        for key in shuffle(&keys[..200], &mut rand) {
            set = set.insert(key);
        }
        assert_eq!(fingerprint(set.root()), fingerprint(expected.root()));
        assert!(same_modes(set.root(), expected.root()));
    }
}

/// Whether two Gk trees agree on array versus tree mode at every node.
fn same_modes<V: Clone>(
    a: &Tree<u64, V, GkItems<u64, V>>,
    b: &Tree<u64, V, GkItems<u64, V>>,
) -> bool {
    let mode = |t: &Tree<u64, V, GkItems<u64, V>>| {
        nodes(t)
            .iter()
            .map(|n| matches!(n.items, GkItems::Tree(..)))
            .collect::<Vec<_>>()
    };
    mode(a) == mode(b)
}

#[test]
fn keys_crafted_to_share_a_rank_still_form_a_shallow_tree() {
    // Every key gets rank 1 in the first dimension: a plain G-tree would be
    // one node holding all n keys. The next dimensions use honest ranks.
    let n = 20_000u64;
    let honest = hashed::<u64>(8);
    let rank = Arc::new(move |key: &u64, seed: u32| if seed == 0 { 1 } else { honest(key, seed) });
    let mut map = GMap::with(gk_by::<u64, u64>(rank, 96));
    map.extend((0..n).map(|i| (i, i)));
    let root = map.root.as_ref().unwrap();
    assert_eq!(root.size, n as usize);
    let GkItems::Tree(inner, _) = &root.items else {
        panic!("root should be in tree mode")
    };
    assert!((height(inner) as f64) < 4.0 * (n as f64).log2());
    assert_eq!(map.get(&(n - 1)), Some(&(n - 1)));
    assert_eq!(map.remove(&0).len(), n as usize - 1);
}

#[test]
fn threshold_1_gives_zip_zip_trees() {
    let mut set = GSet::with(gk_by::<u64, ()>(hashed(2), 1));
    set.extend(0..300u64);
    let mut trees = 0;
    for node in nodes(set.root()) {
        match &node.items {
            GkItems::Array(s, _) => assert_eq!(s.len(), 1),
            GkItems::Tree(..) => trees += 1,
        }
    }
    assert!(trees > 0);
    assert_eq!(set.iter().count(), 300);
    assert_eq!(
        set.root().as_ref().unwrap().items.weight(),
        300 - geometric_tree::tree::size(&set.root().as_ref().unwrap().right)
    );
}
