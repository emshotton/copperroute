//! Plan 6 Task 10: `board.actions.ForcedPadRouter.checkForcedPad` (with `inFrontOfPad` and
//! `calcFromSide`) and `board.actions.ForcedViaInserter`'s `checkLayer` / `check` / the two
//! private helpers.
//!
//! # Where the numbers come from
//!
//! Every expectation below is **read off the HEAD jar**, not off this port. The probe is
//! `scripts/differential/java/probes/P6T10Probe.java`, committed with the exact `javac`/`java`
//! invocation in its header, and its stdout is committed verbatim as
//! `tests/data/p6t10-forced-via.txt`. The big tables (920 `inFrontOfPad` rows, 360 `checkLayer`
//! rows, 384 `ForcedViaInserter.check` rows and the 200-row random table) are compared against
//! that transcript row by row rather than pasted twice; the small, load-bearing rows — the
//! `inFrontOfPad` asymmetry that pins quirk #176, the shove-via budget gate, the arm Task 9 left
//! `unimplemented!()` — are additionally asserted as literals so the intent survives a
//! regenerated transcript.

use std::collections::{BTreeMap, BTreeSet};

use fr_board::BoardError;
use fr_board::ids::{ItemId, PadstackId};
use fr_board::prelude::*;
use fr_board::rules::ViaInfo;
use fr_geometry::{
    FloatPoint, IntBox, IntOctagon, IntPoint, IntVector, Line, Point, Polyline, Shape, ShapeOps,
    TileShape, Vector,
};
use fr_router::board_ext::{
    CheckDrillResult, DrillItemMover, ForcedPadRouter, ForcedViaInserter, TraceShover,
};

// =================================================================================================
// The committed JVM transcript
// =================================================================================================

const TRANSCRIPT: &str = include_str!("data/p6t10-forced-via.txt");

/// The `  …` rows of one `######## <mode>` section of the transcript, trimmed.
fn section(mode: &str) -> Vec<&'static str> {
    let header = format!("######## {mode}");
    let mut rows = Vec::new();
    let mut inside = false;
    for line in TRANSCRIPT.lines() {
        if line.starts_with("######## ") {
            inside = line == header;
            continue;
        }
        if inside && !line.starts_with('#') {
            rows.push(line.trim_end());
        }
    }
    assert!(!rows.is_empty(), "transcript section `{mode}` is empty");
    rows
}

/// The `key=value` fields of a probe row, e.g. `line=above fromSide=0 … -> true`.
fn field<'a>(row: &'a str, key: &str) -> &'a str {
    let needle = format!(" {key}=");
    let start = row
        .find(&needle)
        .unwrap_or_else(|| panic!("row `{row}` has no `{key}=`"))
        + needle.len();
    let rest = &row[start..];
    match rest.find(' ') {
        Some(end) => &rest[..end],
        None => rest,
    }
}

/// A probe field whose value is a `java.util.Arrays.toString` array — `[0, 0]`, which carries a
/// space and so cannot be read with [`field`] — or the bare word `null`.
fn bracketed_field<'a>(row: &'a str, key: &str) -> &'a str {
    let needle = format!(" {key}=");
    let start = row
        .find(&needle)
        .unwrap_or_else(|| panic!("row `{row}` has no `{key}=`"))
        + needle.len();
    let rest = &row[start..];
    if let Some(end) = rest.find(']') {
        &rest[..=end]
    } else {
        rest.split(' ').next().unwrap()
    }
}

/// Everything after the row's ` -> `.
fn answer(row: &str) -> &str {
    let start = row
        .find(" -> ")
        .unwrap_or_else(|| panic!("row `{row}` has no ` -> `"))
        + 4;
    row[start..].trim()
}

/// `IntOctagon[leftX,bottomY,rightX,topY,ulDiag,lrDiag,llDiag,urDiag]`, the probe's `oct()` form.
fn parse_octagon(text: &str) -> IntOctagon {
    let inner = text
        .strip_prefix("IntOctagon[")
        .and_then(|t| t.strip_suffix(']'))
        .unwrap_or_else(|| panic!("not an octagon literal: `{text}`"));
    let v: Vec<i32> = inner
        .split(',')
        .map(|s| s.trim().parse().unwrap())
        .collect();
    assert_eq!(v.len(), 8, "octagon literal `{text}`");
    IntOctagon::new(v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7])
}

/// `(x,y)`, the probe's point form.
fn parse_pair(text: &str) -> (i32, i32) {
    let inner = text
        .strip_prefix('(')
        .and_then(|t| t.strip_suffix(')'))
        .unwrap_or_else(|| panic!("not a point literal: `{text}`"));
    let (x, y) = inner.split_once(',').unwrap();
    (x.trim().parse().unwrap(), y.trim().parse().unwrap())
}

fn drill_result(name: &str) -> CheckDrillResult {
    match name {
        "DRILLABLE" => CheckDrillResult::Drillable,
        "DRILLABLE_WITH_ATTACH_SMD" => CheckDrillResult::DrillableWithAttachSmd,
        "NOT_DRILLABLE" => CheckDrillResult::NotDrillable,
        other => panic!("unknown CheckDrillResult `{other}`"),
    }
}

fn nets(field_value: &str) -> Vec<i32> {
    let inner = field_value.trim_start_matches('[').trim_end_matches(']');
    if inner.is_empty() {
        Vec::new()
    } else {
        inner
            .split(',')
            .map(|s| s.trim().parse().unwrap())
            .collect()
    }
}

// =================================================================================================
// The probe's boards, rebuilt from scratch
// =================================================================================================

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

fn four_layers() -> LayerStructure {
    LayerStructure::new(vec![
        Layer::new("l0", true),
        Layer::new("l1", true),
        Layer::new("l2", true),
        Layer::new("l3", true),
    ])
}

fn rules_with_wide_class(layers: LayerStructure, angle: AngleRestriction) -> BoardRules {
    let mut clearance_matrix = ClearanceMatrix::get_default_instance(&layers, 200);
    assert!(clearance_matrix.append_class("wide"));
    clearance_matrix.set_value_on_all_layers(2, 1, 600);
    clearance_matrix.set_value_on_all_layers(2, 2, 800);
    let mut rules = BoardRules::new(layers, clearance_matrix);
    rules.trace_angle_restriction = angle;
    rules
}

fn through_octagon() -> Shape {
    Shape::Tile(TileShape::Octagon(IntOctagon::new(
        -70, -70, 70, 70, -140, 140, -140, 140,
    )))
}

/// `P6T10Probe.build`, which is `P6T9Probe.build` verbatim.
fn probe_board(angle: AngleRestriction) -> Board {
    let mut padstacks = Padstacks::new(two_layers());
    let smd = padstacks.add(
        "smd",
        vec![
            Some(Shape::Tile(TileShape::Box(IntBox::from_coords(
                -50, -50, 50, 50,
            )))),
            None,
        ],
        false,
        false,
    );
    let through = padstacks.add(
        "thru",
        vec![Some(through_octagon()), Some(through_octagon())],
        true,
        false,
    );
    let mut packages = Packages::new();
    let package = packages.add(
        "pkg",
        vec![
            PackagePin::new("P1", smd, IntVector::new(-500, 0).into(), 0.0),
            PackagePin::new("P2", through, IntVector::new(500, 0).into(), 0.0),
        ],
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

    let mut board = Board::new(
        Vec::new(),
        0,
        BOUNDING_BOX,
        rules_with_wide_class(two_layers(), angle),
        BoardLibrary::new(padstacks, packages),
        components,
        Communication::default(),
    );
    let default_class = board.rules.get_default_net_class();
    board.rules.nets.add("N1", 1, false, default_class);
    board.rules.nets.add("N2", 1, false, default_class);
    board.rules.nets.add("N3", 1, false, default_class);

    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    board.insert_pin(1, 1, vec![1], 1, FixedState::Unfixed);
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[
            Point::new(-500, 0),
            Point::new(0, 0),
            Point::new(0, 400),
            Point::new(500, 400),
        ]),
        0,
        30,
        vec![1],
        1,
        FixedState::Unfixed,
    );
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[
            Point::new(-800, 300),
            Point::new(-800, 900),
            Point::new(300, 900),
        ]),
        0,
        40,
        vec![2],
        2,
        FixedState::Unfixed,
    );
    board
}

/// `P6T10Probe.buildFourLayer`: a 5x4 trace lattice on each of four layers plus sixteen big
/// through pins in four components.
fn four_layer_board() -> Board {
    let mut padstacks = Padstacks::new(four_layers());
    let _through = padstacks.add(
        "thru",
        vec![
            Some(through_octagon()),
            Some(through_octagon()),
            Some(through_octagon()),
            Some(through_octagon()),
        ],
        true,
        false,
    );
    let big_shape = Shape::Tile(TileShape::Octagon(IntOctagon::new(
        -200, -200, 200, 200, -400, 400, -400, 400,
    )));
    let big = padstacks.add(
        "big",
        vec![
            Some(big_shape.clone()),
            Some(big_shape.clone()),
            Some(big_shape.clone()),
            Some(big_shape),
        ],
        true,
        false,
    );
    let mut packages = Packages::new();
    let package = packages.add(
        "grid",
        vec![
            PackagePin::new("P1", big, IntVector::new(0, 0).into(), 0.0),
            PackagePin::new("P2", big, IntVector::new(1200, 0).into(), 0.0),
            PackagePin::new("P3", big, IntVector::new(0, 1200).into(), 0.0),
            PackagePin::new("P4", big, IntVector::new(1200, 1200).into(), 0.0),
        ],
        None,
        None,
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        true,
    );
    let mut components = Components::new();
    for cx in -1..=0 {
        for cy in -1..=0 {
            components.add_with_generated_name(
                Some(Point::new(cx * 2600 + 400, cy * 2600 + 400)),
                0.0,
                true,
                package,
            );
        }
    }

    let mut board = Board::new(
        Vec::new(),
        0,
        BOUNDING_BOX,
        rules_with_wide_class(four_layers(), AngleRestriction::None),
        BoardLibrary::new(padstacks, packages),
        components,
        Communication::default(),
    );
    let default_class = board.rules.get_default_net_class();
    board.rules.nets.add("N1", 1, false, default_class);
    board.rules.nets.add("N2", 1, false, default_class);
    board.rules.nets.add("N3", 1, false, default_class);
    for pin_no in 1..=16 {
        let component = if pin_no <= 4 {
            1
        } else if pin_no <= 8 {
            2
        } else if pin_no <= 12 {
            3
        } else {
            4
        };
        board.insert_pin(
            component,
            (pin_no - 1) % 4,
            vec![((pin_no - 1) % 3) + 1],
            1,
            FixedState::Unfixed,
        );
    }
    for layer in 0..4i32 {
        let net = (layer % 3) + 1;
        for i in -2..=2 {
            board.insert_trace_without_cleaning(
                Polyline::from_points(&[
                    Point::new(i * 1500 + layer * 100, -4000),
                    Point::new(i * 1500 + layer * 100, 4000),
                ]),
                layer as usize,
                40,
                vec![net],
                1,
                FixedState::Unfixed,
            );
        }
        for j in -1..=2 {
            board.insert_trace_without_cleaning(
                Polyline::from_points(&[
                    Point::new(-4000, j * 1700 + layer * 90),
                    Point::new(4000, j * 1700 + layer * 90),
                ]),
                layer as usize,
                40,
                vec![net],
                1,
                FixedState::Unfixed,
            );
        }
    }
    board
}

/// `P6T10Probe.fromSides()`'s lane board: [`probe_board`] plus five net-3 traces — a vertical
/// one at x = 3000, a horizontal one at y = 5000, and a three-sided pocket around (-3000, -3000)
/// — inserted in the probe's order, so a shape placed against them has a different **first**
/// acceptable border line depending on where the copper sits.
fn lane_board(angle: AngleRestriction) -> Board {
    let mut board = probe_board(angle);
    for corners in [
        [Point::new(3000, -4000), Point::new(3000, 4000)],
        [Point::new(-4000, 5000), Point::new(4000, 5000)],
        [Point::new(-3600, -3400), Point::new(-2400, -3400)],
        [Point::new(-2600, -3600), Point::new(-2600, -2400)],
        [Point::new(-3600, -2600), Point::new(-2400, -2600)],
    ] {
        board.insert_trace_without_cleaning(
            Polyline::from_points(&corners),
            0,
            200,
            vec![3],
            1,
            FixedState::Unfixed,
        );
    }
    board
}

