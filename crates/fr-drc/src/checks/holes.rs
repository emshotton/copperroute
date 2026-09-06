use std::collections::BTreeSet;

use fr_board::{Board, DrcConstraints, DrcSeverity};

use crate::checks::geometry::{candidates, hole_of, sub_epsilon};
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
            let gap = hole.gap_to(&other_hole);
            if gap < f64::from(sub_epsilon(minimum, constraints.epsilon)) {
                let actual = gap.max(0.0);
                let distance = hole.center.distance(&other_hole.center);
                let along = if distance > 0.0 {
                    ((hole.radius + actual / 2.0) / distance).min(1.0)
                } else {
                    0.0
                };
                let position = fr_geometry::FloatPoint::new(
                    hole.center.x + (other_hole.center.x - hole.center.x) * along,
                    hole.center.y + (other_hole.center.y - hole.center.y) * along,
                );
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
