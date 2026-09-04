use fr_board::datastructures::StopCheck;
use fr_board::prelude::*;
use fr_board::{BoardError, ItemId};
use fr_geometry::{IntPoint, Point, Polyline};

use crate::autoroute::maze::AutorouteControl;
use crate::autoroute::maze::engine::AutorouteEngine;
use crate::autoroute::path::locator::{
    FoundConnectionLocator, ResultItem, calculate_additional_corner,
};
use crate::board_ext::{ForcedViaInserter, RoutingBoardExt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundConnectionInserter {
            last_corner: Option<IntPoint>,
        first_corner: Option<IntPoint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TraceSnapshot {
    polyline: Polyline,
    layer: usize,
    net_nos: Vec<i32>,
}

impl FoundConnectionInserter {
                    fn new() -> FoundConnectionInserter {
        FoundConnectionInserter {
            last_corner: None,
            first_corner: None,
        }
    }

                                                                pub fn get_instance(
        connection: Option<&FoundConnectionLocator>,
        board: &mut Board,
        ctrl: &AutorouteControl,
        mut engine: Option<&mut AutorouteEngine>,
        stop: StopCheck<'_>,
    ) -> Result<Option<FoundConnectionInserter>, BoardError> {
        let Some(connection) = connection else {
            return Ok(None);
        };
        let target_trace = Self::trace_snapshot(board, connection.target_item);
        let start_trace = Self::trace_snapshot(board, connection.start_item);
        let mut current_layer = connection.target_layer;
        let mut new_instance = FoundConnectionInserter::new();
        for current_new_item in &connection.connection_items {
            let via_location = Point::Int(current_new_item.corners[0]);
            if !new_instance.insert_via(
                board,
                ctrl,
                Some(&via_location),
                current_layer,
                current_new_item.layer,
                stop,
            )? {
                return Ok(None);
            }
            current_layer = current_new_item.layer;
            if !new_instance.insert_trace(
                board,
                ctrl,
                engine.as_deref_mut(),
                current_new_item,
                stop,
            )? {
                return Ok(None);
            }
        }
        let last_corner = new_instance.last_corner.map(Point::Int);
        if !new_instance.insert_via(
            board,
            ctrl,
            last_corner.as_ref(),
            current_layer,
            connection.start_layer,
            stop,
        )? {
            return Ok(None);
        }
        if let Some(target_trace) = &target_trace
            && let Some(first_corner) = new_instance.first_corner
        {
            board.connect_to_trace_of(
                &Point::Int(first_corner),
                &target_trace.polyline,
                target_trace.layer,
                &target_trace.net_nos,
                ctrl.trace_half_width[connection.start_layer],
                ctrl.trace_clearance_class_index,
            );
        }
        if let Some(start_trace) = &start_trace
            && let Some(last_corner) = new_instance.last_corner
        {
            board.connect_to_trace_of(
                &Point::Int(last_corner),
                &start_trace.polyline,
                start_trace.layer,
                &start_trace.net_nos,
                ctrl.trace_half_width[connection.target_layer],
                ctrl.trace_clearance_class_index,
            );
        }

        board.normalize_traces_checked(ctrl.net_number, stop)?;

        Ok(Some(new_instance))
    }

                fn trace_snapshot(board: &Board, item: Option<ItemId>) -> Option<TraceSnapshot> {
        let id = item?;
        let item @ Item::Trace(trace) = board.get_item(id)? else {
            return None;
        };
        Some(TraceSnapshot {
            polyline: trace.polyline().clone(),
            layer: trace.get_layer(),
            net_nos: item.net_nos().to_vec(),
        })
    }

                                                #[allow(clippy::too_many_lines)] 
    fn insert_trace(
        &mut self,
        board: &mut Board,
        ctrl: &AutorouteControl,
        mut engine: Option<&mut AutorouteEngine>,
        trace: &ResultItem,
        stop: StopCheck<'_>,
    ) -> Result<bool, BoardError> {
        if trace.corners.len() == 1 {
            if self.first_corner.is_none() {
                self.first_corner = Some(trace.corners[0]);
            }
            self.last_corner = Some(trace.corners[0]);
            return Ok(true);
        }

        let saved_edge_to_turn_dist = board.rules.get_pin_edge_to_turn_dist();
        board.rules.set_pin_edge_to_turn_dist(-1.0);

        let mut start_pin: Option<ItemId> = None;
        let mut end_pin: Option<ItemId> = None;
        if ctrl.with_neckdown {
            let mut current_end_corner = Point::Int(trace.corners[0]);
            for i in 0..2 {
                let picked = board.pick_items(&current_end_corner, Some(trace.layer));
                for id in picked.into_iter().rev() {
                    let Some(item @ Item::Pin(_)) = board.get_item(id) else {
                        continue;
                    };
                    if item.contains_net(ctrl.net_number)
                        && board.drill_center(id).as_ref() == Some(&current_end_corner)
                    {
                        if i == 0 {
                            start_pin = Some(id);
                        } else {
                            end_pin = Some(id);
                        }
                    }
                }
                current_end_corner = Point::Int(trace.corners[trace.corners.len() - 1]);
            }
        }
        let net_numbers = [ctrl.net_number];

        let mut from_corner_no = 0usize;
        let mut result = true;
        for i in 1..trace.corners.len() {
            let current_corner_arr: Vec<Point> = trace.corners[from_corner_no..=i]
                .iter()
                .map(|corner| Point::Int(*corner))
                .collect();
            let insert_polyline = Polyline::from_points(&current_corner_arr);
            let ok_point = board.insert_forced_trace_polyline(
                engine.as_deref_mut(),
                &insert_polyline,
                ctrl.trace_half_width[trace.layer],
                trace.layer,
                &net_numbers,
                ctrl.trace_clearance_class_index,
                ctrl.max_shove_trace_recursion_depth,
                ctrl.max_shove_via_recursion_depth,
                ctrl.max_spring_over_recursion_depth,
                i32::MAX,
                ctrl.pull_tight_accuracy,
                true,
                None,
                stop,
            )?;
            crate::autoroute::instrument::note_mutation(
                crate::autoroute::instrument::Mutation::ForcedTraceInsert,
            );
            let last_corner = insert_polyline.last_corner();
            let first_corner = insert_polyline.first_corner();
            let mut neckdown_inserted = false;
            let mut micro_neckdown_inserted = false;
            if let Some(ok) = ok_point.as_ref()
                && ok_point != last_corner
                && ctrl.with_neckdown
                && current_corner_arr.len() == 2
            {
                neckdown_inserted = self.insert_neckdown(
                    board,
                    ctrl,
                    engine.as_deref_mut(),
                    ok,
                    &current_corner_arr[1],
                    trace.layer,
                    start_pin,
                    end_pin,
                    stop,
                )?;
            }
            if !neckdown_inserted
                && ok_point != last_corner
                && ctrl.is_fanout
                && current_corner_arr.len() == 2
            {
                micro_neckdown_inserted = self.insert_fanout_micro_neckdown(
                    board,
                    ctrl,
                    engine.as_deref_mut(),
                    ok_point.as_ref(),
                    &current_corner_arr[1],
                    trace.layer,
                    &net_numbers,
                    start_pin,
                    end_pin,
                    stop,
                )?;
            }
            if ok_point == last_corner || neckdown_inserted || micro_neckdown_inserted {
                from_corner_no = i;
            } else if ok_point == first_corner && i != trace.corners.len() - 1 {
                if from_corner_no > 0 {
                    if current_corner_arr.len() < 3 {
                        from_corner_no -= 1;
                    }
                }
            } else {
                result = false;
                break;
            }
        }

        for i in 0..trace.corners.len() - 1 {
            let corner = Point::Int(trace.corners[i]);
            if let Some(trace_stub) = board.get_trace_tail(&corner, Some(trace.layer), &net_numbers)
            {
                board.remove_item(trace_stub);
            }
        }

        board
            .rules
            .set_pin_edge_to_turn_dist(saved_edge_to_turn_dist);
        if self.first_corner.is_none() {
            self.first_corner = Some(trace.corners[0]);
        }
        self.last_corner = Some(trace.corners[trace.corners.len() - 1]);
        Ok(result)
    }

                                                                                                                                        #[allow(clippy::too_many_arguments)] 
    fn insert_fanout_micro_neckdown(
        &self,
        board: &mut Board,
        ctrl: &AutorouteControl,
        mut engine: Option<&mut AutorouteEngine>,
        ok_point: Option<&Point>,
        target_point: &Point,
        layer: usize,
        net_numbers: &[i32],
        start_pin: Option<ItemId>,
        end_pin: Option<ItemId>,
        stop: StopCheck<'_>,
    ) -> Result<bool, BoardError> {
        let from_point = ok_point.unwrap_or(target_point);
        if from_point == target_point {
            return Ok(false);
        }
        let base_half_width = ctrl.trace_half_width[layer];
        let mut candidate_half_widths: Vec<i32> = Vec::new();
        let add = |value: i32, list: &mut Vec<i32>| {
            if !list.contains(&value) {
                list.push(value);
            }
        };
        let ctx = board.ctx();
        if let Some(pin_id) = start_pin
            && let Some(Item::Pin(pin)) = board.get_item(pin_id)
            && pin.is_on_layer(layer, &ctx)
        {
            add(
                pin.get_trace_neckdown_halfwidth(layer, &ctx),
                &mut candidate_half_widths,
            );
        }
        if let Some(pin_id) = end_pin
            && let Some(Item::Pin(pin)) = board.get_item(pin_id)
            && pin.is_on_layer(layer, &ctx)
        {
            add(
                pin.get_trace_neckdown_halfwidth(layer, &ctx),
                &mut candidate_half_widths,
            );
        }
        add(
            1.max(base_half_width.wrapping_mul(3) / 4),
            &mut candidate_half_widths,
        );
        add(
            1.max(base_half_width.wrapping_mul(3) / 5),
            &mut candidate_half_widths,
        );
        add(1.max(base_half_width / 2), &mut candidate_half_widths);

        let min_half_width = board.rules.get_min_trace_half_width();

        for candidate_half_width in candidate_half_widths {
            if candidate_half_width <= 0 || candidate_half_width >= base_half_width {
                continue;
            }
            if candidate_half_width < min_half_width {
                continue;
            }
            let candidate_ok_point = board.insert_forced_trace_segment(
                engine.as_deref_mut(),
                from_point,
                target_point,
                candidate_half_width,
                layer,
                net_numbers,
                ctrl.trace_clearance_class_index,
                ctrl.max_shove_trace_recursion_depth,
                ctrl.max_shove_via_recursion_depth,
                ctrl.max_spring_over_recursion_depth,
                i32::MAX,
                ctrl.pull_tight_accuracy,
                true,
                None,
                stop,
            )?;
            if candidate_ok_point.as_ref() == Some(target_point) {
                return Ok(true);
            }
        }
        Ok(false)
    }

                                    #[allow(clippy::too_many_arguments)] 
    fn insert_neckdown(
        &self,
        board: &mut Board,
        ctrl: &AutorouteControl,
        mut engine: Option<&mut AutorouteEngine>,
        from_corner: &Point,
        to_corner: &Point,
        layer: usize,
        start_pin: Option<ItemId>,
        end_pin: Option<ItemId>,
        stop: StopCheck<'_>,
    ) -> Result<bool, BoardError> {
        if let Some(pin) = start_pin {
            let ok_point = self.try_neck_down(
                board,
                ctrl,
                engine.as_deref_mut(),
                to_corner,
                from_corner,
                layer,
                pin,
                true,
                stop,
            )?;
            if ok_point.as_ref() == Some(from_corner) {
                return Ok(true);
            }
        }
        if let Some(pin) = end_pin {
            let ok_point = self.try_neck_down(
                board,
                ctrl,
                engine,
                from_corner,
                to_corner,
                layer,
                pin,
                false,
                stop,
            )?;
            return Ok(ok_point.as_ref() == Some(to_corner));
        }
        Ok(false)
    }

                            #[allow(clippy::too_many_arguments)] 
    #[allow(clippy::too_many_lines)] 
    fn try_neck_down(
        &self,
        board: &mut Board,
        ctrl: &AutorouteControl,
        mut engine: Option<&mut AutorouteEngine>,
        from_corner: &Point,
        to_corner: &Point,
        layer: usize,
        pin: ItemId,
        at_start: bool,
        stop: StopCheck<'_>,
    ) -> Result<Option<Point>, BoardError> {
        let _ = at_start;
        let ctx = board.ctx();
        let Some(Item::Pin(pin_item)) = board.get_item(pin) else {
            return Ok(None);
        };
        if !pin_item.is_on_layer(layer, &ctx) {
            return Ok(None);
        }
        let pin_center = pin_item.get_center(&ctx).to_float();
        let pin_clearance_class = board
            .get_item(pin)
            .expect("the pin was just read")
            .clearance_class();
        let pin_max_width = pin_item.get_max_width(layer, &ctx);
        let neck_down_halfwidth = pin_item.get_trace_neckdown_halfwidth(layer, &ctx);

        let current_clearance = f64::from(board.rules.clearance_matrix.get_value(
            ctrl.trace_clearance_class_index,
            pin_clearance_class,
            layer,
            true,
        ));
        let pin_neck_down_distance = 2.0 * (0.5 * pin_max_width + current_clearance);
        if pin_center.distance(&to_corner.to_float()) >= pin_neck_down_distance {
            return Ok(None);
        }
        if neck_down_halfwidth >= ctrl.trace_half_width[layer] {
            return Ok(None);
        }

        let float_from_corner = from_corner.to_float();
        let float_to_corner = to_corner.to_float();
        let tolerance = 2.0;
        let net_numbers = [ctrl.net_number];

        let mut ok_length = board.check_trace_segment(
            from_corner,
            to_corner,
            layer,
            &net_numbers,
            ctrl.trace_half_width[layer],
            ctrl.trace_clearance_class_index,
            true,
        );
        if ok_length >= f64::from(i32::MAX) {
            return Ok(Some(from_corner.clone()));
        }
        ok_length -= tolerance;
        let mut neck_down_end_point: Point;
        if ok_length <= tolerance {
            neck_down_end_point = from_corner.clone();
        } else {
            let float_neck_down_end_point =
                float_from_corner.change_length(&float_to_corner, ok_length);
            neck_down_end_point = Point::Int(float_neck_down_end_point.round());
            let horizontal_first = (float_from_corner.x - float_neck_down_end_point.x).abs()
                >= (float_from_corner.y - float_neck_down_end_point.y).abs();
            let mut add_corner = Point::Int(
                calculate_additional_corner(
                    float_from_corner,
                    float_neck_down_end_point,
                    horizontal_first,
                    board.rules.trace_angle_restriction,
                )
                .round(),
            );
            let mut current_ok_point = self.forced_segment(
                board,
                ctrl,
                engine.as_deref_mut(),
                from_corner,
                &add_corner,
                ctrl.trace_half_width[layer],
                layer,
                &net_numbers,
                stop,
            )?;
            if current_ok_point.as_ref() != Some(&add_corner) {
                return Ok(Some(from_corner.clone()));
            }
            current_ok_point = self.forced_segment(
                board,
                ctrl,
                engine.as_deref_mut(),
                &add_corner,
                &neck_down_end_point,
                ctrl.trace_half_width[layer],
                layer,
                &net_numbers,
                stop,
            )?;
            if current_ok_point.as_ref() != Some(&neck_down_end_point) {
                return Ok(Some(from_corner.clone()));
            }
            add_corner = Point::Int(
                calculate_additional_corner(
                    float_neck_down_end_point,
                    float_to_corner,
                    !horizontal_first,
                    board.rules.trace_angle_restriction,
                )
                .round(),
            );
            if &add_corner != to_corner {
                current_ok_point = self.forced_segment(
                    board,
                    ctrl,
                    engine.as_deref_mut(),
                    &neck_down_end_point,
                    &add_corner,
                    ctrl.trace_half_width[layer],
                    layer,
                    &net_numbers,
                    stop,
                )?;
                if current_ok_point.as_ref() != Some(&add_corner) {
                    return Ok(Some(from_corner.clone()));
                }
                neck_down_end_point = add_corner;
            }
        }

        self.forced_segment(
            board,
            ctrl,
            engine,
            &neck_down_end_point,
            to_corner,
            neck_down_halfwidth,
            layer,
            &net_numbers,
            stop,
        )
    }

                        #[allow(clippy::too_many_arguments)]
    fn forced_segment(
        &self,
        board: &mut Board,
        ctrl: &AutorouteControl,
        engine: Option<&mut AutorouteEngine>,
        from_corner: &Point,
        to_corner: &Point,
        half_width: i32,
        layer: usize,
        net_numbers: &[i32],
        stop: StopCheck<'_>,
    ) -> Result<Option<Point>, BoardError> {
        board.insert_forced_trace_segment(
            engine,
            from_corner,
            to_corner,
            half_width,
            layer,
            net_numbers,
            ctrl.trace_clearance_class_index,
            ctrl.max_shove_trace_recursion_depth,
            ctrl.max_shove_via_recursion_depth,
            ctrl.max_spring_over_recursion_depth,
            i32::MAX,
            ctrl.pull_tight_accuracy,
            true,
            None,
            stop,
        )
    }

                                                                                                                            fn insert_via(
        &self,
        board: &mut Board,
        ctrl: &AutorouteControl,
        location: Option<&Point>,
        input_from_layer: usize,
        input_to_layer: usize,
        stop: StopCheck<'_>,
    ) -> Result<bool, BoardError> {
        if input_from_layer == input_to_layer {
            return Ok(true); 
        }
        let (from_layer, to_layer) = if input_from_layer < input_to_layer {
            (input_from_layer, input_to_layer)
        } else {
            (input_to_layer, input_from_layer)
        };
        let net_numbers = [ctrl.net_number];
        let mut via_info = None;
        let mut found_suitable_span = false;
        let via_rule = ctrl
            .via_rule
            .as_ref()
            .expect("FoundConnectionInserter.insertVia:701 dereferences ctrl.viaRule (NPE)");
        let via_count = via_rule.via_count();
        for i in 0..via_count {
            let current_via_info = via_rule.get_via(i).clone();
            let current_via_padstack = current_via_info.get_padstack();
            let padstack_from = board
                .library
                .padstacks
                .padstack_from_layer(current_via_padstack);
            let padstack_to = board
                .library
                .padstacks
                .padstack_to_layer(current_via_padstack);
            if padstack_from > java_int(from_layer) || padstack_to < java_int(to_layer) {
                continue;
            }
            found_suitable_span = true;
            let location = location
                .expect("FoundConnectionInserter.insertVia:708 -> ForcedViaInserter.java:140 dereferences a null location (NPE)");
            if ForcedViaInserter::check(
                board,
                &current_via_info,
                location,
                &net_numbers,
                ctrl.max_shove_trace_recursion_depth,
                ctrl.max_shove_via_recursion_depth,
                Some(&ctrl.trace_half_width),
                ctrl.trace_clearance_class_index,
            ) {
                via_info = Some(current_via_info);
                break;
            }
        }
        let Some(via_info) = via_info else {
            let _ = found_suitable_span;
            return Ok(false);
        };
        let location = location
            .expect("FoundConnectionInserter.insertVia:754 is reached only through :708's check");
        if !ForcedViaInserter::insert(
            board,
            &via_info,
            location,
            &net_numbers,
            ctrl.trace_clearance_class_index,
            &ctrl.trace_half_width,
            ctrl.max_shove_trace_recursion_depth,
            ctrl.max_shove_via_recursion_depth,
            stop,
        )? {
            return Ok(false);
        }
        Ok(true)
    }
}

fn java_int(layer: usize) -> i32 {
    i32::try_from(layer).expect("a board layer index fits in an int, as it does in Java")
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use fr_board::ids::{PadstackId, ViaInfoId};
    use fr_board::prelude::*;
    use fr_board::rules::{ViaInfo, ViaRule};
    use fr_geometry::{IntBox, IntOctagon, IntPoint, IntVector, Point, Polyline, Shape, TileShape};
    use fr_settings::RouterSettings;

    use super::FoundConnectionInserter;
    use crate::autoroute::maze::AutorouteControl;

    const T15: &str = include_str!("../../../tests/data/p6t15-inserter.txt");

        fn section(mode: &str) -> Vec<&'static str> {
        let header = format!("=== mode {mode} ===");
        let mut rows = Vec::new();
        let mut inside = false;
        for line in T15.lines() {
            if line.starts_with("=== mode ") {
                inside = line == header;
                continue;
            }
            if inside {
                rows.push(line.trim_end());
            }
        }
        assert!(!rows.is_empty(), "transcript section `{mode}` is empty");
        rows
    }

    fn assert_rows_match(mode: &str, actual: &[String]) {
        let expected = section(mode);
        let mut diffs = Vec::new();
        for i in 0..expected.len().max(actual.len()) {
            let want = expected.get(i).copied().unwrap_or("<missing>");
            let got = actual.get(i).map(String::as_str).unwrap_or("<missing>");
            if want != got {
                diffs.push(format!("row {i}\n  jvm:  {want}\n  rust: {got}"));
            }
        }
        assert!(
            diffs.is_empty(),
            "mode `{mode}`: {} of {} rows differ\n{}",
            diffs.len(),
            expected.len().max(actual.len()),
            diffs
                .iter()
                .take(12)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

                                        fn neck_board(trace_half_width: i32) -> Board {
        neck_board_widths(trace_half_width, 30, 30)
    }

                                                            fn neck_board_widths(
        class_half_width: i32,
        seed_half_width: i32,
        board_trace_half_width: i32,
    ) -> Board {
        let layers =
            || LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)]);
        let clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
        let mut rules = BoardRules::new(layers(), clearance_matrix);
        rules.trace_angle_restriction = AngleRestriction::None;
        rules.set_default_trace_half_widths(seed_half_width);

        let mut padstacks = Padstacks::new(layers());
        padstacks.add(
            "smd",
            vec![
                Some(Shape::Tile(TileShape::Box(IntBox::from_coords(
                    -50, -50, 50, 50,
                )))),
                None,
            ],
            false,
            false,
        );
        let through_shape = Shape::Tile(TileShape::Octagon(IntOctagon::new(
            -70, -70, 70, 70, -140, 140, -140, 140,
        )));
        padstacks.add(
            "thru",
            vec![Some(through_shape.clone()), Some(through_shape)],
            true,
            false,
        );
        let via_shape = Shape::Tile(TileShape::Octagon(IntOctagon::new(
            -100, -100, 100, 100, -200, 200, -200, 200,
        )));
        let via = padstacks.add(
            "via",
            vec![Some(via_shape.clone()), Some(via_shape)],
            true,
            false,
        );

        rules.via_infos.add(ViaInfo::new("v", via, 1, false));
        let mut via_rule = ViaRule::new("rule");
        via_rule.append_via(rules.via_infos.get(ViaInfoId(0)).clone());
        rules.via_rules.push(via_rule);
        let default_class = rules.get_default_net_class();
        rules
            .net_classes
            .get_mut(default_class)
            .set_via_rule(Some(rules.via_rules[0].clone()));

        let mut board = Board::new(
            Vec::new(),
            0,
            IntBox::from_coords(-1000, -1000, 1000, 1000),
            rules,
            BoardLibrary::new(padstacks, Packages::new()),
            Components::new(),
            Communication::default(),
        );
        let default_class = board.rules.get_default_net_class();
        board.rules.nets.add("N1", 1, false, default_class);
        let pkg = board.library.packages.add(
            "pkg",
            vec![
                PackagePin::new("P1", PadstackId(1), IntVector::new(-400, 0).into(), 0.0),
                PackagePin::new("P2", PadstackId(2), IntVector::new(400, 0).into(), 0.0),
            ],
            None,
            None,
            None,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            true,
        );
        board
            .components
            .add_with_generated_name(Some(Point::new(0, 0)), 0.0, true, pkg);
        board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed); 
        board.insert_pin(1, 1, vec![1], 1, FixedState::Unfixed); 

        board.rules.set_default_trace_half_widths(class_half_width);
        let default_class = board.rules.get_default_net_class();
        board.rules.nets.add("N2", 1, false, default_class);
        board.insert_trace_without_cleaning(
            Polyline::from_points(&[Point::new(0, -900), Point::new(0, 900)]),
            0,
            board_trace_half_width,
            vec![2],
            1,
            FixedState::UserFixed,
        ); 
        board
    }

        fn neck_control(board: &Board) -> AutorouteControl {
        let mut settings = RouterSettings::new();
        settings.set_layer_count(board.get_layer_count());
        settings.apply_board_specific_optimizations(board);
        settings.set_automatic_neckdown(true);
        let trace_costs = settings.get_trace_costs();
        AutorouteControl::new(board, 1, &settings, settings.get_via_costs(), &trace_costs)
    }

    struct Counter {
        calls: Cell<u32>,
    }

    impl Counter {
        fn new() -> Counter {
            Counter {
                calls: Cell::new(0),
            }
        }

        fn check(&self) -> bool {
            self.calls.set(self.calls.get() + 1);
            false
        }
    }

        fn point_of(point: Option<&Point>) -> String {
        match point {
            None => "null".to_string(),
            Some(Point::Int(p)) => format!("({},{})", p.x, p.y),
            Some(other) => {
                let f = other.to_float();
                format!("~({},{})", f.x, f.y)
            }
        }
    }

        fn line_of(line: &fr_geometry::Line) -> String {
        format!("({},{})->({},{})", line.a.x, line.a.y, line.b.x, line.b.y)
    }

            fn poly_of(polyline: &Polyline) -> String {
        let lines: Vec<String> = polyline.lines().iter().map(line_of).collect();
        let corners: Vec<String> = (0..polyline.corner_count())
            .map(|i| match polyline.corner(i) {
                Some(Point::Int(p)) => format!("({},{})", p.x, p.y),
                other => panic!("a rational corner {other:?} the JVM transcript does not carry"),
            })
            .collect();
        format!(
            "n={} lines=[{}] corners=[{}]",
            polyline.lines().len(),
            lines.join(","),
            corners.join(",")
        )
    }

        fn board_dump(board: &Board) -> Vec<String> {
        let ctx = board.ctx();
        let mut out = vec![format!(
            "    maxId={}",
            board.communication.id_gen.max_generated_id()
        )];
        for item in board.get_items() {
            let type_name = match item {
                Item::Trace(_) => "PolylineTrace",
                Item::Pin(_) => "Pin",
                Item::BoardOutline(_) => "BoardOutline",
                other => panic!("the neck fixture holds no {other:?}"),
            };
            let nets: Vec<String> = item.net_nos().iter().map(i32::to_string).collect();
            let mut line = format!(
                "    item id={} type={} nets=[{}] cl={}",
                item.id().0,
                type_name,
                nets.join(","),
                item.clearance_class()
            );
            match item {
                Item::Trace(trace) => line.push_str(&format!(
                    " layer={} hw={} {}",
                    trace.get_layer(),
                    trace.get_half_width(),
                    poly_of(trace.polyline())
                )),
                Item::Pin(pin) => {
                    let center = pin.get_center(&ctx).to_float().round();
                    line.push_str(&format!(" center=({},{})", center.x, center.y));
                }
                _ => {}
            }
            out.push(line);
        }
        out
    }

    const WIDTHS: [i32; 2] = [100, 60];

    const SMD_CENTER: IntPoint = IntPoint { x: -400, y: 0 };
    const THRU_CENTER: IntPoint = IntPoint { x: 400, y: 0 };

                                    #[test]
    fn the_neckdown_retry_matches_the_jvm() {
        let pairs: [(IntPoint, IntPoint); 5] = [
            (IntPoint::new(-200, 0), SMD_CENTER),
            (IntPoint::new(-100, 0), SMD_CENTER),
            (IntPoint::new(200, 0), THRU_CENTER),
            (IntPoint::new(-200, 300), SMD_CENTER),
            (SMD_CENTER, THRU_CENTER),
        ];
        let mut rows = Vec::new();
        for width in WIDTHS {
            for pin_choice in 0..2 {
                for (from, to) in pairs {
                    let mut board = neck_board(width);
                    let ctrl = neck_control(&board);
                    let pin = ItemId(if pin_choice == 0 { 2 } else { 3 });
                    let inserter = FoundConnectionInserter::new();
                    let counter = Counter::new();
                    let from = Point::Int(from);
                    let to = Point::Int(to);
                    let result = inserter
                        .try_neck_down(&mut board, &ctrl, None, &from, &to, 0, pin, true, &|| {
                            counter.check()
                        })
                        .expect("no fixture row reaches an error");
                    rows.push(format!(
                        "tryNeckDown hw={} pin={} from={} to={} result={} isFrom={} isTo={}",
                        width,
                        pin.0,
                        point_of(Some(&from)),
                        point_of(Some(&to)),
                        point_of(result.as_ref()),
                        result.as_ref() == Some(&from),
                        result.as_ref() == Some(&to),
                    ));
                    rows.extend(board_dump(&board));
                }
            }
        }
        let pairs2: [(IntPoint, IntPoint); 3] = [
            (IntPoint::new(-200, 0), THRU_CENTER),
            (SMD_CENTER, IntPoint::new(-310, 0)),
            (IntPoint::new(-310, 0), SMD_CENTER),
        ];
        for width in WIDTHS {
            for (from, to) in pairs2 {
                for start_choice in 0..2 {
                    for end_choice in 0..2 {
                        let mut board = neck_board(width);
                        let ctrl = neck_control(&board);
                        let start_pin = (start_choice != 0).then_some(ItemId(2));
                        let end_pin = (end_choice != 0).then_some(ItemId(3));
                        let inserter = FoundConnectionInserter::new();
                        let counter = Counter::new();
                        let from = Point::Int(from);
                        let to = Point::Int(to);
                        let result = inserter
                            .insert_neckdown(
                                &mut board,
                                &ctrl,
                                None,
                                &from,
                                &to,
                                0,
                                start_pin,
                                end_pin,
                                &|| counter.check(),
                            )
                            .expect("no fixture row reaches an error");
                        rows.push(format!(
                            "insertNeckdown hw={} from={} to={} startPin={} endPin={} result={}",
                            width,
                            point_of(Some(&from)),
                            point_of(Some(&to)),
                            start_pin.map_or_else(|| "null".to_string(), |id| id.0.to_string()),
                            end_pin.map_or_else(|| "null".to_string(), |id| id.0.to_string()),
                            result,
                        ));
                        rows.extend(board_dump(&board));
                    }
                }
            }
        }
        assert_rows_match("neck", &rows);
    }

                                        #[test]
    fn the_micro_neckdown_candidate_order_is_javas_insertion_order() {
        let widths = [100, 66];
        let pairs: [(IntPoint, IntPoint); 2] = [
            (IntPoint::new(-700, 0), SMD_CENTER),
            (IntPoint::new(-300, 0), SMD_CENTER),
        ];
        let mut rows = Vec::new();
        for width in widths {
            for (from, to) in pairs {
                for start_choice in 0..2 {
                    for end_choice in 0..2 {
                        let mut board = neck_board(width);
                        let ctrl = neck_control(&board);
                        let start_pin = (start_choice != 0).then_some(ItemId(2));
                        let end_pin = (end_choice != 0).then_some(ItemId(3));
                        let inserter = FoundConnectionInserter::new();
                        let counter = Counter::new();
                        let from = Point::Int(from);
                        let to = Point::Int(to);
                        let result = inserter
                            .insert_fanout_micro_neckdown(
                                &mut board,
                                &ctrl,
                                None,
                                Some(&from),
                                &to,
                                0,
                                &[1],
                                start_pin,
                                end_pin,
                                &|| counter.check(),
                            )
                            .expect("no fixture row reaches an error");
                        rows.push(format!(
                            "micro base={} from={} to={} startPin={} endPin={} result={}",
                            width,
                            point_of(Some(&from)),
                            point_of(Some(&to)),
                            start_pin.map_or_else(|| "null".to_string(), |id| id.0.to_string()),
                            end_pin.map_or_else(|| "null".to_string(), |id| id.0.to_string()),
                            result,
                        ));
                        rows.extend(board_dump(&board));
                    }
                }
            }
        }
        let mut board = neck_board(100);
        let ctrl = neck_control(&board);
        let inserter = FoundConnectionInserter::new();
        let counter = Counter::new();
        let from = Point::new(-700, 0);
        let same = inserter
            .insert_fanout_micro_neckdown(
                &mut board,
                &ctrl,
                None,
                Some(&from),
                &from,
                0,
                &[1],
                Some(ItemId(2)),
                None,
                &|| counter.check(),
            )
            .expect("the early return cannot fail");
        rows.push(format!("micro same={same}"));
        rows.extend(board_dump(&board));
        assert_rows_match("micro", &rows);
    }


                                                                #[test]
    fn the_micro_neckdown_fallback_never_goes_below_the_rules_minimum() {
        let mut board = neck_board_widths(100, 100, 30);
        assert_eq!(
            board.rules.get_min_trace_half_width(),
            100,
            "the fixture's whole point: the class width IS the rules minimum"
        );
        let ctrl = neck_control(&board);
        assert_eq!(ctrl.trace_half_width[0], 100, "the base half width");

        let items_before = board.get_items().count();
        let inserter = FoundConnectionInserter::new();
        let counter = Counter::new();
        let from = Point::new(-700, 0);
        let to = Point::Int(SMD_CENTER);
        let inserted = inserter
            .insert_fanout_micro_neckdown(
                &mut board,
                &ctrl,
                None,
                Some(&from),
                &to,
                0,
                &[1],
                None,
                None,
                &|| counter.check(),
            )
            .expect("the guarded loop cannot fail");

        assert!(
            !inserted,
            "R2 (#294): 75, 60 and 50 are all below the rules minimum of 100, so every candidate \
             is skipped and the fallback has nothing left to try"
        );
        assert_eq!(
            board.get_items().count(),
            items_before,
            "a rejected candidate must leave no trace behind — before the fix a 75-wide one was \
             inserted here"
        );
    }

                                #[test]
    fn the_fallback_still_necks_down_when_the_class_is_above_the_minimum() {
        let mut board = neck_board_widths(100, 50, 30);
        assert_eq!(board.rules.get_min_trace_half_width(), 50);
        let ctrl = neck_control(&board);
        assert_eq!(ctrl.trace_half_width[0], 100);

        let before: Vec<ItemId> = board.get_items().map(Item::id).collect();
        let inserter = FoundConnectionInserter::new();
        let counter = Counter::new();
        let from = Point::new(-700, 0);
        let to = Point::Int(SMD_CENTER);
        let inserted = inserter
            .insert_fanout_micro_neckdown(
                &mut board,
                &ctrl,
                None,
                Some(&from),
                &to,
                0,
                &[1],
                None,
                None,
                &|| counter.check(),
            )
            .expect("the guarded loop cannot fail");

        assert!(
            inserted,
            "75 clears a minimum of 50, so `:492-508` reaches the target and the fallback \
             succeeds exactly as it did before R2"
        );
        let added: Vec<i32> = board
            .get_items()
            .filter(|item| !before.contains(&item.id()))
            .filter_map(|item| match item {
                Item::Trace(trace) => Some(trace.get_half_width()),
                _ => None,
            })
            .collect();
        assert_eq!(
            added,
            vec![75],
            ":469's `max(1, base * 3 / 4)` is the first element of the LinkedHashSet and is what \
             the loop takes"
        );
    }

                                            #[test]
    fn the_guard_reads_the_rules_minimum_not_the_running_board_minimum() {
        let mut board = neck_board_widths(100, 100, 30);
        board.insert_trace_without_cleaning(
            Polyline::from_points(&[Point::new(-900, -700), Point::new(900, -700)]),
            0,
            40,
            vec![2],
            1,
            FixedState::UserFixed,
        );

        assert_eq!(
            board.rules.get_min_trace_half_width(),
            100,
            "the design rule does not move when a trace is inserted"
        );
        assert_eq!(
            board.get_min_trace_half_width(),
            30,
            "the running board minimum did move — the fixture's 30-wide trace and now a 40-wide \
             one; both sit below every candidate, which is what makes the two readers disagree"
        );

        let ctrl = neck_control(&board);
        let items_before = board.get_items().count();
        let inserter = FoundConnectionInserter::new();
        let counter = Counter::new();
        let from = Point::new(-700, 0);
        let to = Point::Int(SMD_CENTER);
        let inserted = inserter
            .insert_fanout_micro_neckdown(
                &mut board,
                &ctrl,
                None,
                Some(&from),
                &to,
                0,
                &[1],
                None,
                None,
                &|| counter.check(),
            )
            .expect("the guarded loop cannot fail");

        assert!(
            !inserted,
            "the guard must read the rules minimum (100) and reject 75/60/50; reading the \
             board's running minimum (30) would admit every one of them and ratchet the floor \
             down with each insertion"
        );
        assert_eq!(board.get_items().count(), items_before);
    }
}