/// `P6T10Probe.padShapes()`, in probe order.
fn pad_shapes() -> Vec<(&'static str, TileShape)> {
    vec![
        (
            "onNet1",
            TileShape::Box(IntBox::from_coords(-100, 100, 100, 300)),
        ),
        (
            "onNet2",
            TileShape::Box(IntBox::from_coords(-900, 800, -700, 1000)),
        ),
        (
            "onSmdPin",
            TileShape::Box(IntBox::from_coords(-600, -100, -400, 100)),
        ),
        (
            "onThruPin",
            TileShape::Box(IntBox::from_coords(400, -100, 600, 100)),
        ),
        (
            "freeSpace",
            TileShape::Box(IntBox::from_coords(2000, 2000, 2200, 2200)),
        ),
        (
            "offBoard",
            TileShape::Box(IntBox::from_coords(9900, 9900, 10100, 10100)),
        ),
        (
            "octagonOnNet1",
            TileShape::Octagon(IntOctagon::new(-100, 100, 100, 300, -300, 300, -100, 500)),
        ),
    ]
}

/// `P6T10Probe.frontLines()`, in probe order.
fn front_lines() -> Vec<(&'static str, Line)> {
    vec![
        (
            "above",
            Line::new(IntPoint::new(-400, 500), IntPoint::new(400, 500)),
        ),
        (
            "below",
            Line::new(IntPoint::new(-400, -500), IntPoint::new(400, -500)),
        ),
        (
            "left",
            Line::new(IntPoint::new(-500, -400), IntPoint::new(-500, 400)),
        ),
        (
            "right",
            Line::new(IntPoint::new(500, -400), IntPoint::new(500, 400)),
        ),
        (
            "through",
            Line::new(IntPoint::new(-400, 0), IntPoint::new(400, 0)),
        ),
        (
            "diagUp",
            Line::new(IntPoint::new(-400, -400), IntPoint::new(400, 400)),
        ),
        (
            "diagDown",
            Line::new(IntPoint::new(-400, 400), IntPoint::new(400, -400)),
        ),
        (
            "typoA",
            Line::new(IntPoint::new(0, 900), IntPoint::new(900, 0)),
        ),
        (
            "typoB",
            Line::new(IntPoint::new(900, 0), IntPoint::new(0, 900)),
        ),
        (
            "typoC",
            Line::new(IntPoint::new(-900, 900), IntPoint::new(900, -900)),
        ),
    ]
}

/// `P6T10Probe.FRONT_PAD`.
fn front_pad() -> TileShape {
    TileShape::Octagon(IntOctagon::new(-100, -100, 100, 100, -200, 200, -200, 200))
}

fn centre_of(shape: &TileShape) -> Point {
    Point::Int(shape.centre_of_gravity().round())
}

// =================================================================================================
// `ForcedPadRouter.inFrontOfPad`
// =================================================================================================

/// Probe mode `front`, all 160 grid rows plus the three fall-through rows.
#[test]
fn in_front_of_pad_agrees_with_the_jvm_on_every_probe_row() {
    let pad = front_pad();
    let lines = front_lines();
    let mut checked = 0usize;
    for row in section("front") {
        if !row.trim_start().starts_with("line=") {
            continue;
        }
        let label = field(row, "line");
        let (_, line) = lines
            .iter()
            .find(|(name, _)| *name == label)
            .unwrap_or_else(|| panic!("unknown probe line `{label}`"));
        let from_side: i32 = field(row, "fromSide").parse().unwrap();
        let width: i32 = field(row, "width").parse().unwrap();
        let with_sides: bool = field(row, "withSides").parse().unwrap();
        let expected: bool = answer(row).parse().unwrap();
        assert_eq!(
            ForcedPadRouter::in_front_of_pad(line, &pad, from_side, width, with_sides),
            expected,
            "probe row `{row}`"
        );
        checked += 1;
    }
    assert_eq!(
        checked, 320,
        "probe mode `front` should carry 10 lines x 8 sides x 2 withSides x 2 widths rows"
    );

    // `:205-208`: a `fromSide` outside 0..=7 warns and answers `true`.
    let above = &lines[0].1;
    assert!(ForcedPadRouter::in_front_of_pad(above, &pad, 8, 30, true));
    assert!(ForcedPadRouter::in_front_of_pad(above, &pad, -1, 30, true));
    // 600 randomised `(octagon, line, fromSide, width, withSides)` rows, so the eight-case switch
    // is exercised on more than one pad — a consolidated `min`/`max` that happened to agree on
    // the fixed octagon would show up here. 406 `false` / 194 `true` on the JVM.
    let mut rnd_rows = 0usize;
    for row in section("front") {
        let trimmed = row.trim_start();
        if !trimmed.starts_with("rnd i=") {
            continue;
        }
        let pad = parse_octagon(field(row, "pad"));
        let (ax, ay) = parse_pair(field(row, "a"));
        let (bx, by) = parse_pair(field(row, "b"));
        let line = Line::new(IntPoint::new(ax, ay), IntPoint::new(bx, by));
        let expected: bool = answer(row).parse().unwrap();
        assert_eq!(
            ForcedPadRouter::in_front_of_pad(
                &line,
                &TileShape::Octagon(pad),
                field(row, "fromSide").parse().unwrap(),
                field(row, "width").parse().unwrap(),
                field(row, "withSides").parse().unwrap(),
            ),
            expected,
            "probe row `{row}`"
        );
        rnd_rows += 1;
    }
    assert_eq!(rnd_rows, 600);

    // The trap `:59-62` sets: "not a box and not an octagon" is **not** the same predicate as
    // "not an int octagon". A triangle whose three sides are all multiples of 45 degrees passes
    // `Simplex.isIntOctagon` (Simplex.java:365-379), so the switch runs on its bounding octagon.
    //
    // ```text
    //   fortyFiveTriangle isIntOctagon=true -> false
    // ```
    let forty_five_triangle = TileShape::get_instance_from_points(&[
        IntPoint::new(0, 0),
        IntPoint::new(500, 0),
        IntPoint::new(0, 500),
    ]);
    assert!(
        forty_five_triangle.is_int_octagon(),
        "probe `isIntOctagon=true`"
    );
    assert!(!ForcedPadRouter::in_front_of_pad(
        above,
        &forty_five_triangle,
        0,
        30,
        true
    ));
    // `:59-62` proper: a genuinely non-45-degree simplex answers `true` for every `fromSide`
    // without looking at the line.
    //
    // ```text
    //   nonOctagon isIntOctagon=false fromSide=0..7 -> true
    // ```
    let skew_triangle = TileShape::get_instance_from_points(&[
        IntPoint::new(0, 0),
        IntPoint::new(500, 100),
        IntPoint::new(0, 500),
    ]);
    assert!(
        !skew_triangle.is_int_octagon(),
        "probe `isIntOctagon=false`"
    );
    for from_side in 0..8 {
        assert!(ForcedPadRouter::in_front_of_pad(
            above,
            &skew_triangle,
            from_side,
            30,
            true
        ));
    }
}

/// **Quirk #176.** `inFrontOfPad`'s `case 0` third disjunct (ForcedPadRouter.java:78) reads
/// `Math.min(lineA.x + lineA.y, lineB.x + lineB.x)` — `lineB.x` twice, where all seven sibling
/// cases and the two neighbouring disjuncts read `x + y`. The observable consequence is that at
/// `fromSide = 0` the answer depends on **which end point of the line is `a`**, even though the
/// two `Line`s describe the same geometry; `fromSide = 7`, whose corresponding disjunct is
/// spelled correctly, is symmetric on the same pair.
///
/// Probe mode `front`:
///
/// ```text
///   line=typoA fromSide=0 width=30 withSides=false -> true
///   line=typoB fromSide=0 width=30 withSides=false -> false
///   line=typoA fromSide=7 width=30 withSides=false -> true
///   line=typoB fromSide=7 width=30 withSides=false -> true
/// ```
#[test]
fn in_front_of_pad_is_asymmetric_in_a_and_b_at_from_side_zero() {
    let pad = front_pad();
    let forwards = Line::new(IntPoint::new(0, 900), IntPoint::new(900, 0));
    let backwards = Line::new(IntPoint::new(900, 0), IntPoint::new(0, 900));
    for width in [30, 300] {
        for with_sides in [false, true] {
            assert!(
                ForcedPadRouter::in_front_of_pad(&forwards, &pad, 0, width, with_sides),
                "probe `line=typoA fromSide=0 -> true`"
            );
            assert!(
                !ForcedPadRouter::in_front_of_pad(&backwards, &pad, 0, width, with_sides),
                "probe `line=typoB fromSide=0 -> false` — the `lineB.x + lineB.x` typo"
            );
            // The correctly spelled sibling: symmetric.
            assert!(ForcedPadRouter::in_front_of_pad(
                &forwards, &pad, 7, width, with_sides
            ));
            assert!(ForcedPadRouter::in_front_of_pad(
                &backwards, &pad, 7, width, with_sides
            ));
        }
    }
}

// =================================================================================================
// `ForcedPadRouter.checkForcedPad`
// =================================================================================================

/// Probe mode `pad`: the full `shape x nets x copperSharing x onlyFront` grid in both angle
/// regimes, plus the recursion-budget ladder, the `ignoreItems` row and the shove-via block.
#[test]
fn check_forced_pad_agrees_with_the_jvm_on_every_probe_row() {
    let mut angle = AngleRestriction::None;
    let mut board = probe_board(angle);
    let mut shapes = pad_shapes();
    let mut free_via = ItemId(0);
    let mut fixed_via = ItemId(0);
    let mut grid_rows = 0usize;
    let mut via_rows = 0usize;

    for row in section("pad") {
        if let Some(rest) = row.strip_prefix("mode=pad angle=") {
            angle = match rest {
                "NONE" => AngleRestriction::None,
                "NINETY_DEGREE" => AngleRestriction::NinetyDegree,
                other => panic!("unknown angle `{other}`"),
            };
            board = probe_board(angle);
            shapes = pad_shapes();
            continue;
        }
        let trimmed = row.trim_start();
        if let Some(rest) = trimmed.strip_prefix("freeViaId=") {
            let (free_txt, fixed_txt) = rest.split_once(" fixedViaId=").unwrap();
            let through = PadstackId(board.library.padstacks.get_by_name("thru").unwrap().no);
            free_via = board
                .insert_via(
                    through,
                    Point::new(2000, 2000),
                    vec![3],
                    1,
                    FixedState::Unfixed,
                    false,
                )
                .unwrap();
            fixed_via = board
                .insert_via(
                    through,
                    Point::new(2600, 2000),
                    vec![3],
                    1,
                    FixedState::ShoveFixed,
                    false,
                )
                .unwrap();
            assert_eq!(free_via.0.to_string(), free_txt, "probe `{row}`");
            assert_eq!(fixed_via.0.to_string(), fixed_txt, "probe `{row}`");
            continue;
        }
        if trimmed.starts_with("shape=") {
            let label = field(row, "shape");
            let (_, shape) = shapes.iter().find(|(n, _)| *n == label).unwrap();
            let from_side = ShapeEntrySide::from_point(&centre_of(shape), shape);
            assert_eq!(
                from_side.no.to_string(),
                field(row, "fromSide"),
                "probe `{row}`"
            );
            let result = ForcedPadRouter::check_forced_pad(
                &mut board,
                shape,
                &from_side,
                0,
                &nets(field(row, "nets")),
                1,
                field(row, "copperSharing").parse().unwrap(),
                None,
                20,
                5,
                field(row, "onlyFront").parse().unwrap(),
                None,
            );
            assert_eq!(result, drill_result(answer(row)), "probe `{row}`");
            grid_rows += 1;
            continue;
        }
        if trimmed.starts_with("budget onNet1 ") {
            let (_, shape) = &shapes[0];
            let from_side = ShapeEntrySide::from_point(&centre_of(shape), shape);
            let result = ForcedPadRouter::check_forced_pad(
                &mut board,
                shape,
                &from_side,
                0,
                &[3],
                1,
                false,
                None,
                field(row, "depth").parse().unwrap(),
                field(row, "viaDepth").parse().unwrap(),
                false,
                None,
            );
            assert_eq!(result, drill_result(answer(row)), "probe `{row}`");
            continue;
        }
        if trimmed.starts_with("ignoringItem4 onNet1 ") {
            let (_, shape) = &shapes[0];
            let from_side = ShapeEntrySide::from_point(&centre_of(shape), shape);
            let result = ForcedPadRouter::check_forced_pad(
                &mut board,
                shape,
                &from_side,
                0,
                &[3],
                1,
                false,
                Some(&[ItemId(4)]),
                20,
                5,
                false,
                None,
            );
            assert_eq!(result, drill_result(answer(row)), "probe `{row}`");
            continue;
        }
        if trimmed.starts_with("via shape=") {
            let label = field(row, "shape");
            let shape = match label {
                "overFreeVia" => TileShape::Box(IntBox::from_coords(1900, 1900, 2100, 2100)),
                "overFixedVia" => TileShape::Box(IntBox::from_coords(2500, 1900, 2700, 2100)),
                "overBothVias" => TileShape::Box(IntBox::from_coords(1900, 1900, 2700, 2100)),
                other => panic!("unknown via shape `{other}`"),
            };
            let from_side = ShapeEntrySide::from_point(&centre_of(&shape), &shape);
            assert_eq!(
                from_side.no.to_string(),
                field(row, "fromSide"),
                "probe `{row}`"
            );
            let result = ForcedPadRouter::check_forced_pad(
                &mut board,
                &shape,
                &from_side,
                0,
                &nets(field(row, "nets")),
                1,
                false,
                None,
                20,
                field(row, "viaDepth").parse().unwrap(),
                false,
                None,
            );
            assert_eq!(result, drill_result(answer(row)), "probe `{row}`");
            via_rows += 1;
            continue;
        }
        panic!("unhandled probe row `{row}`");
    }
    assert_eq!(grid_rows, 168, "7 shapes x 3 nets x 2 x 2 x 2 regimes");
    assert_eq!(via_rows, 36, "3 shapes x 3 via depths x 2 nets x 2 regimes");
    let _ = (free_via, fixed_via);
}

