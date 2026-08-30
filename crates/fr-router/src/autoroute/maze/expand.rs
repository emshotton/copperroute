//! Part B of `autoroute.maze.MazeSearchEngine` (MazeSearchEngine.java:390-966, `:1105-1215`) —
//! the room-door expansion and the cost model that scores it.
//!
//! Task 11's `search.rs` owns the frame: the struct, `init`, and the pop loop that dispatches
//! here. This file is the body of one pop: `expandToRoomDoors` (`:390-626`) decides whether the
//! room behind the popped door section may be crossed at all, `expandToTargetDoors` (`:629-705`)
//! reaches the start and destination items inside it, `expandToDoor` (`:707-761`) walks the room's
//! other doors, and `expandToDoorSection` (`:791-966`) is where every new queue element and every
//! number in it is made. Three helpers round it out: `roomShapeIsThick` (`:1105-1123`),
//! `shoveTraceRoom` (`:1130-1201`) and `checkNeckDownAtDestPin` (`:1207-1215`).
//!
//! # The cost model, in one place
//!
//! `expandToDoorSection` is the only producer of `MazeListElement`s outside `init`, so it is the
//! only place the A\* costs are formed (`:855-887`):
//!
//! * a **bend penalty** of `ctrl.bendCosts[layer]`, charged when the cross product of the incoming
//!   and outgoing directions satisfies `sin² > 0.01` (about 5.7°) — a *normalised* test, so it is
//!   scale-independent;
//! * `expansionValue = from.expansionValue + addCosts + bend + weightedDistance(from.mid, to.mid)`
//!   under the layer's horizontal/vertical trace costs;
//! * `sortingValue = expansionValue + destinationDistance.calculate(mid, layer)` — the admissible
//!   lower bound that makes the queue an A\* frontier;
//! * `roomRipped` is set by a **positive `addCosts` with no adjustment**, or inherited from an
//!   already-checked parent that was itself ripped; `ripupCost` records only the former.
//!
//! # What Task 13 filled in
//!
//! Four call sites here were `unimplemented!` markers until Task 13:
//! `MazeRipupResolver.checkLeavingRippedItem` / `.checkRipup` (`:506`, `:519`) and
//! `MazeExpansionEngine.expandToDrillPage` / `.expandToDrill` (`:611`, `:620`, the second through
//! `Via.getAutorouteDrillInfo`). They now dispatch to
//! [`MazeRipupResolver`] and [`MazeExpansionEngine`]. Their branches are
//! guarded by `ctrl.ripupAllowed`, `currentDoorIsSmall` and `ctrl.viasAllowed`, so a control with
//! vias and ripup switched off still runs the whole of this file — which is what
//! `crates/fr-router/tests/maze_expand.rs` does.
//!
//! # Visibility
//!
//! All seven of this file's Java methods are `private`, and all seven are `pub` here, for the
//! reason `search.rs`' module docs give for its four: Java's own test for them is a reflective
//! probe (`scripts/differential/java/probes/P6T12Probe.java` calls `setAccessible(true)` on
//! exactly these), Rust integration tests have no reflection, and the branches that matter — the
//! layer-active gate, the small-door refusal, the bend threshold, the stale-index `continue`s —
//! are not separable through `occupyNextElement` alone.

use fr_board::{Board, Item, ObstacleRoomId};
use fr_geometry::{FloatLine, FloatPoint, Point, Polyline, java_min};

use crate::arena::DoorId;
use crate::autoroute::expansion::{ExpandableRef, RoomRef};
use crate::autoroute::maze::expansion_engine::via_autoroute_drill_info;
use crate::autoroute::maze::search::segment_projection;
use crate::autoroute::maze::trace_shover::{DoorSection, MazeTraceShover};
use crate::autoroute::maze::{
    ALREADY_RIPPED_COSTS, MazeAdjustment, MazeExpansionEngine, MazeListElement, MazeRipupResolver,
    MazeSearchEngine, TRACE_WIDTH_TOLERANCE,
};
use crate::board_ext::RoutingBoardExt;

