pub mod copper;
pub mod edge;
pub mod geometry;
pub mod holes;
pub mod single;
pub mod solder_mask;

use copper_board::{Board, DrcConstraints};

use crate::DrcViolation;

#[must_use]
pub fn run_all(board: &mut Board, constraints: &DrcConstraints) -> Vec<DrcViolation> {
    let mut out = Vec::new();
    copper::run(board, constraints, &mut out);
    holes::run(board, constraints, &mut out);
    single::run(board, constraints, &mut out);
    edge::run(board, constraints, &mut out);
    solder_mask::run(board, constraints, &mut out);
    out.sort_by(|a, b| {
        (
            a.first_item.0,
            a.kind,
            a.second_item.map(|id| id.0),
            a.layer,
        )
            .cmp(&(
                b.first_item.0,
                b.kind,
                b.second_item.map(|id| id.0),
                b.layer,
            ))
    });
    out
}
