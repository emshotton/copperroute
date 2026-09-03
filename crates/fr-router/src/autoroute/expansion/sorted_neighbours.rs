//! Port of `autoroute.expansion.SortedRoomNeighbours` (SortedRoomNeighbours.java:34-807) — the
//! any-angle base algorithm and the factory dispatch that picks between it and its two
//! angle-restricted siblings.
//!
//! "To calculate the neighbour rooms of an expansion room. The neighbour rooms will be sorted in
//! counterclock sense around the border of the shape of room. Overlapping neighbours containing
//! an item may be stored in an unordered list."
//!
//! # What replaces `AutorouteEngine`
//!
//! Java's entry points take an `AutorouteEngine` and read exactly five things off it:
//! `getNetNumber()` (`:97`), `autorouteSearchTree` (`:103`), `generateRoomIdNo()` (`:104`),
//! `removeAllDoors(room)` (`:113`) and `addIncompleteExpansionRoom(...)` (`:149`, `:642`). The
//! engine is Task 6's; the five services are not, so the port takes them apart:
//!
//! | Java | port |
//! |---|---|
//! | `autorouteEngine.getNetNumber()` | the `net_number` argument |
//! | `autorouteEngine.autorouteSearchTree` | the `tree_id` argument, resolved against `board.trees` |
//! | `autorouteEngine.generateRoomIdNo()` | [`ExpansionRoomStore::next_room_id_no`] |
//! | `autorouteEngine.removeAllDoors(room)` | [`ExpansionRoomStore::remove_all_doors`] |
//! | `autorouteEngine.addIncompleteExpansionRoom(..)` | [`ExpansionRoomStore::new_incomplete_room`] |
//!
//! Task 6's `AutorouteEngine::calculate_doors` therefore calls [`SortedRoomNeighbours::complete`]
//! with `self.net_number`, `&mut self.board`, `&mut self.rooms` and `self.autoroute_search_tree`,
//! and no signature here changes.
//!
//! # Hazard F — the non-transitive comparator (quirk #160) — **fixed: T8**
//!
//! Java's `compareTo` (`:719-762`) is **not** a total order. Two neighbours whose first corners
//! are *different but equidistant* fall straight through to the id tie-break, while two whose
//! first corners are *equal as points* are refined by their last corners first; `c_dist_tolerance`
//! selects which key answers and then the exact sign of that key is taken; and the
//! `Direction.compareFrom` refinement (`:750-751`) applies to some pairs and not others.
//! Elements land in a `TreeSet` (`:54`, inserted at `:408`), so a comparison that answers `Equal`
//! **drops** the neighbour — a door the room really has is never built.
//!
//! [`SortedRoomNeighbour::compare_to`] is now a lexicographic comparison of Java's own keys in
//! Java's own order, with those three defects removed and the remaining value fields compared past
//! Java's last key, so `Equal` means "equal as a value". Measured on `p6t3` mode 3's own generator
//! — reproduced in `crates/fr-router/tests/sorted_neighbours.rs` down to the xorshift stream —
//! the pre-fix comparator drops **481** of 8 000 neighbours in a `JavaTreeSet` and 482 in a
//! `BTreeSet`; the post-fix number is **0** in both.
//!
//! **The container is still [`JavaTreeSet`]** — Task 24 collects the swap to `BTreeSet`, which the
//! total order now makes sound. The reason it had to be a `JavaTreeSet` was exactly the hazard: a
//! `BTreeSet` searches a B-tree node by binary search where `TreeMap` walks a red-black tree from
//! the root, so on a non-transitive comparator the two keep *different* elements and iterate the
//! survivors in *different* orders. `the_neighbour_comparator_is_a_total_order` asserts that they
//! now agree, over every pair and every ordered triple of all 2 000 cases.
//!
//! # Hazard G — the id tie-break crossed two id spaces (quirk #161) — **fixed: T8**
//!
//! `:759` is `this.searchTreeObject.getId() - other.searchTreeObject.getId()`, and the objects in
//! the tree are board **items** and expansion **rooms**. An item's id comes from the board's
//! `ItemIdGenerator`; a room's from `AutorouteEngine.expansionRoomInstanceCount`. They start at 1
//! and collide constantly. The comparator now compares the object *kind* before the id, so the two
//! spaces never meet, and uses `Ord::cmp` rather than a wrapping subtraction.
//!
//! One site is deliberately **not** changed: `:203-213`'s pre-sort of the overlapping objects
//! (below, in [`SortedRoomNeighbours::calculate_neighbours`]) subtracts the same two id spaces.
//! It is a `List.sort` — a *stable* TimSort — so a collision there loses nothing; both entries
//! keep the raw tree-query order they arrived in. Changing it would move the neighbour insertion
//! order on every board for no defect.
//!
//! # Hazard: `calculateNewIncompleteRooms` can loop for ever (quirk #162)
//!
//! `:512` indexes `fromRoom.getShape().toSimplex()` with side numbers computed against the
//! un-simplified shape. The `// Java bug:` marker on
//! [`SortedRoomNeighbours::calculate_new_incomplete_rooms`] has the detail; it is **reproduced**,
//! not guarded, and Task 6's engine will meet it on about one room completion in 250 of `p6t3`
//! mode 5's random boards.
//!
//! not ported: every `FRLogger` payload of this class — the four `FRLogger.debug` "expected"
//! messages (`:250`, `:256`, `:299`, `:314`), the `FRLogger.warn` at `:196` and `:377`, and the
//! four `ROOM_EDGE_REMOVE`/`calculate_new_incomplete_rooms` `FRLogger.trace` blocks (`:126-129`,
//! `:448-457`, `:466-475`, `:489-500`) — per `global-constraints.md`.

use std::cell::OnceCell;
use std::cmp::Ordering;

use fr_board::datastructures::TreeEntry;
use fr_board::searchtree::ShapeSearchTree;
use fr_board::{
    AngleRestriction, Board, Item, ItemCtx, ItemLookup, ObstacleRoomId, RoomId, RoomLookup, TreeId,
    TreeObject,
};
use fr_geometry::polyline_shape::PolylineShapeOps;
use fr_geometry::{Line, Point, Side, Simplex, TileShape};

use crate::JavaTreeSet;
use crate::autoroute::expansion::sorted_neighbours_45::Sorted45DegreeRoomNeighbours;
use crate::autoroute::expansion::sorted_neighbours_orthogonal::SortedOrthogonalRoomNeighbours;
use crate::autoroute::expansion::{ExpansionRoomStore, IncompleteFreeSpaceExpansionRoom, RoomRef};
use crate::autoroute::item_info;
use crate::autoroute::tree_ext::AutorouteSearchTreeExt;

/// Port of the nested `SortedRoomNeighbours.CalculationMode` (SortedRoomNeighbours.java:37-41).
///
/// The variants are spelled in Java's declaration order so a reader diffing the two files sees
/// the same list; nothing depends on the order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CalculationMode {
    /// `ORTHOGONAL` (`:38`) — `SortedOrthogonalRoomNeighbours`, a 90-degree tree.
    Orthogonal,
    /// `DEGREE_45` (`:39`) — `Sorted45DegreeRoomNeighbours`, a 45-degree tree.
    FortyFiveDegree,
    /// `ANY_ANGLE` (`:40`) — this class, every other tree.
    AnyAngle,
}

/// Port of `SortedRoomNeighbours.selectCalculationMode(ShapeSearchTree)`
/// (SortedRoomNeighbours.java:80-88).
///
/// Java tests `instanceof ShapeSearchTree90Degree` and then `instanceof ShapeSearchTree45Degree`;
/// the port has one angle-parameterised tree type (plan-2), so it tests
/// [`ShapeSearchTree::angle`] — the same value `SearchTreeManager.getAutorouteTree` chose the
/// subclass from (SearchTreeManager.java:147-161), which is what
/// `crates/fr-router/src/autoroute/tree_ext.rs` already dispatches on.
///
/// The **order** of the two tests is transcribed rather than collapsed into a `match`, because a
/// tree that was somehow both would answer `ORTHOGONAL` in Java and must here too. Java's
/// subclasses are unrelated, so this is documentation, not behaviour.
pub fn select_calculation_mode(tree: &ShapeSearchTree) -> CalculationMode {
    // SortedRoomNeighbours.java:81-83.
    if tree.angle() == AngleRestriction::NinetyDegree {
        return CalculationMode::Orthogonal;
    }
    // :84-86.
    if tree.angle() == AngleRestriction::FortyFiveDegree {
        return CalculationMode::FortyFiveDegree;
    }
    // :87.
    CalculationMode::AnyAngle
}

// =================================================================================================
// The sorter
// =================================================================================================

/// Port of `SortedRoomNeighbours` (SortedRoomNeighbours.java:34-807): the neighbour sorter that
/// turns one completed room into its door list.
///
/// One instance is built per `calculate` call and thrown away; it carries no id and is never
/// stored in an arena.
#[derive(Debug, Clone)]
pub struct SortedRoomNeighbours {
    /// `SortedRoomNeighbours.fromRoom` (:43).
    pub from_room: RoomRef,
    /// `SortedRoomNeighbours.completedRoom` (:44).
    pub completed_room: RoomRef,
    /// `SortedRoomNeighbours.roomShape` (:45) — `completedRoom.getShape()` **captured in the
    /// constructor** (`:53`). `calculateNewIncompleteRooms` may replace the completed room's
    /// shape (`:575-576`); this field keeps the shape the comparator was built against, exactly
    /// as Java's does.
    pub room_shape: TileShape,
    /// `SortedRoomNeighbours.sortedNeighbours` (:46), a `TreeSet` — see the module docs on
    /// hazard F for why the container has to be [`JavaTreeSet`] and not a `BTreeSet`.
    pub sorted_neighbours: JavaTreeSet<SortedRoomNeighbour>,
    /// `SortedRoomNeighbours.ownNetObjects` (:47), a `LinkedList` in insertion order.
    pub own_net_objects: Vec<TreeEntry<TreeObject>>,
}

