//! The two `ShapeSearchTree` methods `fr-board` could not carry, because both take and return
//! `autoroute.expansion.IncompleteFreeSpaceExpansionRoom`.
//!
//! An **extension trait** in `fr-router` keeps `fr-board` free of rooms — the `RoutingBoardExt`
//! precedent (plan-2 ruling 4). The dispatch is on [`ShapeSearchTree::angle`], because
//! `SearchTreeManager.getAutorouteTree` (SearchTreeManager.java:147-161) chose the Java subclass
//! from exactly that value and `crates/fr-board/src/searchtree/manager.rs` already reproduces the
//! choice; there is no subclass hierarchy here and there must not be one.
//!
//! The three regimes are **not** refinements of one another. The base class cuts with half
//! planes and returns whatever `TileShape.intersection` produces; the 45-degree override replaces
//! the restraining geometry wholesale with eight-ordinate octagon arithmetic; the 90-degree
//! override does the same with four-ordinate box arithmetic and — alone of the three — never
//! divides the result. Read all three before changing any of them.
//!
//! not ported: the six private diagnostic helpers of the two subclasses — `describeBounds`,
//! `isCompleteShapeDebugAnchor`, `traceCompleteShapeFilter`, `traceCompleteShapeCandidate`,
//! `traceCompleteShapeDecision`, `obstacleId` and `obstacleNets`
//! (ShapeSearchTree45Degree.java:543-647, ShapeSearchTree90Degree.java:324-432). Every one is an
//! `FRLogger.trace` payload for a single hard-coded room the Java author was debugging
//! (`isCompleteShapeDebugAnchor` tests net 77 / net 84 at literal coordinates), and
//! `global-constraints.md` drops every `FRLogger` payload with its guard. The base class's own
//! `COMPLETE_SHAPE_DECISION` trace (ShapeSearchTree.java:650-668) and the two subclasses'
//! `COMPLETE_SHAPE_BLOCKED` traces (…45Degree.java:248-262, …90Degree.java:163-177) go with them.
//! not ported: `ShapeSearchTree45Degree.describeBounds`
//! not ported: `ShapeSearchTree45Degree.isCompleteShapeDebugAnchor`
//! not ported: `ShapeSearchTree45Degree.traceCompleteShapeFilter`
//! not ported: `ShapeSearchTree45Degree.traceCompleteShapeCandidate`
//! not ported: `ShapeSearchTree45Degree.traceCompleteShapeDecision`
//! not ported: `ShapeSearchTree45Degree.obstacleId`
//! not ported: `ShapeSearchTree45Degree.obstacleNets`
//! not ported: `ShapeSearchTree90Degree.describeBounds`
//! not ported: `ShapeSearchTree90Degree.isCompleteShapeDebugAnchor`
//! not ported: `ShapeSearchTree90Degree.traceCompleteShapeFilter`
//! not ported: `ShapeSearchTree90Degree.traceCompleteShapeCandidate`
//! not ported: `ShapeSearchTree90Degree.traceCompleteShapeDecision`
//! not ported: `ShapeSearchTree90Degree.obstacleId`
//! not ported: `ShapeSearchTree90Degree.obstacleNets`

use fr_board::ids::TreeObject;
use fr_board::{AngleRestriction, ItemCtx, ItemLookup, Node, NodeId, ShapeSearchTree};
use fr_geometry::{IntBox, IntOctagon, Line, LineSegment, RegularTileShape, Side, TileShape};

use crate::autoroute::expansion::{ExpansionRoomStore, IncompleteFreeSpaceExpansionRoom};

/// The two `ShapeSearchTree` methods that take and return
/// `autoroute.expansion.IncompleteFreeSpaceExpansionRoom` (the Plan 2 hand-off).
pub trait AutorouteSearchTreeExt {
    /// Port of `ShapeSearchTree.completeShape` (ShapeSearchTree.java:580-693) with its two
    /// overrides (ShapeSearchTree45Degree.java:95-281, ShapeSearchTree90Degree.java:38-191),
    /// dispatched on [`ShapeSearchTree::angle`] exactly as Java dispatches on the subclass.
    ///
    /// "Calculates a new incomplete room with a maximal `TileShape` contained in the shape of
    /// `room`, which may overlap only with items of the input net on the input layer.
    /// `room.getContainedShape()` will be contained in the shape of the result room. If that is
    /// not possible, several rooms are returned with shapes which intersect with
    /// `room.getContainedShape()`. The result room is not yet complete, because its doors are not
    /// yet calculated. If `ignoreShape != null`, objects of type
    /// `CompleteFreeSpaceExpansionRoom` whose intersection with the shape of `room` is contained
    /// in `ignoreShape` are ignored."
    ///
    /// # The two arguments Java does not have
    ///
    /// Java reads `this.board` for the item list and the bounding box, and reaches a room's shape
    /// and layer through the `SearchTreeObject` it stored. The port cannot: `fr-board` has no way
    /// to resolve a [`TreeObject::Room`] — that is the obligation `insert_room` records — so
    /// `complete_shape` resolves both kinds of stored object **itself**, out of `items` and
    /// `rooms`. It therefore never calls `ShapeSearchTree`'s own `overlapping_*` family and never
    /// reaches the two `TreeObject::Room` panics those still carry.
    ///
    /// `rooms` is the extra parameter the brief's signature omits; without it the
    /// `instanceof CompleteFreeSpaceExpansionRoom` branch (ShapeSearchTree.java:646-648,
    /// …45Degree.java:193-215, …90Degree.java:112-130) cannot be evaluated at all, and it is
    /// live: `AutorouteEngine.completeExpansionRoom` (AutorouteEngine.java:450) passes an
    /// `ignoreObject` that *is* a complete room, over a tree the engine has been inserting
    /// complete rooms into (`:534`). An empty [`ExpansionRoomStore`] is the "no rooms yet" case.
    #[allow(clippy::too_many_arguments)]
    fn complete_shape(
        &self,
        room: &IncompleteFreeSpaceExpansionRoom,
        net_no: i32,
        ignore_object: Option<TreeObject>,
        ignore_shape: Option<&TileShape>,
        items: &impl ItemLookup,
        rooms: &ExpansionRoomStore,
        ctx: &ItemCtx<'_>,
    ) -> Vec<IncompleteFreeSpaceExpansionRoom>;

    /// Port of `ShapeSearchTree.divideLargeRoom` (ShapeSearchTree.java:1095-1118) and
    /// `ShapeSearchTree45Degree.divideLargeRoom` (ShapeSearchTree45Degree.java:288-298).
    ///
    /// "Makes sure that on each layer there will be more than 1
    /// `IncompleteFreeSpaceExpansionRoom`, even if there are no objects on the layer. Otherwise
    /// the maze search algorithm gets problems with vias."
    ///
    /// Called only from [`Self::complete_shape`] — and not by the 90-degree regime, which returns
    /// its raw result (ShapeSearchTree90Degree.java:190). A 90-degree tree still answers this
    /// method with the base implementation, because `ShapeSearchTree90Degree` does not override
    /// it.
    ///
    /// **Hazard C** (plan-6 ruling 4): this method *creates* incomplete rooms, and
    /// `IncompleteFreeSpaceExpansionRoom.getId` hashes a shape the 45-degree override then
    /// replaces in place (`:294-295`). Ported as written; the id moves, exactly as in Java.
    fn divide_large_room(
        &self,
        rooms: Vec<IncompleteFreeSpaceExpansionRoom>,
        board_bounds: &IntBox,
    ) -> Vec<IncompleteFreeSpaceExpansionRoom>;
}

impl AutorouteSearchTreeExt for ShapeSearchTree {
    fn complete_shape(
        &self,
        room: &IncompleteFreeSpaceExpansionRoom,
        net_no: i32,
        ignore_object: Option<TreeObject>,
        ignore_shape: Option<&TileShape>,
        items: &impl ItemLookup,
        rooms: &ExpansionRoomStore,
        ctx: &ItemCtx<'_>,
    ) -> Vec<IncompleteFreeSpaceExpansionRoom> {
        // SearchTreeManager.java:149-160 chose the subclass from this value; so does this.
        match self.angle() {
            AngleRestriction::NinetyDegree => complete_shape_90(
                self,
                room,
                net_no,
                ignore_object,
                ignore_shape,
                items,
                rooms,
                ctx,
            ),
            AngleRestriction::FortyFiveDegree => complete_shape_45(
                self,
                room,
                net_no,
                ignore_object,
                ignore_shape,
                items,
                rooms,
                ctx,
            ),
            AngleRestriction::None => complete_shape_base(
                self,
                room,
                net_no,
                ignore_object,
                ignore_shape,
                items,
                rooms,
                ctx,
            ),
        }
    }

