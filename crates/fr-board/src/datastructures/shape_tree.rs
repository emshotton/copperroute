//! Port of `datastructures/ShapeTree.java` + `datastructures/MinAreaTree.java`: the binary
//! search tree over plane shapes that the whole router queries.
//!
//! # Shape of the port
//!
//! Java has an abstract `ShapeTree` (arena-free, `TreeNode`/`InnerNode`/`Leaf` objects linked
//! by references) with exactly one concrete subclass, `MinAreaTree`, which supplies the
//! insertion heuristic, the removal, and `overlaps`. `ShapeSearchTree` (Task 8) extends
//! `MinAreaTree`, not `ShapeTree`, so the abstraction has no second implementation and the port
//! merges the two classes into this single [`ShapeTree`] struct. Method docs name the Java
//! class and line each piece comes from.
//!
//! Java's nodes are heap objects reclaimed by the garbage collector and compared by reference
//! identity. Here they live in an arena, `nodes: Vec<Node<O>>`, addressed by [`NodeId`]; a
//! removed node's slot goes on a free list and is reused. Reference identity becomes
//! index-plus-generation equality: every handle carries the generation its slot had when the
//! handle was minted, so a handle to a removed node stays distinguishable from a handle to
//! whatever now occupies that slot (see [`NodeId`], and [`ShapeTree::remove_leaf`] for the one
//! place the port deliberately parts company with Java).
//!
//! # What `ShapeSearchTree` (Task 10) needs from this type
//!
//! Java's search tree does three things to a live tree that are not "insert" or "remove", and
//! each has a method here:
//!
//! * it **re-keys** leaves in place, assigning `leaf.object` / `leaf.shapeIndexInObject` while
//!   the leaf stays exactly where it is — [`ShapeTree::set_leaf_entry`];
//! * it lets the **tree** turn an object's tree shapes into bounding shapes, and tolerates a
//!   shape that has no bound — [`ShapeTree::insert_tiles`], returning Java's nullable `Leaf[]`
//!   as `Vec<Option<LeafId>>`;
//! * it removes **holed** entry arrays, where some elements were moved elsewhere and nulled —
//!   [`ShapeTree::remove_opt`].
//!
//! Each carries an `obligation:` marker naming what Task 10 must do with it.
//!
//! # Determinism
//!
//! The tree must be a pure function of the insertion order, because the router's results depend
//! on the order `overlaps` reports objects in. Two things guarantee that:
//!
//! * The insertion heuristic (`ShapeTree::position_locate`) is deterministic, ties included:
//!   `firstAreaIncrease <= secondAreaIncrease` (MinAreaTree.java:109) always picks the *first*
//!   child on an exact tie. It reads nothing but the shapes already in the tree.
//! * `overlaps` returns a [`BTreeSet`] of [`TreeEntry`], whose derived [`Ord`] is
//!   `(object, shape_index)` lexicographically. That is exactly Java's `Leaf.compareTo`
//!   (ShapeTree.java:216-223) — `object.compareTo(other.object)`, then
//!   `shapeIndexInObject - other.shapeIndexInObject` — which is the comparator Java's
//!   `TreeSet<Leaf>` (MinAreaTree.java:26) uses. So the Rust set iterates in the same order the
//!   Java set does, independently of the DFS order the nodes were visited in.
//!
//! The arena adds index state that Java does not have, but it too is a pure function of the
//! operation sequence: [`ShapeTree::insert_leaf`] allocates in a fixed order, and the free list
//! is LIFO. Identical operation sequences therefore produce byte-identical `Debug` output —
//! see `crates/fr-board/tests/min_area_tree.rs`.

use std::collections::BTreeSet;

use fr_geometry::bounding_directions::ShapeBoundingDirections;
use fr_geometry::regular_tile_shape::RegularTileShape;
use fr_geometry::tile_shape::TileShape;

/// A handle to a node in a [`ShapeTree`]'s arena — the port's replacement for Java's `TreeNode`
/// reference.
///
/// # Why there is a generation counter
///
/// Java's node handles are object references: once a node is removed nothing else can ever be
/// that object, and using a stale reference throws a `NullPointerException` on the first field
/// access. An arena index alone does not have that property — a freed slot is handed straight
/// back out by the next allocation, so a stale index would silently *alias a live node*.
///
/// That is not hypothetical. `ShapeSearchTree.changeItemShape`
/// (`board/searchtree/ShapeSearchTree.java:856,867`) calls `removeLeaf(oldEntries[shapeIndex])`
/// and then `insert(item, shapeIndex)` — a free immediately followed by an allocation, which a
/// LIFO free list satisfies from the very slot just released. Any handle still pointing at the
/// old leaf would now resolve to the new one, with no error anywhere.
///
/// So every handle carries the generation its slot had when the handle was minted, the slot's
/// generation is bumped when it is freed, and every access asserts the two match. The port is
/// then *louder* than Java (a panic naming the stale handle rather than an NPE at some later
/// field access) and never silently wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId {
    index: u32,
    generation: u32,
}

impl NodeId {
    /// The raw arena index.
    pub fn index(self) -> usize {
        self.index as usize
    }

    /// The arena generation this handle was minted for. No Java counterpart.
    pub fn generation(self) -> u32 {
        self.generation
    }
}

/// A handle to a *leaf* node in a [`ShapeTree`]'s arena — the port's replacement for Java's
/// `ShapeTree.Leaf` reference, which board items hold in `setSearchTreeEntries`
/// (ShapeTree.java:150-154) so they can delete their own tree entries later.
///
/// Carries a generation counter for the reason spelled out on [`NodeId`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LeafId {
    index: u32,
    generation: u32,
}

impl LeafId {
    /// The raw arena index.
    pub fn index(self) -> usize {
        self.index as usize
    }

    /// The arena generation this handle was minted for. No Java counterpart.
    pub fn generation(self) -> u32 {
        self.generation
    }

    /// Widens this leaf handle to a plain node handle.
    pub fn node(self) -> NodeId {
        NodeId {
            index: self.index,
            generation: self.generation,
        }
    }

    fn from_node(id: NodeId) -> Self {
        Self {
            index: id.index,
            generation: id.generation,
        }
    }
}

