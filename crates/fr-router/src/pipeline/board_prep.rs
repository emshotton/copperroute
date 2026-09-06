use fr_board::{Board, structure::Unit};
use fr_settings::RouterSettings;

pub fn prepare_board(board: &mut Board, settings: &RouterSettings) -> bool {
    // Project constraints use board units; explicit router overrides use micrometres.
    let to_um = |value: i32| {
        Unit::scale(
            f64::from(value) / f64::from(board.communication.resolution.max(1)),
            board.communication.unit,
            Unit::Um,
        )
    };
    let constraints = board.rules.drc_constraints.as_ref();
    // Resolved settings include defaults (notably a zero hole clearance), so a
    // project minimum must floor them rather than only fill missing settings.
    let floor = |setting: Option<f64>, minimum: Option<i32>| match (setting, minimum.map(to_um)) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    };
    let copper_clearance = floor(
        settings.copper_to_edge_clearance_um,
        constraints.and_then(|c| c.copper_edge_clearance),
    );
    let hole_clearance = floor(
        settings.hole_clearance_um,
        constraints.and_then(|c| c.hole_clearance),
    );
    let copper_changed = match copper_clearance {
        Some(clearance_um) => board.apply_copper_to_edge_clearance_override(clearance_um),
        None => false,
    };
    // :747.
    let hole_changed = match hole_clearance {
        Some(clearance_um) => board.apply_hole_clearance_override(clearance_um),
        None => false,
    };
    copper_changed || hole_changed
}