impl SortedRoomNeighbours {
    /// Port of `SortedRoomNeighbours.complete(ExpansionRoom, AutorouteEngine)`
    /// (SortedRoomNeighbours.java:65-72): "dispatches room-neighbour calculation to the
    /// implementation matching the search-tree type."
    ///
    /// **Java wins over the brief's signature.** The brief writes
    /// `complete(room, engine, board, net_no, ignore_net)`; Java's method takes two arguments and
    /// has no `ignoreNet` anywhere in the class. The port takes the room plus the five services
    /// the module docs tabulate.
    ///
    /// All three arms are live: the two angle-restricted ones are
    /// [`Sorted45DegreeRoomNeighbours::calculate`] and
    /// [`SortedOrthogonalRoomNeighbours::calculate`], which are separate transcriptions of
    /// separate Java classes and not specialisations of this one.
    pub fn complete(
        room: RoomRef,
        net_number: i32,
        board: &mut Board,
        rooms: &mut ExpansionRoomStore,
        tree_id: TreeId,
    ) -> Option<RoomRef> {
        match select_calculation_mode(tree_of(board, tree_id)) {
            // :68.
            CalculationMode::Orthogonal => {
                SortedOrthogonalRoomNeighbours::calculate(room, net_number, board, rooms, tree_id)
            }
            // :69.
            CalculationMode::FortyFiveDegree => {
                Sorted45DegreeRoomNeighbours::calculate(room, net_number, board, rooms, tree_id)
            }
            // :70.
            CalculationMode::AnyAngle => {
                SortedRoomNeighbours::calculate(room, net_number, board, rooms, tree_id)
            }
        }
    }

    /// Port of `SortedRoomNeighbours.calculate(ExpansionRoom, AutorouteEngine)`
    /// (SortedRoomNeighbours.java:95-136).
    ///
    /// Java's tail call at `:114` (`return calculate(room, autorouteEngine)`, after an edge was
    /// removed) is the loop below: the recursion has no state other than the arguments, and it
    /// takes a **fresh** room id from the counter each time round — which is why room ids skip
    /// (see [`ExpansionRoomStore::next_room_id_no`]).
    ///
    /// `None` is Java's `null` from `:197`, which `:110` then dereferences: the port answers the
    /// `null` one frame earlier rather than reproducing the `NullPointerException`.
    // totalized: SortedRoomNeighbours.calculate NPEs at :110 when calculateNeighbours answered
    // null at :197 (an expansion room that is neither incomplete nor an obstacle room, i.e. a
    // CompleteFreeSpaceExpansionRoom); the port returns `None`. Unreachable from the engine,
    // which only ever completes an incomplete or an obstacle room.
    pub fn calculate(
        room: RoomRef,
        net_number: i32,
        board: &mut Board,
        rooms: &mut ExpansionRoomStore,
        tree_id: TreeId,
    ) -> Option<RoomRef> {
        // :99-104. `generateRoomIdNo()` is an *argument*, so in Java the counter ticks before the
        // method knows whether a complete room will be built at all — and the `edgeRemoved` retry
        // at `:111-114` recurses, drawing a **fresh** id for the room that replaces one Java has
        // just thrown away.
        //
        // fixed: T8 (#165), the improvement column's first half — "have `SortedRoomNeighbours.
        // calculate` take the room id *after* it commits". The id is drawn **once** here and
        // reused across retries, so a room that is discarded does not consume one. The discarded
        // room keeps its arena slot (the arena is Java's heap and Java's is garbage-collected the
        // same way), but it is not in the tree, not in `completeExpansionRooms`, and — since the
        // retry's `removeAllDoors` — not reachable through a door either, so nothing can observe
        // the id it shares with its replacement.
        let room_id_no = rooms.next_room_id_no();
        loop {
            let room_neighbours = SortedRoomNeighbours::calculate_neighbours(
                room, net_number, board, rooms, tree_id, room_id_no,
            )?;

            // :109-115. "Check, that each side of the room shape has at least one touching
            // neighbour. Otherwise, improve the room shape by enlarging."
            let edge_removed = room_neighbours.try_remove_edge(net_number, board, rooms, tree_id);
            let result = room_neighbours.completed_room;
            if edge_removed {
                rooms.remove_all_doors(result);
                continue;
            }

            // :117-130. "Now calculate the new incomplete rooms together with the doors between
            // this room and the sorted neighbours."
            if room_neighbours.sorted_neighbours.is_empty() {
                // :120-122 tests `result` and then casts **`room`**; in this branch they are the
                // same object, because `calculateNeighbours` set `completedRoom = obstacleRoom`.
                if let RoomRef::Obstacle(_) = result {
                    calculate_incomplete_rooms_with_empty_neighbours(room, board, rooms);
                }
            } else {
                room_neighbours.calculate_new_incomplete_rooms(board, rooms);
                // :125-129 is an `FRLogger.trace` and its guard; both dropped.
            }

            // :132-134.
            if let RoomRef::Complete(free_room) = result {
                calculate_target_doors(
                    free_room,
                    &room_neighbours.own_net_objects,
                    net_number,
                    board,
                    rooms,
                    tree_id,
                );
            }
            return Some(result);
        }
    }