/// A node of a [`ShapeTree`]: Java's `ShapeTree.TreeNode` hierarchy
/// (ShapeTree.java:170-235) flattened into one enum.
///
/// Java's `TreeNode` holds `boundingShape` + `parent` and is specialised by `InnerNode`
/// (two children) and `Leaf` (an object and a shape index). The third variant, `Free`, has no
/// Java counterpart: it is the arena's free-list entry, standing in for a node the garbage
/// collector would have reclaimed.
///
/// not ported: `InnerNode` / `Leaf` / `TreeEntry` constructors (ShapeTree.java:187-193,
/// 207-214, 163-167) — the port builds these as enum variants and struct literals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Node<O> {
    /// Java `ShapeTree.InnerNode` (ShapeTree.java:181-194): a fork to two children.
    Inner {
        /// Java `TreeNode.boundingShape` (ShapeTree.java:175).
        bounds: RegularTileShape,
        /// Java `TreeNode.parent` (ShapeTree.java:176). `None` at the root.
        parent: Option<NodeId>,
        /// Java `InnerNode.firstChild` (ShapeTree.java:184).
        first_child: NodeId,
        /// Java `InnerNode.secondChild` (ShapeTree.java:185).
        second_child: NodeId,
        /// The slot's generation. No Java counterpart — see [`NodeId`].
        generation: u32,
    },
    /// Java `ShapeTree.Leaf` (ShapeTree.java:198-235): where the geometry is stored.
    Leaf {
        /// Java `TreeNode.boundingShape` (ShapeTree.java:175).
        bounds: RegularTileShape,
        /// Java `TreeNode.parent` (ShapeTree.java:176). `None` when the leaf is the root.
        parent: Option<NodeId>,
        /// Java `Leaf.object` (ShapeTree.java:199). Rewritten in place by
        /// [`ShapeTree::set_leaf_entry`], exactly as `ShapeSearchTree` does.
        object: O,
        /// Java `Leaf.shapeIndexInObject` (ShapeTree.java:202). Rewritten in place by
        /// [`ShapeTree::set_leaf_entry`].
        shape_index: usize,
        /// The slot's generation. No Java counterpart — see [`NodeId`].
        generation: u32,
    },
    /// A free arena slot. No Java counterpart — see the type docs.
    Free {
        /// The next free slot, forming a LIFO free list.
        next_free: Option<NodeId>,
        /// The slot's generation, already bumped past the handles that used to name it.
        generation: u32,
    },
}

impl<O> Node<O> {
    /// The generation stamped on this slot, whichever variant it is.
    fn generation(&self) -> u32 {
        match self {
            Node::Inner { generation, .. }
            | Node::Leaf { generation, .. }
            | Node::Free { generation, .. } => *generation,
        }
    }
}

/// Information about a single object stored in a tree: Java `ShapeTree.TreeEntry`
/// (ShapeTree.java:157-168), used as the element type of [`ShapeTree::overlaps`]'s result set.
///
/// renamed: `ShapeTree.Leaf.compareTo` (ShapeTree.java:216-223) -> the derived [`Ord`] on [`TreeEntry`], which compares `object` first and `shape_index` second.
///
/// The derived [`Ord`] compares `object` first and `shape_index` second, reproducing Java's
/// `Leaf.compareTo` (ShapeTree.java:216-223) — the comparator behind `MinAreaTree.overlaps`'s
/// `TreeSet<Leaf>`. Java's second half is the `int` subtraction
/// `shapeIndexInObject - other.shapeIndexInObject`; shape indices are small array positions, so
/// it cannot overflow in practice, and `usize` ordering agrees with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TreeEntry<O> {
    /// Java `TreeEntry.object` (ShapeTree.java:160).
    pub object: O,
    /// Java `TreeEntry.shapeIndexInObject` (ShapeTree.java:161), renamed per the task brief.
    pub shape_index: usize,
}

/// A binary search tree for shapes in the plane, storing the shapes in its leaves
/// (ShapeTree.java:13-236 + MinAreaTree.java:17-180).
///
/// `O` is the stored object's key. The task brief asks for `O: Copy + Ord + Hash`; the bounds
/// actually needed are `Copy + Ord` (nothing here hashes), and they are declared on the `impl`
/// blocks rather than on the struct so that `Debug`/`Clone` do not drag them in. The board uses
/// [`crate::ids::TreeObject`], whose `Ord` reproduces Java's `SearchTreeObject` ordering.
///
/// not ported: `ShapeTree.statistics` (ShapeTree.java:111-136) — its whole body is an
/// `FRLogger.info` call, and `fr-board` must not depend on `tracing`
/// (global-constraints.md). Its one input, `distanceToRoot`, is ported below.
///
/// not ported: `ShapeTree.Storable` (ShapeTree.java:138-155) with its `treeShapeCount`,
/// `getTreeShape` and `setSearchTreeEntries` — the port inverts that call direction: the caller
/// hands its shapes to [`ShapeTree::insert_tiles`] and keeps the returned entry vector itself,
/// instead of the tree calling back into the object. The tree still owns the
/// `boundingShape(boundingDirections)` step (ShapeTree.java:51) and still produces Java's
/// nullable `Leaf[]`, as `Vec<Option<LeafId>>`.
///
/// not ported: `ArrayStack` (ArrayStack.java) with its `push`, `pop` and `reset` — replaced by
/// a `Vec` local to [`ShapeTree::overlaps`]. Java constructs it as `new ArrayStack<>(10000)`
/// (MinAreaTree.java:30), but 10000 is only an *initial capacity*: `ArrayStack.push`
/// (ArrayStack.java:24-33) calls `reallocate` and quadruples the backing array when it
/// overflows, so nothing is ever dropped and there is no cap to reproduce. A plain `Vec` has
/// the same unbounded-growth behaviour; the port does not pre-reserve 10000 slots per query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapeTree<O> {
    /// Java `ShapeTree.boundingDirections` (ShapeTree.java:16): the fixed directions used to
    /// compute bounding shapes for this tree.
    bounding_directions: ShapeBoundingDirections,
    /// The node arena. Java has no counterpart — its nodes are individually heap-allocated.
    nodes: Vec<Node<O>>,
    /// Java `ShapeTree.root` (ShapeTree.java:19), initially `null`.
    root: Option<NodeId>,
    /// Java `ShapeTree.leafCount` (ShapeTree.java:22).
    leaf_count: usize,
    /// Head of the arena free list. No Java counterpart.
    first_free: Option<NodeId>,
}

impl<O> ShapeTree<O> {
    /// Creates a new instance (ShapeTree.java:24-29, MinAreaTree.java:20-22).
    pub fn new(bounding_directions: ShapeBoundingDirections) -> Self {
        Self {
            bounding_directions,
            nodes: Vec::new(),
            root: None,
            leaf_count: 0,
            first_free: None,
        }
    }

    /// The fixed directions used for bounding shapes in this tree (Java's `boundingDirections`
    /// field, ShapeTree.java:16 — `protected`, with no accessor).
    pub fn bounding_directions(&self) -> ShapeBoundingDirections {
        self.bounding_directions
    }

    /// The number of entries stored in the tree (ShapeTree.java:106-108).
    ///
    /// renamed: `size` -> `leaf_count` (task brief), matching the Java field it returns.
    pub fn leaf_count(&self) -> usize {
        self.leaf_count
    }

    /// True when the tree holds no leaves.
    pub fn is_empty(&self) -> bool {
        self.leaf_count == 0
    }

    /// The root node, or `None` for an empty tree (Java `ShapeTree.root`, ShapeTree.java:19 —
    /// a `protected` field read directly by subclasses).
    pub fn root(&self) -> Option<NodeId> {
        self.root
    }

