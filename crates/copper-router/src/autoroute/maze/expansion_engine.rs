use copper_board::rules::PadstackLookup;
use copper_board::{Board, Item, ItemId, StopCheck};
use copper_geometry::{FloatLine, Point, TileShape};

use crate::arena::{DrillId, PageId};
use crate::autoroute::drill::ExpansionDrill;
use crate::autoroute::expansion::{ExpandableRef, RoomRef};
use crate::autoroute::item_info;
use crate::autoroute::maze::{AutorouteEngine, MazeAdjustment, MazeListElement, MazeSearchEngine};
use crate::board_ext::{CheckDrillResult, ForcedViaInserter};

pub struct MazeExpansionEngine;

impl MazeExpansionEngine {
    pub fn expand_to_drill(
        search: &mut MazeSearchEngine<'_>,
        board: &mut Board,
        drill: DrillId,
        from_element: &MazeListElement,
        add_costs: i32,
    ) {
        let Some(next_room) = from_element.next_room else {
            return;
        };
        let Some(layer) = search.engine.rooms.room_layer(board, next_room) else {
            return;
        };
        let trace_half_width = search.ctrl.compensated_trace_half_width[layer];
        let Some(room_shape) = search.engine.rooms.room_shape(next_room).cloned() else {
            return;
        };
        let room_shape_is_thin = room_shape.min_width() < 2.0 * f64::from(trace_half_width);

        let Some(drill_shape) = search
            .engine
            .rooms
            .drills
            .get(drill.0)
            .map(|d| d.get_shape().clone())
        else {
            return;
        };
        if room_shape_is_thin {
            let backtrack_intersects = from_element.backtrack_door.is_some_and(|door| {
                Self::expandable_shape(search.engine, door)
                    .is_some_and(|shape| drill_shape.intersects(&shape))
            });
            if !backtrack_intersects {
                return;
            }
        }

        let via_radius = search.ctrl.via_radii[layer];
        let shrinked_drill_shape = drill_shape.shrink(via_radius);
        let mut compare_corner = from_element
            .shape_entry
            .a
            .middle_point(&from_element.shape_entry.b);
        if let (ExpandableRef::Page(_), Some(ExpandableRef::TargetDoor(backtrack))) =
            (from_element.door, from_element.backtrack_door)
        {
            let backtrack_item = search
                .engine
                .rooms
                .target_door(backtrack)
                .map(|door| door.item);
            if let Some(backtrack_item) = backtrack_item {
                let drill_location = search
                    .engine
                    .rooms
                    .drills
                    .get(drill.0)
                    .map(|d| d.location.clone());
                if let (Some(drill_location), Some(Item::Pin(pin))) =
                    (drill_location, board.items.get(&backtrack_item))
                {
                    let ctx = board.ctx();
                    if let Some(nearest_exit_corner) = pin.nearest_trace_exit_corner(
                        &drill_location.to_float(),
                        trace_half_width,
                        layer,
                        &ctx,
                    ) {
                        compare_corner = nearest_exit_corner;
                    }
                }
            }
        }
        let Some(nearest_point) = shrinked_drill_shape.nearest_point_approx(&compare_corner) else {
            return;
        };
        let shape_entry = FloatLine::new(nearest_point, nearest_point);
        let Some(drill_first_layer) = search
            .engine
            .rooms
            .drills
            .get(drill.0)
            .map(|d| d.first_layer)
        else {
            return;
        };
        let section_index = i32::try_from(layer).unwrap_or(i32::MAX)
            - i32::try_from(drill_first_layer).unwrap_or(i32::MAX);
        let mut expansion_value = from_element.expansion_value
            + f64::from(add_costs)
            + nearest_point.weighted_distance(
                &compare_corner,
                search.ctrl.trace_costs[layer].horizontal,
                search.ctrl.trace_costs[layer].vertical,
            );
        let (new_backtrack_door, new_section_no_of_backtrack_door) =
            if matches!(from_element.door, ExpandableRef::Page(_)) {
                (
                    from_element.backtrack_door,
                    from_element.section_no_of_backtrack_door,
                )
            } else {
                expansion_value += search.ctrl.min_normal_via_cost;
                (Some(from_element.door), from_element.section_no_of_door)
            };
        let sorting_value = expansion_value
            + search
                .destination_distance
                .calculate_from_point(&nearest_point, layer);
        let new_element = MazeListElement {
            door: ExpandableRef::Drill(drill),
            section_no_of_door: section_index,
            backtrack_door: new_backtrack_door,
            section_no_of_backtrack_door: new_section_no_of_backtrack_door,
            expansion_value,
            sorting_value,
            next_room: None,
            shape_entry,
            room_ripped: from_element.room_ripped,
            adjustment: MazeAdjustment::None,
            already_checked: false,
            ripup_cost: 0,
        };
        search.push(new_element, board);
    }

