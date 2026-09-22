mod common;

use common::Prng;
use geometric_tree::rank::{hash, hash_utf16, hashed, hashed_by};

/// The rank by definition: 1 + the largest r with k^r * (h + 1) <= 2^32.
fn reference(h: u32, k: u128) -> u32 {
    let mut r = 0;
    while k.pow(r + 1) * (h as u128 + 1) <= 1u128 << 32 {
        r += 1;
    }
    r + 1
}

#[test]
fn ranks_match_the_exact_definition_including_at_every_boundary() {
    let mut rand = Prng::new(12);
    for k in [2u32, 3, 8, 10, 16, 1000] {
        let rank = hashed_by::<u32>(k, |h, _| *h);
        let mut hs = vec![0, 1, u32::MAX, u32::MAX - 1];
        let mut r = 1;
        while (k as u128).pow(r) <= 1 << 32 {
            let b = ((1u128 << 32) / (k as u128).pow(r)) as i64;
            hs.extend(
                [b - 2, b - 1, b, b + 1]
                    .iter()
                    .filter(|&&h| h >= 0)
                    .map(|&h| h as u32),
            );
            r += 1;
        }
        for _ in 0..500 {
            hs.push((rand.next_f64() * 4294967296.0) as u32);
        }
        for h in hs {
            assert_eq!(rank(&h, 0), reference(h, k as u128), "k={k} h={h}");
        }
    }
}

#[test]
fn ranks_are_geometric() {
    for k in [2u32, 8] {
        let rank = hashed::<u64>(k);
        let n = 100_000u64;
        let mut counts = std::collections::HashMap::new();
        for i in 0..n {
            *counts.entry(rank(&i, 0)).or_insert(0u64) += 1;
        }
        for r in 1..=3u32 {
            let above: u64 = counts
                .iter()
                .filter(|(rr, _)| **rr > r)
                .map(|(_, c)| c)
                .sum();
            let observed = above as f64 / n as f64;
            let expected = (k as f64).powi(-(r as i32));
            assert!(
                (observed - expected).abs() < 0.01,
                "k={k} r={r} {observed} vs {expected}"
            );
        }
    }
}

#[test]
fn seeds_give_independent_hashes() {
    assert_ne!(hash("a", 0), hash("a", 1));
    assert_eq!(hash("a", 0), hash("a", 0));
}

#[test]
fn utf16_hash_matches_the_typescript_package() {
    // Values from `hash` in the npm package geometric-tree.
    assert_eq!(hash_utf16("abc", 0), TS_ABC_0);
    assert_eq!(hash_utf16("abc", 1), TS_ABC_1);
    assert_eq!(hash_utf16("12345", 7), TS_12345_7);
    assert_eq!(hash_utf16("", 0), TS_EMPTY_0);
    let ts = hashed_by::<String>(8, |s, seed| hash_utf16(s, seed));
    assert_eq!(ts(&"7919".to_string(), 0), TS_RANK8_7919);
    let ts2 = hashed_by::<String>(2, |s, seed| hash_utf16(s, seed));
    assert_eq!(ts2(&"42".to_string(), 0), TS_RANK2_42);
}

const TS_ABC_0: u32 = 4020954904;
const TS_ABC_1: u32 = 583070294;
const TS_12345_7: u32 = 673852440;
const TS_EMPTY_0: u32 = 2173192212;
const TS_RANK8_7919: u32 = 1;
const TS_RANK2_42: u32 = 1;