    /// Port of the private `SortedRoomNeighbours.calculateNeighbours`
    /// (SortedRoomNeighbours.java:187-329) — the whole of the algorithm that reads the search
    /// tree.
    ///
    /// `pub` where Java's is `private`, because it is the unit `p6t3` mode 0 drives directly: its
    /// output (the sorted set, the own-net list and the doors it creates) is the parity surface,
    /// and the three methods above it all mutate engine state a differential cannot compare.
    ///
    /// `None` is Java's `null` at `:197`.
    pub fn calculate_neighbours(
        room: RoomRef,
        net_number: i32,
        board: &mut Board,
        rooms: &mut ExpansionRoomStore,
        tree_id: TreeId,
        room_id_no: i32,
    ) -> Option<SortedRoomNeighbours> {
        // :189. Java would NPE below on a room with no shape; so does this.
        let room_shape = rooms
            .room_shape(room)
            .unwrap_or_else(|| {
                panic!(
                    "SortedRoomNeighbours.calculateNeighbours: room {room:?} has no shape \
                     (SortedRoomNeighbours.java:189) — Java NPEs here too"
                )
            })
            .clone();
        let layer = rooms.room_layer(board, room).unwrap_or_else(|| {
            panic!(
                "SortedRoomNeighbours.calculateNeighbours: room {room:?} has no layer \
                 (SortedRoomNeighbours.java:192) — Java NPEs here too"
            )
        });
        // fixed: T8 (#162). `:512` builds `roomSimplex = this.fromRoom.getShape().toSimplex()`
        // inside `calculateNewIncompleteRooms`, three method calls after every
        // `touchingSideNoOfRoom` has been computed against the **un-simplified** shape (`:254` for
        // a 1-dimensional touch, `:289-297` for a corner). `Simplex.getInstance` drops redundant
        // lines (Simplex.java:37-46), so an `IntOctagon` whose diagonals are implied by its four
        // sides comes back with 4 or 5 lines rather than 8 — and a `firstTouchingSideNo` naming a
        // line the simplex does not have makes the `for (;;)` at `:562` walk `prevNo` round the
        // simplex for ever, allocating a room and a door per turn until the heap is gone.
        //
        // The simplex is therefore derived **once, here in the constructor**, and every side
        // number in this class is an index into it: this is the room shape the sorted neighbours
        // carry, the shape `tryRemoveEdge` walks, and the shape `calculateNewIncompleteRooms`
        // uses. The loop's exit `currentTouchingSideNo == firstTouchingSideNo` is then reachable
        // by construction, because `firstTouchingSideNo` came from this same shape.
        //
        // Not a loop bound: a bound stops the hang on the wrong side, leaving the room with a
        // silently wrong door set. This makes the two shapes the same shape.
        //
        // The **original** shape is kept for everything that is not a side number — the complete
        // room built at `:191`, the tree query at `:200-201`, and the door intersections — because
        // simplifying is a change of representation, not of geometry, and the arena should hold
        // the shape the engine handed in.
        let room_simplex = TileShape::Simplex(room_shape.to_simplex());

        // :190-198.
        let completed_room = match room {
            RoomRef::Incomplete(_) => RoomRef::Complete(rooms.new_complete_room(
                Some(room_shape.clone()),
                layer,
                room_id_no,
            )),
            RoomRef::Obstacle(id) => RoomRef::Obstacle(id),
            // :195-197: `FRLogger.warn` (dropped) and `return null`.
            RoomRef::Complete(_) => return None,
        };

        // :199. `roomShape = completedRoom.getShape()` — the same shape object in both branches.
        let mut result = SortedRoomNeighbours {
            from_room: room,
            completed_room,
            room_shape: room_simplex.clone(),
            sorted_neighbours: JavaTreeSet::new(),
            own_net_objects: Vec::new(),
        };

        // :200-201.
        let mut overlapping_objects = {
            let ctx = board.ctx();
            tree_of(board, tree_id).overlapping_tree_entries_with_rooms(
                &room_shape,
                Some(layer),
                &[],
                &board.items,
                &*rooms,
                &ctx,
            )
        };

        // :203-213. "Sort the overlapping objects deterministically to ensure parity with v1.9."
        // `List.sort` is a **stable** TimSort, and so is `slice::sort_by`: where a room id and an
        // item id collide (hazard G) the comparator answers 0 and both keep the raw tree-query
        // order, which is what `overlapping_tree_entries` already produces.
        overlapping_objects.sort_by(|e1, e2| {
            let id_diff = object_id(e1.object, rooms).wrapping_sub(object_id(e2.object, rooms));
            if id_diff != 0 {
                return id_diff.cmp(&0);
            }
            e1.shape_index.cmp(&e2.shape_index)
        });

        // :215-327. "Calculate the touching neighbour objects and sort them in counterclock sense
        // around the border of the room shape."
        for current_entry in overlapping_objects {
            let current_object = current_entry.object;
            // :219-221 is `currentObject == room`, a reference comparison between a
            // `SearchTreeObject` in the tree and the input room. It can only be true for a
            // `CompleteFreeSpaceExpansionRoom`, and `:195-197` has already returned `null` for
            // one — so the test is dead in Java as well. Transcribed as the same predicate.
            if rooms
                .get_object(room)
                .is_some_and(|object| room.is_free_space() && object == current_object)
            {
                continue;
            }

            // :222-227. "delay processing the target doors until the room shape will not change
            // anymore"
            if matches!(room, RoomRef::Incomplete(_))
                && !object_is_trace_obstacle(current_object, net_number, &board.items)
            {
                result.own_net_objects.push(current_entry);
                continue;
            }

            // :228-231.
            let current_shape = {
                let ctx = board.ctx();
                object_tree_shape(
                    tree_of(board, tree_id),
                    current_object,
                    current_entry.shape_index,
                    &board.items,
                    &*rooms,
                    &ctx,
                )
            };
            let intersection = room_shape.intersection(&current_shape);
            let dimension = intersection.dimension();

            if dimension > 1 {
                // :232-248. "only Obstacle expansion room may have a 2-dim overlap"
                if let (RoomRef::Obstacle(obstacle_room), TreeObject::Item(item_id)) =
                    (completed_room, current_object)
                    && board
                        .get_item(item_id)
                        .is_some_and(|item| item.is_routable())
                {
                    // :237-239. `getAutorouteInfo()` creates the scratch on demand.
                    let overlap_room = {
                        let (b, r) = (&mut *board, &mut *rooms);
                        item_info::get_expansion_room(
                            b,
                            item_id,
                            current_entry.shape_index,
                            tree_id,
                            |b, item, index, tree| r.new_obstacle_room(b, item, index, tree),
                        )
                    };
                    // :240 hands the result straight to `createOverlapDoor`, which dereferences
                    // `other.item` at ObstacleExpansionRoom.java:81 — so a `null` from the
                    // out-of-range branch of `getExpansionRoom` is a `NullPointerException`.
                    let overlap_room = overlap_room.unwrap_or_else(|| {
                        panic!(
                            "SortedRoomNeighbours.calculateNeighbours: item {item_id} has no \
                             expansion room for shape {} (SortedRoomNeighbours.java:239) — Java \
                             NPEs in createOverlapDoor here too",
                            current_entry.shape_index
                        )
                    });
                    create_overlap_door(obstacle_room, overlap_room, board, rooms);
                }
                // :242-246 is an `FRLogger.trace`; dropped.
                continue;
            }
            if dimension < 0 {
                // :249-252: `FRLogger.debug` (dropped) and `continue`.
                continue;
            }

            if dimension == 1 {
                // :253-258.
                let Some(touching_sides) = room_simplex.touching_sides(&current_shape) else {
                    // Java's `touchingSides.length != 2`: `FRLogger.debug` (dropped), `continue`.
                    continue;
                };
                // :259-266.
                result.add_sorted_neighbour(SortedRoomNeighbour::new(
                    current_object,
                    object_id(current_object, rooms),
                    current_shape.clone(),
                    intersection.clone(),
                    touching_sides[0] as i32,
                    touching_sides[1] as i32,
                    false,
                    false,
                    room_simplex.clone(),
                ));

                // :267-285. "make sure, that there is a door to the neighbour room."
                let neighbour_room: Option<RoomRef> = match current_object {
                    // :269-270: a `CompleteFreeSpaceExpansionRoom` *is* an `ExpansionRoom`.
                    TreeObject::Room(id) => Some(RoomRef::Complete(id)),
                    // :271-278. "expand the item for ripup and pushing purposes"
                    TreeObject::Item(item_id) => {
                        if board
                            .get_item(item_id)
                            .is_some_and(|item| item.is_routable())
                        {
                            let (b, r) = (&mut *board, &mut *rooms);
                            item_info::get_expansion_room(
                                b,
                                item_id,
                                current_entry.shape_index,
                                tree_id,
                                |b, item, index, tree| r.new_obstacle_room(b, item, index, tree),
                            )
                            .map(RoomRef::Obstacle)
                        } else {
                            None
                        }
                    }
                };
                // :279-285. The `null` check is Java's own: `getExpansionRoom`'s out-of-range
                // branch silently skips the door here, unlike at `:239`.
                if let Some(neighbour_room) = neighbour_room
                    && insert_door_ok(completed_room, neighbour_room, &intersection, board, rooms)
                {
                    let new_door = rooms.new_door(completed_room, neighbour_room, 1);
                    rooms.add_door(neighbour_room, new_door);
                    rooms.add_door(completed_room, new_door);
                }
            } else {
                // :286-326, dimension == 0.
                let touching_point = intersection.corner(0);
                let room_corner_no = room_simplex.equals_corner(&touching_point);
                let (room_touch_is_corner, touching_side_no_of_room) = match room_corner_no {
                    // :292-294.
                    Some(no) => (true, no as i32),
                    // :295-301. Java logs and keeps the -1.
                    None => (
                        false,
                        room_simplex
                            .contains_on_border_line_no(&touching_point)
                            .map_or(-1, |no| no as i32),
                    ),
                };
                let neighbour_room_corner_no = current_shape.equals_corner(&touching_point);
                let (neighbour_room_touch_is_corner, touching_side_no_of_neighbour_room) =
                    match neighbour_room_corner_no {
                        // :305-309. "The previous border line is preferred to make the shape of
                        // the incomplete room as big as possible"
                        Some(no) => (true, current_shape.prev_no(no) as i32),
                        // :310-316.
                        None => (
                            false,
                            current_shape
                                .contains_on_border_line_no(&touching_point)
                                .map_or(-1, |no| no as i32),
                        ),
                    };
                // :318-325.
                result.add_sorted_neighbour(SortedRoomNeighbour::new(
                    current_object,
                    object_id(current_object, rooms),
                    current_shape.clone(),
                    intersection.clone(),
                    touching_side_no_of_room,
                    touching_side_no_of_neighbour_room,
                    room_touch_is_corner,
                    neighbour_room_touch_is_corner,
                    room_simplex.clone(),
                ));
            }
        }
        // :328.
        Some(result)
    }

    /// Port of the private `SortedRoomNeighbours.addSortedNeighbour`
    /// (SortedRoomNeighbours.java:391-409). The `new SortedRoomNeighbour(..)` half is
    /// [`SortedRoomNeighbour::new`]; this is `sortedNeighbours.add(newNeighbour)` (`:408`), the
    /// insert that may silently drop the element (hazard F).
    fn add_sorted_neighbour(&mut self, neighbour: SortedRoomNeighbour) {
        self.sorted_neighbours.add(neighbour);
    }

