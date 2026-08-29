//! Port of `autoroute.expansion.SortedOrthogonalRoomNeighbours`
//! (SortedOrthogonalRoomNeighbours.java:19-728) — the neighbour sorter
//! [`select_calculation_mode`](super::sorted_neighbours::select_calculation_mode) picks on a
//! 90-degree autoroute tree.
//!
//! Like its 45-degree sibling it declares its **own** inner `SortedRoomNeighbour` (`:598-727`)
//! with its own constructor and its own `compareTo`; the only thing it shares with the base class
//! is the package-private static `insertDoorOk` (`:200`).
//!
//! # What is different from the base class, and matters
//!
//! * Every shape is an [`IntBox`], and the casts are **hard**: the constructor casts
//!   `completedRoom.getShape()` (`:34`), so a non-box completed room is a `ClassCastException`;
//!   `calculateNeighbours` checks the room's shape (`:117-120`) and each neighbour's tree shape
//!   (`:160-165`) and answers `null` instead. That second `null` aborts the whole calculation —
//!   not just the one neighbour.
//! * A **2-dimensional overlap is only skipped for an obstacle room** (`:168`), exactly as in the
//!   45-degree class and unlike the base (SortedRoomNeighbours.java:232-247) — and the door it
//!   then builds at `:201` is the **two-argument** `ExpansionDoor` constructor
//!   (ExpansionDoor.java:35-39), whose dimension is *computed* from the two rooms' shapes, where
//!   only the base class hard-codes `1` (SortedRoomNeighbours.java:281). That `dimension == 2`
//!   door is what `:468` scans for when it picks `completeShape`'s `ignoreObject`, so the arm
//!   bootstraps the room-enlargement path rather than being a curiosity. `p6t3` mode 7's
//!   `overlap` probe and the test
//!   `a_two_dimensional_overlap_is_an_orthogonal_neighbour_with_a_two_dimensional_door` cover
//!   it.
//! * `calculateTargetDoors` is `CompleteFreeSpaceExpansionRoom`'s per-entry method (`:155`,
//!   [`super::complete_room::calculate_target_doors`]), inside the neighbour loop.
//! * An obstacle room with **no** neighbours at all gets four incomplete rooms, one per side of
//!   the board's bounding box, with no `insertDoorOk` test at all (`:79-108`) — where the base
//!   class builds one room per border line and tests each (SortedRoomNeighbours.java:138-156).
//! * `calculateNewIncompleteRooms` (`:224-401`) is a four-way switch on the first touching side
//!   with an `isObstacleExpansionRoom` sub-case per arm ("no 2-dim doors between
//!   obstacle_expansion_rooms and free space rooms allowed"), not the base class's loop over the
//!   room's border lines.
//!
//! # Hazard F for this class — the comparator is a total order, unlike the base's
//!
//! `SortedRoomNeighbour.compareTo` (`:673-726`) is the lexicographic order on
//! `(firstTouchingSide, one ordinate of the intersection's lower-left/upper-right corner, the
//! touching-side span, one ordinate of the other corner, the object id)`. Every key is a Java
//! `int` and every refinement is entered on the same `cmpValue == 0` condition for both operands,
//! so — unlike `SortedRoomNeighbours.SortedRoomNeighbour.compareTo` (quirk #160) — it is
//! transitive and antisymmetric.
//!
//! **That conclusion rests on a lemma, and it is not the 45-degree class's.** The two `switch`es
//! (`:687`, `:710`) dispatch on `firstTouchingSide` and `lastTouchingSide`, and a `-1` in one
//! operand only would make them select different ordinates — how antisymmetry breaks. The
//! 45-degree sibling excludes that with `addSortedNeighbour`'s `lastTouchingSide >= 0` filter;
//! **this class's `addSortedNeighbour` (`:587-592`) adds unconditionally**, so a `-1` really does
//! reach the set. It is still safe, for a geometric reason: `intersection` is
//! `roomBox.intersection(currentBox)` (`:166`), hence contained in `roomShape`, and under
//! containment each of the four `firstTouchingSide` arms (`:642-649`) forces one of the four
//! `lastTouchingSide` arms (`:655-662`) and vice versa — so `firstTouchingSide == -1` **iff**
//! `lastTouchingSide == -1`. The mixed case cannot occur, and a `(-1, -1)` member is a proper
//! equivalence class that the first `switch`'s `default` (`:692-695`, `return 0`) sorts equal to
//! every other `(-1, -1)` and the `firstTouchingSide` comparison sorts below everything else.
//!
//! Two neighbours whose five keys all agree still compare `Equal` and the `TreeSet` **drops** the
//! second, and the id tie-break still crosses the item and room id spaces (quirk #161). The
//! container is [`JavaTreeSet`] so that the drop happens exactly where Java's does.
//!
//! not ported: every `FRLogger` payload of this class — the `FRLogger.warn`s at `:83`, `:118`,
//! `:127`, `:161`, `:182`, `:218`, `:394`, `:432`, `:651`, `:664`, `:693` and `:716`, the seven
//! `ROOM_EDGE_REMOVE` `FRLogger.trace` blocks (`:448-457`, `:475-488`, `:493-505`, `:508-521`,
//! `:527-542`, `:550-561`, `:566-577`) and the two counters `ignoreCandidateCount` /
//! `equalAreaTieCount` (`:463-464`, `:474`, `:507`) that only those traces read — per
//! `global-constraints.md`.