    /// Borrows an arena node, checking the handle first.
    ///
    /// Panics if `id` is out of range or names a slot whose generation has moved on — i.e. a
    /// handle to a node that has since been removed, whether or not its slot has been reused.
    /// This is the port's stand-in for Java throwing a `NullPointerException` off a stale
    /// `TreeNode` reference, and it is deliberately an `assert!`, not a `debug_assert!`: a
    /// stale handle must fail the same way in release builds, because in release it would
    /// otherwise read a *live but different* node (see [`NodeId`]).
    pub fn node(&self, id: NodeId) -> &Node<O> {
        self.resolve(id)
    }

    fn resolve(&self, id: NodeId) -> &Node<O> {
        let node = self
            .nodes
            .get(id.index())
            .unwrap_or_else(|| panic!("ShapeTree: node index {} is out of range", id.index()));
        assert!(
            node.generation() == id.generation && !matches!(node, Node::Free { .. }),
            "ShapeTree: stale handle to node {} (handle generation {}, slot generation {}) — \
             the node was removed",
            id.index(),
            id.generation,
            node.generation()
        );
        node
    }

    fn resolve_mut(&mut self, id: NodeId) -> &mut Node<O> {
        let len = self.nodes.len();
        let node = self.nodes.get_mut(id.index()).unwrap_or_else(|| {
            panic!(
                "ShapeTree: node index {} is out of range (arena has {len} slots)",
                id.index()
            )
        });
        assert!(
            node.generation() == id.generation && !matches!(node, Node::Free { .. }),
            "ShapeTree: stale handle to node {} (handle generation {}, slot generation {}) — \
             the node was removed",
            id.index(),
            id.generation,
            node.generation()
        );
        node
    }

    /// True when `id` still names the live node it was minted for. No Java counterpart; Java
    /// cannot ask this question of a `Leaf` reference at all.
    pub fn is_live(&self, id: NodeId) -> bool {
        self.nodes.get(id.index()).is_some_and(|node| {
            node.generation() == id.generation && !matches!(node, Node::Free { .. })
        })
    }

    /// Total arena slots, free ones included. No Java counterpart; exposed so tests can show
    /// that removal really does recycle slots instead of leaking them.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// The number of arena slots currently on the free list. No Java counterpart.
    pub fn free_slot_count(&self) -> usize {
        let mut count = 0;
        let mut next = self.first_free;
        while let Some(id) = next {
            count += 1;
            next = match &self.nodes[id.index()] {
                Node::Free { next_free, .. } => *next_free,
                _ => unreachable!("the free list only links Node::Free slots"),
            };
        }
        count
    }

    /// Java's `objectShape.boundingShape(boundingDirections)` step of
    /// `ShapeTree.insert(Storable, int)` (ShapeTree.java:51), exposed because the port moves
    /// shape retrieval to the caller (see [`ShapeTree::insert`]).
    ///
    /// `None` mirrors Java's `boundingShape == null` branch (ShapeTree.java:52-55), reachable
    /// only for an unbounded `Simplex` under 45-degree directions; Java logs a warning and
    /// skips the shape, which here is the caller's decision.
    pub fn bounding_shape(&self, shape: &TileShape) -> Option<RegularTileShape> {
        self.bounding_directions.bounds_tile(shape)
    }

    fn bounds_of(&self, id: NodeId) -> RegularTileShape {
        match self.resolve(id) {
            Node::Inner { bounds, .. } | Node::Leaf { bounds, .. } => *bounds,
            Node::Free { .. } => unreachable!("resolve rejects freed slots"),
        }
    }

    fn set_bounds(&mut self, id: NodeId, new_bounds: RegularTileShape) {
        match self.resolve_mut(id) {
            Node::Inner { bounds, .. } | Node::Leaf { bounds, .. } => *bounds = new_bounds,
            Node::Free { .. } => unreachable!("resolve_mut rejects freed slots"),
        }
    }

    fn parent_of(&self, id: NodeId) -> Option<NodeId> {
        match self.resolve(id) {
            Node::Inner { parent, .. } | Node::Leaf { parent, .. } => *parent,
            Node::Free { .. } => unreachable!("resolve rejects freed slots"),
        }
    }

    fn set_parent(&mut self, id: NodeId, new_parent: Option<NodeId>) {
        match self.resolve_mut(id) {
            Node::Inner { parent, .. } | Node::Leaf { parent, .. } => *parent = new_parent,
            Node::Free { .. } => unreachable!("resolve_mut rejects freed slots"),
        }
    }

    /// The two children of an inner node; panics if `id` is not an inner node.
    fn children_of(&self, id: NodeId) -> (NodeId, NodeId) {
        match self.resolve(id) {
            Node::Inner {
                first_child,
                second_child,
                ..
            } => (*first_child, *second_child),
            _ => panic!("ShapeTree: node {} is not an inner node", id.index()),
        }
    }

    /// Replaces whichever child of the inner node `id` currently equals `old` with `new`.
    ///
    /// Returns false when neither child matches, which is Java's "parent inconsistent" /
    /// "grandParent inconsistent" branch (MinAreaTree.java:141-143, 156-158).
    fn replace_child(&mut self, id: NodeId, old: NodeId, new: NodeId) -> bool {
        match self.resolve_mut(id) {
            Node::Inner {
                first_child,
                second_child,
                ..
            } => {
                // Java tests `secondChild` first in `removeLeaf` and `firstChild` first in
                // `insert`; the two orders can only differ if both children were the same node,
                // which the tree never produces.
                if *second_child == old {
                    *second_child = new;
                    true
                } else if *first_child == old {
                    *first_child = new;
                    true
                } else {
                    false
                }
            }
            _ => panic!("ShapeTree: node {} is not an inner node", id.index()),
        }
    }

    /// Allocates an arena slot, reusing the head of the free list when there is one, and
    /// returns a handle stamped with the slot's current generation.
    ///
    /// `build` receives that generation so the node it produces carries the same stamp.
    fn alloc(&mut self, build: impl FnOnce(u32) -> Node<O>) -> NodeId {
        match self.first_free {
            Some(id) => {
                let slot = &self.nodes[id.index()];
                let generation = slot.generation();
                self.first_free = match slot {
                    Node::Free { next_free, .. } => *next_free,
                    _ => unreachable!("the free list only links Node::Free slots"),
                };
                debug_assert_eq!(
                    generation, id.generation,
                    "the free list handle must carry the slot's post-free generation"
                );
                self.nodes[id.index()] = build(generation);
                id
            }
            None => {
                let index = u32::try_from(self.nodes.len())
                    .expect("ShapeTree: more than u32::MAX arena slots");
                self.nodes.push(build(0));
                NodeId {
                    index,
                    generation: 0,
                }
            }
        }
    }

    /// Returns an arena slot to the free list, **bumping its generation** so that every handle
    /// minted for the old occupant is now stale. Stands in for Java dropping the last reference
    /// to a node and letting the garbage collector take it.
    fn free(&mut self, id: NodeId) {
        let generation = self.nodes[id.index()].generation().wrapping_add(1);
        self.nodes[id.index()] = Node::Free {
            next_free: self.first_free,
            generation,
        };
        self.first_free = Some(NodeId {
            index: id.index,
            generation,
        });
    }
}