    pub fn expand_to_drill_page(
        search: &mut MazeSearchEngine<'_>,
        board: &mut Board,
        drill_page: PageId,
        from_element: &MazeListElement,
    ) {
        let Some(next_room) = from_element.next_room else {
            return;
        };
        let Some(layer) = search.engine.rooms.room_layer(board, next_room) else {
            return;
        };
        let from_element_shape_entry_middle = from_element
            .shape_entry
            .a
            .middle_point(&from_element.shape_entry.b);
        let nearest_point = search
            .engine
            .drill_pages()
            .page(drill_page)
            .shape
            .nearest_point(&from_element_shape_entry_middle);
        let expansion_value = from_element.expansion_value + search.ctrl.min_normal_via_cost;
        let sorting_value = expansion_value
            + nearest_point.weighted_distance(
                &from_element_shape_entry_middle,
                search.ctrl.trace_costs[layer].horizontal,
                search.ctrl.trace_costs[layer].vertical,
            )
            + search
                .destination_distance
                .calculate_from_point(&nearest_point, layer);
        let new_element = MazeListElement {
            door: ExpandableRef::Page(drill_page),
            section_no_of_door: i32::try_from(layer).unwrap_or(i32::MAX),
            backtrack_door: Some(from_element.door),
            section_no_of_backtrack_door: from_element.section_no_of_door,
            expansion_value,
            sorting_value,
            next_room: from_element.next_room,
            shape_entry: from_element.shape_entry,
            room_ripped: from_element.room_ripped,
            adjustment: MazeAdjustment::None,
            already_checked: false,
            ripup_cost: 0,
        };
        search.push(new_element, board);
    }

    pub fn expand_to_drills_of_page(
        search: &mut MazeSearchEngine<'_>,
        board: &mut Board,
        from_element: &MazeListElement,
        stop: StopCheck<'_>,
    ) {
        let from_room_layer = from_element.section_no_of_door;
        let ExpandableRef::Page(drill_page) = from_element.door else {
            return;
        };
        let attach_smd_allowed = search.ctrl.attach_smd_allowed();
        let drill_list =
            search
                .engine
                .drill_page_drills(board, drill_page, attach_smd_allowed, stop);

        for current_drill in drill_list.iter().copied() {
            let Some(drill) = search.engine.rooms.drills.get(current_drill.0) else {
                continue;
            };
            let section_index =
                from_room_layer - i32::try_from(drill.first_layer).unwrap_or(i32::MAX);
            let Ok(section_index_usize) = usize::try_from(section_index) else {
                continue;
            };
            if section_index_usize >= drill.rooms.len() {
                continue;
            }
            if drill.rooms[section_index_usize] != from_element.next_room {
                continue;
            }
            if drill
                .get_maze_search_element(section_index_usize)
                .is_occupied
            {
                continue;
            }
            Self::expand_to_drill(search, board, current_drill, from_element, 0);
        }
    }

