#![doc = include_str!("../README.md")]

pub mod binary;
pub mod gk;
pub mod list;
pub mod map;
pub mod rank;
pub mod set;
pub mod tree;

pub use gk::{Dimension, GkItems, gk, gk_by};
pub use list::GList;
pub use map::{GMap, Options};
pub use rank::{Rank, hash, hash_utf16, hashed, hashed_by, random};
pub use set::GSet;
pub use tree::{ArrayItems, Entry, Items, Node, Tree, Value};
