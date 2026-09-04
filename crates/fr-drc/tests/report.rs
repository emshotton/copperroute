use fr_board::prelude::*;
use fr_drc::DesignRulesChecker;
use fr_drc::report::{
    DrcCoordinates, DrcReportOptions, KiCadDrcReport, KiCadDrcViolation, item_description,
};
use fr_dsn::{BoardReadResult, CoordinateTransform, DsnReadOptions};
use fr_geometry::{Area, IntBox, IntPoint, IntVector, Point, Polyline, Shape, TileShape};


const DEV_BOARD: &str = "Issue575-drc_dev-board_4_hole_clearance_violations.dsn";
const BBD_MARS_64: &str = "Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations.dsn";
const NATURAL_TONE_PREAMP: &str = "Issue575-drc_Natural_Tone_Preamp_7_unconnected_items.dsn";
const EMPTY_BOARD: &str = "empty_board.dsn";

mod common;
use common::JAR_VERSION;

fn fixture_board(name: &str) -> (Board, CoordinateTransform) {
    let path = parity::fixture(name);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()));
    match fr_dsn::read_board(&bytes[..], None, Some(name), &DsnReadOptions::default()) {
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
        freerouting_version: JAR_VERSION.to_string(),
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
fn dev_board_report_shape() {
    if !parity::require_java_dir() {
        return;
    }
    let report = report_for(DEV_BOARD, "mm");

    assert_eq!(report.json_schema, "https://schemas.kicad.org/drc.v1.json");
    assert_eq!(report.kicad_version, "N/A");
    assert_eq!(report.freerouting_version, "Freerouting 2.3.1-SNAPSHOT");
    assert_eq!(report.coordinate_units, "mm");
    assert_eq!(report.source, DEV_BOARD);
    assert_eq!(report.quality_score, None);
    assert!(report.schematic_parity.is_empty());

    assert_eq!(report.violations.len(), 10);
    assert_eq!(
        report
            .violations
            .iter()
            .map(|v| v.kind.as_str())
            .collect::<Vec<_>>(),
        vec![
            "holeClearance",
            "holeClearance",
            "track_dangling",
            "track_dangling",
            "track_dangling",
            "track_dangling",
            "track_dangling",
            "track_dangling",
            "track_dangling",
            "track_dangling",
        ],
    );
    assert_eq!(report.unconnected_items.len(), 4);
}

#[test]
fn first_violation_is_verbatim() {
    if !parity::require_java_dir() {
        return;
    }
    let report = report_for(DEV_BOARD, "mm");
    let first = &report.violations[0];
    assert_eq!(
        first.description,
        "Hole clearance violation between Pin [GND] and Pin [GND] \
         (expected: 0.0500 mm, actual: 0.0000 mm)"
    );
    assert_eq!(first.severity, "error");
    assert_eq!(first.kind, "holeClearance");
    let rendered: Vec<(&str, f64, f64, &str)> = first
        .items
        .iter()
        .map(|i| (i.description.as_str(), i.pos.x, i.pos.y, i.uuid.as_str()))
        .collect();
    assert_eq!(
        rendered,
        vec![
            ("Pin [GND]", 125.5, -49.31, "278"),
            ("Pin [GND]", 125.5, -49.31, "277"),
        ],
    );
}

#[test]
fn the_unconnected_entry_description_and_severity() {
    if !parity::require_java_dir() {
        return;
    }
    let report = report_for(DEV_BOARD, "mm");
    for entry in &report.unconnected_items {
        assert_eq!(entry.kind, "unconnectedItems");
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
    if !parity::require_java_dir() {
        return;
    }
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


fn render(report: &KiCadDrcReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("schema {}\n", report.json_schema));
    out.push_str(&format!("coordinateUnits {}\n", report.coordinate_units));
    out.push_str(&format!("kicadVersion {}\n", report.kicad_version));
    out.push_str(&format!(
        "freeroutingVersion {}\n",
        report.freerouting_version
    ));
    out.push_str(&format!("source {}\n", report.source));
    out.push_str(&format!(
        "qualityScore {}\n",
        match report.quality_score {
            Some(score) => fr_dsn::java_double_to_string(score),
            None => "null".to_string(),
        }
    ));
    out.push_str(&format!(
        "counts violations={} unconnectedItems={} schematicParity={}\n",
        report.violations.len(),
        report.unconnected_items.len(),
        report.schematic_parity.len(),
    ));
    for entry in &report.violations {
        render_entry(&mut out, "V", entry);
    }
    for entry in &report.unconnected_items {
        render_entry(&mut out, "U", entry);
    }
    out
}

fn render_entry(out: &mut String, tag: &str, entry: &KiCadDrcViolation) {
    out.push_str(&format!(
        "{tag} type={} severity={} desc={}\n",
        entry.kind, entry.severity, entry.description
    ));
    let mut items: Vec<_> = entry.items.iter().collect();
    items.sort_by_key(|i| i.uuid.parse::<i64>().expect("uuids are item ids"));
    for item in items {
        out.push_str(&format!(
            "I uuid={} x={} y={} desc={}\n",
            item.uuid,
            fr_dsn::java_double_to_string(item.pos.x),
            fr_dsn::java_double_to_string(item.pos.y),
            item.description,
        ));
    }
}

fn golden(fixture: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(format!("{}.report.txt", fixture.trim_end_matches(".dsn")));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

#[test]
fn three_fixtures_match_the_jvm_byte_for_byte() {
    if !parity::require_java_dir() {
        return;
    }
    for fixture in [DEV_BOARD, BBD_MARS_64, EMPTY_BOARD] {
        assert_eq!(
            render(&report_for(fixture, "mm")),
            golden(fixture),
            "{fixture}"
        );
    }
}

#[test]
fn natural_tone_preamp_is_the_jvms_maximal_run_minus_three_dangling_tracks() {
    if !parity::require_java_dir() {
        return;
    }
    let report = report_for(NATURAL_TONE_PREAMP, "mm");
    assert_eq!(report.violations.len(), 112);
    assert_eq!(report.unconnected_items.len(), 44);

    let golden = golden(NATURAL_TONE_PREAMP);
    let ours = render(&report);

    let mut golden_lines = golden.lines().peekable();
    let mut missing: Vec<String> = Vec::new();
    for line in ours.lines() {
        if line.starts_with("counts ") {
            assert_eq!(
                line,
                "counts violations=112 unconnectedItems=44 schematicParity=0"
            );
            let golden_counts = golden_lines.next().expect("golden has a counts line");
            assert_eq!(
                golden_counts,
                "counts violations=115 unconnectedItems=44 schematicParity=0"
            );
            continue;
        }
        loop {
            let next = golden_lines
                .next()
                .unwrap_or_else(|| panic!("port line not in the golden: {line}"));
            if next == line {
                break;
            }
            missing.push(next.to_string());
        }
    }
    assert_eq!(
        missing.len(),
        6,
        "expected three dropped track_dangling entries, got {missing:#?}"
    );
    assert_eq!(
        missing
            .iter()
            .filter(|l| l.as_str()
                == "V type=track_dangling severity=warning desc=Track has unconnected end")
            .count(),
        3,
    );
    let missing_uuids: Vec<&str> = missing
        .iter()
        .filter_map(|l| l.strip_prefix("I uuid="))
        .map(|l| l.split(' ').next().expect("a uuid"))
        .collect();
    assert_eq!(missing_uuids, vec!["1909", "1696", "1242"]);
}


#[test]
fn coordinates_are_in_a_plausible_mm_range() {
    if !parity::require_java_dir() {
        return;
    }
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
    if !parity::require_java_dir() {
        return;
    }
    let report = report_for(DEV_BOARD, "mm");
    assert!(report.violations[0].items[0].pos.y < 0.0);
    assert!(report.violations[0].items[0].pos.x > 0.0);
}

#[test]
fn unknown_coordinate_unit_falls_back_to_the_board_unit() {
    if !parity::require_java_dir() {
        return;
    }
    let (mut board, transform) = fixture_board(DEV_BOARD);
    assert_eq!(board.communication.unit, Unit::Um);
    let coords = DrcCoordinates {
        transform,
        board_unit: board.communication.unit,
    };
    let report = DesignRulesChecker::new(&mut board)
        .generate_report(&coords, &options(DEV_BOARD, "furlong"));
    assert_eq!(report.coordinate_units, "furlong");
    assert_eq!(report.violations[0].items[0].pos.x, 125_500.0);
    assert!(
        report.violations[0]
            .description
            .contains("expected: 50.0000 furlong")
    );
}

#[test]
fn mil_and_inch_scale() {
    if !parity::require_java_dir() {
        return;
    }
    let mm = report_for(DEV_BOARD, "mm").violations[0].items[0].pos.x;
    let mil = report_for(DEV_BOARD, "mil").violations[0].items[0].pos.x;
    let inch = report_for(DEV_BOARD, "inch").violations[0].items[0].pos.x;
    let um = report_for(DEV_BOARD, "um").violations[0].items[0].pos.x;

    assert_eq!(mm, 125.5);
    assert_eq!(um, 125_500.0);
    assert_eq!(mil, 125_500.0 / 25.4);
    assert_eq!(inch, 125_500.0 / 25_400.0);
}

#[test]
fn percent_four_f_uses_a_dot() {
    if !parity::require_java_dir() {
        return;
    }
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
fn smd_pins_are_classified_as_holes() {
    let mut board = smd_pad_board();
    assert_eq!(board.layer_structure().count(), 2);
    {
        let ctx = board.ctx();
        for id in [ItemId(2), ItemId(3)] {
            let item = board.get_item(id).expect("the two pins");
            assert_eq!(item.kind(), ItemKind::Pin);
            assert_eq!((item.first_layer(&ctx), item.last_layer(&ctx)), (0, 0));
        }
    }

    let coords = DrcCoordinates {
        transform: CoordinateTransform::new(1.0, 0.0, 0.0).expect("a unit scale"),
        board_unit: Unit::Um,
    };
    let report =
        DesignRulesChecker::new(&mut board).generate_report(&coords, &options("smd.dsn", "um"));
    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.violations[0].kind, "holeClearance");
    assert_eq!(
        report.violations[0].description,
        "Hole clearance violation between Pin [N2] and Pin [N1] \
         (expected: 200.0000 um, actual: 0.0000 um)"
    );
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

fn smd_pad_board() -> Board {
    let ls = two_layers();
    let mut padstacks = Padstacks::new(ls.clone());
    let mut pins = Vec::new();
    for (name, offset) in [("a", 0), ("b", 30)] {
        let shape = Shape::Tile(TileShape::Box(IntBox::from_coords(-50, -50, 50, 50)));
        let padstack = padstacks.add(name, vec![Some(shape), None], false, false);
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
                fr_geometry::Vector::ZERO,
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
