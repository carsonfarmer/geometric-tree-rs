mod common;

use std::cmp::Ordering;

use common::*;
use geometric_tree::binary;
use geometric_tree::rank::hashed;
use geometric_tree::tree::{self, ArrayItems, Items};

type B = binary::Tree<u64, ()>;

fn at(key: u64) -> impl Fn(&binary::Node<u64, ()>) -> Ordering {
    move |n| n.key.cmp(&key)
}

fn build(keys: &[u64], rank: &dyn Fn(&u64) -> u32) -> B {
    let mut t = None;
    for &key in keys {
        t = binary::insert(&t, &at(key), &binary::single(key, (), rank(&key)));
    }
    t
}

/// Keys as nested `key(left,right)` for readable structure checks.
fn shape(t: &B) -> String {
    match t {
        None => ".".into(),
        Some(n) => format!("{}({},{})", n.key, shape(&n.left), shape(&n.right)),
    }
}

/// Check zip-tree invariants and return the keys in order.
fn check_binary(t: &B, rank: &dyn Fn(&u64) -> u32, max_left: u32, max_right: u32) -> Vec<u64> {
    let Some(n) = t else { return vec![] };
    assert_eq!(n.rank, rank(&n.key));
    assert!(n.rank < max_left && n.rank <= max_right);
    let mut keys = check_binary(&n.left, rank, n.rank, n.rank);
    keys.push(n.key);
    keys.extend(check_binary(&n.right, rank, u32::MAX, n.rank));
    assert!(keys.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(n.size, keys.len());
    keys
}

/// Unfold a k = 2 G-tree into the binary zip tree it represents.
fn unfold(t: &tree::Tree<u64, (), ArrayItems<u64, ()>>) -> B {
    let Some(n) = t else { return None };
    let mut right = unfold(&n.right);
    let items: Vec<_> = n.items.iter().collect();
    for e in items.into_iter().rev() {
        right = binary::node(e.key, (), n.rank, unfold(&e.left), right);
    }
    right
}

#[test]
fn matches_the_figure_from_the_paper() {
    let t = build(&primes(), &fixed);
    assert_eq!(check_binary(&t, &fixed, u32::MAX, u32::MAX), primes());
    assert_eq!(
        shape(&t),
        "7(3(2(.,.),5(.,.)),31(13(11(.,.),23(17(.,19(.,.)),29(.,.))),53(41(37(.,.),43(.,47(.,.))),67(61(59(.,.),.),71(.,.)))))"
    );
}

#[test]
fn insertion_order_does_not_matter() {
    let mut rand = Prng::new(10);
    let expected = shape(&build(&primes(), &fixed));
    for _ in 0..50 {
        assert_eq!(
            shape(&build(&shuffle(&primes(), &mut rand), &fixed)),
            expected
        );
    }
}

#[test]
fn unzip_zip_remove_and_find() {
    let t = build(&primes(), &fixed);
    let (l, hit, r) = binary::unzip(&t, &at(31));
    assert_eq!(hit.map(|n| n.key), Some(31));
    let before: Vec<u64> = binary::nodes(&l).map(|n| n.key).collect();
    assert_eq!(
        before,
        primes().into_iter().filter(|&p| p < 31).collect::<Vec<_>>()
    );
    assert_eq!(
        shape(&binary::zip(
            &binary::zip(&l, &binary::single(31, (), 3)),
            &r
        )),
        shape(&t)
    );
    let without: Vec<u64> = primes().into_iter().filter(|&p| p != 31).collect();
    assert_eq!(
        shape(&binary::remove(&t, &at(31))),
        shape(&build(&without, &fixed))
    );
    assert_eq!(binary::find(&t, &at(30)).map(|n| n.key), Some(31));
    assert!(binary::find(&t, &at(72)).is_none());
}

#[test]
fn is_the_g_tree_with_k_2_runs_of_equal_rank_folded_into_one_node() {
    let hashed2 = hashed::<u64>(2);
    let many: Vec<u64> = shuffle(&(0..2000).collect::<Vec<_>>(), &mut Prng::new(11));
    for (keys, rank) in [
        (primes(), &fixed as &dyn Fn(&u64) -> u32),
        (many, &|k: &u64| hashed2(k, 0)),
    ] {
        let mut g = None;
        for &key in &keys {
            let one = tree::single(key, (), rank(&key), &ArrayItems::default());
            g = tree::insert(
                &g,
                &move |e: &tree::Entry<u64, (), _>| e.key.cmp(&key),
                &one,
            );
        }
        assert_eq!(shape(&unfold(&g)), shape(&build(&keys, rank)));
    }
}