    pub fn expand_to_other_layers(
        search: &mut MazeSearchEngine<'_>,
        board: &mut Board,
        list_element: &MazeListElement,
    ) {
        #[allow(unused_assignments)]
        let mut via_lower_bound = 0i32;
        #[allow(unused_assignments)]
        let mut via_upper_bound = -1i32;
        let ExpandableRef::Drill(current_drill) = list_element.door else {
            return;
        };
        let Some((drill_first_layer, drill_last_layer, drill_rooms, drill_location)) =
            search.engine.rooms.drills.get(current_drill.0).map(|d| {
                (
                    i32::try_from(d.first_layer).unwrap_or(i32::MAX),
                    i32::try_from(d.last_layer).unwrap_or(i32::MAX),
                    d.rooms.clone(),
                    d.location.clone(),
                )
            })
        else {
            return;
        };
        let from_layer = drill_first_layer + list_element.section_no_of_door;
        let mut smd_attached_on_component_side = false;
        let mut smd_attached_on_solder_side = false;
        let room_ripped;
        let from_room = usize::try_from(list_element.section_no_of_door)
            .ok()
            .and_then(|index| drill_rooms.get(index).copied().flatten());
        if let Some(RoomRef::Obstacle(room)) = from_room {
            if !search.ctrl.ripup_allowed {
                return;
            }
            let Some(obstacle_item) = search
                .engine
                .rooms
                .obstacle_room(room)
                .map(crate::autoroute::expansion::ObstacleExpansionRoom::get_item)
            else {
                return;
            };
            let Some(Item::Via(via)) = board.items.get(&obstacle_item) else {
                return;
            };
            let obstacle_padstack = via.get_padstack_id();
            let obstacle_clearance_class = via.hdr.clearance_class();
            let via_rule_contains = search
                .ctrl
                .via_rule
                .as_ref()
                .is_some_and(|rule| rule.contains_padstack(obstacle_padstack));
            if !via_rule_contains || obstacle_clearance_class != search.ctrl.via_clearance_class {
                return;
            }
            via_lower_bound = board
                .library
                .padstacks
                .padstack_from_layer(obstacle_padstack);
            via_upper_bound = board.library.padstacks.padstack_to_layer(obstacle_padstack);
            room_ripped = true;
        } else {
            let net_numbers = [search.ctrl.net_number];
            room_ripped = false;
            let via_lower_limit = drill_first_layer
                .max(i32::try_from(search.ctrl.via_lower_bound).unwrap_or(i32::MAX));
            let via_upper_limit = drill_last_layer
                .min(i32::try_from(search.ctrl.via_upper_bound).unwrap_or(i32::MAX));
            let mut current_layer = from_layer;
            loop {
                let Some(current_room_shape) =
                    Self::drill_room_shape(search, &drill_rooms, current_layer - drill_first_layer)
                else {
                    return;
                };
                let Ok(layer) = usize::try_from(current_layer) else {
                    return;
                };
                let drill_result = Self::check_layer_with_any_matching_via(
                    search,
                    board,
                    &current_room_shape,
                    &drill_location,
                    layer,
                    &net_numbers,
                );
                if drill_result == CheckDrillResult::NotDrillable {
                    via_lower_bound = current_layer + 1;
                    break;
                } else if drill_result == CheckDrillResult::DrillableWithAttachSmd {
                    if current_layer == 0 {
                        smd_attached_on_component_side = true;
                    } else if current_layer
                        == i32::try_from(search.ctrl.layer_count).unwrap_or(i32::MAX) - 1
                    {
                        smd_attached_on_solder_side = true;
                    }
                }
                if current_layer <= via_lower_limit {
                    via_lower_bound = via_lower_limit;
                    break;
                }
                current_layer -= 1;
            }
            if via_lower_bound > drill_first_layer {
                return;
            }
            current_layer = from_layer + 1;
            loop {
                if current_layer > via_upper_limit {
                    via_upper_bound = via_upper_limit;
                    break;
                }
                let Some(current_room_shape) =
                    Self::drill_room_shape(search, &drill_rooms, current_layer - drill_first_layer)
                else {
                    return;
                };
                let Ok(layer) = usize::try_from(current_layer) else {
                    return;
                };
                let drill_result = Self::check_layer_with_any_matching_via(
                    search,
                    board,
                    &current_room_shape,
                    &drill_location,
                    layer,
                    &net_numbers,
                );
                if drill_result == CheckDrillResult::NotDrillable {
                    via_upper_bound = current_layer - 1;
                    break;
                } else if drill_result == CheckDrillResult::DrillableWithAttachSmd
                    && current_layer
                        == i32::try_from(search.ctrl.layer_count).unwrap_or(i32::MAX) - 1
                {
                    smd_attached_on_solder_side = true;
                }
                current_layer += 1;
            }
            if via_upper_bound < drill_last_layer {
                return;
            }
        }

        let mut to_layer = via_lower_bound;
        while to_layer <= via_upper_bound {
            let this_layer = to_layer;
            to_layer += 1;
            if this_layer == from_layer {
                continue;
            }
            let (current_first_layer, current_last_layer) = if this_layer < from_layer {
                (this_layer, from_layer)
            } else {
                (from_layer, this_layer)
            };
            let mut mask_found = false;
            for current_via_info in &search.ctrl.via_infos {
                if current_first_layer >= current_via_info.from_layer
                    && current_last_layer <= current_via_info.to_layer
                    && current_via_info.from_layer >= via_lower_bound
                    && current_via_info.to_layer <= via_upper_bound
                {
                    let mask_ok = !(current_via_info.from_layer == 0
                        && smd_attached_on_component_side
                        || current_via_info.to_layer
                            == i32::try_from(search.ctrl.layer_count).unwrap_or(i32::MAX) - 1
                            && smd_attached_on_solder_side)
                        || current_via_info.attach_smd_allowed;
                    if mask_ok {
                        mask_found = true;
                        break;
                    }
                }
            }
            if !mask_found {
                continue;
            }
            let current_room_index = this_layer - drill_first_layer;
            let Some(current_drill_layer_info) = search
                .engine
                .maze_search_element(ExpandableRef::Drill(current_drill), current_room_index)
            else {
                return;
            };
            if current_drill_layer_info.is_occupied {
                continue;
            }
            let (Ok(from_layer_index), Ok(to_layer_index)) =
                (usize::try_from(from_layer), usize::try_from(this_layer))
            else {
                return;
            };
            let expansion_value = list_element.expansion_value
                + f64::from(search.ctrl.add_via_costs[from_layer_index][to_layer_index]);
            let shape_entry_middle = list_element
                .shape_entry
                .a
                .middle_point(&list_element.shape_entry.b);
            let sorting_value = expansion_value
                + search
                    .destination_distance
                    .calculate_from_point(&shape_entry_middle, to_layer_index);
            let Ok(room_index) = usize::try_from(current_room_index) else {
                return;
            };
            let new_element = MazeListElement {
                door: ExpandableRef::Drill(current_drill),
                section_no_of_door: current_room_index,
                backtrack_door: Some(ExpandableRef::Drill(current_drill)),
                section_no_of_backtrack_door: list_element.section_no_of_door,
                expansion_value,
                sorting_value,
                next_room: drill_rooms.get(room_index).copied().flatten(),
                shape_entry: list_element.shape_entry,
                room_ripped,
                adjustment: MazeAdjustment::None,
                already_checked: false,
                ripup_cost: 0,
            };
            search.push(new_element, board);
        }
    }

