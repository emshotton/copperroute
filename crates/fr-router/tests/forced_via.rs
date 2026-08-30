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

use std::collections::BTreeSet;

use fr_board::ids::{ItemId, PadstackId};
use fr_board::prelude::*;
use fr_board::rules::ViaInfo;
use fr_geometry::{
    FloatPoint, IntBox, IntOctagon, IntPoint, IntVector, Line, Point, Polyline, Shape, ShapeOps,
    TileShape, Vector,
};
use fr_router::board_ext::{CheckDrillResult, DrillItemMover, ForcedPadRouter, ForcedViaInserter};

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
        let mut ignore = Vec::new();
        assert!(
            DrillItemMover::check(&mut board, free, &delta, 20, 5, Some(&mut ignore), None),
            "probe `check viaId=6 delta=(300,0) result=true`"
        );
        assert_eq!(
            ignore,
            vec![free],
            "quirk #175: `:63` appends the drill item to the caller's list"
        );
        for via in [fixed, on_pin] {
            let mut ignore = Vec::new();
            assert!(!DrillItemMover::check(
                &mut board,
                via,
                &delta,
                20,
                5,
                Some(&mut ignore),
                None
            ));
            assert!(ignore.is_empty());
        }
        for (dx, dy, expected) in [
            (-1500, -2000, false),
            (-2000, -1600, true),
            (-1500, -1900, false),
        ] {
            let mut ignore = Vec::new();
            assert_eq!(
                DrillItemMover::check(
                    &mut board,
                    free,
                    &Vector::from(IntVector::new(dx, dy)),
                    20,
                    5,
                    Some(&mut ignore),
                    None,
                ),
                expected,
                "probe `check viaId=6 delta=({dx},{dy}) result={expected}` ({angle:?})"
            );
            assert_eq!(ignore, vec![free]);
        }
        let mut ignore = Vec::new();
        assert!(
            DrillItemMover::check(&mut board, free, &delta, 20, 0, Some(&mut ignore), None),
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
