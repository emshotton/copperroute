//! Port of `autoroute.maze.MazeTraceShover` (MazeTraceShover.java:24-357) — "auxiliary functions
//! used in MazeSearchEngine".
//!
//! # It is check-only
//!
//! Despite the name, nothing here shoves anything. `checkShoveTraceLine` builds a candidate shove
//! line segment out of the obstacle trace and the door the search arrived through, asks
//! [`RoutingBoard.checkTraceSegment`](fr_board::Board::check_trace_segment_of_line_segment) and
//! [`TraceShover::check_segment`] whether that shove would succeed, and then *collects* the door
//! sections a successful shove would open — into the caller's list. The board is never written:
//! its two queries are the `check*` half of the shove chain, not the `insert*`/`shove*` half.
//! `MazeSearchEngine.shoveTraceRoom` (`:1130-1205`) is the only caller, and it turns the collected
//! sections into queue elements.
//!
//! # The return value is not "did it shove"
//!
//! `false` means "the algorithm did not succeed and trying to shove from another door section may
//! be more successful" (`:29-31`), which `shoveTraceRoom` translates into *delaying* the
//! occupation of the door section. `true` covers both "the shove is possible" and "there is
//! nothing here to shove", so a `true` with an empty `toDoorList` is the common answer.
//!
//! # Hazard N: the stale trace index
//!
//! `ObstacleExpansionRoom` snapshots its shape in its constructor
//! (ObstacleExpansionRoom.java:26-30) but keeps its `indexInItem` live, and a trace's polyline can
//! be replaced underneath it while the maze search runs (pull-tight, shoving, splitting). `:65-66`
//! therefore range-checks the index against the trace's *current* polyline and answers `false`
//! with no warning and no throw. It is transcribed verbatim — see
//! [`MazeTraceShover::check_shove_trace_line`].

use fr_board::{Board, Item, ItemId, ObstacleRoomId};
use fr_geometry::polyline_shape::PolylineShapeOps;
use fr_geometry::{FloatLine, Line, LineSegment, Side};

use crate::arena::DoorId;
use crate::autoroute::expansion::{ExpandableRef, ExpansionRoomStore, RoomRef};
use crate::autoroute::maze::{AutorouteControl, MazeListElement};
use crate::board_ext::TraceShover;

/// Port of the nested `MazeTraceShover.DoorSection` (MazeTraceShover.java:345-356): one door
/// section the shover found on the shove side of the room.
///
/// Java's three fields are package-private and `MazeSearchEngine.shoveTraceRoom` reads them
/// directly (`:1154`, `:1162-1164`), so they are `pub` here.
#[derive(Debug, Clone, PartialEq)]
pub struct DoorSection {
    /// `final ExpansionDoor door` (`:347`).
    pub door: DoorId,
    /// `final int sectionIndex` (`:348`).
    pub section_index: i32,
    /// `final FloatLine sectionLine` (`:349`).
    pub section_line: FloatLine,
}

impl DoorSection {
    /// Port of the constructor (`:351-355`).
    pub fn new(door: DoorId, section_index: i32, section_line: FloatLine) -> DoorSection {
        DoorSection {
            door,
            section_index,
            section_line,
        }
    }
}

/// Port of the final class `MazeTraceShover` (MazeTraceShover.java:24-357), whose constructor is
/// private (`:26`) — a namespace for two static methods, like [`TraceShover`].
pub struct MazeTraceShover;

