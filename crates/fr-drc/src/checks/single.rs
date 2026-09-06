use fr_board::{Board, DrcConstraints, DrcSeverity, Item, ItemId};

use crate::checks::geometry::{hole_of, is_microvia, item_position};
use crate::constraints::{severity, track_width_min};
use crate::{DrcViolation, DrcViolationKind};

#[allow(clippy::too_many_arguments)]
fn push(
    board: &Board,
    constraints: &DrcConstraints,
    out: &mut Vec<DrcViolation>,
    kind: DrcViolationKind,
    id: ItemId,
    layer: Option<usize>,
    expected: i32,
    actual: f64,
    estimated: bool,
) {
    if actual >= f64::from(expected) {
        return;
    }
    let severity = severity(constraints, kind);
    if severity == DrcSeverity::Ignore {
        return;
    }
    out.push(DrcViolation {
        kind,
        severity,
        first_item: id,
        second_item: None,
        layer,
        position: item_position(board, id),
        expected: f64::from(expected),
        actual,
        estimated,
    });
}

pub fn run(board: &mut Board, constraints: &DrcConstraints, out: &mut Vec<DrcViolation>) {
    for id in board.items_in_board_order() {
        let Some(item) = board.get_item(id) else {
            continue;
        };
        match item {
            Item::Trace(trace) => {
                if trace.hdr.net_nos.is_empty() {
                    continue;
                }
                let Some(minimum) = track_width_min(board, constraints, trace.hdr.net_nos[0])
                else {
                    continue;
                };
                let actual = f64::from(2 * trace.get_half_width());
                push(
                    board,
                    constraints,
                    out,
                    DrcViolationKind::TrackWidth,
                    id,
                    Some(trace.get_layer()),
                    minimum,
                    actual,
                    false,
                );
            }
            Item::Via(via) => {
                let ctx = board.ctx();
                let micro = is_microvia(board, id);
                let diameter = 2.0 * via.smallest_radius(&ctx);
                let diameter_min = if micro {
                    constraints.min_microvia_diameter
                } else {
                    constraints.min_via_diameter
                };
                if let Some(minimum) = diameter_min {
                    push(
                        board,
                        constraints,
                        out,
                        DrcViolationKind::ViaDiameter,
                        id,
                        None,
                        minimum,
                        diameter,
                        false,
                    );
                }
                if let Some(hole) = hole_of(board, id) {
                    if let Some(minimum) = constraints.min_via_annular_width {
                        let annulus = via.smallest_radius(&ctx) - hole.radius;
                        push(
                            board,
                            constraints,
                            out,
                            DrcViolationKind::AnnularWidth,
                            id,
                            None,
                            minimum,
                            annulus,
                            hole.estimated,
                        );
                    }
                    let (kind, drill_min) = if micro {
                        (
                            DrcViolationKind::MicroviaDrillOutOfRange,
                            constraints.min_microvia_drill,
                        )
                    } else {
                        (
                            DrcViolationKind::DrillOutOfRange,
                            constraints.min_through_hole_diameter,
                        )
                    };
                    if let Some(minimum) = drill_min {
                        push(
                            board,
                            constraints,
                            out,
                            kind,
                            id,
                            None,
                            minimum,
                            2.0 * hole.radius,
                            hole.estimated,
                        );
                    }
                }
            }
            Item::Pin(pin) => {
                let Some(hole) = hole_of(board, id) else {
                    continue;
                };
                let ctx = board.ctx();
                if let Some(minimum) = constraints.min_via_annular_width
                    && !pin.get_padstack(&ctx).is_some_and(|p| p.hole_only)
                {
                    let annulus = pin.smallest_radius(&ctx) - hole.radius;
                    push(
                        board,
                        constraints,
                        out,
                        DrcViolationKind::AnnularWidth,
                        id,
                        None,
                        minimum,
                        annulus,
                        hole.estimated,
                    );
                }
                if let Some(minimum) = constraints.min_through_hole_diameter {
                    push(
                        board,
                        constraints,
                        out,
                        DrcViolationKind::DrillOutOfRange,
                        id,
                        None,
                        minimum,
                        2.0 * hole.radius,
                        hole.estimated,
                    );
                }
            }
            _ => {}
        }
    }
}
