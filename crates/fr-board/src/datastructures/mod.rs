//! The generic data structures the board model and the router are built on.
//!
//! Java: `datastructures/{ShapeTree,MinAreaTree,TimeLimit,Stoppable,ArrayStack}.java`.
//!
//! Java splits the search tree into an abstract `ShapeTree` and its single concrete subclass
//! `MinAreaTree`; `MinAreaTree` is the only implementation in the tree (`ShapeSearchTree`
//! extends it), so the port merges the pair into one struct, [`ShapeTree`], and marks the
//! Java-side abstraction points in the source.

pub mod shape_tree;
pub mod time_limit;

pub use shape_tree::{LeafId, Node, NodeId, ShapeTree, TreeEntry};
pub use time_limit::{StopCheck, TimeLimit};
