//! Port of `autoroute.expansion.Sorted45DegreeRoomNeighbours`
//! (Sorted45DegreeRoomNeighbours.java:22-982) — the neighbour sorter
//! [`select_calculation_mode`](super::sorted_neighbours::select_calculation_mode) picks on a
//! 45-degree autoroute tree.
//!
//! It is **not** a specialisation of the any-angle base class: it declares its **own** inner
//! `SortedRoomNeighbour` (`:803-981`) with its own fields, its own constructor and its own
//! `compareTo`, and its own door-splitting geometry. The only thing it borrows from
//! `SortedRoomNeighbours` is the package-private static `insertDoorOk` (`:163`), which the port
//! reuses from [`super::sorted_neighbours::insert_door_ok`].
//!
//! # What is different from the base class, and matters
//!
//! * Every shape is an [`IntOctagon`]: `roomShape` is `completedRoom.getShape().boundingOctagon()`
//!   (`:35`), and every neighbour's shape and intersection are bounding octagons too (`:129-130`).
//!   Side numbers are therefore always `0..8` and the base class's `-1`-bearing
//!   `containsOnBorderLineNo` path does not exist.
//! * A **2-dimensional overlap is only skipped for an obstacle room** (`:132`). Where the base
//!   class `continue`s for every `dimension > 1` (SortedRoomNeighbours.java:232-247), this class
//!   falls through and records a 2-dimensional intersection as an ordinary sorted neighbour when
//!   the completed room is a `CompleteFreeSpaceExpansionRoom`.
//! * `calculateTargetDoors` is `CompleteFreeSpaceExpansionRoom`'s per-entry method (`:124`,
//!   [`super::complete_room::calculate_target_doors`]), called **inside** the neighbour loop, not
//!   the base class's static one at the end of `calculate`. The two are not the same function:
//!   this one calls `setNetDependent()` unconditionally, once per own-net entry.
//! * `tryRemoveEdgeLine` (`:316-431`) removes **every** untouched border line at once and hands
//!   `completeShape` the largest 2-dimensional door to a free-space room as the object to ignore;
//!   the base class removes one line and ignores nothing.
//!
//! # Hazard F for this class — the comparator is a total order, unlike the base's
//!
//! `SortedRoomNeighbour.compareTo` (`:922-980`) compares `firstTouchingSide`, then one integer
//! ordinate of the intersection's first corner, then the touching-side span, then one ordinate of
//! the last corner, then the object id. Every key is a Java `int` and every refinement is reached
//! on the **same** condition for both operands (`cmpValue == 0`), so unlike
//! `SortedRoomNeighbours.SortedRoomNeighbour.compareTo` (quirk #160) this one *is* transitive and
//! antisymmetric: it is a lexicographic order on `(firstTouchingSide, corner ordinate, span, last
//! corner ordinate, id)`. Two neighbours still compare `Equal` when all five keys agree, and the
//! `TreeSet` then **drops** the second — the id tie-break crosses the item and room id spaces
//! exactly as the base class's does (quirk #161). The container is [`JavaTreeSet`] anyway, both
//! because the drop has to happen at the same moment Java's does and because a `BTreeSet` would
//! order the survivors by a different walk if the analysis above were ever wrong.
//!
//! not ported: every `FRLogger` payload of this class — the `FRLogger.warn`s at `:94`, `:258`,
//! `:288`, `:321` and `:789` and the five `ROOM_EDGE_REMOVE` `FRLogger.trace` blocks
//! (`:343-350`, `:354-361`, `:362-371`, `:402-409`, `:414-423`), together with the private
//! `describeBounds` helper (`:433-435`) that only those traces call — per `global-constraints.md`.

use std::cmp::Ordering;

use fr_board::{Board, TreeId, TreeObject};
use fr_geometry::{CRIT_INT, IntOctagon, TileShape};

use crate::JavaTreeSet;
use crate::autoroute::expansion::complete_room::calculate_target_doors;
use crate::autoroute::expansion::sorted_neighbours::{
    create_overlap_door, insert_door_ok, object_id, object_is_trace_obstacle, object_tree_shape,
    tree_of,
};
use crate::autoroute::expansion::{ExpansionRoomStore, IncompleteFreeSpaceExpansionRoom, RoomRef};
use crate::autoroute::item_info;
use crate::autoroute::tree_ext::AutorouteSearchTreeExt;

/// Port of `Sorted45DegreeRoomNeighbours` (Sorted45DegreeRoomNeighbours.java:22-982).
#[derive(Debug, Clone)]
pub struct Sorted45DegreeRoomNeighbours {
    /// `completedRoom` (:24).
    pub completed_room: RoomRef,
    /// `sortedNeighbours` (:25), a `TreeSet` (:36).
    pub sorted_neighbours: JavaTreeSet<SortedRoomNeighbour>,
    /// `fromRoom` (:26).
    pub from_room: RoomRef,
    /// `roomShape` (:27) — `completedRoom.getShape().boundingOctagon()` **captured in the
    /// constructor** (`:35`), so a later `setShape` on the completed room does not reach it.
    pub room_shape: IntOctagon,
    /// `edgeInteriorTouchesObstacle` (:28), eight flags, all false (`:38-41`). Written by the
    /// inner class's constructor (`:896-911`) and read by `tryRemoveEdgeLine` (`:330`).
    pub edge_interior_touches_obstacle: [bool; 8],
}

impl Sorted45DegreeRoomNeighbours {
    /// Port of the private constructor (Sorted45DegreeRoomNeighbours.java:31-42).
    fn new(
        from_room: RoomRef,
        completed_room: RoomRef,
        room_shape: IntOctagon,
    ) -> Sorted45DegreeRoomNeighbours {
        Sorted45DegreeRoomNeighbours {
            completed_room,
            sorted_neighbours: JavaTreeSet::new(),
            from_room,
            room_shape,
            edge_interior_touches_obstacle: [false; 8],
        }
    }

