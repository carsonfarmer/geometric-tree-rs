//! Tests of the claims in Farmer & Meyer, "Geometric Search Trees":
//! Definition 3 (structure), section 4.1 (analysis, with the tables of
//! figures 6 to 8), section 4.2 (the family) and section 5 (algorithms).
mod common;

use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

use common::*;
use geometric_tree::GSet;
use geometric_tree::gk::{GkItems, gk_by};
use geometric_tree::rank::{Rank, hashed};
use geometric_tree::tree::*;

type T = Tree<u64, (), ArrayItems<u64, ()>>;
type E = Entry<u64, (), ArrayItems<u64, ()>>;

fn at(key: u64) -> impl Fn(&E) -> Ordering {
    move |e| e.key.cmp(&key)
}

fn one(key: u64, rank: &dyn Fn(&u64) -> u32) -> T {
    single(key, (), rank(&key), &ArrayItems::default())
}

fn build(keys: &[u64], rank: &dyn Fn(&u64) -> u32) -> T {
    let mut t = None;
    for &key in keys {
        t = insert(&t, &at(key), &one(key, rank));
    }
    t
}

fn range(n: u64) -> Vec<u64> {
    (0..n).collect()
}

fn mean(xs: &[f64]) -> f64 {
    xs.iter().sum::<f64>() / xs.len() as f64
}

/// Definition 3, literally: partition sorted keys at those of maximal rank.
fn construct(keys: &[u64], rank: &dyn Fn(&u64) -> u32) -> T {
    if keys.is_empty() {
        return None;
    }
    let r = keys.iter().map(rank).max().unwrap();
    let tops: Vec<usize> = (0..keys.len()).filter(|&i| rank(&keys[i]) == r).collect();
    let mut items = ArrayItems::default();
    for (j, &top) in tops.iter().enumerate().rev() {
        let from = if j == 0 { 0 } else { tops[j - 1] + 1 };
        let left = construct(&keys[from..top], rank);
        items = items.unshift(Entry {
            key: keys[top],
            value: Value::Own(()),
            left,
        });
    }
    node(r, items, construct(&keys[tops[tops.len() - 1] + 1..], rank))
}

fn hashed4() -> Rank<u64> {
    hashed::<u64>(4)
}

#[test]
fn definition_3_no_run_can_be_extended() {
    let rank = hashed4();
    let rank = |k: &u64| rank(k, 0);
    let keys = shuffle(&range(3000), &mut Prng::new(20));
    let t = build(&keys, &rank);
    let sorted = range(3000);
    // The nearest keys of the node's rank or above, on either side, rank above it.
    let nearest = |mut i: i64, step: i64, r: u32| -> Option<u32> {
        loop {
            i += step;
            if i < 0 || i as usize >= sorted.len() {
                return None;
            }
            let rr = rank(&sorted[i as usize]);
            if rr >= r {
                return Some(rr);
            }
        }
    };
    for n in nodes(&t) {
        let items: Vec<&E> = n.items.iter().collect();
        let first = items[0].key as i64;
        let last = items[items.len() - 1].key as i64;
        if let Some(r) = nearest(first, -1, n.rank) {
            assert!(r > n.rank);
        }
        if let Some(r) = nearest(last, 1, n.rank) {
            assert!(r > n.rank);
        }
    }
}

#[test]
fn definition_3_nodes_per_rank_equal_runs_per_rank() {
    let rank = hashed4();
    let rank = |k: &u64| rank(k, 0);
    let keys = shuffle(&range(3000), &mut Prng::new(20));
    let t = build(&keys, &rank);
    let sorted = range(3000);
    let mut counted: HashMap<u32, usize> = HashMap::new();
    for n in nodes(&t) {
        *counted.entry(n.rank).or_default() += 1;
    }
    for (&r, &count) in &counted {
        let mut runs = 0;
        let mut open = false;
        for key in &sorted {
            let rr = rank(key);
            if rr > r {
                open = false;
            } else if rr == r && !open {
                runs += 1;
                open = true;
            }
        }
        assert_eq!(count, runs, "rank {r}");
    }
}

