
mod base;
mod tightener_45;
mod tightener_90;
mod tightener_any_angle;

use std::collections::BTreeSet;

use fr_board::datastructures::StopCheck;
use fr_board::prelude::*;
use fr_geometry::{
    Direction, FloatPoint, IntDirection, IntOctagon, Line, Point, Polyline, Shape, Side, Signum,
    TileShape, java_max,
};
use fr_settings::ExpansionCostFactor;

use crate::autoroute::maze::engine::AutorouteEngine;
use crate::board_ext::{RoutingBoardExt, ViaOptimizer};

fn p7t8b_oca_ledger() -> bool {
    static ON: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| std::env::var_os("P7T8B_OCA").is_some());
    *ON
}

pub(crate) use base::TightenerBase;
pub use tightener_45::TraceTightener45;
pub use tightener_90::TraceTightener90;
pub use tightener_any_angle::TraceTightenerAnyAngle;

pub enum TraceTightener<'a> {
        Ninety(TraceTightener90<'a>),
        FortyFive(TraceTightener45<'a>),
        AnyAngle(TraceTightenerAnyAngle<'a>),
}

impl<'a> TraceTightener<'a> {
                                    #[allow(clippy::too_many_arguments)]
    pub fn get_instance(
        board: &mut Board,
        only_net_no_arr: Vec<i32>,
        clip_shape: Option<IntOctagon>,
        min_translate_dist: i32,
        stoppable_thread: Option<StopCheck<'a>>,
        time_limit: i32,
        keep_point: Option<Point>,
        keep_point_layer: i32,
    ) -> TraceTightener<'a> {
        let angle_restriction = board.rules.trace_angle_restriction;
        let base = TightenerBase::new(
            only_net_no_arr,
            stoppable_thread,
            time_limit,
            keep_point,
            keep_point_layer,
        );
        let mut result = match angle_restriction {
            AngleRestriction::NinetyDegree => TraceTightener::Ninety(TraceTightener90::new(base)),
            AngleRestriction::FortyFiveDegree => {
                TraceTightener::FortyFive(TraceTightener45::new(base))
            }
            AngleRestriction::None => TraceTightener::AnyAngle(TraceTightenerAnyAngle::new(base)),
        };
        result.base_mut().current_clip_shape = clip_shape;
        result.base_mut().min_translate_dist = min_translate_dist.max(100);
        result
    }

        pub(crate) fn base(&self) -> &TightenerBase<'a> {
        match self {
            TraceTightener::Ninety(t) => &t.base,
            TraceTightener::FortyFive(t) => &t.base,
            TraceTightener::AnyAngle(t) => &t.base,
        }
    }

        pub(crate) fn base_mut(&mut self) -> &mut TightenerBase<'a> {
        match self {
            TraceTightener::Ninety(t) => &mut t.base,
            TraceTightener::FortyFive(t) => &mut t.base,
            TraceTightener::AnyAngle(t) => &mut t.base,
        }
    }

        pub fn only_net_no_arr(&self) -> &[i32] {
        &self.base().only_net_no_arr
    }

        pub fn min_translate_dist(&self) -> i32 {
        self.base().min_translate_dist
    }

                                                                                                            pub fn opt_changed_area(
        &mut self,
        board: &mut Board,
        mut engine: Option<&mut AutorouteEngine>,
        trace_costs: Option<&[ExpansionCostFactor]>,
    ) -> Result<(), BoardError> {
        if board.changed_area.is_none() {
            return Ok(());
        }
        let mut something_changed = true;
        while something_changed {
            something_changed = false;
            for i in 0..board.get_layer_count() {
                let Some(changed_area) = &board.changed_area else {
                    return Ok(());
                };
                let changed_region = changed_area.get_area(i);
                if changed_region.is_empty() {
                    continue;
                }
                if let Some(changed_area) = &mut board.changed_area {
                    changed_area.set_empty(i);
                }
                let changed_area_offset = 1.5
                    * f64::from(
                        board.rules.clearance_matrix.max_value_on_layer(i)
                            + 2 * board.rules.get_max_trace_half_width(),
                    );
                let changed_region = changed_region.enlarge(changed_area_offset);
                let items = board.overlapping_objects(&TileShape::Octagon(changed_region), Some(i));
                if p7t8b_oca_ledger() {
                    let objs = items
                        .iter()
                        .map(|o| match o {
                            TreeObject::Item(id) => id.0.to_string(),
                            _ => "R".to_string(),
                        })
                        .collect::<Vec<_>>()
                        .join(",");
                    eprintln!(
                        "OCA layer={i} region=({},{},{},{},{},{},{},{}) n={} objs=[{objs}]",
                        changed_region.left_x,
                        changed_region.bottom_y,
                        changed_region.right_x,
                        changed_region.top_y,
                        changed_region.upper_left_diagonal_x,
                        changed_region.lower_right_diagonal_x,
                        changed_region.lower_left_diagonal_x,
                        changed_region.upper_right_diagonal_x,
                        items.len(),
                    );
                }
                for current_object in items {
                    if self.base().is_stop_requested() {
                        return Ok(());
                    }
                    match current_object {
                        TreeObject::Item(item_id)
                            if matches!(board.items.get(&item_id), Some(Item::Trace(_))) =>
                        {
                            let pulled = <Board as PolylineTraceExt>::pull_tight_with_engine(
                                board,
                                item_id,
                                self,
                                engine.as_deref_mut(),
                            );
                            if p7t8b_oca_ledger() {
                                eprintln!("OCAPT id={} res={pulled}", item_id.0);
                            }
                            if pulled {
                                something_changed = true;
                                if self.split_traces_at_keep_point(board)? {
                                    break;
                                }
                            } else {
                                let smoothed =
                                    self.smoothen_end_corners_at_trace(board, item_id)?;
                                if p7t8b_oca_ledger() {
                                    eprintln!("OCASM id={} res={smoothed}", item_id.0);
                                }
                                if smoothed {
                                    something_changed = true;
                                    break;
                                }
                            }
                        }
                        TreeObject::Item(via_id)
                            if trace_costs.is_some()
                                && matches!(board.items.get(&via_id), Some(Item::Via(_))) =>
                        {
                            let unchanged_trace_costs = trace_costs.expect("just matched");
                            let moved = ViaOptimizer::opt_via_location(
                                board,
                                via_id,
                                Some(unchanged_trace_costs),
                                self.base().min_translate_dist,
                                10,
                            )?;
                            if p7t8b_oca_ledger() {
                                eprintln!("OCAVIA id={} res={moved}", via_id.0);
                            }
                            if moved {
                                something_changed = true;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }

                                        pub(crate) fn pull_tight_opt(
        &mut self,
        board: &mut Board,
        polyline: &Polyline,
    ) -> Option<Polyline> {
        match self {
            TraceTightener::Ninety(t) => t.pull_tight(board, polyline),
            TraceTightener::FortyFive(t) => t.pull_tight(board, polyline),
            TraceTightener::AnyAngle(t) => t.pull_tight(board, polyline),
        }
    }

                                #[allow(clippy::too_many_arguments)]
    pub fn pull_tight_polyline(
        &mut self,
        board: &mut Board,
        polyline: &Polyline,
        layer: usize,
        half_width: i32,
        net_numbers: &[i32],
        clearance_class_index: usize,
        contact_pins: Option<BTreeSet<ItemId>>,
    ) -> Polyline {
        match self.pull_tight_polyline_opt(
            board,
            polyline,
            layer,
            half_width,
            net_numbers,
            clearance_class_index,
            contact_pins,
        ) {
            Some(tightened) => tightened,
            None => polyline.clone(),
        }
    }

        #[allow(clippy::too_many_arguments)]
    pub(crate) fn pull_tight_polyline_opt(
        &mut self,
        board: &mut Board,
        polyline: &Polyline,
        layer: usize,
        half_width: i32,
        net_numbers: &[i32],
        clearance_class_index: usize,
        contact_pins: Option<BTreeSet<ItemId>>,
    ) -> Option<Polyline> {
        let compensation = board.trees.get_default_tree().clearance_compensation_value(
            clearance_class_index,
            layer,
            &board.rules,
        );
        let base = self.base_mut();
        base.current_layer = layer;
        base.current_half_width = half_width + compensation;
        base.current_net_numbers = net_numbers.to_vec();
        base.current_clearance_class_index = clearance_class_index;
        base.contact_pins = contact_pins;
        self.pull_tight_opt(board, polyline)
    }

                    pub fn reposition_lines(&mut self, board: &mut Board, polyline: &Polyline) -> Option<Polyline> {
        match self {
            TraceTightener::Ninety(t) => t.base.reposition_lines(board, polyline),
            TraceTightener::FortyFive(t) => t.base.reposition_lines(board, polyline),
            TraceTightener::AnyAngle(t) => t.reposition_lines(board, polyline),
        }
    }

                pub fn reposition_line(
        &mut self,
        board: &mut Board,
        lines: &[Line],
        no: usize,
    ) -> Option<Line> {
        match self {
            TraceTightener::Ninety(t) => t.base.reposition_line(board, lines, no),
            TraceTightener::FortyFive(t) => t.base.reposition_line(board, lines, no),
            TraceTightener::AnyAngle(t) => t.reposition_line(board, lines, no),
        }
    }

            pub fn skip_segments_of_length_0(
        &mut self,
        board: &mut Board,
        polyline: &Polyline,
    ) -> Option<Polyline> {
        self.base_mut().skip_segments_of_length_0(board, polyline)
    }

        pub fn split_traces_at_keep_point(&mut self, board: &mut Board) -> Result<bool, BoardError> {
        self.base_mut().split_traces_at_keep_point(board)
    }

                                                            pub fn smoothen_start_corner_at_trace(
        &mut self,
        board: &mut Board,
        trace: ItemId,
    ) -> Option<Polyline> {
        match self {
            TraceTightener::Ninety(t) => t.smoothen_start_corner_at_trace(board, trace),
            TraceTightener::FortyFive(t) => t.smoothen_start_corner_at_trace(board, trace),
            TraceTightener::AnyAngle(t) => t.smoothen_start_corner_at_trace(board, trace),
        }
    }

                pub fn smoothen_end_corner_at_trace(
        &mut self,
        board: &mut Board,
        trace: ItemId,
    ) -> Option<Polyline> {
        match self {
            TraceTightener::Ninety(t) => t.smoothen_end_corner_at_trace(board, trace),
            TraceTightener::FortyFive(t) => t.smoothen_end_corner_at_trace(board, trace),
            TraceTightener::AnyAngle(t) => t.smoothen_end_corner_at_trace(board, trace),
        }
    }

            pub fn smoothen_end_corners_at_trace(
        &mut self,
        board: &mut Board,
        trace: ItemId,
    ) -> Result<bool, BoardError> {
        let Some(Item::Trace(polyline_trace)) = board.items.get(&trace) else {
            return Ok(false);
        };
        if !self.base().only_net_no_arr.is_empty()
            && !polyline_trace.hdr.nets_equal(&self.base().only_net_no_arr)
        {
            return Ok(false);
        }
        let layer = polyline_trace.get_layer();
        let half_width = polyline_trace.get_half_width();
        let net_numbers = polyline_trace.hdr.net_nos.clone();
        let clearance_class_index = polyline_trace.hdr.clearance_class();
        let base = self.base_mut();
        base.current_layer = layer;
        base.current_half_width = half_width;
        base.current_net_numbers = net_numbers;
        base.current_clearance_class_index = clearance_class_index;
        self.smoothen_end_corners_at_trace_1(board, trace)
    }

                                        fn smoothen_end_corners_at_trace_1(
        &mut self,
        board: &mut Board,
        trace: ItemId,
    ) -> Result<bool, BoardError> {
        let Some(item) = board.items.get(&trace) else {
            return Ok(false);
        };
        if item.is_shove_fixed(&board.rules) {
            return Ok(false);
        }
        let saved_contact_pins = self.base_mut().contact_pins.take();
        let mut result = false;
        let mut connection_to_trace_improved = true;
        let mut current_trace = trace;
        let mut p7t8b_iter = 0;
        while connection_to_trace_improved {
            connection_to_trace_improved = false;
            let adjusted = self.smoothen_end_corners_at_trace_2(board, current_trace);
            if p7t8b_oca_ledger() {
                p7t8b_iter += 1;
                eprintln!(
                    "SM1 iter={p7t8b_iter} trace={} adj={}",
                    current_trace.0,
                    adjusted
                        .as_ref()
                        .map_or_else(|| "none".to_string(), |p| p.corner_count().to_string())
                );
            }
            let Some(adjusted_polyline) = adjusted else {
                continue;
            };
            let Some(Item::Trace(t)) = board.items.get(&current_trace) else {
                continue;
            };
            let trace_layer = t.get_layer();
            let current_cl_class = t.hdr.clearance_class();
            let current_fixed_state = t.hdr.get_fixed_state();
            let net_numbers = t.hdr.net_nos.clone();
            let current_half_width = self.base().current_half_width;
            board.remove_item(current_trace);
            let adj_ins_trace = board.insert_trace_without_cleaning(
                adjusted_polyline.clone(),
                trace_layer,
                current_half_width,
                net_numbers,
                current_cl_class,
                current_fixed_state,
            );
            if let Some(adj_ins_trace) = adj_ins_trace {
                result = true;
                connection_to_trace_improved = true;
                board.remove_item(current_trace);
                current_trace = adj_ins_trace;
                let net_numbers = match board.items.get(&current_trace) {
                    Some(item) => item.net_nos().to_vec(),
                    None => Vec::new(),
                };
                for current_net_number in net_numbers {
                    let first_corner = adjusted_polyline
                        .first_corner()
                        .expect("an inserted trace has a first corner");
                    let last_corner = adjusted_polyline
                        .last_corner()
                        .expect("an inserted trace has a last corner");
                    let stop = self.base().stop_check();
                    if p7t8b_oca_ledger() {
                        eprintln!(
                            "SMSPL which=first pt={first_corner:?} layer={trace_layer} \
                             net={current_net_number}"
                        );
                    }
                    board.split_traces_checked(
                        &first_corner,
                        trace_layer,
                        current_net_number,
                        stop,
                    )?;
                    if p7t8b_oca_ledger() {
                        eprintln!(
                            "SMSPL which=last pt={last_corner:?} layer={trace_layer} \
                             net={current_net_number}"
                        );
                    }
                    board.split_traces_checked(
                        &last_corner,
                        trace_layer,
                        current_net_number,
                        stop,
                    )?;
                    if let Err(fr_board::BoardError::Stopped) =
                        board.normalize_traces_checked(current_net_number, stop)
                    {
                        return Err(fr_board::BoardError::Stopped);
                    }
                    if self.base_mut().split_traces_at_keep_point(board)? {
                        return Ok(true);
                    }
                }
            }
        }
        self.base_mut().contact_pins = saved_contact_pins;
        Ok(result)
    }

                fn smoothen_end_corners_at_trace_2(
        &mut self,
        board: &mut Board,
        trace: ItemId,
    ) -> Option<Polyline> {
        let on_the_board = board
            .items
            .get(&trace)
            .is_some_and(|item| item.is_on_the_board());
        if !on_the_board {
            return None;
        }
        let mut result = self.smoothen_start_corner_at_trace(board, trace);
        let layer = self.base().current_layer;
        match &result {
            None => {
                result = self.smoothen_end_corner_at_trace(board, trace);
                if let Some(end_result) = &result
                    && board.changed_area.is_some()
                {
                    let corner = end_result
                        .corner_approx(end_result.corner_count() - 1)
                        .expect("cornerCount - 1 is a corner index");
                    board.join_changed_area(&corner, layer);
                }
            }
            Some(start_result) => {
                if board.changed_area.is_some() {
                    let corner = start_result
                        .corner_approx(0)
                        .expect("a polyline has a first corner");
                    board.join_changed_area(&corner, layer);
                }
            }
        }
        let result = result?;
        self.base_mut().contact_pins = Some(board.touching_pins_at_end_corners(trace));
        Some(
            self.base_mut()
                .skip_segments_of_length_0(board, &result)
                .unwrap_or(result),
        )
    }
}


pub(crate) struct ContactScan {
    pub(crate) acute_angle: bool,
    pub(crate) bend: bool,
    pub(crate) other_trace_corner_approx: Option<FloatPoint>,
    pub(crate) other_trace_line: Option<Line>,
    pub(crate) other_prev_trace_line: Option<Line>,
    pub(crate) prev_corner_side: Option<Side>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn scan_contacts(
    board: &Board,
    contacts: &BTreeSet<ItemId>,
    trace_polyline: &Polyline,
    current_end_corner: &Point,
    current_prev_end_corner: &Point,
    line_direction: &IntDirection,
    prev_line_direction: &IntDirection,
    require_orthogonal: bool,
    require_contact_corner_count_gt_2: bool,
) -> Option<ContactScan> {
    let mut scan = ContactScan {
        acute_angle: false,
        bend: false,
        other_trace_corner_approx: None,
        other_trace_line: None,
        other_prev_trace_line: None,
        prev_corner_side: None,
    };
    if p7t8b_oca_ledger() {
        eprintln!(
            "SSC c0={current_end_corner:?} c1={current_prev_end_corner:?} \
             ldir={line_direction:?} pldir={prev_line_direction:?} contacts=[{}]",
            contacts
                .iter()
                .map(|c| c.0.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
    }
    for current_contact in contacts.iter().rev() {
        let item = board.items.get(current_contact)?;
        let Item::Trace(contact_trace) = item else {
            return None;
        };
        if item.is_shove_fixed(&board.rules) {
            return None;
        }
        let contact_trace_polyline = contact_trace.polyline().clone();
        if require_contact_corner_count_gt_2 && contact_trace_polyline.corner_count() <= 2 {
            continue;
        }
        let (
            current_other_trace_corner_approx,
            current_other_trace_line,
            current_other_prev_trace_line,
        ) = if contact_trace_polyline.first_corner().as_ref() == Some(current_end_corner) {
            (
                contact_trace_polyline
                    .corner_approx(1)
                    .expect("a polyline has a second corner"),
                contact_trace_polyline.lines()[1],
                contact_trace_polyline.lines()[2],
            )
        } else {
            let current_corner_no = contact_trace_polyline.corner_count() - 2;
            (
                contact_trace_polyline
                    .corner_approx(current_corner_no)
                    .expect("cornerCount - 2 is a corner index"),
                contact_trace_polyline.lines()[current_corner_no + 1].opposite(),
                contact_trace_polyline.lines()[current_corner_no],
            )
        };
        let current_prev_corner_side =
            current_prev_end_corner.side_of_line(&current_other_trace_line);
        let current_projection = line_direction.projection(&current_other_trace_line.direction());
        let mut other_trace_found = false;
        if current_projection == Signum::Positive && current_prev_corner_side != Side::Collinear {
            if !require_orthogonal || current_other_trace_line.direction().is_orthogonal() {
                scan.acute_angle = true;
                other_trace_found = true;
            }
        } else if current_projection == Signum::Zero
            && trace_polyline.corner_count() > 2
            && prev_line_direction.projection(&current_other_trace_line.direction())
                == Signum::Positive
        {
            scan.bend = true;
            other_trace_found = true;
        }
        if p7t8b_oca_ledger() {
            eprintln!(
                "SSCC id={} otl={} optl={} approx={:?} side={:?} proj={:?} orth={} \
                 acute={} bend={} found={other_trace_found}",
                current_contact.0,
                p7t8b_line(&current_other_trace_line),
                p7t8b_line(&current_other_prev_trace_line),
                current_other_trace_corner_approx,
                current_prev_corner_side,
                current_projection,
                current_other_trace_line.direction().is_orthogonal(),
                scan.acute_angle,
                scan.bend,
            );
        }
        if other_trace_found {
            scan.other_trace_corner_approx = Some(current_other_trace_corner_approx);
            scan.other_trace_line = Some(current_other_trace_line);
            scan.prev_corner_side = Some(current_prev_corner_side);
            scan.other_prev_trace_line = Some(current_other_prev_trace_line);
        }
    }
    if p7t8b_oca_ledger() {
        eprintln!(
            "SSCF acute={} bend={} otl={} side={:?} approx={:?}",
            scan.acute_angle,
            scan.bend,
            scan.other_trace_line
                .map_or_else(|| "null".to_string(), |l| p7t8b_line(&l)),
            scan.prev_corner_side,
            scan.other_trace_corner_approx,
        );
    }
    Some(scan)
}

pub(crate) fn p7t8b_line(l: &Line) -> String {
    format!("({},{})-({},{})", l.a.x, l.a.y, l.b.x, l.b.y)
}


pub trait PolylineTraceExt {
                                fn pull_tight_with(board: &mut Board, trace: ItemId, algo: &mut TraceTightener<'_>) -> bool;

                fn pull_tight_with_engine(
        board: &mut Board,
        trace: ItemId,
        algo: &mut TraceTightener<'_>,
        engine: Option<&mut AutorouteEngine>,
    ) -> bool;

                fn pull_tight(
        board: &mut Board,
        trace: ItemId,
        own_net_only: bool,
        pull_tight_accuracy: i32,
        stop: StopCheck<'_>,
    ) -> Result<bool, BoardError>;

                                                                fn check_connection_to_pin(board: &Board, trace: ItemId, at_start: bool) -> bool;

                                                        fn correct_connection_to_pin(
        board: &mut Board,
        engine: Option<&mut AutorouteEngine>,
        trace: ItemId,
        at_start: bool,
        angle_restriction: AngleRestriction,
    ) -> Result<bool, BoardError>;

                                                                fn swap_connection_to_pin(
        board: &mut Board,
        engine: Option<&mut AutorouteEngine>,
        trace: ItemId,
        at_start: bool,
    ) -> Result<bool, BoardError>;
}

impl PolylineTraceExt for Board {
    fn pull_tight_with(board: &mut Board, trace: ItemId, algo: &mut TraceTightener<'_>) -> bool {
        <Board as PolylineTraceExt>::pull_tight_with_engine(board, trace, algo, None)
    }

    fn pull_tight_with_engine(
        board: &mut Board,
        trace: ItemId,
        algo: &mut TraceTightener<'_>,
        mut engine: Option<&mut AutorouteEngine>,
    ) -> bool {
        let Some(item) = board.items.get(&trace) else {
            return false;
        };
        if !item.is_on_the_board() {
            return false;
        }
        if item.is_shove_fixed(&board.rules) {
            return false;
        }
        if !item.nets_normal() {
            return false;
        }
        if !algo.base().only_net_no_arr.is_empty()
            && !item.nets_equal_to(&algo.base().only_net_no_arr)
        {
            return false;
        }
        let net_numbers = item.net_nos().to_vec();
        if let Some(first_net) = net_numbers.first() {
            let net = board.rules.nets.get(*first_net).expect(
                "PolylineTrace.pullTight:825: nets.get(netNumbers[0]) is null — Java throws a \
                 NullPointerException at .getNetClass()",
            );
            let net_class = net.get_net_class();
            if !board.rules.net_classes.get(net_class).get_pull_tight() {
                return false;
            }
        }
        let Some(Item::Trace(polyline_trace)) = board.items.get(&trace) else {
            return false;
        };
        let lines = polyline_trace.polyline().clone();
        let layer = polyline_trace.get_layer();
        let half_width = polyline_trace.get_half_width();
        let clearance_class_index = polyline_trace.hdr.clearance_class();
        let contact_pins = board.touching_pins_at_end_corners(trace);
        let new_lines = algo.pull_tight_polyline_opt(
            board,
            &lines,
            layer,
            half_width,
            &net_numbers,
            clearance_class_index,
            Some(contact_pins),
        );
        if let Some(new_lines) = new_lines {
            if let Some(engine) = engine.as_deref_mut()
                && board.items.get(&trace).is_some_and(Item::is_on_the_board)
            {
                board.additional_update_after_change(engine, trace);
            }
            board.change_trace(trace, new_lines);
            return true;
        }
        let angle_restriction = board.rules.trace_angle_restriction;
        if angle_restriction != AngleRestriction::NinetyDegree
            && board.rules.get_pin_edge_to_turn_dist() > 0.0
        {
            if <Board as PolylineTraceExt>::swap_connection_to_pin(
                board,
                engine.as_deref_mut(),
                trace,
                true,
            )
            .unwrap_or(true)
            {
                <Board as PolylineTraceExt>::pull_tight_with_engine(
                    board,
                    trace,
                    algo,
                    engine.as_deref_mut(),
                );
                return true;
            }
            if <Board as PolylineTraceExt>::swap_connection_to_pin(
                board,
                engine.as_deref_mut(),
                trace,
                false,
            )
            .unwrap_or(true)
            {
                <Board as PolylineTraceExt>::pull_tight_with_engine(
                    board,
                    trace,
                    algo,
                    engine.as_deref_mut(),
                );
                return true;
            }
            if <Board as PolylineTraceExt>::correct_connection_to_pin(
                board,
                engine.as_deref_mut(),
                trace,
                true,
                angle_restriction,
            )
            .unwrap_or(true)
            {
                <Board as PolylineTraceExt>::pull_tight_with_engine(
                    board,
                    trace,
                    algo,
                    engine.as_deref_mut(),
                );
                return true;
            }
            if <Board as PolylineTraceExt>::correct_connection_to_pin(
                board,
                engine.as_deref_mut(),
                trace,
                false,
                angle_restriction,
            )
            .unwrap_or(true)
            {
                <Board as PolylineTraceExt>::pull_tight_with_engine(board, trace, algo, engine);
                return true;
            }
        }
        false
    }

    fn pull_tight(
        board: &mut Board,
        trace: ItemId,
        own_net_only: bool,
        pull_tight_accuracy: i32,
        stop: StopCheck<'_>,
    ) -> Result<bool, BoardError> {
        let opt_net_no_arr = if own_net_only {
            match board.items.get(&trace) {
                Some(item) => item.net_nos().to_vec(),
                None => return Ok(false),
            }
        } else {
            Vec::new()
        };
        let mut pull_tight_algo = TraceTightener::get_instance(
            board,
            opt_net_no_arr,
            None,
            pull_tight_accuracy,
            Some(stop),
            -1,
            None,
            -1,
        );
        Ok(<Board as PolylineTraceExt>::pull_tight_with(
            board,
            trace,
            &mut pull_tight_algo,
        ))
    }

    fn check_connection_to_pin(board: &Board, trace: ItemId, at_start: bool) -> bool {
        let Some(Item::Trace(polyline_trace)) = board.items.get(&trace) else {
            return true;
        };
        if polyline_trace.corner_count() < 2 {
            return true;
        }
        let layer = polyline_trace.get_layer();
        let half_width = polyline_trace.get_half_width();
        let clearance_class_index = polyline_trace.hdr.clearance_class();
        let trace_polyline = polyline_trace.polyline().clone();
        let contact_list = if at_start {
            board.trace_start_contacts(trace)
        } else {
            board.trace_end_contacts(trace)
        };
        let Some(contact_pin_id) = contact_list
            .into_iter()
            .rev()
            .find(|id| matches!(board.items.get(id), Some(Item::Pin(_))))
        else {
            return true;
        };
        let Some(Item::Pin(contact_pin)) = board.items.get(&contact_pin_id) else {
            unreachable!("just matched")
        };
        let pin_clearance_class_index = contact_pin.hdr.clearance_class();
        let ctx = board.ctx();
        let trace_exit_restrictions = contact_pin.get_trace_exit_restrictions(layer, &ctx);
        if trace_exit_restrictions.is_empty() {
            return true;
        }
        let (end_corner, prev_end_corner) = if at_start {
            (trace_polyline.first_corner(), trace_polyline.corner(1))
        } else {
            (
                trace_polyline.last_corner(),
                trace_polyline.corner(trace_polyline.corner_count() - 2),
            )
        };
        let (Some(end_corner), Some(prev_end_corner)) = (end_corner, prev_end_corner) else {
            return true;
        };
        let Some(trace_end_direction) = Direction::between(&end_corner, &prev_end_corner) else {
            return true;
        };
        let Some(matching_exit_restriction) = trace_exit_restrictions
            .iter()
            .find(|restriction| restriction.direction == trace_end_direction)
        else {
            return false;
        };
        let edge_to_turn_dist = board.rules.get_pin_edge_to_turn_dist();
        if edge_to_turn_dist < 0.0 {
            return false;
        }
        let end_line_length = end_corner.to_float().distance(&prev_end_corner.to_float());
        let current_clearance = f64::from(board.clearance_value(
            clearance_class_index,
            pin_clearance_class_index,
            layer,
        ));
        let add_width = java_max(edge_to_turn_dist, current_clearance + 1.0);
        let preserve_length =
            matching_exit_restriction.min_length + f64::from(half_width) + add_width;
        preserve_length <= end_line_length
    }

    fn correct_connection_to_pin(
        board: &mut Board,
        engine: Option<&mut AutorouteEngine>,
        trace: ItemId,
        at_start: bool,
        angle_restriction: AngleRestriction,
    ) -> Result<bool, BoardError> {
        if <Board as PolylineTraceExt>::check_connection_to_pin(board, trace, at_start) {
            return Ok(false);
        }
        let Some(Item::Trace(polyline_trace)) = board.items.get(&trace) else {
            return Ok(false);
        };
        let layer = polyline_trace.get_layer();
        let half_width = polyline_trace.get_half_width();
        let clearance_class_index = polyline_trace.hdr.clearance_class();
        let net_numbers = polyline_trace.hdr.net_nos.clone();
        let (trace_polyline, contact_list) = if at_start {
            (
                polyline_trace.polyline().clone(),
                board.trace_start_contacts(trace),
            )
        } else {
            let reversed = match polyline_trace.polyline().reverse() {
                Ok(reversed) => reversed,
                Err(_) => return Ok(false),
            };
            (reversed, board.trace_end_contacts(trace))
        };
        let Some(contact_pin_id) = contact_list
            .into_iter()
            .rev()
            .find(|id| matches!(board.items.get(id), Some(Item::Pin(_))))
        else {
            return Ok(false);
        };
        let Some(Item::Pin(contact_pin)) = board.items.get(&contact_pin_id) else {
            unreachable!("just matched")
        };
        let pin_clearance_class_index = contact_pin.hdr.clearance_class();
        let pin_center = contact_pin.get_center(&board.ctx());
        let ctx = board.ctx();
        let trace_exit_restrictions = contact_pin.get_trace_exit_restrictions(layer, &ctx);
        if trace_exit_restrictions.is_empty() {
            return Ok(false);
        }
        let pin_first_layer = contact_pin.first_layer(&ctx);
        let Some(pad_index) = layer.checked_sub(pin_first_layer) else {
            return Ok(false);
        };
        let Some(Shape::Tile(pin_shape)) = contact_pin.get_shape(pad_index, &ctx) else {
            return Ok(false);
        };
        let edge_to_turn_dist = board.rules.get_pin_edge_to_turn_dist();
        if edge_to_turn_dist < 0.0 {
            return Ok(false);
        }
        let current_clearance = f64::from(board.clearance_value(
            clearance_class_index,
            pin_clearance_class_index,
            layer,
        ));
        let add_width = java_max(edge_to_turn_dist, current_clearance + 1.0);
        let mut offset_pin_shape = pin_shape.offset(f64::from(half_width) + add_width);
        if angle_restriction == AngleRestriction::NinetyDegree || offset_pin_shape.is_int_box() {
            offset_pin_shape = TileShape::Box(offset_pin_shape.bounding_box());
        } else if angle_restriction == AngleRestriction::FortyFiveDegree {
            match offset_pin_shape.bounding_octagon() {
                Some(octagon) => offset_pin_shape = TileShape::Octagon(octagon),
                None => return Ok(false),
            }
        }
        let entries = offset_pin_shape.entrance_points(&trace_polyline);
        let Some(latest_entry_tuple) = entries.last().copied() else {
            return Ok(false);
        };
        let Some(entry_border_line) = offset_pin_shape.border_line(latest_entry_tuple[1]) else {
            return Ok(false);
        };
        let trace_entry_location_approx =
            trace_polyline.lines()[latest_entry_tuple[0]].intersection_approx(&entry_border_line);
        let mut min_exit_corner_distance = f64::MAX;
        let mut nearest_pin_exit_ray: Option<Line> = None;
        let mut nearest_border_line_no: usize = 0;
        let mut pin_exit_direction: Option<Direction> = None;
        let mut nearest_exit_corner: Option<FloatPoint> = None;
        let tolerance = 1.0_f64;
        for current_exit_restriction in &trace_exit_restrictions {
            let Some(current_intersecting_border_line_no) = offset_pin_shape
                .intersecting_border_line_no(&pin_center, &current_exit_restriction.direction)
            else {
                continue;
            };
            let Point::Int(pin_center_int) = pin_center else {
                return Ok(false);
            };
            let Some(current_pin_exit_ray) =
                Line::from_direction_any(pin_center_int, &current_exit_restriction.direction)
            else {
                continue;
            };
            let Some(current_border_line) =
                offset_pin_shape.border_line(current_intersecting_border_line_no)
            else {
                continue;
            };
            let current_exit_corner =
                current_pin_exit_ray.intersection_approx(&current_border_line);
            let current_exit_corner_distance =
                current_exit_corner.distance_square(&trace_entry_location_approx);
            let mut new_nearest_corner_found = false;
            if current_exit_corner_distance + tolerance < min_exit_corner_distance {
                new_nearest_corner_found = true;
            } else if current_exit_corner_distance < min_exit_corner_distance + tolerance {
                let Some(old_exit_corner) = nearest_exit_corner else {
                    unreachable!(
                        "PolylineTrace.correctConnectionToPin:1174 dereferences a null \
                         nearestExitCorner — unreachable while minExitCornerDistance is \
                         Double.MAX_VALUE"
                    )
                };
                for i in 1..trace_polyline.corner_count() {
                    let Some(current_trace_corner) = trace_polyline.corner_approx(i) else {
                        break;
                    };
                    let current_trace_corner_distance =
                        current_trace_corner.distance_square(&current_exit_corner);
                    let old_trace_corner_distance =
                        current_trace_corner.distance_square(&old_exit_corner);
                    if current_trace_corner_distance + tolerance < old_trace_corner_distance {
                        new_nearest_corner_found = true;
                        break;
                    } else if current_trace_corner_distance > old_trace_corner_distance + tolerance
                    {
                        break;
                    }
                }
            }
            if new_nearest_corner_found {
                min_exit_corner_distance = current_exit_corner_distance;
                nearest_pin_exit_ray = Some(current_pin_exit_ray);
                nearest_border_line_no = current_intersecting_border_line_no;
                pin_exit_direction = Some(current_exit_restriction.direction.clone());
                nearest_exit_corner = Some(current_exit_corner);
            }
        }
        let (Some(nearest_pin_exit_ray), Some(pin_exit_direction)) =
            (nearest_pin_exit_ray, pin_exit_direction)
        else {
            return Ok(false);
        };
        let corner_count = offset_pin_shape.border_line_count();
        let clock_wise_side_diff =
            (nearest_border_line_no + corner_count - latest_entry_tuple[1]) % corner_count;
        let counter_clock_wise_side_diff =
            (latest_entry_tuple[1] + corner_count - nearest_border_line_no) % corner_count;
        let mut current_border_line_no = nearest_border_line_no;
        let mut current_lines: Vec<Option<Line>>;
        if counter_clock_wise_side_diff <= clock_wise_side_diff {
            current_lines = vec![None; counter_clock_wise_side_diff + 3];
            for i in 0..=counter_clock_wise_side_diff {
                current_lines[i + 1] = offset_pin_shape.border_line(current_border_line_no);
                current_border_line_no = (current_border_line_no + 1) % corner_count;
            }
        } else {
            current_lines = vec![None; clock_wise_side_diff + 3];
            for i in 0..=clock_wise_side_diff {
                current_lines[i + 1] = offset_pin_shape.border_line(current_border_line_no);
                current_border_line_no = (current_border_line_no + corner_count - 1) % corner_count;
            }
        }
        let current_lines_len = current_lines.len();
        current_lines[0] = Some(nearest_pin_exit_ray);
        current_lines[current_lines_len - 1] = Some(trace_polyline.lines()[latest_entry_tuple[0]]);
        let Some(border_lines) = current_lines.into_iter().collect::<Option<Vec<Line>>>() else {
            return Ok(false);
        };
        let mut border_lines = border_lines;
        let Ok(border_polyline) = Polyline::from_lines_in_place(&mut border_lines) else {
            return Ok(false);
        };
        if !board.check_polyline_trace(
            &border_polyline,
            layer,
            half_width,
            &net_numbers,
            clearance_class_index,
        ) {
            return Ok(false);
        }
        let trace_lines = trace_polyline.lines();
        let mut cut_lines = Vec::with_capacity(trace_lines.len() - latest_entry_tuple[0] + 1);
        cut_lines.push(border_lines[border_lines.len() - 2]);
        cut_lines.extend_from_slice(&trace_lines[latest_entry_tuple[0]..]);
        let Ok(cut_polyline) = Polyline::from_lines(cut_lines) else {
            return Ok(false);
        };
        let mut changed_polyline = if cut_polyline.first_corner() == cut_polyline.last_corner() {
            border_polyline.clone()
        } else {
            match border_polyline.combine(&cut_polyline) {
                Ok(combined) => combined,
                Err(_) => return Ok(false),
            }
        };
        if !at_start {
            changed_polyline = match changed_polyline.reverse() {
                Ok(reversed) => reversed,
                Err(_) => return Ok(false),
            };
        }
        if let Some(engine) = engine
            && board.items.get(&trace).is_some_and(Item::is_on_the_board)
        {
            board.additional_update_after_change(engine, trace);
        }
        board.change_trace(trace, changed_polyline);
        let Some(nearest_border_line) = offset_pin_shape.border_line(nearest_border_line_no) else {
            return Ok(false);
        };
        let Point::Int(pin_center_int) = pin_center else {
            return Ok(false);
        };
        let Some(exit_stub_line) =
            Line::from_direction_any(pin_center_int, &pin_exit_direction.turn_45_degree(2))
        else {
            return Ok(false);
        };
        let Ok(exit_line_segment) = Polyline::from_lines(vec![
            exit_stub_line,
            nearest_pin_exit_ray,
            nearest_border_line,
        ]) else {
            return Ok(false);
        };
        board.insert_trace(
            exit_line_segment,
            layer,
            half_width,
            net_numbers,
            clearance_class_index,
            FixedState::ShoveFixed,
        );
        Ok(true)
    }

    fn swap_connection_to_pin(
        board: &mut Board,
        engine: Option<&mut AutorouteEngine>,
        trace: ItemId,
        at_start: bool,
    ) -> Result<bool, BoardError> {
        let Some(Item::Trace(polyline_trace)) = board.items.get(&trace) else {
            return Ok(false);
        };
        let layer = polyline_trace.get_layer();
        let half_width = polyline_trace.get_half_width();
        let fixed_state = polyline_trace.hdr.get_fixed_state();
        let (trace_polyline, contact_list) = if at_start {
            (
                polyline_trace.polyline().clone(),
                board.trace_start_contacts(trace),
            )
        } else {
            let reversed = match polyline_trace.polyline().reverse() {
                Ok(reversed) => reversed,
                Err(_) => return Ok(false),
            };
            (reversed, board.trace_end_contacts(trace))
        };
        if contact_list.len() != 1 {
            return Ok(false);
        }
        let current_contact = *contact_list
            .iter()
            .next()
            .expect("the size test above found exactly one");
        let Some(contact_item) = board.items.get(&current_contact) else {
            return Ok(false);
        };
        if contact_item.get_fixed_state() != FixedState::ShoveFixed {
            return Ok(false);
        }
        let Item::Trace(contact_trace) = contact_item else {
            return Ok(false);
        };
        let contact_polyline = contact_trace.polyline().clone();
        let contact_lines = contact_polyline.lines();
        if contact_lines.len() < 2 || trace_polyline.lines().len() < 2 {
            return Ok(false);
        }
        let contact_last_line = contact_lines[contact_lines.len() - 2];
        let first_line = trace_polyline.lines()[1];
        let mut check_swap = contact_last_line
            .direction()
            .projection(&first_line.direction())
            == Signum::Negative;
        if !check_swap {
            let half_width_f = f64::from(half_width);
            let near_start = match (
                trace_polyline.corner_approx(0),
                trace_polyline.corner_approx(1),
            ) {
                (Some(first), Some(second)) => {
                    first.distance_square(&second) <= half_width_f * half_width_f
                }
                _ => false,
            };
            if trace_polyline.lines().len() > 3 && near_start {
                check_swap = contact_last_line
                    .direction()
                    .projection(&trace_polyline.lines()[2].direction())
                    == Signum::Negative;
            }
        }
        if !check_swap {
            return Ok(false);
        }
        let contact_trace_start_contacts = board.trace_start_contacts(current_contact);
        let Some(contact_pin_id) = contact_trace_start_contacts
            .into_iter()
            .rev()
            .find(|id| matches!(board.items.get(id), Some(Item::Pin(_))))
        else {
            return Ok(false);
        };
        let combined_polyline = match contact_polyline.combine(&trace_polyline) {
            Ok(combined) => combined,
            Err(_) => return Ok(false),
        };
        let Some(Item::Pin(contact_pin)) = board.items.get(&contact_pin_id) else {
            unreachable!("just matched")
        };
        let ctx = board.ctx();
        let nearest_pin_exit_direction = contact_pin.calc_nearest_exit_restriction_direction(
            &combined_polyline,
            half_width,
            layer,
            &ctx,
        );
        let Some(nearest_pin_exit_direction) = nearest_pin_exit_direction else {
            return Ok(false);
        };
        if nearest_pin_exit_direction == Direction::Int(contact_lines[1].direction()) {
            return Ok(false);
        }
        if let Some(Item::Trace(contact_trace)) = board.items.get_mut(&current_contact) {
            contact_trace.hdr.set_fixed_state(fixed_state);
        }
        let mut engine = engine;
        while board.items.get(&trace).is_some_and(Item::is_on_the_board)
            && (board.combine_trace_at_start(trace, true)?
                || board.combine_trace_at_end(trace, true)?)
        {
            if let Some(engine) = engine.as_deref_mut() {
                board.additional_update_after_change(engine, trace);
            }
        }
        Ok(true)
    }
}