    fn divide_large_room(
        &self,
        rooms: Vec<IncompleteFreeSpaceExpansionRoom>,
        board_bounds: &IntBox,
    ) -> Vec<IncompleteFreeSpaceExpansionRoom> {
        let result = divide_large_room_base(rooms, board_bounds);
        if self.angle() != AngleRestriction::FortyFiveDegree {
            return result;
        }
        // ShapeSearchTree45Degree.java:291-297: `super.divideLargeRoom`, then both shapes of
        // every surviving room are replaced by their bounding octagon **in place**.
        let mut result = result;
        for current_room in &mut result {
            let shape = current_room
                .get_shape()
                .and_then(TileShape::bounding_octagon)
                .unwrap_or_else(|| {
                    panic!(
                        "ShapeSearchTree45Degree.divideLargeRoom: a room with no bounding octagon \
                         — Java NPEs at ShapeSearchTree45Degree.java:294"
                    )
                });
            current_room.set_shape(Some(TileShape::Octagon(shape)));
            let contained = current_room
                .get_contained_shape()
                .and_then(TileShape::bounding_octagon)
                .unwrap_or_else(|| {
                    panic!(
                        "ShapeSearchTree45Degree.divideLargeRoom: a room whose contained shape has \
                         no bounding octagon — Java NPEs at ShapeSearchTree45Degree.java:295"
                    )
                });
            current_room.set_contained_shape(Some(TileShape::Octagon(contained)));
        }
        result
    }
}

// =================================================================================================
// `ShapeSearchTree.divideLargeRoom` (ShapeSearchTree.java:1095-1118)
// =================================================================================================

fn divide_large_room_base(
    room_list: Vec<IncompleteFreeSpaceExpansionRoom>,
    board_bounding_box: &IntBox,
) -> Vec<IncompleteFreeSpaceExpansionRoom> {
    // ShapeSearchTree.java:1097-1099.
    if room_list.len() != 1 {
        return room_list;
    }
    let current_room = &room_list[0];
    let room_bounding_box = current_room
        .get_shape()
        .unwrap_or_else(|| {
            panic!(
                "ShapeSearchTree.divideLargeRoom: a room with no shape — Java NPEs at \
                 ShapeSearchTree.java:1101"
            )
        })
        .bounding_box();
    // ShapeSearchTree.java:1102-1105. Java's `2 *` is `int` arithmetic and wraps; a board bigger
    // than 2^30 units in one direction would flip the comparison in Java too.
    if room_bounding_box.height().wrapping_mul(2) <= board_bounding_box.height()
        || room_bounding_box.width().wrapping_mul(2) <= board_bounding_box.width()
    {
        return room_list;
    }
    // ShapeSearchTree.java:1106.
    let max_section_width =
        0.5 * f64::from(board_bounding_box.height().max(board_bounding_box.width()));
    let section_arr = current_room
        .get_shape()
        .expect("checked above")
        .divide_into_sections(max_section_width);
    // ShapeSearchTree.java:1108-1116.
    let mut result = Vec::with_capacity(section_arr.len());
    for current_section in section_arr {
        let current_shape_to_be_contained =
            current_section.intersection(current_room.get_contained_shape().unwrap_or_else(|| {
                panic!(
                    "ShapeSearchTree.divideLargeRoom: a room with no contained shape — Java NPEs \
                     at ShapeSearchTree.java:1111"
                )
            }));
        result.push(IncompleteFreeSpaceExpansionRoom::new(
            Some(current_section),
            current_room.get_layer(),
            Some(current_shape_to_be_contained),
        ));
    }
    result
}

// =================================================================================================
// Resolving a stored `SearchTreeObject`
//
// Java asks the object; the port asks the item list or the room store, because `fr-board` cannot
// name a room. The three answers below are exactly the three `SearchTreeObject` methods
// `completeShape` calls: `isTraceObstacle(int)`, `shapeLayer(int)` and `getTreeShape(tree, int)`.
// =================================================================================================

/// `currentObject.isTraceObstacle(netNumber)` (ShapeSearchTree.java:637).
///
/// `CompleteFreeSpaceExpansionRoom.isTraceObstacle` is the constant `true`
/// (CompleteFreeSpaceExpansionRoom.java:81-84) — a room obstructs every net.
fn object_is_trace_obstacle(object: TreeObject, net_no: i32, items: &impl ItemLookup) -> bool {
    match object {
        TreeObject::Item(id) => item_of(items, id).is_trace_obstacle(net_no),
        TreeObject::Room(_) => true,
    }
}

/// `currentObject.shapeLayer(shapeIndex)` (ShapeSearchTree.java:638).
///
/// `CompleteFreeSpaceExpansionRoom.shapeLayer` ignores the index and answers the room's own layer
/// (CompleteFreeSpaceExpansionRoom.java:71-74).
fn object_shape_layer(
    object: TreeObject,
    shape_index: usize,
    items: &impl ItemLookup,
    rooms: &ExpansionRoomStore,
    ctx: &ItemCtx<'_>,
) -> usize {
    match object {
        TreeObject::Item(id) => item_of(items, id).shape_layer(shape_index, ctx),
        TreeObject::Room(id) => room_of(rooms, id).get_layer(),
    }
}

/// `currentObject.getTreeShape(this, shapeIndex)` (ShapeSearchTree.java:642).
///
/// `CompleteFreeSpaceExpansionRoom.getTreeShape` ignores both arguments and answers `getShape()`
/// (CompleteFreeSpaceExpansionRoom.java:66-69).
fn object_tree_shape(
    tree: &ShapeSearchTree,
    object: TreeObject,
    shape_index: usize,
    items: &impl ItemLookup,
    rooms: &ExpansionRoomStore,
    ctx: &ItemCtx<'_>,
) -> TileShape {
    match object {
        TreeObject::Item(id) => tree
            .get_tree_shape(item_of(items, id), shape_index, ctx)
            .map(std::borrow::Cow::into_owned)
            .unwrap_or_else(|| {
                // Java hands the `null` straight to `intersection`/`boundingOctagon` and throws.
                panic!(
                    "ShapeSearchTree.completeShape: item {id} has a leaf for shape {shape_index} \
                     but no shape for it — Java NPEs here too"
                )
            }),
        TreeObject::Room(id) => room_of(rooms, id)
            .get_shape()
            .unwrap_or_else(|| {
                panic!(
                    "ShapeSearchTree.completeShape: expansion room {id:?} has a tree leaf but no \
                     shape — Java NPEs here too"
                )
            })
            .clone(),
    }
}

fn item_of(items: &impl ItemLookup, id: fr_board::ItemId) -> &fr_board::Item {
    items.item(id).unwrap_or_else(|| {
        panic!("ShapeSearchTree.completeShape: item {id} has a leaf but is not in the item list")
    })
}

/// The live reference Java holds across `isTraceObstacle` (`:637`), `shapeLayer` (`:638`) and
/// `getTreeShape` (`:642`), which a [`TreeObject`] key would make three arena probes.
///
/// The three free helpers above answer the same three questions from a key and remain the
/// definition of what the methods here must answer; [`complete_shape_base`] and
/// [`complete_shape_90`] use those, [`complete_shape_45`] uses this.
enum ResolvedTreeObject<'a> {
    Item(fr_board::ItemId, &'a fr_board::Item),
    Room(
        fr_board::RoomId,
        &'a crate::autoroute::expansion::CompleteFreeSpaceExpansionRoom,
    ),
}