/// The check methods must not touch the item set: `checkForcedPad` builds substitute trace pieces
/// and consults the shove algorithms, but never inserts. (It *does* advance the board's item-id
/// counter — `ShapeTraceEntries.nextSubstituteTracePiece` constructs real `PolylineTrace`s, which
/// Task 9 §9.3 records — so this pins `structural_hash`, not the raw counter.)
#[test]
fn check_forced_pad_does_not_mutate_the_board() {
    let mut board = probe_board(AngleRestriction::None);
    let before = board.structural_hash();
    for (_, shape) in pad_shapes() {
        let from_side = ShapeEntrySide::from_point(&centre_of(&shape), &shape);
        for copper_sharing in [false, true] {
            ForcedPadRouter::check_forced_pad(
                &mut board,
                &shape,
                &from_side,
                0,
                &[3],
                1,
                copper_sharing,
                None,
                20,
                5,
                false,
                None,
            );
        }
    }
    assert_eq!(board.structural_hash(), before);
}

// =================================================================================================
// The cycle Task 9 left open
// =================================================================================================

/// Probe mode `drill`. Task 9 could pin only `viaId=7` and `viaId=8` — the two arms that answer
/// before `checkForcedPad`; every other row here reaches the arm that was `unimplemented!()`
/// until this task, so this test is the one that proves the cycle is closed.
///
/// ```text
///   check viaId=6 delta=(300,0) result=true ignoreSize=1
///   check viaId=7 delta=(300,0) result=false ignoreSize=0
///   check viaId=8 delta=(300,0) result=false ignoreSize=0
///   check viaId=6 delta=(-1500,-2000) result=false ignoreSize=1
///   check viaId=6 delta=(-2000,-1600) result=true ignoreSize=1
///   check viaId=6 delta=(-1500,-1900) result=false ignoreSize=1
///   check viaId=6 delta=(300,0) viaDepth=0 result=true
/// ```
#[test]
fn drill_item_mover_check_answers_the_arm_task_nine_left_unimplemented() {
    for angle in [AngleRestriction::None, AngleRestriction::NinetyDegree] {
        let mut board = probe_board(angle);
        let through = PadstackId(board.library.padstacks.get_by_name("thru").unwrap().no);
        let free = board
            .insert_via(
                through,
                Point::new(2000, 2000),
                vec![3],
                1,
                FixedState::Unfixed,
                false,
            )
            .unwrap();
        let fixed = board
            .insert_via(
                through,
                Point::new(3000, 2000),
                vec![3],
                1,
                FixedState::ShoveFixed,
                false,
            )
            .unwrap();
        let on_pin = board
            .insert_via(
                through,
                Point::new(500, 0),
                vec![1],
                1,
                FixedState::Unfixed,
                false,
            )
            .unwrap();
        assert_eq!((free, fixed, on_pin), (ItemId(6), ItemId(7), ItemId(8)));

        let delta = Vector::from(IntVector::new(300, 0));
        let ignore: Vec<ItemId> = Vec::new();
        assert!(
            DrillItemMover::check(&mut board, free, &delta, 20, 5, Some(&ignore), None),
            "probe `check viaId=6 delta=(300,0) result=true`"
        );
        // quirk #175 (fixed: T10): Java's `:63` appended the drill item to the **caller's** list,
        // so the jar's probe prints `ignoreSize=1` after this successful check. The collection is
        // copied unconditionally now, the way `shoveVias:220-223` already did, and the parameter
        // is a shared slice — so the caller's list is untouched. The routing answer above, which
        // is what the probe row actually measures, is unchanged.
        assert!(ignore.is_empty());
        for via in [fixed, on_pin] {
            let ignore: Vec<ItemId> = Vec::new();
            assert!(!DrillItemMover::check(
                &mut board,
                via,
                &delta,
                20,
                5,
                Some(&ignore),
                None
            ));
            assert!(ignore.is_empty());
        }
        for (dx, dy, expected) in [
            (-1500, -2000, false),
            (-2000, -1600, true),
            (-1500, -1900, false),
        ] {
            let ignore: Vec<ItemId> = Vec::new();
            assert_eq!(
                DrillItemMover::check(
                    &mut board,
                    free,
                    &Vector::from(IntVector::new(dx, dy)),
                    20,
                    5,
                    Some(&ignore),
                    None,
                ),
                expected,
                "probe `check viaId=6 delta=({dx},{dy}) result={expected}` ({angle:?})"
            );
            // quirk #175 (fixed: T10), as above: the jar's `ignoreSize=1` is the caller's list
            // being mutated by a check, and it no longer is.
            assert!(ignore.is_empty());
        }
        let ignore: Vec<ItemId> = Vec::new();
        assert!(
            DrillItemMover::check(&mut board, free, &delta, 20, 0, Some(&ignore), None),
            "probe `check viaId=6 delta=(300,0) viaDepth=0 result=true`"
        );
    }
}

// =================================================================================================
// `ForcedPadRouter.calcFromSide` and `ForcedViaInserter.calculateFromSide`
// =================================================================================================

/// Probe mode `side`, the `calcFromSide` block. Note every answer has `border=null`: Java builds
/// `new ShapeEntrySide(i, null)` (`:480`, `:488`), so the border intersection is never filled in
/// by this calculator — unlike `calculateFromSide`, which always fills it.
#[test]
fn calc_from_side_agrees_with_the_jvm() {
    let mut angle = AngleRestriction::None;
    let mut board = probe_board(angle);
    let shapes = pad_shapes();
    let mut checked = 0usize;
    for row in section("side") {
        if let Some(rest) = row.strip_prefix("mode=side angle=") {
            angle = match rest {
                "NONE" => AngleRestriction::None,
                "NINETY_DEGREE" => AngleRestriction::NinetyDegree,
                other => panic!("unknown angle `{other}`"),
            };
            board = probe_board(angle);
            continue;
        }
        let trimmed = row.trim_start();
        if !trimmed.starts_with("calcFromSide ") {
            continue;
        }
        let label = field(row, "shape");
        let (_, shape) = shapes.iter().find(|(n, _)| *n == label).unwrap();
        let offset: i32 = field(row, "offset").parse().unwrap();
        let side =
            ForcedPadRouter::calc_from_side(&mut board, shape, &centre_of(shape), 0, offset, 1);
        let expected_no: i32 = answer(row)
            .strip_prefix("no=")
            .unwrap()
            .split(' ')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(side.no, expected_no, "probe row `{row}`");
        assert!(
            side.border_intersection.is_none(),
            "probe row `{row}` says `border=null`"
        );
        checked += 1;
    }
    assert_eq!(checked, 42, "7 shapes x 3 offsets x 2 regimes");
}

/// Probe mode `side`, the `lane` block: 504 rows whose answers are **not** all `-1` and `0`
/// (`no` takes -1, 0, 1 and 3), so the order in which `calcFromSide` sweeps
/// `offsetShape.borderLine(i)` — and its fall-back second sweep at clearance class 0 — are
/// pinned rather than merely exercised.
#[test]
fn calc_from_side_walks_javas_border_line_order() {
    let mut board = lane_board(AngleRestriction::None);
    let mut checked = 0usize;
    let mut seen: BTreeSet<i32> = BTreeSet::new();
    for row in section("side") {
        if let Some(rest) = row.strip_prefix("mode=side lanes angle=") {
            board = lane_board(match rest {
                "NONE" => AngleRestriction::None,
                "NINETY_DEGREE" => AngleRestriction::NinetyDegree,
                other => panic!("unknown angle `{other}`"),
            });
            continue;
        }
        let trimmed = row.trim_start();
        if !trimmed.starts_with("lane centre=") {
            continue;
        }
        let (cx, cy) = parse_pair(field(row, "centre"));
        let half: i32 = field(row, "half").parse().unwrap();
        let shape = TileShape::Box(IntBox::from_coords(
            cx - half,
            cy - half,
            cx + half,
            cy + half,
        ));
        let side = ForcedPadRouter::calc_from_side(
            &mut board,
            &shape,
            &Point::new(cx, cy),
            0,
            field(row, "offset").parse().unwrap(),
            field(row, "cc").parse().unwrap(),
        );
        let expected_no: i32 = answer(row)
            .strip_prefix("no=")
            .unwrap()
            .split(' ')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(side.no, expected_no, "probe row `{row}`");
        assert!(side.border_intersection.is_none(), "probe row `{row}`");
        seen.insert(expected_no);
        checked += 1;
    }
    assert_eq!(
        checked, 504,
        "14 centres x 2 half widths x 3 offsets x 3 classes x 2 regimes"
    );
    assert_eq!(
        seen,
        BTreeSet::from([-1, 0, 1, 3]),
        "the probe's answers must span more than `NOT_CALCULATED` and side 0"
    );
}

/// Probe mode `side`, the `calculateFromSide` block: the orthogonal sweep (`:384-420`), the
/// diagonal fallback (`:424-459`) and the two `null` arms.
#[test]
fn calculate_from_side_agrees_with_the_jvm() {
    let via_shape = TileShape::Octagon(IntOctagon::new(-100, -100, 100, 100, -200, 200, -200, 200));
    let rooms: Vec<(&str, TileShape)> = vec![
        (
            "wide",
            TileShape::Box(IntBox::from_coords(-2000, -2000, 2000, 2000)),
        ),
        (
            "tallNarrow",
            TileShape::Box(IntBox::from_coords(-150, -2000, 150, 2000)),
        ),
        (
            "flatWide",
            TileShape::Box(IntBox::from_coords(-2000, -150, 2000, 150)),
        ),
        (
            "upperRight",
            TileShape::Box(IntBox::from_coords(0, 0, 2000, 2000)),
        ),
        (
            "tiny",
            TileShape::Box(IntBox::from_coords(-120, -120, 120, 120)),
        ),
        (
            "diagonalOnly",
            TileShape::get_instance_from_points(&[
                IntPoint::new(-1500, 0),
                IntPoint::new(0, -1500),
                IntPoint::new(1500, 0),
                IntPoint::new(0, 1500),
            ]),
        ),
    ];
    let mut checked = 0usize;
    for row in section("side") {
        let trimmed = row.trim_start();
        if !trimmed.starts_with("calculateFromSide room=") {
            continue;
        }
        let label = field(row, "room");
        let (_, room) = rooms.iter().find(|(n, _)| *n == label).unwrap();
        let dist: f64 = field(row, "dist").parse().unwrap();
        let is_90: bool = field(row, "is90").parse().unwrap();
        let actual = ForcedViaInserter::calculate_from_side(
            &FloatPoint::new(0.0, 0.0),
            &via_shape,
            &room.to_simplex(),
            dist,
            is_90,
        );
        let expected = answer(row);
        match actual {
            None => assert_eq!(expected, "null", "probe row `{row}`"),
            Some(side) => {
                let border = side
                    .border_intersection
                    .expect("calculateFromSide fills it");
                let rendered = format!("no={} border=({:.4},{:.4})", side.no, border.x, border.y);
                assert_eq!(rendered, expected, "probe row `{row}`");
            }
        }
        checked += 1;
    }
    assert_eq!(checked, 36, "6 rooms x 3 distances x 2 regimes");
}