    /// Port of the private `SortedRoomNeighbours.tryRemoveEdge`
    /// (SortedRoomNeighbours.java:415-507): "checks that each side of the room shape has at
    /// least one touching neighbour. Otherwise, the room shape will be improved by enlarging.
    /// Returns true if the room shape was changed."
    fn try_remove_edge(
        &self,
        net_number: i32,
        board: &mut Board,
        rooms: &mut ExpansionRoomStore,
        tree_id: TreeId,
    ) -> bool {
        // :416-418.
        let RoomRef::Incomplete(incomplete_id) = self.from_room else {
            return false;
        };
        // :419-421.
        let mut remove_edge_no: i32 = -1;
        // :420 is `this.fromRoom.getShape().toSimplex()`. fixed: T8 (#162) — the one derivation
        // in `calculateNeighbours` is this shape, so the `currentEdgeNo` walk below and the
        // `touchingSideNoOfRoom`s it compares against are indices into the same shape.
        let TileShape::Simplex(room_simplex) = &self.room_shape else {
            unreachable!("calculate_neighbours builds room_shape as a Simplex")
        };
        let room_shape_area = self.room_shape.area();

        // :423-438.
        let mut prev_edge_no: i32 = -1;
        let mut current_edge_no: i32 = 0;
        for next_neighbour in &self.sorted_neighbours {
            if next_neighbour.touching_side_no_of_room == prev_edge_no {
                continue;
            }
            if next_neighbour.touching_side_no_of_room == current_edge_no {
                prev_edge_no = current_edge_no;
                current_edge_no += 1;
            } else {
                // "On the edge side with index currentEdgeNo is no touching neighbour."
                remove_edge_no = current_edge_no;
                break;
            }
        }

        // :440-443. "missing touching neighbour at the last edge side."
        if remove_edge_no < 0 && current_edge_no < room_simplex.border_line_count() as i32 {
            remove_edge_no = current_edge_no;
        }

        if remove_edge_no < 0 {
            return false;
        }
        // :445-458. "Touching neighbour missing at the edge side with index removeEdgeNo. Remove
        // the edge line and restart the algorithm."
        let enlarged_shape = room_simplex.remove_border_line(index_of(
            remove_edge_no,
            "tryRemoveEdge's removeEdgeNo",
            458,
        ));
        let (layer, contained_shape) = {
            let r = rooms
                .incomplete_room(incomplete_id)
                .expect("the from room is in the arena");
            (r.get_layer(), r.get_contained_shape().cloned())
        };
        // :459-463. A **local** room: Java does not hand it to `addIncompleteExpansionRoom`, so
        // it never enters the engine's list and must not enter the arena either.
        let enlarged_room = IncompleteFreeSpaceExpansionRoom::new(
            Some(TileShape::Simplex(enlarged_shape)),
            layer,
            contained_shape.clone(),
        );
        // :464-465.
        let new_rooms = {
            let ctx = board.ctx();
            tree_of(board, tree_id).complete_shape(
                &enlarged_room,
                net_number,
                None,
                None,
                &board.items,
                &*rooms,
                &ctx,
            )
        };
        // :476-479.
        if new_rooms.len() != 1 {
            return false;
        }
        // :480-485. "Check, that the area increases to prevent endless loop."
        let new_shape = new_rooms[0].get_shape().unwrap_or_else(|| {
            panic!(
                "SortedRoomNeighbours.tryRemoveEdge: completeShape answered a room with no shape \
                 (SortedRoomNeighbours.java:483) — Java NPEs here too"
            )
        });
        if new_shape.area() <= room_shape_area {
            return false;
        }
        // :486-503.
        let (new_shape, new_contained) = {
            let new_room = &new_rooms[0];
            (
                new_room.get_shape().cloned(),
                new_room.get_contained_shape().cloned(),
            )
        };
        if let Some(r) = rooms.incomplete_room_mut(incomplete_id) {
            r.set_shape(new_shape);
            r.set_contained_shape(new_contained);
        }
        true
    }

    /// Port of `SortedRoomNeighbours.calculateNewIncompleteRooms(AutorouteEngine)`
    /// (SortedRoomNeighbours.java:510-659): "called from calculateDoors(). The shape of the room
    /// result may change inside this function."
    ///
    /// The `prevNeighbour == this.sortedNeighbours.getLast()` reference test (`:520`, `:524`,
    /// `:595`) is the loop's first pass and nothing else: `prevNeighbour` starts as the last
    /// element and afterwards is always the *previous* element, which is the last one only at an
    /// index the loop never reaches.
    ///
    // fixed: T8 (#162). Java does not terminate here when the room's shape has more border lines
    // than its `toSimplex()` does: `:512` builds `roomSimplex =
    // this.fromRoom.getShape().toSimplex()`, `Simplex.getInstance` drops redundant lines
    // (Simplex.java:37-46), and `touchingSideNoOfRoom` was computed against the **un-simplified**
    // shape (`:254` for a 1-dimensional touch, `:289-297` for a corner). A `firstTouchingSideNo`
    // naming a line the simplex does not have makes the `for (;;)` at `:562` walk `prevNo` round
    // the simplex for ever, adding an `IncompleteFreeSpaceExpansionRoom` per turn (`:642`) until
    // the heap is gone. `self.room_shape` **is** that simplex, derived once in the constructor,
    // so `:512`'s second derivation is gone and every side number below indexes the shape it came
    // from — see the note in `calculate_neighbours`.
    pub fn calculate_new_incomplete_rooms(
        &self,
        board: &mut Board,
        rooms: &mut ExpansionRoomStore,
    ) {
        let neighbours: Vec<&SortedRoomNeighbour> = self.sorted_neighbours.iter().collect();
        // :511. `SortedSet.getLast()` throws on an empty set; the one caller guards it (`:119`).
        let Some(&last) = neighbours.last() else {
            panic!(
                "SortedRoomNeighbours.calculateNewIncompleteRooms: the neighbour set is empty \
                 (SortedRoomNeighbours.java:511) — Java throws NoSuchElementException here too"
            )
        };
        // :512, hoisted to the constructor (#162): this is `fromRoom.getShape().toSimplex()`,
        // computed once and shared with the sorted neighbours' own side numbers.
        let room_simplex = &self.room_shape;
        let from_room_layer = rooms
            .room_layer(board, self.from_room)
            .expect("the from room is in the arena");

        let mut prev_neighbour = last;
        for (index, next_neighbour) in neighbours.iter().copied().enumerate() {
            // `prevNeighbour == this.sortedNeighbours.getLast()` — see the doc comment.
            let prev_is_last = index == 0;

            // :514-515.
            let mut first_touching_side_no = prev_neighbour.touching_side_no_of_room;
            let mut last_touching_side_no = next_neighbour.touching_side_no_of_room;

            // :517-525.
            let current_next_no = room_simplex.next_no(index_of(
                first_touching_side_no,
                "calculateNewIncompleteRooms' firstTouchingSideNo",
                517,
            )) as i32;
            let intersection_with_prev_neighbour_ends_at_corner =
                (first_touching_side_no != last_touching_side_no || prev_is_last)
                    && *prev_neighbour.last_corner()
                        == room_simplex.corner(index_of(current_next_no, "currentNextNo", 521));
            let intersection_with_next_neighbour_starts_at_corner =
                (first_touching_side_no != last_touching_side_no || prev_is_last)
                    && *next_neighbour.first_corner()
                        == room_simplex.corner(index_of(
                            last_touching_side_no,
                            "lastTouchingSideNo",
                            525,
                        ));

            // :527-533.
            if intersection_with_prev_neighbour_ends_at_corner {
                first_touching_side_no = current_next_no;
            }
            if intersection_with_next_neighbour_starts_at_corner {
                last_touching_side_no = room_simplex.prev_no(index_of(
                    last_touching_side_no,
                    "lastTouchingSideNo",
                    532,
                )) as i32;
            }

            // :534-538.
            let neighbours_touch = neighbours.len() > 1
                && prev_neighbour.last_corner() == next_neighbour.first_corner();

            if !neighbours_touch {
                // :540-655. "create a door to a new incomplete expansion room between the last
                // corner of the previous neighbour and the first corner of the current
                // neighbour."
                let mut last_bounding_line_no = prev_neighbour.touching_side_no_of_neighbour_room;
                if !(intersection_with_prev_neighbour_ends_at_corner
                    || prev_neighbour.room_touch_is_corner)
                {
                    last_bounding_line_no = prev_neighbour.neighbour_shape.prev_no(index_of(
                        last_bounding_line_no,
                        "lastBoundingLineNo",
                        546,
                    )) as i32;
                }

                let mut first_bounding_line_no = next_neighbour.touching_side_no_of_neighbour_room;
                if !(intersection_with_next_neighbour_starts_at_corner
                    || next_neighbour.neighbour_room_touch_is_corner)
                {
                    first_bounding_line_no = next_neighbour.neighbour_shape.next_no(index_of(
                        first_bounding_line_no,
                        "firstBoundingLineNo",
                        552,
                    )) as i32;
                }
                // :554-556. "startEdgeLine is only used for the first new incomplete room."
                let mut start_edge_line: Option<Line> = Some(
                    border_line_of(
                        &next_neighbour.neighbour_shape,
                        first_bounding_line_no,
                        "firstBoundingLineNo",
                        555,
                    )
                    .opposite(),
                );
                let mut middle_edge_line: Option<Line> = None;
                let mut current_touching_side_no = last_touching_side_no;
                let mut first_time = true;
                // :562-655. "The loop goes backwards from the edge line of nextNeighbour to the
                // edge line of prevNeighbour."
                loop {
                    let mut corner_cut_off = false;
                    // :564-584.
                    if let RoomRef::Incomplete(incomplete_id) = self.from_room
                        && current_touching_side_no == last_touching_side_no
                        && first_touching_side_no != last_touching_side_no
                    {
                        // "Create a new line approximately from the last corner of the previous
                        // neighbour to the first corner of the next neighbour to cut off the
                        // outstanding corners of the room shape in the empty space. That is only
                        // tried in the first pass of the loop."
                        let cut_line_start = prev_neighbour.last_corner().to_float().round();
                        let cut_line_end = next_neighbour.first_corner().to_float().round();
                        let cut_line = Line::new(cut_line_start, cut_line_end);
                        let cut_half_plane = TileShape::get_instance_from_line(cut_line);
                        // :575-576. The cast is safe: `fromRoom` being incomplete makes
                        // `completedRoom` a `CompleteFreeSpaceExpansionRoom` (`:191-192`).
                        let new_shape = rooms
                            .room_shape(self.completed_room)
                            .map(|shape| shape.intersection(&cut_half_plane));
                        if let RoomRef::Complete(id) = self.completed_room
                            && let Some(r) = rooms.complete_room_mut(id)
                        {
                            r.set_shape(new_shape);
                        }
                        // :577-582. "Otherwise room.containedShape would no longer be contained
                        // in the shape after cutting of the corner."
                        corner_cut_off = rooms
                            .incomplete_room(incomplete_id)
                            .and_then(|r| r.get_contained_shape())
                            .unwrap_or_else(|| {
                                panic!(
                                    "SortedRoomNeighbours.calculateNewIncompleteRooms: the \
                                     incomplete room has no contained shape \
                                     (SortedRoomNeighbours.java:579) — Java NPEs here too"
                                )
                            })
                            .side_of_line(&cut_line)
                            == Side::OnTheLeft;
                        if corner_cut_off {
                            middle_edge_line = Some(cut_line.opposite());
                        }
                    }
                    // :585.
                    let next_touching_side_no = room_simplex.prev_no(index_of(
                        current_touching_side_no,
                        "currentTouchingSideNo",
                        585,
                    )) as i32;

                    // :587-589.
                    if !corner_cut_off {
                        middle_edge_line = Some(
                            border_line_of(
                                room_simplex,
                                current_touching_side_no,
                                "currentTouchingSideNo",
                                588,
                            )
                            .opposite(),
                        );
                    }
                    let middle_edge_line = middle_edge_line.expect("assigned on both paths");
                    // :591.
                    let middle_line_dir = middle_edge_line.direction();

                    // :593-597. "The expression above handles the case, when all neighbours are
                    // on 1 edge line."
                    let last_time = (current_touching_side_no == first_touching_side_no
                        && !(prev_is_last && first_time))
                        || corner_cut_off;

                    // :599-610. "endEdgeLine is only used for the last new incomplete room."
                    let mut end_edge_line: Option<Line> = if last_time {
                        Some(
                            border_line_of(
                                &prev_neighbour.neighbour_shape,
                                last_bounding_line_no,
                                "lastBoundingLineNo",
                                602,
                            )
                            .opposite(),
                        )
                    } else {
                        None
                    };
                    if let Some(line) = end_edge_line
                        && line.direction().side_of(&middle_line_dir) != Side::OnTheLeft
                    {
                        // "Concave corner between the middle and the last line. Maybe there is a
                        // 1 point touch."
                        end_edge_line = None;
                    }

                    // :612-617.
                    if let Some(line) = start_edge_line
                        && middle_line_dir.side_of(&line.direction()) != Side::OnTheLeft
                    {
                        // "concave corner between the first and the middle line. May be there is
                        // a 1 point touch."
                        start_edge_line = None;
                    }

                    // :618-636.
                    let mut new_edge_lines: Vec<Line> = Vec::with_capacity(3);
                    if let Some(line) = start_edge_line {
                        new_edge_lines.push(line);
                    }
                    new_edge_lines.push(middle_edge_line);
                    if let Some(line) = end_edge_line {
                        new_edge_lines.push(line);
                    }
                    let new_room_shape = Simplex::from_lines(new_edge_lines);
                    // :637-648.
                    if !new_room_shape.is_empty() {
                        let new_room_tile = TileShape::Simplex(new_room_shape);
                        let new_contained_shape = rooms
                            .room_shape(self.completed_room)
                            .map(|shape| shape.intersection(&new_room_tile));
                        if let Some(new_contained_shape) = new_contained_shape
                            && !new_contained_shape.is_empty()
                        {
                            let new_room = RoomRef::Incomplete(rooms.new_incomplete_room(
                                Some(new_room_tile),
                                from_room_layer,
                                Some(new_contained_shape),
                            ));
                            let new_door = rooms.new_door(self.completed_room, new_room, 1);
                            rooms.add_door(self.completed_room, new_door);
                            rooms.add_door(new_room, new_door);
                        }
                    }
                    // :649-654.
                    if last_time {
                        break;
                    }
                    current_touching_side_no = next_touching_side_no;
                    start_edge_line = None;
                    first_time = false;
                }
            }
            // :657.
            prev_neighbour = next_neighbour;
        }
    }
}

