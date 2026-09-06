use std::collections::BTreeSet;

use fr_board::datastructures::StopCheck;
use fr_board::items::Item;
use fr_board::prelude::*;
use fr_board::{BoardError, ItemId, RoomId, TimeLimit};
use fr_geometry::{IntOctagon, Point, Polyline, TileShape};
use fr_settings::{ExpansionCostFactor, RouterSettings};

use crate::autoroute::attempt::{AutorouteAttemptResult, AutorouteAttemptState};
use crate::autoroute::maze::control::AutorouteControl;
use crate::autoroute::maze::engine::AutorouteEngine;
use crate::board_ext::drill_item_mover::tree_by_id;
use crate::board_ext::tightener::{PolylineTraceExt, TraceTightener};
use crate::board_ext::trace_shover::TraceShover;
use crate::pipeline::RouterBudget;

pub trait RoutingBoardExt {
    fn init_autoroute(
        &mut self,
        engine: Option<AutorouteEngine>,
        net_number: i32,
        trace_clearance_class_index: usize,
        time_limit: Option<TimeLimit>,
        retain_autoroute_database: bool,
    ) -> AutorouteEngine;

    fn finish_autoroute(&mut self, engine: AutorouteEngine);

    fn additional_update_after_change(&mut self, engine: &mut AutorouteEngine, item: ItemId);

    fn clear_all_item_temporary_autoroute_data(&mut self);

    #[allow(clippy::too_many_arguments)]
    fn check_forced_trace_polyline(
        &mut self,
        polyline: &Polyline,
        half_width: i32,
        layer: usize,
        net_numbers: &[i32],
        clearance_class_index: usize,
        max_recursion_depth: i32,
        max_via_recursion_depth: i32,
        max_spring_over_recursion_depth: i32,
    ) -> bool;

    #[allow(clippy::too_many_arguments)]
    fn insert_forced_trace_polyline(
        &mut self,
        engine: Option<&mut AutorouteEngine>,
        polyline: &Polyline,
        half_width: i32,
        layer: usize,
        net_numbers: &[i32],
        clearance_class_index: usize,
        max_recursion_depth: i32,
        max_via_recursion_depth: i32,
        max_spring_over_recursion_depth: i32,
        tidy_width: i32,
        pull_tight_accuracy: i32,
        with_check: bool,
        time_limit: Option<&TimeLimit>,
        stop: StopCheck<'_>,
    ) -> Result<Option<Point>, BoardError>;

    #[allow(clippy::too_many_arguments)]
    fn insert_forced_trace_segment(
        &mut self,
        engine: Option<&mut AutorouteEngine>,
        from_corner: &Point,
        to_corner: &Point,
        half_width: i32,
        layer: usize,
        net_numbers: &[i32],
        clearance_class_index: usize,
        max_recursion_depth: i32,
        max_via_recursion_depth: i32,
        max_spring_over_recursion_depth: i32,
        tidy_width: i32,
        pull_tight_accuracy: i32,
        with_check: bool,
        time_limit: Option<&TimeLimit>,
        stop: StopCheck<'_>,
    ) -> Result<Option<Point>, BoardError>;

    #[allow(clippy::too_many_arguments)]
    fn opt_changed_area(
        &mut self,
        engine: Option<&mut AutorouteEngine>,
        only_net_no_arr: &[i32],
        clip_shape: Option<IntOctagon>,
        accuracy: i32,
        trace_costs: Option<&[ExpansionCostFactor]>,
        stop: StopCheck<'_>,
        time_limit_ms: i32,
    ) -> Result<(), BoardError>;

    #[allow(clippy::too_many_arguments)]
    fn opt_changed_area_with_keep_point(
        &mut self,
        engine: Option<&mut AutorouteEngine>,
        only_net_no_arr: &[i32],
        clip_shape: Option<IntOctagon>,
        accuracy: i32,
        trace_costs: Option<&[ExpansionCostFactor]>,
        stop: StopCheck<'_>,
        time_limit_ms: i32,
        keep_point: Option<Point>,
        keep_point_layer: i32,
    ) -> Result<(), BoardError>;