use std::cmp::Ordering;

use fr_board::{Board, TreeId, TreeObject};
use fr_geometry::{CRIT_INT, IntBox, TileShape};

use crate::JavaTreeSet;
use crate::autoroute::expansion::complete_room::calculate_target_doors;
use crate::autoroute::expansion::sorted_neighbours::{
    create_overlap_door, insert_door_ok, object_id, object_is_trace_obstacle, object_tree_shape,
    tree_of,
};
use crate::autoroute::expansion::{ExpansionRoomStore, IncompleteFreeSpaceExpansionRoom, RoomRef};
use crate::autoroute::item_info;
use crate::autoroute::tree_ext::AutorouteSearchTreeExt;

/// Port of `SortedOrthogonalRoomNeighbours` (SortedOrthogonalRoomNeighbours.java:19-728).
#[derive(Debug, Clone)]
pub struct SortedOrthogonalRoomNeighbours {
    /// `completedRoom` (:21).
    pub completed_room: RoomRef,
    /// `sortedNeighbours` (:22), a `TreeSet` (:35).
    pub sorted_neighbours: JavaTreeSet<SortedRoomNeighbour>,
    /// `fromRoom` (:23).
    pub from_room: RoomRef,
    /// `isObstacleExpansionRoom` (:24), computed once in the constructor (`:33`) from the **from**
    /// room, not the completed one.
    pub is_obstacle_expansion_room: bool,
    /// `roomShape` (:25) — `(IntBox) completedRoom.getShape()` **captured in the constructor**
    /// (`:34`).
    pub room_shape: IntBox,
    /// `edgeInteriorTouchesObstacle` (:26), four flags (`:36-39`). Written by the inner class's
    /// constructor (`:621-640`) and read by `tryRemoveEdge` (`:439`).
    pub edge_interior_touches_obstacle: [bool; 4],
}

impl SortedOrthogonalRoomNeighbours {
    /// Port of the private constructor (SortedOrthogonalRoomNeighbours.java:29-40).
    fn new(
        from_room: RoomRef,
        completed_room: RoomRef,
        room_shape: IntBox,
    ) -> SortedOrthogonalRoomNeighbours {
        SortedOrthogonalRoomNeighbours {
            completed_room,
            sorted_neighbours: JavaTreeSet::new(),
            from_room,
            is_obstacle_expansion_room: matches!(from_room, RoomRef::Obstacle(_)),
            room_shape,
            edge_interior_touches_obstacle: [false; 4],
        }
    }

    /// Port of the public static `SortedOrthogonalRoomNeighbours.calculate(ExpansionRoom,
    /// AutorouteEngine)` (SortedOrthogonalRoomNeighbours.java:43-77): "calculates the completed
    /// expansion room for orthogonal routing."
    ///
    /// **Java wins over the brief's name and signature**, for the same reason the 45-degree
    /// class's does: the Java method is `calculate(room, autorouteEngine)`, and neither
    /// `complete` nor `ignoreNet` exists in this class.
    pub fn calculate(
        room: RoomRef,
        net_number: i32,
        board: &mut Board,
        rooms: &mut ExpansionRoomStore,
        tree_id: TreeId,
    ) -> Option<RoomRef> {
        loop {
            // :45-51.
            let room_id_no = rooms.next_room_id_no();
            let room_neighbours = SortedOrthogonalRoomNeighbours::calculate_neighbours(
                room, net_number, board, rooms, tree_id, room_id_no,
            )?;

            // :56-64. "Check, that each side of the room shape has at least one touching
            // neighbour. Otherwise, improve the room shape by enlarging."
            let edge_removed = room_neighbours.try_remove_edge(net_number, board, rooms, tree_id);
            let result = room_neighbours.completed_room;
            if edge_removed {
                rooms.remove_all_doors(result);
                continue;
            }

            // :66-75. "Now calculate the new incomplete rooms together with the doors between
            // this room and the sorted neighbours."
            if room_neighbours.sorted_neighbours.is_empty() {
                // :70-72 casts **`result`**, where the base class casts `room`
                // (SortedRoomNeighbours.java:121); in this branch they are the same object.
                if let RoomRef::Obstacle(_) = result {
                    calculate_incomplete_rooms_with_empty_neighbours(result, board, rooms);
                }
            } else {
                room_neighbours.calculate_new_incomplete_rooms(board, rooms);
            }
            // :76. No `calculateTargetDoors` here either — see `calculateNeighbours`.
            return Some(result);
        }
    }