impl<'a> ResolvedTreeObject<'a> {
    fn resolve(
        object: TreeObject,
        items: &'a impl ItemLookup,
        rooms: &'a ExpansionRoomStore,
    ) -> ResolvedTreeObject<'a> {
        match object {
            TreeObject::Item(id) => ResolvedTreeObject::Item(id, item_of(items, id)),
            TreeObject::Room(id) => ResolvedTreeObject::Room(id, room_of(rooms, id)),
        }
    }

    fn is_trace_obstacle(&self, net_no: i32) -> bool {
        match self {
            ResolvedTreeObject::Item(_, item) => item.is_trace_obstacle(net_no),
            ResolvedTreeObject::Room(_, _) => true,
        }
    }

    fn shape_layer(&self, shape_index: usize, ctx: &ItemCtx<'_>) -> usize {
        match self {
            ResolvedTreeObject::Item(_, item) => item.shape_layer(shape_index, ctx),
            ResolvedTreeObject::Room(_, room) => room.get_layer(),
        }
    }

    fn tree_shape(
        &self,
        tree: &ShapeSearchTree,
        shape_index: usize,
        ctx: &ItemCtx<'_>,
    ) -> TileShape {
        match self {
            ResolvedTreeObject::Item(id, item) => tree
                .get_tree_shape(item, shape_index, ctx)
                .map(std::borrow::Cow::into_owned)
                .unwrap_or_else(|| {
                    // Java hands the `null` straight to `intersection`/`boundingOctagon` and throws.
                    panic!(
                        "ShapeSearchTree.completeShape: item {id} has a leaf for shape \
                         {shape_index} but no shape for it — Java NPEs here too"
                    )
                }),
            ResolvedTreeObject::Room(id, room) => room
                .get_shape()
                .unwrap_or_else(|| {
                    panic!(
                        "ShapeSearchTree.completeShape: expansion room {id:?} has a tree leaf but \
                         no shape — Java NPEs here too"
                    )
                })
                .clone(),
        }
    }
}

fn room_of(
    rooms: &ExpansionRoomStore,
    id: fr_board::RoomId,
) -> &crate::autoroute::expansion::CompleteFreeSpaceExpansionRoom {
    rooms.complete_room(id).unwrap_or_else(|| {
        panic!(
            "ShapeSearchTree.completeShape: expansion room {id:?} has a tree leaf but no live room \
             in the store — Java would read a stale reference"
        )
    })
}

/// The bounding shape stored on an arena node, whichever variant it is.
fn node_bounds(node: &Node<TreeObject>) -> RegularTileShape {
    match node {
        Node::Inner { bounds, .. } | Node::Leaf { bounds, .. } => *bounds,
        Node::Free { .. } => unreachable!("ShapeTree::node rejects freed slots"),
    }
}

// =================================================================================================
// The base class: `ShapeSearchTree.completeShape` (ShapeSearchTree.java:580-693)
// =================================================================================================

#[allow(clippy::too_many_arguments)]
fn complete_shape_base(
    tree: &ShapeSearchTree,
    room: &IncompleteFreeSpaceExpansionRoom,
    net_number: i32,
    ignore_object: Option<TreeObject>,
    ignore_shape: Option<&TileShape>,
    items: &impl ItemLookup,
    rooms: &ExpansionRoomStore,
    ctx: &ItemCtx<'_>,
) -> Vec<IncompleteFreeSpaceExpansionRoom> {
    // ShapeSearchTree.java:585-588 (the `FRLogger.warn` is dropped).
    let Some(contained_shape) = room.get_contained_shape() else {
        return Vec::new();
    };
    // ShapeSearchTree.java:589-591.
    let Some(root) = tree.tree().root() else {
        return Vec::new();
    };
    // ShapeSearchTree.java:593-596.
    let mut start_shape = TileShape::Box(*ctx.bounding_box);
    if let Some(shape) = room.get_shape() {
        start_shape = start_shape.intersection(shape);
    }
    // ShapeSearchTree.java:597.
    let mut bounding_shape = tree.tree().bounding_shape(&start_shape).unwrap_or_else(|| {
        panic!(
            "ShapeSearchTree.completeShape: the start shape has no bound in this tree's \
             directions — Java NPEs at ShapeSearchTree.java:617"
        )
    });
    // ShapeSearchTree.java:598-605.
    let mut result: Vec<IncompleteFreeSpaceExpansionRoom> = Vec::new();
    if start_shape.dimension() == 2 {
        result.push(IncompleteFreeSpaceExpansionRoom::new(
            Some(start_shape),
            room.get_layer(),
            Some(contained_shape.clone()),
        ));
    }

    // ShapeSearchTree.java:607-628: collect every overlapping leaf first, then sort, "to
    // ensure exact algorithmic parity with v1.9" — unlike the two subclasses, which process
    // obstacles inline and let `boundingShape` prune the rest of the traversal.
    let room_layer = room.get_layer();
    let mut overlapping_leaves: Vec<(TreeObject, usize)> = Vec::new();
    let mut node_stack: Vec<NodeId> = vec![root];
    while let Some(current_node) = node_stack.pop() {
        let node = *tree.tree().node(current_node);
        if !node_bounds(&node)
            .to_tile_shape()
            .intersects(&bounding_shape.to_tile_shape())
        {
            continue;
        }
        match node {
            Node::Leaf {
                object,
                shape_index,
                ..
            } => overlapping_leaves.push((object, shape_index)),
            Node::Inner {
                first_child,
                second_child,
                ..
            } => {
                // ShapeSearchTree.java:621-622: first then second, so the second child is
                // popped first.
                node_stack.push(first_child);
                node_stack.push(second_child);
            }
            Node::Free { .. } => unreachable!("ShapeTree::node rejects freed slots"),
        }
    }
    // ShapeSearchTree.java:630: `Collections.sort` on `Leaf.compareTo`
    // (ShapeTree.java:216-223) = the stored object's order, then the shape index — which is
    // exactly the derived `Ord` of `(TreeObject, usize)`. `sort`, not `sort_unstable`, because
    // `Collections.sort` is a stable TimSort — the two can only differ on equal keys, which a
    // consistent tree never has, but the stable one is the transcription.
    overlapping_leaves.sort();

    // ShapeSearchTree.java:632-690.
    for (current_object, shape_index) in overlapping_leaves {
        if !object_is_trace_obstacle(current_object, net_number, items)
            || object_shape_layer(current_object, shape_index, items, rooms, ctx) != room_layer
            || Some(current_object) == ignore_object
        {
            continue;
        }
        let current_object_shape =
            object_tree_shape(tree, current_object, shape_index, items, rooms, ctx);
        let mut new_result: Vec<IncompleteFreeSpaceExpansionRoom> = Vec::new();
        // ShapeSearchTree.java:644: `IntOctagon.EMPTY` as a `RegularTileShape`.
        let mut new_bounding_shape = RegularTileShape::Octagon(IntOctagon::EMPTY);

        for current_incomplete_room in &result {
            let mut something_changed = false;
            let room_shape = current_incomplete_room.get_shape().unwrap_or_else(|| {
                panic!(
                    "ShapeSearchTree.completeShape: an incomplete room with no shape — Java \
                     NPEs at ShapeSearchTree.java:648"
                )
            });
            let intersection = room_shape.intersection(&current_object_shape);
            if intersection.dimension() == 2 {
                // ShapeSearchTree.java:650-652: only a `CompleteFreeSpaceExpansionRoom` can
                // be ignored this way, which is the room half of `TreeObject`.
                let ignore_expansion_room = matches!(current_object, TreeObject::Room(_))
                    && ignore_shape.is_some_and(|shape| shape.contains_tile(&intersection));
                if !ignore_expansion_room {
                    something_changed = true;
                    // ShapeSearchTree.java:670-672.
                    new_result.extend(restrain_shape_base(
                        current_incomplete_room,
                        &current_object_shape,
                    ));
                    // ShapeSearchTree.java:674-679. Note that `boundingShape` is only read by
                    // the traversal above, which has already finished, so this whole
                    // accumulation is dead in the base class — unlike in the two subclasses,
                    // where it prunes. Kept because Java keeps it.
                    for tmp_room in &new_result {
                        new_bounding_shape = union_of(
                            tree,
                            new_bounding_shape,
                            tmp_room.get_shape().expect("just built with a shape"),
                        );
                    }
                }
            }
            if !something_changed {
                // ShapeSearchTree.java:683-687.
                new_result.push(current_incomplete_room.clone());
                new_bounding_shape = union_of(tree, new_bounding_shape, room_shape);
            }
        }
        result = new_result;
        bounding_shape = new_bounding_shape;
    }
    let _ = bounding_shape;
    // ShapeSearchTree.java:692.
    tree.divide_large_room(result, ctx.bounding_box)
}