impl<O: Copy + Ord> ShapeTree<O> {
    /// Inserts all shapes of `object` into the tree (ShapeTree.java:32-42), returning one
    /// [`LeafId`] per shape, in shape-index order.
    ///
    /// Java asks the object itself: `obj.treeShapeCount(this)` and then
    /// `obj.getTreeShape(this, i)` for each index, finally handing the leaf array back through
    /// `obj.setSearchTreeEntries(leafArr, this)` (ShapeTree.java:41). The port has no
    /// `Storable` back-channel — the caller supplies the shapes and keeps the returned ids —
    /// so `shapes.len()` is Java's `treeShapeCount` and an empty slice is Java's
    /// `shapeCount <= 0` early return (ShapeTree.java:33-35).
    ///
    /// This is the **low-level** variant: the shapes passed in are taken as the leaves'
    /// bounding shapes verbatim, so Java's two `null` returns (a missing tree shape,
    /// ShapeTree.java:47-49, and an unboundable shape, ShapeTree.java:52-55) are already
    /// resolved by the caller and the returned vector always has exactly `shapes.len()`
    /// entries. Prefer [`ShapeTree::insert_tiles`], which applies this tree's bounding
    /// directions itself and reproduces Java's nullable `Leaf[]` — see the obligations
    /// recorded there.
    ///
    /// Note also that Java's `insert(Storable)` early-returns at ShapeTree.java:33-35 for a
    /// zero-shape object *without* calling `setSearchTreeEntries`, so the object keeps its
    /// previous entry array; this method returns an empty `Vec` and the caller decides. See
    /// `docs/java-quirks.md`.
    pub fn insert(&mut self, object: O, shapes: &[RegularTileShape]) -> Vec<LeafId> {
        shapes
            .iter()
            .enumerate()
            .map(|(index, shape)| self.insert_leaf(object, index, *shape))
            .collect()
    }

    /// The faithful port of `ShapeTree.insert(Storable)` (ShapeTree.java:32-42): the **tree**
    /// applies its own bounding directions to each of the object's tree shapes, and the result
    /// has one slot per shape index, holding `None` wherever Java would have left a `null` in
    /// `leafArr`.
    ///
    /// Java's `insert(Storable, int)` (ShapeTree.java:45-60) returns `null` in two cases:
    ///
    /// * `object.getTreeShape(this, index) == null` (ShapeTree.java:46-49) — a shape the object
    ///   declines to supply. A `TileShape` is never null in Rust, so a caller that has to model
    ///   this must skip the index itself; see the `// obligation:` note below.
    /// * `objectShape.boundingShape(boundingDirections) == null` (ShapeTree.java:51-55) — the
    ///   shape has no bound in this tree's directions. That is exactly
    ///   [`ShapeTree::bounding_shape`] returning `None`, reachable for an unbounded `Simplex`
    ///   under 45-degree directions, and it is the case reproduced here: the slot stays `None`,
    ///   nothing is inserted, and the remaining shapes are still processed. Java also logs a
    ///   warning there, which `fr-board` drops (no `tracing`).
    ///
    /// obligation: Task 10 (`ShapeSearchTree`) must insert through **this** method, not through
    /// [`ShapeTree::insert`]. `insert` takes shapes that are already `RegularTileShape`s and
    /// stores them as given; a 45-degree tree handed `RegularTileShape::Box` bounds would
    /// silently store box bounds instead of octagons, because only `insert_tiles` performs
    /// Java's `boundingShape(boundingDirections)` step (ShapeTree.java:51). The object's stored
    /// entry array must be `Vec<Option<LeafId>>`, matching Java's nullable `Leaf[]`, and must be
    /// given back to [`ShapeTree::remove_opt`].
    ///
    /// obligation: Task 10 must **not** overwrite an object's stored entries when `shapes` is
    /// empty. Java returns at ShapeTree.java:33-35 *before* `setSearchTreeEntries`
    /// (ShapeTree.java:41), so a zero-shape insert leaves the object's previous entry array
    /// untouched; this method returns an empty `Vec`, and storing it would drop entries Java
    /// keeps. See the `docs/java-quirks.md` note on `insert`.
    pub fn insert_tiles(&mut self, object: O, shapes: &[TileShape]) -> Vec<Option<LeafId>> {
        shapes
            .iter()
            .enumerate()
            .map(|(index, shape)| {
                let bounds = self.bounding_shape(shape)?;
                Some(self.insert_leaf(object, index, bounds))
            })
            .collect()
    }

    /// [`ShapeTree::insert_tiles`] for an object whose tree shapes are themselves nullable —
    /// Java's *first* `null` return of `insert(Storable, int)`, `object.getTreeShape(this,
    /// index) == null` (ShapeTree.java:46-49).
    ///
    /// That branch is not defensive: `ShapeSearchTree.calculateTreeShapes(DrillItem)` writes
    /// `result[i] = null` (ShapeSearchTree.java:882, ShapeSearchTree45Degree.java:500,
    /// ShapeSearchTree90Degree.java:446) for every layer on which the drill item has no pad and
    /// the hole-clearance rule synthesises no obstacle either. The index keeps its slot in the
    /// returned vector — `DrillItem.shapeLayer(index)` is `firstLayer() + index`
    /// (DrillItem.java:147-154), so the numbering may not shift — but no leaf is created for it.
    pub fn insert_tiles_opt(
        &mut self,
        object: O,
        shapes: &[Option<TileShape>],
    ) -> Vec<Option<LeafId>> {
        shapes
            .iter()
            .enumerate()
            .map(|(index, shape)| {
                // ShapeTree.java:46-49.
                let shape = shape.as_ref()?;
                // ShapeTree.java:51-55.
                let bounds = self.bounding_shape(shape)?;
                Some(self.insert_leaf(object, index, bounds))
            })
            .collect()
    }

    /// Inserts one leaf (ShapeTree.java:45-60 for the leaf construction, MinAreaTree.java:51-87
    /// for the placement).
    ///
    /// `bounds` is the leaf's bounding shape; `shape_index` is Java's `shapeIndexInObject`.
    pub fn insert_leaf(
        &mut self,
        object: O,
        shape_index: usize,
        bounds: RegularTileShape,
    ) -> LeafId {
        // Java `new Leaf(object, index, null, boundingShape)` (ShapeTree.java:57), allocated
        // before `MinAreaTree.insert(Leaf)` runs.
        let leaf = self.alloc(|generation| Node::Leaf {
            bounds,
            parent: None,
            object,
            shape_index,
            generation,
        });

        if p7t14b_mat_ledger() {
            p7t14b_mat("ins", self.leaf_count, &bounds);
        }
        // MinAreaTree.java:52.
        self.leaf_count += 1;

        // Tree is empty - just insert the new leaf (MinAreaTree.java:54-58).
        let Some(root) = self.root else {
            self.root = Some(leaf);
            return LeafId::from_node(leaf);
        };

        // Non-empty tree - do a recursive location for leaf replacement (MinAreaTree.java:61).
        let to_replace = self.position_locate(root, bounds);

        // Construct a new node - whenever a leaf is added so is a new node
        // (MinAreaTree.java:63-67).
        let new_bounds = bounds.union(&self.bounds_of(to_replace));
        let current_parent = self.parent_of(to_replace);
        let new_node = self.alloc(|generation| Node::Inner {
            bounds: new_bounds,
            parent: current_parent,
            // Insert the children in any order (MinAreaTree.java:80-82): Java assigns
            // firstChild = leafToReplace, secondChild = leaf. That choice is load-bearing —
            // `positionLocate` breaks ties toward the first child — so the port keeps it.
            first_child: to_replace,
            second_child: leaf,
            generation,
        });

        // Replace the pointer from the parent to the leaf with our new node
        // (MinAreaTree.java:68-75).
        if let Some(parent) = current_parent {
            let replaced = self.replace_child(parent, to_replace, new_node);
            debug_assert!(replaced, "ShapeTree.insert_leaf: parent inconsistent");
        }
        self.set_parent(to_replace, Some(new_node));
        self.set_parent(leaf, Some(new_node));

        // MinAreaTree.java:84-86.
        if root == to_replace {
            self.root = Some(new_node);
        }
        LeafId::from_node(leaf)
    }

