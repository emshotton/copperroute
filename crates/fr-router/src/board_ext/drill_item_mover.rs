use fr_board::board::ShapeTraceEntries;
use fr_board::datastructures::StopCheck;
use fr_board::items::Item;
use fr_board::prelude::*;
use fr_board::searchtree::ShapeSearchTree;
use fr_board::{BoardError, ItemId, TimeLimit, TreeId};
use fr_geometry::{IntOctagon, IntPoint, Point, ShapeOps, TileShape, Vector};

use crate::board_ext::forced_pad_router::{CheckDrillResult, ForcedPadRouter};

pub struct DrillItemMover;

impl DrillItemMover {
                                                                                pub fn check(
        board: &mut Board,
        drill_item: ItemId,
        vector: &Vector,
        max_recursion_depth: i32,
        max_via_recursion_depth: i32,
        ignore_items: Option<&mut Vec<ItemId>>,
        time_limit: Option<&TimeLimit>,
    ) -> bool {
        if time_limit.is_some_and(TimeLimit::is_exceeded) {
            return false;
        }
        let Some(item) = board.get_item(drill_item) else {
            return false;
        };
        if !item.is_drill_item() {
            return false;
        }
        if item.is_shove_fixed(&board.rules) {
            return false;
        }

        for contact in board.normal_contacts(drill_item) {
            let is_shovable_contact = board
                .get_item(contact)
                .is_some_and(|it| it.is_trace() || matches!(it, Item::ConductionArea(_)));
            if !is_shovable_contact {
                return false;
            }
        }

        let mut owned_ignore_items: Vec<ItemId>;
        let effective_ignore_items: &mut Vec<ItemId> = match ignore_items {
            Some(list) => list,
            None => {
                owned_ignore_items = Vec::new();
                &mut owned_ignore_items
            }
        };
        effective_ignore_items.push(drill_item);

        let item = board.get_item(drill_item).expect("checked above");
        let attach_allowed = match item {
            Item::Via(via) => via.attach_allowed,
            _ => false,
        };
        let net_numbers = item.net_nos().to_vec();
        let clearance_class_index = item.clearance_class();
        let center = drill_item_center(board, drill_item).expect("a drill item has a centre");
        let (first_layer, last_layer) = {
            let ctx = board.ctx();
            (item.first_layer(&ctx), item.last_layer(&ctx))
        };
        let tree = board.trees.get_default_tree().id();
        let orthogonal_mode = board.rules.trace_angle_restriction == AngleRestriction::NinetyDegree;

        for current_layer in first_layer..=last_layer {
            let current_ind = current_layer - first_layer;
            let Some(current_shape) = board.item_tree_shape(drill_item, tree, current_ind) else {
                continue;
            };
            let new_shape = current_shape.translate_by(vector);
            let current_tile_shape = if orthogonal_mode {
                TileShape::Box(new_shape.bounding_box())
            } else {
                match new_shape.bounding_octagon() {
                    Some(octagon) => TileShape::Octagon(octagon),
                    None => continue,
                }
            };
            let from_side = ShapeEntrySide::from_point(&center, &current_tile_shape);
            if ForcedPadRouter::check_forced_pad(
                board,
                &current_tile_shape,
                &from_side,
                current_layer,
                &net_numbers,
                clearance_class_index,
                attach_allowed,
                Some(effective_ignore_items),
                max_recursion_depth,
                max_via_recursion_depth,
                true,
                time_limit,
            ) == CheckDrillResult::NotDrillable
            {
                return false;
            }
        }
        true
    }

