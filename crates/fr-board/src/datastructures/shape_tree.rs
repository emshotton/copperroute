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
//! removed node's slot goes on a free list and is reused. Reference identity becomes index
//! equality, which behaves the same for every operation the tree performs — with one caveat
//! spelled out on [`ShapeTree::remove_leaf`].
//!
//! # Determinism
//!
//! The tree must be a pure function of the insertion order, because the router's results depend
//! on the order `overlaps` reports objects in. Two things guarantee that:
//!
//! * The insertion heuristic ([`ShapeTree::position_locate`]) is deterministic, ties included:
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

/// Index of a node in a [`ShapeTree`]'s arena — the port's replacement for Java's `TreeNode`
/// reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(usize);

impl NodeId {
    /// The raw arena index.
    pub fn index(self) -> usize {
        self.0
    }
}

/// Index of a *leaf* node in a [`ShapeTree`]'s arena — the port's replacement for Java's
/// `ShapeTree.Leaf` reference, which board items hold in `setSearchTreeEntries`
/// (ShapeTree.java:150-154) so they can delete their own tree entries later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LeafId(usize);

impl LeafId {
    /// The raw arena index.
    pub fn index(self) -> usize {
        self.0
    }

    /// Widens this leaf index to a plain node index.
    pub fn node(self) -> NodeId {
        NodeId(self.0)
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
    },
    /// Java `ShapeTree.Leaf` (ShapeTree.java:198-235): where the geometry is stored.
    Leaf {
        /// Java `TreeNode.boundingShape` (ShapeTree.java:175).
        bounds: RegularTileShape,
        /// Java `TreeNode.parent` (ShapeTree.java:176). `None` when the leaf is the root.
        parent: Option<NodeId>,
        /// Java `Leaf.object` (ShapeTree.java:199).
        object: O,
        /// Java `Leaf.shapeIndexInObject` (ShapeTree.java:202).
        shape_index: usize,
    },
    /// A free arena slot. No Java counterpart — see the type docs.
    Free {
        /// The next free slot, forming a LIFO free list.
        next_free: Option<NodeId>,
    },
}

