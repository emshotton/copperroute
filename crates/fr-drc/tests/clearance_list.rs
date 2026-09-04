use fr_board::prelude::*;
use fr_drc::{ClearanceViolation, DesignRulesChecker};
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_geometry::{IntBox, IntPoint, IntVector, Point, Shape, TileShape};


fn fixture_board(name: &str) -> Board {
    let path = parity::fixture(name);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()));
    match fr_dsn::read_board(&bytes[..], None, Some(name), &DsnReadOptions::default()) {
        BoardReadResult::Success { board, .. } | BoardReadResult::OutlineMissing { board, .. } => {
            *board.unwrap_or_else(|| panic!("{name} produced no board"))
        }
        other => panic!("{name} did not read: {other:?}"),
    }
}

const DEV_BOARD: &str = "Issue575-drc_dev-board_4_hole_clearance_violations.dsn";
const BBD_MARS_64: &str = "Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations.dsn";
const NATURAL_TONE_PREAMP: &str = "Issue575-drc_Natural_Tone_Preamp_7_unconnected_items.dsn";

fn violation_count(fixture: &str) -> usize {
    let mut board = fixture_board(fixture);
    DesignRulesChecker::new(&mut board)
        .get_all_clearance_violations()
        .len()
}


#[test]
fn dev_board_has_two_deduplicated_violations() {
    if !parity::require_java_dir() {
        return;
    }
    assert_eq!(violation_count(DEV_BOARD), 2);
}

#[test]
fn bbd_mars_64_has_seventy_six() {
    if !parity::require_java_dir() {
        return;
    }
    assert_eq!(violation_count(BBD_MARS_64), 76);
}

#[test]
fn natural_tone_preamp_has_none() {
    if !parity::require_java_dir() {
        return;
    }
    assert_eq!(violation_count(NATURAL_TONE_PREAMP), 0);
}

#[test]
fn empty_board_has_none() {
    if !parity::require_java_dir() {
        return;
    }
    assert_eq!(violation_count("empty_board.dsn"), 0);
}


#[test]
fn the_surviving_first_item_is_the_higher_id() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = fixture_board(DEV_BOARD);
    let violations = DesignRulesChecker::new(&mut board).get_all_clearance_violations();
    let pairs: Vec<(u32, u32)> = violations
        .iter()
        .map(|v| (v.first_item.0, v.second_item.0))
        .collect();
    assert_eq!(pairs, vec![(278, 277), (276, 275)]);
    for v in &violations {
        assert_eq!(v.layer, 0);
        assert_eq!(v.expected_clearance, 500.0);
        assert_eq!(v.actual_clearance, 0.0);
    }
}

#[test]
fn the_ordered_list_matches_the_jvm() {
    if !parity::require_java_dir() {
        return;
    }
    for fixture in [DEV_BOARD, BBD_MARS_64] {
        let golden_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(format!("{}.list.txt", fixture.trim_end_matches(".dsn")));
        let golden = std::fs::read_to_string(&golden_path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", golden_path.display()));
        let mut board = fixture_board(fixture);
        let violations = DesignRulesChecker::new(&mut board).get_all_clearance_violations();
        assert_eq!(render(&violations), golden, "{fixture}");
    }
}

fn render(violations: &[ClearanceViolation]) -> String {
    let mut out = format!("count {}\n", violations.len());
    for v in violations {
        out.push_str(&format!(
            "first={} second={} layer={} expected={} actual={}\n",
            v.first_item.0,
            v.second_item.0,
            v.layer,
            fr_dsn::java_double_to_string(v.expected_clearance),
            fr_dsn::java_double_to_string(v.actual_clearance),
        ));
    }
    out
}

#[test]
fn a_pair_on_two_layers_yields_two_entries() {
    let mut board = two_layer_pad_board();
    let violations = DesignRulesChecker::new(&mut board).get_all_clearance_violations();
    let rows: Vec<(u32, u32, usize)> = violations
        .iter()
        .map(|v| (v.first_item.0, v.second_item.0, v.layer))
        .collect();
    assert_eq!(rows, vec![(3, 2, 0), (3, 2, 1)]);
}


const BOUNDING_BOX: IntBox = IntBox {
    ll: IntPoint {
        x: -10_000,
        y: -10_000,
    },
    ur: IntPoint {
        x: 10_000,
        y: 10_000,
    },
};

fn layers() -> LayerStructure {
    LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)])
}

fn two_layer_pad_board() -> Board {
    let mut padstacks = Padstacks::new(layers());
    let mut pins = Vec::new();
    for (name, offset) in [("a", 0), ("b", 30)] {
        let shape = Shape::Tile(TileShape::Box(IntBox::from_coords(-50, -50, 50, 50)));
        let padstack = padstacks.add(name, vec![Some(shape.clone()), Some(shape)], false, false);
        pins.push(PackagePin::new(
            name,
            padstack,
            IntVector::new(offset, 0).into(),
            0.0,
        ));
    }
    let mut packages = Packages::new();
    let package = packages.add(
        "pkg",
        pins,
        None,
        None,
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        true,
    );
    let mut components = Components::new();
    components.add_with_generated_name(Some(Point::new(0, 0)), 0.0, true, package);

    let ls = layers();
    let matrix = ClearanceMatrix::get_default_instance(&ls, 200);
    let mut rules = BoardRules::new(ls, matrix);
    rules.create_default_net_class();
    let default_class = rules.get_default_net_class();
    let mut board = Board::new(
        Vec::new(),
        1,
        BOUNDING_BOX,
        rules,
        BoardLibrary::new(padstacks, packages),
        components,
        Communication::default(),
    );
    for i in 0..2 {
        board
            .rules
            .nets
            .add(format!("N{}", i + 1), 1, false, default_class);
    }
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    board.insert_pin(1, 1, vec![2], 1, FixedState::Unfixed);
    board
}
