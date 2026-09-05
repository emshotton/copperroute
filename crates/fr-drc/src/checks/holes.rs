use std::collections::BTreeSet;

use fr_board::{Board, DrcConstraints, DrcSeverity};

use crate::checks::geometry::{candidates, gap_below, hole_of};
use crate::constraints::severity;
use crate::{DrcViolation, DrcViolationKind};

pub fn run(board: &mut Board, constraints: &DrcConstraints, out: &mut Vec<DrcViolation>) {
    let Some(minimum) = constraints.hole_to_hole else {
        return;
    };
    let severity = severity(constraints, DrcViolationKind::HoleToHole);
    if severity == DrcSeverity::Ignore {
        return;
    }
    let mut seen: BTreeSet<(u32, u32)> = BTreeSet::new();
    for id in board.items_in_board_order() {
        let Some(hole) = hole_of(board, id) else {
            continue;
        };
        for other in candidates(board, &hole.shape, None, minimum) {
            if other.0 <= id.0 {
                continue;
            }
            let Some(other_hole) = hole_of(board, other) else {
                continue;
            };
            if !seen.insert((id.0, other.0)) {
                continue;
            }
            if let Some((actual, position)) = gap_below(&hole.shape, &other_hole.shape, minimum) {
                out.push(DrcViolation {
                    kind: DrcViolationKind::HoleToHole,
                    severity,
                    first_item: id,
                    second_item: Some(other),
                    layer: None,
                    position,
                    expected: f64::from(minimum),
                    actual,
                    estimated: hole.estimated || other_hole.estimated,
                });
            }
        }
    }
}