                                                                #[allow(clippy::too_many_arguments)]
    pub fn insert(
        board: &mut Board,
        drill_item: ItemId,
        vector: &Vector,
        max_recursion_depth: i32,
        max_via_recursion_depth: i32,
        tidy_region: Option<IntOctagon>,
        stop: StopCheck<'_>,
    ) -> Result<bool, BoardError> {
        let Some(item) = board.get_item(drill_item) else {
            return Ok(false);
        };
        if !item.is_drill_item() {
            return Ok(false);
        }
        if item.is_shove_fixed(&board.rules) {
            return Ok(false);
        }
        let attach_allowed = match item {
            Item::Via(via) => via.attach_allowed,
            _ => false,
        };
        let net_numbers = item.net_nos().to_vec();
        let clearance_class_index = item.clearance_class();
        let ignore_items = vec![drill_item];
        let (first_layer, last_layer) = {
            let ctx = board.ctx();
            (item.first_layer(&ctx), item.last_layer(&ctx))
        };
        let tree = board.trees.get_default_tree().id();
        let orthogonal_mode = board.rules.trace_angle_restriction == AngleRestriction::NinetyDegree;
        let mut tidy_region = tidy_region;

        for current_layer in first_layer..=last_layer {
            let current_ind = current_layer - first_layer;
            let Some(current_shape) = board.item_tree_shape(drill_item, tree, current_ind) else {
                continue;
            };
            let new_shape = current_shape.translate_by(vector);
            let current_tile_shape = if orthogonal_mode {
                TileShape::Box(new_shape.bounding_box())
            } else {
                match new_shape.bounding_octagon() {
                    Some(octagon) => TileShape::Octagon(octagon),
                    None => continue,
                }
            };
            if let Some(region) = tidy_region {
                if let Some(octagon) = current_tile_shape.bounding_octagon() {
                    tidy_region = Some(region.union(&octagon));
                }
            }
            let Some(center) = drill_item_center(board, drill_item) else {
                return Ok(false);
            };
            let from_side = ShapeEntrySide::from_point(&center, &current_tile_shape);
            if !ForcedPadRouter::forced_pad(
                board,
                &current_tile_shape,
                &from_side,
                current_layer,
                &net_numbers,
                clearance_class_index,
                attach_allowed,
                Some(&ignore_items),
                max_recursion_depth,
                max_via_recursion_depth,
                stop,
            )? {
                return Ok(false);
            }
            let current_bounding_box = current_shape.bounding_box();
            for j in 0..4 {
                let corner = current_bounding_box.corner(j).to_float();
                board.join_changed_area(&corner, current_layer);
            }
        }
        board.move_item_by(drill_item, vector)?;
        Ok(true)
    }

                                                                                #[allow(clippy::too_many_arguments)]
    pub fn shove_vias(
        board: &mut Board,
        obstacle_shape: &TileShape,
        from_side: &ShapeEntrySide,
        layer: usize,
        net_numbers: &[i32],
        clearance_class_index: usize,
        ignore_items: Option<&[ItemId]>,
        max_recursion_depth: i32,
        max_via_recursion_depth: i32,
        copper_sharing_allowed: bool,
        stop: StopCheck<'_>,
    ) -> Result<bool, BoardError> {
        let mut shape_entries = ShapeTraceEntries::new(
            obstacle_shape.clone(),
            layer,
            net_numbers.to_vec(),
            clearance_class_index,
            Some(*from_side),
        );
        let obstacles = board.overlapping_items_with_clearance(
            obstacle_shape,
            Some(layer),
            &[],
            clearance_class_index,
        );
        if !shape_entries.store_items(board, &obstacles, false, copper_sharing_allowed) {
            return Ok(true);
        }
        if let Some(ignored) = ignore_items {
            shape_entries
                .shove_via_list
                .retain(|id| !ignored.contains(id));
        }
        if shape_entries.shove_via_list.is_empty() {
            return Ok(true);
        }
        let shape_radius = 0.5 * obstacle_shape.bounding_box().min_width();

        for current_via in shape_entries.shove_via_list.clone() {
            if board
                .get_item(current_via)
                .is_none_or(|item| item.shares_net_no(net_numbers))
            {
                continue;
            }
            if max_via_recursion_depth <= 0 {
                return Ok(true);
            }
            let try_via_centers = Self::try_shove_via_points(
                board,
                obstacle_shape,
                layer,
                current_via,
                clearance_class_index,
                true,
            );
            let Some(current_via_center) = drill_item_center(board, current_via) else {
                return Ok(false);
            };
            let via_max_width = via_shape_max_width(board, current_via, layer);
            let max_dist = 0.5 * via_max_width + shape_radius;
            let max_dist_square = max_dist * max_dist;
            let check_via_center = current_via_center.to_float();

            let mut new_via_center: Option<IntPoint> = None;
            let mut rel_coor: Option<Vector> = None;
            for (i, try_via_center) in try_via_centers.iter().enumerate() {
                if i == 0
                    || check_via_center.distance_square(&try_via_center.to_float())
                        <= max_dist_square
                {
                    let mut local_ignore_items: Vec<ItemId> =
                        ignore_items.map(<[ItemId]>::to_vec).unwrap_or_default();
                    let delta = Point::Int(*try_via_center).difference_by(&current_via_center);
                    rel_coor = Some(delta.clone());
                    let shove_ok = Self::check(
                        board,
                        current_via,
                        &delta,
                        max_recursion_depth,
                        max_via_recursion_depth - 1,
                        Some(&mut local_ignore_items),
                        None,
                    );
                    if shove_ok {
                        new_via_center = Some(*try_via_center);
                        break;
                    }
                }
            }
            if new_via_center.is_none() {
                continue;
            }
            let rel_coor = rel_coor.expect("a candidate that answered `shoveOk` set `relCoor`");
            if !Self::insert(
                board,
                current_via,
                &rel_coor,
                max_recursion_depth,
                max_via_recursion_depth - 1,
                None,
                stop,
            )? {
                return Ok(false);
            }
        }
        Ok(true)
    }

