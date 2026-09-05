use fr_board::{Board, DrcConstraints, DrcSeverity, Item};

use crate::DrcViolationKind;

#[must_use]
pub fn canonical_class_name(name: &str) -> &str {
    if name.eq_ignore_ascii_case("default") || name.eq_ignore_ascii_case("kicad_default") {
        "Default"
    } else {
        name
    }
}

#[must_use]
pub fn from_dsn(board: &Board) -> DrcConstraints {
    let mut constraints = DrcConstraints::default();
    let matrix = &board.rules.clearance_matrix;
    for class in board.rules.net_classes.iter() {
        let name = canonical_class_name(class.get_name()).to_string();
        let clearance_class = class.get_trace_clearance_class();
        let clearance = matrix.get_value(clearance_class, clearance_class, 0, false);
        if clearance > 0 {
            constraints.netclass_clearance.insert(name.clone(), clearance);
        }
        let width = 2 * class.get_trace_half_width(0);
        if width > 0 {
            constraints.netclass_track_width.insert(name, width);
        }
    }
    constraints
}

#[must_use]
pub fn resolve(board: &Board) -> DrcConstraints {
    board
        .rules
        .drc_constraints
        .clone()
        .unwrap_or_else(|| from_dsn(board))
}

#[must_use]
pub fn netclass_name(board: &Board, net_number: i32) -> Option<String> {
    let net = board.rules.nets.get(net_number)?;
    let class = board.rules.net_classes.get(net.net_class);
    Some(canonical_class_name(class.get_name()).to_string())
}

fn netclass_clearance(board: &Board, constraints: &DrcConstraints, item: &Item) -> Option<i32> {
    if item.net_count() == 0 {
        return None;
    }
    let name = netclass_name(board, item.get_net_number(0))?;
    constraints.netclass_clearance.get(&name).copied()
}

#[must_use]
pub fn pair_clearance(
    board: &Board,
    constraints: &DrcConstraints,
    a: &Item,
    b: &Item,
) -> Option<i32> {
    [
        netclass_clearance(board, constraints, a),
        netclass_clearance(board, constraints, b),
        constraints.min_clearance,
    ]
    .into_iter()
    .flatten()
    .max()
}

#[must_use]
pub fn track_width_min(board: &Board, constraints: &DrcConstraints, net_number: i32) -> Option<i32> {
    let class_width = netclass_name(board, net_number)
        .and_then(|name| constraints.netclass_track_width.get(&name).copied());
    [class_width, constraints.min_track_width]
        .into_iter()
        .flatten()
        .max()
}

#[must_use]
pub fn severity(constraints: &DrcConstraints, kind: DrcViolationKind) -> DrcSeverity {
    constraints
        .severities
        .get(kind.kicad_type())
        .copied()
        .unwrap_or(DrcSeverity::Error)
}

#[must_use]
pub fn search_radius(constraints: &DrcConstraints) -> i32 {
    constraints
        .netclass_clearance
        .values()
        .copied()
        .chain(constraints.min_clearance)
        .chain(constraints.hole_clearance)
        .chain(constraints.hole_to_hole)
        .chain(constraints.copper_edge_clearance)
        .max()
        .unwrap_or(0)
}