/// `accumulated.union(shape.boundingShape(this.boundingDirections))` (ShapeSearchTree.java:676-678
/// and :685-687).
fn union_of(
    tree: &ShapeSearchTree,
    accumulated: RegularTileShape,
    shape: &TileShape,
) -> RegularTileShape {
    match tree.tree().bounding_shape(shape) {
        Some(bounds) => accumulated.union(&bounds),
        // Java's `boundingShape` answers `null` for an unbounded simplex and `union` throws.
        None => panic!(
            "ShapeSearchTree.completeShape: a restrained room has no bound in this tree's \
             directions — Java NPEs at ShapeSearchTree.java:677"
        ),
    }
}

/// Port of the private `ShapeSearchTree.restrainShape` (ShapeSearchTree.java:701-811).
///
/// "Restrains the shape of `incompleteRoom` to a `TileShape` which does not intersect with the
/// interior of `obstacleShape`. `incompleteRoom.getContainedShape()` must be contained in the
/// shape of the result room. If that is not possible, several rooms are returned with shapes
/// which intersect with `incompleteRoom.getContainedShape()`."
fn restrain_shape_base(
    incomplete_room: &IncompleteFreeSpaceExpansionRoom,
    obstacle_shape: &TileShape,
) -> Vec<IncompleteFreeSpaceExpansionRoom> {
    let mut result: Vec<IncompleteFreeSpaceExpansionRoom> = Vec::new();
    // ShapeSearchTree.java:713: "always convert to Simplex to match v1.9 semantics — otherwise
    // border_lines of length 0 for octagons may not be handled correctly".
    let obstacle_simplex = TileShape::Simplex(obstacle_shape.to_simplex());

    // ShapeSearchTree.java:715-721: the contained shape is converted too, and the **converted**
    // one is what the result rooms carry.
    let shape_to_be_contained = incomplete_room
        .get_contained_shape()
        .map(|shape| TileShape::Simplex(shape.to_simplex()));
    let room_shape = incomplete_room.get_shape();
    // ShapeSearchTree.java:723-726.
    let Some(shape_to_be_contained) = shape_to_be_contained else {
        return result;
    };
    if shape_to_be_contained.is_empty() {
        return result;
    }
    let layer = incomplete_room.get_layer();

    // ShapeSearchTree.java:731-748: the border line of `obstacleShape` whose segment cuts the
    // interior of the room and which is furthest from the contained shape.
    let mut cut_line: Option<Line> = None;
    let mut cut_line_distance = -1.0_f64;
    for i in 0..obstacle_simplex.border_line_count() {
        if !room_shape_is_intersected(room_shape, &obstacle_simplex, i) {
            continue;
        }
        let current_line = obstacle_simplex
            .border_line(i)
            .expect("i is below border_line_count");
        let current_min_distance = shape_to_be_contained.distance_to_the_left(&current_line);
        if current_min_distance > cut_line_distance {
            cut_line_distance = current_min_distance;
            cut_line = Some(current_line.opposite());
        }
    }

    if let Some(cut_line) = cut_line {
        // ShapeSearchTree.java:750-757.
        let mut result_piece = TileShape::get_instance_from_line(cut_line);
        if let Some(room_shape) = room_shape {
            result_piece = room_shape.intersection(&result_piece);
        }
        if result_piece.dimension() >= 2 {
            result.push(IncompleteFreeSpaceExpansionRoom::new(
                Some(result_piece),
                layer,
                Some(shape_to_be_contained),
            ));
        }
        return result;
    }

    // ShapeSearchTree.java:758-765: no line has all of the contained shape on its right, so look
    // for one that has *part* of it there.
    if shape_to_be_contained.dimension() < 1 {
        // "There is already a completed expansion room around shapeToBeContained."
        return result;
    }

    // ShapeSearchTree.java:767-778.
    let mut cut_line: Option<Line> = None;
    for i in 0..obstacle_simplex.border_line_count() {
        if !room_shape_is_intersected(room_shape, &obstacle_simplex, i) {
            continue;
        }
        let current_line = obstacle_simplex
            .border_line(i)
            .expect("i is below border_line_count");
        if shape_to_be_contained.side_of_line(&current_line) == Side::Collinear {
            // The line intersects the interior of the contained shape.
            cut_line = Some(current_line.opposite());
            break;
        }
    }
    let Some(cut_line) = cut_line else {
        // ShapeSearchTree.java:780-784: "cut line not found, parts or the whole of shape may be
        // already occupied from somewhere else."
        return result;
    };

    // ShapeSearchTree.java:786-793.
    let cut_half_plane = TileShape::get_instance_from_line(cut_line);
    let new_shape_to_be_contained = shape_to_be_contained.intersection(&cut_half_plane);
    let result_piece = match room_shape {
        None => cut_half_plane.clone(),
        Some(room_shape) => room_shape.intersection(&cut_half_plane),
    };
    if result_piece.dimension() >= 2 {
        result.push(IncompleteFreeSpaceExpansionRoom::new(
            Some(result_piece),
            layer,
            Some(new_shape_to_be_contained),
        ));
    }
    // ShapeSearchTree.java:795-809: the other half plane is restrained again, recursively.
    let opposite_half_plane = TileShape::get_instance_from_line(cut_line.opposite());
    let rest_piece = match room_shape {
        None => opposite_half_plane.clone(),
        Some(room_shape) => room_shape.intersection(&opposite_half_plane),
    };
    if rest_piece.dimension() >= 2 {
        let rest_shape_to_be_contained = shape_to_be_contained.intersection(&opposite_half_plane);
        let rest_incomplete_room = IncompleteFreeSpaceExpansionRoom::new(
            Some(rest_piece),
            layer,
            Some(rest_shape_to_be_contained),
        );
        result.extend(restrain_shape_base(&rest_incomplete_room, obstacle_shape));
    }
    result
}

/// `roomShape.isIntersectedInteriorBy(new LineSegment(obstacleSimplex, i))`
/// (ShapeSearchTree.java:733 and :771).
///
/// Java dereferences `roomShape` without a guard here even though it guards it thirty lines
/// later (`:751`, `:788`), so a whole-plane room throws before it can reach either guard.
fn room_shape_is_intersected(
    room_shape: Option<&TileShape>,
    obstacle_simplex: &TileShape,
    line_no: usize,
) -> bool {
    let room_shape = room_shape.unwrap_or_else(|| {
        panic!(
            "ShapeSearchTree.restrainShape: a whole-plane room reaches \
             isIntersectedInteriorBy — Java NPEs at ShapeSearchTree.java:733"
        )
    });
    match LineSegment::from_tile_shape(obstacle_simplex, line_no) {
        Some(segment) => room_shape.is_intersected_interior_by(&segment),
        None => false,
    }
}

// =================================================================================================
// The 45-degree override: `ShapeSearchTree45Degree.completeShape` (…45Degree.java:95-281)
// =================================================================================================

