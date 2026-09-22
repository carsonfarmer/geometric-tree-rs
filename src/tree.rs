//! Geometric search trees (G-trees), after Farmer & Meyer,
//! <https://g-trees.github.io/g_trees/>.
//!
//! A G-tree is a randomised, history-independent search tree. Every key has a
//! geometrically distributed `rank`. A G-node holds the maximal run of keys
//! that share the highest rank in its subtree, each paired with the subtree of
//! the keys that precede it, plus one `right` subtree of the keys that follow
//! the last one. The keys of a node live in an [`Items`] collection, which is
//! pluggable: a sorted array gives the plain G-tree, another G-tree gives a
//! Gk-tree (see [`crate::gk`]).
//!
//! Everything here is immutable and shares structure through [`Arc`]. The two
//! structural operations are [`unzip`] (split a tree around a position) and
//! [`zip`] (join two ordered trees); insertion and deletion are compositions
//! of them.

use std::cmp::Ordering;
use std::marker::PhantomData;
use std::sync::Arc;

/// A possibly empty G-tree.
pub type Tree<K, V, S> = Option<Arc<Node<K, V, S>>>;

/// A G-node: the keys of one rank, plus the subtree of the keys after them.
pub struct Node<K, V, S> {
    pub rank: u32,
    pub items: S,
    pub right: Tree<K, V, S>,
    pub size: usize,
    _kv: PhantomData<fn() -> (K, V)>,
}

/// A key, its value, and the subtree of all keys that precede it.
pub struct Entry<K, V, S> {
    pub key: K,
    pub value: Value<K, V, S>,
    pub left: Tree<K, V, S>,
}

/// What an entry holds: a value of its own, or, in a tree that serves as the
/// inner set of another tree, the entry of that outer tree it stands for.
pub enum Value<K, V, S> {
    Own(V),
    Outer(Arc<Entry<K, V, S>>),
}

impl<K, V, S> Entry<K, V, S> {
    /// Number of keys this entry stands for: itself and its left subtree.
    pub fn weight(&self) -> usize {
        let own = match &self.value {
            Value::Own(_) => 1,
            Value::Outer(outer) => outer.weight(),
        };
        size(&self.left) + own
    }

    /// The value stored for this key, following outer entries as needed.
    pub fn value(&self) -> &V {
        match &self.value {
            Value::Own(v) => v,
            Value::Outer(outer) => outer.value(),
        }
    }
}

impl<K: Clone, V: Clone, S> Clone for Entry<K, V, S> {
    fn clone(&self) -> Self {
        Entry {
            key: self.key.clone(),
            value: self.value.clone(),
            left: self.left.clone(),
        }
    }
}

impl<K, V: Clone, S> Clone for Value<K, V, S> {
    fn clone(&self) -> Self {
        match self {
            Value::Own(v) => Value::Own(v.clone()),
            Value::Outer(e) => Value::Outer(e.clone()),
        }
    }
}

/// Number of keys in a tree.
pub fn size<K, V, S>(t: &Tree<K, V, S>) -> usize {
    t.as_ref().map_or(0, |n| n.size)
}

/// An immutable, ordered collection of entries: the inner set of a G-node.
///
/// `at` locates a position: it returns [`Ordering::Less`] for entries before
/// the position, [`Ordering::Equal`] for an entry at it and
/// [`Ordering::Greater`] for entries after; it must be monotone over the
/// collection. Expected sizes are constant, so O(size) implementations are fine.
pub trait Items<K, V>: Clone {
    /// Number of keys in the entries and their left subtrees.
    fn weight(&self) -> usize;
    /// Entries before the position, the entry at it (if any), entries after.
    fn split(
        &self,
        at: &impl Fn(&Entry<K, V, Self>) -> Ordering,
    ) -> (Self, Option<Entry<K, V, Self>>, Self);
    /// The first entry at or after the position.
    fn find(&self, at: &impl Fn(&Entry<K, V, Self>) -> Ordering) -> Option<&Entry<K, V, Self>>;
    /// Concatenate; every entry of `other` must follow every entry of this.
    fn join(&self, other: &Self) -> Self;
    /// The first entry and the rest. Must not be called when empty.
    fn shift(&self) -> (Entry<K, V, Self>, Self);
    /// Prepend an entry that precedes every existing entry.
    fn unshift(&self, e: Entry<K, V, Self>) -> Self;
    /// The entries, in order.
    fn iter(&self) -> Box<dyn Iterator<Item = &Entry<K, V, Self>> + '_>;
}

/// A sorted, shared slice of entries with its weight: the storage behind
/// [`ArrayItems`] and the array mode of [`crate::gk::GkItems`].
pub struct Slice<K, V, S> {
    pub entries: Arc<[Entry<K, V, S>]>,
    pub weight: usize,
}

