//! Rough timings: `cargo run --release --example bench [n]`.
use std::time::Instant;

use geometric_tree::{GMap, Options, gk};

fn time(label: &str, f: impl FnOnce()) {
    let start = Instant::now();
    f();
    println!("{label:<28} {:>6} ms", start.elapsed().as_millis());
}

fn run<S: geometric_tree::Items<u64, u64>>(name: &str, options: Options<u64, S>, keys: &[u64]) {
    let mut map = GMap::with(options);
    time(&format!("{name} insert"), || {
        for &k in keys {
            map = map.insert(k, k);
        }
    });
    time(&format!("{name} get"), || {
        for &k in keys {
            assert_eq!(map.get(&k), Some(&k));
        }
    });
    time(&format!("{name} iterate"), || {
        assert_eq!(map.iter().count(), keys.len())
    });
    time(&format!("{name} remove"), || {
        for &k in keys {
            map = map.remove(&k);
        }
    });
    println!();
}

fn main() {
    let n: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(100_000);
    let mut keys: Vec<u64> = (0..n).collect();
    let mut state = 0x9e3779b97f4a7c15u64;
    for i in (1..keys.len()).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        keys.swap(i, (state % (i as u64 + 1)) as usize);
    }
    println!("n = {n}\n");
    for k in [2, 4, 8, 16, 32] {
        run(
            &format!("k={k}"),
            Options::new(k, geometric_tree::ArrayItems::default()),
            &keys,
        );
    }
    run("gk k=8", gk::<u64, u64>(8), &keys);
    time("BTreeMap (mutable, reference)", || {
        let mut m = std::collections::BTreeMap::new();
        for &k in &keys {
            m.insert(k, k);
        }
        for &k in &keys {
            m.get(&k);
        }
        for &k in &keys {
            m.remove(&k);
        }
    });
}