    fn remove_items_and_pull_tight(
        &mut self,
        engine: Option<&mut AutorouteEngine>,
        item_list: &[ItemId],
        tidy_width: i32,
        pull_tight_accuracy: i32,
    ) -> Result<bool, BoardError>;

    #[allow(clippy::too_many_arguments)]
    fn fanout(
        &mut self,
        engine: &mut Option<AutorouteEngine>,
        pin: ItemId,
        router_settings: &RouterSettings,
        ripup_costs: i32,
        stop: StopCheck<'_>,
        time_limit: Option<TimeLimit>,
        budget: RouterBudget,
    ) -> Result<AutorouteAttemptResult, BoardError>;
}

impl RoutingBoardExt for Board {
    fn init_autoroute(
        &mut self,
        engine: Option<AutorouteEngine>,
        net_number: i32,
        trace_clearance_class_index: usize,
        time_limit: Option<TimeLimit>,
        retain_autoroute_database: bool,
    ) -> AutorouteEngine {
        let reusable = engine.filter(|existing| {
            retain_autoroute_database
                && tree_by_id(self, existing.tree).compensated_clearance_class()
                    == trace_clearance_class_index
        });
        let mut engine = match reusable {
            Some(existing) => existing,
            None => {
                AutorouteEngine::new(self, trace_clearance_class_index, retain_autoroute_database)
            }
        };
        engine.init_connection(self, net_number, time_limit);
        engine
    }

    fn finish_autoroute(&mut self, mut engine: AutorouteEngine) {
        engine.clear(self);
        drop(engine);
    }

    fn additional_update_after_change(&mut self, engine: &mut AutorouteEngine, item: ItemId) {
        if self.get_item(item).is_none() {
            return;
        }
        if !engine.maintain_database {
            return;
        }
        let tree = engine.tree;
        let shape_count = self.item_tree_shape_count(item, tree);
        for i in 0..shape_count {
            let Some(current_shape) = self.item_tree_shape(item, tree, i) else {
                continue;
            };
            engine.invalidate_drill_pages(&current_shape);
            let current_layer = {
                let ctx = self.ctx();
                let Some(current_item) = self.get_item(item) else {
                    continue;
                };
                current_item.shape_layer(i, &ctx)
            };
            let rooms_to_remove: Vec<RoomId> = {
                let ctx = self.ctx();
                tree_by_id(self, tree)
                    .overlapping_objects_with_rooms(
                        &current_shape,
                        Some(current_layer),
                        &[],
                        &self.items,
                        &engine.rooms,
                        &ctx,
                    )
                    .into_iter()
                    .filter_map(|object| match object {
                        TreeObject::Room(room) => Some(room),
                        TreeObject::Item(_) => None,
                    })
                    .collect()
            };
            for room in rooms_to_remove {
                engine.remove_complete_expansion_room(self, room);
            }
        }
        if let Some(current_item) = self.items.get_mut(&item) {
            current_item.clear_autoroute_info();
        }
    }

    fn clear_all_item_temporary_autoroute_data(&mut self) {
        Board::clear_all_item_temporary_autoroute_data(self);
    }

    fn check_forced_trace_polyline(
        &mut self,
        polyline: &Polyline,
        half_width: i32,
        layer: usize,
        net_numbers: &[i32],
        clearance_class_index: usize,
        max_recursion_depth: i32,
        max_via_recursion_depth: i32,
        max_spring_over_recursion_depth: i32,
    ) -> bool {
        let compensated_half_width = half_width
            + self.trees.get_default_tree().clearance_compensation_value(
                clearance_class_index,
                layer,
                &self.rules,
            );
        let line_count = polyline.lines().len();
        if line_count == 0 {
            return true;
        }
        let trace_shapes =
            polyline.offset_shapes_between(compensated_half_width, 0, line_count - 1);
        let orthogonal_mode = self.rules.trace_angle_restriction == AngleRestriction::NinetyDegree;
        for (i, trace_shape) in trace_shapes.iter().enumerate() {
            let current_trace_shape = if orthogonal_mode {
                TileShape::Box(trace_shape.bounding_box())
            } else {
                trace_shape.clone()
            };
            let from_side = ShapeEntrySide::from_polyline(polyline, i + 1, &current_trace_shape);
            let check_shove_ok = TraceShover::check(
                self,
                &current_trace_shape,
                Some(&from_side),
                None,
                layer,
                net_numbers,
                clearance_class_index,
                max_recursion_depth,
                max_via_recursion_depth,
                max_spring_over_recursion_depth,
                None,
            );
            if !check_shove_ok {
                return false;
            }
        }
        true
    }