// =================================================================================================
// The static helpers of `SortedRoomNeighbours`
// =================================================================================================

/// Port of the private static `SortedRoomNeighbours.calculateIncompleteRoomsWithEmptyNeighbours`
/// (SortedRoomNeighbours.java:138-156): one new incomplete room per border line of an obstacle
/// room that turned out to have no touching neighbour at all.
fn calculate_incomplete_rooms_with_empty_neighbours(
    room: RoomRef,
    board: &mut Board,
    rooms: &mut ExpansionRoomStore,
) {
    // :140.
    let room_shape = rooms
        .room_shape(room)
        .unwrap_or_else(|| {
            panic!(
                "SortedRoomNeighbours.calculateIncompleteRoomsWithEmptyNeighbours: room {room:?} \
                 has no shape (SortedRoomNeighbours.java:140) — Java NPEs here too"
            )
        })
        .clone();
    let layer = rooms
        .room_layer(board, room)
        .expect("the obstacle room is in the arena");
    // :141-155.
    for i in 0..room_shape.border_line_count() {
        let current_line = room_shape
            .border_line(i)
            .expect("i < borderLineCount, so the line exists");
        if insert_door_ok_for_obstacle_room(room, Some(&current_line), board, rooms) {
            // :144-147.
            let new_room_shape =
                TileShape::Simplex(Simplex::from_lines(vec![current_line.opposite()]));
            let new_contained_shape = room_shape.intersection(&new_room_shape);
            // :148-153.
            let new_room = RoomRef::Incomplete(rooms.new_incomplete_room(
                Some(new_room_shape),
                layer,
                Some(new_contained_shape),
            ));
            let new_door = rooms.new_door(room, new_room, 1);
            rooms.add_door(room, new_door);
            rooms.add_door(new_room, new_door);
        }
    }
}

/// Port of the private static `SortedRoomNeighbours.calculateTargetDoors`
/// (SortedRoomNeighbours.java:158-185).
fn calculate_target_doors(
    room: RoomId,
    own_net_objects: &[TreeEntry<TreeObject>],
    net_number: i32,
    board: &mut Board,
    rooms: &mut ExpansionRoomStore,
    tree_id: TreeId,
) {
    // :162-164.
    if !own_net_objects.is_empty()
        && let Some(r) = rooms.complete_room_mut(room)
    {
        r.set_net_dependent();
    }
    // :165-184.
    for current_entry in own_net_objects {
        // :166: `currentEntry.object instanceof Connectable` — a `TreeObject::Room` never is.
        let TreeObject::Item(item_id) = current_entry.object else {
            continue;
        };
        let connection_shape = {
            let ctx = board.ctx();
            let Some(item) = board.items.item(item_id) else {
                continue;
            };
            let Some(connectable) = item.as_connectable() else {
                continue;
            };
            // :167.
            if !connectable.as_dyn().contains_net(net_number) {
                continue;
            }
            // :168-170.
            connectable.as_dyn().get_trace_connection_shape(
                tree_id,
                current_entry.shape_index,
                &ctx,
            )
        };
        // :171-172.
        let Some(connection_shape) = connection_shape else {
            continue;
        };
        let intersects = rooms
            .complete_room(room)
            .and_then(|r| r.get_shape())
            .is_some_and(|shape| shape.intersects(&connection_shape));
        if !intersects {
            continue;
        }
        // :173-180.
        let new_target_door = rooms.new_target_door(
            board,
            item_id,
            current_entry.shape_index,
            Some(RoomRef::Complete(room)),
            tree_id,
        );
        if let Some(r) = rooms.complete_room_mut(room) {
            r.add_target_door(new_target_door);
        }
    }
}

/// Port of the package-private static `SortedRoomNeighbours.insertDoorOk(ExpansionRoom,
/// ExpansionRoom, TileShape)` (SortedRoomNeighbours.java:332-368). "Door shape is expected to
/// have dimension 1."
pub fn insert_door_ok(
    room1: RoomRef,
    room2: RoomRef,
    door_shape: &TileShape,
    board: &Board,
    rooms: &ExpansionRoomStore,
) -> bool {
    // :333-335.
    if rooms.door_exists(room1, room2) {
        return false;
    }
    // :336-342. "insert only overlap_doors between items of the same net for performance
    // reasons."
    if let (RoomRef::Obstacle(id1), RoomRef::Obstacle(id2)) = (room1, room2) {
        let (Some(r1), Some(r2)) = (rooms.obstacle_room(id1), rooms.obstacle_room(id2)) else {
            return false;
        };
        let (Some(first), Some(second)) =
            (board.get_item(r1.get_item()), board.get_item(r2.get_item()))
        else {
            return false;
        };
        return first.shares_net(second);
    }
    // :343-345.
    if !matches!(room1, RoomRef::Obstacle(_)) && !matches!(room2, RoomRef::Obstacle(_)) {
        return true;
    }
    // :346-358. "Insert 1 dimensional doors of trace rooms only, if they are parallel to the
    // trace line. Otherwise, there may be check ripup problems with entering at the wrong side at
    // a fork."
    let mut door_line: Option<Line> = None;
    let mut prev_corner = door_shape.corner(0);
    let corner_count = door_shape.border_line_count();
    for i in 1..corner_count {
        let current_corner = door_shape.corner(i);
        if current_corner != prev_corner {
            door_line = door_shape.border_line(i - 1);
            break;
        }
        prev_corner = current_corner;
    }
    // :359-367.
    if matches!(room1, RoomRef::Obstacle(_))
        && !insert_door_ok_for_obstacle_room(room1, door_line.as_ref(), board, rooms)
    {
        return false;
    }
    if matches!(room2, RoomRef::Obstacle(_)) {
        return insert_door_ok_for_obstacle_room(room2, door_line.as_ref(), board, rooms);
    }
    true
}

