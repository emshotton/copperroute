use copper_board::prelude::*;
use copper_drc::DesignRulesChecker;
use copper_drc::report::{DrcCoordinates, DrcReportOptions, KiCadDrcReport, item_description};
use copper_dsn::{BoardReadResult, CoordinateTransform, DsnReadOptions};
use copper_geometry::{Area, IntBox, IntPoint, IntVector, Point, Polyline, Shape, TileShape};

const DEV_BOARD: &str = "Issue575-drc_dev-board_4_hole_clearance_violations.dsn";
const BBD_MARS_64: &str = "Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations.dsn";
const NATURAL_TONE_PREAMP: &str = "Issue575-drc_Natural_Tone_Preamp_7_unconnected_items.dsn";

mod common;
use common::JAR_VERSION;

fn fixture_board(name: &str) -> (Board, CoordinateTransform) {
    let path = testkit::fixture(name);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()));
    match copper_dsn::read_board(&bytes[..], None, Some(name), &DsnReadOptions::default()) {
        BoardReadResult::Success {
            board,
            coordinate_transform,
            ..
        }
        | BoardReadResult::OutlineMissing {
            board,
            coordinate_transform,
            ..
        } => (
            *board.unwrap_or_else(|| panic!("{name} produced no board")),
            coordinate_transform.unwrap_or_else(|| panic!("{name} produced no transform")),
        ),
        other => panic!("{name} did not read: {other:?}"),
    }
}

fn options(source: &str, unit: &str) -> DrcReportOptions {
    DrcReportOptions {
        source: source.to_string(),
        coordinate_unit: unit.to_string(),
        date: "2026-08-29T00:00:00Z".to_string(),
        router_version: JAR_VERSION.to_string(),
        quality_score: None,
    }
}

fn report_for(fixture: &str, unit: &str) -> KiCadDrcReport {
    let (mut board, transform) = fixture_board(fixture);
    let board_unit = board.communication.unit;
    let coords = DrcCoordinates {
        transform,
        board_unit,
    };
    DesignRulesChecker::new(&mut board).generate_report(&coords, &options(fixture, unit))
}

#[test]
fn the_unconnected_entry_description_and_severity() {
    let report = report_for(DEV_BOARD, "mm");
    for entry in &report.unconnected_items {
        assert_eq!(entry.kind, "unconnected_items");
        assert_eq!(entry.severity, "warning");
        assert!(
            entry.description.starts_with("Unconnected items: ")
                && entry
                    .description
                    .ends_with(&format!(" ({} total items in net)", entry.items.len())),
            "{}",
            entry.description
        );
    }
}

#[test]
fn a_dangling_track_carries_the_detailed_description() {
    let report = report_for(DEV_BOARD, "mm");
    let track = report
        .violations
        .iter()
        .find(|v| v.kind == "track_dangling")
        .expect("the dev board has eight");
    assert_eq!(track.description, "Track has unconnected end");
    assert_eq!(track.severity, "warning");
    assert_eq!(track.items.len(), 1);
    assert_eq!(
        track.items[0].description,
        "Track [GND] on F.Cu, length 1.4000 mm"
    );
}

#[test]
fn coordinates_are_in_a_plausible_mm_range() {
    let report = report_for(NATURAL_TONE_PREAMP, "mm");
    let pos = &report.unconnected_items[0].items[0].pos;
    assert!(
        (10.0..500.0).contains(&pos.x.abs()),
        "x out of range: {}",
        pos.x
    );
    assert!(
        (10.0..500.0).contains(&pos.y.abs()),
        "y out of range: {}",
        pos.y
    );
}

#[test]
fn y_is_negative_on_a_kicad_sourced_board() {
    let report = report_for(DEV_BOARD, "mm");
    assert!(report.violations[0].items[0].pos.y < 0.0);
    assert!(report.violations[0].items[0].pos.x > 0.0);
}

#[test]
fn unknown_coordinate_unit_falls_back_to_the_board_unit() {
    let (mut board, transform) = fixture_board(BBD_MARS_64);
    assert_eq!(board.communication.unit, Unit::Um);
    let coords = DrcCoordinates {
        transform,
        board_unit: board.communication.unit,
    };
    let report = DesignRulesChecker::new(&mut board)
        .generate_report(&coords, &options(BBD_MARS_64, "furlong"));
    assert_eq!(report.coordinate_units, "furlong");
    // This fixture's trace-vs-trace clearance gaps are all within `DRC_EPSILON_MM` of the
    // rule, so `checks::run_all` reports none of them and a dangling-track entry sorts first.
    assert_eq!(report.violations[0].items[0].pos.x, 90_181.65);
    assert!(
        report.violations[0]
            .description
            .contains("Track has unconnected end")
    );
}