    fn insert_forced_trace_polyline(
        &mut self,
        engine: Option<&mut AutorouteEngine>,
        polyline: &Polyline,
        half_width: i32,
        layer: usize,
        net_numbers: &[i32],
        clearance_class_index: usize,
        max_recursion_depth: i32,
        max_via_recursion_depth: i32,
        max_spring_over_recursion_depth: i32,
        tidy_width: i32,
        pull_tight_accuracy: i32,
        with_check: bool,
        time_limit: Option<&TimeLimit>,
        stop: StopCheck<'_>,
    ) -> Result<Option<Point>, BoardError> {
        self.clear_shove_failing_obstacle();
        let from_corner = polyline.first_corner();
        let to_corner = polyline.last_corner();
        let (Some(from_corner), Some(to_corner)) = (from_corner, to_corner) else {
            return Ok(None);
        };
        if from_corner == to_corner {
            return Ok(Some(to_corner));
        }
        // :484-487. FRLogger.warn("only implemented for IntPoints")
        if !matches!(from_corner, Point::Int(_)) || !matches!(to_corner, Point::Int(_)) {
            return Ok(Some(from_corner));
        }
        self.start_marking_changed_area();

        let picked_items = self.pick_traces(&from_corner, Some(layer));
        let mut picked_trace: Option<Polyline> = None;
        if picked_items.len() == 1 {
            let current = *picked_items.iter().next().expect("size is 1");
            if let Some(Item::Trace(trace)) = self.items.get(&current)
                && trace.hdr.nets_equal(net_numbers)
                && trace.get_half_width() == half_width
                && trace.hdr.clearance_class() == clearance_class_index
            {
                picked_trace = Some(trace.polyline().clone());
            }
        }

        let compensated_half_width = half_width
            + self.trees.get_default_tree().clearance_compensation_value(
                clearance_class_index,
                layer,
                &self.rules,
            );
        let Some(mut new_polyline) = TraceShover::spring_over_obstacles(
            self,
            polyline,
            compensated_half_width,
            layer,
            net_numbers,
            clearance_class_index,
            None,
        ) else {
            return Ok(Some(from_corner));
        };

        let mut combined_polyline = combine_with_picked(&new_polyline, picked_trace.as_ref());
        if combined_polyline.lines().len() < 3 {
            return Ok(Some(from_corner));
        }
        let start_shape_no = (combined_polyline.lines().len() as i64
            - new_polyline.lines().len() as i64)
            .max(0) as usize;
        let trace_shapes = combined_polyline.offset_shapes_between(
            compensated_half_width,
            start_shape_no,
            combined_polyline.lines().len() - 1,
        );
        let mut last_shape_no = trace_shapes.len();
        let orthogonal_mode = self.rules.trace_angle_restriction == AngleRestriction::NinetyDegree;

        for i in 0..trace_shapes.len() {
            let current_trace_shape = if orthogonal_mode {
                TileShape::Box(trace_shapes[i].bounding_box())
            } else {
                trace_shapes[i].clone()
            };
            let from_side = entry_side(
                &combined_polyline,
                trace_shapes.len(),
                i,
                &current_trace_shape,
            );
            if with_check {
                let check_shove_ok = TraceShover::check(
                    self,
                    &current_trace_shape,
                    Some(&from_side),
                    None,
                    layer,
                    net_numbers,
                    clearance_class_index,
                    max_recursion_depth,
                    max_via_recursion_depth,
                    max_spring_over_recursion_depth,
                    time_limit,
                );
                if !check_shove_ok {
                    last_shape_no = i;
                    break;
                }
            }
            let insert_ok = TraceShover::insert(
                self,
                &current_trace_shape,
                Some(&from_side),
                layer,
                net_numbers,
                clearance_class_index,
                None,
                max_recursion_depth,
                max_via_recursion_depth,
                max_spring_over_recursion_depth,
                stop,
            )?;
            if !insert_ok {
                return Ok(None);
            }
        }

        let mut new_corner = to_corner.clone();
        if last_shape_no < trace_shapes.len() {
            let mut last_trace_shape = if orthogonal_mode {
                TileShape::Box(trace_shapes[last_shape_no].bounding_box())
            } else {
                trace_shapes[last_shape_no].clone()
            };
            let sample_width = 2 * self.get_min_trace_half_width();
            let (Some(last_corner), Some(prev_last_corner)) = (
                new_polyline.corner_approx(last_shape_no + 1),
                new_polyline.corner_approx(last_shape_no),
            ) else {
                return Ok(Some(from_corner));
            };
            let last_segment_length = last_corner.distance(&prev_last_corner);
            if last_segment_length > f64::from(100_i32.wrapping_mul(sample_width)) {
                return Ok(Some(from_corner));
            }
            let mut shape_index =
                shape_entry_index(&combined_polyline, trace_shapes.len(), last_shape_no);
            if last_segment_length > f64::from(sample_width) {
                let new_line_count = i64::try_from(new_polyline.lines().len()).unwrap_or(i64::MAX)
                    - (trace_shapes.len() as i64 - last_shape_no as i64 - 1);
                let new_line_count = usize::try_from(new_line_count).unwrap_or_else(|_| {
                    panic!(
                        "RoutingBoard.insertForcedTracePolyline:659-661: newLineCount is \
                         {new_line_count}, and Polyline.shorten throws on a negative one"
                    )
                });
                new_polyline = new_polyline
                    .shorten(new_line_count, f64::from(sample_width))
                    .unwrap_or_else(|e| {
                        panic!("Polyline.shorten threw (Polyline.java:148, quirk #22): {e}")
                    });
                let current_last_corner = new_polyline.last_corner();
                let Some(current_last_corner @ Point::Int(_)) = current_last_corner else {
                    return Ok(Some(from_corner));
                };
                new_corner = current_last_corner;
                combined_polyline = combine_with_picked(&new_polyline, picked_trace.as_ref());
                if combined_polyline.lines().len() < 3 {
                    return Ok(Some(new_corner));
                }
                shape_index = combined_polyline.lines().len() - 3;
                // `FRLogger.warn("offsetShape: no out of range")` + `null` cannot happen.
                let Some(shape) =
                    combined_polyline.offset_shape(compensated_half_width, shape_index)
                else {
                    return Ok(Some(from_corner));
                };
                last_trace_shape = if orthogonal_mode {
                    TileShape::Box(shape.bounding_box())
                } else {
                    shape
                };
            }
            let from_side =
                ShapeEntrySide::from_polyline(&combined_polyline, shape_index, &last_trace_shape);
            let check_shove_ok = TraceShover::check(
                self,
                &last_trace_shape,
                Some(&from_side),
                None,
                layer,
                net_numbers,
                clearance_class_index,
                max_recursion_depth,
                max_via_recursion_depth,
                max_spring_over_recursion_depth,
                time_limit,
            );
            if !check_shove_ok {
                return Ok(Some(from_corner));
            }
            let insert_ok = TraceShover::insert(
                self,
                &last_trace_shape,
                Some(&from_side),
                layer,
                net_numbers,
                clearance_class_index,
                None,
                max_recursion_depth,
                max_via_recursion_depth,
                max_spring_over_recursion_depth,
                stop,
            )?;
            if !insert_ok {
                return Ok(None);
            }
        }
        for i in 0..new_polyline.corner_count() {
            let Some(corner) = new_polyline.corner_approx(i) else {
                continue;
            };
            self.join_changed_area(&corner, layer);
        }
        let mut new_trace = self.insert_trace_without_cleaning(
            new_polyline.clone(),
            layer,
            half_width,
            net_numbers.to_vec(),
            clearance_class_index,
            FixedState::Unfixed,
        );
        if let Some(combine_target) = new_trace {
            let _combine_result = self.combine_trace(combine_target)?;
        }

        let tidy_region: Option<IntOctagon> = if tidy_width < i32::MAX {
            Some(
                new_corner
                    .surrounding_octagon()
                    .enlarge(f64::from(tidy_width)),
            )
        } else {
            None
        };
        let opt_net_no_arr = if max_recursion_depth <= 0 {
            net_numbers.to_vec()
        } else {
            Vec::new()
        };
        let mut pull_tight_algo = TraceTightener::get_instance(
            self,
            opt_net_no_arr,
            tidy_region,
            pull_tight_accuracy,
            None,
            -1,
            Some(new_corner.clone()),
            layer as i32,
        );

        {
            let clip_shape = self
                .changed_area
                .as_ref()
                .expect("RoutingBoard.insertForcedTracePolyline:488 called startMarkingChangedArea")
                .get_area(layer);
            let normalize_result = match new_trace {
                Some(trace) => self.normalize_trace_checked(trace, Some(&clip_shape), stop),
                None => Ok(false),
            };
            match normalize_result {
                Err(BoardError::Stopped) => return Err(BoardError::Stopped),
                Err(_) => {}
                Ok(false) => {}
                Ok(true) => match pull_tight_algo.split_traces_at_keep_point(self) {
                    Err(BoardError::Stopped) => return Err(BoardError::Stopped),
                    Err(_) => {}
                    Ok(_) => {
                        let picked = self.pick_traces(&new_corner, Some(layer));
                        new_trace = picked
                            .iter()
                            .next_back()
                            .copied()
                            .filter(|id| matches!(self.items.get(id), Some(Item::Trace(_))));
                    }
                },
            }
        }

        if tidy_width > 0
            && let Some(trace) = new_trace
        {
            <Board as PolylineTraceExt>::pull_tight_with_engine(
                self,
                trace,
                &mut pull_tight_algo,
                engine,
            );
        }
        Ok(Some(new_corner))
    }