impl MazeSearchEngine<'_> {
    // =============================================================================================
    // expandToRoomDoors (:390-626)
    // =============================================================================================

    /// Port of `expandToRoomDoors(MazeListElement)` (MazeSearchEngine.java:390-626): "expands the
    /// other door section of the room. Returns true, if the from door section has to be occupied,
    /// and false, if the occupation is delayed."
    ///
    /// Java takes no `Stoppable` here — `occupyNextElement`'s check at `:323` is the last one
    /// before this runs — so the port's parameter list has no `stop` either.
    ///
    /// Every `FRLogger.trace` payload (`:421-438`, `:560-588`) is dropped, and with it the
    /// `doorCountBeforeCompletion` / `doorCountAfterCompletion` pair that exists only to fill one
    /// in.
    pub fn expand_to_room_doors(
        &mut self,
        board: &mut Board,
        list_element: &MazeListElement,
    ) -> bool {
        // :392-394. "Complete the neighbour rooms to make sure, that the doors of this room will
        // not change later on."
        let Some(next_room) = list_element.next_room else {
            // Java's caller (`:375`) has already tested `nextRoom != null`.
            return true;
        };
        let Some(layer_index) = self.engine.rooms.room_layer(board, next_room) else {
            // Java would NPE on `nextRoom.getLayer()`.
            return true;
        };

        // :396-401.
        let layer_active = self.ctrl.layer_active[layer_index];
        if !layer_active && board.layer_structure().layers[layer_index].is_signal {
            return true;
        }

        // :403-416.
        let mut half_width = f64::from(self.ctrl.compensated_trace_half_width[layer_index]);
        let mut current_door_is_small = false;
        if let ExpandableRef::Door(current_door) = list_element.door {
            let mut half_width_add = half_width + f64::from(TRACE_WIDTH_TOLERANCE);
            if self.ctrl.with_neckdown {
                // :407-414. "try evtl. neckdown at a destination pin".
                let neck_down_half_width = self.check_neck_down_at_dest_pin(board, next_room);
                if neck_down_half_width > 0.0 {
                    half_width_add = java_min(half_width_add, neck_down_half_width);
                    half_width = half_width_add;
                }
            }
            // :415.
            current_door_is_small = self.door_is_small(board, current_door, 2.0 * half_width_add);
        }

        // :418-420. The two door counts either side are the dropped trace payload's; the call is
        // not.
        self.engine.complete_neighbour_rooms(board, next_room);

        // :440.
        let shape_entry_middle = list_element
            .shape_entry
            .a
            .middle_point(&list_element.shape_entry.b);

        // :442-451. "try evtl. neckdown at a start pin".
        if self.ctrl.with_neckdown
            && let ExpandableRef::TargetDoor(door) = list_element.door
        {
            let start_item = self
                .engine
                .rooms
                .target_door(door)
                .map(|target_door| target_door.item);
            if let Some(start_item) = start_item {
                let ctx = board.ctx();
                if let Some(Item::Pin(pin)) = board.items.get(&start_item) {
                    let neckdown_half_width =
                        f64::from(pin.get_trace_neckdown_halfwidth(layer_index, &ctx));
                    if neckdown_half_width > 0.0 {
                        half_width = java_min(half_width, neckdown_half_width);
                    }
                }
            }
        }

        // :453-477.
        let mut next_room_is_thick = true;
        if let RoomRef::Obstacle(obstacle_room) = next_room {
            // :454-455.
            next_room_is_thick = self.room_shape_is_thick(board, obstacle_room);
        } else {
            // :457.
            let Some(next_room_shape) = self.engine.rooms.room_shape(next_room).cloned() else {
                return true;
            };
            if next_room_shape.min_width() < 2.0 * half_width {
                // :458-459. "to prevent problems with the opposite side".
                next_room_is_thick = false;
            } else if !list_element.already_checked
                && self.expandable_dimension(list_element.door) == 1
                && !current_door_is_small
            {
                // :460-476. "The algorithm below works only, if location is on the border of
                // roomShape. That is only the case for 1 dimensional doors. For small doors the
                // check is done in check_leaving_via below."
                let nearest_points =
                    next_room_shape.nearest_border_points_approx(&shape_entry_middle, 2);
                if nearest_points.len() < 2 {
                    // "MazeSearchEngine.expand_to_room_doors: nearestPoints.length == 2 expected"
                    next_room_is_thick = false;
                } else {
                    let current_distance = nearest_points[1].distance(&shape_entry_middle);
                    next_room_is_thick = current_distance > half_width + 1.0;
                }
            }
        }

        // :478-489. "check for drill to a foreign conduction area on split plane."
        if !layer_active && let ExpandableRef::Drill(drill) = list_element.door {
            let drill_location = self
                .engine
                .rooms
                .drills
                .get(drill.0)
                .map(|drill| drill.location.clone());
            if let Some(drill_location) = drill_location {
                // `new ItemSelectionFilter(SelectableChoices.CONDUCTION)` also selects both
                // FIXED and UNFIXED (ItemSelectionFilter.java:27-32), so the fixed half of
                // `Item.isSelectedByFilter` never rejects anything and the filter reduces to
                // "is a ConductionArea" (ConductionArea.java:407-413).
                //
                // Java's `pickItems` answers a `TreeSet<Item>`, i.e. **descending** id
                // (`Item.compareTo`, Item.java:95-101), so the walk is `.rev()`. The order is
                // not observable — the loop answers `true` for *any* foreign-net member —
                // but the convention is to walk Java's order regardless.
                let picked_items = board.pick_items(&drill_location, Some(layer_index));
                for current_item in picked_items.into_iter().rev() {
                    let is_foreign_conduction =
                        board.items.get(&current_item).is_some_and(|item| {
                            matches!(item, Item::ConductionArea(_))
                                && !item.contains_net(self.ctrl.net_number)
                        });
                    if is_foreign_conduction {
                        return true;
                    }
                }
            }
        }

        // :490-491.
        let mut something_expanded = self.expand_to_target_doors(
            board,
            list_element,
            next_room_is_thick,
            current_door_is_small,
            &shape_entry_middle,
        );

        // :493-495.
        if !layer_active {
            return true;
        }

        // :497.
        let mut ripup_costs: i32 = 0;

        match next_room {
            // :499-512. A `CompleteFreeSpaceExpansionRoom` is Java's `FreeSpaceExpansionRoom`
            // here: an incomplete one can never be a `MazeListElement.nextRoom`, which is typed
            // `CompleteExpansionRoom`.
            RoomRef::Complete(_) if !list_element.already_checked && current_door_is_small => {
                let mut enter_through_small_door = false;
                if next_room_is_thick {
                    // :504-506. "check to enter the thick room from a ripped item through a small
                    // door (after ripup)".
                    enter_through_small_door =
                        MazeRipupResolver::check_leaving_ripped_item(self, board, list_element);
                }
                if !enter_through_small_door {
                    return something_expanded;
                }
            }
            RoomRef::Obstacle(obstacle_room) if !list_element.already_checked => {
                // :513-556.
                let mut room_rippable = false;
                if self.ctrl.ripup_allowed {
                    // :518-520.
                    let obstacle_item = self
                        .engine
                        .rooms
                        .obstacle_room(obstacle_room)
                        .map(super::super::expansion::ObstacleExpansionRoom::get_item);
                    ripup_costs = match obstacle_item {
                        Some(item) => MazeRipupResolver::check_ripup(
                            self,
                            board,
                            list_element,
                            item,
                            current_door_is_small,
                        ),
                        None => -1,
                    };
                    room_rippable = ripup_costs >= 0;
                }

                // :523-552.
                if ripup_costs != ALREADY_RIPPED_COSTS && next_room_is_thick {
                    let obstacle_item = self
                        .engine
                        .rooms
                        .obstacle_room(obstacle_room)
                        .map(super::super::expansion::ObstacleExpansionRoom::get_item);
                    let obstacle_is_trace = obstacle_item
                        .and_then(|item| board.items.get(&item))
                        .is_some_and(|item| matches!(item, Item::Trace(_)));
                    if !current_door_is_small
                        && self.ctrl.max_shove_trace_recursion_depth > 0
                        && obstacle_is_trace
                    {
                        // :528.
                        let shoved = self.shove_trace_room(board, list_element, obstacle_room);
                        if !shoved {
                            if ripup_costs > 0 {
                                // :530-548. "delay the occupation by ripup to allow shoving the
                                // room by another door sections."
                                let new_element = MazeListElement {
                                    door: list_element.door,
                                    section_no_of_door: list_element.section_no_of_door,
                                    backtrack_door: list_element.backtrack_door,
                                    section_no_of_backtrack_door: list_element
                                        .section_no_of_backtrack_door,
                                    expansion_value: list_element.expansion_value
                                        + f64::from(ripup_costs),
                                    sorting_value: list_element.sorting_value
                                        + f64::from(ripup_costs),
                                    next_room: list_element.next_room,
                                    shape_entry: list_element.shape_entry,
                                    room_ripped: true,
                                    adjustment: list_element.adjustment,
                                    already_checked: true,
                                    // :546. Java writes the field after construction.
                                    ripup_cost: ripup_costs,
                                };
                                self.push(new_element, board);
                            }
                            // :549.
                            return something_expanded;
                        }
                    }
                }
                // :553-555.
                if !room_rippable {
                    return true;
                }
            }
            _ => {}
        }

        // :559. **The snapshot.** `new LinkedList<>(nextRoom.getDoors())` is taken *after*
        // `completeNeighbourRooms` (`:419`) has already rewritten the list — it guards the
        // iteration below, not the completion above.
        let room_doors_snapshot = self.engine.rooms.room_doors(next_room).to_vec();

        // :590-598.
        for to_door in room_doors_snapshot {
            // :591-593. Java's `==` on the two `ExpandableObject`s is reference identity, which
            // is `ExpandableRef` equality here: a target door is never equal to an expansion door.
            if list_element.door == ExpandableRef::Door(to_door) {
                continue;
            }
            if self.expand_to_door(
                board,
                to_door,
                list_element,
                ripup_costs,
                next_room_is_thick,
                MazeAdjustment::None,
            ) {
                something_expanded = true;
            }
        }

        // :600-623. "Expand also the drill pages intersecting the room."
        if self.ctrl.vias_allowed && !matches!(list_element.door, ExpandableRef::Drill(_)) {
            if (something_expanded || next_room_is_thick)
                && matches!(next_room, RoomRef::Complete(_))
            {
                // :602-614. "avoid setting somethingExpanded to true when nextRoom is thin to
                // allow occupying by different sections of the door".
                let Some(next_room_shape) = self.engine.rooms.room_shape(next_room).cloned() else {
                    return something_expanded;
                };
                let overlapping_drill_pages = self
                    .engine
                    .drill_pages()
                    .overlapping_pages(&next_room_shape);
                for to_drill_page in overlapping_drill_pages {
                    MazeExpansionEngine::expand_to_drill_page(
                        self,
                        board,
                        to_drill_page,
                        list_element,
                    );
                    something_expanded = true;
                }
            } else if let RoomRef::Obstacle(obstacle_room) = next_room {
                // :615-621.
                let obstacle_item = self
                    .engine
                    .rooms
                    .obstacle_room(obstacle_room)
                    .map(super::super::expansion::ObstacleExpansionRoom::get_item);
                let is_via = obstacle_item
                    .and_then(|item| board.items.get(&item))
                    .is_some_and(|item| matches!(item, Item::Via(_)));
                if let (Some(current_via), true) = (obstacle_item, is_via) {
                    // :618-619. `Via.getAutorouteDrillInfo(autorouteSearchTree)` — a `None` is
                    // the port's stale-item read, where Java holds the live `Via`.
                    if let Some(via_drill_info) =
                        via_autoroute_drill_info(self.engine, board, current_via)
                    {
                        // :620.
                        MazeExpansionEngine::expand_to_drill(
                            self,
                            board,
                            via_drill_info,
                            list_element,
                            ripup_costs,
                        );
                    }
                }
            }
        }

        // :625.
        something_expanded
    }

    // =============================================================================================
    // expandToTargetDoors (:629-705)
    // =============================================================================================

    /// Port of `expandToTargetDoors(MazeListElement, boolean, boolean, FloatPoint)`
    /// (MazeSearchEngine.java:629-704): "expand the target doors of the room. Returns true, if at
    /// least 1 target door was expanded."
    ///
    /// The two `continue`s at `:653-668` are the stale-index guard the comment there calls
    /// "prevents warning when indices become stale during routing": an item's tree shapes can be
    /// rebuilt underneath a `TargetItemExpansionDoor` that keeps its `treeEntryNo`. Both are
    /// silent, and both are transcribed verbatim.
    pub fn expand_to_target_doors(
        &mut self,
        board: &mut Board,
        list_element: &MazeListElement,
        next_room_is_thick: bool,
        current_door_is_small: bool,
        shape_entry_middle: &FloatPoint,
    ) -> bool {
        let Some(next_room) = list_element.next_room else {
            return false;
        };
        // :634-647.
        if current_door_is_small {
            let mut enter_through_small_door = false;
            if let ExpandableRef::Door(door) = list_element.door {
                // :636-643. "otherwise entering through the small door may fail, because it was
                // not checked."
                let from_room = self
                    .engine
                    .rooms
                    .door(door)
                    .and_then(|door| door.other_complete_room(next_room));
                if matches!(from_room, Some(RoomRef::Obstacle(_))) {
                    enter_through_small_door = true;
                }
            }
            if !enter_through_small_door {
                return false;
            }
        }
        // :648.
        let mut result = false;
        // :649. A snapshot, because `expandToDoorSection` can append to the queue but not to the
        // room — Java iterates the live `List` and the port needs an owned copy to satisfy the
        // borrow checker; the two agree because nothing in the loop touches the room's doors.
        let target_doors = self.engine.rooms.room_target_doors(next_room).to_vec();
        for to_door in target_doors {
            // :650-652.
            if list_element.door == ExpandableRef::TargetDoor(to_door) {
                continue;
            }
            let Some(door) = self.engine.rooms.target_door(to_door) else {
                continue;
            };
            let (item, tree_entry_no) = (door.item, door.tree_entry_no);
            // :653-659. "Validate index before calling - prevents warning when indices become
            // stale during routing". `treeEntryNo` is a `usize` here, so Java's `< 0` arm is
            // unrepresentable.
            let tree_shape_count = board.item_tree_shape_count(item, self.search_tree);
            if tree_entry_no >= tree_shape_count {
                continue;
            }
            // :660-668.
            let target_shape = {
                let ctx = board.ctx();
                board
                    .items
                    .get(&item)
                    .and_then(Item::as_connectable)
                    .and_then(|connectable| {
                        connectable.as_dyn().get_trace_connection_shape(
                            self.search_tree,
                            tree_entry_no,
                            &ctx,
                        )
                    })
            };
            let Some(target_shape) = target_shape else {
                // "Item's tree shape index out of range (can happen when traces are modified
                // during routing)".
                continue;
            };
            // :669.
            let Some(connection_point) = target_shape.nearest_point_approx(shape_entry_middle)
            else {
                // Java would NPE; an empty target shape has no connection point either way.
                continue;
            };
            // :670-694.
            if !next_room_is_thick {
                // "check the line from shapeEntryMiddle to the nearest point."
                let current_net_numbers = [self.ctrl.net_number];
                let Some(current_layer) = self.engine.rooms.room_layer(board, next_room) else {
                    continue;
                };
                let check_points = [
                    Point::from(shape_entry_middle.round()),
                    Point::from(connection_point.round()),
                ];
                if check_points[0] != check_points[1] {
                    let check_polyline = Polyline::from_points(&check_points);
                    let check_ok = board.check_forced_trace_polyline(
                        &check_polyline,
                        self.ctrl.trace_half_width[current_layer],
                        current_layer,
                        &current_net_numbers,
                        self.ctrl.trace_clearance_class_index,
                        self.ctrl.max_shove_trace_recursion_depth,
                        self.ctrl.max_shove_via_recursion_depth,
                        self.ctrl.max_spring_over_recursion_depth,
                    );
                    if !check_ok {
                        continue;
                    }
                }
            }

            // :696.
            let new_shape_entry = FloatLine::new(connection_point, connection_point);

            // :698-701.
            if self.expand_to_door_section(
                board,
                ExpandableRef::TargetDoor(to_door),
                0,
                Some(&new_shape_entry),
                list_element,
                0,
                MazeAdjustment::None,
            ) {
                result = true;
            }
        }
        // :703.
        result
    }

    // =============================================================================================
    // expandToDoor (:707-760)
    // =============================================================================================

    /// Port of `expandToDoor(ExpansionDoor, MazeListElement, int, boolean, Adjustment)`
    /// (MazeSearchEngine.java:707-759): "return true, if at least 1 door section was expanded."
    ///
    /// `:715` is one of the three `getSectionSegments` calls that can **reallocate** the door's
    /// section array (Task 11 review S2). Java reads `toDoor.sectionArr[i]` at `:718` through the
    /// field, i.e. through the array that call has just installed, and `expandToDoorSection` reads
    /// it again through `getMazeSearchElement`. The port resolves the door out of the arena at
    /// both points for exactly that reason: nothing is cached across the call.
    pub fn expand_to_door(
        &mut self,
        board: &mut Board,
        to_door: DoorId,
        list_element: &MazeListElement,
        add_costs: i32,
        next_room_is_thick: bool,
        adjustment: MazeAdjustment,
    ) -> bool {
        let Some(next_room) = list_element.next_room else {
            return false;
        };
        let Some(layer) = self.engine.rooms.room_layer(board, next_room) else {
            return false;
        };
        // :713.
        let half_width = f64::from(self.ctrl.compensated_trace_half_width[layer]);
        // :714.
        let mut something_expanded = false;
        // :715 — allocates the section array.
        let line_sections = self.engine.rooms.door_section_segments(to_door, half_width);

        // :717-758.
        for (i, line_section) in line_sections.iter().enumerate() {
            // :718-720.
            let is_occupied = self
                .engine
                .rooms
                .door(to_door)
                .and_then(|door| door.get_maze_search_element(i))
                .is_some_and(|section| section.is_occupied);
            if is_occupied {
                continue;
            }
            let new_shape_entry;
            if next_room_is_thick {
                // :722-740.
                new_shape_entry = *line_section;
                let door = self.engine.rooms.door(to_door);
                let both_free_space = door.is_some_and(|door| {
                    matches!(door.first_room, RoomRef::Complete(_))
                        && matches!(door.second_room, RoomRef::Complete(_))
                });
                let dimension = door.map_or(0, |door| door.dimension);
                if dimension == 1 && line_sections.len() == 1 && both_free_space {
                    // "check entering the toDoor at an acute corner of the shape of
                    // listElement.nextRoom"
                    let shape_entry_middle = new_shape_entry.a.middle_point(&new_shape_entry.b);
                    let Some(room_shape) = self.engine.rooms.room_shape(next_room).cloned() else {
                        return false;
                    };
                    // :732-734.
                    if room_shape.min_width() < 2.0 * half_width {
                        return false;
                    }
                    // :735-739.
                    let nearest_points =
                        room_shape.nearest_border_points_approx(&shape_entry_middle, 2);
                    if nearest_points.len() < 2
                        || nearest_points[1].distance(&shape_entry_middle) <= half_width + 1.0
                    {
                        return false;
                    }
                }
            } else {
                // :741-753. "expand only doors on the opposite side of the room from the
                // shapeEntry."
                let dimension = self
                    .engine
                    .rooms
                    .door(to_door)
                    .map_or(0, |door| door.dimension);
                if dimension == 1
                    && i == 0
                    && line_sections[0].b.distance_square(&line_sections[0].a) < 1.0
                {
                    // "toDoor is small belonging to a via or thin room"
                    continue;
                }
                let Some(projected) = segment_projection(&list_element.shape_entry, line_section)
                else {
                    continue;
                };
                new_shape_entry = projected;
            }

            // :755-757.
            if self.expand_to_door_section(
                board,
                ExpandableRef::Door(to_door),
                i32::try_from(i).unwrap_or(i32::MAX),
                Some(&new_shape_entry),
                list_element,
                add_costs,
                adjustment,
            ) {
                something_expanded = true;
            }
        }
        // :759.
        something_expanded
    }

    // =============================================================================================
    // expandToDoorSection (:791-966) — the cost model
    // =============================================================================================

    /// Port of `expandToDoorSection(ExpandableObject, int, FloatLine, MazeListElement, int,
    /// Adjustment)` (MazeSearchEngine.java:791-965): "return true, if the door section was
    /// successfully expanded."
    ///
    /// `shape_entry` is an `Option` because Java's parameter is nullable and `:799` tests it:
    /// `expandToDoor`'s `segmentProjection` can answer `null`, and so can a caller in Task 13.
    ///
    /// Both `FRLogger.trace` blocks (`:800-848`, `:907-963`) are dropped; the `return false` the
    /// first guards and the `mazeExpansionList.add` the second precedes are not.
    #[allow(clippy::too_many_arguments)]
    pub fn expand_to_door_section(
        &mut self,
        board: &mut Board,
        door: ExpandableRef,
        section_index: i32,
        shape_entry: Option<&FloatLine>,
        from_element: &MazeListElement,
        add_costs: i32,
        adjustment: MazeAdjustment,
    ) -> bool {
        // :798-799. Java evaluates `door.getMazeSearchElement(sectionIndex).isOccupied` *before*
        // the null test on `shapeEntry`, so an unallocated section array throws even for a null
        // entry; the port resolves it in the same order.
        let door_section_occupied = self
            .engine
            .maze_search_element(door, section_index)
            .unwrap_or_else(|| {
                panic!(
                    "MazeSearchEngine.expandToDoorSection: no maze search element for section \
                     {section_index} of {door:?} — Java throws at MazeSearchEngine.java:798"
                )
            })
            .is_occupied;
        let Some(shape_entry) = shape_entry else {
            // :799-849.
            return false;
        };
        if door_section_occupied {
            return false;
        }
        let Some(from_next_room) = from_element.next_room else {
            // Java's `door.otherRoom(null)` answers null, and `getLayer()` on it throws.
            return false;
        };
        // :851-853.
        let next_room = self.engine.expandable_other_room(door, from_next_room);
        let Some(layer) = self.engine.rooms.room_layer(board, from_next_room) else {
            return false;
        };
        let shape_entry_middle = shape_entry.a.middle_point(&shape_entry.b);

        // :855-873. The bend penalty.
        let mut bend_cost_penalty = 0.0;
        if self.ctrl.bend_costs[layer] > 0.0
            && let Some(backtrack_door) = from_element.backtrack_door
        {
            // :857.
            let from_mid = from_element
                .shape_entry
                .a
                .middle_point(&from_element.shape_entry.b);
            // :858-859. "Build vectors prev→current and current→next to detect a direction
            // change."
            let Some(backtrack_cog) = self.expandable_shape_centre(backtrack_door) else {
                return false;
            };
            let prev_dx = from_mid.x - backtrack_cog.x;
            let prev_dy = from_mid.y - backtrack_cog.y;
            let next_dx = shape_entry_middle.x - from_mid.x;
            let next_dy = shape_entry_middle.y - from_mid.y;
            let cross_product = prev_dx * next_dy - prev_dy * next_dx;
            let sq_len_prev = prev_dx * prev_dx + prev_dy * prev_dy;
            let sq_len_next = next_dx * next_dx + next_dy * next_dy;
            // :867-872. "Use a normalized threshold (sin² of angle > 0.01, approx. 5.7°) to
            // be scale-independent."
            if sq_len_prev > 0.0
                && sq_len_next > 0.0
                && (cross_product * cross_product) > 0.01 * sq_len_prev * sq_len_next
            {
                bend_cost_penalty = self.ctrl.bend_costs[layer];
            }
        }

        // :875-882.
        let expansion_value = from_element.expansion_value
            + f64::from(add_costs)
            + bend_cost_penalty
            + shape_entry_middle.weighted_distance(
                &from_element
                    .shape_entry
                    .a
                    .middle_point(&from_element.shape_entry.b),
                self.ctrl.trace_costs[layer].horizontal,
                self.ctrl.trace_costs[layer].vertical,
            );
        // :883-884.
        let sorting_value = expansion_value
            + self
                .destination_distance
                .calculate_from_point(&shape_entry_middle, layer);
        // :885-887.
        let room_ripped = add_costs > 0 && adjustment == MazeAdjustment::None
            || from_element.already_checked && from_element.room_ripped;

        // :889-906. "Store the direct ripup cost on this element (non-zero only when this specific
        // door caused a ripup; propagated roomRipped from a parent keeps ripupCost=0)."
        let new_element = MazeListElement {
            door,
            section_no_of_door: section_index,
            backtrack_door: Some(from_element.door),
            section_no_of_backtrack_door: from_element.section_no_of_door,
            expansion_value,
            sorting_value,
            next_room,
            shape_entry: *shape_entry,
            room_ripped,
            adjustment,
            already_checked: false,
            ripup_cost: if add_costs > 0 && adjustment == MazeAdjustment::None {
                add_costs
            } else {
                0
            },
        };
        // :964. The guarded `add`, whose answer Java discards (quirk #178's sibling site).
        self.push(new_element, board);
        // :965.
        true
    }

    // =============================================================================================
    // roomShapeIsThick (:1105-1123)
    // =============================================================================================

    /// Port of `roomShapeIsThick(ObstacleExpansionRoom)` (MazeSearchEngine.java:1105-1123).
    ///
    /// The `FRLogger.warn` at `:1119` is dropped; its `obstacleHalfWidth = 0` is not, and it is
    /// reachable — an `ObstacleArea` room and a `Pin` room both take it (the pin is a `DrillItem`
    /// but not a `Via`), and `0 >= compensatedTraceHalfWidth` is false for every positive trace.
    pub fn room_shape_is_thick(&self, board: &Board, obstacle_room: ObstacleRoomId) -> bool {
        let Some(room) = self.engine.rooms.obstacle_room(obstacle_room) else {
            return false;
        };
        // :1106-1107.
        let obstacle_item = room.get_item();
        let Some(layer) = room.get_layer(board) else {
            return false;
        };
        // :1108-1121.
        let obstacle_half_width = match board.items.get(&obstacle_item) {
            // :1109-1113.
            Some(Item::Trace(trace)) => {
                let clearance_class = trace.hdr.clearance_class();
                let compensation = board
                    .trees
                    .trees()
                    .find(|tree| tree.id() == self.search_tree)
                    .map_or(0, |tree| {
                        tree.clearance_compensation_value(clearance_class, layer, &board.rules)
                    });
                f64::from(trace.get_half_width()) + f64::from(compensation)
            }
            // :1115-1117.
            Some(Item::Via(via)) => {
                let ctx = board.ctx();
                match via.get_tree_shape_on_layer(self.search_tree, layer, &ctx) {
                    Some(via_shape) => 0.5 * via_shape.max_width(),
                    // Java would NPE on a via with no shape on this layer.
                    None => return false,
                }
            }
            // :1118-1121.
            _ => 0.0,
        };
        // :1122.
        obstacle_half_width >= f64::from(self.ctrl.compensated_trace_half_width[layer])
    }

    // =============================================================================================
    // shoveTraceRoom (:1130-1201)
    // =============================================================================================

    /// Port of `shoveTraceRoom(MazeListElement, ObstacleExpansionRoom)`
    /// (MazeSearchEngine.java:1130-1201): "shoves a trace room and expands the corresponding
    /// doors. Return false, if no door was expanded. In this case occupation of the door_section
    /// by ripup can be delayed to allow shoving the room from a different door section."
    ///
    /// It is check-only — see [`MazeTraceShover`]'s module docs. The two halves are symmetric: the
    /// left one is skipped for an element already adjusted `RIGHT` and vice versa, and each turns
    /// its collected [`DoorSection`]s into queue elements whose adjustment is `LEFT`/`RIGHT` for a
    /// 2-dimensional link door and `NONE` otherwise (`:1153-1159`, `:1184-1190`).
    pub fn shove_trace_room(
        &mut self,
        board: &mut Board,
        list_element: &MazeListElement,
        obstacle_room: ObstacleRoomId,
    ) -> bool {
        // :1131-1137. "No delay of occupation necessary because inner sections of a door are
        // currently not shoved."
        // `listElement.door.mazeSearchElementCount()` (`:1132`). `None` is the still-null
        // `sectionArr` Java throws on (ExpansionDoor.java:95-97); `occupyNextElement` has already
        // resolved a section of this door, so it is unreachable here.
        let section_count = i32::try_from(
            self.engine
                .maze_search_element_count(list_element.door)
                .unwrap_or_else(|| {
                    panic!(
                        "MazeSearchEngine.shoveTraceRoom: the element's door has a null sectionArr \
                         — Java throws at MazeSearchEngine.java:1132"
                    )
                }),
        )
        .unwrap_or(i32::MAX)
            - 1;
        if list_element.section_no_of_door != 0 && list_element.section_no_of_door != section_count
        {
            return true;
        }
        // :1138.
        let mut result = false;
        // :1139-1169.
        if list_element.adjustment != MazeAdjustment::Right {
            let mut left_to_door_section_list: Vec<DoorSection> = Vec::new();
            if MazeTraceShover::check_shove_trace_line(
                list_element,
                obstacle_room,
                &mut self.engine.rooms,
                board,
                self.ctrl,
                false,
                &mut left_to_door_section_list,
            ) {
                result = true;
            }
            for current_left_door_section in left_to_door_section_list {
                // :1153-1159. "the door is the link door to the next room".
                let current_adjustment = if self
                    .engine
                    .rooms
                    .door(current_left_door_section.door)
                    .map_or(0, |door| door.dimension)
                    == 2
                {
                    MazeAdjustment::Left
                } else {
                    MazeAdjustment::None
                };
                self.expand_to_door_section(
                    board,
                    ExpandableRef::Door(current_left_door_section.door),
                    current_left_door_section.section_index,
                    Some(&current_left_door_section.section_line),
                    list_element,
                    0,
                    current_adjustment,
                );
            }
        }

        // :1171-1199.
        if list_element.adjustment != MazeAdjustment::Left {
            let mut right_to_door_section_list: Vec<DoorSection> = Vec::new();
            if MazeTraceShover::check_shove_trace_line(
                list_element,
                obstacle_room,
                &mut self.engine.rooms,
                board,
                self.ctrl,
                true,
                &mut right_to_door_section_list,
            ) {
                result = true;
            }
            for current_right_door_section in right_to_door_section_list {
                // :1184-1190.
                let current_adjustment = if self
                    .engine
                    .rooms
                    .door(current_right_door_section.door)
                    .map_or(0, |door| door.dimension)
                    == 2
                {
                    MazeAdjustment::Right
                } else {
                    MazeAdjustment::None
                };
                self.expand_to_door_section(
                    board,
                    ExpandableRef::Door(current_right_door_section.door),
                    current_right_door_section.section_index,
                    Some(&current_right_door_section.section_line),
                    list_element,
                    0,
                    current_adjustment,
                );
            }
        }
        // :1200.
        result
    }

    // =============================================================================================
    // checkNeckDownAtDestPin (:1207-1215)
    // =============================================================================================

    /// Port of `checkNeckDownAtDestPin(CompleteExpansionRoom)` (MazeSearchEngine.java:1207-1215):
    /// "checks, if the next room contains a destination pin, where evtl. neckdown is necessary.
    /// Return the neck down width in this case, or 0, if no such pin was found."
    ///
    /// # Java bug: `MazeSearchEngine.checkNeckDownAtDestPin`
    ///
    /// The name and the javadoc both say *destination* pin; the loop (`:1209-1213`) never calls
    /// `isDestinationDoor()`. It answers the neckdown half width of the **first** target door
    /// whose item is a `Pin` — start pin or destination pin, whichever the room lists first — and
    /// `return`s from inside the loop, so a room whose first pin has no neckdown answers `0`
    /// without looking at the rest. `docs/java-quirks.md` #179, JVM-pinned by `P6T12Probe` mode
    /// `neck`: room 2's target doors are `(item 2 = the start pin, item 3 = the destination pin)`
    /// and the answer is the *start* pin's `49.0`.
    pub fn check_neck_down_at_dest_pin(&self, board: &Board, room: RoomRef) -> f64 {
        // :1208.
        let target_doors = self.engine.rooms.room_target_doors(room);
        let ctx = board.ctx();
        // :1209-1213.
        for current_target_door in target_doors {
            let Some(door) = self.engine.rooms.target_door(*current_target_door) else {
                continue;
            };
            if let Some(Item::Pin(pin)) = board.items.get(&door.item) {
                let Some(layer) = self.engine.rooms.room_layer(board, room) else {
                    return 0.0;
                };
                return f64::from(pin.get_trace_neckdown_halfwidth(layer, &ctx));
            }
        }
        // :1214.
        0.0
    }

    // =============================================================================================
    // The two `ExpandableObject` reads this file needs
    // =============================================================================================

    /// `ExpandableObject.getDimension()` (ExpandableObject.java:13) over the four implementors —
    /// `MazeSearchEngine.java:461`'s virtual call.
    fn expandable_dimension(&self, object: ExpandableRef) -> i32 {
        match object {
            ExpandableRef::Door(door) => self
                .engine
                .rooms
                .door(door)
                .map_or(0, |door| door.dimension),
            // TargetItemExpansionDoor.java:39-42 and ExpansionDrill.java:99-102 both answer 2;
            // DrillPage.java's is 2 as well.
            ExpandableRef::TargetDoor(_) | ExpandableRef::Drill(_) | ExpandableRef::Page(_) => 2,
        }
    }

    /// `ExpandableObject.getShape().centreOfGravity()` (`MazeSearchEngine.java:859`) over the four
    /// implementors. `None` is Java's `NullPointerException` on a door whose rooms have no shape.
    fn expandable_shape_centre(&self, object: ExpandableRef) -> Option<FloatPoint> {
        match object {
            ExpandableRef::Door(door) => {
                Some(self.engine.rooms.door_shape(door)?.centre_of_gravity())
            }
            ExpandableRef::TargetDoor(door) => Some(
                self.engine
                    .rooms
                    .target_door(door)?
                    .get_shape()
                    .centre_of_gravity(),
            ),
            ExpandableRef::Drill(drill) => Some(
                self.engine
                    .rooms
                    .drills
                    .get(drill.0)?
                    .get_shape()
                    .centre_of_gravity(),
            ),
            ExpandableRef::Page(page) => Some(
                fr_geometry::TileShape::Box(self.engine.drill_pages().page(page).shape)
                    .centre_of_gravity(),
            ),
        }
    }
}
