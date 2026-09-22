mod common;

use std::cmp::Ordering;

use common::*;
use geometric_tree::rank::hashed;
use geometric_tree::tree::*;

type T = Tree<u64, (), ArrayItems<u64, ()>>;

fn at(key: u64) -> impl Fn(&Entry<u64, (), ArrayItems<u64, ()>>) -> Ordering {
    move |e| e.key.cmp(&key)
}

fn one(key: u64, rank: u32) -> T {
    single(key, (), rank, &ArrayItems::default())
}

fn build(keys: &[u64], rank: &dyn Fn(&u64) -> u32) -> T {
    let mut t = None;
    for &key in keys {
        t = insert(&t, &at(key), &one(key, rank(&key)));
    }
    t
}

#[test]
fn insertion_order_does_not_matter() {
    let mut rand = Prng::new(1);
    let expected = build(&primes(), &fixed);
    assert_eq!(check(&expected, &fixed), primes());
    for _ in 0..50 {
        assert_eq!(
            fingerprint(&build(&shuffle(&primes(), &mut rand), &fixed)),
            fingerprint(&expected)
        );
    }
}

#[test]
fn root_holds_the_keys_of_maximal_rank() {
    let root = build(&primes(), &fixed).unwrap();
    assert_eq!(root.rank, 3);
    let keys: Vec<u64> = root.items.iter().map(|e| e.key).collect();
    assert_eq!(keys, [7, 31, 53, 67]);
    assert_eq!(size(&root.right), 1);
}

#[test]
fn unzip_then_zip_is_the_identity() {
    let t = build(&primes(), &fixed);
    for key in [0, 1, 2, 30, 31, 53, 71, 100] {
        let (l, hit, r) = unzip(&t, &at(key));
        let before: Vec<u64> = primes().into_iter().filter(|&p| p < key).collect();
        let after: Vec<u64> = primes().into_iter().filter(|&p| p > key).collect();
        assert_eq!(check(&l, &fixed), before);
        assert_eq!(check(&r, &fixed), after);
        assert_eq!(
            hit.as_ref().map(|e| e.key),
            primes().contains(&key).then_some(key)
        );
        let back = match hit {
            Some(_) => zip(&zip(&l, &one(key, fixed(&key))), &r),
            None => zip(&l, &r),
        };
        assert_eq!(fingerprint(&back), fingerprint(&t));
    }
}

#[test]
fn zip_of_two_ordered_trees_is_the_tree_of_their_union() {
    let expected = fingerprint(&build(&primes(), &fixed));
    for i in 0..=primes().len() {
        let l = build(&primes()[..i], &fixed);
        let r = build(&primes()[i..], &fixed);
        assert_eq!(fingerprint(&zip(&l, &r)), expected);
    }
}

#[test]
fn remove_keys() {
    let mut t = build(&primes(), &fixed);
    let mut rest = primes();
    for key in shuffle(&primes(), &mut Prng::new(2)) {
        t = remove(&t, &at(key));
        rest.retain(|&p| p != key);
        assert_eq!(fingerprint(&t), fingerprint(&build(&rest, &fixed)));
    }
    assert!(t.is_none());
}

#[test]
fn find_returns_the_entry_at_or_after_a_position() {
    let t = build(&primes(), &fixed);
    assert_eq!(find(&t, &at(31)).map(|e| e.key), Some(31));
    assert_eq!(find(&t, &at(30)).map(|e| e.key), Some(31));
    assert_eq!(find(&t, &at(0)).map(|e| e.key), Some(2));
    assert!(find(&t, &at(72)).is_none());
}

#[test]
fn insert_replaces_the_entry_at_the_same_position() {
    type S = Tree<u64, &'static str, ArrayItems<u64, &'static str>>;
    let boxed = |v| single(1u64, v, 1, &ArrayItems::default());
    let at = |e: &Entry<u64, &str, _>| e.key.cmp(&1);
    let t: S = insert(&None, &at, &boxed("a"));
    let t = insert(&t, &at, &boxed("b"));
    let got: Vec<(u64, &str)> = entries(&t).map(|e| (e.key, *e.value())).collect();
    assert_eq!(got, [(1, "b")]);
}

#[test]
fn hashed_ranks_keep_the_invariants_for_many_keys_and_any_k() {
    for k in [2, 3, 8, 32] {
        let rank = hashed::<u64>(k);
        let keys: Vec<u64> = (0..2000).collect();
        let t = build(&shuffle(&keys, &mut Prng::new(k)), &|key| rank(key, 0));
        assert_eq!(check(&t, &|key| rank(key, 0)), keys);
    }
}

#[test]
fn entries_iterate_lazily_in_order() {
    let t = build(&primes(), &fixed);
    let first_three: Vec<u64> = entries(&t).take(3).map(|e| e.key).collect();
    assert_eq!(first_three, [2, 3, 5]);
    assert_eq!(entries(&t).count(), 20);
}