impl<K, V, S> Clone for Slice<K, V, S> {
    fn clone(&self) -> Self {
        Slice {
            entries: self.entries.clone(),
            weight: self.weight,
        }
    }
}

impl<K: Clone, V: Clone, S> Slice<K, V, S> {
    pub fn new(entries: Vec<Entry<K, V, S>>) -> Self {
        let weight = entries.iter().map(Entry::weight).sum();
        Slice {
            entries: entries.into(),
            weight,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn split(
        &self,
        at: &impl Fn(&Entry<K, V, S>) -> Ordering,
    ) -> (Self, Option<Entry<K, V, S>>, Self) {
        let Some(i) = self.entries.iter().position(|e| at(e) != Ordering::Less) else {
            return (self.clone(), None, Self::new(vec![]));
        };
        let hit = at(&self.entries[i]) == Ordering::Equal;
        let lo = Self::new(self.entries[..i].to_vec());
        let hi = Self::new(self.entries[if hit { i + 1 } else { i }..].to_vec());
        (lo, hit.then(|| self.entries[i].clone()), hi)
    }

    pub fn find(&self, at: &impl Fn(&Entry<K, V, S>) -> Ordering) -> Option<&Entry<K, V, S>> {
        self.entries.iter().find(|e| at(e) != Ordering::Less)
    }

    pub fn join(&self, other: &Self) -> Self {
        Self::new(
            self.entries
                .iter()
                .chain(other.entries.iter())
                .cloned()
                .collect(),
        )
    }

    pub fn shift(&self) -> (Entry<K, V, S>, Self) {
        (
            self.entries[0].clone(),
            Self::new(self.entries[1..].to_vec()),
        )
    }

    pub fn unshift(&self, e: Entry<K, V, S>) -> Self {
        let mut entries = Vec::with_capacity(self.entries.len() + 1);
        entries.push(e);
        entries.extend_from_slice(&self.entries);
        Self::new(entries)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Entry<K, V, S>> {
        self.entries.iter()
    }
}

/// [`Items`] backed by a sorted array.
pub struct ArrayItems<K, V>(pub Slice<K, V, ArrayItems<K, V>>);

impl<K, V> Clone for ArrayItems<K, V> {
    fn clone(&self) -> Self {
        ArrayItems(self.0.clone())
    }
}

impl<K: Clone, V: Clone> Default for ArrayItems<K, V> {
    fn default() -> Self {
        ArrayItems(Slice::new(vec![]))
    }
}

impl<K: Clone, V: Clone> Items<K, V> for ArrayItems<K, V> {
    fn weight(&self) -> usize {
        self.0.weight
    }
    fn split(
        &self,
        at: &impl Fn(&Entry<K, V, Self>) -> Ordering,
    ) -> (Self, Option<Entry<K, V, Self>>, Self) {
        let (lo, hit, hi) = self.0.split(at);
        (ArrayItems(lo), hit, ArrayItems(hi))
    }
    fn find(&self, at: &impl Fn(&Entry<K, V, Self>) -> Ordering) -> Option<&Entry<K, V, Self>> {
        self.0.find(at)
    }
    fn join(&self, other: &Self) -> Self {
        ArrayItems(self.0.join(&other.0))
    }
    fn shift(&self) -> (Entry<K, V, Self>, Self) {
        let (e, rest) = self.0.shift();
        (e, ArrayItems(rest))
    }
    fn unshift(&self, e: Entry<K, V, Self>) -> Self {
        ArrayItems(self.0.unshift(e))
    }
    fn iter(&self) -> Box<dyn Iterator<Item = &Entry<K, V, Self>> + '_> {
        Box::new(self.0.iter())
    }
}

/// Build a G-node, or collapse to `right` when there are no items.
pub fn node<K, V, S: Items<K, V>>(rank: u32, items: S, right: Tree<K, V, S>) -> Tree<K, V, S> {
    let weight = items.weight();
    if weight == 0 {
        return right;
    }
    Some(Arc::new(Node {
        rank,
        size: weight + size(&right),
        items,
        right,
        _kv: PhantomData,
    }))
}

/// A tree holding a single entry, built from an empty inner set.
pub fn single<K, V, S: Items<K, V>>(key: K, value: V, rank: u32, empty: &S) -> Tree<K, V, S> {
    node(
        rank,
        empty.unshift(Entry {
            key,
            value: Value::Own(value),
            left: None,
        }),
        None,
    )
}

/// Split a tree into the keys before a position, the entry at it (if any), and
/// the keys after it. `at` compares an entry to the position; see [`Items`].
#[allow(clippy::type_complexity)]
pub fn unzip<K, V, S: Items<K, V>>(
    t: &Tree<K, V, S>,
    at: &impl Fn(&Entry<K, V, S>) -> Ordering,
) -> (Tree<K, V, S>, Option<Entry<K, V, S>>, Tree<K, V, S>) {
    let Some(n) = t else {
        return (None, None, None);
    };
    let (lo, e, hi) = n.items.split(at);
    if let Some(e) = e {
        // Found in this node: its left subtree ends the left result.
        let left = e.left.clone();
        return (
            node(n.rank, lo, left),
            Some(e),
            node(n.rank, hi, n.right.clone()),
        );
    }
    if hi.weight() == 0 {
        // Every key here precedes the position: continue in the right subtree.
        let (l, hit, r) = unzip(&n.right, at);
        return (node(n.rank, lo, l), hit, r);
    }
    // The position lies in the left subtree of the first greater entry.
    let (first, rest) = hi.shift();
    let (l, hit, r) = unzip(&first.left, at);
    let right = node(
        n.rank,
        rest.unshift(Entry { left: r, ..first }),
        n.right.clone(),
    );
    (node(n.rank, lo, l), hit, right)
}

/// Join two trees; every key of `l` must precede every key of `r`.
pub fn zip<K, V, S: Items<K, V>>(l: &Tree<K, V, S>, r: &Tree<K, V, S>) -> Tree<K, V, S> {
    let (Some(ln), Some(rn)) = (l, r) else {
        return l.clone().or_else(|| r.clone());
    };
    // l has the higher rank: r belongs somewhere down l's right spine.
    if ln.rank > rn.rank {
        return node(ln.rank, ln.items.clone(), zip(&ln.right, r));
    }
    // Otherwise (part of) l belongs in the leftmost subtree of r.
    let (first, rest) = rn.items.shift();
    if ln.rank < rn.rank {
        let left = zip(l, &first.left);
        return node(
            rn.rank,
            rest.unshift(Entry { left, ..first }),
            rn.right.clone(),
        );
    }
    // Equal ranks: the two nodes merge into one.
    let left = zip(&ln.right, &first.left);
    node(
        rn.rank,
        ln.items.join(&rest.unshift(Entry { left, ..first })),
        rn.right.clone(),
    )
}

/// Insert a single-entry tree, replacing any entry at the same position.
pub fn insert<K, V, S: Items<K, V>>(
    t: &Tree<K, V, S>,
    at: &impl Fn(&Entry<K, V, S>) -> Ordering,
    one: &Tree<K, V, S>,
) -> Tree<K, V, S> {
    let (l, _, r) = unzip(t, at);
    zip(&zip(&l, one), &r)
}

/// Remove the entry at a position, if any.
pub fn remove<K, V, S: Items<K, V>>(
    t: &Tree<K, V, S>,
    at: &impl Fn(&Entry<K, V, S>) -> Ordering,
) -> Tree<K, V, S> {
    let (l, _, r) = unzip(t, at);
    zip(&l, &r)
}

/// The first entry at or after a position, without allocating.
pub fn find<'a, K, V, S: Items<K, V>>(
    mut t: &'a Tree<K, V, S>,
    at: &impl Fn(&Entry<K, V, S>) -> Ordering,
) -> Option<&'a Entry<K, V, S>> {
    let mut best = None;
    while let Some(n) = t {
        match n.items.find(at) {
            None => t = &n.right,
            Some(e) if at(e) == Ordering::Equal => return Some(e),
            Some(e) => (best, t) = (Some(e), &e.left),
        }
    }
    best
}