#[allow(clippy::too_many_arguments)]
fn complete_shape_45(
    tree: &ShapeSearchTree,
    room: &IncompleteFreeSpaceExpansionRoom,
    net_number: i32,
    ignore_object: Option<TreeObject>,
    ignore_shape: Option<&TileShape>,
    items: &impl ItemLookup,
    rooms: &ExpansionRoomStore,
    ctx: &ItemCtx<'_>,
) -> Vec<IncompleteFreeSpaceExpansionRoom> {
    // ShapeSearchTree45Degree.java:101-107.
    let Some(contained_raw) = room.get_contained_shape() else {
        return Vec::new();
    };
    // :108-115 is a `FRLogger.debug` and its guard; the bounding octagon below is taken
    // whether or not the contained shape already is one.
    let Some(shape_to_be_contained) = contained_raw.bounding_octagon() else {
        // :117-126: `boundingOctagon()` answers null for an empty or degenerate shape.
        return Vec::new();
    };
    // :128-130.
    let Some(root) = tree.tree().root() else {
        return Vec::new();
    };
    // :132-140.
    let mut start_shape = ctx.bounding_box.bounding_octagon();
    if let Some(shape) = room.get_shape() {
        // `instanceof IntOctagon` is a **type** test, so an `IntBox` room shape is rejected
        // here even though it is geometrically an octagon.
        let TileShape::Octagon(_) = shape else {
            return Vec::new();
        };
        start_shape = shape
            .bounding_octagon()
            .expect("an IntOctagon has a bounding octagon")
            .intersection(&start_shape);
    }

    if p7t14b_fp_ledger() {
        p7t14b_treefp(tree, root);
    }
    // :142-150.
    let mut bounding_shape = start_shape;
    let room_layer = room.get_layer();
    let mut result = vec![IncompleteFreeSpaceExpansionRoom::new(
        Some(TileShape::Octagon(start_shape)),
        room_layer,
        Some(TileShape::Octagon(shape_to_be_contained)),
    )];
    let mut node_stack: Vec<NodeId> = vec![root];
    // `:186`'s per-obstacle list, hoisted so it and `result` can swap at `:263-264`.
    let mut new_result: Vec<IncompleteFreeSpaceExpansionRoom> = Vec::new();

    // :152-274.
    while let Some(current_node) = node_stack.pop() {
        let node = *tree.tree().node(current_node);
        if !node_bounds(&node)
            .to_tile_shape()
            .intersects_octagon(&bounding_shape)
        {
            continue;
        }
        let (current_object, shape_index) = match node {
            Node::Leaf {
                object,
                shape_index,
                ..
            } => (object, shape_index),
            Node::Inner {
                first_child,
                second_child,
                ..
            } => {
                // :270-271.
                node_stack.push(first_child);
                node_stack.push(second_child);
                continue;
            }
            Node::Free { .. } => unreachable!("ShapeTree::node rejects freed slots"),
        };

        // :160-178.
        let resolved = ResolvedTreeObject::resolve(current_object, items, rooms);
        if !(resolved.is_trace_obstacle(net_number)
            && resolved.shape_layer(shape_index, ctx) == room_layer
            && Some(current_object) != ignore_object)
        {
            continue;
        }

        // :180-181.
        let current_object_shape = resolved
            .tree_shape(tree, shape_index, ctx)
            .bounding_octagon()
            .unwrap_or_else(|| {
                panic!(
                    "ShapeSearchTree45Degree.completeShape: an obstacle shape with no \
                     bounding octagon — Java NPEs at ShapeSearchTree45Degree.java:191"
                )
            });
        if p7t14b_cs_ledger() {
            let kind = match current_object {
                TreeObject::Room(_) => "CompleteFreeSpaceExpansionRoom",
                TreeObject::Item(_) => "Item",
            };
            let mut line = format!(
                "CSO obj={kind} idx={shape_index} oct={} rooms={}",
                p7t14b_oct(&current_object_shape),
                result.len()
            );
            for room in &result {
                let octagon = room
                    .get_shape()
                    .and_then(TileShape::bounding_octagon)
                    .unwrap_or(IntOctagon::EMPTY);
                line.push(' ');
                line.push_str(&p7t14b_oct(&octagon));
            }
            eprintln!("{line}");
        }
        // :186-247.
        new_result.clear();
        let mut new_bounding_shape = IntOctagon::EMPTY;
        for current_room in &result {
            let current_shape = match current_room.get_shape() {
                Some(TileShape::Octagon(shape)) => *shape,
                // `(IntOctagon) currentRoom.getShape()` at :190 — a cast, so anything else is
                // a `ClassCastException`.
                _ => panic!(
                    "ShapeSearchTree45Degree.completeShape: a room whose shape is not an \
                     IntOctagon — Java throws ClassCastException at \
                     ShapeSearchTree45Degree.java:190"
                ),
            };
            if !current_shape.overlaps(&current_object_shape) {
                // :233-246.
                new_result.push(current_room.clone());
                new_bounding_shape = new_bounding_shape.union_box(&current_shape.bounding_box());
                continue;
            }
            // :193-215.
            if matches!(current_object, TreeObject::Room(_))
                && let Some(ignore_shape) = ignore_shape
            {
                let intersection = current_shape.intersection(&current_object_shape);
                if ignore_shape.contains_tile(&TileShape::Octagon(intersection)) {
                    // "ignore also all objects whose intersection is contained in the 2-dim
                    // overlap-door with the fromRoom" — but keep the room itself unless the
                    // ignore shape swallows it whole (:209-212). The 90-degree override drops
                    // it either way; this one does not.
                    if !ignore_shape.contains_tile(&TileShape::Octagon(current_shape)) {
                        new_result.push(current_room.clone());
                        new_bounding_shape =
                            new_bounding_shape.union_box(&current_shape.bounding_box());
                    }
                    continue;
                }
            }
            // :226-232. Java folds the whole of `newResult` on every iteration of `:189`.
            // `IntOctagon::union` is per-coordinate `min`/`max` over a field-wise
            // [`IntOctagon::new`] — no normalisation — hence idempotent, commutative and
            // associative, and everything below `appended_from` is already in
            // `new_bounding_shape`, so folding only the tail reaches the same octagon.
            let appended_from = new_result.len();
            new_result.extend(restrain_shape_45(current_room, &current_object_shape));
            for tmp_shape in &new_result[appended_from..] {
                new_bounding_shape = new_bounding_shape.union_box(
                    &tmp_shape
                        .get_shape()
                        .expect("just built with a shape")
                        .bounding_box(),
                );
            }
        }
        // :248-262 is the `COMPLETE_SHAPE_BLOCKED` trace; :263-264 is the state update.
        std::mem::swap(&mut result, &mut new_result);
        bounding_shape = new_bounding_shape;
    }

    // :276.
    let mut result = tree.divide_large_room(result, ctx.bounding_box);
    // :277-279: "remove rooms with shapes equal to the contained shape to prevent endless
    // loop."
    result.retain(|expansion_room| {
        let contained = expansion_room.get_contained_shape().unwrap_or_else(|| {
            panic!(
                "ShapeSearchTree45Degree.completeShape: a room with no contained shape — Java \
                 NPEs at ShapeSearchTree45Degree.java:279"
            )
        });
        let shape = expansion_room.get_shape().unwrap_or_else(|| {
            panic!(
                "ShapeSearchTree45Degree.completeShape: a room with no shape — Java NPEs at \
                 ShapeSearchTree45Degree.java:279"
            )
        });
        !contained.contains_tile(shape)
    });
    result
}

