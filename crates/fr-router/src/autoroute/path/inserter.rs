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
    /// `private IntPoint lastCorner` (`:27`), written by [`Self::insert_trace`] at `:134` and
    /// `:451` and read at `:74` and `:95`.
    last_corner: Option<IntPoint>,
    /// `private IntPoint firstCorner` (`:28`), written at `:132` and `:449` and read at `:80`.
    first_corner: Option<IntPoint>,
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
        let mut current_layer = connection.target_layer;
        // :46.
        let mut new_instance = FoundConnectionInserter::new();
        for current_new_item in &connection.connection_items {
            // :48-65 is `FRLogger.trace` only. `:49-53`'s `corners.length > 0` guard is
            // defensive: `calculateNextTrace:411` seeds the corner list with `currentFromPoint`
            // before anything else, so a `ResultItem` always has at least one corner and `:66`'s
            // unguarded `corners[0]` cannot throw.
            // :66-68.
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
            // :69.
            current_layer = current_new_item.layer;
            // :70-72.
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
        // :74-76. The last via closes the connection back onto the start item's layer.
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
        if let Some(target_item) = connection.target_item
            && let Some(first_corner) = new_instance.first_corner
        {
            board.connect_to_trace_sized_by_layer(
                &Point::Int(first_corner),
                target_item,
                &ctrl.trace_half_width,
                ctrl.trace_clearance_class_index,
            );
        }
        if let Some(start_item) = connection.start_item
            && let Some(last_corner) = new_instance.last_corner
        {
            board.connect_to_trace_sized_by_layer(
                &Point::Int(last_corner),
                start_item,
                &ctrl.trace_half_width,
                ctrl.trace_clearance_class_index,
            );
        }

        // :108.
        board.normalize_traces_checked(ctrl.net_number, stop)?;

        // :110.
        Ok(Some(new_instance))
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
        // :128-136. "Single-point trace: the start and end are the same location (already at the
        // target). Set both firstCorner and lastCorner so that connect_to_trace is not called
        // with null."
        if trace.corners.len() == 1 {
            if self.first_corner.is_none() {
                self.first_corner = Some(trace.corners[0]);
            }
            self.last_corner = Some(trace.corners[0]);
            return Ok(true);
        }

        // :138-141. "switch off correcting connection to pin because it may get wrong in
        // inserting the polygon line for line."
        let saved_edge_to_turn_dist = board.rules.get_pin_edge_to_turn_dist();
        board.rules.set_pin_edge_to_turn_dist(-1.0);

        // :143-165. "Look for pins att the start and the end of trace in case that neckdown is
        // necessary."
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
                    // :154-155.
                    if item.contains_net(ctrl.net_number)
                        && board.drill_center(id).as_ref() == Some(&current_end_corner)
                    {
                        // :156-160.
                        if i == 0 {
                            start_pin = Some(id);
                        } else {
                            end_pin = Some(id);
                        }
                    }
                }
                // :163.
                current_end_corner = Point::Int(trace.corners[trace.corners.len() - 1]);
            }
        }
        // :166-167.
        let net_numbers = [ctrl.net_number];

        // :169-170.
        let mut from_corner_no = 0usize;
        let mut result = true;
        // :171-405.
        for i in 1..trace.corners.len() {
            // :172-173.
            let current_corner_arr: Vec<Point> = trace.corners[from_corner_no..=i]
                .iter()
                .map(|corner| Point::Int(*corner))
                .collect();
            let insert_polyline = Polyline::from_points(&current_corner_arr);
            // :174 and :189-200 read `maxGeneratedId` for `FRLogger.trace` only.
            // :175-188. `tidyWidth` is `Integer.MAX_VALUE`, `withCheck` true, `timeLimit` null.
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
            // T17: #193's fourth mutation source. It runs *after* the maze search of this
            // connection, so a fire it explains belongs to the **next** connection's search.
            crate::autoroute::instrument::note_mutation(
                crate::autoroute::instrument::Mutation::ForcedTraceInsert,
            );
            let last_corner = insert_polyline.last_corner();
            let first_corner = insert_polyline.first_corner();
            // :201-202.
            let mut neckdown_inserted = false;
            let mut micro_neckdown_inserted = false;
            // :203-209.
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
            // :210-217.
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
                // :218-263 — the ADVANCE arm, whose body below `:219` is `FRLogger.trace` only.
                from_corner_no = i;
            } else if ok_point == first_corner && i != trace.corners.len() - 1 {
                // :264-319. "if okPoint == insertPolyline.firstCorner() the spring over may have
                // failed. Spring over may correct the situation because an insertion, which is ok
                // with clearance compensation may cause violations without clearance
                // compensation. In this case repeating the insertion with more distant corners
                // may allow the spring over to correct the situation."
                if from_corner_no > 0 {
                    // :272-273. "trace.corners[i] may be inside the offset for the substitute
                    // trace around a spring_over obstacle (if clearance compensation is off)."
                    if current_corner_arr.len() < 3 {
                        // :275-276, "first correction".
                        from_corner_no -= 1;
                    }
                }
                // :279-319 is `FRLogger.trace` only.
            } else {
                // :320-404 — the FAIL arm; everything above `:402` is diagnostics.
                result = false;
                break;
            }
        }

        // :407-431. Every corner before the last one may have left a stub behind.
        for i in 0..trace.corners.len() - 1 {
            let corner = Point::Int(trace.corners[i]);
            if let Some(trace_stub) = board.get_trace_tail(&corner, Some(trace.layer), &net_numbers)
            {
                // :411-427 is `FRLogger.trace`; `:428` is the removal.
                board.remove_item(trace_stub);
            }
        }
        // :433-445 is `FRLogger.trace`.

        // :447.
        board
            .rules
            .set_pin_edge_to_turn_dist(saved_edge_to_turn_dist);
        if self.first_corner.is_none() {
            self.first_corner = Some(trace.corners[0]);
        }
        self.last_corner = Some(trace.corners[trace.corners.len() - 1]);
        // :452.
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
        // :457.
        let from_point = ok_point.unwrap_or(target_point);
        // :458-460.
        if from_point == target_point {
            return Ok(false);
        }
        // :461.
        let base_half_width = ctrl.trace_half_width[layer];
        // :462-471. `add` on a `LinkedHashSet` keeps the *first* insertion's position.
        let mut candidate_half_widths: Vec<i32> = Vec::new();
        let add = |value: i32, list: &mut Vec<i32>| {
            if !list.contains(&value) {
                list.push(value);
            }
        };
        let ctx = board.ctx();
        // :463-465.
        if let Some(pin_id) = start_pin
            && let Some(Item::Pin(pin)) = board.get_item(pin_id)
            && pin.is_on_layer(layer, &ctx)
        {
            add(
                pin.get_trace_neckdown_halfwidth(layer, &ctx),
                &mut candidate_half_widths,
            );
        }
        // :466-468.
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

        // :473-509.
        for candidate_half_width in candidate_half_widths {
            // :474-476.
            if candidate_half_width <= 0 || candidate_half_width >= base_half_width {
                continue;
            }
            if candidate_half_width < min_half_width {
                continue;
            }
            // :477-491.
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
        // :510-522 is `traceFanoutDiagnostic` only.
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
        // :526-531. Note the swap: the start pin's neck runs *back* from `toCorner`.
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
        // :532-535.
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
        // :536.
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
        // :540-542.
        if !pin_item.is_on_layer(layer, &ctx) {
            return Ok(None);
        }
        // :543.
        let pin_center = pin_item.get_center(&ctx).to_float();
        let pin_clearance_class = board
            .get_item(pin)
            .expect("the pin was just read")
            .clearance_class();
        // :547.
        let pin_max_width = pin_item.get_max_width(layer, &ctx);
        // :552.
        let neck_down_halfwidth = pin_item.get_trace_neckdown_halfwidth(layer, &ctx);

        // :544-546.
        let current_clearance = f64::from(board.rules.clearance_matrix.get_value(
            ctrl.trace_clearance_class_index,
            pin_clearance_class,
            layer,
            true,
        ));
        // :547.
        let pin_neck_down_distance = 2.0 * (0.5 * pin_max_width + current_clearance);
        // :548-550.
        if pin_center.distance(&to_corner.to_float()) >= pin_neck_down_distance {
            return Ok(None);
        }
        if neck_down_halfwidth >= ctrl.trace_half_width[layer] {
            return Ok(None);
        }

        // :557-558.
        let float_from_corner = from_corner.to_float();
        let float_to_corner = to_corner.to_float();
        // :560.
        let tolerance = 2.0;
        // :562-563.
        let net_numbers = [ctrl.net_number];

        // :565-573.
        let mut ok_length = board.check_trace_segment(
            from_corner,
            to_corner,
            layer,
            &net_numbers,
            ctrl.trace_half_width[layer],
            ctrl.trace_clearance_class_index,
            true,
        );
        // :574-576.
        if ok_length >= f64::from(i32::MAX) {
            return Ok(Some(from_corner.clone()));
        }
        // :577.
        ok_length -= tolerance;
        // :578-660.
        let mut neck_down_end_point: Point;
        if ok_length <= tolerance {
            // :579-580.
            neck_down_end_point = from_corner.clone();
        } else {
            // :582-583.
            let float_neck_down_end_point =
                float_from_corner.change_length(&float_to_corner, ok_length);
            neck_down_end_point = Point::Int(float_neck_down_end_point.round());
            let horizontal_first = (float_from_corner.x - float_neck_down_end_point.x).abs()
                >= (float_from_corner.y - float_neck_down_end_point.y).abs();
            // :589-595.
            let mut add_corner = Point::Int(
                calculate_additional_corner(
                    float_from_corner,
                    float_neck_down_end_point,
                    horizontal_first,
                    board.rules.trace_angle_restriction,
                )
                .round(),
            );
            // :596-613.
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
            // :614-631.
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
            // :632-638.
            add_corner = Point::Int(
                calculate_additional_corner(
                    float_neck_down_end_point,
                    float_to_corner,
                    !horizontal_first,
                    board.rules.trace_angle_restriction,
                )
                .round(),
            );
            // :639-659.
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
                // :658.
                neck_down_end_point = add_corner;
            }
        }

        // :662-675.
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
        // :684-686.
        if input_from_layer == input_to_layer {
            return Ok(true); // no via necessary
        }
        let (from_layer, to_layer) = if input_from_layer < input_to_layer {
            (input_from_layer, input_to_layer)
        } else {
            (input_to_layer, input_from_layer)
        };
        // :697-698.
        let net_numbers = [ctrl.net_number];
        // :699-700.
        let mut via_info = None;
        let mut found_suitable_span = false;
        // :701.
        let via_rule = ctrl
            .via_rule
            .as_ref()
            .expect("FoundConnectionInserter.insertVia:701 dereferences ctrl.viaRule (NPE)");
        let via_count = via_rule.via_count();
        for i in 0..via_count {
            // :702-703.
            let current_via_info = via_rule.get_via(i).clone();
            let current_via_padstack = current_via_info.get_padstack();
            // :704-706.
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
            // :707.
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
        // :721-752. The whole body is `FRLogger.debug` and `traceFanoutDiagnostic`; only the
        // `return false` at `:751` is behaviour. `found_suitable_span` picks between the two
        // diagnostic messages and nothing else.
        let Some(via_info) = via_info else {
            let _ = found_suitable_span;
            return Ok(false);
        };
        // :753-783. "insert the via". Reached only when `:708`'s `check` answered true, which
        // already opened the `Option` above.
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
        // :784.
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
        let mut allowed = 0usize;
        for i in 0..expected.len().max(actual.len()) {
            let want = expected.get(i).copied().unwrap_or("<missing>");
            let got = actual.get(i).map(String::as_str).unwrap_or("<missing>");
            if want != got && !differs_only_by_the_end_closing_line(want, got) {
                diffs.push(format!("row {i}\n  jvm:  {want}\n  rust: {got}"));
            }
            if want != got {
                allowed += 1;
            }
        }
        // The allowance is not a licence: on these two modes it has to keep firing, or #23 has
        // been reverted and these transcripts would go green against a jar they no longer match
        // for the stated reason. Measured: `micro` 16 rows of 118, `neck` 11 of 276.
        assert!(
            allowed > 0,
            "mode `{mode}`: no row differs by #23's end closing line any more — if #23 was \
             reverted, this transcript is no longer saying what it says"
        );
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

    /// `P6T15Probe.buildNeck` — `P6T13Probe.buildSimple` with a wider default trace half width
    /// and a **user-fixed** foreign-net blocker at x = 0.
    ///
    /// Both matter. `tryNeckDown:553` gives up unless the pin's neckdown half width — 49 for the
    /// 100-unit `smd` pad, 69 for the 140-unit `thru` octagon — is strictly below
    /// `ctrl.traceHalfWidth`, and `buildSimple`'s 30 is below both; and with nothing in the way
    /// `checkTraceSegment` (`:566`) answers `Integer.MAX_VALUE`, so `:574-576` returns before
    /// anything is inserted.
    /// `P6T15Probe.buildNeck(traceHalfWidth)` — the probe's fixture, at its own widths.
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
        board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed); // id 2, the smd pin
        board.insert_pin(1, 1, vec![1], 1, FixedState::Unfixed); // id 3, the thru pin

        // `buildNeck`'s own two lines, after `buildSimple`.
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
        ); // id 4
        board
    }

    /// `P6T15Probe.control(board, 1, true)`.
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

    /// `P6T15Probe.ptOf`.
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

    /// `P6T15Probe.ln`.
    fn line_of(line: &fr_geometry::Line) -> String {
        format!("({},{})->({},{})", line.a.x, line.a.y, line.b.x, line.b.y)
    }

    /// `P6T15Probe.poly`. Every corner these fixtures produce is an `IntPoint`, so the rational
    /// arm of `pt` (`~(x,y)`) is unreachable here and asserted away.
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

    /// `P6T15Probe.boardDump`, restricted to the item kinds these two fixtures hold.
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

    /// Probe mode `neck`: `tryNeckDown` (`:539-676`) over the five corner pairs times both pins
    /// times two trace half widths, then `insertNeckdown` (`:525-537`) over three corner pairs
    /// times the four `(startPin, endPin)` combinations.
    ///
    /// The discriminating rows are `pin=3 from=(-400,0) to=(400,0)` at half width 100 — the only
    /// one that reaches the `:578-660` insertion arm, leaving a 100-wide segment and a 69-wide
    /// neck behind — and the four `insertNeckdown … result=true` rows, which are the only ones in
    /// the file where `:662`'s neck segment reaches its target.
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

    /// Probe mode `micro`: `insertFanoutMicroNeckdown` (`:455-523`), which exists here to pin the
    /// **insertion order** of `:462`'s `LinkedHashSet`.
    ///
    /// The far pair leaves room for every candidate, so the inserted trace's half width is
    /// literally the set's first element: 75 with no pins (`[75, 60, 50]`), 69 with the end pin
    /// only (`[69, 75, 60, 50]`), 49 with the start pin. A `BTreeSet` would answer 50, 50 and 49
    /// — so two of those four rows fail the moment the `Vec` is replaced by a sorted set. The
    /// `base=66` rows add the de-duplication: `max(1, 66 * 3 / 4)` is 49, the same value the smd
    /// pin contributed, and the set keeps the **first** position.
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
        // `:457-460`: an equal target is the early false, before the set is built at all.
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

    // =============================================================================================
    // R2 (register row #294) — the micro-neckdown fallback's rules-minimum floor
    //
    // These three are the tests `.superpowers/sdd/…/task-2-brief.md` names against
    // `crates/fr-router/tests/inserter.rs`. They live here for the reason that file's own module
    // doc already gives: `insertFanoutMicroNeckdown` is **private**, so an integration test — a
    // different crate — cannot call it, and the neckdown tests have always lived beside it.
    // =============================================================================================

    /// The whole of R2, on the board shape the report says is "very common": a net class whose
    /// half width **is** the design-rule minimum.
    ///
    /// `neck_board_widths(100, 100, 30)` — class 100, rules minimum 100 — makes every candidate
    /// `:469-471` can produce sub-minimum: `max(1, 100*3/4) = 75`, `max(1, 100*3/5) = 60`,
    /// `max(1, 100/2) = 50`. With no pins there are no other candidates, so the guarded loop
    /// rejects all three, `insertFanoutMicroNeckdown` answers `false`, and **nothing is
    /// inserted** — the connection fails honestly rather than shipping a trace a fabricator will
    /// reject.
    ///
    /// **Fails before the fix**: the unguarded loop takes the first candidate, and the far pair
    /// leaves room for it, so a **75**-wide trace lands on the board and the method answers
    /// `true`. That is exactly the row `the_micro_neckdown_candidate_order_is_javas_insertion_order`
    /// pins against the jar at `base=100 startPin=null endPin=null` — the same mechanism, on a
    /// board whose minimum makes it a defect.
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

    /// The other half of "guard, not revert": the large-board benefit the report measures
    /// (fully-connected 0.17 -> 0.33) is preserved.
    ///
    /// `neck_board_widths(100, 50, 30)` — class 100, rules minimum 50 — leaves all three
    /// candidates legal, so the loop takes the **first** one, `max(1, 100*3/4) = 75`, exactly as
    /// it did before R2. The half width of the trace that lands is the assertion: a guard that
    /// had reordered or filtered the set would show up here as a 60 or a 50.
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
        // The narrow trace, inserted first — on net 2 and out of the way, so it changes nothing
        // but the running minimum.
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

    fn differs_only_by_the_end_closing_line(want: &str, got: &str) -> bool {
        let Some((want_head, want_rest)) = want.split_once("lines=[") else {
            return false;
        };
        let Some((got_head, got_rest)) = got.split_once("lines=[") else {
            return false;
        };
        if want_head != got_head {
            return false;
        }
        let split_tail = |rest: &str| -> Option<(Vec<String>, String)> {
            let end = rest.find("] corners=[")?;
            let lines: Vec<String> = rest[..end].split("),(").map(str::to_string).collect();
            Some((lines, rest[end..].to_string()))
        };
        let (Some((want_lines, want_corners)), Some((got_lines, got_corners))) =
            (split_tail(want_rest), split_tail(got_rest))
        else {
            return false;
        };
        // The corner list — the copper's actual shape — must be identical, and so must the line count.
        if want_corners != got_corners
            || want_lines.len() != got_lines.len()
            || want_lines.is_empty()
        {
            return false;
        }
        // Every line but the last must be identical.
        if want_lines[..want_lines.len() - 1] != got_lines[..got_lines.len() - 1] {
            return false;
        }
        // And the last must be the same line through the same point, running the other way:
        // `a->b` against `a->(2a - b)`.
        let parse = |s: &str| -> Option<[i64; 4]> {
            let s = s.trim_start_matches('(').trim_end_matches(')');
            let (a, b) = s.split_once(")->(")?;
            let (ax, ay) = a.split_once(',')?;
            let (bx, by) = b.split_once(',')?;
            Some([
                ax.parse().ok()?,
                ay.parse().ok()?,
                bx.parse().ok()?,
                by.parse().ok()?,
            ])
        };
        match (
            parse(&want_lines[want_lines.len() - 1]),
            parse(&got_lines[got_lines.len() - 1]),
        ) {
            (Some(w), Some(g)) => {
                w[0] == g[0] && w[1] == g[1] && g[2] == 2 * w[0] - w[2] && g[3] == 2 * w[1] - w[3]
            }
            _ => false,
        }
    }
}