/// The entries of a tree, in order.
pub fn entries<K, V, S: Items<K, V>>(t: &Tree<K, V, S>) -> Entries<'_, K, V, S> {
    let mut it = Entries { stack: Vec::new() };
    it.push(t);
    it
}

/// In-order iterator over a tree's entries.
pub struct Entries<'a, K, V, S> {
    stack: Vec<Frame<'a, K, V, S>>,
}

struct Frame<'a, K, V, S> {
    items: Box<dyn Iterator<Item = &'a Entry<K, V, S>> + 'a>,
    right: &'a Tree<K, V, S>,
    /// An entry whose left subtree is being visited; yielded once it is done.
    pending: Option<&'a Entry<K, V, S>>,
}

impl<'a, K, V, S: Items<K, V>> Entries<'a, K, V, S> {
    fn push(&mut self, t: &'a Tree<K, V, S>) {
        if let Some(n) = t {
            self.stack.push(Frame {
                items: n.items.iter(),
                right: &n.right,
                pending: None,
            });
        }
    }
}

impl<'a, K, V, S: Items<K, V>> Iterator for Entries<'a, K, V, S> {
    type Item = &'a Entry<K, V, S>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let i = self.stack.len().checked_sub(1)?;
            if let Some(e) = self.stack[i].pending.take() {
                return Some(e);
            }
            match self.stack[i].items.next() {
                Some(e) => {
                    self.stack[i].pending = Some(e);
                    self.push(&e.left);
                }
                None => {
                    let right = self.stack[i].right;
                    self.stack.pop();
                    self.push(right);
                }
            }
        }
    }
}
