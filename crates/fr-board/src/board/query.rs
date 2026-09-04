use std::collections::BTreeSet;

use fr_geometry::{Area, LineSegment, Point, Polyline, ShapeOps, TileShape, Vector};

use crate::datastructures::StopCheck;
use crate::error::BoardError;
use crate::ids::{ItemId, TreeObject};
use crate::items::Item;
use crate::structure::FixedState;

use super::connectivity::StopConnectionOption;
use super::{Board, item_ctx};

impl Board {
    pub fn overlapping_objects(
        &self,
        shape: &TileShape,
        layer: Option<usize>,
    ) -> BTreeSet<TreeObject> {
        let ctx = self.ctx();
        self.trees
            .get_default_tree()
            .overlapping_objects(shape, layer, &[], &self.items, &ctx)
    }

    pub fn overlapping_items(&self, area: &Area, layer: Option<usize>) -> BTreeSet<ItemId> {
        let mut result = BTreeSet::new();
        let tiles = area.split_to_convex().expect(
            "BasicBoard.overlappingItems: area.splitToConvex() is null — Java throws a \
             NullPointerException here too (BasicBoard.java:941)",
        );
        for tile in tiles {
            for object in self.overlapping_objects(&tile, layer) {
                if let TreeObject::Item(id) = object {
                    result.insert(id);
                }
            }
        }
        result
    }

    pub fn overlapping_items_with_clearance(
        &mut self,
        shape: &TileShape,
        layer: Option<usize>,
        ignore_net_nos: &[i32],
        clearance_class: usize,
    ) -> Vec<ItemId> {
        let ctx = item_ctx!(self);
        let entry_counter = self.trees.entry_counter_mut();
        let counter_value = &mut *entry_counter;
        let mut counter = *counter_value;
        let result = self
            .trees
            .get_default_tree()
            .overlapping_items_with_clearance(
                shape,
                layer,
                ignore_net_nos,
                clearance_class,
                &self.items,
                &ctx,
                &mut counter,
            );
        *self.trees.entry_counter_mut() = counter;
        result
    }

    pub fn pick_items(&self, location: &Point, layer: Option<usize>) -> BTreeSet<ItemId> {
        let point_shape = TileShape::Box(TileShape::get_instance_from_point(location));
        self.overlapping_objects(&point_shape, layer)
            .into_iter()
            .filter_map(|object| match object {
                TreeObject::Item(id) => Some(id),
                TreeObject::Room(_) => None,
            })
            .collect()
    }

    pub fn pick_traces(&self, location: &Point, layer: Option<usize>) -> BTreeSet<ItemId> {
        self.pick_items(location, layer)
            .into_iter()
            .filter(|id| self.items.get(id).is_some_and(Item::is_trace))
            .collect()
    }

    pub fn check_shape(
        &mut self,
        shape: &Area,
        layer: Option<usize>,
        net_nos: &[i32],
        clearance_class: usize,
    ) -> bool {
        let bounding_box = self.bounding_box;
        let tiles = shape.split_to_convex().expect(
            "BasicBoard.checkShape: shape.splitToConvex() is null — Java throws a \
             NullPointerException here too (BasicBoard.java:957)",
        );
        for tile in tiles {
            if !tile.is_contained_in(&bounding_box) {
                return false;
            }
            let obstacles =
                self.overlapping_objects_with_clearance(&tile, layer, net_nos, clearance_class);
            for object in obstacles {
                let TreeObject::Item(other_id) = object else {
                    continue;
                };
                let Some(other) = self.items.get(&other_id) else {
                    continue;
                };
                let is_obstacle = net_nos
                    .iter()
                    .all(|net_no| other.is_obstacle_for_net(*net_no));
                if is_obstacle {
                    return false;
                }
            }
        }
        true
    }

