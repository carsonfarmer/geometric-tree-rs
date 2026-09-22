mod common;

use std::collections::{BTreeMap, BTreeSet};

use common::*;
use geometric_tree::rank::hashed;
use geometric_tree::{GList, GMap, GSet};

#[test]
fn map_behaves_like_btreemap_in_key_order() {
    let mut rand = Prng::new(3);
    let mut map = GMap::<u64, String>::new();
    let mut reference = BTreeMap::new();
    for i in 0..5000 {
        let key = rand.below(1000) as u64;
        if rand.next_f64() < 0.3 {
            map = map.remove(&key);
            reference.remove(&key);
        } else {
            map = map.insert(key, format!("v{i}"));
            reference.insert(key, format!("v{i}"));
        }
        assert_eq!(map.len(), reference.len());
        assert_eq!(map.get(&key), reference.get(&key));
        assert_eq!(map.contains_key(&key), reference.contains_key(&key));
    }
    let got: Vec<(u64, String)> = map.iter().map(|(k, v)| (*k, v.clone())).collect();
    let want: Vec<(u64, String)> = reference.iter().map(|(k, v)| (*k, v.clone())).collect();
    assert_eq!(got, want);
    let rank = hashed::<u64>(8);
    check(&map.root, &|k| rank(k, 0));
}

#[test]
fn map_is_persistent_and_history_independent() {
    let a: GMap<u64, u64> = (0..500u64).map(|i| (i, i * i)).collect();
    let b = a.insert(1000, 0).remove(&1);
    assert_eq!(a.len(), 500);
    assert_eq!(b.len(), 500);
    assert!(a.contains_key(&1) && !b.contains_key(&1));
    let mut rand = Prng::new(4);
    let pairs: Vec<(u64, u64)> = (0..500u64).map(|i| (i, i * i)).collect();
    for _ in 0..10 {
        let mut m: GMap<u64, u64> = shuffle(&pairs, &mut rand).into_iter().collect();
        for (k, _) in shuffle(&pairs[..100], &mut rand) {
            m = m.remove(&k);
        }
        for (k, v) in shuffle(&pairs[..100], &mut rand) {
            m = m.insert(k, v);
        }
        assert_eq!(fingerprint(&m.root), fingerprint(&a.root));
    }
}

#[test]
fn map_insert_replaces_and_remove_of_absent_is_a_clone() {
    let m: GMap<u64, &str> = [(1, "a"), (2, "b")].into_iter().collect();
    let n = m.insert(1, "c");
    assert_eq!(n.len(), 2);
    assert_eq!(n.get(&1), Some(&"c"));
    assert_eq!(m.get(&1), Some(&"a"));
    let r = m.remove(&9);
    assert_eq!(fingerprint(&r.root), fingerprint(&m.root));
    let dup: GMap<u64, &str> = [(1, "a"), (1, "b")].into_iter().collect();
    assert_eq!(dup.iter().collect::<Vec<_>>(), [(&1, &"b")]);
}

#[test]
fn set_behaves_like_btreeset() {
    let mut rand = Prng::new(5);
    let mut set = GSet::<u64>::new();
    let mut reference = BTreeSet::new();
    for _ in 0..5000 {
        let key = rand.below(1000) as u64;
        if rand.next_f64() < 0.3 {
            set = set.remove(&key);
            reference.remove(&key);
        } else {
            set = set.insert(key);
            reference.insert(key);
        }
        assert_eq!(set.len(), reference.len());
        assert_eq!(set.contains(&key), reference.contains(&key));
    }
    assert_eq!(
        set.iter().copied().collect::<Vec<_>>(),
        reference.iter().copied().collect::<Vec<_>>()
    );
    let again: GSet<u64> = reference.iter().rev().copied().collect();
    assert_eq!(fingerprint(set.root()), fingerprint(again.root()));
    assert_eq!(GSet::<u64>::from_iter([1, 1, 1]).len(), 1);
}

#[test]
fn list_behaves_like_vec_under_random_edits() {
    let mut rand = Prng::new(7);
    let mut list = GList::<u32>::with_k(4);
    let mut reference: Vec<u32> = Vec::new();
    for i in 0..3000u32 {
        let r = rand.next_f64();
        let pos = rand.below(reference.len() + 1);
        if r < 0.2 {
            list = list.push_back(i);
            reference.push(i);
        } else if r < 0.4 {
            list = list.push_front(i);
            reference.insert(0, i);
        } else if r < 0.7 {
            list = list.insert(pos, i);
            reference.insert(pos, i);
        } else if !reference.is_empty() {
            let j = pos.min(reference.len() - 1);
            list = list.remove(j);
            reference.remove(j);
        }
        assert_eq!(list.len(), reference.len());
        assert_eq!(list.get(pos), reference.get(pos));
    }
    assert_eq!(list.iter().copied().collect::<Vec<_>>(), reference);
}

#[test]
#[allow(clippy::reversed_empty_ranges)]
fn list_slice_concat_and_split_agree_with_vec() {
    let xs: Vec<u32> = (0..100).collect();
    let list: GList<u32> = xs.clone().into();
    let v = |l: &GList<u32>| l.iter().copied().collect::<Vec<_>>();
    assert_eq!(v(&list.slice(..)), xs);
    assert_eq!(v(&list.slice(10..20)), xs[10..20]);
    assert_eq!(v(&list.slice(95..)), xs[95..]);
    assert_eq!(v(&list.slice(..3)), xs[..3]);
    assert_eq!(v(&list.slice(50..10)), Vec::<u32>::new());
    assert_eq!(v(&list.slice(7..=7)), xs[7..=7]);
    assert_eq!(v(&list.slice(98..500)), xs[98..]);
    let rotated = list.slice(20..).concat(&list.slice(..20));
    assert_eq!(v(&rotated), [&xs[20..], &xs[..20]].concat());
    for i in [0, 1, 42, 100, 200] {
        let (l, r) = list.split_at(i);
        assert_eq!(v(&l), xs[..i.min(100)]);
        assert_eq!(v(&r), xs[i.min(100)..]);
    }
    assert_eq!(list.get(100), None);
    assert_eq!(list.remove(500).len(), 100);
    assert!(GList::<u32>::new().is_empty());
}