/// Port of the private `ShapeSearchTree45Degree.restrainShape` (…45Degree.java:305-418).
fn restrain_shape_45(
    incomplete_room: &IncompleteFreeSpaceExpansionRoom,
    obstacle_shape: &IntOctagon,
) -> Vec<IncompleteFreeSpaceExpansionRoom> {
    let mut result: Vec<IncompleteFreeSpaceExpansionRoom> = Vec::new();

    // :317-321.
    let Some(contained_shape) = incomplete_room.get_contained_shape() else {
        return result;
    };
    if contained_shape.is_empty() {
        return result;
    }
    // :322-334. The first branch is `isIntOctagon()` — the *geometric* predicate, true for an
    // `IntBox` too — and has no null guard; the second is `instanceof Simplex` and has one.
    let shape_to_be_contained = if contained_shape.is_int_octagon() {
        contained_shape.bounding_octagon().unwrap_or_else(|| {
            panic!(
                "ShapeSearchTree45Degree.restrainShape: an isIntOctagon() shape with no bounding \
                 octagon — Java NPEs at ShapeSearchTree45Degree.java:324"
            )
        })
    } else if matches!(contained_shape, TileShape::Simplex(_)) {
        match contained_shape.bounding_octagon() {
            Some(octagon) => octagon,
            None => return Vec::new(),
        }
    } else {
        return Vec::new();
    };

    // :336-348. `instanceof IntOctagon` is a type test here, so an `IntBox` room shape falls to
    // the `else` and answers the empty list.
    let room_shape = match incomplete_room.get_shape() {
        Some(shape @ TileShape::Octagon(_)) => shape.bounding_octagon().unwrap_or_else(|| {
            panic!(
                "ShapeSearchTree45Degree.restrainShape: an IntOctagon with no bounding octagon — \
                 Java NPEs at ShapeSearchTree45Degree.java:338"
            )
        }),
        Some(shape @ TileShape::Simplex(_)) => match shape.bounding_octagon() {
            Some(octagon) => octagon,
            None => return Vec::new(),
        },
        _ => return Vec::new(),
    };

    // :350-362.
    let mut cut_line_distance = -1.0_f64;
    let mut restraining_line_no: i32 = -1;
    for obstacle_line_no in 0..8usize {
        let current_distance =
            signed_line_distance(obstacle_shape, obstacle_line_no, &shape_to_be_contained);
        if current_distance > cut_line_distance
            && obstacle_segment_touches_inside(obstacle_shape, obstacle_line_no, &room_shape)
        {
            cut_line_distance = current_distance;
            restraining_line_no = obstacle_line_no as i32;
        }
    }
    // :363-370. Note the gap Java leaves open: a restraining line found at a *negative* distance
    // above `-1` sets `restrainingLineNo` but fails this test, and the second loop then runs and
    // may pick a different line.
    if cut_line_distance >= 0.0 {
        let restrained_shape = calc_outside_restrained_shape(
            obstacle_shape,
            restraining_line_no as usize,
            &room_shape,
        );
        result.push(IncompleteFreeSpaceExpansionRoom::new(
            Some(TileShape::Octagon(restrained_shape)),
            incomplete_room.get_layer(),
            Some(TileShape::Octagon(shape_to_be_contained)),
        ));
        return result;
    }

    // :372-378.
    if shape_to_be_contained.dimension() < 1 {
        // "There is already a completed expansion room around shapeToBeContained."
        return result;
    }

    // :380-395.
    let mut restraining_line_no: i32 = -1;
    for obstacle_line_no in 0..8usize {
        if obstacle_segment_touches_inside(obstacle_shape, obstacle_line_no, &room_shape) {
            let current_line = obstacle_shape.border_line(obstacle_line_no);
            if TileShape::Octagon(shape_to_be_contained).side_of_line(&current_line)
                == Side::Collinear
            {
                // The line intersects the interior of the contained shape.
                restraining_line_no = obstacle_line_no as i32;
                break;
            }
        }
    }
    if restraining_line_no < 0 {
        // "cut line not found, parts or the whole of shape may be already occupied from somewhere
        // else."
        return result;
    }

    // :396-405.
    let restrained_shape =
        calc_outside_restrained_shape(obstacle_shape, restraining_line_no as usize, &room_shape);
    if restrained_shape.dimension() == 2 {
        let new_shape_to_be_contained = shape_to_be_contained.intersection(&restrained_shape);
        if new_shape_to_be_contained.dimension() > 0 {
            result.push(IncompleteFreeSpaceExpansionRoom::new(
                Some(TileShape::Octagon(restrained_shape)),
                incomplete_room.get_layer(),
                Some(TileShape::Octagon(new_shape_to_be_contained)),
            ));
        }
    }

    // :407-416.
    let rest_piece =
        calc_inside_restrained_shape(obstacle_shape, restraining_line_no as usize, &room_shape);
    if rest_piece.dimension() >= 2 {
        let rest_shape_to_be_contained = shape_to_be_contained.intersection(&rest_piece);
        if rest_shape_to_be_contained.dimension() >= 0 {
            let rest_incomplete_room = IncompleteFreeSpaceExpansionRoom::new(
                Some(TileShape::Octagon(rest_piece)),
                incomplete_room.get_layer(),
                Some(TileShape::Octagon(rest_shape_to_be_contained)),
            );
            result.extend(restrain_shape_45(&rest_incomplete_room, obstacle_shape));
        }
    }
    result
}

/// Port of the private static `ShapeSearchTree45Degree.obstacleSegmentTouchesInside`
/// (…45Degree.java:38-65): "checks if the border line segment with index `obstacleBorderLineNo`
/// intersects with the inside of `roomShape`".
fn obstacle_segment_touches_inside(
    obstacle_shape: &IntOctagon,
    obstacle_border_line_no: usize,
    room_shape: &IntOctagon,
) -> bool {
    let mut current_border_line_no = obstacle_border_line_no;
    let current_obstacle_corner_x = obstacle_shape.corner_x(obstacle_border_line_no);
    let current_obstacle_corner_y = obstacle_shape.corner_y(obstacle_border_line_no);
    for _ in 0..5 {
        if room_shape.side_of_border_line(
            current_obstacle_corner_x,
            current_obstacle_corner_y,
            current_border_line_no,
        ) != Side::OnTheLeft
        {
            return false;
        }
        current_border_line_no = (current_border_line_no + 1) % 8;
    }

    let next_obstacle_border_line_no = (obstacle_border_line_no + 1) % 8;
    let next_obstacle_corner_x = obstacle_shape.corner_x(next_obstacle_border_line_no);
    let next_obstacle_corner_y = obstacle_shape.corner_y(next_obstacle_border_line_no);
    let mut current_border_line_no = (obstacle_border_line_no + 5) % 8;
    for _ in 0..3 {
        if room_shape.side_of_border_line(
            next_obstacle_corner_x,
            next_obstacle_corner_y,
            current_border_line_no,
        ) != Side::OnTheLeft
        {
            return false;
        }
        current_border_line_no = (current_border_line_no + 1) % 8;
    }
    true
}

/// Port of the private static `ShapeSearchTree45Degree.signedLineDistance`
/// (…45Degree.java:67-86).
///
/// The four diagonal cases carry a factor `0.5` "used instead of `1 / sqrt(2)` to prefer
/// orthogonal lines slightly to diagonal restraining lines" — Java's own comment. Each
/// subtraction is `int` arithmetic *before* the widening to `double`, so it wraps.
fn signed_line_distance(
    obstacle_shape: &IntOctagon,
    obstacle_line_no: usize,
    contained_shape: &IntOctagon,
) -> f64 {
    match obstacle_line_no {
        0 => f64::from(obstacle_shape.bottom_y.wrapping_sub(contained_shape.top_y)),
        2 => f64::from(contained_shape.left_x.wrapping_sub(obstacle_shape.right_x)),
        4 => f64::from(contained_shape.bottom_y.wrapping_sub(obstacle_shape.top_y)),
        6 => f64::from(obstacle_shape.left_x.wrapping_sub(contained_shape.right_x)),
        1 => {
            0.5 * f64::from(
                contained_shape
                    .upper_left_diagonal_x
                    .wrapping_sub(obstacle_shape.lower_right_diagonal_x),
            )
        }
        3 => {
            0.5 * f64::from(
                contained_shape
                    .lower_left_diagonal_x
                    .wrapping_sub(obstacle_shape.upper_right_diagonal_x),
            )
        }
        5 => {
            0.5 * f64::from(
                obstacle_shape
                    .upper_left_diagonal_x
                    .wrapping_sub(contained_shape.lower_right_diagonal_x),
            )
        }
        7 => {
            0.5 * f64::from(
                obstacle_shape
                    .lower_left_diagonal_x
                    .wrapping_sub(contained_shape.upper_right_diagonal_x),
            )
        }
        // :81-84: the `FRLogger.warn` is dropped, the `yield 0` is not. Unreachable from the two
        // callers above, which both iterate `0..8`.
        _ => 0.0,
    }
}

/// Port of `ShapeSearchTree45Degree.calcOutsideRestrainedShape` (…45Degree.java:424-452):
/// "intersects `roomShape` with the half plane defined by the **outside** of the border line with
/// index `obstacleLineNo` of `obstacleShape`".
fn calc_outside_restrained_shape(
    obstacle_shape: &IntOctagon,
    obstacle_line_no: usize,
    room_shape: &IntOctagon,
) -> IntOctagon {
    let mut result = *room_shape;
    match obstacle_line_no {
        0 => result.top_y = obstacle_shape.bottom_y,
        2 => result.left_x = obstacle_shape.right_x,
        4 => result.bottom_y = obstacle_shape.top_y,
        6 => result.right_x = obstacle_shape.left_x,
        1 => result.upper_left_diagonal_x = obstacle_shape.lower_right_diagonal_x,
        3 => result.lower_left_diagonal_x = obstacle_shape.upper_right_diagonal_x,
        5 => result.lower_right_diagonal_x = obstacle_shape.upper_left_diagonal_x,
        7 => result.upper_right_diagonal_x = obstacle_shape.lower_left_diagonal_x,
        // :444-447: the `FRLogger.warn` is dropped and the untouched room shape is normalized.
        _ => {}
    }
    result.normalize()
}