/// Port of the private static `SortedRoomNeighbours.insertDoorOk(ObstacleExpansionRoom, Line)`
/// (SortedRoomNeighbours.java:375-389): "insert 1 dimensional doors for the first and the last
/// room of a trace rooms only, if they are parallel to the trace line. Otherwise, there may be
/// check ripup problems with entering at the wrong side at a fork."
///
// renamed: the second `SortedRoomNeighbours.insertDoorOk` overload is
// `insert_door_ok_for_obstacle_room` — Rust has no overloading.
fn insert_door_ok_for_obstacle_room(
    room: RoomRef,
    door_line: Option<&Line>,
    board: &Board,
    rooms: &ExpansionRoomStore,
) -> bool {
    // :376-379: `FRLogger.warn` (dropped) and `return false`.
    let Some(door_line) = door_line else {
        return false;
    };
    let RoomRef::Obstacle(id) = room else {
        // Java's parameter is typed `ObstacleExpansionRoom`, so this arm cannot happen.
        return true;
    };
    let Some(obstacle_room) = rooms.obstacle_room(id) else {
        return true;
    };
    // :380-387.
    let Some(Item::Trace(current_trace)) = board.get_item(obstacle_room.get_item()) else {
        return true;
    };
    let room_index = obstacle_room.get_index_in_item();
    if room_index == 0 || room_index + 1 == current_trace.tile_shape_count() {
        // :384: `currentTrace.polyline().lines[roomIndex + 1]`.
        let lines = current_trace.polyline().lines();
        let Some(current_trace_line) = lines.get(room_index + 1) else {
            panic!(
                "SortedRoomNeighbours.insertDoorOk: trace line {} of {} \
                 (SortedRoomNeighbours.java:384) — Java throws ArrayIndexOutOfBoundsException here",
                room_index + 1,
                lines.len()
            )
        };
        return current_trace_line.is_parallel(door_line);
    }
    // :388.
    true
}

/// Port of `ObstacleExpansionRoom.createOverlapDoor(ObstacleExpansionRoom)`
/// (ObstacleExpansionRoom.java:77-100): "creates a 2-dim door with the other obstacle room if
/// that is useful for the autoroute algorithm. It is assumed that this room and other have a
/// 2-dimensional overlap. Returns false if no door was created."
///
/// It lives here rather than on [`ObstacleExpansionRoom`](crate::ObstacleExpansionRoom) because the last three of its five
/// guards need the board (`Item.isRoutable`, `Item.sharesNet`, `instanceof PolylineTrace`) and
/// the door it builds has to go into the store's arena; the marker on `obstacle_room.rs` names
/// this function.
pub fn create_overlap_door(
    room: ObstacleRoomId,
    other: ObstacleRoomId,
    board: &Board,
    rooms: &mut ExpansionRoomStore,
) -> bool {
    let this_ref = RoomRef::Obstacle(room);
    let other_ref = RoomRef::Obstacle(other);
    // :78-80.
    if rooms.door_exists(this_ref, other_ref) {
        return false;
    }
    let (Some(this_room), Some(other_room)) =
        (rooms.obstacle_room(room), rooms.obstacle_room(other))
    else {
        return false;
    };
    let (this_item_id, other_item_id) = (this_room.get_item(), other_room.get_item());
    let (this_index, other_index) = (
        this_room.get_index_in_item(),
        other_room.get_index_in_item(),
    );
    let (Some(this_item), Some(other_item)) =
        (board.get_item(this_item_id), board.get_item(other_item_id))
    else {
        return false;
    };
    // :81-83.
    if !(this_item.is_routable() && other_item.is_routable()) {
        return false;
    }
    // :84-86.
    if !this_item.shares_net(other_item) {
        return false;
    }
    // :87-95. "create only doors between consecutive trace segments"
    if this_item_id == other_item_id {
        if !matches!(this_item, Item::Trace(_)) {
            return false;
        }
        // `this.indexInItem != other.indexInItem + 1 && this.indexInItem != other.indexInItem - 1`
        // over Java `int`s, so `other.indexInItem == 0` compares against -1 rather than wrapping.
        let (this_index, other_index) = (this_index as i64, other_index as i64);
        if this_index != other_index + 1 && this_index != other_index - 1 {
            return false;
        }
    }
    // :96-99.
    let new_door = rooms.new_door(this_ref, other_ref, 2);
    rooms.add_door(this_ref, new_door);
    rooms.add_door(other_ref, new_door);
    true
}

// =================================================================================================
// The inner class
// =================================================================================================

// not ported: `SortedRoomNeighbours.c_dist_tolerance` (SortedRoomNeighbours.java:667), the
// `1.0` band `compareTo` used to decide *which* key answers. fixed: T8 (#160) removed the last
// reader — see `SortedRoomNeighbour::compare_to`'s change 2, where a tolerance that selects a key
// and then takes the exact sign of it is the reason the relation was not transitive.

/// Port of the private inner class `SortedRoomNeighbours.SortedRoomNeighbour`
/// (SortedRoomNeighbours.java:665-806): "helper class to sort the doors of an expansion room
/// counterclockwise around the border of the room shape."
///
/// Java's inner class reaches the outer instance's `roomShape` through the implicit `this$0`
/// reference; the port stores a copy, which is equivalent because the field is assigned once in
/// the outer constructor (`:53`) and never written again.
#[derive(Debug, Clone)]
pub struct SortedRoomNeighbour {
    /// `searchTreeObject` (:670): "the search tree object of the neighbour room."
    pub search_tree_object: TreeObject,
    /// `searchTreeObject.getId()`, resolved once at construction because the port's leaves hold
    /// a key rather than the object. **Two id spaces meet here** — see hazard G in the module
    /// docs (quirk #161).
    pub object_id: i32,
    /// `neighbourShape` (:673): "the shape of the neighbour room."
    pub neighbour_shape: TileShape,
    /// `intersection` (:676): "the intersection of this ExpansionRoom shape with the
    /// neighbourShape."
    pub intersection: TileShape,
    /// `touchingSideNoOfRoom` (:679): "the side number of this room, where it touches the
    /// neighbour."
    ///
    /// `i32`, not `usize`: `:297-300` keeps the `-1` that `containsOnBorderLineNo` answers when
    /// the touching point is on no border line, logs a debug message and uses it anyway.
    pub touching_side_no_of_room: i32,
    /// `touchingSideNoOfNeighbourRoom` (:682): "the side number of the neighbour room, where it
    /// touches this room." Also `-1`-bearing (`:312-316`).
    pub touching_side_no_of_neighbour_room: i32,
    /// `roomTouchIsCorner` (:687): "true, if the intersection of this room and the neighbour is
    /// equal to a corner of this room."
    pub room_touch_is_corner: bool,
    /// `neighbourRoomTouchIsCorner` (:693): "true, if the intersection of this room and the
    /// neighbour is equal to a corner of the neighbour room."
    pub neighbour_room_touch_is_corner: bool,
    /// The outer instance's `roomShape` (:45), reached in Java through `this$0`.
    pub room_shape: TileShape,
    /// `precalculatedFirstCorner` (:695) — lazily filled by [`Self::first_corner`], exactly as
    /// Java's is, so an input that would make the computation throw only throws where Java's
    /// would.
    first_corner: OnceCell<Point>,
    /// `precalculatedLastCorner` (:696).
    last_corner: OnceCell<Point>,
}

