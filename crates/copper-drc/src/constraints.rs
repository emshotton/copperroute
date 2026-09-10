use copper_board::{Board, DrcConstraints, DrcSeverity, Item, Unit};
use copper_dsn::CoordinateTransform;
use serde_json::Value;

use crate::{DrcError, DrcViolationKind};

/// KiCad's `ADVANCED_CFG::m_DRCEpsilon` default
/// (`DRC_TEST_PROVIDER_COPPER_CLEARANCE::sub_e`), subtracted from a clearance requirement
/// before the gap test so that a trace routed at exactly the clearance is not flagged by
/// integer rounding.
pub const DRC_EPSILON_MM: f64 = 0.0005;

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
    constraints.solder_mask_min_width = board.rules.solder_mask_min_width;
    let matrix = &board.rules.clearance_matrix;
    for class in board.rules.net_classes.iter() {
        let name = canonical_class_name(class.get_name()).to_string();
        let clearance_class = class.get_trace_clearance_class();
        let clearance = matrix.get_value(clearance_class, clearance_class, 0, false);
        if clearance > 0 {
            constraints
                .netclass_clearance
                .insert(name.clone(), clearance);
        }
        let width = 2 * class.get_trace_half_width(0);
        if width > 0 {
            constraints.netclass_track_width.insert(name, width);
        }
    }
    constraints.epsilon = (Unit::scale(DRC_EPSILON_MM, Unit::Mm, board.communication.unit)
        * f64::from(board.communication.resolution.max(1)))
    .round() as i64 as i32;
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

/// Unlike clearance, KiCad's `track_width` DRC test never floors a net at its net class's `track
/// width`: that field is only the default width the interactive router draws with. The one
/// enforced minimum is the board's own `min_track_width` design rule — `kicad-cli` reports no
/// violation for a 0.15 mm trace on a net whose class calls for 0.2 mm when the board's own
/// minimum is unset.
#[must_use]
pub fn track_width_min(
    _board: &Board,
    constraints: &DrcConstraints,
    _net_number: i32,
) -> Option<i32> {
    constraints.min_track_width
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

fn to_board_units(mm: f64, board: &Board, transform: &CoordinateTransform) -> i32 {
    let dsn_value = Unit::scale(mm, Unit::Mm, board.communication.unit);
    (transform.dsn_to_board(dsn_value)).round() as i64 as i32
}

fn rule(rules: &Value, key: &str, board: &Board, transform: &CoordinateTransform) -> Option<i32> {
    let mm = rules.get(key)?.as_f64()?;
    Some(to_board_units(mm, board, transform))
}

fn positive(value: Option<i32>) -> Option<i32> {
    value.filter(|v| *v > 0)
}

fn parse_severity(text: &str) -> Option<DrcSeverity> {
    match text {
        "error" => Some(DrcSeverity::Error),
        "warning" => Some(DrcSeverity::Warning),
        "ignore" | "exclusion" => Some(DrcSeverity::Ignore),
        _ => None,
    }
}

pub fn from_kicad_project(
    json: &str,
    board: &Board,
    transform: &CoordinateTransform,
) -> Result<DrcConstraints, DrcError> {
    let document: Value = serde_json::from_str(json)?;
    let settings = document
        .pointer("/board/design_settings")
        .ok_or_else(|| DrcError::Project("no board.design_settings object".to_string()))?;
    let rules = settings.get("rules").cloned().unwrap_or(Value::Null);
    let mut constraints = DrcConstraints {
        min_clearance: positive(rule(&rules, "min_clearance", board, transform)),
        min_track_width: positive(rule(&rules, "min_track_width", board, transform)),
        solder_mask_to_copper_clearance: rule(
            &rules,
            "solder_mask_to_copper_clearance",
            board,
            transform,
        ),
        hole_clearance: rule(&rules, "min_hole_clearance", board, transform),
        hole_to_hole: positive(rule(&rules, "min_hole_to_hole", board, transform)),
        copper_edge_clearance: positive(rule(
            &rules,
            "min_copper_edge_clearance",
            board,
            transform,
        )),
        min_via_diameter: positive(rule(&rules, "min_via_diameter", board, transform)),
        min_via_annular_width: positive(rule(&rules, "min_via_annular_width", board, transform)),
        min_through_hole_diameter: positive(rule(
            &rules,
            "min_through_hole_diameter",
            board,
            transform,
        )),
        min_microvia_diameter: positive(rule(&rules, "min_microvia_diameter", board, transform)),
        min_microvia_drill: positive(rule(&rules, "min_microvia_drill", board, transform)),
        epsilon: to_board_units(DRC_EPSILON_MM, board, transform),
        ..DrcConstraints::default()
    };
    if let Some(severities) = settings.get("rule_severities").and_then(Value::as_object) {
        for (name, value) in severities {
            if let Some(severity) = value.as_str().and_then(parse_severity) {
                constraints.severities.insert(name.clone(), severity);
            }
        }
    }
    if let Some(classes) = document
        .pointer("/net_settings/classes")
        .and_then(Value::as_array)
    {
        for class in classes {
            let Some(name) = class.get("name").and_then(Value::as_str) else {
                continue;
            };
            let name = canonical_class_name(name).to_string();
            if let Some(clearance) = positive(rule(class, "clearance", board, transform)) {
                constraints
                    .netclass_clearance
                    .insert(name.clone(), clearance);
            }
            if let Some(width) = positive(rule(class, "track_width", board, transform)) {
                constraints.netclass_track_width.insert(name, width);
            }
        }
    }
    Ok(constraints)
}

pub fn apply_kicad_project(
    json: &str,
    board: &mut Board,
    transform: &CoordinateTransform,
) -> Result<(), DrcError> {
    let project = from_kicad_project(json, board, transform)?;
    let dsn = from_dsn(board);
    board.rules.drc_constraints = Some(DrcConstraints::merge(dsn, project));
    Ok(())
}