/// Port of `ShapeSearchTree45Degree.calcInsideRestrainedShape` (…45Degree.java:458-486):
/// "intersects `roomShape` with the half plane defined by the **inside** of the border line with
/// index `obstacleLineNo` of `obstacleShape`".
fn calc_inside_restrained_shape(
    obstacle_shape: &IntOctagon,
    obstacle_line_no: usize,
    room_shape: &IntOctagon,
) -> IntOctagon {
    let mut result = *room_shape;
    match obstacle_line_no {
        0 => result.bottom_y = obstacle_shape.bottom_y,
        2 => result.right_x = obstacle_shape.right_x,
        4 => result.top_y = obstacle_shape.top_y,
        6 => result.left_x = obstacle_shape.left_x,
        1 => result.lower_right_diagonal_x = obstacle_shape.lower_right_diagonal_x,
        3 => result.upper_right_diagonal_x = obstacle_shape.upper_right_diagonal_x,
        5 => result.upper_left_diagonal_x = obstacle_shape.upper_left_diagonal_x,
        7 => result.lower_left_diagonal_x = obstacle_shape.lower_left_diagonal_x,
        _ => {}
    }
    result.normalize()
}

// =================================================================================================
// The 90-degree override: `ShapeSearchTree90Degree.completeShape` (…90Degree.java:38-191)
// =================================================================================================

#[allow(clippy::too_many_arguments)]
fn complete_shape_90(
    tree: &ShapeSearchTree,
    room: &IncompleteFreeSpaceExpansionRoom,
    net_number: i32,
    ignore_object: Option<TreeObject>,
    ignore_shape: Option<&TileShape>,
    items: &impl ItemLookup,
    rooms: &ExpansionRoomStore,
    ctx: &ItemCtx<'_>,
) -> Vec<IncompleteFreeSpaceExpansionRoom> {
    // :44-47: `instanceof IntBox` — a type test, so a `Simplex` that happens to be a box is
    // rejected.
    let Some(TileShape::Box(shape_to_be_contained)) = room.get_contained_shape() else {
        return Vec::new();
    };
    // :48-50.
    let Some(root) = tree.tree().root() else {
        return Vec::new();
    };
    // :51-58.
    let mut start_shape = *ctx.bounding_box;
    if let Some(shape) = room.get_shape() {
        let TileShape::Box(room_box) = shape else {
            return Vec::new();
        };
        start_shape = room_box.intersection(&start_shape);
    }
    // :59-64.
    let mut bounding_shape = start_shape;
    let room_layer = room.get_layer();
    let mut result = vec![IncompleteFreeSpaceExpansionRoom::new(
        Some(TileShape::Box(start_shape)),
        room_layer,
        Some(TileShape::Box(*shape_to_be_contained)),
    )];

    // :66-189: obstacles are processed inline, so the shrinking `boundingShape` prunes the
    // rest of the traversal.
    let mut node_stack: Vec<NodeId> = vec![root];
    while let Some(current_node) = node_stack.pop() {
        let node = *tree.tree().node(current_node);
        if !node_bounds(&node)
            .to_tile_shape()
            .intersects_box(&bounding_shape)
        {
            continue;
        }
        let (current_object, shape_index) = match node {
            Node::Leaf {
                object,
                shape_index,
                ..
            } => (object, shape_index),
            Node::Inner {
                first_child,
                second_child,
                ..
            } => {
                // :185-186.
                node_stack.push(first_child);
                node_stack.push(second_child);
                continue;
            }
            Node::Free { .. } => unreachable!("ShapeTree::node rejects freed slots"),
        };

        // :82-98.
        let is_obstacle = object_is_trace_obstacle(current_object, net_number, items);
        let same_layer =
            object_shape_layer(current_object, shape_index, items, rooms, ctx) == room_layer;
        let ignored_object = Some(current_object) == ignore_object;
        if !(is_obstacle && same_layer && !ignored_object) {
            continue;
        }

        // :100.
        let current_object_shape =
            object_tree_shape(tree, current_object, shape_index, items, rooms, ctx).bounding_box();
        // :105-162.
        let mut new_result: Vec<IncompleteFreeSpaceExpansionRoom> = Vec::new();
        let mut new_bounding_shape = IntBox::EMPTY;
        for current_room in &result {
            let current_shape = match current_room.get_shape() {
                Some(TileShape::Box(shape)) => *shape,
                // `(IntBox) currentRoom.getShape()` at :109.
                _ => panic!(
                    "ShapeSearchTree90Degree.completeShape: a room whose shape is not an \
                     IntBox — Java throws ClassCastException at \
                     ShapeSearchTree90Degree.java:109"
                ),
            };
            if !current_shape.overlaps(&current_object_shape) {
                // :148-161.
                new_result.push(current_room.clone());
                new_bounding_shape = new_bounding_shape.union(&current_shape.bounding_box());
                continue;
            }
            // :112-130.
            // Java bug: ShapeSearchTree90Degree.completeShape drops the room here instead of
            // keeping it (quirk #159). The base class falls through to
            // `if (!somethingChanged) newResult.add(currentIncompleteRoom)`
            // (ShapeSearchTree.java:683-687) and the 45-degree override re-adds it explicitly
            // unless the ignore shape swallows it whole (…45Degree.java:209-212); this one has
            // no re-add before the `continue`, so the room vanishes from the result.
            if matches!(current_object, TreeObject::Room(_))
                && let Some(ignore_shape) = ignore_shape
            {
                let intersection = current_shape.intersection(&current_object_shape);
                if ignore_shape.contains_tile(&TileShape::Box(intersection)) {
                    continue;
                }
            }
            // :141-147.
            new_result.extend(restrain_shape_90(current_room, &current_object_shape));
            for tmp_shape in &new_result {
                new_bounding_shape = new_bounding_shape.union(
                    &tmp_shape
                        .get_shape()
                        .expect("just built with a shape")
                        .bounding_box(),
                );
            }
        }
        // :163-177 is the `COMPLETE_SHAPE_BLOCKED` trace; :178-179 is the state update.
        result = new_result;
        bounding_shape = new_bounding_shape;
    }
    // :190. This regime never calls `divideLargeRoom`.
    result
}