    fn insert_forced_trace_segment(
        &mut self,
        engine: Option<&mut AutorouteEngine>,
        from_corner: &Point,
        to_corner: &Point,
        half_width: i32,
        layer: usize,
        net_numbers: &[i32],
        clearance_class_index: usize,
        max_recursion_depth: i32,
        max_via_recursion_depth: i32,
        max_spring_over_recursion_depth: i32,
        tidy_width: i32,
        pull_tight_accuracy: i32,
        with_check: bool,
        time_limit: Option<&TimeLimit>,
        stop: StopCheck<'_>,
    ) -> Result<Option<Point>, BoardError> {
        if from_corner == to_corner {
            return Ok(Some(to_corner.clone()));
        }
        let insert_polyline = Polyline::from_two_points(from_corner, to_corner);
        let ok_point = self.insert_forced_trace_polyline(
            engine,
            &insert_polyline,
            half_width,
            layer,
            net_numbers,
            clearance_class_index,
            max_recursion_depth,
            max_via_recursion_depth,
            max_spring_over_recursion_depth,
            tidy_width,
            pull_tight_accuracy,
            with_check,
            time_limit,
            stop,
        )?;
        let result = if ok_point.is_some() && ok_point == insert_polyline.first_corner() {
            Some(from_corner.clone())
        } else if ok_point.is_some() && ok_point == insert_polyline.last_corner() {
            Some(to_corner.clone())
        } else {
            ok_point
        };
        Ok(result)
    }