// =================================================================================================
// `ForcedViaInserter.holeCheckShape`
// =================================================================================================

/// Probe mode `hole`:
///
/// ```text
///   holeClearance=0 padstack=thru drillRadius=31.5000 -> null
///   holeClearance=100 padstack=thru drillRadius=31.5000 -> Circle[858,858..1142,1142]
///   holeClearance=100 padstack=smd drillRadius=22.5000 -> Circle[867,867..1133,1133]
///   holeClearance=400 padstack=thru drillRadius=31.5000 -> Circle[558,558..1442,1442]
///   holeClearance=400 padstack=smd drillRadius=22.5000 -> Circle[567,567..1433,1433]
/// ```
#[test]
fn hole_check_shape_agrees_with_the_jvm() {
    let mut board = probe_board(AngleRestriction::None);
    let location = Point::new(1000, 1000);
    for row in section("hole") {
        let trimmed = row.trim_start();
        if !trimmed.starts_with("holeClearance=") {
            continue;
        }
        let clearance: i32 = row
            .split("holeClearance=")
            .nth(1)
            .unwrap()
            .split(' ')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        board.rules.set_hole_clearance(clearance);
        let name = field(row, "padstack");
        let padstack = PadstackId(board.library.padstacks.get_by_name(name).unwrap().no);
        let actual = ForcedViaInserter::hole_check_shape(&board, padstack, &location);
        let expected = answer(row);
        match actual {
            None => assert_eq!(expected, "null", "probe row `{row}`"),
            Some(shape) => {
                let bb = shape.bounding_box();
                let rendered = format!("Circle[{},{}..{},{}]", bb.ll.x, bb.ll.y, bb.ur.x, bb.ur.y);
                assert_eq!(rendered, expected, "probe row `{row}`");
            }
        }
    }
}

// =================================================================================================
// `ForcedViaInserter.checkLayer`
// =================================================================================================

/// `P6T10Probe.checkLayer`'s five probe spots, in order.
fn layer_spots() -> Vec<(&'static str, Point)> {
    vec![
        ("onNet1", Point::new(0, 200)),
        ("onNet2", Point::new(-800, 900)),
        ("onSmdPin", Point::new(-500, 0)),
        ("onThruPin", Point::new(500, 0)),
        ("freeSpace", Point::new(2000, 2000)),
    ]
}

/// Probe mode `layer`, every row of the `spot x radius x attachSmd x thw x nets` grid in both
/// angle regimes, plus the `tinyRoom` row where `calculateFromSide` answers `null` (`:70-72`).
#[test]
fn check_layer_agrees_with_the_jvm_on_every_probe_row() {
    let room = TileShape::Box(IntBox::from_coords(-3000, -3000, 3000, 3000));
    let tiny_room = TileShape::Box(IntBox::from_coords(-10, -10, 10, 10));
    let spots = layer_spots();
    let mut board = probe_board(AngleRestriction::None);
    let mut checked = 0usize;
    for row in section("layer") {
        if let Some(rest) = row.strip_prefix("mode=layer angle=") {
            board = probe_board(match rest {
                "NONE" => AngleRestriction::None,
                "NINETY_DEGREE" => AngleRestriction::NinetyDegree,
                other => panic!("unknown angle `{other}`"),
            });
            continue;
        }
        let trimmed = row.trim_start();
        if trimmed.starts_with("tinyRoom ") {
            let result = ForcedViaInserter::check_layer(
                &mut board,
                60.0,
                1,
                false,
                &tiny_room,
                &Point::new(2000, 2000),
                0,
                &[3],
                20,
                5,
                30,
                1,
            );
            assert_eq!(result, drill_result(answer(row)), "probe row `{row}`");
            continue;
        }
        let label = field(row, "spot");
        let (_, location) = spots.iter().find(|(n, _)| *n == label).unwrap();
        let result = ForcedViaInserter::check_layer(
            &mut board,
            field(row, "radius").parse().unwrap(),
            1,
            field(row, "attachSmd").parse().unwrap(),
            &room,
            location,
            0,
            &nets(field(row, "nets")),
            20,
            5,
            field(row, "thw").parse().unwrap(),
            1,
        );
        assert_eq!(result, drill_result(answer(row)), "probe row `{row}`");
        checked += 1;
    }
    assert_eq!(
        checked, 360,
        "5 spots x 3 radii x 2 x 3 thw x 2 nets x 2 regimes"
    );
}

/// `:118-124`: the via phase and the trace phase each answer a `CheckDrillResult`, and
/// `DRILLABLE_WITH_ATTACH_SMD` from **either** wins. On the probe board the SMD pin at (-500, 0)
/// is what produces it, and `attachSmdAllowed` only reaches the via phase — the trace phase
/// hard-codes `true` at `:111` — so the promotion is observable from both sides.
#[test]
fn check_layer_promotes_to_attach_smd_when_either_phase_does() {
    let room = TileShape::Box(IntBox::from_coords(-3000, -3000, 3000, 3000));
    let mut board = probe_board(AngleRestriction::None);
    let on_smd = Point::new(-500, 0);

    // Via phase only (`traceHalfWidth = 0` returns at `:92-94`), with copper sharing allowed.
    assert_eq!(
        ForcedViaInserter::check_layer(
            &mut board,
            60.0,
            1,
            true,
            &room,
            &on_smd,
            0,
            &[1],
            20,
            5,
            0,
            1
        ),
        CheckDrillResult::DrillableWithAttachSmd
    );
    // Via phase refuses without it.
    assert_eq!(
        ForcedViaInserter::check_layer(
            &mut board,
            60.0,
            1,
            false,
            &room,
            &on_smd,
            0,
            &[1],
            20,
            5,
            0,
            1
        ),
        CheckDrillResult::NotDrillable
    );
    // `viaRadius <= 0` skips the via phase entirely (`:43-45`) — DRILLABLE, no promotion.
    assert_eq!(
        ForcedViaInserter::check_layer(
            &mut board,
            0.0,
            1,
            false,
            &room,
            &on_smd,
            0,
            &[1],
            20,
            5,
            400,
            1
        ),
        CheckDrillResult::Drillable
    );
    // Both phases run and the trace phase (whose `copperSharingAllowed` is the hard-coded `true`
    // at `:111`) supplies the promotion.
    assert_eq!(
        ForcedViaInserter::check_layer(
            &mut board,
            60.0,
            1,
            true,
            &room,
            &on_smd,
            0,
            &[1],
            20,
            5,
            30,
            1
        ),
        CheckDrillResult::DrillableWithAttachSmd
    );
}

/// `:88-90` and `:117-119`: a `NOT_DRILLABLE` from either phase is returned immediately.
///
/// Probe mode `layer` supplies both halves. The through pin at (500, 0) is unshovable and copper
/// sharing is off, so **phase 1** refuses whatever the trace half width is:
///
/// ```text
///   spot=onThruPin radius=60.0000 attachSmd=false thw=0 nets=[3] -> NOT_DRILLABLE
/// ```
///
/// while at (0, 200) the 60-unit via clears the net-1 trace but a 400-unit start-trace circle
/// does not, so **phase 2** is the one that refuses:
///
/// ```text
///   spot=onNet1 radius=60.0000 attachSmd=false thw=0   nets=[3] -> DRILLABLE
///   spot=onNet1 radius=60.0000 attachSmd=false thw=30  nets=[3] -> DRILLABLE
///   spot=onNet1 radius=60.0000 attachSmd=false thw=400 nets=[3] -> NOT_DRILLABLE
/// ```
#[test]
fn check_layer_short_circuits_on_the_first_not_drillable() {
    let room = TileShape::Box(IntBox::from_coords(-3000, -3000, 3000, 3000));
    for angle in [AngleRestriction::None, AngleRestriction::NinetyDegree] {
        let mut board = probe_board(angle);
        // Phase 1 refuses.
        for thw in [0, 30, 400] {
            assert_eq!(
                ForcedViaInserter::check_layer(
                    &mut board,
                    60.0,
                    1,
                    false,
                    &room,
                    &Point::new(500, 0),
                    0,
                    &[3],
                    20,
                    5,
                    thw,
                    1
                ),
                CheckDrillResult::NotDrillable,
                "probe `spot=onThruPin radius=60.0000 attachSmd=false thw={thw} nets=[3]`"
            );
        }
        // Phase 1 passes and phase 2 decides.
        for (thw, expected) in [
            (0, CheckDrillResult::Drillable),
            (30, CheckDrillResult::Drillable),
            (400, CheckDrillResult::NotDrillable),
        ] {
            assert_eq!(
                ForcedViaInserter::check_layer(
                    &mut board,
                    60.0,
                    1,
                    false,
                    &room,
                    &Point::new(0, 200),
                    0,
                    &[3],
                    20,
                    5,
                    thw,
                    1
                ),
                expected,
                "probe `spot=onNet1 radius=60.0000 attachSmd=false thw={thw} nets=[3]`"
            );
        }
        // `location` is not an `IntPoint` (`:46-48`) — refused before either phase runs.
        assert_eq!(
            ForcedViaInserter::check_layer(
                &mut board,
                60.0,
                1,
                false,
                &room,
                &Point::Rational(fr_geometry::RationalPoint::new(
                    3.into(),
                    3.into(),
                    2.into()
                )),
                0,
                &[3],
                20,
                5,
                0,
                1
            ),
            CheckDrillResult::NotDrillable
        );
    }
}

/// The brief's `0 diffs` table: 200 pseudo-random `(location, layer, net, radius, halfWidth)`
/// triples through `checkLayer` on a four-layer board with sixteen unshovable pins and a trace
/// lattice on every layer. Probe mode `rand`; 108 `DRILLABLE` and 92 `NOT_DRILLABLE` on the JVM.
#[test]
fn check_layer_agrees_with_the_jvm_on_two_hundred_random_triples() {
    let mut board = four_layer_board();
    let room = TileShape::Box(IntBox::from_coords(-5000, -5000, 5000, 5000));
    let mut diffs = Vec::new();
    let mut checked = 0usize;
    for row in section("rand") {
        let trimmed = row.trim_start();
        if !trimmed.starts_with("i=") {
            continue;
        }
        let i: usize = trimmed
            .strip_prefix("i=")
            .unwrap()
            .split(' ')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let x: i32 = field(row, "x").parse().unwrap();
        let y: i32 = field(row, "y").parse().unwrap();
        let layer: usize = field(row, "layer").parse().unwrap();
        let net: i32 = field(row, "net").parse().unwrap();
        let radius: f64 = field(row, "radius").parse().unwrap();
        let thw: i32 = field(row, "thw").parse().unwrap();
        let result = ForcedViaInserter::check_layer(
            &mut board,
            radius,
            1,
            i.is_multiple_of(3),
            &room,
            &Point::new(x, y),
            layer,
            &[net],
            20,
            5,
            thw,
            1,
        );
        if result != drill_result(answer(row)) {
            diffs.push(format!("{row} but the port answers {result:?}"));
        }
        checked += 1;
    }
    assert_eq!(checked, 200);
    assert!(
        diffs.is_empty(),
        "0 diffs expected, got:\n{}",
        diffs.join("\n")
    );
}

// =================================================================================================
// `ForcedViaInserter.check`
// =================================================================================================

/// `P6T10Probe.checkVia`'s six probe spots, in order.
fn check_spots() -> Vec<(&'static str, Point)> {
    vec![
        ("onNet1", Point::new(0, 200)),
        ("onNet2", Point::new(-800, 900)),
        ("onSmdPin", Point::new(-500, 0)),
        ("onThruPin", Point::new(500, 0)),
        ("freeSpace", Point::new(2000, 2000)),
        ("offBoard", Point::new(9990, 9990)),
    ]
}