/// Port of the private `ShapeSearchTree90Degree.restrainShape` (…90Degree.java:198-322).
// The fourth `cutLineDistance = currentDistance` (…90Degree.java:267) is dead in Java too — it is
// the last of the four candidate borders and nothing reads the variable afterwards. Transcribed
// rather than dropped, so the four blocks stay line-for-line identical to Java's.
#[allow(unused_assignments)]
fn restrain_shape_90(
    incomplete_room: &IncompleteFreeSpaceExpansionRoom,
    obstacle_shape: &IntBox,
) -> Vec<IncompleteFreeSpaceExpansionRoom> {
    let mut result: Vec<IncompleteFreeSpaceExpansionRoom> = Vec::new();

    // :210-214.
    let Some(contained_shape) = incomplete_room.get_contained_shape() else {
        return result;
    };
    if contained_shape.is_empty() {
        return result;
    }
    // :215-216.
    let room_shape = incomplete_room
        .get_shape()
        .unwrap_or_else(|| {
            panic!(
                "ShapeSearchTree90Degree.restrainShape: a whole-plane room — Java NPEs at \
                 ShapeSearchTree90Degree.java:215"
            )
        })
        .bounding_box();
    let shape_to_be_contained = contained_shape.bounding_box();
    let mut cut_line_distance: i32 = 0;
    let mut restrained_shape: Option<IntBox> = None;

    // :220-232: the right border of the obstacle cuts the interior of the room.
    if room_shape.ll.x < obstacle_shape.ur.x
        && room_shape.ur.x > obstacle_shape.ur.x
        && room_shape.ur.y > obstacle_shape.ll.y
        && room_shape.ll.y < obstacle_shape.ur.y
    {
        let current_distance = shape_to_be_contained.ll.x.wrapping_sub(obstacle_shape.ur.x);
        if current_distance > cut_line_distance {
            cut_line_distance = current_distance;
            restrained_shape = Some(IntBox::from_coords(
                obstacle_shape.ur.x,
                room_shape.ll.y,
                room_shape.ur.x,
                room_shape.ur.y,
            ));
        }
    }
    // :233-245: the left border.
    if room_shape.ll.x < obstacle_shape.ll.x
        && room_shape.ur.x > obstacle_shape.ll.x
        && room_shape.ur.y > obstacle_shape.ll.y
        && room_shape.ll.y < obstacle_shape.ur.y
    {
        let current_distance = obstacle_shape.ll.x.wrapping_sub(shape_to_be_contained.ur.x);
        if current_distance > cut_line_distance {
            cut_line_distance = current_distance;
            restrained_shape = Some(IntBox::from_coords(
                room_shape.ll.x,
                room_shape.ll.y,
                obstacle_shape.ll.x,
                room_shape.ur.y,
            ));
        }
    }
    // :246-258: the lower border.
    if room_shape.ll.y < obstacle_shape.ll.y
        && room_shape.ur.y > obstacle_shape.ll.y
        && room_shape.ur.x > obstacle_shape.ll.x
        && room_shape.ll.x < obstacle_shape.ur.x
    {
        let current_distance = obstacle_shape.ll.y.wrapping_sub(shape_to_be_contained.ur.y);
        if current_distance > cut_line_distance {
            cut_line_distance = current_distance;
            restrained_shape = Some(IntBox::from_coords(
                room_shape.ll.x,
                room_shape.ll.y,
                room_shape.ur.x,
                obstacle_shape.ll.y,
            ));
        }
    }
    // :259-271: the upper border.
    if room_shape.ll.y < obstacle_shape.ur.y
        && room_shape.ur.y > obstacle_shape.ur.y
        && room_shape.ur.x > obstacle_shape.ll.x
        && room_shape.ll.x < obstacle_shape.ur.x
    {
        let current_distance = shape_to_be_contained.ll.y.wrapping_sub(obstacle_shape.ur.y);
        if current_distance > cut_line_distance {
            cut_line_distance = current_distance;
            restrained_shape = Some(IntBox::from_coords(
                room_shape.ll.x,
                obstacle_shape.ur.y,
                room_shape.ur.x,
                room_shape.ur.y,
            ));
        }
    }
    // :272-277.
    if let Some(restrained_shape) = restrained_shape {
        result.push(IncompleteFreeSpaceExpansionRoom::new(
            Some(TileShape::Box(restrained_shape)),
            incomplete_room.get_layer(),
            Some(TileShape::Box(shape_to_be_contained)),
        ));
        return result;
    }

    // :279-287: "now shapeToBeContained intersects with the obstacleShape. shapeToBeContained and
    // shape evtl. need to be divided in two."
    let is = shape_to_be_contained.intersection(obstacle_shape);
    if is.is_empty() {
        return result;
    }
    // :288-308.
    let (new_shape_1, new_shape_2) =
        if is.ll.x > room_shape.ll.x && is.ll.x == obstacle_shape.ll.x && is.ll.x < room_shape.ur.x
        {
            (
                Some(IntBox::from_coords(
                    room_shape.ll.x,
                    room_shape.ll.y,
                    is.ll.x,
                    room_shape.ur.y,
                )),
                IntBox::from_coords(is.ll.x, room_shape.ll.y, room_shape.ur.x, room_shape.ur.y),
            )
        } else if is.ur.x > room_shape.ll.x
            && is.ur.x == obstacle_shape.ur.x
            && is.ur.x < room_shape.ur.x
        {
            (
                Some(IntBox::from_coords(
                    is.ur.x,
                    room_shape.ll.y,
                    room_shape.ur.x,
                    room_shape.ur.y,
                )),
                IntBox::from_coords(room_shape.ll.x, room_shape.ll.y, is.ur.x, room_shape.ur.y),
            )
        } else if is.ll.y > room_shape.ll.y
            && is.ll.y == obstacle_shape.ll.y
            && is.ll.y < room_shape.ur.y
        {
            (
                Some(IntBox::from_coords(
                    room_shape.ll.x,
                    room_shape.ll.y,
                    room_shape.ur.x,
                    is.ll.y,
                )),
                IntBox::from_coords(room_shape.ll.x, is.ll.y, room_shape.ur.x, room_shape.ur.y),
            )
        } else if is.ur.y > room_shape.ll.y
            && is.ur.y == obstacle_shape.ur.y
            && is.ur.y < room_shape.ur.y
        {
            (
                Some(IntBox::from_coords(
                    room_shape.ll.x,
                    is.ur.y,
                    room_shape.ur.x,
                    room_shape.ur.y,
                )),
                IntBox::from_coords(room_shape.ll.x, room_shape.ll.y, room_shape.ur.x, is.ur.y),
            )
        } else {
            (None, IntBox::EMPTY)
        };

    // :309-320.
    if let Some(new_shape_1) = new_shape_1 {
        let new_shape_to_be_contained = shape_to_be_contained.intersection(&new_shape_1);
        if new_shape_to_be_contained.dimension() > 0 {
            result.push(IncompleteFreeSpaceExpansionRoom::new(
                Some(TileShape::Box(new_shape_1)),
                incomplete_room.get_layer(),
                Some(TileShape::Box(new_shape_to_be_contained)),
            ));
            let new_incomplete_room = IncompleteFreeSpaceExpansionRoom::new(
                Some(TileShape::Box(new_shape_2)),
                incomplete_room.get_layer(),
                Some(TileShape::Box(
                    shape_to_be_contained.intersection(&new_shape_2),
                )),
            );
            result.extend(restrain_shape_90(&new_incomplete_room, obstacle_shape));
        }
    }
    result
}

// =================================================================================================
// Plan 7 Task 14b: the level-8 `TREEFP` / `CSO` ledgers (quirk #229)
// =================================================================================================
//
// Instrumentation, not behaviour. Both gates are `false` unless their variable is set in the
// environment, both are read once into a `LazyLock`, and everything they print goes to **stderr**.
// The Java halves are level 8 of `scripts/differential/java/p6t17b-bisect.patch`, which puts the
// same two ledgers into `ShapeSearchTree45Degree.completeShape` under the same variables.
//
// `TREEFP` is a structural fingerprint of the whole autoroute tree — pre-order, each node's
// bounding box and its leaf flag, FNV-1a — taken once per `completeShape`. `CSO` is one line per
// obstacle the walk actually restrains against, **in traversal order**, with the result rooms as
// they stand. Together they were the evidence for quirk #229: the two sides reached the same
// `completeShape` with the same node *count* and a different fingerprint, and the walk — whose
// `bounding_shape` prune shrinks as obstacles are consumed — then restrained against a different
// obstacle set, so the completed room came out a different shape. **Task 14c fixed the cause**
// (`Board::undo_from_snapshot`); the ledgers stay as the regression witness, and the `TREEFP`
// stream is now identical on both sides for the whole run.

/// `P7T14B_FP` — the `TREEFP` fingerprint (and `fr-board`'s `TINS8`, which reads the same
/// variable through its own gate).
fn p7t14b_fp_ledger() -> bool {
    static ON: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| std::env::var_os("P7T14B_FP").is_some());
    *ON
}

/// `P7T14B_CS` — the `CSO` obstacle ledger here, and `CSHAPE` / `CROOM8` in
/// [`crate::autoroute::maze::AutorouteEngine`].
pub(crate) fn p7t14b_cs_ledger() -> bool {
    static ON: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| std::env::var_os("P7T14B_CS").is_some());
    *ON
}

fn p7t14b_oct(octagon: &IntOctagon) -> String {
    format!(
        "({},{},{},{},{},{},{},{})",
        octagon.left_x,
        octagon.bottom_y,
        octagon.right_x,
        octagon.top_y,
        octagon.upper_left_diagonal_x,
        octagon.lower_right_diagonal_x,
        octagon.lower_left_diagonal_x,
        octagon.upper_right_diagonal_x
    )
}

/// The whole-tree fingerprint. The traversal order is Java's `ArrayStack` order exactly — push
/// `firstChild` then `secondChild`, pop — so the two hashes are comparable.
fn p7t14b_treefp(tree: &ShapeSearchTree, root: NodeId) {
    let mut hash: u64 = 1469598103934665603;
    let mut node_count: u64 = 0;
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let node = *tree.tree().node(id);
        node_count += 1;
        let bounds = node_bounds(&node).bounding_box();
        let is_leaf = u64::from(matches!(node, Node::Leaf { .. }));
        for field in [
            i64::from(bounds.ll.x) as u64,
            i64::from(bounds.ll.y) as u64,
            i64::from(bounds.ur.x) as u64,
            i64::from(bounds.ur.y) as u64,
            is_leaf,
        ] {
            hash = (hash ^ field).wrapping_mul(1099511628211);
        }
        if let Node::Inner {
            first_child,
            second_child,
            ..
        } = node
        {
            stack.push(first_child);
            stack.push(second_child);
        }
    }
    eprintln!("TREEFP n={node_count} h={hash:x}");
}
