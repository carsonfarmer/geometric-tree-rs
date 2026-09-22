//! The classic binary zip tree (Tarjan, Levy and Timmel, 2021), kept as the
//! reference that [`crate::tree`] generalises: a G-tree with k = 2 is this
//! tree with every run of equal-rank right children folded into one node. The
//! tests check that correspondence. Read the two modules side by side.

use std::cmp::Ordering;
use std::sync::Arc;

/// A node: key, value, rank, and the subtrees of smaller and greater keys.
pub struct Node<K, V> {
    pub key: K,
    pub value: V,
    pub rank: u32,
    pub left: Tree<K, V>,
    pub right: Tree<K, V>,
    pub size: usize,
}

/// A possibly empty zip tree.
pub type Tree<K, V> = Option<Arc<Node<K, V>>>;

/// Number of keys in a tree.
pub fn size<K, V>(t: &Tree<K, V>) -> usize {
    t.as_ref().map_or(0, |n| n.size)
}

/// Build a node, computing its size.
pub fn node<K, V>(key: K, value: V, rank: u32, left: Tree<K, V>, right: Tree<K, V>) -> Tree<K, V> {
    let size = size(&left) + size(&right) + 1;
    Some(Arc::new(Node {
        key,
        value,
        rank,
        left,
        right,
        size,
    }))
}

/// A node with new subtrees and the same key, value and rank.
fn with<K: Clone, V: Clone>(n: &Node<K, V>, left: Tree<K, V>, right: Tree<K, V>) -> Tree<K, V> {
    node(n.key.clone(), n.value.clone(), n.rank, left, right)
}

/// A tree holding a single key.
pub fn single<K, V>(key: K, value: V, rank: u32) -> Tree<K, V> {
    node(key, value, rank, None, None)
}

/// Split a tree into the keys before a position, the node at it (if any), and
/// the keys after it. `at` compares a node to the position, as in the G-tree.
#[allow(clippy::type_complexity)]
pub fn unzip<K: Clone, V: Clone>(
    t: &Tree<K, V>,
    at: &impl Fn(&Node<K, V>) -> Ordering,
) -> (Tree<K, V>, Option<Arc<Node<K, V>>>, Tree<K, V>) {
    let Some(n) = t else {
        return (None, None, None);
    };
    match at(n) {
        Ordering::Equal => (n.left.clone(), Some(n.clone()), n.right.clone()),
        // n is after the position: n and its right subtree go right.
        Ordering::Greater => {
            let (l, hit, r) = unzip(&n.left, at);
            (l, hit, with(n, r, n.right.clone()))
        }
        Ordering::Less => {
            let (l, hit, r) = unzip(&n.right, at);
            (with(n, n.left.clone(), l), hit, r)
        }
    }
}

/// Join two trees; every key of `l` must precede every key of `r`.
pub fn zip<K: Clone, V: Clone>(l: &Tree<K, V>, r: &Tree<K, V>) -> Tree<K, V> {
    let (Some(ln), Some(rn)) = (l, r) else {
        return l.clone().or_else(|| r.clone());
    };
    // Ties go left, so equal ranks chain down a right spine.
    if ln.rank >= rn.rank {
        with(ln, ln.left.clone(), zip(&ln.right, r))
    } else {
        with(rn, zip(l, &rn.left), rn.right.clone())
    }
}

/// Insert a single-node tree, replacing any node at the same position.
pub fn insert<K: Clone, V: Clone>(
    t: &Tree<K, V>,
    at: &impl Fn(&Node<K, V>) -> Ordering,
    one: &Tree<K, V>,
) -> Tree<K, V> {
    let (l, _, r) = unzip(t, at);
    zip(&zip(&l, one), &r)
}

/// Remove the node at a position, if any.
pub fn remove<K: Clone, V: Clone>(
    t: &Tree<K, V>,
    at: &impl Fn(&Node<K, V>) -> Ordering,
) -> Tree<K, V> {
    let (l, _, r) = unzip(t, at);
    zip(&l, &r)
}

/// The first node at or after a position, without allocating.
pub fn find<'a, K, V>(
    mut t: &'a Tree<K, V>,
    at: &impl Fn(&Node<K, V>) -> Ordering,
) -> Option<&'a Node<K, V>> {
    let mut best = None;
    while let Some(n) = t {
        match at(n) {
            Ordering::Equal => return Some(n),
            Ordering::Less => t = &n.right,
            Ordering::Greater => (best, t) = (Some(&**n), &n.left),
        }
    }
    best
}

/// The nodes of a tree, in key order.
pub fn nodes<K, V>(t: &Tree<K, V>) -> impl Iterator<Item = &Node<K, V>> {
    let mut stack = Vec::new();
    let mut t = t;
    std::iter::from_fn(move || {
        while let Some(n) = t {
            stack.push(&**n);
            t = &n.left;
        }
        let n = stack.pop()?;
        t = &n.right;
        Some(n)
    })
}
