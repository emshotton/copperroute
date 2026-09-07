use std::collections::BTreeSet;

use copper_geometry::bounding_directions::ShapeBoundingDirections;
use copper_geometry::regular_tile_shape::RegularTileShape;
use copper_geometry::tile_shape::TileShape;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId {
    index: u32,
    generation: u32,
}

impl NodeId {
    pub fn index(self) -> usize {
        self.index as usize
    }

    pub fn generation(self) -> u32 {
        self.generation
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LeafId {
    index: u32,
    generation: u32,
}

impl LeafId {
    pub fn index(self) -> usize {
        self.index as usize
    }

    pub fn generation(self) -> u32 {
        self.generation
    }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Node<O> {
    Inner {
        bounds: RegularTileShape,
        parent: Option<NodeId>,
        first_child: NodeId,
        second_child: NodeId,
        generation: u32,
    },
    Leaf {
        bounds: RegularTileShape,
        parent: Option<NodeId>,
        object: O,
        shape_index: usize,
        generation: u32,
    },
    Free {
        next_free: Option<NodeId>,
        generation: u32,
    },
}

impl<O> Node<O> {
    fn generation(&self) -> u32 {
        match self {
            Node::Inner { generation, .. }
            | Node::Leaf { generation, .. }
            | Node::Free { generation, .. } => *generation,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TreeEntry<O> {
    pub object: O,
    pub shape_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapeTree<O> {
    bounding_directions: ShapeBoundingDirections,
    nodes: Vec<Node<O>>,
    root: Option<NodeId>,
    leaf_count: usize,
    first_free: Option<NodeId>,
}

impl<O> ShapeTree<O> {
    pub fn new(bounding_directions: ShapeBoundingDirections) -> Self {
        Self {
            bounding_directions,
            nodes: Vec::new(),
            root: None,
            leaf_count: 0,
            first_free: None,
        }
    }

    pub fn bounding_directions(&self) -> ShapeBoundingDirections {
        self.bounding_directions
    }

    pub fn leaf_count(&self) -> usize {
        self.leaf_count
    }

    pub fn is_empty(&self) -> bool {
        self.leaf_count == 0
    }

    pub fn root(&self) -> Option<NodeId> {
        self.root
    }

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

    pub fn is_live(&self, id: NodeId) -> bool {
        self.nodes.get(id.index()).is_some_and(|node| {
            node.generation() == id.generation && !matches!(node, Node::Free { .. })
        })
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

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

    fn replace_child(&mut self, id: NodeId, old: NodeId, new: NodeId) -> bool {
        match self.resolve_mut(id) {
            Node::Inner {
                first_child,
                second_child,
                ..
            } => {
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
    pub fn insert(&mut self, object: O, shapes: &[RegularTileShape]) -> Vec<LeafId> {
        shapes
            .iter()
            .enumerate()
            .map(|(index, shape)| self.insert_leaf(object, index, *shape))
            .collect()
    }

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

    pub fn insert_tiles_opt(
        &mut self,
        object: O,
        shapes: &[Option<TileShape>],
    ) -> Vec<Option<LeafId>> {
        shapes
            .iter()
            .enumerate()
            .map(|(index, shape)| {
                let shape = shape.as_ref()?;
                let bounds = self.bounding_shape(shape)?;
                Some(self.insert_leaf(object, index, bounds))
            })
            .collect()
    }

    pub fn insert_leaf(
        &mut self,
        object: O,
        shape_index: usize,
        bounds: RegularTileShape,
    ) -> LeafId {
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
        self.leaf_count += 1;

        let Some(root) = self.root else {
            self.root = Some(leaf);
            return LeafId::from_node(leaf);
        };

        let to_replace = self.position_locate(root, bounds);

        let new_bounds = bounds.union(&self.bounds_of(to_replace));
        let current_parent = self.parent_of(to_replace);
        let new_node = self.alloc(|generation| Node::Inner {
            bounds: new_bounds,
            parent: current_parent,
            first_child: to_replace,
            second_child: leaf,
            generation,
        });

        if let Some(parent) = current_parent {
            let replaced = self.replace_child(parent, to_replace, new_node);
            debug_assert!(replaced, "ShapeTree.insert_leaf: parent inconsistent");
        }
        self.set_parent(to_replace, Some(new_node));
        self.set_parent(leaf, Some(new_node));

        if root == to_replace {
            self.root = Some(new_node);
        }
        LeafId::from_node(leaf)
    }

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

            let widened = leaf_bounds.union(&self.bounds_of(node));
            self.set_bounds(node, widened);

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

    pub fn remove(&mut self, entries: &[LeafId]) {
        for entry in entries {
            self.remove_leaf(*entry);
        }
    }

    pub fn remove_opt(&mut self, entries: &[Option<LeafId>]) {
        for entry in entries {
            self.remove_leaf_opt(*entry);
        }
    }

    pub fn remove_leaf_opt(&mut self, leaf: Option<LeafId>) {
        if let Some(leaf) = leaf {
            self.remove_leaf(leaf);
        }
    }

    pub fn remove_leaf(&mut self, leaf: LeafId) {
        let leaf = leaf.node();
        assert!(
            matches!(self.resolve(leaf), Node::Leaf { .. }),
            "ShapeTree.remove_leaf: node {} is not a leaf",
            leaf.index()
        );

        if p7t14b_mat_ledger() {
            p7t14b_mat("rem", self.leaf_count, &self.bounds_of(leaf));
        }
        let parent = self.parent_of(leaf);
        self.free(leaf);
        self.leaf_count -= 1;

        let Some(parent) = parent else {
            self.root = None;
            return;
        };

        let (first_child, second_child) = self.children_of(parent);
        let other_leaf = if second_child == leaf {
            first_child
        } else if first_child == leaf {
            second_child
        } else {
            panic!("MinAreaTree.remove_leaf: parent inconsistent");
        };

        let grand_parent = self.parent_of(parent);
        self.set_parent(other_leaf, grand_parent);
        match grand_parent {
            None => self.root = Some(other_leaf),
            Some(grand_parent) => {
                let replaced = self.replace_child(grand_parent, parent, other_leaf);
                debug_assert!(
                    replaced,
                    "MinAreaTree.remove_leaf: grandParent inconsistent"
                );
            }
        }
        self.free(parent);

        let mut node_to_recalculate = grand_parent;
        while let Some(node) = node_to_recalculate {
            let (first_child, second_child) = self.children_of(node);
            let new_bounds = self
                .bounds_of(second_child)
                .union(&self.bounds_of(first_child));
            if new_bounds.contains(&self.bounds_of(node)) {
                break;
            }
            self.set_bounds(node, new_bounds);
            node_to_recalculate = self.parent_of(node);
        }
    }

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

    pub fn leaf_bounds(&self, leaf: LeafId) -> RegularTileShape {
        match self.resolve(leaf.node()) {
            Node::Leaf { bounds, .. } => *bounds,
            _ => panic!("ShapeTree: node {} is not a leaf", leaf.index()),
        }
    }

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

    pub fn to_array(&self) -> Vec<LeafId> {
        let mut result = Vec::with_capacity(self.leaf_count);
        let Some(root) = self.root else {
            return result;
        };
        let mut current_node = root;
        loop {
            while let Node::Inner { first_child, .. } = self.resolve(current_node) {
                current_node = *first_child;
            }
            result.push(LeafId::from_node(current_node));

            let mut current_parent = self.parent_of(current_node);
            while let Some(parent) = current_parent {
                if self.children_of(parent).1 != current_node {
                    break;
                }
                current_node = parent;
                current_parent = self.parent_of(parent);
            }
            let Some(parent) = current_parent else {
                break;
            };
            current_node = self.children_of(parent).1;
        }
        result
    }

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
    use copper_geometry::int_box::IntBox;
    use copper_geometry::int_point::IntPoint;
    use copper_geometry::simplex::Simplex;

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
        let mut tree = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
        let leaf = tree.insert_leaf(obj(1), 0, boxed(0, 0, 10, 10));
        assert_eq!(tree.root(), Some(leaf.node()));
        assert_eq!(tree.leaf_count(), 1);
        assert_eq!(tree.leaf_bounds(leaf), boxed(0, 0, 10, 10));
        assert_eq!(tree.to_array(), vec![leaf]);
    }

    #[test]
    fn octagon_bounds_work_end_to_end() {
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
            vec![obj(3), obj(1)]
        );
    }

    #[test]
    #[should_panic(expected = "the node was removed")]
    fn removing_a_leaf_twice_panics() {
        let mut tree = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
        let a = tree.insert_leaf(obj(1), 0, boxed(0, 0, 10, 10));
        tree.insert_leaf(obj(2), 0, boxed(20, 0, 30, 10));
        tree.remove_leaf(a);
        tree.remove_leaf(a);
    }

    #[test]
    fn insert_tiles_applies_the_trees_bounding_directions() {
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
        assert_eq!(tree.leaf_entry(entries[0].unwrap()).shape_index, 0);
        assert_eq!(tree.leaf_entry(entries[2].unwrap()).shape_index, 2);
    }

    #[test]
    fn insert_tiles_on_no_shapes_returns_no_entries() {
        let mut tree = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
        assert!(tree.insert_tiles(obj(1), &[]).is_empty());
        assert_eq!(tree.leaf_count(), 0);
    }

    #[test]
    fn remove_opt_skips_holes_like_java() {
        let mut tree = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
        let a = tree.insert_leaf(obj(1), 0, boxed(0, 0, 10, 10));
        let b = tree.insert_leaf(obj(1), 1, boxed(20, 0, 30, 10));
        let c = tree.insert_leaf(obj(1), 2, boxed(40, 0, 50, 10));
        tree.remove_opt(&[Some(a), None, Some(c)]);
        assert_eq!(tree.leaf_count(), 1);
        assert_eq!(tree.leaf_entry(b).shape_index, 1);
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
        let mut tree = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
        let a = tree.insert_leaf(obj(1), 0, boxed(0, 0, 10, 10));
        tree.insert_leaf(obj(2), 0, boxed(20, 0, 30, 10));
        let slots = tree.node_count();

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
        let mut tree = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
        let only = tree.insert_leaf(obj(1), 0, boxed(0, 0, 10, 10));
        let _ = tree.distance_to_root(only);
    }
}