impl SortedRoomNeighbour {
    /// Port of the constructor `SortedRoomNeighbour(...)` (SortedRoomNeighbours.java:698-713).
    ///
    // renamed: the inner class's constructor `SortedRoomNeighbour` is `SortedRoomNeighbour::new`.
    ///
    /// The port takes two arguments Java's does not: `object_id`, because the tree's leaves hold
    /// a [`TreeObject`] key rather than the `SearchTreeObject` whose `getId()` the comparator
    /// subtracts, and `room_shape`, because there is no outer instance to reach through.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        search_tree_object: TreeObject,
        object_id: i32,
        neighbour_shape: TileShape,
        intersection: TileShape,
        touching_side_no_of_room: i32,
        touching_side_no_of_neighbour_room: i32,
        room_touch_is_corner: bool,
        neighbour_room_touch_is_corner: bool,
        room_shape: TileShape,
    ) -> SortedRoomNeighbour {
        SortedRoomNeighbour {
            search_tree_object,
            object_id,
            neighbour_shape,
            intersection,
            touching_side_no_of_room,
            touching_side_no_of_neighbour_room,
            room_touch_is_corner,
            neighbour_room_touch_is_corner,
            room_shape,
            first_corner: OnceCell::new(),
            last_corner: OnceCell::new(),
        }
    }

    /// Port of `firstCorner()` (SortedRoomNeighbours.java:765-784): "returns the first corner of
    /// the intersection shape with the neighbour."
    pub fn first_corner(&self) -> &Point {
        self.first_corner.get_or_init(|| {
            if self.room_touch_is_corner {
                // :767-768.
                self.room_shape.corner(index_of(
                    self.touching_side_no_of_room,
                    "touchingSideNoOfRoom",
                    768,
                ))
            } else if self.neighbour_room_touch_is_corner {
                // :769-770.
                self.neighbour_shape.corner(index_of(
                    self.touching_side_no_of_neighbour_room,
                    "touchingSideNoOfNeighbourRoom",
                    770,
                ))
            } else {
                // :771-780.
                let current_first_corner =
                    self.neighbour_shape
                        .corner(self.neighbour_shape.next_no(index_of(
                            self.touching_side_no_of_neighbour_room,
                            "touchingSideNoOfNeighbourRoom",
                            773,
                        )));
                let prev_line = border_line_of(
                    &self.room_shape,
                    self.room_shape.prev_no(index_of(
                        self.touching_side_no_of_room,
                        "touchingSideNoOfRoom",
                        774,
                    )) as i32,
                    "prevNo(touchingSideNoOfRoom)",
                    774,
                );
                if prev_line.side_of(&current_first_corner) == Side::OnTheRight {
                    current_first_corner
                } else {
                    // "currentFirstCorner is outside the door shape"
                    self.room_shape.corner(index_of(
                        self.touching_side_no_of_room,
                        "touchingSideNoOfRoom",
                        779,
                    ))
                }
            }
        })
    }

    /// Port of `lastCorner()` (SortedRoomNeighbours.java:787-805): "returns the last corner of
    /// the intersection shape with the neighbour."
    pub fn last_corner(&self) -> &Point {
        self.last_corner.get_or_init(|| {
            if self.room_touch_is_corner {
                // :789-790.
                self.room_shape.corner(index_of(
                    self.touching_side_no_of_room,
                    "touchingSideNoOfRoom",
                    790,
                ))
            } else if self.neighbour_room_touch_is_corner {
                // :791-792.
                self.neighbour_shape.corner(index_of(
                    self.touching_side_no_of_neighbour_room,
                    "touchingSideNoOfNeighbourRoom",
                    792,
                ))
            } else {
                // :793-801.
                let current_last_corner = self.neighbour_shape.corner(index_of(
                    self.touching_side_no_of_neighbour_room,
                    "touchingSideNoOfNeighbourRoom",
                    794,
                ));
                let next_line = border_line_of(
                    &self.room_shape,
                    self.room_shape.next_no(index_of(
                        self.touching_side_no_of_room,
                        "touchingSideNoOfRoom",
                        795,
                    )) as i32,
                    "nextNo(touchingSideNoOfRoom)",
                    795,
                );
                if next_line.side_of(&current_last_corner) == Side::OnTheRight {
                    current_last_corner
                } else {
                    // "currentLastCorner is outside the door shape"
                    self.room_shape.corner(self.room_shape.next_no(index_of(
                        self.touching_side_no_of_room,
                        "touchingSideNoOfRoom",
                        800,
                    )))
                }
            }
        })
    }

    /// Port of `compareTo(SortedRoomNeighbour)` (SortedRoomNeighbours.java:719-762): "compare
    /// function for sorting the neighbours in counterclock sense around the border of the room
    /// shape in ascending order."
    ///
    /// fixed: T8 (#160, #161). Java's version is **not** a total order and the `TreeSet` it feeds
    /// (`:54`, inserted at `:408`) silently drops every element it calls equal — a door the room
    /// really has is never built. This is a **lexicographic** comparison of the same keys in the
    /// same order, with the three defects removed; see [`Self::compare_to`]'s own notes below and
    /// hazards F and G in the module docs.
    ///
    /// # The three changes, each against the line it replaces
    ///
    /// **1. `:729-733`'s inner `firstCorner().equals(other.firstCorner())` gate is gone.** Java
    /// refines by the *last* corner only when the two first corners are the *same point*, which is
    /// strictly stronger than the distances tying — two different corners equidistant from the
    /// compare corner (`(300,400)` and `(400,300)` at 500 from `(0,0)`) skip the refinement, keep
    /// `deltaDistance == 0.0`, and fall through to an id tie-break that answers `0` whenever they
    /// come from the same object. That is #160's drop, and both corners are real, different places
    /// on the same wall. The refinement now runs whenever the first-corner distances are equal.
    ///
    /// **2. `c_dist_tolerance` no longer gates which key decides.** Java's `<= 1.0` band selects
    /// *which* key answers and then takes the exact sign of whichever it selected, which is the
    /// textbook non-transitivity: `a ≈ b` and `b ≈ c` do not give `a ≈ c`, so the relation is not
    /// a strict weak ordering and a `TreeSet`'s red-black invariants stop meaning anything. The
    /// keys are compared exactly instead, and "tie" means *equal*. This is strictly a refinement
    /// of Java's order — every pair Java called a tie either still ties (the distances really are
    /// equal, and the last corner decides exactly as Java intended) or is separated by a genuine
    /// difference in where the door starts, which is a legal counterclockwise order either way.
    /// `c_dist_tolerance` itself has no reader left and is gone with the gate.
    ///
    /// **3. `:756-760`'s id subtraction compares the object *kind* first (#161), and does not
    /// wrap.** `searchTreeObject.getId()` is a `BasicBoard.ItemIdGenerator` number for an item and
    /// an `AutorouteEngine.expansionRoomInstanceCount` number for a room; both start at 1, they are
    /// allocated independently, and subtracting one from the other made "item 3" and "room 3" tie
    /// — one of them dropped. Comparing the kind first (an item before a room, arbitrarily but
    /// **consistently**) makes the pair total inside each id space. The subtraction also wrapped,
    /// so two ids more than `i32::MAX` apart ordered backwards; `Ord::cmp` cannot.
    ///
    /// # Why there are keys past the id
    ///
    /// A set drops an element only when the comparator answers `Equal`, so "no neighbour is lost"
    /// is exactly "`Equal` implies equal as a value". Two neighbours of the **same object** — one
    /// item contributing two tree shapes that touch the same side — share every key up to and
    /// including the id, so the remaining value fields are compared after it. They are Java's
    /// fields, in Java's declaration order, and they are only ever reached where Java answered
    /// `0` and lost one of the two.
    ///
    /// # The one place a key is skipped rather than compared
    ///
    /// `:741-751`'s `Direction.compareFrom` refinement needs a `compareDir` built from
    /// `this.roomTouchIsCorner` — a field of the **left** operand — so on a pair whose flags
    /// differ Java's own comparison is asymmetric: `a.compareTo(b)` and `b.compareTo(a)` would
    /// consult different reference directions. The refinement is therefore taken only when both
    /// operands agree about it, and the pair falls to the kind/id keys otherwise. `compareFrom`
    /// itself is a total order on directions for a fixed reference (`IntDirection::compare_from`),
    /// so where it does run it is sound.
    pub fn compare_to(&self, other: &SortedRoomNeighbour) -> Ordering {
        // :721-724. Java subtracts two `int`s and takes the sign; `cmp` is the same answer without
        // the wrap.
        match self
            .touching_side_no_of_room
            .cmp(&other.touching_side_no_of_room)
        {
            Ordering::Equal => {}
            ordering => return ordering,
        }
        // :725-728.
        let compare_corner = self
            .room_shape
            .corner_approx(index_of(
                self.touching_side_no_of_room,
                "touchingSideNoOfRoom",
                725,
            ))
            .unwrap_or_else(|| {
                panic!(
                    "SortedRoomNeighbour.compareTo: the room shape has no corner \
                     {} (SortedRoomNeighbours.java:725) — Java NPEs here too",
                    self.touching_side_no_of_room
                )
            });
        let this_distance = self.first_corner().to_float().distance(&compare_corner);
        let other_distance = other.first_corner().to_float().distance(&compare_corner);
        match this_distance.total_cmp(&other_distance) {
            Ordering::Equal => {}
            ordering => return ordering,
        }
        // :734-737. "in this case compare the last corners" — now reached whenever the first
        // corners are equidistant, not only when they are the same point.
        let this_distance2 = self.last_corner().to_float().distance(&compare_corner);
        let other_distance2 = other.last_corner().to_float().distance(&compare_corner);
        match this_distance2.total_cmp(&other_distance2) {
            Ordering::Equal => {}
            ordering => return ordering,
        }
        // The two corner flags, **before** the refinement they gate. `:738-740` takes the
        // `Direction.compareFrom` branch only when both neighbours are
        // `neighbourRoomTouchIsCorner`, and builds its reference direction out of the **left**
        // operand's `roomTouchIsCorner` — so a conditional key applies to some pairs and not
        // others, and that alone is enough to destroy transitivity even with every key below it
        // total: `a` (corner) against `b` (not) falls to the id, `b` against `c` (corner) falls to
        // the id, and `a` against `c` is decided by direction, which need not agree. Measured on
        // `p6t3` mode 3's own generator, case 118: `0 <= 1 <= 3` while `0 > 3`.
        //
        // Comparing the flags first confines the refinement to one equivalence class, where it is
        // a genuine total order (`IntDirection::compare_from` is the circular order rotated to
        // start at the reference, and the reference is then the same for every member of the
        // class). `false` before `true`, arbitrarily and consistently.
        match self.room_touch_is_corner.cmp(&other.room_touch_is_corner) {
            Ordering::Equal => {}
            ordering => return ordering,
        }
        match self
            .neighbour_room_touch_is_corner
            .cmp(&other.neighbour_room_touch_is_corner)
        {
            Ordering::Equal => {}
            ordering => return ordering,
        }
        // :738-752. "Otherwise there may be a short 1 dim. touch at a link between 2 trace lines.
        // In this case equality is ok, because the 2 intersection pieces with the expansion room
        // are identical, so that only 1 obstacle is needed."
        if self.neighbour_room_touch_is_corner {
            let mut compare_line_no = self.touching_side_no_of_room;
            if self.room_touch_is_corner {
                compare_line_no =
                    self.room_shape
                        .prev_no(index_of(compare_line_no, "touchingSideNoOfRoom", 743))
                        as i32;
            }
            let compare_dir =
                border_line_of(&self.room_shape, compare_line_no, "compareLineNo", 745)
                    .direction()
                    .opposite();
            let this_compare_line = border_line_of(
                &self.neighbour_shape,
                self.touching_side_no_of_neighbour_room,
                "touchingSideNoOfNeighbourRoom",
                747,
            );
            let other_compare_line = border_line_of(
                &other.neighbour_shape,
                other.touching_side_no_of_neighbour_room,
                "touchingSideNoOfNeighbourRoom",
                749,
            );
            match compare_dir.compare_from(
                &this_compare_line.direction(),
                &other_compare_line.direction(),
            ) {
                Ordering::Equal => {}
                ordering => return ordering,
            }
        }
        // :756-760's tie-break, with the kind ahead of the id (#161) and no wrapping subtraction.
        match object_kind_rank(self.search_tree_object)
            .cmp(&object_kind_rank(other.search_tree_object))
        {
            Ordering::Equal => {}
            ordering => return ordering,
        }
        match self.object_id.cmp(&other.object_id) {
            Ordering::Equal => {}
            ordering => return ordering,
        }
        // Past Java's last key: the remaining value fields, so that `Equal` means "equal as a
        // value" and a set can no longer drop a door the room really has.
        match self
            .touching_side_no_of_neighbour_room
            .cmp(&other.touching_side_no_of_neighbour_room)
        {
            Ordering::Equal => {}
            ordering => return ordering,
        }
        // The corners themselves, last: two neighbours of the same object that agree on every key
        // above can still start and end at different places (the two equidistant corners of the
        // paragraph at the top), and those are two doors, not one.
        corner_key(self.first_corner())
            .cmp(&corner_key(other.first_corner()))
            .then_with(|| corner_key(self.last_corner()).cmp(&corner_key(other.last_corner())))
            // The neighbour's own shape, last and lazily: one *item* contributes one neighbour per
            // tree shape and every one of them carries the same object id, so this is the key that
            // separates two tree shapes of one item which touch the same side at the same corners.
            // It is reached only when the ten keys above have all tied.
            .then_with(|| shape_key(&self.neighbour_shape).cmp(&shape_key(&other.neighbour_shape)))
    }
}

