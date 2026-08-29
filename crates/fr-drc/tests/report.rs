//! Plan 5 Task 7: the four `io/kicad/KiCadDrc*.java` DTOs and
//! `DesignRulesChecker.generateReport` (`drc/DesignRulesChecker.java:210-290`, with its five
//! private helpers through `:533`).
//!
//! # Provenance
//!
//! `crates/fr-drc/tests/data/ReportProbe.java` runs the real `generateReport(source, "mm")` on the
//! clone's HEAD jar (plan-5 ruling 1) and writes the report as **normalised text**: `date` dropped
//! (it is `ZonedDateTime.now()` in Java, KiCadDrcReport.java:70, and injected in the port —
//! ruling 5), each entry's `items` array sorted by numeric uuid (it comes out of a `HashSet<Item>`
//! on the Java side, quirk #144), entry order kept. [`render`] renders the port's report in that
//! exact format. `tests/data/README.md` records the command and the hash-mode sweep.
//!
//! # The one fixture that is not byte-comparable
//!
//! Natural Tone Preamp's `violations` count is hash-dependent on the JVM — 114 or 115 over the
//! six runs of the sweep — because `generateReport` folds in `getAllUnconnectedItems`'
//! `track_dangling` entries and that phase's dedup drops whichever dangling trace a net entry's
//! hash-ordered `firstItem` happens to be (quirk #146). The port's ascending-id representatives
//! (ruling 3) drop **three**, so it emits 112. The committed transcript is the **maximal** run
//! (115 — four of the six modes, byte-identical), and
//! [`natural_tone_preamp_is_the_jvms_maximal_run_minus_three_dangling_tracks`] requires the port's
//! report to be that transcript with exactly three `track_dangling` entries removed, in place.
//! See Task 4's report and `tests/data/README.md`.

use fr_board::prelude::*;
use fr_drc::DesignRulesChecker;
use fr_drc::report::{
    DrcCoordinates, DrcReportOptions, KiCadDrcReport, KiCadDrcViolation, item_description,
};
use fr_dsn::{BoardReadResult, CoordinateTransform, DsnReadOptions};
use fr_geometry::{Area, IntBox, IntPoint, IntVector, Point, Polyline, Shape, TileShape};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

const DEV_BOARD: &str = "Issue575-drc_dev-board_4_hole_clearance_violations.dsn";
const BBD_MARS_64: &str = "Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations.dsn";
const NATURAL_TONE_PREAMP: &str = "Issue575-drc_Natural_Tone_Preamp_7_unconnected_items.dsn";
const EMPTY_BOARD: &str = "empty_board.dsn";

mod common;
use common::JAR_VERSION;

/// A real board plus the transform `Structure.createBoard` built for it — Java reaches both
/// through `board.communication` (DesignRulesChecker.java:507, `:510`); Plan 3 ruling A leaves the
/// transform in `fr-dsn`, so the port takes it as a parameter (plan-5 ruling 7).
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