/// Probe mode `check`: every row of the `holeClearance x spot x attachSmd x nets x pen` grid in
/// both angle regimes, including the `shoveFailingLayer` each refusal records (`:180`, `:205`,
/// `:234`).
#[test]
fn forced_via_inserter_check_agrees_with_the_jvm_on_every_probe_row() {
    let spots = check_spots();
    let mut board = probe_board(AngleRestriction::None);
    let mut through = PadstackId(board.library.padstacks.get_by_name("thru").unwrap().no);
    let mut checked = 0usize;
    for row in section("check") {
        if let Some(rest) = row.strip_prefix("mode=check angle=") {
            board = probe_board(match rest {
                "NONE" => AngleRestriction::None,
                "NINETY_DEGREE" => AngleRestriction::NinetyDegree,
                other => panic!("unknown angle `{other}`"),
            });
            through = PadstackId(board.library.padstacks.get_by_name("thru").unwrap().no);
            continue;
        }
        let trimmed = row.trim_start();
        if !trimmed.starts_with("hc=") {
            continue;
        }
        let hole_clearance: i32 = trimmed
            .strip_prefix("hc=")
            .unwrap()
            .split(' ')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        board.rules.set_hole_clearance(hole_clearance);
        let label = field(row, "spot");
        let (_, location) = spots.iter().find(|(n, _)| *n == label).unwrap();
        let attach_smd: bool = field(row, "attachSmd").parse().unwrap();
        let via_info = ViaInfo::new(if attach_smd { "va" } else { "v" }, through, 1, attach_smd);
        let pen_text = bracketed_field(row, "pen");
        let pen: Option<Vec<i32>> = if pen_text == "null" {
            None
        } else {
            Some(nets(pen_text))
        };
        board.set_shove_failing_layer(-1);
        let ok = ForcedViaInserter::check(
            &mut board,
            &via_info,
            location,
            &nets(field(row, "nets")),
            20,
            5,
            pen.as_deref(),
            1,
        );
        let expected_ok: bool = answer(row).split(' ').next().unwrap().parse().unwrap();
        let expected_layer: i32 = field(row, "failingLayer").parse().unwrap();
        assert_eq!(ok, expected_ok, "probe row `{row}`");
        assert_eq!(
            board.get_shove_failing_layer(),
            expected_layer,
            "probe row `{row}`"
        );
        checked += 1;
    }
    assert_eq!(
        checked, 384,
        "2 hc x 6 spots x 2 infos x 2 nets x 4 pens x 2 regimes"
    );
}

/// The `check` answers must agree with what `checkLayer` says for the same geometry wherever
/// both are asked the same question: a via that fits, and one that does not.
#[test]
fn check_agrees_with_check_layer_where_the_via_fits_and_where_it_does_not() {
    let mut board = probe_board(AngleRestriction::None);
    let through = PadstackId(board.library.padstacks.get_by_name("thru").unwrap().no);
    let via_info = ViaInfo::new("v", through, 1, false);
    let room = TileShape::Box(IntBox::from_coords(-3000, -3000, 3000, 3000));

    // Free space: both answer yes.
    assert!(ForcedViaInserter::check(
        &mut board,
        &via_info,
        &Point::new(2000, 2000),
        &[3],
        20,
        5,
        None,
        1
    ));
    assert_eq!(
        ForcedViaInserter::check_layer(
            &mut board,
            70.0,
            1,
            false,
            &room,
            &Point::new(2000, 2000),
            0,
            &[3],
            20,
            5,
            0,
            1
        ),
        CheckDrillResult::Drillable
    );

    // On the foreign through pin: both refuse.
    assert!(!ForcedViaInserter::check(
        &mut board,
        &via_info,
        &Point::new(500, 0),
        &[3],
        20,
        5,
        None,
        1
    ));
    assert_eq!(board.get_shove_failing_layer(), 0);
    assert_eq!(
        ForcedViaInserter::check_layer(
            &mut board,
            70.0,
            1,
            false,
            &room,
            &Point::new(500, 0),
            0,
            &[3],
            20,
            5,
            0,
            1
        ),
        CheckDrillResult::NotDrillable
    );
}

// =================================================================================================
// Task 10b — the via-insertion chain: `ForcedPadRouter::forced_pad`, `TraceShover::insert`,
// `DrillItemMover::{insert, shove_vias}` and `ForcedViaInserter::insert`
// =================================================================================================
//
// # Where these numbers come from
//
// `scripts/differential/java/probes/P6T10bProbe.java`, committed with its stdout as
// `tests/data/p6t10b-via-insert.txt`. Every row here **mutates the board**, so the probe rebuilds
// its board per row and prints the whole item list in `getItems()` order (descending id, quirk
// #63) plus `communication.idGenerator.maxGeneratedId()` — which is what pins controller ruling
// AA's "exact board-state parity": item ids, split-trace polylines, via positions and padstacks,
// and the item order itself.

const TRANSCRIPT_10B: &str = include_str!("data/p6t10b-via-insert.txt");

/// One probe row of a mutating mode: the `  …` line, the `    maxId=… items=…` line under it and
/// the `    item …` lines under that.
struct ProbeCase {
    row: &'static str,
    max_id: u32,
    items: Vec<&'static str>,
}

/// The `######## <mode>` section of [`TRANSCRIPT_10B`], parsed into [`ProbeCase`]s.
fn cases(mode: &str) -> Vec<ProbeCase> {
    let header = format!("######## {mode}");
    let mut out: Vec<ProbeCase> = Vec::new();
    let mut inside = false;
    for line in TRANSCRIPT_10B.lines() {
        if line.starts_with("######## ") {
            inside = line == header;
            continue;
        }
        if !inside || line.starts_with('#') || line.starts_with("mode=") {
            continue;
        }
        if let Some(rest) = line.strip_prefix("    maxId=") {
            let max_id = rest.split(' ').next().unwrap()["".len()..].parse().unwrap();
            out.last_mut()
                .expect("a maxId line always follows a probe row")
                .max_id = max_id;
        } else if line.starts_with("    item ") {
            out.last_mut()
                .expect("an item line always follows a probe row")
                .items
                .push(line);
        } else {
            out.push(ProbeCase {
                row: line,
                max_id: 0,
                items: Vec::new(),
            });
        }
    }
    assert!(!out.is_empty(), "transcript section `{mode}` is empty");
    out
}

/// `P6T10bProbe.pt` — `p.toFloat().round()`, rendered `(x,y)`.
fn dump_point(point: &Point) -> String {
    let rounded = point.to_float().round();
    format!("({},{})", rounded.x, rounded.y)
}

/// `P6T10bProbe.nets` — `java.util.Arrays.toString` with the spaces removed.
fn dump_nets(net_nos: &[i32]) -> String {
    let inner: Vec<String> = net_nos.iter().map(i32::to_string).collect();
    format!("[{}]", inner.join(","))
}

/// `P6T10bProbe.dump()` — one line per item, in `Board::get_items()` order (descending id).
fn dump_board(board: &Board) -> Vec<String> {
    let ctx = board.ctx();
    let mut out = Vec::new();
    for item in board.get_items() {
        let type_name = match item {
            Item::Trace(_) => "PolylineTrace",
            Item::Via(_) => "Via",
            Item::Pin(_) => "Pin",
            Item::ObstacleArea(_) => "ObstacleArea",
            Item::ConductionArea(_) => "ConductionArea",
            Item::ViaObstacleArea(_) => "ViaObstacleArea",
            Item::ComponentObstacleArea(_) => "ComponentObstacleArea",
            Item::ComponentOutline(_) => "ComponentOutline",
            Item::BoardOutline(_) => "BoardOutline",
        };
        let mut line = format!(
            "    item id={} type={} nets={} cl={}",
            item.id().0,
            type_name,
            dump_nets(item.net_nos()),
            item.clearance_class()
        );
        match item {
            Item::Trace(trace) => {
                let corners: Vec<String> = (0..trace.corner_count())
                    .map(|i| {
                        dump_point(
                            &trace
                                .polyline()
                                .corner(i)
                                .expect("a corner index below cornerCount"),
                        )
                    })
                    .collect();
                line.push_str(&format!(
                    " layer={} hw={} corners=[{}]",
                    trace.get_layer(),
                    trace.get_half_width(),
                    corners.join(",")
                ));
            }
            Item::Via(via) => {
                let padstack = board
                    .library
                    .padstacks
                    .get(via.get_padstack_id())
                    .expect("a via's padstack is in the library");
                line.push_str(&format!(
                    " padstack={} center={} attach={}",
                    padstack.name,
                    dump_point(&via.get_center()),
                    via.attach_allowed
                ));
            }
            Item::Pin(pin) => {
                let padstack = pin.get_padstack(&ctx).expect("a pin's padstack");
                line.push_str(&format!(
                    " padstack={} center={}",
                    padstack.name,
                    dump_point(&pin.get_center(&ctx))
                ));
            }
            _ => {}
        }
        out.push(line);
    }
    out
}

fn max_generated_id(board: &Board) -> u32 {
    board.communication.id_gen.max_generated_id().0
}

/// `String.hashCode()` (JLS: `s[0]*31^(n-1) + …`, `int` arithmetic, so wrapping) of the probe's
/// `maxId|items|dump` string — mode `rand`'s compact whole-board fingerprint.
fn java_string_hash(text: &str) -> i32 {
    let mut hash: i32 = 0;
    for c in text.chars() {
        hash = hash.wrapping_mul(31).wrapping_add(c as i32);
    }
    hash
}

fn board_fingerprint(board: &Board) -> i32 {
    let dump = dump_board(board);
    let mut text = format!("{}|{}|", max_generated_id(board), dump.len());
    for line in &dump {
        text.push_str(line);
        text.push('\n');
    }
    java_string_hash(&text)
}

/// Assert the whole board state against one probe row.
fn assert_board_matches(board: &Board, case: &ProbeCase) {
    assert_eq!(
        max_generated_id(board),
        case.max_id,
        "maxId after probe row `{}`",
        case.row
    );
    let actual = dump_board(board);
    assert_eq!(
        actual.len(),
        case.items.len(),
        "item count after probe row `{}`\n  rust: {:#?}\n  java: {:#?}",
        case.row,
        actual,
        case.items
    );
    for (got, want) in actual.iter().zip(case.items.iter()) {
        assert_eq!(got, want, "board state after probe row `{}`", case.row);
    }
}

/// [`assert_board_matches`] as a predicate rather than an assertion, for the one probe axis a
/// Plan 9 fix has moved off the jar — see `trace_shover_insert_agrees_with_the_jvm_on_every_probe_row`.
fn board_matches(board: &Board, case: &ProbeCase) -> bool {
    max_generated_id(board) == case.max_id && dump_board(board) == case.items
}

/// `P6T10bProbe.buildWithVia` — the probe board plus a free, unfixed net-3 via.
fn probe_board_with_via(angle: AngleRestriction, via_center: Point) -> (Board, ItemId) {
    let mut board = probe_board(angle);
    let through = PadstackId(board.library.padstacks.get_by_name("thru").unwrap().no);
    let via = board
        .insert_via_checked(
            through,
            via_center,
            vec![3],
            1,
            FixedState::Unfixed,
            false,
            &|| false,
        )
        .expect("the probe board has nothing to split");
    (board, via)
}

/// `P6T10bProbe.padShape` — a box in the 90-degree regime, an octagon otherwise.
fn probe_pad_shape(centre: IntPoint, radius: i32, is_90: bool) -> TileShape {
    if is_90 {
        TileShape::Box(IntBox::from_coords(
            centre.x - radius,
            centre.y - radius,
            centre.x + radius,
            centre.y + radius,
        ))
    } else {
        TileShape::Octagon(IntOctagon::new(
            centre.x - radius,
            centre.y - radius,
            centre.x + radius,
            centre.y + radius,
            centre.x - centre.y - 2 * radius,
            centre.x - centre.y + 2 * radius,
            centre.x + centre.y - 2 * radius,
            centre.x + centre.y + 2 * radius,
        ))
    }
}