    fn opt_changed_area(
        &mut self,
        engine: Option<&mut AutorouteEngine>,
        only_net_no_arr: &[i32],
        clip_shape: Option<IntOctagon>,
        accuracy: i32,
        trace_costs: Option<&[ExpansionCostFactor]>,
        stop: StopCheck<'_>,
        time_limit_ms: i32,
    ) -> Result<(), BoardError> {
        self.opt_changed_area_with_keep_point(
            engine,
            only_net_no_arr,
            clip_shape,
            accuracy,
            trace_costs,
            stop,
            time_limit_ms,
            None,
            0,
        )
    }

    fn opt_changed_area_with_keep_point(
        &mut self,
        engine: Option<&mut AutorouteEngine>,
        only_net_no_arr: &[i32],
        clip_shape: Option<IntOctagon>,
        accuracy: i32,
        trace_costs: Option<&[ExpansionCostFactor]>,
        stop: StopCheck<'_>,
        time_limit_ms: i32,
        keep_point: Option<Point>,
        keep_point_layer: i32,
    ) -> Result<(), BoardError> {
        if self.changed_area.is_none() {
            return Ok(());
        }
        if !clip_shape.is_some_and(|shape| shape.is_empty()) {
            let mut pull_tight_algo = TraceTightener::get_instance(
                self,
                only_net_no_arr.to_vec(),
                clip_shape,
                accuracy,
                Some(stop),
                time_limit_ms,
                keep_point,
                keep_point_layer,
            );
            pull_tight_algo.opt_changed_area(self, engine, trace_costs)?;
        }
        self.changed_area = None;
        Ok(())
    }