#[test]
fn definition_3_insertion_yields_the_constructed_tree() {
    let rank = hashed4();
    let rank = |k: &u64| rank(k, 0);
    let keys = shuffle(&range(3000), &mut Prng::new(20));
    let sorted = range(3000);
    let expected = fingerprint(&construct(&sorted, &rank));
    assert_eq!(fingerprint(&build(&keys, &rank)), expected);
    assert_eq!(
        fingerprint(&build(&shuffle(&keys, &mut Prng::new(21)), &rank)),
        expected
    );
    assert_eq!(check(&build(&keys, &rank), &rank), sorted);
}

// Figure 6: mean maximal rank and height of 200 random trees, by k and n.
#[allow(clippy::type_complexity)]
const FIGURE6: [(u32, [(u64, f64, f64); 3]); 4] = [
    (2, [(100, 8.0, 6.7), (1000, 11.3, 9.9), (10000, 14.7, 13.2)]),
    (4, [(100, 4.2, 4.0), (1000, 5.9, 5.6), (10000, 7.5, 7.3)]),
    (16, [(100, 2.4, 2.3), (1000, 3.2, 3.2), (10000, 4.1, 4.1)]),
    (64, [(100, 1.8, 1.8), (1000, 2.2, 2.2), (10000, 3.0, 3.0)]),
];
// Figure 7: mean node size and figure 8: mean node count, at n = 10 000.
const FIGURE7: [(u32, f64); 4] = [(2, 1.998), (4, 3.994), (16, 15.908), (64, 62.68)];
const FIGURE8: [(u32, f64); 4] = [(2, 5002.0), (4, 2504.0), (16, 628.2), (64, 159.16)];

fn forest(k: u32, n: u64) -> Vec<(T, Rank<u64>)> {
    (0..3)
        .map(|i| {
            let rank = trial(k, i);
            let keys = shuffle(&range(n), &mut Prng::new(i));
            (build(&keys, &|key| rank(key, 0)), rank)
        })
        .collect()
}

#[test]
fn analysis_4_1_1_height_is_at_most_the_root_rank_about_log_k_n() {
    for (k, rows) in FIGURE6 {
        for (n, max_rank, h) in rows {
            let ts = forest(k, n);
            for (t, _) in &ts {
                assert!(height(t) as u32 <= t.as_ref().unwrap().rank);
            }
            // E(M_T) <= ceil(log_k n) + k, with high probability.
            let bound = ((n as f64).ln() / (k as f64).ln()).ceil() + k as f64;
            let ranks: Vec<f64> = ts
                .iter()
                .map(|(t, _)| t.as_ref().unwrap().rank as f64)
                .collect();
            assert!(mean(&ranks) <= bound + 1.0);
            let slack = if k == 2 { 3.0 } else { 1.5 };
            assert!(
                (mean(&ranks) - max_rank).abs() <= slack,
                "k={k} n={n} ranks {ranks:?}"
            );
            let heights: Vec<f64> = ts.iter().map(|(t, _)| height(t) as f64).collect();
            assert!(
                (mean(&heights) - h).abs() <= slack,
                "k={k} n={n} heights {heights:?}"
            );
        }
    }
}

#[test]
fn analysis_4_1_2_node_sizes_are_geometric_with_mean_k() {
    for (k, figure7) in FIGURE7 {
        let sizes: Vec<f64> = forest(k, 10000)
            .iter()
            .flat_map(|(t, _)| nodes(t).into_iter().map(|n| n.items.iter().count() as f64))
            .collect();
        assert!(
            (mean(&sizes) - figure7).abs() <= 0.1 * k as f64,
            "k={k} mean {}",
            mean(&sizes)
        );
        // P(|g| >= s) = (1 - 1/k)^(s - 1): the paper's bound with a corrected exponent.
        for c in [1, 2, 3] {
            let s = (c * k) as f64;
            let observed = sizes.iter().filter(|&&x| x >= s).count() as f64 / sizes.len() as f64;
            let expected = (1.0 - 1.0 / k as f64).powf(s - 1.0);
            let sd = (expected * (1.0 - expected) / sizes.len() as f64).sqrt();
            assert!(
                (observed - expected).abs() <= 4.0 * sd + 1e-3,
                "k={k} s={s} {observed} vs {expected}"
            );
        }
    }
}

