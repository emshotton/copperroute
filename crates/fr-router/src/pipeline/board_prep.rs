use fr_board::Board;
use fr_settings::RouterSettings;

pub fn prepare_board(board: &mut Board, settings: &RouterSettings) -> bool {
    let copper_changed = match settings.copper_to_edge_clearance_um {
        Some(clearance_um) => board.apply_copper_to_edge_clearance_override(clearance_um),
        None => false,
    };
    let hole_changed = match settings.hole_clearance_um {
        Some(clearance_um) => board.apply_hole_clearance_override(clearance_um),
        None => false,
    };
    copper_changed || hole_changed
}
