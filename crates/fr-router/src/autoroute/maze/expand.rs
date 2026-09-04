use fr_board::{Board, Item, ObstacleRoomId};
use fr_geometry::{FloatLine, FloatPoint, Point, Polyline, java_min};

use crate::arena::DoorId;
use crate::autoroute::expansion::{ExpandableRef, RoomRef};
use crate::autoroute::maze::expansion_engine::via_autoroute_drill_info;
use crate::autoroute::maze::queue::p7t14b_maze_ledger;
use crate::autoroute::maze::search::segment_projection;
use crate::autoroute::maze::trace_shover::{DoorSection, MazeTraceShover};
use crate::autoroute::maze::{
    ALREADY_RIPPED_COSTS, MazeAdjustment, MazeExpansionEngine, MazeListElement, MazeRipupResolver,
    MazeSearchEngine, TRACE_WIDTH_TOLERANCE,
};
use crate::board_ext::RoutingBoardExt;

impl MazeSearchEngine<'_> {

                                                    fn p7t14b_room(&self, room: RoomRef) -> String {
        match room {
            RoomRef::Obstacle(id) => self.engine.rooms.obstacle_room(id).map_or_else(
                || "obst?".to_string(),
                |room| format!("obst{}:{}", room.get_item().0, room.get_index_in_item()),
            ),
            _ => "free".to_string(),
        }
    }

    pub fn expand_to_room_doors(
        &mut self,
        board: &mut Board,
        list_element: &MazeListElement,
    ) -> bool {
        let Some(next_room) = list_element.next_room else {
            return true;
        };
        let Some(layer_index) = self.engine.rooms.room_layer(board, next_room) else {
            return true;
        };

        let layer_active = self.ctrl.layer_active[layer_index];
        if !layer_active && board.layer_structure().layers[layer_index].is_signal {
            return true;
        }

        let mut half_width = f64::from(self.ctrl.compensated_trace_half_width[layer_index]);
        let mut current_door_is_small = false;
        if let ExpandableRef::Door(current_door) = list_element.door {
            let mut half_width_add = half_width + f64::from(TRACE_WIDTH_TOLERANCE);
            if self.ctrl.with_neckdown {
                let neck_down_half_width = self.check_neck_down_at_dest_pin(board, next_room);
                if neck_down_half_width > 0.0 {
                    half_width_add = java_min(half_width_add, neck_down_half_width);
                    half_width = half_width_add;
                }
            }
            current_door_is_small = self.door_is_small(board, current_door, 2.0 * half_width_add);
        }

        self.engine.complete_neighbour_rooms(board, next_room);

        let shape_entry_middle = list_element
            .shape_entry
            .a
            .middle_point(&list_element.shape_entry.b);

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

        let mut next_room_is_thick = true;
        if let RoomRef::Obstacle(obstacle_room) = next_room {
            next_room_is_thick = self.room_shape_is_thick(board, obstacle_room);
        } else {
            let Some(next_room_shape) = self.engine.rooms.room_shape(next_room).cloned() else {
                return true;
            };
            if next_room_shape.min_width() < 2.0 * half_width {
                next_room_is_thick = false;
            } else if !list_element.already_checked
                && self.expandable_dimension(list_element.door) == 1
                && !current_door_is_small
            {
                let nearest_points =
                    next_room_shape.nearest_border_points_approx(&shape_entry_middle, 2);
                if nearest_points.len() < 2 {
                    next_room_is_thick = false;
                } else {
                    let current_distance = nearest_points[1].distance(&shape_entry_middle);
                    next_room_is_thick = current_distance > half_width + 1.0;
                }
            }
        }

        if !layer_active && let ExpandableRef::Drill(drill) = list_element.door {
            let drill_location = self
                .engine
                .rooms
                .drills
                .get(drill.0)
                .map(|drill| drill.location.clone());
            if let Some(drill_location) = drill_location {
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

        let mut something_expanded = self.expand_to_target_doors(
            board,
            list_element,
            next_room_is_thick,
            current_door_is_small,
            &shape_entry_middle,
        );

        if !layer_active {
            return true;
        }

        let mut ripup_costs: i32 = 0;

        match next_room {
            RoomRef::Complete(_) if !list_element.already_checked && current_door_is_small => {
                let mut enter_through_small_door = false;
                if next_room_is_thick {
                    enter_through_small_door =
                        MazeRipupResolver::check_leaving_ripped_item(self, board, list_element);
                }
                if !enter_through_small_door {
                    return something_expanded;
                }
            }
            RoomRef::Obstacle(obstacle_room) if !list_element.already_checked => {
                let mut room_rippable = false;
                if self.ctrl.ripup_allowed {
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
                        let shoved = self.shove_trace_room(board, list_element, obstacle_room);
                        if !shoved {
                            if ripup_costs > 0 {
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
                                    ripup_cost: ripup_costs,
                                };
                                self.push(new_element, board);
                            }
                            return something_expanded;
                        }
                    }
                }
                if !room_rippable {
                    return true;
                }
            }
            _ => {}
        }

        let room_doors_snapshot = self.engine.rooms.room_doors(next_room).to_vec();
        if p7t14b_maze_ledger() {
            let mut line = format!(
                "EXPROOM room={} n={}",
                self.p7t14b_room(next_room),
                room_doors_snapshot.len()
            );
            for door in &room_doors_snapshot {
                let dimension = self.engine.rooms.door(*door).map_or(-1, |d| d.dimension);
                let bounds = self.engine.rooms.door_shape(*door).map_or_else(
                    || "null".to_string(),
                    |shape| {
                        let b = shape.bounding_box();
                        format!("{},{},{},{}", b.ll.x, b.ll.y, b.ur.x, b.ur.y)
                    },
                );
                line.push_str(&format!(" [dim={dimension} bb={bounds}]"));
            }
            eprintln!("{line}");
        }

        for to_door in room_doors_snapshot {
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

        if self.ctrl.vias_allowed && !matches!(list_element.door, ExpandableRef::Drill(_)) {
            if (something_expanded || next_room_is_thick)
                && matches!(next_room, RoomRef::Complete(_))
            {
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
                let obstacle_item = self
                    .engine
                    .rooms
                    .obstacle_room(obstacle_room)
                    .map(super::super::expansion::ObstacleExpansionRoom::get_item);
                let is_via = obstacle_item
                    .and_then(|item| board.items.get(&item))
                    .is_some_and(|item| matches!(item, Item::Via(_)));
                if let (Some(current_via), true) = (obstacle_item, is_via) {
                    if let Some(via_drill_info) =
                        via_autoroute_drill_info(self.engine, board, current_via)
                    {
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

        something_expanded
    }


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
        if current_door_is_small {
            let mut enter_through_small_door = false;
            if let ExpandableRef::Door(door) = list_element.door {
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
        let mut result = false;
        let target_doors = self.engine.rooms.room_target_doors(next_room).to_vec();
        for to_door in target_doors {
            if list_element.door == ExpandableRef::TargetDoor(to_door) {
                continue;
            }
            let Some(door) = self.engine.rooms.target_door(to_door) else {
                continue;
            };
            let (item, tree_entry_no) = (door.item, door.tree_entry_no);
            let tree_shape_count = board.item_tree_shape_count(item, self.search_tree);
            crate::autoroute::instrument::record_visit(
                crate::autoroute::instrument::Guard::G1aTreeEntryOutOfRange,
            );
            if tree_entry_no >= tree_shape_count {
                crate::autoroute::instrument::record_guard(
                    crate::autoroute::instrument::Guard::G1aTreeEntryOutOfRange,
                    u64::from(item.0),
                    tree_entry_no,
                    tree_shape_count,
                    tree_shape_count > 0,
                );
                continue;
            }
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
            crate::autoroute::instrument::record_visit(
                crate::autoroute::instrument::Guard::G1bNullConnectionShape,
            );
            let Some(target_shape) = target_shape else {
                crate::autoroute::instrument::record_guard(
                    crate::autoroute::instrument::Guard::G1bNullConnectionShape,
                    u64::from(item.0),
                    tree_entry_no,
                    tree_shape_count,
                    false,
                );
                continue;
            };
            let Some(connection_point) = target_shape.nearest_point_approx(shape_entry_middle)
            else {
                continue;
            };
            if !next_room_is_thick {
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

            let new_shape_entry = FloatLine::new(connection_point, connection_point);

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
        result
    }


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
        let half_width = f64::from(self.ctrl.compensated_trace_half_width[layer]);
        let mut something_expanded = false;
        let line_sections = self.engine.rooms.door_section_segments(to_door, half_width);

        for (i, line_section) in line_sections.iter().enumerate() {
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
                new_shape_entry = *line_section;
                let door = self.engine.rooms.door(to_door);
                let both_free_space = door.is_some_and(|door| {
                    matches!(door.first_room, RoomRef::Complete(_))
                        && matches!(door.second_room, RoomRef::Complete(_))
                });
                let dimension = door.map_or(0, |door| door.dimension);
                if dimension == 1 && line_sections.len() == 1 && both_free_space {
                    let shape_entry_middle = new_shape_entry.a.middle_point(&new_shape_entry.b);
                    let Some(room_shape) = self.engine.rooms.room_shape(next_room).cloned() else {
                        return false;
                    };
                    if room_shape.min_width() < 2.0 * half_width {
                        return false;
                    }
                    let nearest_points =
                        room_shape.nearest_border_points_approx(&shape_entry_middle, 2);
                    if nearest_points.len() < 2
                        || nearest_points[1].distance(&shape_entry_middle) <= half_width + 1.0
                    {
                        return false;
                    }
                }
            } else {
                let dimension = self
                    .engine
                    .rooms
                    .door(to_door)
                    .map_or(0, |door| door.dimension);
                if dimension == 1
                    && i == 0
                    && line_sections[0].b.distance_square(&line_sections[0].a) < 1.0
                {
                    continue;
                }
                let Some(projected) = segment_projection(&list_element.shape_entry, line_section)
                else {
                    continue;
                };
                new_shape_entry = projected;
            }

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
        something_expanded
    }


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
            return false;
        };
        if door_section_occupied {
            return false;
        }
        let Some(from_next_room) = from_element.next_room else {
            return false;
        };
        let next_room = self.engine.expandable_other_room(door, from_next_room);
        let Some(layer) = self.engine.rooms.room_layer(board, from_next_room) else {
            return false;
        };
        let shape_entry_middle = shape_entry.a.middle_point(&shape_entry.b);

        let mut bend_cost_penalty = 0.0;
        if self.ctrl.bend_costs[layer] > 0.0
            && let Some(backtrack_door) = from_element.backtrack_door
        {
            let from_mid = from_element
                .shape_entry
                .a
                .middle_point(&from_element.shape_entry.b);
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
            if sq_len_prev > 0.0
                && sq_len_next > 0.0
                && (cross_product * cross_product) > 0.01 * sq_len_prev * sq_len_next
            {
                bend_cost_penalty = self.ctrl.bend_costs[layer];
            }
        }

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
        let sorting_value = expansion_value
            + self
                .destination_distance
                .calculate_from_point(&shape_entry_middle, layer);
        let room_ripped = add_costs > 0 && adjustment == MazeAdjustment::None
            || from_element.already_checked && from_element.room_ripped;

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
        self.push(new_element, board);
        true
    }


                                                        pub fn room_shape_is_thick(&self, board: &Board, obstacle_room: ObstacleRoomId) -> bool {
        let Some(room) = self.engine.rooms.obstacle_room(obstacle_room) else {
            return false;
        };
        let obstacle_item = room.get_item();
        let Some(layer) = room.get_layer(board) else {
            return false;
        };
        let obstacle_half_width = match board.items.get(&obstacle_item) {
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
            Some(Item::Via(via)) => {
                let ctx = board.ctx();
                match via.get_tree_shape_on_layer(self.search_tree, layer, &ctx) {
                    Some(via_shape) => 0.5 * via_shape.max_width(),
                    None => return false,
                }
            }
            _ => 0.0,
        };
        obstacle_half_width >= f64::from(self.ctrl.compensated_trace_half_width[layer])
    }


                                        pub fn shove_trace_room(
        &mut self,
        board: &mut Board,
        list_element: &MazeListElement,
        obstacle_room: ObstacleRoomId,
    ) -> bool {
        if p7t14b_maze_ledger() {
            eprintln!(
                "SHOVEROOM item={} sec={} cnt={} adj={:?}",
                self.engine
                    .rooms
                    .obstacle_room(obstacle_room)
                    .map_or(0, |room| room.get_item().0),
                list_element.section_no_of_door,
                self.engine
                    .maze_search_element_count(list_element.door)
                    .map_or(-1, |count| i64::try_from(count).unwrap_or(-1)),
                list_element.adjustment
            );
        }
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
        let mut result = false;
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
        result
    }


                                                        pub fn check_neck_down_at_dest_pin(&self, board: &Board, room: RoomRef) -> f64 {
        let target_doors = self.engine.rooms.room_target_doors(room);
        let ctx = board.ctx();
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
        0.0
    }


            fn expandable_dimension(&self, object: ExpandableRef) -> i32 {
        self.engine.expandable_dimension(object)
    }

            fn expandable_shape_centre(&self, object: ExpandableRef) -> Option<FloatPoint> {
        Some(self.engine.expandable_shape(object)?.centre_of_gravity())
    }
}