#[test]
fn mil_and_inch_scale() {
    let mm = report_for(BBD_MARS_64, "mm").violations[0].items[0].pos.x;
    let mil = report_for(BBD_MARS_64, "mil").violations[0].items[0].pos.x;
    let inch = report_for(BBD_MARS_64, "inch").violations[0].items[0].pos.x;
    let um = report_for(BBD_MARS_64, "um").violations[0].items[0].pos.x;

    assert_eq!(mm, 90.18164999999999);
    assert_eq!(um, 90_181.65);
    assert_eq!(mil, 90_181.65 / 25.4);
    assert_eq!(inch, 90_181.65 / 25_400.0);
}

#[test]
fn percent_four_f_uses_a_dot() {
    let has_comma_decimal = |s: &str| {
        let bytes = s.as_bytes();
        bytes
            .windows(3)
            .any(|w| w[1] == b',' && w[0].is_ascii_digit() && w[2].is_ascii_digit())
    };
    for fixture in [DEV_BOARD, BBD_MARS_64, NATURAL_TONE_PREAMP] {
        let report = report_for(fixture, "mm");
        for entry in report.violations.iter().chain(&report.unconnected_items) {
            assert!(
                !has_comma_decimal(&entry.description),
                "{}",
                entry.description
            );
            for item in &entry.items {
                assert!(
                    !has_comma_decimal(&item.description),
                    "{}",
                    item.description
                );
            }
        }
    }
    assert!(has_comma_decimal("expected: 0,0500 mm"));
}

#[test]
fn item_description_maps_every_item_variant() {
    let (board, ids) = all_variants_board();
    let described: Vec<String> = ids.iter().map(|&id| item_description(&board, id)).collect();
    assert_eq!(
        described,
        vec![
            "BoardOutline".to_string(),
            "Trace [N1]".to_string(),
            "Pin [N1]".to_string(),
            "Via [N1]".to_string(),
            "Conduction Area [N1]".to_string(),
            "ObstacleArea".to_string(),
            "ViaObstacleArea".to_string(),
            "ComponentObstacleArea".to_string(),
            "ComponentOutline".to_string(),
        ],
    );
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

fn two_layers() -> LayerStructure {
    LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)])
}

fn all_variants_board() -> Board2 {
    let ls = two_layers();
    let pad = Shape::Tile(TileShape::Box(IntBox::from_coords(-70, -70, 70, 70)));
    let mut padstacks = Padstacks::new(ls.clone());
    let thru = padstacks.add(
        "thru",
        vec![Some(pad.clone()), Some(pad.clone())],
        true,
        false,
    );
    let smd = padstacks.add("smd", vec![Some(pad)], false, false);
    let package_pins = vec![PackagePin::new(
        "p0",
        smd,
        IntVector::new(0, 4_000).into(),
        0.0,
    )];
    let mut packages = Packages::new();
    let package = packages.add(
        "pkg",
        package_pins,
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
    board.rules.nets.add("N1", 1, false, default_class);

    let mut ids = vec![ItemId(1)];
    ids.push(
        board
            .insert_trace_without_cleaning(
                Polyline::from_points(&[Point::new(-2_000, 0), Point::new(-1_000, 0)]),
                0,
                30,
                vec![1],
                1,
                FixedState::Unfixed,
            )
            .expect("the synthetic trace is neither degenerate nor closed"),
    );
    ids.push(board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed));
    ids.push(
        board
            .insert_via(
                thru,
                Point::new(2_000, 0),
                vec![1],
                1,
                FixedState::Unfixed,
                true,
            )
            .expect("the synthetic via inserts cleanly"),
    );
    let area = |x: i32| {
        Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            x - 200,
            -6_000,
            x + 200,
            -5_600,
        ))))
    };
    ids.push(board.insert_conduction_area(area(0), 0, vec![1], 1, true, FixedState::Unfixed));
    ids.push(board.insert_obstacle(area(1_000), 0, 1, FixedState::Unfixed));
    ids.push(board.insert_via_obstacle(area(2_000), 0, 1, FixedState::Unfixed));
    ids.push(board.insert_component_obstacle(area(3_000), 0, 1, FixedState::Unfixed));
    ids.push(
        board
            .insert_component_outline(
                area(4_000),
                true,
                copper_geometry::Vector::ZERO,
                0.0,
                1,
                false,
                false,
                true,
                FixedState::Unfixed,
            )
            .expect("the synthetic component outline is bounded"),
    );
    (board, ids)
}

type Board2 = (Board, Vec<ItemId>);