    /// Port of `MinAreaTree.positionLocate` (MinAreaTree.java:89-116).
    ///
    /// Walks down from `start` to the leaf the new leaf should be paired with, widening every
    /// inner node it passes to include `leaf_bounds` on the way (MinAreaTree.java:94-95) —
    /// which is why this takes `&mut self` and is called unconditionally, before the caller
    /// knows where the new leaf lands.
    ///
    /// At each fork it takes the child whose bounding shape grows *least in area* after the
    /// union with `leaf_bounds`. Java's test is `firstAreaIncrease <= secondAreaIncrease`
    /// (MinAreaTree.java:109), so an exact tie goes to the **first** child; the `<=` also means
    /// a NaN on either side (impossible for these shapes, but the comparison is `f64`) falls to
    /// the second child in Rust exactly as it does in Java, since both languages evaluate every
    /// NaN comparison to false.
    fn position_locate(&mut self, start: NodeId, leaf_bounds: RegularTileShape) -> NodeId {
        let mut node = start;
        loop {
            let (first_child, second_child) = match self.resolve(node) {
                Node::Leaf { .. } => return node,
                Node::Inner {
                    first_child,
                    second_child,
                    ..
                } => (*first_child, *second_child),
                Node::Free { .. } => unreachable!("resolve rejects freed slots"),
            };

            // MinAreaTree.java:94-95.
            let widened = leaf_bounds.union(&self.bounds_of(node));
            self.set_bounds(node, widened);

            // MinAreaTree.java:100-107.
            let first_shape = self.bounds_of(first_child);
            let first_area_increase = leaf_bounds.union(&first_shape).area() - first_shape.area();
            let second_shape = self.bounds_of(second_child);
            let second_area_increase =
                leaf_bounds.union(&second_shape).area() - second_shape.area();

            node = if first_area_increase <= second_area_increase {
                first_child
            } else {
                second_child
            };
        }
    }

    /// Removes all listed entries from the tree (ShapeTree.java:96-104).
    ///
    /// Java guards against a `null` array (ShapeTree.java:98-100); the Rust equivalent is an
    /// empty slice, handled by the loop itself.
    pub fn remove(&mut self, entries: &[LeafId]) {
        for entry in entries {
            self.remove_leaf(*entry);
        }
    }

    /// The faithful port of `ShapeTree.remove(Leaf[])` (ShapeTree.java:96-104) for the
    /// **holed** entry arrays Java actually passes it.
    ///
    /// `MinAreaTree.removeLeaf`'s `if (leaf == null) return;` (MinAreaTree.java:121-123) is
    /// load-bearing, not defensive: `ShapeSearchTree.reuseEntriesAfterCutout` writes `null`
    /// into the middle of a trace's entry array (`ShapeSearchTree.java:327,344`) after moving
    /// those leaves to the two new pieces, and `SearchTreeManager.remove`
    /// (`SearchTreeManager.java:53-57`) later hands that same holed array straight to
    /// `remove(Leaf[])`. Each `None` is skipped exactly as Java skips a `null`.
    ///
    /// obligation: Task 10 stores an object's tree entries as `Vec<Option<LeafId>>` and removes
    /// them through this method; [`ShapeTree::remove`] is only the convenience form for arrays
    /// that are known to be hole-free.
    pub fn remove_opt(&mut self, entries: &[Option<LeafId>]) {
        for entry in entries {
            self.remove_leaf_opt(*entry);
        }
    }

    /// `MinAreaTree.removeLeaf(Leaf)` including its `leaf == null` guard
    /// (MinAreaTree.java:121-123): `None` is a no-op.
    ///
    /// Java calls this with a possibly-`null` element directly, e.g.
    /// `ShapeSearchTree.changeEntries` at `ShapeSearchTree.java:143`.
    pub fn remove_leaf_opt(&mut self, leaf: Option<LeafId>) {
        if let Some(leaf) = leaf {
            self.remove_leaf(leaf);
        }
    }

    /// Removes one entry from this tree (MinAreaTree.java:120-179).
    ///
    /// The sibling of the removed leaf is relinked to the grandparent and the parent inner node
    /// disappears (MinAreaTree.java:145-159); the ancestors' bounding shapes are then
    /// recomputed upward, stopping as soon as one does not actually get smaller
    /// (MinAreaTree.java:167-178).
    ///
    /// # Deviation from Java
    ///
    /// Java bug (**not** reproduced, quirk #40's neighbour in `docs/java-quirks.md`):
    /// Java's `removeLeaf` accepts a leaf it has already removed: the earlier call nulled that
    /// leaf's `parent`, so the second call takes the `parent == null` branch
    /// (MinAreaTree.java:130-134), decrements `leafCount` again and sets `root = null` —
    /// silently discarding the whole tree. The arena cannot reproduce that without reproducing
    /// the corruption too, so a [`LeafId`] whose slot is free (or was reused) panics instead.
    /// Reproducing a silent, unrecoverable data loss would be worse than surfacing the bug.
    /// See `docs/java-quirks.md`.
    pub fn remove_leaf(&mut self, leaf: LeafId) {
        let leaf = leaf.node();
        // `resolve` rejects an out-of-range index, a freed slot and a stale generation; this
        // additionally rejects a handle that names a live *inner* node.
        assert!(
            matches!(self.resolve(leaf), Node::Leaf { .. }),
            "ShapeTree.remove_leaf: node {} is not a leaf",
            leaf.index()
        );

        // MinAreaTree.java:125-129: read the parent, then clear the leaf. The port frees the
        // arena slot where Java nulls the leaf's fields and lets the GC take it.
        if p7t14b_mat_ledger() {
            p7t14b_mat("rem", self.leaf_count, &self.bounds_of(leaf));
        }
        let parent = self.parent_of(leaf);
        self.free(leaf);
        self.leaf_count -= 1;

        // Tree gets empty (MinAreaTree.java:130-134).
        let Some(parent) = parent else {
            self.root = None;
            return;
        };

        // Find the other leaf of the parent (MinAreaTree.java:135-144). Java compares
        // `secondChild` first; "parent inconsistent" leaves `otherLeaf` null and Java then
        // throws an NPE one line later (MinAreaTree.java:147), so the port panics too.
        let (first_child, second_child) = self.children_of(parent);
        let other_leaf = if second_child == leaf {
            first_child
        } else if first_child == leaf {
            second_child
        } else {
            panic!("MinAreaTree.remove_leaf: parent inconsistent");
        };

        // Link the other leaf to the grandParent and remove the parent node
        // (MinAreaTree.java:145-159).
        let grand_parent = self.parent_of(parent);
        self.set_parent(other_leaf, grand_parent);
        match grand_parent {
            // Only one leaf left in the tree (MinAreaTree.java:148-150).
            None => self.root = Some(other_leaf),
            Some(grand_parent) => {
                let replaced = self.replace_child(grand_parent, parent, other_leaf);
                // MinAreaTree.java:156-158: Java logs "grandParent inconsistent" and carries
                // on with a dangling link; `fr-board` turns invariant-guard logs into
                // `debug_assert!` (global-constraints.md).
                debug_assert!(
                    replaced,
                    "MinAreaTree.remove_leaf: grandParent inconsistent"
                );
            }
        }
        // MinAreaTree.java:160-163.
        self.free(parent);

        // Recalculate the bounding shapes of the ancestors as long as it gets smaller after
        // removing leaf (MinAreaTree.java:165-178).
        let mut node_to_recalculate = grand_parent;
        while let Some(node) = node_to_recalculate {
            let (first_child, second_child) = self.children_of(node);
            let new_bounds = self
                .bounds_of(second_child)
                .union(&self.bounds_of(first_child));
            if new_bounds.contains(&self.bounds_of(node)) {
                // The new bounds are not smaller, no further recalculate necessary
                // (MinAreaTree.java:172-175).
                break;
            }
            self.set_bounds(node, new_bounds);
            node_to_recalculate = self.parent_of(node);
        }
    }