    /// Port of the public static `Sorted45DegreeRoomNeighbours.calculate(ExpansionRoom,
    /// AutorouteEngine)` (Sorted45DegreeRoomNeighbours.java:45-79): "calculates room neighbours."
    ///
    /// **Java wins over the brief's name and signature.** The brief calls this `complete_45(room,
    /// engine, board, net_no, ignore_net)`; the Java method is `calculate(room, autorouteEngine)`,
    /// there is no `complete` in this class at all, and no `ignoreNet` anywhere in it. The port
    /// takes the room plus the same five services the base class's module docs tabulate.
    ///
    /// Java's tail call at `:65` (after an edge line was removed) is the loop below; it takes a
    /// **fresh** room id from the counter each time round, exactly as the base class does.
    pub fn calculate(
        room: RoomRef,
        net_number: i32,
        board: &mut Board,
        rooms: &mut ExpansionRoomStore,
        tree_id: TreeId,
    ) -> Option<RoomRef> {
        loop {
            // :47-53. `generateRoomIdNo()` is an *argument*, so the counter ticks before the
            // method knows whether a complete room will be built at all.
            let room_id_no = rooms.next_room_id_no();
            let room_neighbours = Sorted45DegreeRoomNeighbours::calculate_neighbours(
                room, net_number, board, rooms, tree_id, room_id_no,
            )?;

            // :58-66. "Check, that each side of the room shape has at least one touching
            // neighbour. Otherwise, improve the room shape by enlarging."
            let edge_removed =
                room_neighbours.try_remove_edge_line(net_number, board, rooms, tree_id);
            let result = room_neighbours.completed_room;
            if edge_removed {
                rooms.remove_all_doors(result);
                continue;
            }

            // :68-77. "Now calculate the new incomplete rooms together with the doors between
            // this room and the sorted neighbours."
            if room_neighbours.sorted_neighbours.is_empty() {
                if let RoomRef::Obstacle(_) = result {
                    room_neighbours.calculate_edge_incomplete_rooms_of_obstacle_expansion_room(
                        0, 7, board, rooms,
                    );
                }
            } else {
                room_neighbours.calculate_new_incomplete_rooms(board, rooms);
            }
            // :78. Unlike the base class (SortedRoomNeighbours.java:132-134) there is no
            // `calculateTargetDoors` here: this class builds the target doors inside
            // `calculateNeighbours` instead (`:124`).
            return Some(result);
        }
    }