/// The options `Freerouting.initializeDrc` fills (`Freerouting.java:335-352`), with the CLI's
/// hard-coded `"mm"` (quirk #151) and no quality score (that is Plan 8's, ruling 5).
fn options(source: &str, unit: &str) -> DrcReportOptions {
    DrcReportOptions {
        source: source.to_string(),
        coordinate_unit: unit.to_string(),
        // The port has no clock (ruling 5); the probe drops this field.
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

// ---------------------------------------------------------------------------------------------
// The report's shape (port of `DesignRulesCheckerTest.java:48-62`)
// ---------------------------------------------------------------------------------------------

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

    // `violations` is *all* clearance entries followed by *all* dangling entries
    // (DesignRulesChecker.java:231-233 then `:271-276`): two `holeClearance`, then eight
    // `track_dangling`.
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
    // Java's items are `[firstItem, secondItem]` (DesignRulesChecker.java:323-324) and
    // `getAllClearanceViolations` keeps the **higher**-id item's report (Task 3), so 278 precedes
    // 277. This is the one place the golden's uuid sort would have hidden the order.
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
    // `:414-416` — the two representatives and `allItems.size()`; severity `"warning"` (`:421`).
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
    // `getDetailedTraceDescription` (`:470-495`), used **only** for `track_dangling` (`:373-375`).
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

// ---------------------------------------------------------------------------------------------
// The JVM golden
// ---------------------------------------------------------------------------------------------

/// `ReportProbe.java`'s transcript format, exactly.
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
    // The transcript is the run in which the trace phase's dedup catches nothing (115
    // violations; four of the six hash modes, byte-identical). The port's ascending-id
    // representatives make three of the fixture's four `Trace`-represented net entries dangling,
    // so it drops three `track_dangling` entries — in place, because both sides walk the board's
    // items descending. Everything else, including the whole 44-entry `unconnectedItems` block,
    // is identical.
    //
    // **What this fixture cannot see.** Natural Tone Preamp has *zero* clearance violations
    // (`tests/data/*.incompletes.txt`: `totalCount=0`), so every one of its 112 violations comes
    // out of `getAllUnconnectedItems`. A regression anywhere in `convert_clearance_violation` —
    // the `holeClearance`/`clearance` split, the two `%.4f` clearances, the two-item `items`
    // array — is invisible here. That half is pinned by `three_fixtures_match_the_jvm_byte_for_byte`
    // on the dev board (2 `holeClearance`) and BBD Mars-64 (64 `holeClearance` + 12 `clearance`),
    // and corpus-wide by `p5t1`.
    if !parity::require_java_dir() {
        return;
    }
    let report = report_for(NATURAL_TONE_PREAMP, "mm");
    assert_eq!(report.violations.len(), 112);
    assert_eq!(report.unconnected_items.len(), 44);

    let golden = golden(NATURAL_TONE_PREAMP);
    let ours = render(&report);

    // Every line of ours appears in the golden, in order.
    let mut golden_lines = golden.lines().peekable();
    let mut missing: Vec<String> = Vec::new();
    for line in ours.lines() {
        if line.starts_with("counts ") {
            // The one header line that must differ: 115 vs 112.
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
    // Three dropped entries, each two lines (the `V` line and its one `I` line).
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
    // The three items themselves, so a *different* trio would fail rather than pass on the count:
    // each is the `firstItem` of a net entry whose connected group holds no `Pin`, which is what
    // the trace phase's dedup keys on (`:160`, quirk #146).
    let missing_uuids: Vec<&str> = missing
        .iter()
        .filter_map(|l| l.strip_prefix("I uuid="))
        .map(|l| l.split(' ').next().expect("a uuid"))
        .collect();
    assert_eq!(missing_uuids, vec!["1909", "1696", "1242"]);
}

// ---------------------------------------------------------------------------------------------
// `convertCoordinate` (DesignRulesChecker.java:504-530) and quirk #151
// ---------------------------------------------------------------------------------------------

#[test]
fn coordinates_are_in_a_plausible_mm_range() {
    // Port of `DrcCoordinateTest.java:27-71`, which guards a historical 10x bug: coordinates that
    // came out in board units rather than mm.
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
    // KiCad writes DSN with y growing downwards and negates on export, so the DSN — and therefore
    // `boardToDsn`, and therefore the report — carries negative y. This is *not* a sign flip in
    // the port: `CoordinateTransform.boardToDsn` is a pure scale-and-translate
    // (CoordinateTransform.java:44-48), and the JVM golden carries the same negatives.
    if !parity::require_java_dir() {
        return;
    }
    let report = report_for(DEV_BOARD, "mm");
    assert!(report.violations[0].items[0].pos.y < 0.0);
    assert!(report.violations[0].items[0].pos.x > 0.0);
}

#[test]
fn unknown_coordinate_unit_falls_back_to_the_board_unit() {
    // `:512-525`'s `else` arm. Unreachable from the CLI, which hard-codes `"mm"`
    // (Freerouting.java:336, quirk #151); the API path could reach it.
    if !parity::require_java_dir() {
        return;
    }
    let (mut board, transform) = fixture_board(DEV_BOARD);
    // Every Issue575 fixture declares `(unit um)`, so the fallback is observably not `"mm"`.
    assert_eq!(board.communication.unit, Unit::Um);
    let coords = DrcCoordinates {
        transform,
        board_unit: board.communication.unit,
    };
    let report = DesignRulesChecker::new(&mut board)
        .generate_report(&coords, &options(DEV_BOARD, "furlong"));
    assert_eq!(report.coordinate_units, "furlong");
    // 125.5 mm in the `"mm"` report; 125 500 um here.
    assert_eq!(report.violations[0].items[0].pos.x, 125_500.0);
    assert!(
        report.violations[0]
            .description
            .contains("expected: 50.0000 furlong")
    );
}

#[test]
fn mil_and_inch_scale() {
    // The two other reachable-only-from-the-API branches (`:516-521`).
    if !parity::require_java_dir() {
        return;
    }
    let mm = report_for(DEV_BOARD, "mm").violations[0].items[0].pos.x;
    let mil = report_for(DEV_BOARD, "mil").violations[0].items[0].pos.x;
    let inch = report_for(DEV_BOARD, "inch").violations[0].items[0].pos.x;
    let um = report_for(DEV_BOARD, "um").violations[0].items[0].pos.x;

    assert_eq!(mm, 125.5);
    assert_eq!(um, 125_500.0);
    // `Unit.scale(value, UM, MIL) = value * 1 / 25.4` (Unit.java:18-21).
    assert_eq!(mil, 125_500.0 / 25.4);
    assert_eq!(inch, 125_500.0 / 25_400.0);
}

#[test]
fn percent_four_f_uses_a_dot() {
    // Plan-5 ruling 6: Java's `String.formatted` follows the default `FORMAT` locale, so a German
    // JVM writes `0,0500`; `java_format_fixed` is locale-free.
    if !parity::require_java_dir() {
        return;
    }
    // A comma is legal *prose* here — "…, actual: …", "…F.Cu, length…" — so what is forbidden is
    // a comma sitting **between two digits**, which is what a `de_DE` JVM writes for `0,0500`.
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
    // …and the guard itself sees one when there is one.
    assert!(has_comma_decimal("expected: 0,0500 mm"));
}

// ---------------------------------------------------------------------------------------------
// `isHole` (DesignRulesChecker.java:424-430) — quirk #152
// ---------------------------------------------------------------------------------------------

#[test]
fn smd_pins_are_classified_as_holes() {
    // Java's `isHole` (`:424-431`) is `instanceof Via || instanceof Pin`, with a comment admitting the second
    // half "might include SMT pins". Two surface-mount pads on different nets, overlapping, come
    // out `holeClearance` rather than `clearance`.
    let mut board = smd_pad_board();
    // Both pads carry a shape on layer 0 only, on a two-layer board: surface mount, no drill.
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
        transform: CoordinateTransform::new(1.0, 0.0, 0.0),
        board_unit: Unit::Um,
    };
    let report =
        DesignRulesChecker::new(&mut board).generate_report(&coords, &options("smd.dsn", "um"));
    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.violations[0].kind, "holeClearance");
    // The surviving report is the **higher**-id item's (Task 3), so pin 3 (net N2) leads.
    assert_eq!(
        report.violations[0].description,
        "Hole clearance violation between Pin [N2] and Pin [N1] \
         (expected: 200.0000 um, actual: 0.0000 um)"
    );
}

// ---------------------------------------------------------------------------------------------
// `getItemDescription`'s variant table (DesignRulesChecker.java:439-460)
// ---------------------------------------------------------------------------------------------

#[test]
fn item_description_maps_every_item_variant() {
    // Four named kinds (`:442-452`) and Java's `getClass().getSimpleName()` for the rest. Nothing in the
    // fixture corpus puts one of the five fallback classes into a violation, so the table is
    // pinned here rather than by a golden.
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

// ---------------------------------------------------------------------------------------------
// The synthetic boards
// ---------------------------------------------------------------------------------------------

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

/// Two overlapping **single-layer** pads on different nets: items 2 (net 1) and 3 (net 2). The
/// board has two layers and each padstack has a shape only on layer 0, so both pins are SMD.
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

/// A two-layer board carrying one item of every [`Item`] variant, returned with their ids in
/// declaration order: outline, trace, pin, via, conduction area, obstacle area, via obstacle,
/// component obstacle, component outline.
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

    let mut ids = vec![ItemId(1)]; // `Board::new` inserts the board outline as item 1.
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