    /// Calculates the objects in this tree which overlap with `shape`
    /// (MinAreaTree.java:25-48).
    ///
    /// The traversal stack is a local `Vec`, matching Java's `ArrayStack<TreeNode>` local
    /// (MinAreaTree.java:30) — and, as `MinAreaTreeConcurrencyTest.java` records, *not* an
    /// instance field. Java pushes `firstChild` then `secondChild` (MinAreaTree.java:42-43), so
    /// the second child is visited first; the port keeps that push order even though the
    /// [`BTreeSet`] result makes the visit order unobservable.
    pub fn overlaps(&self, shape: &RegularTileShape) -> BTreeSet<TreeEntry<O>> {
        let mut found_overlaps = BTreeSet::new();
        let Some(root) = self.root else {
            return found_overlaps;
        };
        let query = shape.to_tile_shape();

        let mut node_stack: Vec<NodeId> = Vec::new();
        node_stack.push(root);
        while let Some(current_node) = node_stack.pop() {
            match self.resolve(current_node) {
                Node::Leaf {
                    bounds,
                    object,
                    shape_index,
                    ..
                } => {
                    if bounds.to_tile_shape().intersects(&query) {
                        found_overlaps.insert(TreeEntry {
                            object: *object,
                            shape_index: *shape_index,
                        });
                    }
                }
                Node::Inner {
                    bounds,
                    first_child,
                    second_child,
                    ..
                } => {
                    if bounds.to_tile_shape().intersects(&query) {
                        node_stack.push(*first_child);
                        node_stack.push(*second_child);
                    }
                }
                Node::Free { .. } => unreachable!("resolve rejects freed slots"),
            }
        }
        found_overlaps
    }

    /// The bounding shape stored for `leaf` (Java `Leaf.boundingShape`, ShapeTree.java:175).
    pub fn leaf_bounds(&self, leaf: LeafId) -> RegularTileShape {
        match self.resolve(leaf.node()) {
            Node::Leaf { bounds, .. } => *bounds,
            _ => panic!("ShapeTree: node {} is not a leaf", leaf.index()),
        }
    }

    /// The `(object, shape_index)` pair stored for `leaf` (Java `Leaf.object` /
    /// `Leaf.shapeIndexInObject`, ShapeTree.java:202-205).
    pub fn leaf_entry(&self, leaf: LeafId) -> TreeEntry<O> {
        match self.resolve(leaf.node()) {
            Node::Leaf {
                object,
                shape_index,
                ..
            } => TreeEntry {
                object: *object,
                shape_index: *shape_index,
            },
            _ => panic!("ShapeTree: node {} is not a leaf", leaf.index()),
        }
    }

    /// Rewrites a live leaf's `(object, shape_index)` key in place, leaving its bounding shape,
    /// its parent link and the whole tree layout untouched.
    ///
    /// This is not a convenience: `ShapeSearchTree` mutates `leaf.object` and
    /// `leaf.shapeIndexInObject` of leaves that stay in the tree, at
    /// `ShapeSearchTree.java:150` (`changeEntries`), `:214-215` and `:221`
    /// (`mergeEntriesInFront`), `:294-295` (`mergeEntriesAtEnd`), and `:325-326` and `:342-343`
    /// (`reuseEntriesAfterCutout`). Every one of those re-keys an existing leaf — a trace piece
    /// is handed to a different `Item`, or renumbered within the same one — without re-inserting
    /// it, so the tree keeps the exact shape the original insertion order produced. Emulating it
    /// with remove + insert would re-run `positionLocate` and change the tree.
    ///
    /// The bounding shape is deliberately *not* a parameter: none of those Java sites touches
    /// it. Use [`ShapeTree::remove_leaf`] + [`ShapeTree::insert_leaf`] when the geometry
    /// changes, which is what `ShapeSearchTree.changeItemShape`
    /// (`ShapeSearchTree.java:856,867`) does.
    ///
    /// obligation: Task 10 uses this for the five re-keying sites listed above.
    pub fn set_leaf_entry(&mut self, leaf: LeafId, object: O, shape_index: usize) {
        match self.resolve_mut(leaf.node()) {
            Node::Leaf {
                object: stored_object,
                shape_index: stored_index,
                ..
            } => {
                *stored_object = object;
                *stored_index = shape_index;
            }
            _ => panic!("ShapeTree: node {} is not a leaf", leaf.index()),
        }
    }

    /// Inserts the leaves of this tree into an array (ShapeTree.java:66-94): a left-to-right
    /// in-order walk, leftmost leaf first.
    pub fn to_array(&self) -> Vec<LeafId> {
        let mut result = Vec::with_capacity(self.leaf_count);
        let Some(root) = self.root else {
            return result;
        };
        let mut current_node = root;
        loop {
            // Go down from currentNode to the left most leaf (ShapeTree.java:75-78).
            while let Node::Inner { first_child, .. } = self.resolve(current_node) {
                current_node = *first_child;
            }
            result.push(LeafId::from_node(current_node));

            // Go up until parent.secondChild != currentNode, which means we came from
            // firstChild (ShapeTree.java:82-87).
            let mut current_parent = self.parent_of(current_node);
            while let Some(parent) = current_parent {
                if self.children_of(parent).1 != current_node {
                    break;
                }
                current_node = parent;
                current_parent = self.parent_of(parent);
            }
            // ShapeTree.java:88-91.
            let Some(parent) = current_parent else {
                break;
            };
            current_node = self.children_of(parent).1;
        }
        result
    }