    /// Port of the private static `Sorted45DegreeRoomNeighbours.calculateNeighbours`
    /// (Sorted45DegreeRoomNeighbours.java:85-172): "calculates all touching neighbours of room
    /// and sorts them in counterclock sense around the boundary of the room shape."
    ///
    /// `pub` where Java's is `private`, because it is the unit `p6t3` mode 6 drives directly —
    /// the sorted set is unobservable from outside otherwise.
    pub fn calculate_neighbours(
        room: RoomRef,
        net_number: i32,
        board: &mut Board,
        rooms: &mut ExpansionRoomStore,
        tree_id: TreeId,
        room_id_no: i32,
    ) -> Option<Sorted45DegreeRoomNeighbours> {
        // :87. Java would NPE below on a room with no shape; so does this.
        let room_shape = rooms
            .room_shape(room)
            .unwrap_or_else(|| {
                panic!(
                    "Sorted45DegreeRoomNeighbours.calculateNeighbours: room {room:?} has no shape \
                     (Sorted45DegreeRoomNeighbours.java:87) — Java NPEs here too"
                )
            })
            .clone();
        let layer = rooms.room_layer(board, room).unwrap_or_else(|| {
            panic!(
                "Sorted45DegreeRoomNeighbours.calculateNeighbours: room {room:?} has no layer \
                 (Sorted45DegreeRoomNeighbours.java:90) — Java NPEs here too"
            )
        });

        // :88-97.
        let completed_room = match room {
            RoomRef::Incomplete(_) => RoomRef::Complete(rooms.new_complete_room(
                Some(room_shape.clone()),
                layer,
                room_id_no,
            )),
            RoomRef::Obstacle(id) => RoomRef::Obstacle(id),
            // :93-96: `FRLogger.warn` (dropped) and `return null`.
            RoomRef::Complete(_) => return None,
        };

        // :98. The bounding octagon of the **input** room's shape; `:35` takes the completed
        // room's, which is the same shape object in both branches above.
        let room_oct = bounding_octagon_of(&room_shape, 98);
        let completed_shape = rooms
            .room_shape(completed_room)
            .expect("the completed room was just built from a shape")
            .clone();
        // :99.
        let mut result = Sorted45DegreeRoomNeighbours::new(
            room,
            completed_room,
            bounding_octagon_of(&completed_shape, 35),
        );

        // :100-101.
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

        // :103-113. "Sort the overlapping objects deterministically to ensure parity with v1.9."
        // `List.sort` is a stable TimSort, and so is `slice::sort_by`.
        overlapping_objects.sort_by(|e1, e2| {
            let id_diff = object_id(e1.object, rooms).wrapping_sub(object_id(e2.object, rooms));
            if id_diff != 0 {
                return id_diff.cmp(&0);
            }
            e1.shape_index.cmp(&e2.shape_index)
        });

        // :115-170. "Calculate the touching neighbour objects and sort them in counterclock sense
        // around the border of the room shape."
        for current_entry in overlapping_objects {
            let current_object = current_entry.object;
            // :119-121, the same dead reference test the base class carries
            // (SortedRoomNeighbours.java:219-221): `:93-96` has already returned `null` for a
            // `CompleteFreeSpaceExpansionRoom`, which is the only room in the tree.
            if rooms
                .get_object(room)
                .is_some_and(|object| room.is_free_space() && object == current_object)
            {
                continue;
            }

            // :122-126. The **per-entry** `CompleteFreeSpaceExpansionRoom.calculateTargetDoors`,
            // not the base class's static one: it calls `setNetDependent()` unconditionally.
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

            // :127-131.
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
            let current_oct = bounding_octagon_of(&current_shape, 129);
            let intersection = room_oct.intersection(&current_oct);
            let dimension = intersection.dimension();

            // :132-143. "only Obstacle expansion room may have a 2-dim overlap" — and note the
            // `&&`: unlike the base class (SortedRoomNeighbours.java:232), a 2-dimensional
            // overlap with a **free-space** completed room is *not* skipped here, it falls
            // through to `addSortedNeighbour` below.
            if dimension > 1
                && let RoomRef::Obstacle(obstacle_room) = completed_room
            {
                if let TreeObject::Item(item_id) = current_object
                    && board
                        .get_item(item_id)
                        .is_some_and(|item| item.is_routable())
                {
                    // :136-138. `getAutorouteInfo()` creates the scratch on demand.
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
                    // :139 hands the result straight to `createOverlapDoor`, which dereferences
                    // `other.item` at ObstacleExpansionRoom.java:81 — so a `null` from the
                    // out-of-range branch of `getExpansionRoom` is a `NullPointerException`.
                    let overlap_room = overlap_room.unwrap_or_else(|| {
                        panic!(
                            "Sorted45DegreeRoomNeighbours.calculateNeighbours: item {item_id} has \
                             no expansion room for shape {} \
                             (Sorted45DegreeRoomNeighbours.java:139) — Java NPEs in \
                             createOverlapDoor here too",
                            current_entry.shape_index
                        )
                    });
                    create_overlap_door(obstacle_room, overlap_room, board, rooms);
                }
                continue;
            }
            // :144-147. "may happen at a corner from 2 diagonal lines with non integer
            // coordinates (--.5, ---.5)."
            if dimension < 0 {
                continue;
            }

            // :148.
            result.add_sorted_neighbour(current_object, rooms, current_oct, intersection);

            // :149-169. "make sure, that there is a door to the neighbour room."
            if dimension > 0 {
                let neighbour_room: Option<RoomRef> = match current_object {
                    // :152-153: a `CompleteFreeSpaceExpansionRoom` *is* an `ExpansionRoom`.
                    TreeObject::Room(id) => Some(RoomRef::Complete(id)),
                    // :154-161. "expand the item for ripup and pushing purposes"
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
                // :162-168. `SortedRoomNeighbours.insertDoorOk`, the package-private static this
                // class shares with the base.
                if let Some(neighbour_room) = neighbour_room
                    && insert_door_ok(
                        completed_room,
                        neighbour_room,
                        &TileShape::Octagon(intersection),
                        board,
                        rooms,
                    )
                {
                    let new_door = rooms.new_door(completed_room, neighbour_room, 1);
                    rooms.add_door(neighbour_room, new_door);
                    rooms.add_door(completed_room, new_door);
                }
            }
        }
        // :171.
        Some(result)
    }

    /// Port of the private `addSortedNeighbour` (Sorted45DegreeRoomNeighbours.java:245-252).
    ///
    /// The `lastTouchingSide >= 0` guard (`:249`) is this class's own: the base class adds every
    /// neighbour, and the orthogonal class does too.
    fn add_sorted_neighbour(
        &mut self,
        search_tree_object: TreeObject,
        rooms: &ExpansionRoomStore,
        neighbour_shape: IntOctagon,
        intersection: IntOctagon,
    ) {
        let new_neighbour = SortedRoomNeighbour::new(
            search_tree_object,
            object_id(search_tree_object, rooms),
            neighbour_shape,
            intersection,
            &self.room_shape,
            &mut self.edge_interior_touches_obstacle,
        );
        if new_neighbour.last_touching_side >= 0 {
            self.sorted_neighbours.add(new_neighbour);
        }
    }

    /// Port of the private `calculateEdgeIncompleteRoomsOfObstacleExpansionRoom`
    /// (Sorted45DegreeRoomNeighbours.java:255-310): "calculates an incomplete room for each edge
    /// side from fromSideIndex to toSideIndex."
    ///
    // Java bug: Sorted45DegreeRoomNeighbours.calculateEdgeIncompleteRoomsOfObstacleExpansionRoom
    // never advances `currentCorner` (quirk #163). `:264` sets it to
    // `roomShape.corner(fromSideIndex)` and the loop body (`:266-309`) never reassigns it, so the
    // `!currentCorner.equals(nextCorner)` guard at `:269` — plainly meant to skip a *degenerate*
    // side, whose two corners coincide — instead compares every side's end corner with the
    // **start** corner of the walk. On an octagon with a degenerate side the room for that side
    // is built anyway, and the side whose end corner happens to equal `corner(fromSideIndex)`
    // (normally the last one of a full 0..7 walk) is skipped although it is not degenerate.
    // Reproduced.
    fn calculate_edge_incomplete_rooms_of_obstacle_expansion_room(
        &self,
        from_side_index: usize,
        to_side_index: usize,
        board: &mut Board,
        rooms: &mut ExpansionRoomStore,
    ) {
        // :257-262: `FRLogger.warn` (dropped) and `return`.
        if !matches!(self.from_room, RoomRef::Obstacle(_)) {
            return;
        }
        // :263.
        let board_bounding_oct = board.get_bounding_box().bounding_octagon();
        // :264-265.
        let current_corner = self.room_shape.corner(from_side_index);
        let mut current_side_index = from_side_index;
        // :266-309.
        loop {
            let next_side_no = (current_side_index + 1) % 8;
            let next_corner = self.room_shape.corner(next_side_no);
            if current_corner != next_corner {
                // :270-277.
                let mut left_x = board_bounding_oct.left_x;
                let mut bottom_y = board_bounding_oct.bottom_y;
                let mut right_x = board_bounding_oct.right_x;
                let mut top_y = board_bounding_oct.top_y;
                let mut upper_left_diagonal_x = board_bounding_oct.upper_left_diagonal_x;
                let mut lower_right_diagonal_x = board_bounding_oct.lower_right_diagonal_x;
                let mut lower_left_diagonal_x = board_bounding_oct.lower_left_diagonal_x;
                let mut upper_right_diagonal_x = board_bounding_oct.upper_right_diagonal_x;
                // :278-293. The `default` arm is unreachable: `currentSideIndex` is always taken
                // mod 8.
                match current_side_index {
                    0 => top_y = self.room_shape.bottom_y,
                    1 => upper_left_diagonal_x = self.room_shape.lower_right_diagonal_x,
                    2 => left_x = self.room_shape.right_x,
                    3 => lower_left_diagonal_x = self.room_shape.upper_right_diagonal_x,
                    4 => bottom_y = self.room_shape.top_y,
                    5 => lower_right_diagonal_x = self.room_shape.upper_left_diagonal_x,
                    6 => right_x = self.room_shape.left_x,
                    _ => upper_right_diagonal_x = self.room_shape.lower_left_diagonal_x,
                }
                // :294-303.
                self.insert_incomplete_room(
                    board,
                    rooms,
                    left_x,
                    bottom_y,
                    right_x,
                    top_y,
                    upper_left_diagonal_x,
                    lower_right_diagonal_x,
                    lower_left_diagonal_x,
                    upper_right_diagonal_x,
                );
            }
            // :305-308.
            if current_side_index == to_side_index {
                break;
            }
            current_side_index = next_side_no;
        }
    }

    /// Port of the private `tryRemoveEdgeLine` (Sorted45DegreeRoomNeighbours.java:316-431):
    /// "check, that each side of the room shape has at least one touching neighbour. Otherwise,
    /// the room shape will be improved the by enlarging. Returns true, if the room shape was
    /// changed."
    fn try_remove_edge_line(
        &self,
        net_number: i32,
        board: &mut Board,
        rooms: &mut ExpansionRoomStore,
        tree_id: TreeId,
    ) -> bool {
        // :317-319.
        let RoomRef::Incomplete(incomplete_id) = self.from_room else {
            return false;
        };
        // :320-325: `FRLogger.warn` (dropped) and `return false` when the shape is not an
        // octagon — which a box-shaped seed room on a 45-degree tree is not.
        let Some(TileShape::Octagon(room_oct)) = rooms.room_shape(self.from_room).cloned() else {
            return false;
        };
        // :326.
        let room_area = room_oct.area();

        // :328-338.
        let mut try_remove_edge_lines = false;
        for i in 0..8 {
            if !self.edge_interior_touches_obstacle[i] {
                let prev_corner = self.room_shape.corner(i).to_float();
                // `IntOctagon.nextNo(i)` is `(i + 1) % borderLineCount`, and an octagon always
                // has eight border lines.
                let next_corner = self.room_shape.corner((i + 1) % 8).to_float();
                if prev_corner.distance_square(&next_corner) > 1.0 {
                    try_remove_edge_lines = true;
                    break;
                }
            }
        }

        if !try_remove_edge_lines {
            // :430.
            return false;
        }
        // :340-353. "Touching neighbour missing at the edge side with index removeEdgeNo. Remove
        // the edge line and restart the algorithm."
        let enlarged_oct =
            remove_not_touching_border_lines(&room_oct, &self.edge_interior_touches_obstacle);

        // :373-394. "insert the overlapping doors with CompleteFreeSpaceExpansionRooms for the
        // information in complete_shape about the objects to ignore." The **largest** such door
        // wins; ties keep the first, because the test is `>`.
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

        // :395-399.
        let (layer, contained_shape) = {
            let r = rooms
                .incomplete_room(incomplete_id)
                .expect("the from room is in the arena");
            (r.get_layer(), r.get_contained_shape().cloned())
        };
        // A **local** room: Java does not hand it to `addIncompleteExpansionRoom`, so it never
        // enters the engine's list and must not enter the arena either.
        let enlarged_room = IncompleteFreeSpaceExpansionRoom::new(
            Some(TileShape::Octagon(enlarged_oct)),
            layer,
            contained_shape,
        );
        // :400-401.
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
        // :410-412.
        if new_rooms.len() != 1 {
            return false;
        }
        // :413. "Check, that the area increases to prevent endless loop."
        let new_shape = new_rooms[0].get_shape().unwrap_or_else(|| {
            panic!(
                "Sorted45DegreeRoomNeighbours.tryRemoveEdgeLine: completeShape answered a room \
                 with no shape (Sorted45DegreeRoomNeighbours.java:413) — Java NPEs here too"
            )
        });
        if new_shape.area() <= room_area {
            return false;
        }
        // :424-426.
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
    /// (Sorted45DegreeRoomNeighbours.java:438-473): "inserts a new incomplete room with an
    /// octagon shape."
    #[allow(clippy::too_many_arguments)] // Java's eight-ordinate parameter list, ported verbatim.
    fn insert_incomplete_room(
        &self,
        board: &mut Board,
        rooms: &mut ExpansionRoomStore,
        left_x: i32,
        bottom_y: i32,
        right_x: i32,
        top_y: i32,
        upper_left_diagonal_x: i32,
        lower_right_diagonal_x: i32,
        lower_left_diagonal_x: i32,
        upper_right_diagonal_x: i32,
    ) {
        // :448-458.
        let new_incomplete_room_shape = IntOctagon::new(
            left_x,
            bottom_y,
            right_x,
            top_y,
            upper_left_diagonal_x,
            lower_right_diagonal_x,
            lower_left_diagonal_x,
            upper_right_diagonal_x,
        )
        .normalize();
        // :459-472.
        if new_incomplete_room_shape.dimension() != 2 {
            return;
        }
        let new_contained_shape = self.room_shape.intersection(&new_incomplete_room_shape);
        if new_contained_shape.is_empty() {
            return;
        }
        let door_dimension = new_contained_shape.dimension();
        if door_dimension <= 0 {
            return;
        }
        let layer = rooms
            .room_layer(board, self.from_room)
            .expect("the from room is in the arena");
        let new_room = RoomRef::Incomplete(rooms.new_incomplete_room(
            Some(TileShape::Octagon(new_incomplete_room_shape)),
            layer,
            Some(TileShape::Octagon(new_contained_shape)),
        ));
        let new_door = rooms.new_door(self.completed_room, new_room, door_dimension);
        rooms.add_door(self.completed_room, new_door);
        rooms.add_door(new_room, new_door);
    }

    /// Port of the private `calculateNewIncompleteRoomsForObstacleExpansionRoom`
    /// (Sorted45DegreeRoomNeighbours.java:475-609).
    fn calculate_new_incomplete_rooms_for_obstacle_expansion_room(
        &self,
        prev_neighbour: &SortedRoomNeighbour,
        next_neighbour: &SortedRoomNeighbour,
        prev_is_next: bool,
        board: &mut Board,
        rooms: &mut ExpansionRoomStore,
    ) {
        // :479-484. `prevNeighbour != nextNeighbour` is a *reference* comparison; the one caller
        // that passes the same object twice (`:616-617`) is the only place it is false.
        let from_side_index = prev_neighbour.last_touching_side;
        let to_side_index = next_neighbour.first_touching_side;
        if from_side_index == to_side_index && !prev_is_next {
            return;
        }
        // :485.
        let board_bounding_oct = board.bounding_box.bounding_octagon();

        // :487-541. "insert the new incomplete room from prevNeighbour to the next corner of the
        // room shape."
        let mut left_x = board_bounding_oct.left_x;
        let mut bottom_y = board_bounding_oct.bottom_y;
        let mut right_x = board_bounding_oct.right_x;
        let mut top_y = board_bounding_oct.top_y;
        let mut upper_left_diagonal_x = board_bounding_oct.upper_left_diagonal_x;
        let mut lower_right_diagonal_x = board_bounding_oct.lower_right_diagonal_x;
        let mut lower_left_diagonal_x = board_bounding_oct.lower_left_diagonal_x;
        let mut upper_right_diagonal_x = board_bounding_oct.upper_right_diagonal_x;
        let prev = &prev_neighbour.intersection;
        match from_side_index {
            0 => {
                top_y = self.room_shape.bottom_y;
                upper_left_diagonal_x = prev.lower_right_diagonal_x;
            }
            1 => {
                upper_left_diagonal_x = self.room_shape.lower_right_diagonal_x;
                left_x = prev.right_x;
            }
            2 => {
                left_x = self.room_shape.right_x;
                lower_left_diagonal_x = prev.upper_right_diagonal_x;
            }
            3 => {
                lower_left_diagonal_x = self.room_shape.upper_right_diagonal_x;
                bottom_y = prev.top_y;
            }
            4 => {
                bottom_y = self.room_shape.top_y;
                lower_right_diagonal_x = prev.upper_left_diagonal_x;
            }
            5 => {
                lower_right_diagonal_x = self.room_shape.upper_left_diagonal_x;
                right_x = prev.left_x;
            }
            6 => {
                right_x = self.room_shape.left_x;
                upper_right_diagonal_x = prev.lower_left_diagonal_x;
            }
            7 => {
                upper_right_diagonal_x = self.room_shape.lower_left_diagonal_x;
                top_y = prev.bottom_y;
            }
            // :530. Java's empty `default` — the board's bounding octagon, unchanged.
            _ => {}
        }
        self.insert_incomplete_room(
            board,
            rooms,
            left_x,
            bottom_y,
            right_x,
            top_y,
            upper_left_diagonal_x,
            lower_right_diagonal_x,
            lower_left_diagonal_x,
            upper_right_diagonal_x,
        );

        // :543-598. Java's comment repeats the one above; this is the room from `nextNeighbour`.
        let mut left_x = board_bounding_oct.left_x;
        let mut bottom_y = board_bounding_oct.bottom_y;
        let mut right_x = board_bounding_oct.right_x;
        let mut top_y = board_bounding_oct.top_y;
        let mut upper_left_diagonal_x = board_bounding_oct.upper_left_diagonal_x;
        let mut lower_right_diagonal_x = board_bounding_oct.lower_right_diagonal_x;
        let mut lower_left_diagonal_x = board_bounding_oct.lower_left_diagonal_x;
        let mut upper_right_diagonal_x = board_bounding_oct.upper_right_diagonal_x;
        let next = &next_neighbour.intersection;
        match to_side_index {
            0 => {
                top_y = self.room_shape.bottom_y;
                upper_right_diagonal_x = next.lower_left_diagonal_x;
            }
            1 => {
                upper_left_diagonal_x = self.room_shape.lower_right_diagonal_x;
                top_y = next.bottom_y;
            }
            2 => {
                left_x = self.room_shape.right_x;
                upper_left_diagonal_x = next.lower_right_diagonal_x;
            }
            3 => {
                lower_left_diagonal_x = self.room_shape.upper_right_diagonal_x;
                left_x = next.right_x;
            }
            4 => {
                bottom_y = self.room_shape.top_y;
                lower_left_diagonal_x = next.upper_right_diagonal_x;
            }
            5 => {
                lower_right_diagonal_x = self.room_shape.upper_left_diagonal_x;
                bottom_y = next.top_y;
            }
            6 => {
                right_x = self.room_shape.left_x;
                lower_right_diagonal_x = next.upper_left_diagonal_x;
            }
            7 => {
                upper_right_diagonal_x = self.room_shape.lower_left_diagonal_x;
                right_x = next.left_x;
            }
            // :587.
            _ => {}
        }
        self.insert_incomplete_room(
            board,
            rooms,
            left_x,
            bottom_y,
            right_x,
            top_y,
            upper_left_diagonal_x,
            lower_right_diagonal_x,
            lower_left_diagonal_x,
            upper_right_diagonal_x,
        );

        // :600-608. "Insert the new incomplete rooms on the intermediate free sides of the
        // obstacle expansion room."
        let current_from_side_no = (from_side_index + 1).rem_euclid(8);
        if current_from_side_no == to_side_index {
            return;
        }
        let current_to_side_no = (to_side_index + 7).rem_euclid(8);
        self.calculate_edge_incomplete_rooms_of_obstacle_expansion_room(
            side_index(current_from_side_no, 607),
            side_index(current_to_side_no, 608),
            board,
            rooms,
        );
    }

    /// Port of the private `calculateNewIncompleteRooms`
    /// (Sorted45DegreeRoomNeighbours.java:611-797).
    pub fn calculate_new_incomplete_rooms(
        &self,
        board: &mut Board,
        rooms: &mut ExpansionRoomStore,
    ) {
        // :612.
        let board_bounding_oct = board.bounding_box.bounding_octagon();
        let neighbours: Vec<&SortedRoomNeighbour> = self.sorted_neighbours.iter().collect();
        // :613. `SortedSet.getLast()` throws on an empty set; the one caller guards it (`:71`).
        let Some(&last) = neighbours.last() else {
            panic!(
                "Sorted45DegreeRoomNeighbours.calculateNewIncompleteRooms: the neighbour set is \
                 empty (Sorted45DegreeRoomNeighbours.java:613) — Java throws \
                 NoSuchElementException here too"
            )
        };
        let mut prev_neighbour = last;

        // :614-619. "ObstacleExpansionRoom has only 1 neighbour" — and `prevNeighbour` and
        // `nextNeighbour` are then the *same object*, which is what `:481`'s reference test
        // detects.
        if matches!(self.from_room, RoomRef::Obstacle(_)) && neighbours.len() == 1 {
            self.calculate_new_incomplete_rooms_for_obstacle_expansion_room(
                prev_neighbour,
                prev_neighbour,
                true,
                board,
                rooms,
            );
            return;
        }

        // :621-796.
        for next_neighbour in neighbours.iter().copied() {
            let insert_incomplete_room;

            if matches!(self.completed_room, RoomRef::Obstacle(_)) && neighbours.len() == 2 {
                // :624-642. "check, if this site is touching or open."
                let intersection = next_neighbour
                    .intersection
                    .intersection(&prev_neighbour.intersection);
                if intersection.is_empty() {
                    insert_incomplete_room = true;
                } else if intersection.dimension() >= 1 {
                    insert_incomplete_room = false;
                } else {
                    // Java's comment says "dimension = 1"; the branch is reached when the
                    // dimension is 0, i.e. a "touch at a corner of the room shape".
                    if prev_neighbour.last_touching_side == next_neighbour.first_touching_side {
                        // "touch along the side of the room shape"
                        insert_incomplete_room = false;
                    } else {
                        insert_incomplete_room = prev_neighbour.last_touching_side
                            != (next_neighbour.first_touching_side + 1).rem_euclid(8);
                    }
                }
            } else {
                // :643-646. "the 2 neighbours do not touch"
                insert_incomplete_room = !next_neighbour
                    .intersection
                    .intersects_octagon(&prev_neighbour.intersection);
            }

            if insert_incomplete_room {
                // :648-794. "create a door to a new incomplete expansion room between the last
                // corner of the previous neighbour and the first corner of the current
                // neighbour"
                if matches!(self.from_room, RoomRef::Obstacle(_))
                    && next_neighbour.first_touching_side != prev_neighbour.last_touching_side
                {
                    self.calculate_new_incomplete_rooms_for_obstacle_expansion_room(
                        prev_neighbour,
                        next_neighbour,
                        false,
                        board,
                        rooms,
                    );
                } else {
                    // :658-665.
                    let mut lx = board_bounding_oct.left_x;
                    let mut ly = board_bounding_oct.bottom_y;
                    let mut rx = board_bounding_oct.right_x;
                    let mut uy = board_bounding_oct.top_y;
                    let mut ulx = board_bounding_oct.upper_left_diagonal_x;
                    let mut lrx = board_bounding_oct.lower_right_diagonal_x;
                    let mut llx = board_bounding_oct.lower_left_diagonal_x;
                    let mut urx = board_bounding_oct.upper_right_diagonal_x;
                    let prev = &prev_neighbour.intersection;
                    let next = &next_neighbour.intersection;

                    // :667-791.
                    match next_neighbour.first_touching_side {
                        0 => {
                            if prev.lower_left_diagonal_x < next.lower_left_diagonal_x {
                                urx = next.lower_left_diagonal_x;
                                uy = prev.bottom_y;
                                if prev_neighbour.last_touching_side == 0 {
                                    ulx = prev.lower_right_diagonal_x;
                                }
                            } else if prev.lower_left_diagonal_x > next.lower_left_diagonal_x {
                                rx = next.left_x;
                                urx = prev.lower_left_diagonal_x;
                            } else {
                                urx = next.lower_left_diagonal_x;
                            }
                        }
                        1 => {
                            if prev.bottom_y < next.bottom_y {
                                uy = next.bottom_y;
                                ulx = prev.lower_right_diagonal_x;
                                if prev_neighbour.last_touching_side == 1 {
                                    lx = prev.right_x;
                                }
                            } else if prev.bottom_y > next.bottom_y {
                                uy = prev.bottom_y;
                                urx = next.lower_left_diagonal_x;
                            } else {
                                uy = next.bottom_y;
                            }
                        }
                        2 => {
                            if prev.lower_right_diagonal_x > next.lower_right_diagonal_x {
                                ulx = next.lower_right_diagonal_x;
                                lx = prev.right_x;
                                if prev_neighbour.last_touching_side == 2 {
                                    llx = prev.upper_right_diagonal_x;
                                }
                            } else if prev.lower_right_diagonal_x < next.lower_right_diagonal_x {
                                uy = next.bottom_y;
                                ulx = prev.lower_right_diagonal_x;
                            } else {
                                ulx = next.lower_right_diagonal_x;
                            }
                        }
                        3 => {
                            if prev.right_x > next.right_x {
                                lx = next.right_x;
                                llx = prev.upper_right_diagonal_x;
                                if prev_neighbour.last_touching_side == 3 {
                                    ly = prev.top_y;
                                }
                            } else if prev.right_x < next.right_x {
                                lx = prev.right_x;
                                ulx = next.lower_right_diagonal_x;
                            } else {
                                lx = next.right_x;
                            }
                        }
                        4 => {
                            if prev.upper_right_diagonal_x > next.upper_right_diagonal_x {
                                llx = next.upper_right_diagonal_x;
                                ly = prev.top_y;
                                if prev_neighbour.last_touching_side == 4 {
                                    lrx = prev.upper_left_diagonal_x;
                                }
                            } else if prev.upper_right_diagonal_x < next.upper_right_diagonal_x {
                                lx = next.right_x;
                                llx = prev.upper_right_diagonal_x;
                            } else {
                                llx = next.upper_right_diagonal_x;
                            }
                        }
                        5 => {
                            if prev.top_y > next.top_y {
                                ly = next.top_y;
                                lrx = prev.upper_left_diagonal_x;
                                if prev_neighbour.last_touching_side == 5 {
                                    rx = prev.left_x;
                                }
                            } else if prev.top_y < next.top_y {
                                ly = prev.top_y;
                                llx = next.upper_right_diagonal_x;
                            } else {
                                ly = next.top_y;
                            }
                        }
                        6 => {
                            if prev.upper_left_diagonal_x < next.upper_left_diagonal_x {
                                lrx = next.upper_left_diagonal_x;
                                rx = prev.left_x;
                                if prev_neighbour.last_touching_side == 6 {
                                    urx = prev.lower_left_diagonal_x;
                                }
                            } else if prev.upper_left_diagonal_x > next.upper_left_diagonal_x {
                                ly = next.top_y;
                                lrx = prev.upper_left_diagonal_x;
                            } else {
                                lrx = next.upper_left_diagonal_x;
                            }
                        }
                        7 => {
                            if prev.left_x < next.left_x {
                                rx = next.left_x;
                                urx = prev.lower_left_diagonal_x;
                                if prev_neighbour.last_touching_side == 7 {
                                    uy = prev.bottom_y;
                                }
                            } else if prev.left_x > next.left_x {
                                rx = prev.left_x;
                                lrx = next.upper_left_diagonal_x;
                            } else {
                                rx = next.left_x;
                            }
                        }
                        // :788-790: `FRLogger.warn` (dropped) and nothing else — the board's
                        // bounding octagon goes through unchanged.
                        _ => {}
                    }
                    // :792.
                    self.insert_incomplete_room(board, rooms, lx, ly, rx, uy, ulx, lrx, llx, urx);
                }
            }
            // :795.
            prev_neighbour = next_neighbour;
        }
    }
}

// =================================================================================================
// The static helper
// =================================================================================================

/// Port of the private static `removeNotTouchingBorderLines`
/// (Sorted45DegreeRoomNeighbours.java:174-243): every border line with no touching neighbour is
/// pushed out to `±Limits.CRIT_INT`, and the result normalized.
///
/// Note the index mapping Java uses here: side 0 is the bottom, 1 the lower-right diagonal, 2 the
/// right, 3 the upper-right diagonal, and so on counterclockwise.
fn remove_not_touching_border_lines(
    room_oct: &IntOctagon,
    edge_interior_touches_obstacle: &[bool; 8],
) -> IntOctagon {
    let pick = |touched: bool, value: i32, fallback: i32| if touched { value } else { fallback };
    // :176-230.
    let left_x = pick(
        edge_interior_touches_obstacle[6],
        room_oct.left_x,
        -CRIT_INT,
    );
    let bottom_y = pick(
        edge_interior_touches_obstacle[0],
        room_oct.bottom_y,
        -CRIT_INT,
    );
    let right_x = pick(
        edge_interior_touches_obstacle[2],
        room_oct.right_x,
        CRIT_INT,
    );
    let top_y = pick(edge_interior_touches_obstacle[4], room_oct.top_y, CRIT_INT);
    let upper_left_diagonal_x = pick(
        edge_interior_touches_obstacle[5],
        room_oct.upper_left_diagonal_x,
        -CRIT_INT,
    );
    let lower_right_diagonal_x = pick(
        edge_interior_touches_obstacle[1],
        room_oct.lower_right_diagonal_x,
        CRIT_INT,
    );
    let lower_left_diagonal_x = pick(
        edge_interior_touches_obstacle[7],
        room_oct.lower_left_diagonal_x,
        -CRIT_INT,
    );
    let upper_right_diagonal_x = pick(
        edge_interior_touches_obstacle[3],
        room_oct.upper_right_diagonal_x,
        CRIT_INT,
    );
    // :232-242.
    IntOctagon::new(
        left_x,
        bottom_y,
        right_x,
        top_y,
        upper_left_diagonal_x,
        lower_right_diagonal_x,
        lower_left_diagonal_x,
        upper_right_diagonal_x,
    )
    .normalize()
}

/// `shape.boundingOctagon()` where Java's return type is `IntOctagon`, never `null`.
///
/// The port's [`TileShape::bounding_octagon`] answers `None` for an unbounded simplex, where
/// Java's `Simplex.boundingOctagon` answers a `null`-free octagon built from `±CRIT_INT`
/// ordinates; a `None` here would be that `null` reaching `IntOctagon.intersection`, which is a
/// `NullPointerException`.
fn bounding_octagon_of(shape: &TileShape, java_line: u32) -> IntOctagon {
    shape.bounding_octagon().unwrap_or_else(|| {
        panic!(
            "Sorted45DegreeRoomNeighbours.java:{java_line}: boundingOctagon answered null for an \
             unbounded shape — Java NPEs here too"
        )
    })
}

/// A Java `int` octagon side number, narrowed to the `usize` [`IntOctagon::corner`] takes.
fn side_index(no: i32, java_line: u32) -> usize {
    usize::try_from(no).unwrap_or_else(|_| {
        panic!(
            "Sorted45DegreeRoomNeighbours.java:{java_line}: the touching side is {no} — Java \
             throws ArrayIndexOutOfBoundsException here"
        )
    })
}

// =================================================================================================
// The inner class
// =================================================================================================

/// Port of the private inner class `Sorted45DegreeRoomNeighbours.SortedRoomNeighbour`
/// (Sorted45DegreeRoomNeighbours.java:803-981): "helper class to sort the doors of an expansion
/// room counterclockwise around the border of the room shape."
///
/// This is **not** [`super::sorted_neighbours::SortedRoomNeighbour`]: it carries an [`IntOctagon`]
/// shape and intersection, a first *and* a last touching side rather than one side plus two
/// corner flags, and no memoized corners at all.
#[derive(Debug, Clone)]
pub struct SortedRoomNeighbour {
    /// `searchTreeObject` (:806): "the search tree object of the neighbour room."
    pub search_tree_object: TreeObject,
    /// `searchTreeObject.getId()`, resolved once at construction because the port's leaves hold a
    /// key rather than the object. Two id spaces meet here — quirk #161, exactly as in the base
    /// class.
    pub object_id: i32,
    /// `shape` (:809): "the shape of the neighbour room."
    pub shape: IntOctagon,
    /// `intersection` (:812): "the intersection of this ExpansionRoom shape with the
    /// neighbourShape."
    pub intersection: IntOctagon,
    /// `firstTouchingSide` (:815): "the first side of the room shape, where the neighbourShape
    /// touches." `-1` when "the roomShape may be contained in the neighbourShape" (`:857`).
    pub first_touching_side: i32,
    /// `lastTouchingSide` (:818): "the last side of the room shape, where the neighbourShape
    /// touches." `-1` likewise; `addSortedNeighbour` drops such a neighbour (`:249`).
    pub last_touching_side: i32,
}

impl SortedRoomNeighbour {
    /// Port of the constructor (Sorted45DegreeRoomNeighbours.java:825-916): "creates a new
    /// instance of SortedRoomNeighbour and calculates the first and last touching sides with the
    /// room shape. this.lastTouchingSide will be -1, if sorting did not work because the roomShape
    /// is contained in the neighbour shape."
    ///
    // renamed: the inner class's constructor `SortedRoomNeighbour` is `SortedRoomNeighbour::new`.
    ///
    /// The port takes three arguments Java's does not: `object_id` (the tree's leaves hold a
    /// [`TreeObject`] key, not the object whose `getId()` the comparator subtracts), `room_shape`
    /// and `edge_interior_touches_obstacle` — the two pieces of outer-instance state Java's inner
    /// class reaches through `this$0`, the second of which it **writes** (`:896-911`).
    pub fn new(
        search_tree_object: TreeObject,
        object_id: i32,
        neighbour_shape: IntOctagon,
        intersection: IntOctagon,
        room_shape: &IntOctagon,
        edge_interior_touches_obstacle: &mut [bool; 8],
    ) -> SortedRoomNeighbour {
        // :831-860.
        let first_touching_side = if intersection.bottom_y == room_shape.bottom_y
            && intersection.lower_left_diagonal_x > room_shape.lower_left_diagonal_x
        {
            0
        } else if intersection.lower_right_diagonal_x == room_shape.lower_right_diagonal_x
            && intersection.bottom_y > room_shape.bottom_y
        {
            1
        } else if intersection.right_x == room_shape.right_x
            && intersection.lower_right_diagonal_x < room_shape.lower_right_diagonal_x
        {
            2
        } else if intersection.upper_right_diagonal_x == room_shape.upper_right_diagonal_x
            && intersection.right_x < room_shape.right_x
        {
            3
        } else if intersection.top_y == room_shape.top_y
            && intersection.upper_right_diagonal_x < room_shape.upper_right_diagonal_x
        {
            4
        } else if intersection.upper_left_diagonal_x == room_shape.upper_left_diagonal_x
            && intersection.top_y < room_shape.top_y
        {
            5
        } else if intersection.left_x == room_shape.left_x
            && intersection.upper_left_diagonal_x > room_shape.upper_left_diagonal_x
        {
            6
        } else if intersection.lower_left_diagonal_x == room_shape.lower_left_diagonal_x
            && intersection.left_x > room_shape.left_x
        {
            7
        } else {
            // :855-859. "the roomShape may be contained in the neighbourShape" — both sides are
            // -1 and the constructor returns before the edge-touch loop.
            return SortedRoomNeighbour {
                search_tree_object,
                object_id,
                shape: neighbour_shape,
                intersection,
                first_touching_side: -1,
                last_touching_side: -1,
            };
        };

        // :862-890.
        let last_touching_side = if intersection.lower_left_diagonal_x
            == room_shape.lower_left_diagonal_x
            && intersection.bottom_y > room_shape.bottom_y
        {
            7
        } else if intersection.left_x == room_shape.left_x
            && intersection.lower_left_diagonal_x > room_shape.lower_left_diagonal_x
        {
            6
        } else if intersection.upper_left_diagonal_x == room_shape.upper_left_diagonal_x
            && intersection.left_x > room_shape.left_x
        {
            5
        } else if intersection.top_y == room_shape.top_y
            && intersection.upper_left_diagonal_x > room_shape.upper_left_diagonal_x
        {
            4
        } else if intersection.upper_right_diagonal_x == room_shape.upper_right_diagonal_x
            && intersection.top_y < room_shape.top_y
        {
            3
        } else if intersection.right_x == room_shape.right_x
            && intersection.upper_right_diagonal_x < room_shape.upper_right_diagonal_x
        {
            2
        } else if intersection.lower_right_diagonal_x == room_shape.lower_right_diagonal_x
            && intersection.right_x < room_shape.right_x
        {
            1
        } else if intersection.bottom_y == room_shape.bottom_y
            && intersection.lower_right_diagonal_x < room_shape.lower_right_diagonal_x
        {
            0
        } else {
            // :886-889. Only `lastTouchingSide` is -1 here; `firstTouchingSide` keeps the value
            // above, and the edge-touch loop is skipped.
            return SortedRoomNeighbour {
                search_tree_object,
                object_id,
                shape: neighbour_shape,
                intersection,
                first_touching_side,
                last_touching_side: -1,
            };
        };

        // :892-915. Walk the sides from the first to the last touching one and mark each whose
        // *interior* the neighbour touches. Terminates because both indices are in `0..8`.
        let mut next_side_no = first_touching_side as usize;
        loop {
            let current_side_index = next_side_no;
            next_side_no = (next_side_no + 1) % 8;
            if !edge_interior_touches_obstacle[current_side_index] {
                let mut touch_only_at_corner = false;
                // :898-902.
                if current_side_index as i32 == first_touching_side
                    && intersection.corner(current_side_index) == room_shape.corner(next_side_no)
                {
                    touch_only_at_corner = true;
                }
                // :903-907.
                if current_side_index as i32 == last_touching_side
                    && intersection.corner(next_side_no) == room_shape.corner(current_side_index)
                {
                    touch_only_at_corner = true;
                }
                if !touch_only_at_corner {
                    edge_interior_touches_obstacle[current_side_index] = true;
                }
            }
            // :912-914.
            if current_side_index as i32 == last_touching_side {
                break;
            }
        }

        SortedRoomNeighbour {
            search_tree_object,
            object_id,
            shape: neighbour_shape,
            intersection,
            first_touching_side,
            last_touching_side,
        }
    }

    /// Port of `compareTo(SortedRoomNeighbour)` (Sorted45DegreeRoomNeighbours.java:922-980):
    /// "compare function for or sorting the neighbours in counterclock sense around the border of
    /// the room shape in ascending order."
    ///
    /// Transcribed line for line, including the four `is2 - is1` reversals for the sides that run
    /// the other way round the octagon and the wrapping Java `int` subtractions.
    pub fn compare_to(&self, other: &SortedRoomNeighbour) -> Ordering {
        // :924-929.
        if self.first_touching_side > other.first_touching_side {
            return Ordering::Greater;
        }
        if self.first_touching_side < other.first_touching_side {
            return Ordering::Less;
        }

        // :931-949. "now the first touch of this and other is at the same side"
        let is1 = &self.intersection;
        let is2 = &other.intersection;
        let corner_x = |oct: &IntOctagon, no: usize| oct.corner(no).x;
        let corner_y = |oct: &IntOctagon, no: usize| oct.corner(no).y;
        let mut cmp_value: i32 = match self.first_touching_side {
            0 => corner_x(is1, 0).wrapping_sub(corner_x(is2, 0)),
            1 => corner_x(is1, 1).wrapping_sub(corner_x(is2, 1)),
            2 => corner_y(is1, 2).wrapping_sub(corner_y(is2, 2)),
            3 => corner_y(is1, 3).wrapping_sub(corner_y(is2, 3)),
            4 => corner_x(is2, 4).wrapping_sub(corner_x(is1, 4)),
            5 => corner_x(is2, 5).wrapping_sub(corner_x(is1, 5)),
            6 => corner_y(is2, 6).wrapping_sub(corner_y(is1, 6)),
            7 => corner_y(is2, 7).wrapping_sub(corner_y(is1, 7)),
            // :945-948: `FRLogger.warn` (dropped) and `return 0`. Reachable with the `-1` a
            // neighbour that never entered the set carries.
            _ => return Ordering::Equal,
        };

        if cmp_value == 0 {
            // :951-974. "The first touching points of this neighbour and other with the room
            // shape are equal. Compare the last touching points."
            let this_touching_side_diff =
                (self.last_touching_side - self.first_touching_side + 8).rem_euclid(8);
            let other_touching_side_diff =
                (other.last_touching_side - other.first_touching_side + 8).rem_euclid(8);
            if this_touching_side_diff > other_touching_side_diff {
                return Ordering::Greater;
            }
            if this_touching_side_diff < other_touching_side_diff {
                return Ordering::Less;
            }
            // :962-973. "now the last touch of this and other is at the same side". Java's
            // `default` arm is empty, so `cmpValue` keeps its 0.
            cmp_value = match self.last_touching_side {
                0 => corner_x(is1, 1).wrapping_sub(corner_x(is2, 1)),
                1 => corner_x(is1, 2).wrapping_sub(corner_x(is2, 2)),
                2 => corner_y(is1, 3).wrapping_sub(corner_y(is2, 3)),
                3 => corner_y(is1, 4).wrapping_sub(corner_y(is2, 4)),
                4 => corner_x(is2, 5).wrapping_sub(corner_x(is1, 5)),
                5 => corner_x(is2, 6).wrapping_sub(corner_x(is1, 6)),
                6 => corner_y(is2, 7).wrapping_sub(corner_y(is1, 7)),
                7 => corner_y(is2, 0).wrapping_sub(corner_y(is1, 0)),
                _ => 0,
            };
        }
        if cmp_value == 0 {
            // :975-978. "Deterministic tie-breaker for identical geometry" — and the two ids come
            // from two different counters (quirk #161).
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