#[test]
fn analysis_4_1_3_the_number_of_nodes_is_about_n_over_k() {
    for (k, figure8) in FIGURE8 {
        let counts: Vec<f64> = forest(k, 10000)
            .iter()
            .map(|(t, _)| nodes(t).len() as f64)
            .collect();
        let expected = 10000.0 / k as f64 + 1.0;
        for &c in &counts {
            assert!(
                (c - expected).abs() <= 4.0 * expected.sqrt(),
                "k={k} count {c}"
            );
        }
        assert!((mean(&counts) - figure8).abs() <= 3.0 * expected.sqrt());
    }
}

#[test]
fn ranks_from_the_default_hash_are_geometric_by_chi_square() {
    let n = 100_000u64;
    for k in [2u32, 3, 8] {
        let rank = hashed::<u64>(k);
        let buckets = 6;
        let mut counts = vec![0f64; buckets + 1];
        for i in 0..n {
            counts[(rank(&i, 0) as usize).min(buckets + 1) - 1] += 1.0;
        }
        let mut chi2 = 0.0;
        for r in 1..=buckets + 1 {
            let kf = k as f64;
            let p = if r <= buckets {
                (1.0 - 1.0 / kf) / kf.powi(r as i32 - 1)
            } else {
                1.0 / kf.powi(buckets as i32)
            };
            chi2 += (counts[r - 1] - n as f64 * p).powi(2) / (n as f64 * p);
        }
        assert!(chi2 < 22.5, "k={k} chi2={chi2}"); // 99.9th percentile, 6 degrees of freedom
    }
}

#[test]
fn algorithms_unzip_and_zip_laws() {
    let rank = hashed4();
    let rank = |k: &u64| rank(k, 0);
    let keys = shuffle(&range(400), &mut Prng::new(22));
    let t = build(&keys, &rank);
    let mut rand = Prng::new(23);
    // Positions between keys, expressed as an ordering on entries.
    let gap = |g: f64| move |e: &E| (e.key as f64).partial_cmp(&g).unwrap();
    for _ in 0..100 {
        let key = rand.below(400) as u64;
        let (l, hit, r) = unzip(&t, &at(key));
        assert_eq!(hit.as_ref().map(|e| e.key), Some(key));
        assert_eq!(
            fingerprint(&zip(&zip(&l, &one(key, &rank)), &r)),
            fingerprint(&t)
        );
        let without: Vec<u64> = keys.iter().copied().filter(|&x| x != key).collect();
        assert_eq!(
            fingerprint(&zip(&l, &r)),
            fingerprint(&build(&without, &rank))
        );
        assert_eq!(
            fingerprint(&remove(&t, &at(key))),
            fingerprint(&zip(&l, &r))
        );
        let g = key as f64 + 0.5;
        let (gl, none, gr) = unzip(&t, &gap(g));
        assert!(none.is_none());
        assert_eq!(fingerprint(&zip(&gl, &gr)), fingerprint(&t));
        let (a, _, bc) = unzip(&t, &gap(g));
        let (b, _, c) = unzip(&bc, &gap(g + rand.below(100) as f64));
        assert_eq!(fingerprint(&zip(&zip(&a, &b), &c)), fingerprint(&t));
        assert_eq!(fingerprint(&zip(&a, &zip(&b, &c))), fingerprint(&t));
    }
    assert!(Arc::ptr_eq(
        zip(&t, &None).as_ref().unwrap(),
        t.as_ref().unwrap()
    ));
    assert!(Arc::ptr_eq(
        zip(&None, &t).as_ref().unwrap(),
        t.as_ref().unwrap()
    ));
    assert_eq!(
        fingerprint(&insert(&t, &at(7), &one(7, &rank))),
        fingerprint(&t)
    );
}

#[test]
fn algorithms_sorted_zip_append_builds_the_same_tree_as_inserting() {
    let rank = hashed4();
    let rank = |k: &u64| rank(k, 0);
    let keys = shuffle(&range(400), &mut Prng::new(22));
    let mut appended = None;
    for key in range(400) {
        appended = zip(&appended, &one(key, &rank));
    }
    assert_eq!(fingerprint(&appended), fingerprint(&build(&keys, &rank)));
}

