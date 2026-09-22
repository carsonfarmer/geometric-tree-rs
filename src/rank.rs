//! Rank functions. Ranks are geometric random variables with
//! P(rank = r) = (1 - 1/k) / k^(r - 1), so G-nodes hold about k keys and the
//! tree has about n/k nodes and log_k(n) height.

use std::hash::{BuildHasher, Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Assigns a rank to a key. The seed selects an independent rank function.
pub type Rank<K> = Arc<dyn Fn(&K, u32) -> u32 + Send + Sync>;

/// The number of hash values: hashes are 32-bit integers.
const M: f64 = 4294967296.0;

/// A small, fast, well-mixed string hash (cyrb53's mixing, folded to 32
/// bits), fed byte by byte. Deterministic on every platform and Rust version,
/// which the standard library's hashers do not promise. Not cryptographic: it
/// makes trees history independent, but whoever can choose keys can also
/// choose their ranks.
pub struct Cyrb {
    h1: u32,
    h2: u32,
}

impl Cyrb {
    /// The seed is mixed in as a leading input unit, so different seeds give
    /// unrelated hashes rather than a permutation of the same values.
    pub fn new(seed: u32) -> Self {
        let mut h = Cyrb {
            h1: 0xdeadbeef,
            h2: 0x41c6ce57,
        };
        h.mix(seed);
        h
    }

    /// Mix one unit of input (a byte, or a UTF-16 code unit).
    pub fn mix(&mut self, c: u32) {
        self.h1 = (self.h1 ^ c).wrapping_mul(2654435761);
        self.h2 = (self.h2 ^ c).wrapping_mul(1597334677);
    }

    pub fn value(&self) -> u32 {
        let (h1, h2) = (self.h1, self.h2);
        let h1 =
            (h1 ^ (h1 >> 16)).wrapping_mul(2246822507) ^ (h2 ^ (h2 >> 13)).wrapping_mul(3266489909);
        let h2 =
            (h2 ^ (h2 >> 16)).wrapping_mul(2246822507) ^ (h1 ^ (h1 >> 13)).wrapping_mul(3266489909);
        h1 ^ h2
    }
}

impl Hasher for Cyrb {
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.mix(b as u32);
        }
    }
    fn finish(&self) -> u64 {
        self.value() as u64
    }
}

/// Hash any [`Hash`] key to a 32-bit integer with [`Cyrb`].
pub fn hash<K: Hash + ?Sized>(key: &K, seed: u32) -> u32 {
    let mut h = Cyrb::new(seed);
    key.hash(&mut h);
    h.value()
}

/// Hash a string by its UTF-16 code units: the same function as the default
/// hash of the TypeScript package, so both build identical trees for keys
/// with the same string form.
pub fn hash_utf16(s: &str, seed: u32) -> u32 {
    let mut h = Cyrb::new(seed);
    for c in s.encode_utf16() {
        h.mix(c as u32);
    }
    h.value()
}

/// Deterministic ranks by inverse transform: the hash is a uniform u in
/// (0, 1], and rank = 1 + floor(log_k(1/u)), the number of leading zero digits
/// of the hash in base k (the paper's section 3.2). One logarithm, then an
/// integer comparison against precomputed powers of k, so the result is exact
/// and identical on every platform. Trees built with these depend only on
/// their contents (history independence), so equal sets have equal trees.
pub fn hashed_by<K: 'static>(
    k: u32,
    hash: impl Fn(&K, u32) -> u32 + Send + Sync + 'static,
) -> Rank<K> {
    let bound: Vec<f64> = (0..35).map(|r| M / (k as f64).powi(r)).collect();
    let lnk = (k as f64).ln();
    Arc::new(move |key: &K, seed: u32| {
        let h = hash(key, seed) as f64 + 1.0;
        let mut r = ((M / h).ln() / lnk).floor() as usize;
        if h > bound[r] {
            r -= 1;
        } else if h <= bound[r + 1] {
            r += 1;
        }
        r as u32 + 1
    })
}

/// [`hashed_by`] with [`hash`], for any [`Hash`] key.
pub fn hashed<K: Hash + 'static>(k: u32) -> Rank<K> {
    hashed_by(k, hash)
}

/// Random ranks: the tree depends on insertion order, but keys need no hash.
pub fn random<K: 'static>(k: u32) -> Rank<K> {
    let seed = std::hash::RandomState::new().build_hasher().finish() | 1;
    let state = AtomicU64::new(seed);
    Arc::new(move |_: &K, _: u32| {
        let mut rank = 1;
        loop {
            // xorshift64*, one step per Bernoulli trial.
            let mut x = state.load(Ordering::Relaxed);
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            state.store(x, Ordering::Relaxed);
            if (x.wrapping_mul(0x2545F4914F6CDD1D) >> 32) as u32 % k != 0 {
                return rank;
            }
            rank += 1;
        }
    })
}