    /// Port of the private static `SortedOrthogonalRoomNeighbours.calculateNeighbours`
    /// (SortedOrthogonalRoomNeighbours.java:114-209): "calculates all touching neighbours of room
    /// and sorts them in counterclock sense around the boundary of the room shape."
    ///
    /// `pub` where Java's is `private`, because it is the unit `p6t3` mode 7 drives directly.
    pub fn calculate_neighbours(
        room: RoomRef,
        net_number: i32,
        board: &mut Board,
        rooms: &mut ExpansionRoomStore,
        tree_id: TreeId,
        room_id_no: i32,
    ) -> Option<SortedOrthogonalRoomNeighbours> {
        // :116. Java would NPE below on a room with no shape; so does this.
        let room_shape = rooms
            .room_shape(room)
            .unwrap_or_else(|| {
                panic!(
                    "SortedOrthogonalRoomNeighbours.calculateNeighbours: room {room:?} has no \
                     shape (SortedOrthogonalRoomNeighbours.java:116) — Java NPEs here too"
                )
            })
            .clone();
        // :117-120: `FRLogger.warn` (dropped) and `return null`.
        let TileShape::Box(room_box) = room_shape else {
            return None;
        };
        let layer = rooms.room_layer(board, room).unwrap_or_else(|| {
            panic!(
                "SortedOrthogonalRoomNeighbours.calculateNeighbours: room {room:?} has no layer \
                 (SortedOrthogonalRoomNeighbours.java:123) — Java NPEs here too"
            )
        });

        // :121-129.
        let completed_room = match room {
            RoomRef::Incomplete(_) => RoomRef::Complete(rooms.new_complete_room(
                Some(room_shape.clone()),
                layer,
                room_id_no,
            )),
            RoomRef::Obstacle(id) => RoomRef::Obstacle(id),
            // :126-128: `FRLogger.warn` (dropped) and `return null`.
            RoomRef::Complete(_) => return None,
        };

        // :130. `(IntBox) completedRoom.getShape()` — the same shape object in both branches
        // above, so the cast cannot fail where the `:117` check passed.
        let mut result = SortedOrthogonalRoomNeighbours::new(room, completed_room, room_box);

        // :131-132.
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

        // :134-144. "Sort the overlapping objects deterministically to ensure parity with v1.9."
        overlapping_objects.sort_by(|e1, e2| {
            let id_diff = object_id(e1.object, rooms).wrapping_sub(object_id(e2.object, rooms));
            if id_diff != 0 {
                return id_diff.cmp(&0);
            }
            e1.shape_index.cmp(&e2.shape_index)
        });

        // :146-207. "Calculate the touching neighbour objects and sort them in counterclock sense
        // around the border of the room shape."
        for current_entry in overlapping_objects {
            let current_object = current_entry.object;
            // :150-152, the same dead reference test the base class carries.
            if rooms
                .get_object(room)
                .is_some_and(|object| room.is_free_space() && object == current_object)
            {
                continue;
            }

            // :153-157. The per-entry `CompleteFreeSpaceExpansionRoom.calculateTargetDoors`.
            if let RoomRef::Complete(free_room) = completed_room
                && !object_is_trace_obstacle(current_object, net_number, &board.items)
            {
                calculate_target_doors(
                    free_room,
                    &current_entry,
                    net_number,
                    board,
                    rooms,
                    tree_id,
                );
                continue;
            }

            // :158-165.
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
            // :160-165: a neighbour whose tree shape is not an `IntBox` aborts the **whole**
            // calculation (`FRLogger.warn` dropped, `return null`), not just this neighbour.
            let TileShape::Box(current_box) = current_shape else {
                return None;
            };
            let intersection = room_box.intersection(&current_box);
            let dimension = intersection.dimension();

            // :168-179. "only Obstacle expansion room may have a 2-dim overlap" — note the `&&`,
            // as in the 45-degree class: a 2-dimensional overlap with a free-space completed room
            // falls through to `addSortedNeighbour`.
            if dimension > 1
                && let RoomRef::Obstacle(obstacle_room) = completed_room
            {
                if let TreeObject::Item(item_id) = current_object
                    && board
                        .get_item(item_id)
                        .is_some_and(|item| item.is_routable())
                {
                    // :172-174.
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
                    // :175 hands the result straight to `createOverlapDoor`, which dereferences
                    // `other.item` at ObstacleExpansionRoom.java:81.
                    let overlap_room = overlap_room.unwrap_or_else(|| {
                        panic!(
                            "SortedOrthogonalRoomNeighbours.calculateNeighbours: item {item_id} \
                             has no expansion room for shape {} \
                             (SortedOrthogonalRoomNeighbours.java:175) — Java NPEs in \
                             createOverlapDoor here too",
                            current_entry.shape_index
                        )
                    });
                    create_overlap_door(obstacle_room, overlap_room, board, rooms);
                }
                continue;
            }
            // :180-184: `FRLogger.warn` (dropped) and `continue`.
            if dimension < 0 {
                continue;
            }

            // :185.
            result.add_sorted_neighbour(current_object, rooms, current_box, intersection);

            // :186-206. "make sure, that there is a door to the neighbour room."
            if dimension > 0 {
                let neighbour_room: Option<RoomRef> = match current_object {
                    // :189-190.
                    TreeObject::Room(id) => Some(RoomRef::Complete(id)),
                    // :191-198. "expand the item for ripup and pushing purposes"
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
                // :199-205.
                if let Some(neighbour_room) = neighbour_room
                    && insert_door_ok(
                        completed_room,
                        neighbour_room,
                        &TileShape::Box(intersection),
                        board,
                        rooms,
                    )
                {
                    // :201 is the **two**-argument `new ExpansionDoor(completedRoom,
                    // neighbourRoom)` (ExpansionDoor.java:35-39), which *computes* the door's
                    // dimension from the two rooms' shapes; only the base class hard-codes 1 at
                    // the same spot (SortedRoomNeighbours.java:281). `:468` then scans for a door
                    // with `dimension == 2` to pick `completeShape`'s `ignoreObject`, and this is
                    // the only site that can build one.
                    let new_door = rooms
                        .new_door_from_shapes(completed_room, neighbour_room)
                        .unwrap_or_else(|| {
                            panic!(
                                "SortedOrthogonalRoomNeighbours.calculateNeighbours: a door room \
                                 has no shape (SortedOrthogonalRoomNeighbours.java:201) — Java \
                                 NPEs in the ExpansionDoor constructor here too"
                            )
                        });
                    rooms.add_door(neighbour_room, new_door);
                    rooms.add_door(completed_room, new_door);
                }
            }
        }
        // :208.
        Some(result)
    }

    /// Port of the private `addSortedNeighbour` (SortedOrthogonalRoomNeighbours.java:587-592).
    /// Unlike the 45-degree class's (`:249`) there is no `lastTouchingSide >= 0` guard: a
    /// neighbour whose sides both came out `-1` is added anyway.
    fn add_sorted_neighbour(
        &mut self,
        search_tree_object: TreeObject,
        rooms: &ExpansionRoomStore,
        neighbour_shape: IntBox,
        intersection: IntBox,
    ) {
        let new_neighbour = SortedRoomNeighbour::new(
            search_tree_object,
            object_id(search_tree_object, rooms),
            neighbour_shape,
            intersection,
            &self.room_shape,
            &mut self.edge_interior_touches_obstacle,
        );
        self.sorted_neighbours.add(new_neighbour);
    }

    /// Port of the private `tryRemoveEdge` (SortedOrthogonalRoomNeighbours.java:426-585):
    /// "checks that each side of the room shape has at least one touching neighbour. Otherwise,
    /// the room shape will be improved by enlarging. Returns true if the room shape was changed."
    fn try_remove_edge(
        &self,
        net_number: i32,
        board: &mut Board,
        rooms: &mut ExpansionRoomStore,
        tree_id: TreeId,
    ) -> bool {
        // :427-429.
        let RoomRef::Incomplete(incomplete_id) = self.from_room else {
            return false;
        };
        // :430-434: `FRLogger.warn` (dropped) and `return false`.
        let Some(TileShape::Box(room_box)) = rooms.room_shape(self.from_room).cloned() else {
            return false;
        };
        // :435.
        let room_area = room_box.area();

        // :437-443. The **first** untouched side, not all of them — this is where the 45-degree
        // class differs (`:328-338` removes every untouched line at once).
        let mut remove_edge_no: i32 = -1;
        for i in 0..4 {
            if !self.edge_interior_touches_obstacle[i] {
                remove_edge_no = i as i32;
                break;
            }
        }
        if remove_edge_no < 0 {
            // :584.
            return false;
        }

        // :445-458. "Touching neighbour missing at the edge side with index removeEdgeNo. Remove
        // the edge line and restart the algorithm."
        let enlarged_box = remove_border_line(&room_box, remove_edge_no);

        // :459-526. "insert the overlapping doors with CompleteFreeSpaceExpansionRooms for the
        // information in complete_shape about the objects to ignore." The largest such door wins;
        // an equal-area tie keeps the first, and Java only counts it for a trace message.
        let mut ignore_shape: Option<TileShape> = None;
        let mut ignore_object: Option<TreeObject> = None;
        let mut max_door_area = 0.0f64;
        for door_id in rooms.room_doors(self.completed_room).to_vec() {
            let Some(door) = rooms.door(door_id) else {
                continue;
            };
            if door.dimension != 2 {
                continue;
            }
            let Some(RoomRef::Complete(other_room)) = door.other_room(self.completed_room) else {
                continue;
            };
            let Some(current_door_shape) = rooms.door_shape(door_id) else {
                continue;
            };
            let current_door_area = current_door_shape.area();
            if current_door_area > max_door_area {
                max_door_area = current_door_area;
                ignore_shape = Some(current_door_shape);
                ignore_object = Some(TreeObject::Room(other_room));
            }
        }

        // :543-547.
        let (layer, contained_shape) = {
            let r = rooms
                .incomplete_room(incomplete_id)
                .expect("the from room is in the arena");
            (r.get_layer(), r.get_contained_shape().cloned())
        };
        // A **local** room: Java does not hand it to `addIncompleteExpansionRoom`.
        let enlarged_room = IncompleteFreeSpaceExpansionRoom::new(
            enlarged_box.map(TileShape::Box),
            layer,
            contained_shape,
        );
        // :548-549.
        let new_rooms = {
            let ctx = board.ctx();
            tree_of(board, tree_id).complete_shape(
                &enlarged_room,
                net_number,
                ignore_object,
                ignore_shape.as_ref(),
                &board.items,
                &*rooms,
                &ctx,
            )
        };
        // :562-564.
        if new_rooms.len() != 1 {
            return false;
        }
        // :565. "Check, that the area increases to prevent endless loop."
        let new_shape = new_rooms[0].get_shape().unwrap_or_else(|| {
            panic!(
                "SortedOrthogonalRoomNeighbours.tryRemoveEdge: completeShape answered a room with \
                 no shape (SortedOrthogonalRoomNeighbours.java:565) — Java NPEs here too"
            )
        });
        if new_shape.area() <= room_area {
            return false;
        }
        // :578-580.
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

    /// Port of the private `insertIncompleteRoom`
    /// (SortedOrthogonalRoomNeighbours.java:403-420).
    ///
    /// Java computes the door dimension from `newIncompleteRoomShape.intersection(this.roomShape)`
    /// (`:409`) after having already computed `this.roomShape.intersection(newIncompleteRoomShape)`
    /// (`:407`) — the same box, computed twice; transcribed as written.
    fn insert_incomplete_room(
        &self,
        board: &mut Board,
        rooms: &mut ExpansionRoomStore,
        ll_x: i32,
        ll_y: i32,
        ur_x: i32,
        ur_y: i32,
    ) {
        // :405-406.
        let new_incomplete_room_shape = IntBox::from_coords(ll_x, ll_y, ur_x, ur_y);
        if new_incomplete_room_shape.dimension() != 2 {
            return;
        }
        // :407-408.
        let new_contained_shape = self.room_shape.intersection(&new_incomplete_room_shape);
        if new_contained_shape.is_empty() {
            return;
        }
        // :409-410.
        let door_dimension = new_incomplete_room_shape
            .intersection(&self.room_shape)
            .dimension();
        if door_dimension <= 0 {
            return;
        }
        // :411-417.
        let layer = rooms
            .room_layer(board, self.from_room)
            .expect("the from room is in the arena");
        let new_room = RoomRef::Incomplete(rooms.new_incomplete_room(
            Some(TileShape::Box(new_incomplete_room_shape)),
            layer,
            Some(TileShape::Box(new_contained_shape)),
        ));
        let new_door = rooms.new_door(self.completed_room, new_room, door_dimension);
        rooms.add_door(self.completed_room, new_door);
        rooms.add_door(new_room, new_door);
    }

    /// Port of the private `calculateNewIncompleteRooms`
    /// (SortedOrthogonalRoomNeighbours.java:224-401).
    pub fn calculate_new_incomplete_rooms(
        &self,
        board: &mut Board,
        rooms: &mut ExpansionRoomStore,
    ) {
        // :225.
        let board_bounds = board.bounding_box;
        let neighbours: Vec<&SortedRoomNeighbour> = self.sorted_neighbours.iter().collect();
        // :226. `SortedSet.getLast()` throws on an empty set; the one caller guards it (`:69`).
        let Some(&last) = neighbours.last() else {
            panic!(
                "SortedOrthogonalRoomNeighbours.calculateNewIncompleteRooms: the neighbour set is \
                 empty (SortedOrthogonalRoomNeighbours.java:226) — Java throws \
                 NoSuchElementException here too"
            )
        };
        let mut prev_neighbour = last;

        // :228-400.
        for next_neighbour in neighbours.iter().copied() {
            if !next_neighbour
                .intersection
                .intersects(&prev_neighbour.intersection)
            {
                // "create a door to a new incomplete expansion room between the last corner of
                // the previous neighbour and the first corner of the current neighbour."
                let prev = &prev_neighbour.intersection;
                let next = &next_neighbour.intersection;
                match next_neighbour.first_touching_side {
                    // :234-273.
                    0 => {
                        if prev_neighbour.last_touching_side == 0 {
                            if prev.ur.x < next.ll.x {
                                self.insert_incomplete_room(
                                    board,
                                    rooms,
                                    prev.ur.x,
                                    board_bounds.ll.y,
                                    next.ll.x,
                                    self.room_shape.ll.y,
                                );
                            }
                        } else if prev.ll.y > self.room_shape.ll.y
                            || next.ll.x > self.room_shape.ll.x
                        {
                            if self.is_obstacle_expansion_room {
                                // "no 2-dim doors between obstacle_expansion_rooms and free space
                                // rooms allowed."
                                if prev_neighbour.last_touching_side == 3 {
                                    self.insert_incomplete_room(
                                        board,
                                        rooms,
                                        board_bounds.ll.x,
                                        self.room_shape.ll.y,
                                        self.room_shape.ll.x,
                                        prev.ll.y,
                                    );
                                }
                                self.insert_incomplete_room(
                                    board,
                                    rooms,
                                    self.room_shape.ll.x,
                                    board_bounds.ll.y,
                                    next.ll.x,
                                    self.room_shape.ll.y,
                                );
                            } else {
                                self.insert_incomplete_room(
                                    board,
                                    rooms,
                                    board_bounds.ll.x,
                                    board_bounds.ll.y,
                                    next.ll.x,
                                    prev.ll.y,
                                );
                            }
                        }
                    }
                    // :274-313.
                    1 => {
                        if prev_neighbour.last_touching_side == 1 {
                            if prev.ur.y < next.ll.y {
                                self.insert_incomplete_room(
                                    board,
                                    rooms,
                                    self.room_shape.ur.x,
                                    prev.ur.y,
                                    board_bounds.ur.x,
                                    next.ll.y,
                                );
                            }
                        } else if prev.ur.x < self.room_shape.ur.x
                            || next.ll.y > self.room_shape.ll.y
                        {
                            if self.is_obstacle_expansion_room {
                                if prev_neighbour.last_touching_side == 0 {
                                    self.insert_incomplete_room(
                                        board,
                                        rooms,
                                        prev.ur.x,
                                        board_bounds.ll.y,
                                        self.room_shape.ur.x,
                                        self.room_shape.ll.y,
                                    );
                                }
                                self.insert_incomplete_room(
                                    board,
                                    rooms,
                                    self.room_shape.ur.x,
                                    self.room_shape.ll.y,
                                    self.room_shape.ur.x,
                                    next.ll.y,
                                );
                            } else {
                                self.insert_incomplete_room(
                                    board,
                                    rooms,
                                    prev.ur.x,
                                    board_bounds.ll.y,
                                    board_bounds.ur.x,
                                    next.ll.y,
                                );
                            }
                        }
                    }
                    // :314-353.
                    2 => {
                        if prev_neighbour.last_touching_side == 2 {
                            if prev.ll.x > next.ur.x {
                                self.insert_incomplete_room(
                                    board,
                                    rooms,
                                    next.ur.x,
                                    self.room_shape.ur.y,
                                    prev.ll.x,
                                    board_bounds.ur.y,
                                );
                            }
                        } else if prev.ur.y < self.room_shape.ur.y
                            || next.ur.x < self.room_shape.ur.x
                        {
                            if self.is_obstacle_expansion_room {
                                if prev_neighbour.last_touching_side == 1 {
                                    self.insert_incomplete_room(
                                        board,
                                        rooms,
                                        self.room_shape.ur.x,
                                        prev.ur.y,
                                        board_bounds.ur.x,
                                        self.room_shape.ur.y,
                                    );
                                }
                                self.insert_incomplete_room(
                                    board,
                                    rooms,
                                    next.ur.x,
                                    self.room_shape.ur.y,
                                    self.room_shape.ur.x,
                                    board_bounds.ur.y,
                                );
                            } else {
                                self.insert_incomplete_room(
                                    board,
                                    rooms,
                                    next.ur.x,
                                    prev.ur.y,
                                    board_bounds.ur.x,
                                    board_bounds.ur.y,
                                );
                            }
                        }
                    }
                    // :354-393.
                    3 => {
                        if prev_neighbour.last_touching_side == 3 {
                            if prev.ll.y > next.ur.y {
                                self.insert_incomplete_room(
                                    board,
                                    rooms,
                                    board_bounds.ll.x,
                                    next.ur.y,
                                    self.room_shape.ll.x,
                                    prev.ll.y,
                                );
                            }
                        } else if next.ur.y < self.room_shape.ur.y
                            || prev.ll.x > self.room_shape.ll.x
                        {
                            if self.is_obstacle_expansion_room {
                                if prev_neighbour.last_touching_side == 2 {
                                    self.insert_incomplete_room(
                                        board,
                                        rooms,
                                        self.room_shape.ll.x,
                                        self.room_shape.ur.y,
                                        prev.ll.x,
                                        board_bounds.ur.y,
                                    );
                                }
                                self.insert_incomplete_room(
                                    board,
                                    rooms,
                                    board_bounds.ll.x,
                                    next.ur.y,
                                    self.room_shape.ll.x,
                                    self.room_shape.ur.y,
                                );
                            } else {
                                self.insert_incomplete_room(
                                    board,
                                    rooms,
                                    board_bounds.ll.x,
                                    next.ur.y,
                                    prev.ll.x,
                                    board_bounds.ur.y,
                                );
                            }
                        }
                    }
                    // :394-396: `FRLogger.warn` (dropped) and nothing else. Reached with the `-1`
                    // the inner class's `else` branch (`:651-652`) assigns.
                    _ => {}
                }
            }
            // :399.
            prev_neighbour = next_neighbour;
        }
    }
}

// =================================================================================================
// The static helpers
// =================================================================================================

/// Port of the private static `calculateIncompleteRoomsWithEmptyNeighbours`
/// (SortedOrthogonalRoomNeighbours.java:79-108): four incomplete rooms, one per side of the
/// board's bounding box, for an obstacle room with no touching neighbour at all.
///
/// Note what this does **not** do: no `insertDoorOk`, no dimension test, no `isEmpty` guard — a
/// door of dimension 1 is created for every one of the four, even when the new box is degenerate.
/// The base class's namesake (SortedRoomNeighbours.java:138-156) tests each.
fn calculate_incomplete_rooms_with_empty_neighbours(
    room: RoomRef,
    board: &mut Board,
    rooms: &mut ExpansionRoomStore,
) {
    // :81-87: `FRLogger.warn` (dropped) and `return`.
    let Some(TileShape::Box(room_box)) = rooms.room_shape(room).cloned() else {
        return;
    };
    // :88.
    let bounding_box = board.get_bounding_box();
    let layer = rooms
        .room_layer(board, room)
        .expect("the obstacle room is in the arena");
    // :89-107.
    for i in 0..4 {
        let new_room_box = match i {
            0 => IntBox::from_coords(
                bounding_box.ll.x,
                bounding_box.ll.y,
                bounding_box.ur.x,
                room_box.ll.y,
            ),
            1 => IntBox::from_coords(
                room_box.ur.x,
                bounding_box.ll.y,
                bounding_box.ur.x,
                bounding_box.ur.y,
            ),
            2 => IntBox::from_coords(
                bounding_box.ll.x,
                room_box.ur.y,
                bounding_box.ur.x,
                bounding_box.ur.y,
            ),
            // i == 3.
            _ => IntBox::from_coords(
                bounding_box.ll.x,
                bounding_box.ll.y,
                room_box.ll.x,
                bounding_box.ur.y,
            ),
        };
        let new_contained_box = room_box.intersection(&new_room_box);
        let new_room = RoomRef::Incomplete(rooms.new_incomplete_room(
            Some(TileShape::Box(new_room_box)),
            layer,
            Some(TileShape::Box(new_contained_box)),
        ));
        let new_door = rooms.new_door(room, new_room, 1);
        rooms.add_door(room, new_door);
        rooms.add_door(new_room, new_door);
    }
}

/// Port of the private static `removeBorderLine`
/// (SortedOrthogonalRoomNeighbours.java:211-222). `None` is Java's `null` from the `default` arm
/// (`:217-220`, `FRLogger.warn` dropped), which `:545` then hands to the
/// `IncompleteFreeSpaceExpansionRoom` constructor as a null shape.
fn remove_border_line(room_box: &IntBox, remove_edge_no: i32) -> Option<IntBox> {
    match remove_edge_no {
        0 => Some(IntBox::from_coords(
            room_box.ll.x,
            -CRIT_INT,
            room_box.ur.x,
            room_box.ur.y,
        )),
        1 => Some(IntBox::from_coords(
            room_box.ll.x,
            room_box.ll.y,
            CRIT_INT,
            room_box.ur.y,
        )),
        2 => Some(IntBox::from_coords(
            room_box.ll.x,
            room_box.ll.y,
            room_box.ur.x,
            CRIT_INT,
        )),
        3 => Some(IntBox::from_coords(
            -CRIT_INT,
            room_box.ll.y,
            room_box.ur.x,
            room_box.ur.y,
        )),
        _ => None,
    }
}

// =================================================================================================
// The inner class
// =================================================================================================

/// Port of the private inner class `SortedOrthogonalRoomNeighbours.SortedRoomNeighbour`
/// (SortedOrthogonalRoomNeighbours.java:598-727): "helper class to sort the doors of an expansion
/// room counterclockwise around the border of the room shape."
///
/// Its own class, not [`super::sorted_neighbours::SortedRoomNeighbour`] and not the 45-degree
/// one: [`IntBox`] shapes, four sides, and no corner flags or memoized corners.
#[derive(Debug, Clone)]
pub struct SortedRoomNeighbour {
    /// `searchTreeObject` (:601): "the search tree object of the neighbour room."
    pub search_tree_object: TreeObject,
    /// `searchTreeObject.getId()`, resolved once at construction — quirk #161's two id spaces.
    pub object_id: i32,
    /// `shape` (:604): "the shape of the neighbour room."
    pub shape: IntBox,
    /// `intersection` (:607): "the intersection of this ExpansionRoom shape with the
    /// neighbourShape."
    pub intersection: IntBox,
    /// `firstTouchingSide` (:610): "the first side of the room shape, where the neighbourShape
    /// touches." `-1` for the "case not expected" branch (`:650-653`), which — unlike the
    /// 45-degree class — does **not** stop the neighbour being added to the set.
    pub first_touching_side: i32,
    /// `lastTouchingSide` (:613): "the last side of the room shape, where the neighbourShape
    /// touches." `-1` likewise (`:663-666`).
    pub last_touching_side: i32,
}

impl SortedRoomNeighbour {
    /// Port of the constructor (SortedOrthogonalRoomNeighbours.java:615-667).
    ///
    // renamed: the inner class's constructor `SortedRoomNeighbour` is `SortedRoomNeighbour::new`.
    ///
    /// `room_shape` and `edge_interior_touches_obstacle` are the outer-instance state Java's
    /// inner class reaches through `this$0`; the second is **written** here (`:621-640`), and —
    /// unlike the 45-degree class's loop — unconditionally, before the touching sides are known.
    pub fn new(
        search_tree_object: TreeObject,
        object_id: i32,
        neighbour_shape: IntBox,
        intersection: IntBox,
        room_shape: &IntBox,
        edge_interior_touches_obstacle: &mut [bool; 4],
    ) -> SortedRoomNeighbour {
        // :621-640. Four independent tests, all evaluated.
        if intersection.ll.y == room_shape.ll.y
            && intersection.ur.x > room_shape.ll.x
            && intersection.ll.x < room_shape.ur.x
        {
            edge_interior_touches_obstacle[0] = true;
        }
        if intersection.ur.x == room_shape.ur.x
            && intersection.ur.y > room_shape.ll.y
            && intersection.ll.y < room_shape.ur.y
        {
            edge_interior_touches_obstacle[1] = true;
        }
        if intersection.ur.y == room_shape.ur.y
            && intersection.ur.x > room_shape.ll.x
            && intersection.ll.x < room_shape.ur.x
        {
            edge_interior_touches_obstacle[2] = true;
        }
        if intersection.ll.x == room_shape.ll.x
            && intersection.ur.y > room_shape.ll.y
            && intersection.ll.y < room_shape.ur.y
        {
            edge_interior_touches_obstacle[3] = true;
        }

        // :642-653.
        let first_touching_side =
            if intersection.ll.y == room_shape.ll.y && intersection.ll.x > room_shape.ll.x {
                0
            } else if intersection.ur.x == room_shape.ur.x && intersection.ll.y > room_shape.ll.y {
                1
            } else if intersection.ur.y == room_shape.ur.y {
                2
            } else if intersection.ll.x == room_shape.ll.x {
                3
            } else {
                // :650-652: `FRLogger.warn` (dropped) and `-1`.
                -1
            };

        // :655-666.
        let last_touching_side =
            if intersection.ll.x == room_shape.ll.x && intersection.ll.y > room_shape.ll.y {
                3
            } else if intersection.ur.y == room_shape.ur.y && intersection.ll.x > room_shape.ll.x {
                2
            } else if intersection.ur.x == room_shape.ur.x {
                1
            } else if intersection.ll.y == room_shape.ll.y {
                0
            } else {
                // :663-665.
                -1
            };

        SortedRoomNeighbour {
            search_tree_object,
            object_id,
            shape: neighbour_shape,
            intersection,
            first_touching_side,
            last_touching_side,
        }
    }

    /// Port of `compareTo(SortedRoomNeighbour)`
    /// (SortedOrthogonalRoomNeighbours.java:673-726): "compare function for sorting the
    /// neighbours in counterclock sense around the border of the room shape in ascending order."
    pub fn compare_to(&self, other: &SortedRoomNeighbour) -> Ordering {
        // :675-680.
        if self.first_touching_side > other.first_touching_side {
            return Ordering::Greater;
        }
        if self.first_touching_side < other.first_touching_side {
            return Ordering::Less;
        }

        // :682-696. "now the first touch of this and other is at the same side"
        let is1 = &self.intersection;
        let is2 = &other.intersection;
        let mut cmp_value: i32 = match self.first_touching_side {
            0 => is1.ll.x.wrapping_sub(is2.ll.x),
            1 => is1.ll.y.wrapping_sub(is2.ll.y),
            2 => is2.ur.x.wrapping_sub(is1.ur.x),
            3 => is2.ur.y.wrapping_sub(is1.ur.y),
            // :692-695: `FRLogger.warn` (dropped) and `return 0` — the `-1` case.
            _ => return Ordering::Equal,
        };

        if cmp_value == 0 {
            // :697-720. "The first touching points of this neighbour and other with the room
            // shape are equal. Compare the last touching points."
            let this_touching_side_diff =
                (self.last_touching_side - self.first_touching_side + 4).rem_euclid(4);
            let other_touching_side_diff =
                (other.last_touching_side - other.first_touching_side + 4).rem_euclid(4);
            if this_touching_side_diff > other_touching_side_diff {
                return Ordering::Greater;
            }
            if this_touching_side_diff < other_touching_side_diff {
                return Ordering::Less;
            }

            // :709-719. "now the last touch of this and other is at the same side"
            cmp_value = match self.last_touching_side {
                0 => is1.ur.x.wrapping_sub(is2.ur.x),
                1 => is1.ur.y.wrapping_sub(is2.ur.y),
                2 => is2.ll.x.wrapping_sub(is1.ll.x),
                3 => is2.ll.y.wrapping_sub(is1.ll.y),
                // :715-718: `FRLogger.warn` (dropped) and `return 0`.
                _ => return Ordering::Equal,
            };
        }
        if cmp_value == 0 {
            // :721-724. "Deterministic tie-breaker for identical geometry" (quirk #161).
            cmp_value = self.object_id.wrapping_sub(other.object_id);
        }
        cmp_value.cmp(&0)
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