/// `Item` before `Room` — the object *kind*, which #161's tie-break has to consult before the id
/// because the two ids come from two independent counters that both start at 1.
///
/// The direction is arbitrary and the *consistency* is what matters; `Item` first because that is
/// the order [`TreeObject`]'s own derived `Ord` uses.
fn object_kind_rank(object: TreeObject) -> u8 {
    match object {
        TreeObject::Item(_) => 0,
        TreeObject::Room(_) => 1,
    }
}

/// A [`Point`] as a totally-ordered key. `Point` is an enum of an `IntPoint` and a
/// `RationalPoint` and has no `Ord`; its float projection does, through `total_cmp`, and the two
/// corners this is used on are exact integer corners of tile shapes in every reachable case.
fn corner_key(point: &Point) -> (OrderedF64, OrderedF64) {
    let float = point.to_float();
    (OrderedF64(float.x), OrderedF64(float.y))
}

/// A [`TileShape`] as a totally-ordered key: its dimension, then its corners. `TileShape` has no
/// `Ord` and does not need one — this exists only as [`SortedRoomNeighbour::compare_to`]'s last
/// resort, where every other key has tied.
fn shape_key(shape: &TileShape) -> (i32, Vec<(OrderedF64, OrderedF64)>) {
    (
        shape.dimension(),
        shape
            .corner_approx_arr()
            .iter()
            .map(|corner| (OrderedF64(corner.x), OrderedF64(corner.y)))
            .collect(),
    )
}

/// An `f64` with a total order (`f64::total_cmp`), so a tuple of two can be `cmp`ed.
#[derive(Debug, Clone, Copy, PartialEq)]
struct OrderedF64(f64);

impl Eq for OrderedF64 {}

impl PartialOrd for OrderedF64 {
    fn partial_cmp(&self, other: &OrderedF64) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedF64 {
    fn cmp(&self, other: &OrderedF64) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

impl PartialEq for SortedRoomNeighbour {
    fn eq(&self, other: &SortedRoomNeighbour) -> bool {
        self.compare_to(other) == Ordering::Equal
    }
}

impl Eq for SortedRoomNeighbour {}

impl PartialOrd for SortedRoomNeighbour {
    fn partial_cmp(&self, other: &SortedRoomNeighbour) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SortedRoomNeighbour {
    fn cmp(&self, other: &SortedRoomNeighbour) -> Ordering {
        self.compare_to(other)
    }
}

// =================================================================================================
// Resolving a stored `SearchTreeObject`, and the index helpers
// =================================================================================================

/// The autoroute search tree, resolved out of the board's manager.
///
/// Java writes `autorouteEngine.autorouteSearchTree`, a field the engine's constructor filled
/// from `board.searchTreeManager.getAutorouteTree(..)` (AutorouteEngine.java:88); the port
/// carries the [`TreeId`] and looks the tree back up, so nothing borrows the board across a
/// mutation.
pub(crate) fn tree_of(board: &Board, tree_id: TreeId) -> &ShapeSearchTree {
    board
        .trees
        .trees()
        .find(|tree| tree.id() == tree_id)
        .unwrap_or_else(|| panic!("SortedRoomNeighbours: no search tree with id {tree_id:?}"))
}

/// `((SearchTreeObject) currentEntry.object).getId()` (SortedRoomNeighbours.java:208, `:759`).
///
/// `Item.getId()` for an item and `CompleteFreeSpaceExpansionRoom.getId()` for a room — the two
/// id spaces hazard G is about.
pub(crate) fn object_id(object: TreeObject, rooms: &ExpansionRoomStore) -> i32 {
    match object {
        TreeObject::Item(id) => id.0 as i32,
        TreeObject::Room(id) => rooms.complete_room(id).map_or(0, |room| room.get_id()),
    }
}

/// `currentObject.isTraceObstacle(netNumber)` (SortedRoomNeighbours.java:223).
///
/// `CompleteFreeSpaceExpansionRoom.isTraceObstacle` is the constant `true`
/// (CompleteFreeSpaceExpansionRoom.java:81-84).
pub(crate) fn object_is_trace_obstacle(
    object: TreeObject,
    net_number: i32,
    items: &impl ItemLookup,
) -> bool {
    match object {
        TreeObject::Item(id) => items
            .item(id)
            .is_some_and(|item| item.is_trace_obstacle(net_number)),
        TreeObject::Room(_) => true,
    }
}

/// `currentObject.getTreeShape(autorouteSearchTree, currentEntry.shapeIndexInObject)`
/// (SortedRoomNeighbours.java:228-229).
pub(crate) fn object_tree_shape(
    tree: &ShapeSearchTree,
    object: TreeObject,
    shape_index: usize,
    items: &impl ItemLookup,
    rooms: &impl RoomLookup,
    ctx: &ItemCtx<'_>,
) -> TileShape {
    match object {
        TreeObject::Item(id) => {
            let item = items.item(id).unwrap_or_else(|| {
                panic!(
                    "SortedRoomNeighbours.calculateNeighbours: item {id} has a leaf but is not on \
                     the board"
                )
            });
            tree.get_tree_shape(item, shape_index, ctx)
                .map(std::borrow::Cow::into_owned)
                .unwrap_or_else(|| {
                    panic!(
                        "SortedRoomNeighbours.calculateNeighbours: item {id} has a leaf for shape \
                         {shape_index} but no shape — Java NPEs here too"
                    )
                })
        }
        TreeObject::Room(id) => rooms
            .room_tree_shape(id)
            .unwrap_or_else(|| {
                panic!(
                    "SortedRoomNeighbours.calculateNeighbours: expansion room {id:?} has a leaf \
                     but no shape — Java NPEs here too"
                )
            })
            .clone(),
    }
}

/// A Java `int` shape index, narrowed to the `usize` the port's geometry takes.
///
/// Java keeps the `-1` that `containsOnBorderLineNo` answers (SortedRoomNeighbours.java:297-300,
/// `:312-316`) and then indexes an array with it; the port panics with the Java line rather than
/// wrapping into a huge index.
fn index_of(no: i32, what: &str, java_line: u32) -> usize {
    usize::try_from(no).unwrap_or_else(|_| {
        panic!(
            "SortedRoomNeighbours.java:{java_line}: {what} is {no} — Java throws \
             ArrayIndexOutOfBoundsException here (the -1 comes from :297-300 / :312-316, which \
             log it and use it anyway)"
        )
    })
}

/// `shape.borderLine(no)` for a Java `int` index.
fn border_line_of(shape: &TileShape, no: i32, what: &str, java_line: u32) -> Line {
    let index = index_of(no, what, java_line);
    shape.border_line(index).unwrap_or_else(|| {
        panic!(
            "SortedRoomNeighbours.java:{java_line}: {what} is {no}, past the shape's \
             {} border lines — Java throws ArrayIndexOutOfBoundsException here",
            shape.border_line_count()
        )
    })
}