    /// The number of nodes between `leaf` and the root of the tree
    /// (`ShapeTree.Leaf.distanceToRoot`, ShapeTree.java:226-234).
    ///
    /// Java bug: `distanceToRoot` dereferences `this.parent` without a null check
    /// (ShapeTree.java:228-229), so it throws a `NullPointerException` for the leaf of a
    /// one-element tree — which is exactly the tree `ShapeTree.statistics`
    /// (ShapeTree.java:111-136) would walk. The port panics for the same input rather than
    /// inventing a value; see `docs/java-quirks.md`.
    pub fn distance_to_root(&self, leaf: LeafId) -> usize {
        let mut result = 1;
        let mut current_parent = self.parent_of(leaf.node()).expect(
            "Java bug: ShapeTree.Leaf.distanceToRoot NPEs on the leaf of a one-element tree \
             (ShapeTree.java:229)",
        );
        while let Some(parent) = self.parent_of(current_parent) {
            current_parent = parent;
            result += 1;
        }
        result
    }
}

// ---- Plan 7 Task 14b: the level-8 `MAT` ledger ------------------------------------------------
//
// Instrumentation, not behaviour: `false` unless `P7T14B_MAT` is set in the environment, read
// once into a `LazyLock`, and its only callers are the two `eprintln!` sites in
// [`ShapeTree::insert_leaf`] and [`ShapeTree::remove_leaf`]. The Java side of the pair is level 8
// of `scripts/differential/java/p6t17b-bisect.patch`, which adds the same two lines to
// `MinAreaTree.insert(Leaf)` / `MinAreaTree.removeLeaf(Leaf)` under the same variable (its lines
// carry an extra `cls=` tag naming the tree's compensated clearance class; strip it before
// diffing). Both write to **stderr**.
//
// This is the ledger that names quirk #229: after a failed `BatchOptimizer.optRouteItem`, Java's
// `routingBoard.undo(null)` replays the attempt's item changes through `SearchTreeManager` and
// these ops fire; the port restores a cloned board and none of them do, so the two sides carry
// the same leaves in a different tree topology from there on.
fn p7t14b_mat_ledger() -> bool {
    static ON: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| std::env::var_os("P7T14B_MAT").is_some());
    *ON
}