    fn remove_items_and_pull_tight(
        &mut self,
        engine: Option<&mut AutorouteEngine>,
        item_list: &[ItemId],
        tidy_width: i32,
        pull_tight_accuracy: i32,
    ) -> Result<bool, BoardError> {
        let (mut tidy_region, calculate_tidy_region) = if tidy_width < i32::MAX {
            (Some(IntOctagon::EMPTY), tidy_width > 0)
        } else {
            (None, false)
        };

        if calculate_tidy_region {
            for id in item_list {
                let Some(item) = self.items.get(id) else {
                    continue;
                };
                if item.is_deletion_forbidden(&self.rules) || item.is_user_fixed() {
                    continue;
                }
                let shape_count = {
                    let ctx = self.ctx();
                    item.tile_shape_count(&ctx)
                };
                for i in 0..shape_count {
                    if let Some(shape) = self.item_tile_shape(*id, i)
                        && let Some(octagon) = shape.bounding_octagon()
                    {
                        tidy_region =
                            Some(tidy_region.unwrap_or(IntOctagon::EMPTY).union(&octagon));
                    }
                }
            }
        }

        let (result, changed_nets) = self.remove_items_marking_changed_area(item_list.to_vec());

        for net_number in changed_nets {
            self.combine_traces(net_number)?;
        }

        if calculate_tidy_region {
            tidy_region = tidy_region.map(|region| region.enlarge(f64::from(tidy_width)));
        }

        self.opt_changed_area(
            engine,
            &[],
            tidy_region,
            pull_tight_accuracy,
            None,
            &|| false,
            PULL_TIGHT_TIME_LIMIT,
        )?;
        Ok(result)
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn fanout(
        &mut self,
        engine: &mut Option<AutorouteEngine>,
        pin: ItemId,
        router_settings: &RouterSettings,
        ripup_costs: i32,
        stop: StopCheck<'_>,
        time_limit: Option<TimeLimit>,
        budget: RouterBudget,
    ) -> Result<AutorouteAttemptResult, BoardError> {
        let ctx = self.ctx();
        let pin_item = self
            .get_item(pin)
            .expect("RoutingBoard.fanout takes a Pin off the board");
        let already_connected = || {
            AutorouteAttemptResult::with_details(
                AutorouteAttemptState::AlreadyConnected,
                format!("The pin '{pin_item}' is already connected."),
            )
        };
        if pin_item.first_layer(&ctx) != pin_item.last_layer(&ctx) || pin_item.net_count() != 1 {
            return Ok(already_connected());
        }
        let pin_net_no = pin_item.get_net_number(0);
        let pin_layer = pin_item.first_layer(&ctx);
        let pin_connected_set = self.connected_set(pin, pin_net_no, false);
        for current_item in pin_connected_set.iter().rev() {
            let Some(item) = self.get_item(*current_item) else {
                continue;
            };
            if item.first_layer(&ctx) != pin_layer || item.last_layer(&ctx) != pin_layer {
                return Ok(already_connected());
            }
        }
        let unconnected_set = self.unconnected_set(pin, pin_net_no);
        if unconnected_set.is_empty() {
            return Ok(AutorouteAttemptResult::with_details(
                AutorouteAttemptState::NoUnconnectedNets,
                format!("The pin '{pin_item}' is already connected."),
            ));
        }

        let pin_center = pin_center_of(self, pin).to_float();
        let sorted_unconnected_list =
            sorted_unconnected_targets(self, &pin_center, &unconnected_set);

        let mut ctrl_settings = AutorouteControl::from_settings(self, pin_net_no, router_settings);
        ctrl_settings.is_fanout = true;

        let fallback_to_board_vias = router_settings
            .fanout
            .as_ref()
            .and_then(|f| f.fallback_to_board_vias)
            .unwrap_or(false);
        if fallback_to_board_vias && ctrl_settings.via_rule.is_some() {
            let net_class_rule = ctrl_settings
                .via_rule
                .clone()
                .expect("guarded by the `is_some` above");
            let combined_via_rule =
                combined_fallback_via_rule(&net_class_rule, &self.rules.via_rules);
            ctrl_settings.via_rule = Some(combined_via_rule);
            ctrl_settings.rebuild_via_info(self, router_settings.get_via_costs(), pin_net_no);
        }

        let component_name = self.components.get(pin_item.component_id()).name.clone();
        let pin_name = match self.get_item(pin) {
            Some(Item::Pin(p)) => p.name(&ctx).map(str::to_owned),
            _ => None,
        };
        ctrl_settings.fanout_start_pin_name = match pin_name {
            Some(name) => Some(format!("{component_name}-{name}")),
            None => Some(format!("{pin_item}")),
        };
        ctrl_settings.fanout_start_pin_center = Some(pin_center_of(self, pin));
        ctrl_settings.fanout_start_pin_layer =
            i32::try_from(pin_layer).expect("a board layer index");
        ctrl_settings.remove_unconnected_vias = false;
        if ripup_costs >= 0 {
            ctrl_settings.ripup_allowed = true;
            ctrl_settings.ripup_costs = ripup_costs;
        }

        let mut ripped_item_list: BTreeSet<ItemId> = BTreeSet::new();
        *engine = Some(self.init_autoroute(
            engine.take(),
            pin_net_no,
            ctrl_settings.trace_clearance_class_index,
            time_limit,
            false,
        ));
        let autoroute_engine = engine
            .as_mut()
            .expect("initAutoroute always answers an engine");

        let mut result: Option<AutorouteAttemptResult> = None;
        if sorted_unconnected_list.len() <= 4 {
            if let Some(closest_target) = sorted_unconnected_list.first().copied() {
                let mut single_target = BTreeSet::new();
                single_target.insert(closest_target);
                let first = autoroute_engine.autoroute_connection(
                    self,
                    &pin_connected_set,
                    &single_target,
                    &ctrl_settings,
                    &mut ripped_item_list,
                    None,
                    stop,
                );
                let retry = first.state != AutorouteAttemptState::Routed
                    && first.state != AutorouteAttemptState::AlreadyConnected
                    && sorted_unconnected_list.len() > 1;
                result = Some(if retry {
                    let mut retry_ripped_item_list = BTreeSet::new();
                    let retry_result = autoroute_engine.autoroute_connection(
                        self,
                        &pin_connected_set,
                        &unconnected_set,
                        &ctrl_settings,
                        &mut retry_ripped_item_list,
                        None,
                        stop,
                    );
                    if retry_result.state == AutorouteAttemptState::Routed {
                        ripped_item_list.extend(retry_ripped_item_list);
                    }
                    retry_result
                } else {
                    first
                });
            }
        } else {
            result = Some(autoroute_engine.autoroute_connection(
                self,
                &pin_connected_set,
                &unconnected_set,
                &ctrl_settings,
                &mut ripped_item_list,
                None,
                stop,
            ));
        }

        let result = result.unwrap_or_else(|| {
            AutorouteAttemptResult::with_details(
                AutorouteAttemptState::Failed,
                "No target items to route connection.".to_string(),
            )
        });

        if result.state == AutorouteAttemptState::Routed {
            let trace_costs = ctrl_settings.trace_costs.clone();
            let opt_result = self.opt_changed_area(
                engine.as_mut(),
                &[pin_net_no],
                None,
                router_settings
                    .trace_pull_tight_accuracy
                    .expect("RoutingBoard.fanout:1103 unboxes tracePullTightAccuracy; null NPEs"),
                Some(&trace_costs),
                stop,
                budget.opt_changed_area_ms,
            );
            if opt_result.is_err() {
                self.changed_area = None;
            }
            opt_result?;
        }
        Ok(result)
    }
}

const PULL_TIGHT_TIME_LIMIT: i32 = 2000;

pub fn sorted_unconnected_targets(
    board: &Board,
    pin_center: &fr_geometry::FloatPoint,
    unconnected_set: &BTreeSet<ItemId>,
) -> Vec<ItemId> {
    let ctx = board.ctx();
    let dist_sq = |id: ItemId| -> f64 {
        let bx = board
            .get_item(id)
            .expect("an unconnected-set item")
            .bounding_box(&ctx);
        let cx = f64::from(bx.ll.x + bx.ur.x) / 2.0;
        let cy = f64::from(bx.ll.y + bx.ur.y) / 2.0;
        let dx = cx - pin_center.x;
        let dy = cy - pin_center.y;
        dx * dx + dy * dy
    };
    let mut list: Vec<ItemId> = unconnected_set.iter().rev().copied().collect();
    list.sort_by(|item1, item2| dist_sq(*item1).total_cmp(&dist_sq(*item2)));
    list
}

pub fn combined_fallback_via_rule(
    net_class_rule: &ViaRule,
    board_via_rules: &[ViaRule],
) -> ViaRule {
    let mut combined_via_rule = ViaRule::new(format!("{}_fallback", net_class_rule.name));
    for i in 0..net_class_rule.via_count() {
        combined_via_rule.append_via(net_class_rule.get_via(i).clone());
    }
    if let Some(default_via_rule) = board_via_rules.first() {
        for i in 0..default_via_rule.via_count() {
            let default_via = default_via_rule.get_via(i);
            if !combined_via_rule.contains(default_via) {
                combined_via_rule.append_via(default_via.clone());
            }
        }
    }
    combined_via_rule
}

fn pin_center_of(board: &Board, pin: ItemId) -> Point {
    let ctx = board.ctx();
    match board.get_item(pin) {
        Some(Item::Pin(p)) => p.get_center(&ctx),
        _ => panic!("RoutingBoard.fanout takes a Pin"),
    }
}

fn combine_with_picked(new_polyline: &Polyline, picked: Option<&Polyline>) -> Polyline {
    let Some(combine_polyline) = picked else {
        return new_polyline.clone();
    };
    new_polyline
        .combine(combine_polyline)
        .unwrap_or_else(|e| panic!("Polyline.combine threw (Polyline.java:148, quirk #22): {e}"))
}

fn shape_entry_index(combined_polyline: &Polyline, trace_shape_count: usize, i: usize) -> usize {
    combined_polyline.corner_count() - trace_shape_count - 1 + i
}

fn entry_side(
    combined_polyline: &Polyline,
    trace_shape_count: usize,
    i: usize,
    shape: &TileShape,
) -> ShapeEntrySide {
    ShapeEntrySide::from_polyline(
        combined_polyline,
        shape_entry_index(combined_polyline, trace_shape_count, i),
        shape,
    )
}