    pub fn check_trace_shape(
        &mut self,
        shape: &TileShape,
        layer: usize,
        net_nos: &[i32],
        clearance_class: usize,
        contact_pins: Option<&BTreeSet<ItemId>>,
    ) -> bool {
        if !shape.is_contained_in(&self.bounding_box) {
            return false;
        }
        let compensation_used = self
            .trees
            .get_default_tree()
            .is_clearance_compensation_used();
        let tree_entries = if compensation_used {
            let ctx = self.ctx();
            self.trees.get_default_tree().overlapping_tree_entries(
                shape,
                Some(layer),
                &[],
                &self.items,
                &ctx,
            )
        } else {
            self.overlapping_tree_entries_with_clearance(shape, Some(layer), &[], clearance_class)
        };
        for entry in tree_entries {
            let TreeObject::Item(other_id) = entry.object else {
                continue;
            };
            let Some(other) = self.items.get(&other_id) else {
                continue;
            };
            if let Some(contact_pins) = contact_pins {
                if contact_pins.contains(&other_id) {
                    continue;
                }
                if matches!(other, Item::Pin(_)) {
                    return false;
                }
            }
            let mut is_obstacle = net_nos
                .iter()
                .all(|net_no| other.is_trace_obstacle(*net_no));
            let other_is_trace = other.is_trace();
            if is_obstacle
                && other_is_trace
                && let Some(contact_pins) = contact_pins
            {
                let tie_pins: Vec<ItemId> = contact_pins
                    .iter()
                    .copied()
                    .filter(|pin_id| match self.items.get(pin_id) {
                        Some(pin @ Item::Pin(_)) => {
                            pin.net_count() > 1 && pin.shares_net(&self.items[&other_id])
                        }
                        _ => false,
                    })
                    .collect();
                let mut intersection: Option<TileShape> = None;
                for pin_id in tie_pins {
                    if intersection.is_none() {
                        let Some(obstacle_trace_shape) =
                            self.item_tile_shape(other_id, entry.shape_index)
                        else {
                            continue;
                        };
                        intersection = Some(shape.intersection(&obstacle_trace_shape));
                    }
                    let Some(pin_shape) = self.drill_item_tile_shape_on_layer(pin_id, layer) else {
                        continue;
                    };
                    if pin_shape.contains_approx(intersection.as_ref().expect("just set")) {
                        is_obstacle = false;
                        break;
                    }
                }
            }
            if is_obstacle {
                return false;
            }
        }
        true
    }

    pub fn check_polyline_trace(
        &mut self,
        polyline: &Polyline,
        layer: usize,
        pen_half_width: i32,
        net_nos: &[i32],
        clearance_class: usize,
    ) -> bool {
        self.new_item_id();
        let default_tree = self.trees.get_default_tree();
        let compensated_half_width = pen_half_width
            + default_tree.clearance_compensation_value(clearance_class, layer, &self.rules);
        let shape_count = polyline.lines().len().saturating_sub(2);
        let shapes: Vec<TileShape> = (0..shape_count)
            .filter_map(|i| default_tree.offset_shape(polyline, compensated_half_width, i))
            .collect();
        let contact_pins = self.touching_pins_at_end_corners_of(
            polyline,
            layer,
            pen_half_width,
            net_nos,
            clearance_class,
        );
        for shape in shapes {
            if !self.check_trace_shape(&shape, layer, net_nos, clearance_class, Some(&contact_pins))
            {
                return false;
            }
        }
        true
    }

    pub fn touching_pins_at_end_corners(&mut self, id: ItemId) -> BTreeSet<ItemId> {
        let Some(Item::Trace(trace)) = self.items.get(&id) else {
            return BTreeSet::new();
        };
        let polyline = trace.polyline().clone();
        let layer = trace.get_layer();
        let half_width = trace.get_half_width();
        let clearance_class = trace.hdr.clearance_class();
        let net_nos = trace.hdr.net_nos.clone();
        self.touching_pins_at_end_corners_of(
            &polyline,
            layer,
            half_width,
            &net_nos,
            clearance_class,
        )
    }