impl MazeTraceShover {
    /// Port of `checkShoveTraceLine(MazeListElement, ObstacleExpansionRoom, RoutingBoard,
    /// AutorouteControl, boolean, Collection<DoorSection>)` (MazeTraceShover.java:32-314):
    /// "returns false, if the algorithm did not succeed and trying to shove from another door
    /// section may be more successful."
    ///
    /// `rooms` is `&mut` because `getSectionSegments` (`:254`, `:294`) allocates the door's
    /// section array (ExpansionDoor.java:141) — the two calls Task 11's review S2 names.
    ///
    /// The two `FRLogger.trace` payloads (`:263`) are dropped; their `continue` is not.
    #[allow(clippy::too_many_arguments)]
    pub fn check_shove_trace_line(
        list_element: &MazeListElement,
        obstacle_room: ObstacleRoomId,
        rooms: &mut ExpansionRoomStore,
        board: &mut Board,
        ctrl: &AutorouteControl,
        shove_to_the_left: bool,
        to_door_list: &mut Vec<DoorSection>,
    ) -> bool {
        let obstacle_room_ref = RoomRef::Obstacle(obstacle_room);
        // :39-41. Only an `ExpansionDoor` can be shoved from; a target door, a drill or a page
        // answers "nothing to do here".
        let ExpandableRef::Door(from_door) = list_element.door else {
            return true;
        };
        // :42-44.
        let Some(room) = rooms.obstacle_room(obstacle_room) else {
            // A stale room id, where Java holds a live object. `true` is `:43`'s answer for an
            // item that is not a trace, which is the same "nothing to shove" outcome.
            return true;
        };
        let obstacle_trace_id = room.get_item();
        let trace_layer = match room.get_layer(board) {
            // :45.
            Some(layer) => layer,
            None => return true,
        };
        let Some(Item::Trace(obstacle_trace)) = board.items.get(&obstacle_trace_id) else {
            return true;
        };
        // :46-51. "only traces with the same halfwidth and the same clearance class can be
        // shoved."
        if obstacle_trace.get_half_width() != ctrl.trace_half_width[trace_layer]
            || obstacle_trace.hdr.clearance_class() != ctrl.trace_clearance_class_index
        {
            return true;
        }
        // :52.
        let compensated_trace_half_width =
            f64::from(ctrl.compensated_trace_half_width[trace_layer]);
        // :53-56.
        let Some(from_door_shape) = rooms.door_shape(from_door) else {
            // Java would NPE on a room with no shape; there is nothing to shove from either way.
            return true;
        };
        if from_door_shape.max_width() < 2.0 * compensated_trace_half_width {
            return true;
        }
        // :57.
        let trace_corner_no = rooms
            .obstacle_room(obstacle_room)
            .expect("just read")
            .get_index_in_item();

        // :59.
        let trace_polyline = match board.items.get(&obstacle_trace_id) {
            Some(Item::Trace(trace)) => trace.polyline().clone(),
            _ => return true,
        };

        // :61-67. **Hazard N**, transcribed verbatim: "check if traceCornerNo allows access to
        // indices up to traceCornerNo + 2 … Stale indices can occur when traces are modified
        // during routing (pull-tight, shoving, etc.)". `traceCornerNo` is a `usize` here, so
        // Java's `< 0` arm is unrepresentable and only the upper bound survives; `lines.length`
        // is at least 3 for every constructed trace (PolylineTrace.java:56-57), so the
        // subtraction cannot wrap.
        let line_count = trace_polyline.lines().len();
        if line_count < 2 || trace_corner_no >= line_count - 2 {
            return false;
        }
        // :68. "The side of the trace line seen from the doors to expand. Used to determine, if a
        // door is on the right side to put it into the doorList."
        let room_doors = rooms.room_doors(obstacle_room_ref).to_vec();

        // :72-172.
        let mut shove_line_segment: LineSegment;
        let from_door_dimension = rooms
            .door(from_door)
            .expect("the from door of a live list element")
            .dimension;
        if from_door_dimension == 2 {
            // :73-96. "shove from a link door into the direction of the other link door."
            let Some(RoomRef::Obstacle(other)) = rooms
                .door(from_door)
                .expect("just read")
                .other_complete_room(obstacle_room_ref)
            else {
                // :76-78 — `otherRoom` is not an `ObstacleExpansionRoom` (or the door does not
                // touch this room at all, which is Java's `null`). Java's `:75` binds a
                // `CompleteExpansionRoom`, so it is the narrowing overload
                // (ExpansionDoor.java:78-92) that runs here, not the `ExpansionRoom` one.
                return false;
            };
            let other_item = rooms
                .obstacle_room(other)
                .map(super::super::expansion::ObstacleExpansionRoom::get_item);
            let Some(other_item) = other_item else {
                return false;
            };
            // :79-81.
            if !end_points_matching(board, obstacle_trace_id, other_item) {
                return false;
            }
            // :82-84.
            let door_center = from_door_shape.centre_of_gravity();
            let (Some(corner1), Some(corner2)) = (
                trace_polyline.corner_approx(trace_corner_no),
                trace_polyline.corner_approx(trace_corner_no + 1),
            ) else {
                return false;
            };
            // :85-88. "shoveLineSegment may be reduced to a point".
            if corner1.distance_square(&corner2) < 1.0 {
                return false;
            }
            // :89-91.
            let shove_into_direction_of_trace_start =
                door_center.distance_square(&corner2) < door_center.distance_square(&corner1);
            let Some(segment) = LineSegment::from_polyline(&trace_polyline, trace_corner_no + 1)
            else {
                return false;
            };
            shove_line_segment = segment;
            if shove_into_direction_of_trace_start {
                // :92-96. "shove from the endpoint to the start point of the line segment".
                shove_line_segment = shove_line_segment.opposite();
            }
        } else {
            // :98-99.
            let Some(from_room) = rooms
                .door(from_door)
                .expect("just read")
                .other_complete_room(obstacle_room_ref)
            else {
                // Java's `fromRoom.getShape()` would NPE.
                return false;
            };
            let Some(from_room_shape) = rooms.room_shape(from_room) else {
                return false;
            };
            let from_point = from_room_shape.centre_of_gravity();
            // :100.
            let shove_trace_line = trace_polyline.lines()[trace_corner_no + 1];
            // :101.
            let Some(door_line_segment) = from_door_shape.diagonal_corner_segment() else {
                return false;
            };
            // :102.
            let side_of_trace_line = shove_trace_line.side_of_float(&door_line_segment.a, 0.0);
            // :104.
            let Some(polar_line_segment) = from_door_shape.polar_line_segment(&from_point) else {
                return false;
            };
            // :106-108.
            let door_line_swapped = polar_line_segment.b.distance_square(&door_line_segment.a)
                < polar_line_segment.a.distance_square(&door_line_segment.a);

            // :110-116. "shove only from the right most section to the right or from the left
            // most section to the left."
            let shape_entry_check_distance = compensated_trace_half_width + 5.0;
            let check_dist_square = shape_entry_check_distance * shape_entry_check_distance;

            // :118-130. `listElement.door.mazeSearchElementCount()` is the *section* count of the
            // door the search arrived through, which is `fromDoor` here.
            let section_ok = if shove_to_the_left && !door_line_swapped
                || !shove_to_the_left && door_line_swapped
            {
                // `listElement.door.mazeSearchElementCount()` (`:120`). `None` is the still-null
                // `sectionArr` Java throws a `NullPointerException` on
                // (ExpansionDoor.java:95-97); it is unreachable from `shoveTraceRoom`, whose
                // caller has already resolved a section of this door.
                let last_section = i32::try_from(
                    rooms
                        .door(from_door)
                        .expect("just read")
                        .maze_search_element_count()
                        .unwrap_or_else(|| {
                            panic!(
                                "MazeTraceShover.checkShoveTraceLine: the from door's sectionArr \
                                 is still null — Java throws at MazeTraceShover.java:120"
                            )
                        }),
                )
                .unwrap_or(i32::MAX)
                    - 1;
                list_element.section_no_of_door == last_section
                    && (list_element
                        .shape_entry
                        .a
                        .distance_square(&door_line_segment.b)
                        <= check_dist_square
                        || list_element
                            .shape_entry
                            .b
                            .distance_square(&door_line_segment.b)
                            <= check_dist_square)
            } else {
                list_element.section_no_of_door == 0
                    && (list_element
                        .shape_entry
                        .a
                        .distance_square(&door_line_segment.a)
                        <= check_dist_square
                        || list_element
                            .shape_entry
                            .b
                            .distance_square(&door_line_segment.a)
                            <= check_dist_square)
            };
            // :131-133.
            if !section_ok {
                return false;
            }

            // :135-171. "create the line segment for shoving".
            let shrinked_line_segment =
                polar_line_segment.shrink_segment(compensated_trace_half_width);
            let perpendicular_direction = shove_trace_line.direction().turn_45_degree(2);
            let (closing_point, middle, end) = if side_of_trace_line == Side::OnTheLeft {
                if shove_to_the_left {
                    // :140-146.
                    (
                        shrinked_line_segment.b.round(),
                        trace_polyline.lines()[trace_corner_no + 1],
                        trace_polyline.lines()[trace_corner_no + 2],
                    )
                } else {
                    // :147-154.
                    (
                        shrinked_line_segment.a.round(),
                        trace_polyline.lines()[trace_corner_no + 1].opposite(),
                        trace_polyline.lines()[trace_corner_no].opposite(),
                    )
                }
            } else if shove_to_the_left {
                // :156-162.
                (
                    shrinked_line_segment.b.round(),
                    trace_polyline.lines()[trace_corner_no + 1].opposite(),
                    trace_polyline.lines()[trace_corner_no].opposite(),
                )
            } else {
                // :163-169.
                (
                    shrinked_line_segment.a.round(),
                    trace_polyline.lines()[trace_corner_no + 1],
                    trace_polyline.lines()[trace_corner_no + 2],
                )
            };
            let start_closing_line = Line::from_direction(closing_point, &perpendicular_direction);
            shove_line_segment = LineSegment::new(start_closing_line, middle, end);
        }

        // :173-175.
        let trace_half_width = ctrl.trace_half_width[trace_layer];
        let net_numbers = [ctrl.net_number];

        // :177-184.
        let mut shove_width = board.check_trace_segment_of_line_segment(
            &shove_line_segment,
            trace_layer,
            &net_numbers,
            trace_half_width,
            ctrl.trace_clearance_class_index,
            true,
        );
        // :185-194.
        let mut segment_shortened = false;
        if shove_width < f64::from(i32::MAX) {
            // "shorten shoveLineSegment"
            shove_width -= 1.0;
            if shove_width <= 0.0 {
                return true;
            }
            shove_line_segment = shove_line_segment.change_length_approx(shove_width);
            segment_shortened = true;
        }

        // :196-198.
        let from_corner = shove_line_segment.start_point_approx();
        let to_corner = shove_line_segment.end_point_approx();
        let segment_ist_point = from_corner.distance_square(&to_corner) < 0.1;

        // :200-216.
        if !segment_ist_point {
            shove_width = TraceShover::check_segment(
                board,
                &shove_line_segment,
                shove_to_the_left,
                trace_layer,
                &net_numbers,
                trace_half_width,
                ctrl.trace_clearance_class_index,
                ctrl.max_shove_trace_recursion_depth,
                ctrl.max_shove_via_recursion_depth,
            );
            if shove_width <= 0.0 {
                return true;
            }
        }

        // :218-221. "Put the doors on this side of the room into toDoorList".
        if segment_shortened {
            shove_width = fr_geometry::java_min(shove_width, to_corner.distance(&from_corner));
        }

        // :223.
        let shove_line = shove_line_segment.get_line();

        // :225-234. "From_door_compare_distance is used to check, that a door is between fromDoor
        // and the end point of the shove line."
        let from_door_compare_distance = if from_door_dimension == 2 || segment_ist_point {
            f64::MAX
        } else {
            match from_door_shape.corner_approx(0) {
                Some(corner) => to_corner.distance_square(&corner),
                // Java would NPE on an empty shape; `:54` has already required a wide one.
                None => f64::MAX,
            }
        };

        // :236-312. The door-section collector.
        //
        // obligation: `MazeTraceShover.checkShoveTraceLine`'s door-section collector (`:236-312`)
        // — **discharged in Task 17**. Task 12 had no ground truth for it: every one of
        // `P6T12Probe` mode `shove`'s 26 cells reported `sections=0`, because `TraceShover.check`
        // answers `shoveWidth <= 0` on that board and `:213-215` returns before this loop is
        // reached — so the trace-fork skip (`:240-248`), both `sideOf` tests (`:266-278`), the
        // nearest-corner choice (`:279-286`), the `fromDoorCompareDistance` skip (`:287-290`),
        // the projection test (`:293-306`) and both `DoorSection` constructions (`:255`, `:307`)
        // were transcription only, and with the two lists always empty
        // `MazeSearchEngine.shoveTraceRoom`'s adjustment mapping (`:1153-1159`, `:1184-1190`)
        // never ran either.
        //
        // Task 17's acceptance corpus fills both lists. Instrumented over
        // `tests/reference/router-fixtures.txt`, `:255` constructs a `DoorSection` 15 times on
        // `router-rpi-splitter`, 498 on `router-j2-reference` and 27 083 on
        // `router-dac2020-bm01`, and `:307` constructs one 12 / 819 / 52 450 times on the same
        // three. `MazeAdjustment::Left`/`Right` therefore *are* produced — which the
        // `roomWasShoved` marker in `ripup_resolver.rs` confirms downstream, entering its branch
        // 5 / 214 / 12 798 times — and every connection of all three boards matches the HEAD jar
        // byte for byte, so `shoveTraceRoom`'s two halves (`:1139`, `:1171`) and
        // `expandToDoorSection`'s `roomRipped` (`:885-887`) are pinned to Java, not to the port.
        for current_door in room_doors {
            // :237-239.
            if current_door == from_door {
                continue;
            }
            let Some(door) = rooms.door(current_door) else {
                continue;
            };
            let (first_room, second_room, dimension) =
                (door.first_room, door.second_room, door.dimension);
            // :240-248. "there may be topological problems at a trace fork".
            if let (RoomRef::Obstacle(first), RoomRef::Obstacle(second)) = (first_room, second_room)
            {
                let first_item = rooms.obstacle_room(first).map(|r| r.get_item());
                let second_item = rooms.obstacle_room(second).map(|r| r.get_item());
                if first_item != second_item {
                    continue;
                }
            }
            // :249.
            let Some(current_door_shape) = rooms.door_shape(current_door) else {
                continue;
            };
            if dimension == 2 && shove_width >= f64::from(i32::MAX) {
                // :250-256.
                if current_door_shape.contains_float(&to_corner) {
                    let line_sections =
                        rooms.door_section_segments(current_door, compensated_trace_half_width);
                    // Java indexes `lineSections[0]` with no guard; an empty array is its
                    // `ArrayIndexOutOfBoundsException`, which is a door whose shape cannot be
                    // sectioned at all — and `contains` above has already found a point in it.
                    if let Some(first_section) = line_sections.first() {
                        to_door_list.push(DoorSection::new(current_door, 0, *first_section));
                    }
                }
            } else if !segment_ist_point {
                // :257-311. "now currentDoor is 1-dimensional".
                // :260-265. "check, that currentDoor is on the same borderLine as fromDoor."
                let Some(current_door_segment) = current_door_shape.diagonal_corner_segment()
                else {
                    // "MazeTraceShover.check_shove_trace_line: door shape is empty" — the
                    // `FRLogger.trace` is dropped, the `continue` is not.
                    continue;
                };
                // :266-278.
                let start_corner_side = shove_line.side_of_float(&current_door_segment.a, 0.0);
                let end_corner_side = shove_line.side_of_float(&current_door_segment.b, 0.0);
                if shove_to_the_left {
                    if start_corner_side != Side::OnTheLeft || end_corner_side != Side::OnTheLeft {
                        continue;
                    }
                } else if start_corner_side != Side::OnTheRight
                    || end_corner_side != Side::OnTheRight
                {
                    continue;
                }
                // :279-286.
                let Some(current_door_line) = current_door_shape.polar_line_segment(&from_corner)
                else {
                    continue;
                };
                let current_door_nearest_corner =
                    if current_door_line.a.distance_square(&from_corner)
                        <= current_door_line.b.distance_square(&from_corner)
                    {
                        current_door_line.a
                    } else {
                        current_door_line.b
                    };
                // :287-290. "currentDoor is not located into the direction of toCorner."
                if to_corner.distance_square(&current_door_nearest_corner)
                    >= from_door_compare_distance
                {
                    continue;
                }
                // :291.
                let current_door_projection =
                    current_door_nearest_corner.projection_approx(&shove_line);

                // :293-310.
                if current_door_projection.distance(&from_corner) + compensated_trace_half_width
                    <= shove_width
                {
                    let line_sections =
                        rooms.door_section_segments(current_door, compensated_trace_half_width);
                    for (i, current_line_section) in line_sections.iter().enumerate() {
                        let current_section_nearest_corner =
                            if current_line_section.a.distance_square(&from_corner)
                                <= current_line_section.b.distance_square(&from_corner)
                            {
                                current_line_section.a
                            } else {
                                current_line_section.b
                            };
                        let current_section_projection =
                            current_section_nearest_corner.projection_approx(&shove_line);
                        if current_section_projection.distance(&from_corner) <= shove_width {
                            to_door_list.push(DoorSection::new(
                                current_door,
                                i32::try_from(i).unwrap_or(i32::MAX),
                                *current_line_section,
                            ));
                        }
                    }
                }
            }
        }
        // :313.
        true
    }
}