#[test]
fn algorithms_an_update_shares_all_but_a_path_of_nodes() {
    let n = 5000;
    let rank = hashed::<u64>(8);
    let rank = |k: &u64| rank(k, 0);
    let old = build(&shuffle(&range(n), &mut Prng::new(24)), &rank);
    let before = fingerprint(&old);
    let shared: std::collections::HashSet<*const Node<u64, (), ArrayItems<u64, ()>>> =
        nodes(&old).into_iter().map(|n| n as *const _).collect();
    for key in [n + 1, 100_000] {
        let updated = insert(&old, &at(key), &one(key, &rank));
        let fresh = nodes(&updated)
            .into_iter()
            .filter(|n| !shared.contains(&(*n as *const _)))
            .count();
        assert!(fresh <= 2 * height(&updated) + 2, "{fresh} fresh nodes");
        assert!(fresh < shared.len() / 20);
        assert_eq!(size(&updated), n as usize + 1);
    }
    assert_eq!(fingerprint(&old), before);
}

#[test]
fn algorithms_work_per_operation_grows_like_log_n() {
    let rank = hashed::<u64>(8);
    let rank = |k: &u64| rank(k, 0);
    let cost = |n: u64| {
        let t = build(&shuffle(&range(n), &mut Prng::new(n as u32)), &rank);
        let compares = std::cell::Cell::new(0usize);
        for i in 0..200u64 {
            let g = i as f64 + 0.5;
            let counting = |e: &E| {
                compares.set(compares.get() + 1);
                (e.key as f64).partial_cmp(&g).unwrap()
            };
            insert(&t, &counting, &one(i, &rank));
        }
        compares.get() as f64 / 200.0
    };
    let (small, large) = (cost(1000), cost(100_000));
    assert!(large / small < 4.0, "{small} vs {large}"); // log growth; linear would be 100
}

#[test]
fn family_zip_zip_trees_rank_inner_sets_by_the_second_seed() {
    let keys = range(2000);
    let mut set = GSet::with(gk_by::<u64, ()>(hashed(2), 1));
    set.extend(shuffle(&keys, &mut Prng::new(25)));
    let mut again = GSet::with(gk_by::<u64, ()>(hashed(2), 1));
    again.extend(shuffle(&keys, &mut Prng::new(26)));
    assert_eq!(fingerprint(set.root()), fingerprint(again.root()));
    let inner = hashed::<u64>(2);
    let mut tree_nodes = 0;
    let mut inner_depth = 0;
    for n in nodes(set.root()) {
        let GkItems::Tree(t, _) = &n.items else {
            continue;
        };
        tree_nodes += 1;
        inner_depth = inner_depth.max(height(t));
        for m in nodes(t) {
            for e in m.items.iter() {
                assert_eq!(inner(&e.key, 1), m.rank);
            }
        }
    }
    assert!(tree_nodes > 0);
    assert!(inner_depth <= 8); // runs have mean length 2
    assert_eq!(set.iter().copied().collect::<Vec<_>>(), keys);
}

#[test]
fn family_treaps_ranks_capped_at_two_nest_until_they_tell_keys_apart() {
    let geometric = hashed::<u64>(2);
    let rank: Rank<u64> = Arc::new(move |key: &u64, seed: u32| geometric(key, seed).min(2));
    let keys = range(300);
    let mut set = GSet::with(gk_by::<u64, ()>(rank.clone(), 1));
    set.extend(shuffle(&keys, &mut Prng::new(27)));
    assert_eq!(set.iter().copied().collect::<Vec<_>>(), keys);
    fn depth<V: Clone>(items: &GkItems<u64, V>, d: usize) -> usize {
        match items {
            GkItems::Array(..) => 0,
            GkItems::Tree(t, _) => nodes(t)
                .iter()
                .map(|n| depth(&n.items, d + 1))
                .max()
                .unwrap_or(d)
                .max(d),
        }
    }
    let deepest = nodes(set.root())
        .iter()
        .map(|n| depth(&n.items, 1))
        .max()
        .unwrap();
    assert!(deepest >= 3, "only {deepest} dimensions");
    for e in entries(set.root()) {
        assert!(rank(&e.key, 0) <= 2);
    }
}
