use copper_board::Board;
use copper_settings::RouterSettings;

pub fn prepare_board(board: &mut Board, settings: &RouterSettings) -> bool {
    let copper_changed = match settings.copper_to_edge_clearance_um {
        Some(clearance_um) => board.apply_copper_to_edge_clearance_override(clearance_um),
        None => false,
    };
    let hole_changed = match settings.hole_clearance_um {
        Some(clearance_um) => board.apply_hole_clearance_override(clearance_um),
        None => false,
    };
    let raised = raise_to_project_minimums(board);
    copper_changed || hole_changed || raised
}

/// Resolved settings always carry a hole clearance (zero by default), so a project
/// minimum has to raise the applied value rather than fill in a missing one.
pub fn raise_to_project_minimums(board: &mut Board) -> bool {
    let Some(constraints) = board.rules.drc_constraints.as_ref() else {
        return false;
    };
    let copper_edge_minimum = constraints.copper_edge_clearance;
    let hole_minimum = constraints.hole_clearance;
    let copper_raised =
        copper_edge_minimum.is_some_and(|minimum| board.raise_copper_to_edge_clearance_to(minimum));
    let hole_raised = hole_minimum.is_some_and(|minimum| board.raise_hole_clearance_to(minimum));
    copper_raised || hole_raised
}