fn p7t14b_mat(op: &str, leaf_count: usize, bounds: &RegularTileShape) {
    let b = bounds.bounding_box();
    eprintln!(
        "MAT {op} n={leaf_count} bb=({},{},{},{})",
        b.ll.x, b.ll.y, b.ur.x, b.ur.y
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{ItemId, TreeObject};
    use fr_geometry::int_box::IntBox;
    use fr_geometry::int_point::IntPoint;
    use fr_geometry::simplex::Simplex;

    fn obj(n: u32) -> TreeObject {
        TreeObject::Item(ItemId(n))
    }

    fn boxed(a: i32, b: i32, c: i32, d: i32) -> RegularTileShape {
        RegularTileShape::Box(IntBox::from_coords(a, b, c, d))
    }

    #[test]
    fn a_new_tree_is_empty() {
        let tree: ShapeTree<TreeObject> = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
        assert!(tree.is_empty());
        assert_eq!(tree.leaf_count(), 0);
        assert_eq!(tree.root(), None);
        assert_eq!(tree.node_count(), 0);
        assert_eq!(tree.to_array(), Vec::new());
        assert_eq!(
            tree.bounding_directions(),
            ShapeBoundingDirections::Orthogonal
        );
    }

    #[test]
    fn tree_entry_orders_by_object_then_shape_index() {
        // Java `Leaf.compareTo` (ShapeTree.java:216-223): `object.compareTo(other.object)`
        // first, then `shapeIndexInObject - other.shapeIndexInObject`. The object half is
        // `Item.compareTo` (Item.java:93-103), whose subtraction is reversed, so objects sort by
        // *descending* id while the shape index sorts ascending — see `TreeObject`'s `Ord`.
        let a = TreeEntry {
            object: obj(1),
            shape_index: 7,
        };
        let b = TreeEntry {
            object: obj(2),
            shape_index: 0,
        };
        let c = TreeEntry {
            object: obj(1),
            shape_index: 8,
        };
        assert!(b < a);
        assert!(a < c);
        assert!(b < c);
    }

    #[test]
    fn bounding_shape_applies_the_trees_directions() {
        // ShapeTree.java:51: `objectShape.boundingShape(boundingDirections)`.
        let orthogonal: ShapeTree<TreeObject> = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
        let tile = TileShape::Box(IntBox::from_coords(0, 0, 10, 10));
        assert!(matches!(
            orthogonal.bounding_shape(&tile),
            Some(RegularTileShape::Box(_))
        ));
        let diagonal: ShapeTree<TreeObject> =
            ShapeTree::new(ShapeBoundingDirections::FortyfiveDegree);
        assert!(matches!(
            diagonal.bounding_shape(&tile),
            Some(RegularTileShape::Octagon(_))
        ));
    }

    #[test]
    fn bounding_shape_is_none_for_an_unbounded_simplex() {
        // ShapeTree.java:52-55: Java's `boundingShape == null` branch.
        let diagonal: ShapeTree<TreeObject> =
            ShapeTree::new(ShapeBoundingDirections::FortyfiveDegree);
        let half_plane = Simplex::from_points(&[IntPoint::new(0, 0), IntPoint::new(10, 0)]);
        assert_eq!(
            diagonal.bounding_shape(&TileShape::Simplex(half_plane)),
            None
        );
    }

    #[test]
    fn a_single_leaf_becomes_the_root() {
        // MinAreaTree.java:54-58.
        let mut tree = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
        let leaf = tree.insert_leaf(obj(1), 0, boxed(0, 0, 10, 10));
        assert_eq!(tree.root(), Some(leaf.node()));
        assert_eq!(tree.leaf_count(), 1);
        assert_eq!(tree.leaf_bounds(leaf), boxed(0, 0, 10, 10));
        assert_eq!(tree.to_array(), vec![leaf]);
    }

    #[test]
    fn octagon_bounds_work_end_to_end() {
        // The 45-degree tree is what the router actually uses; the integration tests use
        // orthogonal boxes because their areas are hand-checkable.
        let mut tree = ShapeTree::new(ShapeBoundingDirections::FortyfiveDegree);
        let shapes: Vec<RegularTileShape> = [(0, 0, 10, 10), (100, 100, 110, 110), (5, 5, 15, 15)]
            .into_iter()
            .map(|(a, b, c, d)| {
                RegularTileShape::Octagon(IntBox::from_coords(a, b, c, d).to_int_octagon())
            })
            .collect();
        for (i, shape) in shapes.iter().enumerate() {
            tree.insert(obj(i as u32 + 1), &[*shape]);
        }
        assert_eq!(tree.leaf_count(), 3);
        let found = tree.overlaps(&shapes[0]);
        // Descending object id: `Item.compareTo` (Item.java:98) subtracts the wrong way round.
        assert_eq!(
            found.iter().map(|e| e.object).collect::<Vec<_>>(),
            vec![obj(3), obj(1)]
        );
    }

    #[test]
    #[should_panic(expected = "the node was removed")]
    fn removing_a_leaf_twice_panics() {
        // Java silently empties the whole tree instead; see `remove_leaf`'s doc comment.
        let mut tree = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
        let a = tree.insert_leaf(obj(1), 0, boxed(0, 0, 10, 10));
        tree.insert_leaf(obj(2), 0, boxed(20, 0, 30, 10));
        tree.remove_leaf(a);
        tree.remove_leaf(a);
    }

    #[test]
    fn insert_tiles_applies_the_trees_bounding_directions() {
        // The hazard `insert_tiles` exists to remove: a 45-degree tree must store OCTAGON
        // bounds even when the object's tree shape is a box (ShapeTree.java:51).
        let mut diagonal = ShapeTree::new(ShapeBoundingDirections::FortyfiveDegree);
        let tile = TileShape::Box(IntBox::from_coords(0, 0, 10, 10));
        let entries = diagonal.insert_tiles(obj(1), std::slice::from_ref(&tile));
        assert_eq!(entries.len(), 1);
        let leaf = entries[0].expect("a box is always boundable");
        assert!(matches!(
            diagonal.leaf_bounds(leaf),
            RegularTileShape::Octagon(_)
        ));

        let mut orthogonal = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
        let leaf =
            orthogonal.insert_tiles(obj(1), std::slice::from_ref(&tile))[0].expect("boundable");
        assert!(matches!(
            orthogonal.leaf_bounds(leaf),
            RegularTileShape::Box(_)
        ));
    }

    #[test]
    fn insert_tiles_leaves_a_hole_where_java_leaves_null() {
        // ShapeTree.java:51-55: `boundingShape == null` -> the leaf array slot stays null and
        // the remaining shapes are still inserted.
        let mut tree = ShapeTree::new(ShapeBoundingDirections::FortyfiveDegree);
        let half_plane = TileShape::Simplex(Simplex::from_points(&[
            IntPoint::new(0, 0),
            IntPoint::new(10, 0),
        ]));
        assert_eq!(tree.bounding_shape(&half_plane), None);
        let entries = tree.insert_tiles(
            obj(1),
            &[
                TileShape::Box(IntBox::from_coords(0, 0, 10, 10)),
                half_plane,
                TileShape::Box(IntBox::from_coords(20, 20, 30, 30)),
            ],
        );
        assert_eq!(entries.len(), 3);
        assert!(entries[0].is_some());
        assert_eq!(entries[1], None);
        assert!(entries[2].is_some());
        assert_eq!(tree.leaf_count(), 2);
        // The shape indices of the leaves that were inserted keep their original positions.
        assert_eq!(tree.leaf_entry(entries[0].unwrap()).shape_index, 0);
        assert_eq!(tree.leaf_entry(entries[2].unwrap()).shape_index, 2);
    }

    #[test]
    fn insert_tiles_on_no_shapes_returns_no_entries() {
        // ShapeTree.java:33-35 — Java returns before `setSearchTreeEntries`; see the
        // `obligation:` note on `insert_tiles`.
        let mut tree = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
        assert!(tree.insert_tiles(obj(1), &[]).is_empty());
        assert_eq!(tree.leaf_count(), 0);
    }

    #[test]
    fn remove_opt_skips_holes_like_java() {
        // MinAreaTree.java:121-123 — `reuseEntriesAfterCutout` nulls entries it moved
        // (ShapeSearchTree.java:327,344) and `SearchTreeManager.remove` passes the holed array
        // straight to `remove(Leaf[])`.
        let mut tree = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
        let a = tree.insert_leaf(obj(1), 0, boxed(0, 0, 10, 10));
        let b = tree.insert_leaf(obj(1), 1, boxed(20, 0, 30, 10));
        let c = tree.insert_leaf(obj(1), 2, boxed(40, 0, 50, 10));
        tree.remove_opt(&[Some(a), None, Some(c)]);
        assert_eq!(tree.leaf_count(), 1);
        assert_eq!(tree.leaf_entry(b).shape_index, 1);
        // The all-`None` array is Java's fully emptied entry array: a complete no-op.
        tree.remove_opt(&[None, None]);
        assert_eq!(tree.leaf_count(), 1);
        tree.remove_leaf_opt(None);
        assert_eq!(tree.leaf_count(), 1);
        tree.remove_leaf_opt(Some(b));
        assert_eq!(tree.leaf_count(), 0);
    }

    #[test]
    fn a_stale_handle_is_reported_by_is_live() {
        let mut tree = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
        let a = tree.insert_leaf(obj(1), 0, boxed(0, 0, 10, 10));
        let b = tree.insert_leaf(obj(2), 0, boxed(20, 0, 30, 10));
        assert!(tree.is_live(a.node()));
        tree.remove_leaf(a);
        assert!(!tree.is_live(a.node()));
        assert!(tree.is_live(b.node()));
    }

    #[test]
    #[should_panic(expected = "stale handle")]
    fn a_stale_leaf_id_panics_after_its_slot_is_reused() {
        // `ShapeSearchTree.changeItemShape` (ShapeSearchTree.java:856,867) removes a leaf and
        // immediately re-inserts at the same shape index; the LIFO free list hands back the
        // very slot just released, so without the generation counter the stale handle would
        // silently name the NEW leaf. Java NPEs on the nulled fields instead.
        let mut tree = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
        let a = tree.insert_leaf(obj(1), 0, boxed(0, 0, 10, 10));
        tree.insert_leaf(obj(2), 0, boxed(20, 0, 30, 10));
        let slots = tree.node_count();

        // Removing `a` frees two slots (the leaf and its parent inner node); the next insert
        // allocates two and takes both back, so `a`'s slot is occupied again and the arena has
        // not grown. Which node lands in `a`'s exact slot depends on the LIFO order, and that
        // is precisely the point: an index-only handle could name either.
        tree.remove_leaf(a);
        assert_eq!(tree.free_slot_count(), 2);
        assert!(!tree.is_live(a.node()));
        tree.insert_leaf(obj(3), 0, boxed(0, 0, 10, 10));
        assert_eq!(tree.free_slot_count(), 0);
        assert_eq!(
            tree.node_count(),
            slots,
            "a's slot was reused, not abandoned"
        );
        assert!(
            !tree.is_live(a.node()),
            "and the stale handle still does not resolve"
        );

        let _ = tree.leaf_entry(a);
    }

    #[test]
    #[should_panic(expected = "distanceToRoot NPEs")]
    fn distance_to_root_panics_on_a_one_element_tree() {
        // Java bug: ShapeTree.java:228-229 dereferences a null parent.
        let mut tree = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
        let only = tree.insert_leaf(obj(1), 0, boxed(0, 0, 10, 10));
        let _ = tree.distance_to_root(only);
    }
}