                            pub fn try_shove_via_points(
        board: &mut Board,
        obstacle_shape: &TileShape,
        layer: usize,
        via: ItemId,
        clearance_class_index: usize,
        extended_check: bool,
    ) -> Vec<IntPoint> {
        let tree = board.trees.get_default_tree().id();
        let Some(mut current_via_shape) = tree_shape_on_layer(board, via, tree, layer) else {
            return Vec::new();
        };
        let is_int_octagon = obstacle_shape.is_int_octagon();
        let Some(via_clearance_class) = board.get_item(via).map(Item::clearance_class) else {
            return Vec::new();
        };
        let clearance_value =
            f64::from(board.clearance_value(clearance_class_index, via_clearance_class, layer));
        let orthogonal_mode = board.rules.trace_angle_restriction == AngleRestriction::NinetyDegree;
        let compensation_used = board
            .trees
            .get_default_tree()
            .is_clearance_compensation_used();

        let mut shove_distance;
        if orthogonal_mode || is_int_octagon {
            shove_distance = 0.5 * current_via_shape.bounding_box().max_width();
            if !compensation_used {
                shove_distance += clearance_value;
            }
        } else {
            shove_distance = 0.0;
            if !compensation_used {
                shove_distance += 0.5 * clearance_value;
            }
        }
        shove_distance += 2.0;

        let Some(Point::Int(current_via_center)) = drill_item_center(board, via) else {
            return Vec::new();
        };
        if orthogonal_mode {
            let current_offset_box = obstacle_shape.bounding_box().offset(shove_distance);
            let try_count = if extended_check { 2 } else { 1 };
            current_offset_box.nearest_border_projections(&current_via_center, try_count)
        } else if is_int_octagon {
            let Some(bounding_octagon) = obstacle_shape.bounding_octagon() else {
                return Vec::new();
            };
            let current_offset_octagon = bounding_octagon.enlarge(shove_distance);
            let try_count = if extended_check { 4 } else { 1 };
            current_offset_octagon.nearest_border_projections(&current_via_center, try_count)
        } else {
            let current_offset_shape = obstacle_shape.enlarge(shove_distance);
            if !compensation_used {
                current_via_shape = current_via_shape.enlarge(0.5 * clearance_value);
            }
            let try_count = if extended_check { 4 } else { 1 };
            let shove_deltas = current_offset_shape
                .nearest_relative_outside_locations(&current_via_shape, try_count);
            shove_deltas
                .into_iter()
                .map(|delta| {
                    let current_delta = Point::Int(delta.round()).difference_by(&Point::ZERO);
                    match Point::Int(current_via_center).translate_by(&current_delta) {
                        Point::Int(p) => p,
                        Point::Rational(p) => p.to_float().round(),
                    }
                })
                .collect()
        }
    }
}

pub(crate) fn drill_item_center(board: &Board, id: ItemId) -> Option<Point> {
    let ctx = board.ctx();
    match board.get_item(id)? {
        Item::Via(via) => Some(via.get_center()),
        Item::Pin(pin) => Some(pin.get_center(&ctx)),
        _ => None,
    }
}

pub(crate) fn via_shape_max_width(board: &Board, id: ItemId, layer: usize) -> f64 {
    let ctx = board.ctx();
    match board.get_item(id) {
        Some(Item::Via(via)) => via
            .get_shape_on_layer(layer, &ctx)
            .map_or(0.0, |shape| shape.bounding_box().max_width()),
        Some(Item::Pin(pin)) => pin
            .get_shape_on_layer(layer, &ctx)
            .map_or(0.0, |shape| shape.bounding_box().max_width()),
        _ => 0.0,
    }
}

pub(crate) fn tree_shape_on_layer(
    board: &mut Board,
    id: ItemId,
    tree: TreeId,
    layer: usize,
) -> Option<TileShape> {
    let (from_layer, to_layer) = {
        let ctx = board.ctx();
        let item = board.get_item(id)?;
        (item.first_layer(&ctx), item.last_layer(&ctx))
    };
    if layer < from_layer || layer > to_layer {
        return None;
    }
    board.item_tree_shape(id, tree, layer - from_layer)
}

pub(crate) fn tree_by_id(board: &Board, tree: TreeId) -> &ShapeSearchTree {
    board
        .trees
        .trees()
        .find(|candidate| candidate.id() == tree)
        .unwrap_or_else(|| panic!("board_ext: no search tree with id {tree:?}"))
}

