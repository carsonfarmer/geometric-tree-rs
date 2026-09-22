//! A G-tree used as a sequence: positions replace keys, and node sizes guide
//! the search. Ranks are random, since there is no key to hash.

use std::cmp::Ordering;
use std::ops::{Bound, RangeBounds};
use std::sync::Arc;

use crate::rank::{Rank, random};
use crate::tree::{ArrayItems, Entry, Items, Tree, entries, node, single, size, zip};

pub type Seq<T> = Tree<T, (), ArrayItems<T, ()>>;

/// The entry at index `i`.
pub fn at<T: Clone>(t: &Seq<T>, mut i: usize) -> Option<&Entry<T, (), ArrayItems<T, ()>>> {
    let n = t.as_ref()?;
    for e in n.items.iter() {
        let left = size(&e.left);
        if i < left {
            return at(&e.left, i);
        }
        if i == left {
            return Some(e);
        }
        i -= left + 1;
    }
    at(&n.right, i)
}

/// Split a sequence into its first `i` elements and the rest.
pub fn split_at<T: Clone>(t: &Seq<T>, mut i: usize) -> (Seq<T>, Seq<T>) {
    let Some(n) = t else { return (None, None) };
    if i == 0 {
        return (None, t.clone());
    }
    if i >= n.size {
        return (t.clone(), None);
    }
    for e in n.items.iter() {
        let left = size(&e.left);
        if i <= left {
            // The cut falls inside (or right after) e's left subtree.
            let (l, r) = split_at(&e.left, i);
            let (lo, _, hi) = n.items.split(&|x: &Entry<T, (), _>| {
                if std::ptr::eq(x, e) {
                    Ordering::Equal
                } else {
                    Ordering::Less
                }
            });
            let right = node(
                n.rank,
                hi.unshift(Entry {
                    left: r,
                    ..e.clone()
                }),
                n.right.clone(),
            );
            return (node(n.rank, lo, l), right);
        }
        i -= left + 1;
    }
    let (l, r) = split_at(&n.right, i);
    (node(n.rank, n.items.clone(), l), r)
}

/// An immutable sequence with logarithmic positional updates.
pub struct GList<T> {
    pub root: Seq<T>,
    rank: Rank<T>,
}

impl<T> Clone for GList<T> {
    fn clone(&self) -> Self {
        GList {
            root: self.root.clone(),
            rank: self.rank.clone(),
        }
    }
}

impl<T: Clone + 'static> Default for GList<T> {
    fn default() -> Self {
        Self::with_k(8)
    }
}

impl<T: Clone + 'static> GList<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// `k` is the expected number of elements per node.
    pub fn with_k(k: u32) -> Self {
        GList {
            root: None,
            rank: random(k),
        }
    }
}

impl<T: Clone> GList<T> {
    pub fn len(&self) -> usize {
        size(&self.root)
    }

    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    fn with(&self, root: Seq<T>) -> Self {
        GList {
            root,
            rank: self.rank.clone(),
        }
    }

    fn one(&self, x: T) -> Seq<T> {
        single(x.clone(), (), (self.rank)(&x, 0), &ArrayItems::default())
    }

    /// The element at index `i`.
    pub fn get(&self, i: usize) -> Option<&T> {
        at(&self.root, i).map(|e| &e.key)
    }

    pub fn push_back(&self, x: T) -> Self {
        self.with(zip(&self.root, &self.one(x)))
    }

    pub fn push_front(&self, x: T) -> Self {
        self.with(zip(&self.one(x), &self.root))
    }

    /// Insert `x` so that it ends up at index `i`.
    pub fn insert(&self, i: usize, x: T) -> Self {
        let (l, r) = split_at(&self.root, i);
        self.with(zip(&zip(&l, &self.one(x)), &r))
    }

    /// Remove the element at index `i`, if any.
    pub fn remove(&self, i: usize) -> Self {
        let (l, r) = split_at(&self.root, i);
        self.with(zip(&l, &split_at(&r, 1).1))
    }

    /// The elements in `range`, as a new list.
    pub fn slice(&self, range: impl RangeBounds<usize>) -> Self {
        let n = self.len();
        let start = match range.start_bound() {
            Bound::Included(&i) => i,
            Bound::Excluded(&i) => i + 1,
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(&i) => i + 1,
            Bound::Excluded(&i) => i,
            Bound::Unbounded => n,
        };
        let (head, _) = split_at(&self.root, end.min(n));
        self.with(split_at(&head, start.min(n)).1)
    }

    pub fn concat(&self, other: &Self) -> Self {
        self.with(zip(&self.root, &other.root))
    }

    /// Split into the first `i` elements and the rest.
    pub fn split_at(&self, i: usize) -> (Self, Self) {
        let (l, r) = split_at(&self.root, i);
        (self.with(l), self.with(r))
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        entries(&self.root).map(|e| &e.key)
    }
}

impl<T: Clone + 'static> FromIterator<T> for GList<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut list = GList::new();
        for x in iter {
            list = list.push_back(x);
        }
        list
    }
}

impl<T: Clone + 'static> From<Vec<T>> for GList<T> {
    fn from(v: Vec<T>) -> Self {
        v.into_iter().collect()
    }
}

/// Hold a shared rank function so lists can be built from any iterator.
impl<T> GList<T> {
    pub fn rank(&self) -> &Rank<T> {
        &self.rank
    }
}

#[allow(dead_code)]
fn _assert_arc<T>(_: &Arc<T>) {}