    fn touching_pins_at_end_corners_of(
        &mut self,
        polyline: &Polyline,
        layer: usize,
        half_width: i32,
        net_nos: &[i32],
        clearance_class: usize,
    ) -> BTreeSet<ItemId> {
        let mut result = BTreeSet::new();
        let end_points = [polyline.first_corner(), polyline.last_corner()];
        for end_point in end_points.into_iter().flatten() {
            let octagon = end_point
                .surrounding_octagon()
                .enlarge(f64::from(half_width));
            let overlaps = self.overlapping_items_with_clearance(
                &TileShape::Octagon(octagon),
                Some(layer),
                &[],
                clearance_class,
            );
            for other_id in overlaps {
                let Some(other) = self.items.get(&other_id) else {
                    continue;
                };
                if matches!(other, Item::Pin(_)) && other.shares_net_no(net_nos) {
                    result.insert(other_id);
                }
            }
        }
        result
    }

    fn overlapping_objects_with_clearance(
        &mut self,
        shape: &TileShape,
        layer: Option<usize>,
        ignore_net_nos: &[i32],
        clearance_class: usize,
    ) -> BTreeSet<TreeObject> {
        let ctx = item_ctx!(self);
        let mut counter = self.trees.entry_counter();
        let result = self
            .trees
            .get_default_tree()
            .overlapping_objects_with_clearance(
                shape,
                layer,
                ignore_net_nos,
                clearance_class,
                &self.items,
                &ctx,
                &mut counter,
            );
        *self.trees.entry_counter_mut() = counter;
        result
    }

    fn overlapping_tree_entries_with_clearance(
        &mut self,
        shape: &TileShape,
        layer: Option<usize>,
        ignore_net_nos: &[i32],
        clearance_class: usize,
    ) -> Vec<crate::TreeEntry<TreeObject>> {
        let ctx = item_ctx!(self);
        let mut counter = self.trees.entry_counter();
        let result = self
            .trees
            .get_default_tree()
            .overlapping_tree_entries_with_clearance(
                shape,
                layer,
                ignore_net_nos,
                clearance_class,
                &self.items,
                &ctx,
                &mut counter,
            );
        *self.trees.entry_counter_mut() = counter;
        result
    }

