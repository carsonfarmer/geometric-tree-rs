# geometric-tree

Immutable, history-independent maps, sets and lists built on geometric search trees.

A [G-tree](https://g-trees.github.io/g_trees/) is a randomised search tree in which every key
gets a geometrically distributed *rank*, and a node holds the whole run of keys that share the
highest rank in its subtree. Ranks derived from a hash of the key make the tree a pure function
of its contents: the same set of keys always yields the same tree, whatever the order of
insertions and deletions. With about `k` keys per node the family spans zip trees (`k = 2`),
cache-friendly wide trees, and, with a G-tree as the inner set of each node, zip-zip trees.

The point of this crate is how little code it takes. The whole tree is two operations, `unzip`
and `zip`, and everything else is a composition of them. It is a port of the TypeScript package
[`geometric-tree`](https://github.com/carsonfarmer/geometric-tree), module for module, and the
two build bit-identical trees for keys with the same string form (the tests check it).

## Install

```sh
cargo add geometric-tree
```

No dependencies. Compiles for `wasm32-unknown-unknown` as is.

## Usage

```rust
use geometric_tree::{GList, GMap, GSet};

// Map: ordered by key, every update returns a new map.
let m: GMap<&str, u32> = [("b", 2), ("a", 1)].into_iter().collect();
let m = m.insert("c", 3).remove(&"a");
assert_eq!(m.get(&"b"), Some(&2));
assert!(!m.contains_key(&"a"));
assert_eq!(m.iter().collect::<Vec<_>>(), [(&"b", &2), (&"c", &3)]);

// Set.
let s: GSet<u32> = [3, 1, 2].into_iter().collect();
let s = s.insert(4).remove(&1);
assert_eq!(s.iter().copied().collect::<Vec<_>>(), [2, 3, 4]);

// List: logarithmic insert, remove, slice and concat at any position.
let l: GList<&str> = vec!["a", "b", "d"].into();
let l = l.insert(2, "c").remove(0);
assert_eq!(l.get(0), Some(&"b"));
assert_eq!(l.slice(1..).iter().copied().collect::<Vec<_>>(), ["c", "d"]);
assert_eq!(l.concat(&l).len(), 6);
```

All structures are persistent: old versions stay valid and share structure with new ones, through
`Arc`. Keys need `Ord + Clone`, values `Clone`; wrap large ones in `Arc`.

## How it works

A tree is `Option<Arc<Node>>`. A node has a `rank`, an ordered collection of `items`, each an
`Entry` holding a key, a value and the subtree of the keys before it, and one `right` subtree of
the keys after the last item.

```rust,ignore
pub type Tree<K, V, S> = Option<Arc<Node<K, V, S>>>;
pub struct Node<K, V, S> { pub rank: u32, pub items: S, pub right: Tree<K, V, S>, pub size: usize, /* … */ }
pub struct Entry<K, V, S> { pub key: K, pub value: Value<K, V, S>, pub left: Tree<K, V, S> }
```

`unzip` splits a tree around a position into everything before it, the entry at it, and
everything after; `zip` joins two ordered trees. Both walk one path of the tree and rebuild the
nodes on it, so they take logarithmic time.

```rust,ignore
pub fn unzip<K, V, S: Items<K, V>>(t: &Tree<K, V, S>, at: &impl Fn(&Entry<K, V, S>) -> Ordering)
    -> (Tree<K, V, S>, Option<Entry<K, V, S>>, Tree<K, V, S>)
{
    let Some(n) = t else { return (None, None, None) };
    let (lo, e, hi) = n.items.split(at);
    if let Some(e) = e {
        let left = e.left.clone();
        return (node(n.rank, lo, left), Some(e), node(n.rank, hi, n.right.clone()));
    }
    if hi.weight() == 0 {
        let (l, hit, r) = unzip(&n.right, at);
        return (node(n.rank, lo, l), hit, r);
    }
    let (first, rest) = hi.shift();
    let (l, hit, r) = unzip(&first.left, at);
    let right = node(n.rank, rest.unshift(Entry { left: r, ..first }), n.right.clone());
    (node(n.rank, lo, l), hit, right)
}

pub fn zip<K, V, S: Items<K, V>>(l: &Tree<K, V, S>, r: &Tree<K, V, S>) -> Tree<K, V, S> {
    let (Some(ln), Some(rn)) = (l, r) else { return l.clone().or_else(|| r.clone()) };
    if ln.rank > rn.rank {
        return node(ln.rank, ln.items.clone(), zip(&ln.right, r));
    }
    let (first, rest) = rn.items.shift();
    if ln.rank < rn.rank {
        let left = zip(l, &first.left);
        return node(rn.rank, rest.unshift(Entry { left, ..first }), rn.right.clone());
    }
    let left = zip(&ln.right, &first.left);
    node(rn.rank, ln.items.join(&rest.unshift(Entry { left, ..first })), rn.right.clone())
}
```

Insertion is `zip(zip(before, single), after)` and deletion is `zip(before, after)`, both
starting from `unzip`. Because insertion drops any existing entry at the position, a map's
`insert` is an upsert for free. The list variant splits by position instead of by key, using the
node sizes, and shares `zip`.

The inner collection of a node, `Items`, is a trait with six methods (`weight`, `split`, `find`,
`join`, `shift`, `unshift`) plus iteration. The default, `ArrayItems`, is a sorted shared slice.
Nodes have constant expected size, so linear-time slice operations cost nothing asymptotically.

For contrast, `src/binary.rs` is the classic binary zip tree with the same function names. A
G-tree with `k = 2` is that tree with every run of equal-rank right children folded into one
node, and the tests unfold one into the other to check it.

## Options

`GMap::with` and `GSet::with` take an `Options { rank, empty }`: the rank function and the empty
inner set for a node. `Options::new(k, ArrayItems::default())` gives hashed ranks with about `k`
keys per node; `GMap::new()` uses `k = 8`.

`hashed(k)` is the construction from section 3.2 of the paper: hash the key, then count the
leading zero digits of the hash in base `k`. It is computed by inverse transform, one logarithm
followed by an exact integer check, so it costs the same for any `k` and gives identical ranks
on every platform, wasm included. The default hash feeds any `Hash` key through a small,
deterministic, non-cryptographic mixer (`Cyrb`); `hash_utf16` hashes a string the way the
TypeScript package does, and `hashed_by(k, hash)` accepts any 32-bit hash. `random(k)` needs no
hash but is history dependent; `GList` uses it, since positions carry no key.

## Gk-trees

A G-tree is only as balanced as its ranks. Someone who can craft keys can give them all the
same rank, and a plain G-tree then degenerates into one node holding every key. Jannik
Hehemann's master's thesis (Mittweida, 2025, section 4.1) proposes a fix: once a node holds
more than a threshold of entries, store them as a G-tree of their own, ranked by a fresh seed,
and so on recursively. Forcing a collision now costs the attacker one search per dimension,
and operations stay in O(log² n) even against an adaptive adversary.

`GkItems` is that inner set: a sorted slice that becomes a G-tree past the threshold and back
below it. Converting in both directions at the same size keeps the representation a function of
the stored set, so history independence survives. Inner trees store the outer entries they
stand for as `Value::Outer`, so every dimension uses one entry type and the core is untouched.

```rust
use geometric_tree::{GSet, gk, gk_by, hashed};

let s = GSet::with(gk::<String, ()>(8));                     // threshold 12k
let zz = GSet::with(gk_by::<String, ()>(hashed(2), 1));      // zip-zip trees
```

## Performance

Simplicity comes first, but the structures are usable. Rough figures for 100 000 `u64` keys on
one core (`cargo run --release --example bench`), per operation:

| structure    | insert | get     | iterate  | remove |
| ------------ | ------ | ------- | -------- | ------ |
| `GMap`, k=8  | ~6 µs  | ~0.2 µs | ~0.02 µs | ~5 µs  |
| `GMap`, k=2  | ~6 µs  | ~0.5 µs | ~0.05 µs | ~5 µs  |

Updates allocate a new node for every node on the path, plus a copy of each node's entries; that
is the price of persistence. Building insertion from `unzip` and `zip` costs roughly two extra
path walks compared to a hand-written insert, which is the trade the paper makes as well.

## Development

```sh
cargo test --release
cargo clippy --all-targets -- -D warnings
cargo build --target wasm32-unknown-unknown
cargo run --release --example bench
```

To release, bump `version` in `Cargo.toml`, then push a matching tag (`git tag v0.2.0 && git push
--tags`); the release workflow tests and publishes to crates.io by trusted publishing.

## References

- Carson Farmer and Aljoscha Meyer. *Geometric Search Trees.* https://g-trees.github.io/g_trees/
- Jannik Hehemann. *History-independent data structures and their synchronisation.* Master's
  thesis, Hochschule Mittweida, 2025. Section 4.1, Gk-trees.
- Tarjan, Levy and Timmel. *Zip Trees.* ACM Transactions on Algorithms, 2021.

## License

MIT © Carson Farmer