/// Port of the private static `endPointsMatching(PolylineTrace, Item)`
/// (MazeTraceShover.java:320-342): "check if the endpoints of trace and fromItem are matching, so
/// that the shove can continue through a link door."
///
/// `pub(crate)` where Java's is `private static`: `check_shove_trace_line` is its only caller in
/// either language, and the port's test for it goes through that caller.
pub(crate) fn end_points_matching(board: &Board, trace: ItemId, from_item: ItemId) -> bool {
    // :321-323. Java's `==` is reference identity, which is `ItemId` equality here.
    if from_item == trace {
        return true;
    }
    // :324-326.
    let (Some(from), Some(trace_item)) = (board.items.get(&from_item), board.items.get(&trace))
    else {
        return false;
    };
    if !trace_item.shares_net(from) {
        return false;
    }
    let Some(Item::Trace(trace_item)) = board.items.get(&trace) else {
        return false;
    };
    let (first_corner, last_corner) = (trace_item.first_corner(), trace_item.last_corner());
    match board.items.get(&from_item) {
        // :328-332. `fromItem instanceof DrillItem` — a `Pin` or a `Via`.
        Some(Item::Pin(_) | Item::Via(_)) => {
            let Some(from_center) = board.drill_center(from_item) else {
                return false;
            };
            first_corner.as_ref() == Some(&from_center)
                || last_corner.as_ref() == Some(&from_center)
        }
        // :332-338.
        Some(Item::Trace(from_trace)) => {
            let (from_first, from_last) = (from_trace.first_corner(), from_trace.last_corner());
            (first_corner.is_some() && first_corner == from_first)
                || (first_corner.is_some() && first_corner == from_last)
                || (last_corner.is_some() && last_corner == from_first)
                || (last_corner.is_some() && last_corner == from_last)
        }
        // :338-340.
        _ => false,
    }
}