    pub fn check_layer_with_any_matching_via(
        search: &mut MazeSearchEngine<'_>,
        board: &mut Board,
        room_shape: &TileShape,
        location: &Point,
        layer: usize,
        net_numbers: &[i32],
    ) -> CheckDrillResult {
        let mut drillable_with_attach_smd = false;
        let Some(via_rule) = search.ctrl.via_rule.as_ref() else {
            return CheckDrillResult::NotDrillable;
        };
        let vias: Vec<copper_board::ViaInfo> = via_rule.iter().cloned().collect();
        for via_info in &vias {
            let via_padstack = via_info.get_padstack();
            let clearance_class_index = via_info.get_clearance_class_index();
            let attach_smd_allowed = via_info.attach_smd_allowed();
            let from_layer = board.library.padstacks.padstack_from_layer(via_padstack);
            let to_layer = board.library.padstacks.padstack_to_layer(via_padstack);
            let layer_no = i32::try_from(layer).unwrap_or(i32::MAX);
            if layer_no < from_layer || layer_no > to_layer {
                continue;
            }
            let via_radius = board
                .library
                .padstacks
                .padstack_shape_max_width(via_padstack, layer_no)
                .map_or(0.0, |width| 0.5 * width);
            let required_radius = (via_radius).max(f64::from(search.ctrl.trace_half_width[layer]));
            let result = ForcedViaInserter::check_layer(
                board,
                required_radius,
                clearance_class_index,
                attach_smd_allowed,
                room_shape,
                location,
                layer,
                net_numbers,
                search.ctrl.max_shove_trace_recursion_depth,
                0,
                search.ctrl.trace_half_width[layer],
                search.ctrl.trace_clearance_class_index,
            );
            if result == CheckDrillResult::Drillable {
                return result;
            }
            if result == CheckDrillResult::DrillableWithAttachSmd {
                drillable_with_attach_smd = true;
            }
        }
        if drillable_with_attach_smd {
            CheckDrillResult::DrillableWithAttachSmd
        } else {
            CheckDrillResult::NotDrillable
        }
    }

    fn expandable_shape(engine: &AutorouteEngine, object: ExpandableRef) -> Option<TileShape> {
        engine.expandable_shape(object)
    }

    fn drill_room_shape(
        search: &MazeSearchEngine<'_>,
        drill_rooms: &[Option<RoomRef>],
        index: i32,
    ) -> Option<TileShape> {
        let index = usize::try_from(index).ok()?;
        let room = (*drill_rooms.get(index)?)?;
        search.engine.rooms.room_shape(room).cloned()
    }
}

pub fn via_autoroute_drill_info(
    engine: &mut AutorouteEngine,
    board: &mut Board,
    via: ItemId,
) -> Option<DrillId> {
    if let Some(existing) = board
        .get_item_mut(via)?
        .get_autoroute_info()
        .autoroute_drill_info
    {
        return Some(existing);
    }
    let ctx = board.ctx();
    let Some(Item::Via(via_item)) = board.items.get(&via) else {
        return None;
    };
    let centre = via_item.get_center();
    let first_layer = via_item.first_layer(&ctx);
    let last_layer = via_item.last_layer(&ctx);
    let current_drill_shape = TileShape::Box(TileShape::get_instance_from_point(&centre));
    let mut new_drill = ExpansionDrill::new(current_drill_shape, centre, first_layer, last_layer);
    let tree = engine.tree;
    let via_layer_count = last_layer - first_layer + 1;
    for i in 0..via_layer_count {
        let room = item_info::get_expansion_room(board, via, i, tree, |b, item, index, tree| {
            engine.rooms.new_obstacle_room(b, item, index, tree)
        });
        new_drill.rooms[i] = room.map(RoomRef::Obstacle);
    }
    let id = DrillId(engine.rooms.drills.insert(new_drill));
    board
        .get_item_mut(via)?
        .get_autoroute_info()
        .autoroute_drill_info = Some(id);
    Some(id)
}