/// Information about a single object stored in a tree: Java `ShapeTree.TreeEntry`
/// (ShapeTree.java:157-168), used as the element type of [`ShapeTree::overlaps`]'s result set.
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
/// hands its shapes to [`ShapeTree::insert`] and keeps the returned [`LeafId`]s itself, instead
/// of the tree calling back into the object.
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

    /// Borrows an arena node. Panics if `id` names a freed slot or is out of range — both are
    /// programming errors, equivalent to dereferencing a stale Java `TreeNode` reference.
    pub fn node(&self, id: NodeId) -> &Node<O> {
        let node = self
            .nodes
            .get(id.0)
            .unwrap_or_else(|| panic!("ShapeTree: node index {} is out of range", id.0));
        debug_assert!(
            !matches!(node, Node::Free { .. }),
            "ShapeTree: node index {} names a freed slot",
            id.0
        );
        node
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
            next = match &self.nodes[id.0] {
                Node::Free { next_free } => *next_free,
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
        match self.node(id) {
            Node::Inner { bounds, .. } | Node::Leaf { bounds, .. } => *bounds,
            Node::Free { .. } => panic!("ShapeTree: node {} is a freed slot", id.0),
        }
    }

    fn set_bounds(&mut self, id: NodeId, new_bounds: RegularTileShape) {
        match &mut self.nodes[id.0] {
            Node::Inner { bounds, .. } | Node::Leaf { bounds, .. } => *bounds = new_bounds,
            Node::Free { .. } => panic!("ShapeTree: node {} is a freed slot", id.0),
        }
    }

    fn parent_of(&self, id: NodeId) -> Option<NodeId> {
        match self.node(id) {
            Node::Inner { parent, .. } | Node::Leaf { parent, .. } => *parent,
            Node::Free { .. } => panic!("ShapeTree: node {} is a freed slot", id.0),
        }
    }

    fn set_parent(&mut self, id: NodeId, new_parent: Option<NodeId>) {
        match &mut self.nodes[id.0] {
            Node::Inner { parent, .. } | Node::Leaf { parent, .. } => *parent = new_parent,
            Node::Free { .. } => panic!("ShapeTree: node {} is a freed slot", id.0),
        }
    }

    /// The two children of an inner node; panics if `id` is not an inner node.
    fn children_of(&self, id: NodeId) -> (NodeId, NodeId) {
        match self.node(id) {
            Node::Inner {
                first_child,
                second_child,
                ..
            } => (*first_child, *second_child),
            _ => panic!("ShapeTree: node {} is not an inner node", id.0),
        }
    }

    /// Replaces whichever child of the inner node `id` currently equals `old` with `new`.
    ///
    /// Returns false when neither child matches, which is Java's "parent inconsistent" /
    /// "grandParent inconsistent" branch (MinAreaTree.java:141-143, 156-158).
    fn replace_child(&mut self, id: NodeId, old: NodeId, new: NodeId) -> bool {
        match &mut self.nodes[id.0] {
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
            _ => panic!("ShapeTree: node {} is not an inner node", id.0),
        }
    }

    /// Allocates an arena slot, reusing the head of the free list when there is one.
    fn alloc(&mut self, node: Node<O>) -> NodeId {
        match self.first_free {
            Some(id) => {
                self.first_free = match &self.nodes[id.0] {
                    Node::Free { next_free } => *next_free,
                    _ => unreachable!("the free list only links Node::Free slots"),
                };
                self.nodes[id.0] = node;
                id
            }
            None => {
                self.nodes.push(node);
                NodeId(self.nodes.len() - 1)
            }
        }
    }

    /// Returns an arena slot to the free list. Stands in for Java dropping the last reference
    /// to a node and letting the garbage collector take it.
    fn free(&mut self, id: NodeId) {
        self.nodes[id.0] = Node::Free {
            next_free: self.first_free,
        };
        self.first_free = Some(id);
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
    /// The shapes passed in are the *bounding* shapes Java computes at ShapeTree.java:51; use
    /// [`ShapeTree::bounding_shape`] to derive them from an object's raw
    /// [`TileShape`]s. Because they are already `RegularTileShape`s, Java's two `null` returns
    /// (a missing tree shape, ShapeTree.java:47-49, and an unboundable shape,
    /// ShapeTree.java:52-55) are resolved before the call, and the returned vector always has
    /// exactly `shapes.len()` entries — Java's `leafArr` can contain nulls.
    pub fn insert(&mut self, object: O, shapes: &[RegularTileShape]) -> Vec<LeafId> {
        shapes
            .iter()
            .enumerate()
            .map(|(index, shape)| self.insert_leaf(object, index, *shape))
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
        let leaf = self.alloc(Node::Leaf {
            bounds,
            parent: None,
            object,
            shape_index,
        });

        // MinAreaTree.java:52.
        self.leaf_count += 1;

        // Tree is empty - just insert the new leaf (MinAreaTree.java:54-58).
        let Some(root) = self.root else {
            self.root = Some(leaf);
            return LeafId(leaf.0);
        };

        // Non-empty tree - do a recursive location for leaf replacement (MinAreaTree.java:61).
        let to_replace = self.position_locate(root, bounds);

        // Construct a new node - whenever a leaf is added so is a new node
        // (MinAreaTree.java:63-67).
        let new_bounds = bounds.union(&self.bounds_of(to_replace));
        let current_parent = self.parent_of(to_replace);
        let new_node = self.alloc(Node::Inner {
            bounds: new_bounds,
            parent: current_parent,
            // Insert the children in any order (MinAreaTree.java:80-82): Java assigns
            // firstChild = leafToReplace, secondChild = leaf. That choice is load-bearing —
            // `positionLocate` breaks ties toward the first child — so the port keeps it.
            first_child: to_replace,
            second_child: leaf,
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
        LeafId(leaf.0)
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
            let (first_child, second_child) = match self.node(node) {
                Node::Leaf { .. } => return node,
                Node::Inner {
                    first_child,
                    second_child,
                    ..
                } => (*first_child, *second_child),
                Node::Free { .. } => panic!("ShapeTree: node {} is a freed slot", node.0),
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
        assert!(
            matches!(self.nodes.get(leaf.0), Some(Node::Leaf { .. })),
            "ShapeTree.remove_leaf: node {} is not a live leaf (already removed?)",
            leaf.0
        );

        // MinAreaTree.java:125-129: read the parent, then clear the leaf. The port frees the
        // arena slot where Java nulls the leaf's fields and lets the GC take it.
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
            match self.node(current_node) {
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
                Node::Free { .. } => {
                    panic!("ShapeTree: node {} is a freed slot", current_node.0)
                }
            }
        }
        found_overlaps
    }

    /// The bounding shape stored for `leaf` (Java `Leaf.boundingShape`, ShapeTree.java:175).
    pub fn leaf_bounds(&self, leaf: LeafId) -> RegularTileShape {
        match self.node(leaf.node()) {
            Node::Leaf { bounds, .. } => *bounds,
            _ => panic!("ShapeTree: node {} is not a leaf", leaf.0),
        }
    }

    /// The `(object, shape_index)` pair stored for `leaf` (Java `Leaf.object` /
    /// `Leaf.shapeIndexInObject`, ShapeTree.java:202-205).
    pub fn leaf_entry(&self, leaf: LeafId) -> TreeEntry<O> {
        match self.node(leaf.node()) {
            Node::Leaf {
                object,
                shape_index,
                ..
            } => TreeEntry {
                object: *object,
                shape_index: *shape_index,
            },
            _ => panic!("ShapeTree: node {} is not a leaf", leaf.0),
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
            while let Node::Inner { first_child, .. } = self.node(current_node) {
                current_node = *first_child;
            }
            result.push(LeafId(current_node.0));

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
        // Java `Leaf.compareTo` (ShapeTree.java:216-223).
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
        assert!(a < b);
        assert!(a < c);
        assert!(c < b);
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
        assert_eq!(
            found.iter().map(|e| e.object).collect::<Vec<_>>(),
            vec![obj(1), obj(3)]
        );
    }

    #[test]
    #[should_panic(expected = "not a live leaf")]
    fn removing_a_leaf_twice_panics() {
        // Java silently empties the whole tree instead; see `remove_leaf`'s doc comment.
        let mut tree = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
        let a = tree.insert_leaf(obj(1), 0, boxed(0, 0, 10, 10));
        tree.insert_leaf(obj(2), 0, boxed(20, 0, 30, 10));
        tree.remove_leaf(a);
        tree.remove_leaf(a);
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