/// `P6T10bProbe.spots()`.
fn probe_spots() -> [(&'static str, IntPoint); 5] {
    [
        ("onNet1Trace", IntPoint::new(0, 200)),
        ("onNet2Trace", IntPoint::new(-800, 900)),
        ("onSmdPin", IntPoint::new(-500, 0)),
        ("freeSpace", IntPoint::new(2000, 2000)),
        ("offBoard", IntPoint::new(9990, 9990)),
    ]
}

fn never_stop() -> impl Fn() -> bool {
    || false
}

/// Mode `pad`: `ForcedPadRouter.forcedPad` over 5 spots x 2 radii x 2 net arrays x
/// `copperSharingAllowed` x 2 recursion depths x `changedArea` on/off, in both angle regimes,
/// plus the two early arms — **320 grid rows and 2 extra rows**, each with its whole board.
#[test]
fn forced_pad_agrees_with_the_jvm_on_every_probe_row() {
    let stop = never_stop();
    let mut probe_rows = cases("pad").into_iter();
    let mut checked = 0usize;
    for angle in [AngleRestriction::None, AngleRestriction::NinetyDegree] {
        let is_90 = angle == AngleRestriction::NinetyDegree;
        for (label, centre) in probe_spots() {
            for radius in [60, 250] {
                for net_arr in [vec![1], vec![3]] {
                    for copper_sharing in [false, true] {
                        for max_recursion_depth in [0, 20] {
                            for with_changed_area in [false, true] {
                                let case = probe_rows.next().expect("a probe row per grid point");
                                let mut board = probe_board(angle);
                                if with_changed_area {
                                    board.start_marking_changed_area();
                                }
                                board.set_shove_failing_obstacle(None);
                                let shape = probe_pad_shape(centre, radius, is_90);
                                let from_side =
                                    ShapeEntrySide::from_point(&Point::Int(centre), &shape);
                                let ok = ForcedPadRouter::forced_pad(
                                    &mut board,
                                    &shape,
                                    &from_side,
                                    0,
                                    &net_arr,
                                    1,
                                    copper_sharing,
                                    None,
                                    max_recursion_depth,
                                    5,
                                    &stop,
                                )
                                .expect("no stop check trips here");
                                let expected: bool =
                                    answer(case.row).split(' ').next().unwrap().parse().unwrap();
                                assert_eq!(
                                    field(case.row, "spot"),
                                    label,
                                    "grid and transcript are out of step"
                                );
                                assert_eq!(ok, expected, "probe row `{}`", case.row);
                                assert_eq!(
                                    board
                                        .get_shove_failing_obstacle()
                                        .map_or("null".to_string(), |id| id.0.to_string()),
                                    field(case.row, "failing"),
                                    "shoveFailingObstacle after `{}`",
                                    case.row
                                );
                                assert_board_matches(&board, &case);
                                checked += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(
        checked, 320,
        "2 regimes x 5 spots x 2 radii x 2 nets x 2 share x 2 depths x 2 changedArea"
    );

    // `:355-358` — an empty pad shape answers `true` and leaves the board alone.
    let empty_case = probe_rows.next().expect("the emptyShape row");
    let mut board = probe_board(AngleRestriction::None);
    board.set_shove_failing_obstacle(None);
    let empty = TileShape::Box(IntBox::from_coords(100, 100, 0, 0));
    assert!(
        ForcedPadRouter::forced_pad(
            &mut board,
            &empty,
            &ShapeEntrySide::NOT_CALCULATED,
            0,
            &[1],
            1,
            false,
            None,
            20,
            5,
            &stop
        )
        .unwrap()
    );
    assert_board_matches(&board, &empty_case);

    // `:359-362` — a pad outside the bounding box answers `false` and records the outline, which
    // on a board built without one is `null`.
    let outside_case = probe_rows.next().expect("the outsideBoundingBox row");
    let mut board = probe_board(AngleRestriction::None);
    board.set_shove_failing_obstacle(None);
    let huge = TileShape::Box(IntBox::from_coords(-20_000, -20_000, 20_000, 20_000));
    assert!(
        !ForcedPadRouter::forced_pad(
            &mut board,
            &huge,
            &ShapeEntrySide::NOT_CALCULATED,
            0,
            &[1],
            1,
            false,
            None,
            20,
            5,
            &stop
        )
        .unwrap()
    );
    assert_board_matches(&board, &outside_case);
    assert!(probe_rows.next().is_none(), "every `pad` row was consumed");
}

/// The number of the 320 `trace` grid rows whose board **moved off the jar** when T11 fixed
/// quirk #177. Every one of them is a `changedArea=false` row, and every one of them moved to the
/// answer its `changedArea=true` twin already gave — the test below asserts both, so this literal
/// is a count, not a claim on its own.
/// Measured: **4** — `spot=onNet1Trace r=60 nets=[3] maxRec=20`, at `spring=0` and `spring=3`, in
/// both angle regimes. That is the row the register's JVM-pinned literal already named (three
/// traces, ids 6/7/8, where a marked board leaves one, id 8 with all eight corners), and it is the
/// only grid point at which the un-normalized pieces are distinguishable: everywhere else the
/// shove either fails, or leaves a single piece that normalisation would not have changed.
const MOVED_BY_177: usize = 4;

/// Mode `trace`: `TraceShover.insert` over the same grid plus the spring-over budget — **320 grid
/// rows and 2 extra rows**.
///
/// # PORT-REGRESSION PIN on the `changedArea=false` axis — fixed: T11 (#177)
///
/// This transcript is the jar's own stdout, and 316 of its 320 grid rows still match it exactly.
/// The rest are the rows quirk #177 moved: `TraceShover.insert:572` dereferenced a null
/// `changedArea` and its own `catch` swallowed the `NullPointerException`, so an unmarked board
/// kept the substitute pieces un-normalized. `ForcedPadRouter.forcedPad:439-444` guards the
/// identical call in the identical loop, so the two mutating halves of the shove disagreed about
/// the same board state, and the register's own remedy was to compute `optArea` the way the
/// sibling already does.
///
/// Under ruling CC (the M1 accept wave's precedent, ruling BV) a jar row a Plan 9 fix moves is
/// **re-cut as a port-regression pin with provenance** rather than left red. The re-cut here is
/// not a literal but the invariant that replaces it, which is stronger than a transcript row and
/// cannot rot into agreement with a future bug: for every one of the moved rows, the port's
/// unmarked board must equal the board its **marked** twin produces. That is the sibling being
/// the specification, asserted 320 rows wide instead of on the one hand-picked row
/// `an_unmarked_changed_area_still_normalises` uses.
///
/// The jar comparison is kept for every other row and every other axis, so a real parity
/// regression anywhere else in the grid still fails here.
#[test]
fn trace_shover_insert_agrees_with_the_jvm_on_every_probe_row() {
    let stop = never_stop();
    let mut probe_rows = cases("trace").into_iter();
    let mut checked = 0usize;
    let mut moved: Vec<String> = Vec::new();
    let mut unmarked_boards: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut marked_boards: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for angle in [AngleRestriction::None, AngleRestriction::NinetyDegree] {
        let is_90 = angle == AngleRestriction::NinetyDegree;
        for (label, centre) in probe_spots() {
            for radius in [60, 250] {
                for net_arr in [vec![1], vec![3]] {
                    for max_recursion_depth in [0, 20] {
                        for spring_over in [0, 3] {
                            // Everything but the `changedArea` axis: the key that pairs a
                            // `changedArea=false` row with its `changedArea=true` twin.
                            let key = format!(
                                "{is_90}|{label}|{radius}|{net_arr:?}|{max_recursion_depth}|{spring_over}"
                            );
                            for with_changed_area in [false, true] {
                                let case = probe_rows.next().expect("a probe row per grid point");
                                let mut board = probe_board(angle);
                                if with_changed_area {
                                    board.start_marking_changed_area();
                                }
                                board.set_shove_failing_obstacle(None);
                                let shape = probe_pad_shape(centre, radius, is_90);
                                let from_side =
                                    ShapeEntrySide::from_point(&Point::Int(centre), &shape);
                                let ok = TraceShover::insert(
                                    &mut board,
                                    &shape,
                                    Some(&from_side),
                                    0,
                                    &net_arr,
                                    1,
                                    None,
                                    max_recursion_depth,
                                    5,
                                    spring_over,
                                    &stop,
                                )
                                .expect("no stop check trips here");
                                let expected: bool =
                                    answer(case.row).split(' ').next().unwrap().parse().unwrap();
                                assert_eq!(field(case.row, "spot"), label);
                                assert_eq!(ok, expected, "probe row `{}`", case.row);
                                assert_eq!(
                                    board
                                        .get_shove_failing_obstacle()
                                        .map_or("null".to_string(), |id| id.0.to_string()),
                                    field(case.row, "failing"),
                                    "shoveFailingObstacle after `{}`",
                                    case.row
                                );
                                if with_changed_area || board_matches(&board, &case) {
                                    assert_board_matches(&board, &case);
                                } else {
                                    moved.push(case.row.to_string());
                                    unmarked_boards.insert(key.clone(), dump_board(&board));
                                }
                                if with_changed_area {
                                    marked_boards.insert(key.clone(), dump_board(&board));
                                }
                                checked += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(checked, 320);

    // fixed: T11 (#177). Every row that has moved off the jar is a `changedArea=false` row, and it
    // moved because `TraceShover.insert:572` no longer dereferences a null `changedArea`. See the
    // doc comment above for why these rows are a port-regression pin rather than a parity failure.
    assert!(
        moved.iter().all(|row| row.contains("changedArea=false")),
        "a row moved off the jar on an axis #177 does not touch:\n{}",
        moved.join("\n")
    );
    assert_eq!(
        moved.len(),
        MOVED_BY_177,
        "the number of probe rows #177 moves changed:\n{}",
        moved.join("\n")
    );
    // And they moved *to* the marked board's answer, which is the whole content of the fix: the
    // sibling `ForcedPadRouter.forcedPad` is the specification, so the shove's result must not
    // depend on whether the caller happened to be marking the changed area.
    for (key, unmarked) in &unmarked_boards {
        assert_eq!(
            unmarked,
            marked_boards
                .get(key)
                .expect("every unmarked row has its marked twin"),
            "fixed: T11 (#177) — the two halves of the shove still disagree at `{key}`"
        );
    }

    let empty_case = probe_rows.next().expect("the emptyShape row");
    let mut board = probe_board(AngleRestriction::None);
    board.set_shove_failing_obstacle(None);
    assert!(
        TraceShover::insert(
            &mut board,
            &TileShape::Box(IntBox::from_coords(100, 100, 0, 0)),
            Some(&ShapeEntrySide::NOT_CALCULATED),
            0,
            &[1],
            1,
            None,
            20,
            5,
            0,
            &stop
        )
        .unwrap()
    );
    assert_board_matches(&board, &empty_case);

    let outside_case = probe_rows.next().expect("the outsideBoundingBox row");
    let mut board = probe_board(AngleRestriction::None);
    board.set_shove_failing_obstacle(None);
    assert!(
        !TraceShover::insert(
            &mut board,
            &TileShape::Box(IntBox::from_coords(-20_000, -20_000, 20_000, 20_000)),
            Some(&ShapeEntrySide::NOT_CALCULATED),
            0,
            &[1],
            1,
            None,
            20,
            5,
            0,
            &stop
        )
        .unwrap()
    );
    assert_board_matches(&board, &outside_case);
    assert!(
        probe_rows.next().is_none(),
        "every `trace` row was consumed"
    );
}

/// **fixed: T11 (#177).** Java bug: with `board.changedArea == null`,
/// `TraceShover.insert:572`'s `board.changedArea.getArea(layer)` throws a `NullPointerException`
/// that the `catch (Exception)` one line down swallows — so the substitute pieces were inserted
/// **un-normalized**: three separate traces (ids 6, 7, 8), where the identical call on a board
/// that *is* marking its changed area normalizes them into one (id 8, all eight corners).
/// `ForcedPadRouter.forcedPad:439-444` guards the same null in the same loop over the same
/// pieces, so the two mutating halves of the shove disagreed about the same board state.
///
/// This test was `trace_shover_insert_swallows_the_null_changed_area_npe_and_leaves_the_pieces_
/// unnormalized` and pinned that three-trace board as the unmarked answer.
///
/// The sibling is the specification, so the binding assertion is now an **equality between the
/// two halves** rather than two literals: the board after the shove does not depend on whether
/// `changed_area` was marked. It holds exactly — the unmarked board is not merely "also one
/// trace", it is the *same* board, item for item and corner for corner, and `maxGeneratedId` is 8
/// on both sides as it always was. The marked half's literal is kept below so the pair still says
/// what that board is, rather than only that the two agree about something.
#[test]
fn an_unmarked_changed_area_still_normalises() {
    let stop = never_stop();
    let shape = probe_pad_shape(IntPoint::new(0, 200), 60, false);
    let from_side = ShapeEntrySide::from_point(&Point::new(0, 200), &shape);

    // changedArea == null. Probe row `spot=onNet1Trace r=60 nets=[3] maxRec=20 spring=0
    // changedArea=false`, whose JVM answer is the three un-normalized pieces.
    let mut board = probe_board(AngleRestriction::None);
    assert!(board.changed_area.is_none());
    assert!(
        TraceShover::insert(
            &mut board,
            &shape,
            Some(&from_side),
            0,
            &[3],
            1,
            None,
            20,
            5,
            0,
            &stop
        )
        .unwrap()
    );
    assert_eq!(max_generated_id(&board), 8);
    let unmarked = dump_board(&board);
    // One trace, not three. The pinned answer was
    //   id=8 corners=[(269,400),(162,507),(-162,507),(-307,362),(-307,38),(-269,0)]
    //   id=7 corners=[(269,400),(500,400)]
    //   id=6 corners=[(-500,0),(-269,0)]
    // — the same copper, left in the three pieces `nextSubstituteTracePiece` built.
    assert_eq!(
        unmarked,
        vec![
            "    item id=8 type=PolylineTrace nets=[1] cl=1 layer=0 hw=30 corners=[(500,400),(269,400),(162,507),(-162,507),(-307,362),(-307,38),(-269,0),(-500,0)]",
            "    item id=5 type=PolylineTrace nets=[2] cl=2 layer=0 hw=40 corners=[(-800,300),(-800,900),(300,900)]",
            "    item id=3 type=Pin nets=[1] cl=1 padstack=thru center=(500,0)",
            "    item id=2 type=Pin nets=[1] cl=1 padstack=smd center=(-500,0)",
            "    item id=1 type=BoardOutline nets=[] cl=0",
        ]
    );

    // changedArea != null: the half that always worked, and the specification for the half above.
    let mut board = probe_board(AngleRestriction::None);
    board.start_marking_changed_area();
    assert!(
        TraceShover::insert(
            &mut board,
            &shape,
            Some(&from_side),
            0,
            &[3],
            1,
            None,
            20,
            5,
            0,
            &stop
        )
        .unwrap()
    );
    assert_eq!(max_generated_id(&board), 8);
    let marked = dump_board(&board);
    assert_eq!(
        marked,
        vec![
            "    item id=8 type=PolylineTrace nets=[1] cl=1 layer=0 hw=30 corners=[(500,400),(269,400),(162,507),(-162,507),(-307,362),(-307,38),(-269,0),(-500,0)]",
            "    item id=5 type=PolylineTrace nets=[2] cl=2 layer=0 hw=40 corners=[(-800,300),(-800,900),(300,900)]",
            "    item id=3 type=Pin nets=[1] cl=1 padstack=thru center=(500,0)",
            "    item id=2 type=Pin nets=[1] cl=1 padstack=smd center=(-500,0)",
            "    item id=1 type=BoardOutline nets=[] cl=0",
        ]
    );

    // The invariant, stated on its own: the shove does not depend on whether the caller happened
    // to be marking the changed area. This is the assertion that would catch a future divergence
    // between `TraceShover::insert` and `ForcedPadRouter::forced_pad` even if both literals above
    // were updated together.
    assert_eq!(
        unmarked, marked,
        "fixed: T11 (#177) — the shove's two mutating halves agree about the same board state"
    );
}

/// Mode `shove`: `DrillItemMover.shoveVias` (144 rows) then `DrillItemMover.insert` (72 rows) and
/// the shove-fixed row, in both angle regimes.
#[test]
fn drill_item_mover_shove_vias_and_insert_agree_with_the_jvm() {
    let stop = never_stop();
    let mut probe_rows = cases("shove").into_iter();
    let via_spots = [
        ("freeSpace", IntPoint::new(2000, 2000)),
        ("nearNet1Trace", IntPoint::new(0, 250)),
        ("nearNet2Trace", IntPoint::new(-500, 900)),
    ];
    let mut shove_rows = 0usize;
    let mut insert_rows = 0usize;
    for angle in [AngleRestriction::None, AngleRestriction::NinetyDegree] {
        let is_90 = angle == AngleRestriction::NinetyDegree;
        for (label, via_centre) in via_spots {
            for radius in [80, 300] {
                for net_arr in [vec![1], vec![3]] {
                    for max_via_recursion_depth in [0, 1, 5] {
                        for copper_sharing in [false, true] {
                            let case = probe_rows.next().expect("a shoveVias row");
                            let (mut board, _via) =
                                probe_board_with_via(angle, Point::Int(via_centre));
                            board.start_marking_changed_area();
                            board.set_shove_failing_obstacle(None);
                            let shape = probe_pad_shape(via_centre, radius, is_90);
                            let from_side =
                                ShapeEntrySide::from_point(&Point::Int(via_centre), &shape);
                            let ok = DrillItemMover::shove_vias(
                                &mut board,
                                &shape,
                                &from_side,
                                0,
                                &net_arr,
                                1,
                                None,
                                20,
                                max_via_recursion_depth,
                                copper_sharing,
                                &stop,
                            )
                            .expect("no stop check trips here");
                            assert_eq!(field(case.row, "via"), label);
                            let expected: bool =
                                answer(case.row).split(' ').next().unwrap().parse().unwrap();
                            assert_eq!(ok, expected, "probe row `{}`", case.row);
                            assert_board_matches(&board, &case);
                            shove_rows += 1;
                        }
                    }
                }
            }
        }
        for (label, via_centre) in via_spots {
            for delta in [
                (0, 0),
                (300, 0),
                (-300, 0),
                (0, 700),
                (4000, 4000),
                (20_000, 0),
            ] {
                for max_recursion_depth in [0, 20] {
                    let case = probe_rows.next().expect("an insert row");
                    let (mut board, via) = probe_board_with_via(angle, Point::Int(via_centre));
                    board.start_marking_changed_area();
                    board.set_shove_failing_obstacle(None);
                    let ok = DrillItemMover::insert(
                        &mut board,
                        via,
                        &Vector::new(delta.0, delta.1),
                        max_recursion_depth,
                        5,
                        None,
                        &stop,
                    )
                    .expect("no stop check trips here");
                    assert_eq!(field(case.row, "via"), label);
                    let expected: bool =
                        answer(case.row).split(' ').next().unwrap().parse().unwrap();
                    assert_eq!(ok, expected, "probe row `{}`", case.row);
                    assert_board_matches(&board, &case);
                    insert_rows += 1;
                }
            }
        }
        // `:117-119` — a shove-fixed drill item refuses without touching the board.
        let case = probe_rows.next().expect("the shoveFixed row");
        let (mut board, unfixed) = probe_board_with_via(angle, Point::new(2000, 2000));
        board.set_shove_failing_obstacle(None);
        let through = PadstackId(board.library.padstacks.get_by_name("thru").unwrap().no);
        let user_fixed = board
            .insert_via_checked(
                through,
                Point::new(3000, 3000),
                vec![3],
                1,
                FixedState::UserFixed,
                false,
                &stop,
            )
            .unwrap();
        assert!(
            !DrillItemMover::insert(
                &mut board,
                user_fixed,
                &Vector::new(300, 0),
                20,
                5,
                None,
                &stop
            )
            .unwrap()
        );
        assert!(
            DrillItemMover::insert(&mut board, unfixed, &Vector::new(0, 0), 20, 5, None, &stop)
                .unwrap()
        );
        assert_board_matches(&board, &case);
    }
    assert_eq!(
        shove_rows, 144,
        "2 regimes x 3 vias x 2 radii x 2 nets x 3 depths x 2 share"
    );
    assert_eq!(insert_rows, 72, "2 regimes x 3 vias x 6 deltas x 2 depths");
    assert!(
        probe_rows.next().is_none(),
        "every `shove` row was consumed"
    );
}

/// Mode `via`: `ForcedViaInserter.insert` over 2 hole clearances x 7 spots x `attachSmd` x 2 net
/// arrays x 3 `tracePenHalfwidthArr`s, in both angle regimes — **336 rows**, each with its whole
/// board, the `shoveFailingLayer` it left and the `shoveFailingObstacle`.
#[test]
fn forced_via_inserter_insert_agrees_with_the_jvm_on_every_probe_row() {
    let stop = never_stop();
    let mut probe_rows = cases("via").into_iter();
    let spots = [
        ("onNet1Trace", Point::new(0, 200)),
        ("crossesNet1Trace", Point::new(0, 400)),
        ("onNet2Trace", Point::new(-800, 900)),
        ("onSmdPin", Point::new(-500, 0)),
        ("onThruPin", Point::new(500, 0)),
        ("freeSpace", Point::new(2000, 2000)),
        ("offBoard", Point::new(9990, 9990)),
    ];
    let mut checked = 0usize;
    for angle in [AngleRestriction::None, AngleRestriction::NinetyDegree] {
        for hole_clearance in [0, 300] {
            for (label, location) in &spots {
                for attach_smd in [false, true] {
                    for net_arr in [vec![1], vec![3]] {
                        for pen in [[0, 0], [30, 30], [400, 400]] {
                            let case = probe_rows.next().expect("a probe row per grid point");
                            let mut board = probe_board(angle);
                            board.rules.set_hole_clearance(hole_clearance);
                            board.start_marking_changed_area();
                            board.set_shove_failing_layer(-1);
                            board.set_shove_failing_obstacle(None);
                            let through =
                                PadstackId(board.library.padstacks.get_by_name("thru").unwrap().no);
                            let via_info = ViaInfo::new("v", through, 1, attach_smd);
                            let ok = ForcedViaInserter::insert(
                                &mut board, &via_info, location, &net_arr, 1, &pen, 20, 5, &stop,
                            )
                            .expect("no stop check trips here");
                            assert_eq!(field(case.row, "spot"), *label);
                            let expected: bool =
                                answer(case.row).split(' ').next().unwrap().parse().unwrap();
                            assert_eq!(ok, expected, "probe row `{}`", case.row);
                            assert_eq!(
                                board.get_shove_failing_layer().to_string(),
                                field(case.row, "failingLayer"),
                                "shoveFailingLayer after `{}`",
                                case.row
                            );
                            assert_eq!(
                                board
                                    .get_shove_failing_obstacle()
                                    .map_or("null".to_string(), |id| id.0.to_string()),
                                field(case.row, "failing"),
                                "shoveFailingObstacle after `{}`",
                                case.row
                            );
                            assert_board_matches(&board, &case);
                            checked += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(
        checked, 336,
        "2 regimes x 2 hole clearances x 7 spots x 2 attachSmd x 2 nets x 3 pens"
    );
    assert!(probe_rows.next().is_none(), "every `via` row was consumed");
}

/// The plan-3 ruling F path, asserted as literals: a via inserted at a corner of the net-1 trace
/// reaches `BasicBoard.insertVia:287-293` -> `splitTraces` -> `PolylineTrace.split` and **splits
/// the trace it crosses in two**, with the `StopCheck` never tripping.
///
/// Probe row: `hc=0 spot=crossesNet1Trace attachSmd=false nets=[1] pen=[0,0]`.
#[test]
fn insert_splits_the_traces_it_crosses() {
    let stop = never_stop();
    let mut board = probe_board(AngleRestriction::None);
    board.start_marking_changed_area();
    let through = PadstackId(board.library.padstacks.get_by_name("thru").unwrap().no);
    let via_info = ViaInfo::new("v", through, 1, false);
    assert!(
        ForcedViaInserter::insert(
            &mut board,
            &via_info,
            &Point::new(0, 400),
            &[1],
            1,
            &[0, 0],
            20,
            5,
            &stop
        )
        .unwrap()
    );
    // The via is id 6 (burnt before the split), and the split pieces are 7 and 8: the id-burn
    // order is `insertVia` -> `splitTraces` -> `split`, exactly as Java's.
    assert_eq!(max_generated_id(&board), 8);
    assert_eq!(
        dump_board(&board),
        vec![
            "    item id=8 type=PolylineTrace nets=[1] cl=1 layer=0 hw=30 corners=[(0,400),(500,400)]",
            "    item id=7 type=PolylineTrace nets=[1] cl=1 layer=0 hw=30 corners=[(-500,0),(0,0),(0,400)]",
            "    item id=6 type=Via nets=[1] cl=1 padstack=thru center=(0,400) attach=false",
            "    item id=5 type=PolylineTrace nets=[2] cl=2 layer=0 hw=40 corners=[(-800,300),(-800,900),(300,900)]",
            "    item id=3 type=Pin nets=[1] cl=1 padstack=thru center=(500,0)",
            "    item id=2 type=Pin nets=[1] cl=1 padstack=smd center=(-500,0)",
            "    item id=1 type=BoardOutline nets=[] cl=0",
        ]
    );
}

/// Plan-6 ruling 6, the other half of plan-3 ruling F: on a ladder board (quirk #76's minimal
/// repro) the walk `insertVia` -> `splitTraces` -> `PolylineTrace.split` ->
/// `Item.getConnectionItems` does not terminate in Java, so the port threads a [`StopCheck`]
/// through it. A check that trips after `n` calls must answer [`BoardError::Stopped`] — never
/// hang, and never be swallowed by the `catch` at `TraceShover.java:573` / `ForcedPadRouter`'s.
#[test]
fn insert_stops_when_the_stop_check_trips() {
    let mut board = probe_board(AngleRestriction::None);
    board.start_marking_changed_area();
    // `P6T10bProbe.buildLadder`: four rungs between two rails, all on net 3.
    for i in 0..4 {
        let x = 1000 + i * 200;
        board.insert_trace_without_cleaning(
            Polyline::from_points(&[Point::new(x, 1000), Point::new(x, 1600)]),
            0,
            30,
            vec![3],
            1,
            FixedState::Unfixed,
        );
    }
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(1000, 1000), Point::new(1600, 1000)]),
        0,
        30,
        vec![3],
        1,
        FixedState::Unfixed,
    );
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(1000, 1600), Point::new(1600, 1600)]),
        0,
        30,
        vec![3],
        1,
        FixedState::Unfixed,
    );

    let calls = std::cell::Cell::new(0u32);
    let stop = || {
        calls.set(calls.get() + 1);
        calls.get() > 3
    };
    let through = PadstackId(board.library.padstacks.get_by_name("thru").unwrap().no);
    let via_info = ViaInfo::new("v", through, 1, false);
    let outcome = ForcedViaInserter::insert(
        &mut board,
        &via_info,
        &Point::new(1200, 1000),
        &[3],
        1,
        &[0, 0],
        20,
        5,
        &stop,
    );
    assert_eq!(outcome, Err(BoardError::Stopped));
    assert!(calls.get() > 3, "the stop check was actually consulted");
}

/// The brief's `forced_pad_shoves_a_foreign_via_and_reports_the_moved_items`: a pad shape over a
/// free foreign-net via reaches `DrillItemMover.shoveVias` (`forcedPad:364`), which moves the via
/// out of the way and leaves it at the JVM's coordinates.
///
/// Probe rows: `shoveVias via=freeSpace r=80 nets=[1] maxViaRec=1 share=false` and the
/// `maxViaRec=0` row above it, where the budget is spent and the via stays put.
#[test]
fn forced_pad_shoves_a_foreign_via_and_reports_the_moved_items() {
    let stop = never_stop();
    let centre = IntPoint::new(2000, 2000);
    let shape = probe_pad_shape(centre, 80, false);
    let from_side = ShapeEntrySide::from_point(&Point::Int(centre), &shape);

    // The via budget is spent: `shoveVias:206-208` answers `true` without moving anything.
    let (mut board, via) = probe_board_with_via(AngleRestriction::None, Point::Int(centre));
    board.start_marking_changed_area();
    assert!(
        DrillItemMover::shove_vias(
            &mut board,
            &shape,
            &from_side,
            0,
            &[1],
            1,
            None,
            20,
            0,
            false,
            &stop
        )
        .unwrap()
    );
    assert!(
        board.get_item(via).is_some(),
        "the via is still on the board"
    );
    assert_eq!(
        dump_board(&board)
            .into_iter()
            .find(|line| line.contains("type=Via"))
            .unwrap(),
        "    item id=6 type=Via nets=[3] cl=1 padstack=thru center=(2000,2000) attach=false"
    );

    // One unit of via budget is enough: the via moves to (2368, 2000).
    let (mut board, _via) = probe_board_with_via(AngleRestriction::None, Point::Int(centre));
    board.start_marking_changed_area();
    assert!(
        DrillItemMover::shove_vias(
            &mut board,
            &shape,
            &from_side,
            0,
            &[1],
            1,
            None,
            20,
            1,
            false,
            &stop
        )
        .unwrap()
    );
    assert_eq!(
        dump_board(&board)
            .into_iter()
            .find(|line| line.contains("type=Via"))
            .unwrap(),
        "    item id=6 type=Via nets=[3] cl=1 padstack=thru center=(2368,2000) attach=false"
    );
}

/// The brief's `insert_on_an_unroutable_layer_returns_false_and_leaves_the_board_unchanged`: a
/// `tracePenHalfwidthArr` wide enough that the start-trace circle cannot be shoved makes
/// `ForcedViaInserter.insert` refuse at `:343-344` **before** `BasicBoard.insertVia` — so no id
/// is burnt, no via exists, and the board is byte-identical to the one it started with.
///
/// Probe row: `hc=0 spot=crossesNet1Trace attachSmd=false nets=[1] pen=[400,400]`.
#[test]
fn insert_on_an_unroutable_layer_returns_false_and_leaves_the_board_unchanged() {
    let stop = never_stop();
    let mut board = probe_board(AngleRestriction::None);
    board.start_marking_changed_area();
    let before = board.structural_hash();
    let before_dump = dump_board(&board);
    let through = PadstackId(board.library.padstacks.get_by_name("thru").unwrap().no);
    let via_info = ViaInfo::new("v", through, 1, false);
    assert!(
        !ForcedViaInserter::insert(
            &mut board,
            &via_info,
            &Point::new(0, 400),
            &[1],
            1,
            &[400, 400],
            20,
            5,
            &stop
        )
        .unwrap()
    );
    assert_eq!(board.get_shove_failing_layer(), 0);
    assert_eq!(board.get_shove_failing_obstacle(), Some(ItemId(3)));
    assert_eq!(max_generated_id(&board), 5, "no id was burnt");
    assert_eq!(dump_board(&board), before_dump);
    assert_eq!(board.structural_hash(), before);
}

/// The brief's `>= 100 random cases each, 0 diffs`: **five blocks of 120** pseudo-random rows,
/// one per method, each asserting the answer, `maxId`, the item count and a `String.hashCode` of
/// the whole board dump — so a single wrong coordinate anywhere on the board fails the row.
#[test]
fn the_five_random_blocks_agree_with_the_jvm() {
    let stop = never_stop();
    let mut rows = cases("rand").into_iter();
    for (which, _label) in [
        (0, "forcedPad"),
        (1, "traceShoverInsert"),
        (2, "shoveVias"),
        (3, "drillItemMoverInsert"),
        (4, "forcedViaInsert"),
    ] {
        let mut seed: u64 = 20_261_111u64.wrapping_add((which as u64).wrapping_mul(7919));
        let mut next = move || {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            seed >> 33
        };
        let mut next_int = |bound: u64| -> i32 { (next() % bound) as i32 };
        let mut diffs: Vec<String> = Vec::new();
        for i in 0..120 {
            let case = rows.next().expect("120 rows per block");
            let angle = if next_int(2) == 0 {
                AngleRestriction::None
            } else {
                AngleRestriction::NinetyDegree
            };
            let is_90 = angle == AngleRestriction::NinetyDegree;
            let x = next_int(4000) - 2000;
            let y = next_int(4000) - 2000;
            let radius = next_int(400) + 40;
            let net_no = next_int(3) + 1;
            let max_recursion_depth = next_int(4) * 7;
            let max_via_recursion_depth = next_int(4);
            let spring_over = next_int(3);
            let copper_sharing = next_int(2) == 0;
            let with_changed_area = next_int(2) == 0;
            // `P6T10bProbe.randomBlock`: the via is placed near the probed shape, so the
            // `shoveVias` block has something to shove; see the comment there.
            let via_x = x + next_int(700) - 350;
            let via_y = y + next_int(700) - 350;
            // `P6T10bProbe.randomBlock`'s block-2 narrowing: the `shoveVias` rows put the via
            // inside the shape, off the shape's net and with a via budget, because a row that
            // takes one of `:203-208`'s two skip arms proves nothing. The skip arms themselves
            // are the deterministic `shove` mode's job.
            let (via_x, via_y, net_no, max_via_recursion_depth) = if which == 2 {
                (
                    x + (via_x - x) / 3,
                    y + (via_y - y) / 3,
                    (net_no % 2) + 1,
                    max_via_recursion_depth + 1,
                )
            } else {
                (via_x, via_y, net_no, max_via_recursion_depth)
            };
            let centre = IntPoint::new(x, y);
            let (mut board, via) = probe_board_with_via(angle, Point::new(via_x, via_y));
            if with_changed_area {
                board.start_marking_changed_area();
            }
            board.set_shove_failing_obstacle(None);
            board.set_shove_failing_layer(-1);
            let shape = probe_pad_shape(centre, radius, is_90);
            let from_side = ShapeEntrySide::from_point(&Point::Int(centre), &shape);
            let net_arr = [net_no];
            let ok = match which {
                0 => ForcedPadRouter::forced_pad(
                    &mut board,
                    &shape,
                    &from_side,
                    0,
                    &net_arr,
                    1,
                    copper_sharing,
                    None,
                    max_recursion_depth,
                    max_via_recursion_depth,
                    &stop,
                ),
                1 => TraceShover::insert(
                    &mut board,
                    &shape,
                    Some(&from_side),
                    0,
                    &net_arr,
                    1,
                    None,
                    max_recursion_depth,
                    max_via_recursion_depth,
                    spring_over,
                    &stop,
                ),
                2 => DrillItemMover::shove_vias(
                    &mut board,
                    &shape,
                    &from_side,
                    0,
                    &net_arr,
                    1,
                    None,
                    max_recursion_depth,
                    max_via_recursion_depth,
                    copper_sharing,
                    &stop,
                ),
                3 => DrillItemMover::insert(
                    &mut board,
                    via,
                    &Vector::new(x / 8, y / 8),
                    max_recursion_depth,
                    max_via_recursion_depth,
                    None,
                    &stop,
                ),
                _ => {
                    let through =
                        PadstackId(board.library.padstacks.get_by_name("thru").unwrap().no);
                    let via_info = ViaInfo::new("v", through, 1, copper_sharing);
                    ForcedViaInserter::insert(
                        &mut board,
                        &via_info,
                        &Point::Int(centre),
                        &net_arr,
                        1,
                        &[radius / 4, radius / 4],
                        max_recursion_depth,
                        max_via_recursion_depth,
                        &stop,
                    )
                }
            }
            .expect("no stop check trips here");
            let expected: bool = answer(case.row).split(' ').next().unwrap().parse().unwrap();
            let expected_hash: i32 = field(case.row, "hash").parse().unwrap();
            let expected_max_id: u32 = field(case.row, "maxId").parse().unwrap();
            let expected_items: usize = field(case.row, "items").parse().unwrap();
            let actual_items = dump_board(&board).len();
            if ok != expected
                || max_generated_id(&board) != expected_max_id
                || actual_items != expected_items
                || board_fingerprint(&board) != expected_hash
            {
                diffs.push(format!(
                    "i={i}: rust ok={ok} maxId={} items={actual_items} hash={} | java `{}`\n{:#?}",
                    max_generated_id(&board),
                    board_fingerprint(&board),
                    case.row,
                    dump_board(&board)
                ));
            }
        }
        assert!(
            diffs.is_empty(),
            "{} diffs:\n{}",
            diffs.len(),
            diffs.join("\n")
        );
    }
    assert!(rows.next().is_none(), "every `rand` row was consumed");
}