    #[allow(clippy::too_many_arguments)]
    pub fn check_trace_segment(
        &mut self,
        from_point: &Point,
        to_point: &Point,
        layer: usize,
        net_nos: &[i32],
        trace_half_width: i32,
        clearance_class: usize,
        only_not_shovable_obstacles: bool,
    ) -> f64 {
        if from_point == to_point {
            return 0.0;
        }
        let polyline = Polyline::from_two_points(from_point, to_point);
        let Some(line_segment) = LineSegment::from_polyline(&polyline, 1) else {
            return 0.0;
        };
        self.check_trace_segment_of_line_segment(
            &line_segment,
            layer,
            net_nos,
            trace_half_width,
            clearance_class,
            only_not_shovable_obstacles,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn check_trace_segment_of_line_segment(
        &mut self,
        line_segment: &LineSegment,
        layer: usize,
        net_nos: &[i32],
        trace_half_width: i32,
        clearance_class: usize,
        only_not_shovable_obstacles: bool,
    ) -> f64 {
        let Ok(check_polyline) = line_segment.to_polyline() else {
            return 0.0;
        };
        if check_polyline.lines().len() != 3 {
            return 0.0;
        }
        let Some(shape_to_check) = check_polyline.offset_shape(trace_half_width, 0) else {
            return 0.0;
        };
        let from_point = line_segment.start_point_approx();
        let to_point = line_segment.end_point_approx();
        let line_length = to_point.distance(&from_point);
        let mut ok_length = f64::from(i32::MAX);
        let compensation_used = self
            .trees
            .get_default_tree()
            .is_clearance_compensation_used();

        let obstacle_entries = self.overlapping_tree_entries_with_clearance(
            &shape_to_check,
            Some(layer),
            net_nos,
            clearance_class,
        );

        let default_tree = self.default_tree_id();
        for entry in obstacle_entries {
            let TreeObject::Item(obstacle_id) = entry.object else {
                continue;
            };
            let Some(obstacle) = self.items.get(&obstacle_id) else {
                continue;
            };
            if only_not_shovable_obstacles
                && obstacle.is_routable()
                && !obstacle.is_shove_fixed(&self.rules)
            {
                continue;
            }
            let Some(obstacle_shape) =
                self.item_tree_shape_ref(obstacle_id, default_tree, entry.shape_index)
            else {
                continue;
            };
            let obstacle_shape = obstacle_shape.into_owned();
            let obstacle = &self.items[&obstacle_id];
            let (current_offset_shape, shorten_value) = if compensation_used {
                let compensation = self
                    .rules
                    .clearance_matrix
                    .clearance_compensation_value(obstacle.clearance_class(), layer);
                (
                    shape_to_check.clone(),
                    f64::from(trace_half_width + compensation),
                )
            } else {
                let clearance_value =
                    self.clearance_value(obstacle.clearance_class(), clearance_class, layer);
                (
                    shape_to_check.offset(f64::from(clearance_value)),
                    f64::from(trace_half_width + clearance_value),
                )
            };
            let intersection = obstacle_shape.intersection(&current_offset_shape);
            if intersection.is_empty() {
                continue;
            }
            let Some(nearest_obstacle_point) = intersection.nearest_point_approx(&from_point)
            else {
                continue;
            };
            let projection =
                from_point.scalar_product(&to_point, &nearest_obstacle_point) / line_length;
            let projection = 0.0_f64.max(projection - shorten_value - 1.0);
            if projection < ok_length {
                ok_length = projection;
                if ok_length <= 0.0 {
                    return 0.0;
                }
            }
        }
        ok_length
    }

    pub fn check_move_item(
        &mut self,
        id: ItemId,
        vector: &Vector,
        ignore_items: &mut Option<BTreeSet<ItemId>>,
    ) -> bool {
        let ctx = self.ctx();
        let Some(item) = self.items.get(&id) else {
            return false;
        };
        if item.net_count() > 1 {
            return false;
        }
        let contact_count = if item.as_connectable().is_some() {
            self.all_contacts(id).len()
        } else {
            0
        };
        let item = &self.items[&id];
        if item.is_trace() && contact_count > 0 {
            return false;
        }
        if let Some(ignore_items) = ignore_items {
            ignore_items.insert(id);
        }
        let net_nos = item.net_nos().to_vec();
        let clearance_class = item.clearance_class();
        let shape_layers: Vec<usize> = (0..item.tile_shape_count(&ctx))
            .map(|i| item.shape_layer(i, &ctx))
            .collect();
        let moved: Vec<(TileShape, usize)> = shape_layers
            .into_iter()
            .enumerate()
            .filter_map(|(i, layer)| {
                self.item_tile_shape(id, i)
                    .map(|shape| (shape.translate_by(vector), layer))
            })
            .collect();
        let bounding_box = self.bounding_box;
        for (moved_shape, shape_layer) in moved {
            if !moved_shape.is_contained_in(&bounding_box) {
                return false;
            }
            let obstacles = self.overlapping_items_with_clearance(
                &moved_shape,
                Some(shape_layer),
                &net_nos,
                clearance_class,
            );
            let item = &self.items[&id];
            for other_id in obstacles {
                let Some(other) = self.items.get(&other_id) else {
                    continue;
                };
                match ignore_items.as_ref() {
                    Some(ignore_items) => {
                        if !ignore_items.contains(&other_id) && other.is_obstacle(item, &self.ctx())
                        {
                            return false;
                        }
                    }
                    None => {
                        if other_id != id && other.is_obstacle(item, &self.ctx()) {
                            return false;
                        }
                    }
                }
            }
        }
        true
    }

    pub fn check_change_net(&mut self, id: ItemId, new_net_no: i32) -> bool {
        let ctx = self.ctx();
        let Some(item) = self.items.get(&id) else {
            return false;
        };
        let clearance_class = item.clearance_class();
        let shape_layers: Vec<usize> = (0..item.tile_shape_count(&ctx))
            .map(|i| item.shape_layer(i, &ctx))
            .collect();
        let shapes: Vec<(TileShape, usize)> = shape_layers
            .into_iter()
            .enumerate()
            .filter_map(|(i, layer)| self.item_tile_shape(id, i).map(|shape| (shape, layer)))
            .collect();
        for (shape, shape_layer) in shapes {
            let obstacles = self.overlapping_items_with_clearance(
                &shape,
                Some(shape_layer),
                &[new_net_no],
                clearance_class,
            );
            for other_id in obstacles {
                if other_id == id {
                    continue;
                }
                let Some(other) = self.items.get(&other_id) else {
                    continue;
                };
                if other.as_connectable().is_some() && !other.contains_net(new_net_no) {
                    return false;
                }
            }
        }
        true
    }

    pub fn pick_nearest_routing_item(
        &self,
        location: &Point,
        layer: Option<usize>,
        from_item: Option<ItemId>,
    ) -> Option<ItemId> {
        let ctx = self.ctx();
        let point_shape = TileShape::Box(TileShape::get_instance_from_point(location));
        let found_items: Vec<ItemId> = self
            .overlapping_items(&Area::Shape(point_shape.into()), layer)
            .into_iter()
            .rev()
            .collect();
        let pick_location = location.to_float();
        let mut min_dist = f64::from(i32::MAX);
        let mut nearest_item: Option<ItemId> = None;
        let mut ignore_set: Option<BTreeSet<ItemId>> = None;
        for current_id in found_items {
            let Some(current) = self.items.get(&current_id) else {
                continue;
            };
            if !current.is_connectable() {
                continue;
            }
            let nearest_is_drill = nearest_item
                .and_then(|id| self.items.get(&id))
                .is_some_and(Item::is_drill_item);
            let nearest_is_trace = nearest_item
                .and_then(|id| self.items.get(&id))
                .is_some_and(Item::is_trace);
            let mut candidate_found = false;
            let mut current_distance = 0.0;
            match current {
                Item::Trace(trace) => {
                    if layer.is_none_or(|layer| trace.get_layer() == layer) {
                        if nearest_is_drill {
                            continue;
                        }
                        let trace_radius = f64::from(trace.get_half_width());
                        current_distance = trace.polyline().distance(&pick_location);
                        if current_distance < min_dist && current_distance <= trace_radius {
                            candidate_found = true;
                        }
                    }
                }
                Item::Via(_) | Item::Pin(_) => {
                    if layer.is_none_or(|layer| current.is_on_layer(layer, &ctx)) {
                        let center = self
                            .drill_center(current_id)
                            .expect("a drill item has a centre");
                        current_distance = center.to_float().distance(&pick_location);
                        if current_distance < min_dist || nearest_is_trace {
                            candidate_found = true;
                        }
                    }
                }
                Item::ConductionArea(area) => {
                    if layer.is_none_or(|layer| area.get_layer() == layer) && nearest_item.is_none()
                    {
                        candidate_found = true;
                        current_distance = f64::from(i32::MAX);
                    }
                }
                _ => {}
            }
            if candidate_found {
                if let Some(from_item) = from_item {
                    let ignore_set =
                        ignore_set.get_or_insert_with(|| self.connected_set(from_item, -1, false));
                    if ignore_set.contains(&current_id) {
                        continue;
                    }
                }
                min_dist = current_distance;
                nearest_item = Some(current_id);
            }
        }
        nearest_item
    }

    pub fn get_trace_tail(
        &self,
        location: &Point,
        layer: Option<usize>,
        net_nos: &[i32],
    ) -> Option<ItemId> {
        let point_shape = TileShape::Box(TileShape::get_instance_from_point(location));
        for object in self.overlapping_objects(&point_shape, layer) {
            let TreeObject::Item(id) = object else {
                continue;
            };
            let Some(item @ Item::Trace(trace)) = self.items.get(&id) else {
                continue;
            };
            if !item.nets_equal_to(net_nos) {
                continue;
            }
            if trace.first_corner().as_ref() == Some(location)
                && self.trace_start_contacts(id).is_empty()
            {
                return Some(id);
            }
            if trace.last_corner().as_ref() == Some(location)
                && self.trace_end_contacts(id).is_empty()
            {
                return Some(id);
            }
        }
        None
    }

    pub fn remove_if_cycle(&mut self, id: ItemId) -> bool {
        self.remove_if_cycle_checked(id, &|| false)
            .expect("a `|| false` stop check never trips")
    }

    pub fn remove_if_cycle_checked(
        &mut self,
        id: ItemId,
        stop: StopCheck<'_>,
    ) -> Result<bool, BoardError> {
        let Some(item @ Item::Trace(trace)) = self.items.get(&id) else {
            return Ok(false);
        };
        if !item.is_on_the_board() || !self.is_trace_cycle(id) {
            return Ok(false);
        }
        let current_layer = trace.get_layer();
        let net_nos = item.net_nos().to_vec();
        let end_corners = [trace.first_corner(), trace.last_corner()];
        let tail_before: Vec<bool> = end_corners
            .iter()
            .map(|corner| {
                corner.as_ref().is_some_and(|corner| {
                    self.get_trace_tail(corner, Some(current_layer), &net_nos)
                        .is_some()
                })
            })
            .collect();
        let connection_items =
            self.connection_items_checked(id, StopConnectionOption::None, stop)?;
        self.remove_items(connection_items);
        for (index, corner) in end_corners.iter().enumerate() {
            if tail_before[index] {
                continue;
            }
            let Some(corner) = corner else { continue };
            if let Some(tail) = self.get_trace_tail(corner, Some(current_layer), &net_nos) {
                let tail_connection =
                    self.connection_items_checked(tail, StopConnectionOption::None, stop)?;
                self.remove_items(tail_connection);
            }
        }
        Ok(true)
    }

    pub fn contains_trace_tails(
        &self,
        ids: impl IntoIterator<Item = ItemId>,
        except_net_nos: &[i32],
    ) -> bool {
        for id in ids {
            let Some(item) = self.items.get(&id) else {
                continue;
            };
            if !item.is_trace() {
                continue;
            }
            if !item.nets_equal_to(except_net_nos) && self.is_tail(id) {
                return true;
            }
        }
        false
    }

    pub fn remove_trace_tails(
        &mut self,
        net_number: i32,
        stop_connection_option: StopConnectionOption,
    ) -> Result<bool, crate::BoardError> {
        let mut stub_set: BTreeSet<ItemId> = BTreeSet::new();
        for id in self.items_in_board_order() {
            let Some(item) = self.items.get(&id) else {
                continue;
            };
            if !item.is_routable() || item.net_count() != 1 {
                continue;
            }
            if net_number > 0 && item.get_net_number(0) != net_number {
                continue;
            }
            if !self.is_tail(id) {
                continue;
            }
            if matches!(self.items.get(&id), Some(Item::Via(_))) {
                if stop_connection_option == StopConnectionOption::Via {
                    continue;
                }
                if stop_connection_option == StopConnectionOption::FanoutVia
                    && self.is_fanout_via(id, None)
                {
                    continue;
                }
            }
            stub_set.insert(id);
        }
        let mut stub_connections: BTreeSet<ItemId> = BTreeSet::new();
        for id in stub_set {
            if self.normal_contacts(id).len() == 1 {
                stub_connections.extend(self.connection_items(id, stop_connection_option));
            } else {
                stub_connections.insert(id);
            }
        }
        if stub_connections.is_empty() {
            return Ok(false);
        }
        self.remove_items(stub_connections);
        self.combine_traces(net_number)?;
        Ok(true)
    }

    pub fn connect_to_trace(
        &mut self,
        from_point: &Point,
        to_trace: ItemId,
        pen_half_width: i32,
        clearance_class: usize,
    ) -> bool {
        let Some(item @ Item::Trace(trace)) = self.items.get(&to_trace) else {
            return false;
        };
        let polyline = trace.polyline().clone();
        let trace_layer = trace.get_layer();
        let net_nos = item.net_nos().to_vec();
        self.connect_to_trace_of(
            from_point,
            &polyline,
            trace_layer,
            &net_nos,
            pen_half_width,
            clearance_class,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn connect_to_trace_of(
        &mut self,
        from_point: &Point,
        polyline: &Polyline,
        trace_layer: usize,
        net_nos: &[i32],
        pen_half_width: i32,
        clearance_class: usize,
    ) -> bool {
        let first_corner = polyline.first_corner();
        let last_corner = polyline.last_corner();
        let net_nos = net_nos.to_vec();
        if polyline.contains(from_point) {
            return true;
        }
        let Some(projection_line) = polyline.projection_line(from_point) else {
            return false;
        };
        let Ok(connection_line) = projection_line.to_polyline() else {
            return false;
        };
        if connection_line.lines().len() != 3 {
            return false;
        }
        if !self.check_polyline_trace(
            &connection_line,
            trace_layer,
            pen_half_width,
            &net_nos,
            clearance_class,
        ) {
            return false;
        }
        if self.changed_area.is_some() {
            for i in 0..connection_line.corner_count() {
                if let Some(corner) = connection_line.corner_approx(i) {
                    self.join_changed_area(&corner, trace_layer);
                }
            }
        }
        self.insert_trace(
            connection_line,
            trace_layer,
            pen_half_width,
            net_nos.clone(),
            clearance_class,
            FixedState::Unfixed,
        );
        for corner in [first_corner, last_corner].into_iter().flatten() {
            if *from_point == corner {
                continue;
            }
            if let Some(tail) = self.get_trace_tail(&corner, Some(trace_layer), &net_nos)
                && !self.items[&tail].is_user_fixed()
            {
                self.remove_item(tail);
            }
        }
        true
    }

    pub fn reduce_nets_of_route_items(&mut self) -> bool {
        let result = false;
        let mut something_changed = true;
        while something_changed {
            something_changed = false;
            for id in self.items_in_board_order() {
                let Some(item) = self.items.get(&id) else {
                    continue;
                };
                if item.net_nos().len() <= 1 || item.get_fixed_state() == FixedState::SystemFixed {
                    continue;
                }
                let net_nos = item.net_nos().to_vec();
                if matches!(item, Item::Via(_)) {
                    let contacts = self.normal_contacts(id);
                    let mut to_remove = None;
                    'outer: for current_net_number in &net_nos {
                        for contact_id in &contacts {
                            let Some(contact) = self.items.get(contact_id) else {
                                continue;
                            };
                            if !contact.contains_net(*current_net_number) {
                                to_remove = Some(*current_net_number);
                                break 'outer;
                            }
                        }
                    }
                    if let Some(net_number) = to_remove {
                        self.items
                            .get_mut(&id)
                            .expect("present")
                            .remove_from_net(net_number);
                        something_changed = true;
                        break;
                    }
                } else if item.is_trace() {
                    let mut removed: Option<i32> = None;
                    let mut contacts = self.trace_start_contacts(id);
                    'ends: for end in 0..2 {
                        for current_net_number in &net_nos {
                            let mut pin_found = false;
                            for contact_id in &contacts {
                                let Some(contact @ Item::Pin(_)) = self.items.get(contact_id)
                                else {
                                    continue;
                                };
                                pin_found = true;
                                if !contact.contains_net(*current_net_number) {
                                    removed = Some(*current_net_number);
                                    break;
                                }
                            }
                            if !pin_found {
                                for contact_id in &contacts {
                                    let Some(contact) = self.items.get(contact_id) else {
                                        continue;
                                    };
                                    if !matches!(contact, Item::Pin(_))
                                        && !contact.contains_net(*current_net_number)
                                    {
                                        removed = Some(*current_net_number);
                                        break;
                                    }
                                }
                            }
                            if removed.is_some() {
                                break 'ends;
                            }
                        }
                        if end == 0 {
                            contacts = self.trace_end_contacts(id);
                        }
                    }
                    if let Some(net_number) = removed {
                        self.items
                            .get_mut(&id)
                            .expect("present")
                            .remove_from_net(net_number);
                        something_changed = true;
                        break;
                    }
                }
            }
        }
        result
    }
}
