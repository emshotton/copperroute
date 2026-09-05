use fr_board::ids::ItemId;
use fr_board::items::Item;
use fr_board::prelude::*;
use fr_dsn::format::double::java_double_to_string;
use fr_geometry::{
    IntBox, IntOctagon, IntPoint, IntVector, JavaRandom, Line, Point, Polyline, Shape, TileShape,
};
use fr_router::board_ext::{PolylineTraceExt, TraceTightener};

// =================================================================================================
// The committed JVM transcript
// =================================================================================================

const TRANSCRIPT: &str = include_str!("data/p6t15a-tightener.txt");

/// The rows of one `######## <mode>` section of the transcript, trailing blanks trimmed.
fn section(mode: &str) -> Vec<&'static str> {
    let header = format!("######## {mode}");
    let mut rows = Vec::new();
    let mut inside = false;
    for line in TRANSCRIPT.lines() {
        if line.starts_with("######## ") {
            inside = line == header;
            continue;
        }
        if inside {
            rows.push(line.trim_end());
        }
    }
    assert!(!rows.is_empty(), "transcript section `{mode}` is empty");
    rows
}

// =================================================================================================
// The probe's board, rebuilt from scratch (`P6T9Probe.build`, which `P6T15aProbe` reuses)
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

fn layers() -> LayerStructure {
    LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)])
}

fn rules_with_wide_class(angle: AngleRestriction) -> BoardRules {
    let mut clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
    assert!(clearance_matrix.append_class("wide"));
    clearance_matrix.set_value_on_all_layers(2, 1, 600);
    clearance_matrix.set_value_on_all_layers(2, 2, 800);
    let mut rules = BoardRules::new(layers(), clearance_matrix);
    rules.trace_angle_restriction = angle;
    rules
}

/// `P6T9Probe.build(angleRestriction)`: two layers, a 200-unit clearance matrix with a "wide"
/// class, a two-pin component (an **SMD** pad at (-500, 0) on layer 0 only and a **through** pad
/// at (500, 0) on both layers) and two traces, one on net 1 and one on net 2.
fn probe_board(angle: AngleRestriction) -> Board {
    let mut padstacks = Padstacks::new(layers());
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
    let through_shape = Shape::Tile(TileShape::Octagon(IntOctagon::new(
        -70, -70, 70, 70, -140, 140, -140, 140,
    )));
    let through = padstacks.add(
        "thru",
        vec![Some(through_shape.clone()), Some(through_shape)],
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
        rules_with_wide_class(angle),
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

fn never() -> bool {
    false
}

/// `TraceTightener.getInstance(board, new int[0], null, minTranslateDist, null, -1, null, -1)` —
/// `P6T15aProbe.algo`.
fn algo(board: &mut Board, min_translate_dist: i32) -> TraceTightener<'static> {
    TraceTightener::get_instance(
        board,
        Vec::new(),
        None,
        min_translate_dist,
        None,
        -1,
        None,
        -1,
    )
}

// =================================================================================================
// The probe's dump format
// =================================================================================================

/// `P6T15aProbe.ln` — `(ax,ay)->(bx,by)`.
fn dump_line(line: &Line) -> String {
    format!("({},{})->({},{})", line.a.x, line.a.y, line.b.x, line.b.y)
}

/// `P6T15aProbe.pt` — an `IntPoint` corner prints exactly, any other prints its `cornerApprox`
/// through `Double.toString`.
fn dump_corner(polyline: &Polyline, no: usize) -> String {
    match polyline.corner(no).expect("no is below cornerCount") {
        Point::Int(p) => format!("({},{})", p.x, p.y),
        Point::Rational(_) => {
            let f = polyline.corner_approx(no).expect("no is below cornerCount");
            format!(
                "~({},{})",
                java_double_to_string(f.x),
                java_double_to_string(f.y)
            )
        }
    }
}

/// `P6T15aProbe.poly`.
fn dump_polyline(polyline: &Polyline) -> String {
    let lines: Vec<String> = polyline.lines().iter().map(dump_line).collect();
    let corners: Vec<String> = (0..polyline.corner_count())
        .map(|i| dump_corner(polyline, i))
        .collect();
    format!(
        "n={} lines=[{}] corners=[{}]",
        polyline.lines().len(),
        lines.join(","),
        corners.join(",")
    )
}

/// `P6T15aProbe.pointOf` — `p.toFloat().round()`, rendered `(x,y)`.
fn dump_point(point: &Point) -> String {
    let rounded = point.to_float().round();
    format!("({},{})", rounded.x, rounded.y)
}

/// `P6T15aProbe.nets` — `java.util.Arrays.toString` with the spaces removed.
fn dump_nets(net_nos: &[i32]) -> String {
    let inner: Vec<String> = net_nos.iter().map(i32::to_string).collect();
    format!("[{}]", inner.join(","))
}

fn dump_board(board: &Board) -> Vec<String> {
    let mut out = vec![format!(
        "    maxId={}",
        board.communication.id_gen.max_generated_id()
    )];
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
                line.push_str(&format!(
                    " layer={} hw={} {}",
                    trace.get_layer(),
                    trace.get_half_width(),
                    dump_polyline(trace.polyline())
                ));
            }
            Item::Pin(_) => {
                let center = board.drill_center(item.id()).expect("a pin has a centre");
                line.push_str(&format!(" center={}", dump_point(&center)));
            }
            _ => {}
        }
        out.push(line);
    }
    out
}

// =================================================================================================
// The fixed polyline table (`P6T15aProbe.table`)
// =================================================================================================

fn p(x: i32, y: i32) -> Point {
    Point::new(x, y)
}

struct Case {
    name: &'static str,
    corners: Vec<Point>,
    layer: usize,
    half_width: i32,
    nets: Vec<i32>,
    cl: usize,
}

fn table() -> Vec<Case> {
    vec![
        Case {
            name: "straight",
            corners: vec![p(-3000, -3000), p(-1000, -3000)],
            layer: 0,
            half_width: 30,
            nets: vec![3],
            cl: 1,
        },
        Case {
            name: "elbow",
            corners: vec![p(-3000, -3000), p(-3000, -1000), p(-1000, -1000)],
            layer: 0,
            half_width: 30,
            nets: vec![3],
            cl: 1,
        },
        Case {
            name: "staircase",
            corners: vec![
                p(-3000, -3000),
                p(-3000, -2500),
                p(-2500, -2500),
                p(-2500, -2000),
                p(-2000, -2000),
                p(-2000, -1500),
                p(-1500, -1500),
            ],
            layer: 0,
            half_width: 30,
            nets: vec![3],
            cl: 1,
        },
        Case {
            name: "detour",
            corners: vec![
                p(-3000, -3000),
                p(-3000, 0),
                p(-2000, 0),
                p(-2000, -3000),
                p(-1000, -3000),
            ],
            layer: 0,
            half_width: 30,
            nets: vec![3],
            cl: 1,
        },
        Case {
            name: "diagonal",
            corners: vec![
                p(-3000, -3000),
                p(-2000, -2000),
                p(-2000, -1000),
                p(-1000, 0),
            ],
            layer: 0,
            half_width: 30,
            nets: vec![3],
            cl: 1,
        },
        Case {
            name: "zerolen",
            corners: vec![
                p(-3000, -3000),
                p(-3000, -3000),
                p(-2000, -3000),
                p(-2000, -3000),
                p(-2000, -2000),
            ],
            layer: 0,
            half_width: 30,
            nets: vec![3],
            cl: 1,
        },
        Case {
            name: "wide",
            corners: vec![
                p(-3000, -3000),
                p(-3000, -2000),
                p(-2000, -2000),
                p(-2000, -1000),
                p(-1000, -1000),
            ],
            layer: 0,
            half_width: 400,
            nets: vec![3],
            cl: 2,
        },
        Case {
            name: "pastNet2",
            corners: vec![p(-1500, 400), p(-1500, 1400), p(-200, 1400), p(-200, 400)],
            layer: 0,
            half_width: 30,
            nets: vec![3],
            cl: 1,
        },
        Case {
            name: "roundPad",
            corners: vec![p(-500, -600), p(-1200, -600), p(-1200, 600), p(-500, 600)],
            layer: 0,
            half_width: 30,
            nets: vec![3],
            cl: 1,
        },
        Case {
            name: "layer1",
            corners: vec![p(200, -1500), p(200, -500), p(1200, -500), p(1200, -1500)],
            layer: 1,
            half_width: 30,
            nets: vec![3],
            cl: 1,
        },
        Case {
            name: "ownNet1",
            corners: vec![p(-400, -700), p(-400, -100), p(400, -100), p(400, -700)],
            layer: 0,
            half_width: 30,
            nets: vec![1],
            cl: 1,
        },
    ]
}

/// `P6T15aProbe.fixedTable`, replayed against this port.
fn fixed_table_rows(angle: AngleRestriction) -> Vec<String> {
    let mut board = probe_board(angle);
    let mut out = vec![format!("regime={}", java_angle_name(angle))];
    for case in table() {
        let before = Polyline::from_points(&case.corners);
        let mut a = algo(&mut board, 500);
        let after = a.pull_tight_polyline(
            &mut board,
            &before,
            case.layer,
            case.half_width,
            &case.nets,
            case.cl,
            None,
        );
        out.push(format!(
            "case={} same={}",
            case.name,
            same_polyline(&before, &after)
        ));
        out.push(format!("  before {}", dump_polyline(&before)));
        out.push(format!("  after  {}", dump_polyline(&after)));

        let mut b = algo(&mut board, 500);
        b.pull_tight_polyline(
            &mut board,
            &before,
            case.layer,
            case.half_width,
            &case.nets,
            case.cl,
            None,
        );
        let repositioned = b.reposition_lines(&mut board, &before);
        let skipped = b.skip_segments_of_length_0(&mut board, &before);
        out.push(format!(
            "  reposition same={} {}",
            repositioned.is_none(),
            dump_polyline(repositioned.as_ref().unwrap_or(&before))
        ));
        out.push(format!(
            "  skip0      same={} {}",
            skipped.is_none(),
            dump_polyline(skipped.as_ref().unwrap_or(&before))
        ));
    }

    // The clip-shape gate.
    let clip = IntOctagon::new(0, 0, 100, 100, -200, 200, -200, 200);
    for case in table() {
        let before = Polyline::from_points(&case.corners);
        let mut a = TraceTightener::get_instance(
            &mut board,
            Vec::new(),
            Some(clip),
            500,
            None,
            -1,
            None,
            -1,
        );
        let after = a.pull_tight_polyline(
            &mut board,
            &before,
            case.layer,
            case.half_width,
            &case.nets,
            case.cl,
            None,
        );
        out.push(format!(
            "clip={} same={} {}",
            case.name,
            same_polyline(&before, &after),
            dump_polyline(&after)
        ));
    }

    // `minTranslateDist` below the `Math.max(.., 100)` floor, on the `detour` case.
    for mtd in [0, 1, 100, 5000] {
        let before = Polyline::from_points(&table()[3].corners);
        let mut a = algo(&mut board, mtd);
        let after = a.pull_tight_polyline(&mut board, &before, 0, 30, &[3], 1, None);
        out.push(format!(
            "mtd={} same={} {}",
            mtd,
            same_polyline(&before, &after),
            dump_polyline(&after)
        ));
    }
    out
}

fn same_polyline(before: &Polyline, after: &Polyline) -> bool {
    before.lines() == after.lines()
}

fn java_angle_name(angle: AngleRestriction) -> &'static str {
    match angle {
        AngleRestriction::NinetyDegree => "NINETY_DEGREE",
        AngleRestriction::FortyFiveDegree => "FORTYFIVE_DEGREE",
        AngleRestriction::None => "NONE",
    }
}

/// Probe mode `inst`, first six rows of each regime:
///
/// ```text
/// regime=NINETY_DEGREE mtd=-5 class=TraceTightener90 minTranslateDist=100 …
/// regime=FORTYFIVE_DEGREE mtd=101 class=TraceTightener45 minTranslateDist=101 …
/// regime=NONE mtd=500 class=TraceTightenerAnyAngle minTranslateDist=500 …
/// ```
#[test]
fn get_instance_dispatches_on_the_board_angle_restriction() {
    for (angle, expected) in [
        (AngleRestriction::NinetyDegree, "TraceTightener90"),
        (AngleRestriction::FortyFiveDegree, "TraceTightener45"),
        (AngleRestriction::None, "TraceTightenerAnyAngle"),
    ] {
        let mut board = probe_board(angle);
        let built = algo(&mut board, 500);
        let name = match built {
            TraceTightener::Ninety(_) => "TraceTightener90",
            TraceTightener::FortyFive(_) => "TraceTightener45",
            TraceTightener::AnyAngle(_) => "TraceTightenerAnyAngle",
        };
        assert_eq!(name, expected, "regime {}", java_angle_name(angle));
    }
}

/// Probe mode `inst`: `getInstance:112` is `Math.max(minTranslateDist, 100)`, so `-5`, `0` and
/// `99` all become `100` and `101` stays `101`.
#[test]
fn get_instance_clamps_min_translate_dist_at_one_hundred() {
    let mut board = probe_board(AngleRestriction::None);
    for (input, expected) in [
        (-5, 100),
        (0, 100),
        (99, 100),
        (100, 100),
        (101, 101),
        (500, 500),
    ] {
        let built = algo(&mut board, input);
        assert_eq!(built.min_translate_dist(), expected, "mtd={input}");
        assert!(built.only_net_no_arr().is_empty());
    }
}

/// Probe mode `inst`, the `splitTracesAtKeepPoint` rows: `false` without a keep point and
/// `true` at `(0, 200)`, which lies on the net-1 trace's second segment.
#[test]
fn split_traces_at_keep_point_splits_only_with_a_keep_point() {
    for angle in [
        AngleRestriction::NinetyDegree,
        AngleRestriction::FortyFiveDegree,
        AngleRestriction::None,
    ] {
        let mut board = probe_board(angle);
        let mut without = algo(&mut board, 500);
        assert!(
            !without
                .split_traces_at_keep_point(&mut board)
                .expect("a never-tripping stop check"),
            "regime {}",
            java_angle_name(angle)
        );
        let mut with = TraceTightener::get_instance(
            &mut board,
            Vec::new(),
            None,
            500,
            None,
            -1,
            Some(Point::new(0, 200)),
            0,
        );
        assert!(
            with.split_traces_at_keep_point(&mut board)
                .expect("a never-tripping stop check"),
            "regime {}",
            java_angle_name(angle)
        );
    }
}

/// The whole of probe mode `inst`, replayed row for row.
#[test]
fn inst_mode_matches_the_jvm() {
    let expected = section("inst");
    let mut actual: Vec<String> = Vec::new();
    for angle in [
        AngleRestriction::NinetyDegree,
        AngleRestriction::FortyFiveDegree,
        AngleRestriction::None,
    ] {
        let mut board = probe_board(angle);
        for mtd in [-5, 0, 99, 100, 101, 500] {
            let built = algo(&mut board, mtd);
            let class = match built {
                TraceTightener::Ninety(_) => "TraceTightener90",
                TraceTightener::FortyFive(_) => "TraceTightener45",
                TraceTightener::AnyAngle(_) => "TraceTightenerAnyAngle",
            };
            actual.push(format!(
                "regime={} mtd={} class={} minTranslateDist={} onlyNetNoArrLen={} clip=null",
                java_angle_name(angle),
                mtd,
                class,
                built.min_translate_dist(),
                built.only_net_no_arr().len()
            ));
        }
        let mut no_keep = algo(&mut board, 500);
        actual.push(format!(
            "regime={} splitAtKeepPoint(null)={}",
            java_angle_name(angle),
            no_keep
                .split_traces_at_keep_point(&mut board)
                .expect("a never-tripping stop check")
        ));
        let mut keep = TraceTightener::get_instance(
            &mut board,
            Vec::new(),
            None,
            500,
            None,
            -1,
            Some(Point::new(0, 200)),
            0,
        );
        actual.push(format!(
            "regime={} splitAtKeepPoint(0,200)={}",
            java_angle_name(angle),
            keep.split_traces_at_keep_point(&mut board)
                .expect("a never-tripping stop check")
        ));
        actual.extend(dump_board(&board));
    }
    assert_rows("inst", &expected, &actual);
}

// =================================================================================================
// Quirk #34: `Line.equals` at `TraceTightener.repositionLine:281` — probe mode `lineeq`
// =================================================================================================

#[test]
fn reposition_line_uses_geometric_line_equality() {
    let diagonal = Line::from_coords(0, 0, 1000, 1000);
    let translated = diagonal.translate(0.0);
    assert_eq!(
        dump_line(&translated),
        "(0,0)->(1,1)",
        "Line.translate(0.0) rebuilds the line from its direction"
    );
    assert!(
        translated.equals_geometric(&diagonal),
        "Line.equals (Line.java:57-79) is the geometric test"
    );
    assert!(
        translated != diagonal,
        "the derived structural PartialEq disagrees — this is quirk #34's whole point"
    );

    // The other side of the same coin: a line that is genuinely moved is unequal both ways.
    let moved = diagonal.translate(0.4);
    assert_eq!(dump_line(&moved), "(-1,0)->(0,1)");
    assert!(!moved.equals_geometric(&diagonal));
    assert!(moved != diagonal);
}

#[test]
fn lineeq_mode_matches_the_jvm() {
    let expected = section("lineeq");
    let base = Line::from_coords(0, 0, 100, 0);
    let same_geometry = Line::from_coords(-50, 0, 250, 0);
    let opposite = Line::from_coords(100, 0, 0, 0);
    let parallel = Line::from_coords(0, 1, 100, 1);
    let degenerate = Line::from_coords(7, 9, 7, 9);
    let pairs: [(Line, Line, bool); 6] = [
        (base, base, true),
        (base, same_geometry, false),
        (base, opposite, false),
        (base, parallel, false),
        (degenerate, degenerate, true),
        (degenerate, Line::from_coords(7, 9, 7, 9), false),
    ];
    let mut actual: Vec<String> = Vec::new();
    for (x, y, same_ref) in pairs {
        let equals = if same_ref {
            true
        } else {
            x.equals_geometric(&y)
        };
        actual.push(format!(
            "pair {} vs {} equals={} structural={} sameRef={} idEqual={}",
            dump_line(&x),
            dump_line(&y),
            equals,
            x == y,
            same_ref,
            x.get_id() == y.get_id()
        ));
    }
    let diagonal = Line::from_coords(0, 0, 1000, 1000);
    for dist in [0.0, 0.2, 0.4, 0.6, 0.9, 1.0, 1.5, -0.4, -0.9] {
        let translated = diagonal.translate(dist);
        actual.push(format!(
            "translate dist={} line={} equals={} structural={} sameRef=false",
            java_double_to_string(dist),
            dump_line(&translated),
            translated.equals_geometric(&diagonal),
            translated == diagonal
        ));
    }
    // The four hand-built line arrays of `P6T15aProbe.lineEquality`, in all three regimes.
    let scripts: [(&str, Vec<Line>); 4] = [
        (
            "square",
            vec![
                Line::from_coords(5000, 5000, 5000, 5100),
                Line::from_coords(5000, 5000, 6000, 5000),
                Line::from_coords(6000, 5000, 6000, 6000),
                Line::from_coords(6000, 6000, 5000, 6000),
                Line::from_coords(5000, 6000, 5000, 5000),
            ],
        ),
        (
            "onLine",
            vec![
                Line::from_coords(-3000, -3000, -1000, -3000),
                Line::from_coords(-3000, -3100, -3000, -2900),
                Line::from_coords(-2500, -3000, -1500, -3000),
                Line::from_coords(-2000, -3100, -2000, -2900),
                Line::from_coords(-2000, -3000, -1000, -3000),
            ],
        ),
        (
            "subUnit",
            vec![
                Line::from_coords(5500, 5001, 5500, 5002),
                Line::from_coords(5500, 5001, 5600, 5001),
                Line::from_coords(5000, 5000, 7000, 5002),
                Line::from_coords(5900, 5003, 5900, 5004),
                Line::from_coords(5900, 5003, 6000, 5003),
            ],
        ),
        (
            "halfInt",
            vec![
                Line::from_coords(5000, 5000, 5002, 5001),
                Line::from_coords(5000, 5001, 5002, 5000),
                Line::from_coords(5000, 5000, 7000, 5002),
                Line::from_coords(5500, 5003, 5500, 5004),
                Line::from_coords(5500, 5003, 5600, 5003),
            ],
        ),
    ];
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    for angle in [
        AngleRestriction::NinetyDegree,
        AngleRestriction::FortyFiveDegree,
        AngleRestriction::None,
    ] {
        let mut board = probe_board(angle);
        for (name, lines) in &scripts {
            let mut a = algo(&mut board, 500);
            // The probe primes the instance off a polyline of its own; the scripts are raw
            // line arrays the normalising constructor would mangle.
            let priming = Polyline::from_points(&[p(-3000, -3000), p(-1000, -3000)]);
            a.pull_tight_polyline(&mut board, &priming, 0, 30, &[3], 1, None);
            for no in 0..=2 {
                let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    a.reposition_line(&mut board, lines, no)
                }));
                actual.push(match caught {
                    Err(_) => format!(
                        "repositionLine regime={} script={} no={} -> threw \
                         ArrayIndexOutOfBoundsException",
                        java_angle_name(angle),
                        name,
                        no
                    ),
                    Ok(None) => format!(
                        "repositionLine regime={} script={} no={} -> null",
                        java_angle_name(angle),
                        name,
                        no
                    ),
                    Ok(Some(line)) => format!(
                        "repositionLine regime={} script={} no={} -> {}",
                        java_angle_name(angle),
                        name,
                        no,
                        dump_line(&line)
                    ),
                });
            }
        }
    }
    std::panic::set_hook(hook);
    assert_rows("lineeq", &expected, &actual);
}

// =================================================================================================
// The three regime tables — probe modes `t90`, `t45`, `tany`
// =================================================================================================

#[test]
fn ninety_degree_regime_matches_the_jvm() {
    assert_rows(
        "t90",
        &section("t90"),
        &fixed_table_rows(AngleRestriction::NinetyDegree),
    );
}

#[test]
fn forty_five_degree_regime_matches_the_jvm() {
    assert_rows(
        "t45",
        &section("t45"),
        &fixed_table_rows(AngleRestriction::FortyFiveDegree),
    );
}

#[test]
fn any_angle_regime_matches_the_jvm() {
    assert_rows(
        "tany",
        &section("tany"),
        &fixed_table_rows(AngleRestriction::None),
    );
}

// =================================================================================================
// `PolylineTrace.pullTight` — probe mode `trace`
// =================================================================================================

#[test]
fn polyline_trace_pull_tight_matches_the_jvm() {
    let expected = section("trace");
    let mut actual: Vec<String> = Vec::new();
    for angle in [
        AngleRestriction::NinetyDegree,
        AngleRestriction::FortyFiveDegree,
        AngleRestriction::None,
    ] {
        // (a) `pullTight(TraceTightener)` on the two traces the board already carries.
        let mut board = probe_board(angle);
        actual.push(format!("regime={} overload=algo", java_angle_name(angle)));
        let mut a = algo(&mut board, 500);
        for id in trace_ids(&board) {
            actual.push(format!(
                "  trace id={} before {}",
                id.0,
                dump_polyline(polyline_of(&board, id).expect("a live trace"))
            ));
            let changed = <Board as PolylineTraceExt>::pull_tight_with(&mut board, id, &mut a);
            actual.push(format!(
                "  trace id={} changed={} after  {}",
                id.0,
                changed,
                dump_polyline(polyline_of(&board, id).expect("a live trace"))
            ));
        }
        actual.extend(dump_board(&board));

        // (b) a freshly inserted detour trace, which the tightener can actually shorten.
        let mut board = probe_board(angle);
        actual.push(format!(
            "regime={} overload=algo-detour",
            java_angle_name(angle)
        ));
        board.insert_trace_without_cleaning(
            detour_polyline(),
            0,
            30,
            vec![3],
            1,
            FixedState::Unfixed,
        );
        let mut b = algo(&mut board, 500);
        for id in trace_ids(&board) {
            if board.items[&id].net_nos() != [3] {
                continue;
            }
            let changed = <Board as PolylineTraceExt>::pull_tight_with(&mut board, id, &mut b);
            actual.push(format!("  trace id={} changed={}", id.0, changed));
        }
        actual.extend(dump_board(&board));

        // (c) the `(ownNetOnly, accuracy, Stoppable)` overload.
        for own_net_only in [true, false] {
            let mut board = probe_board(angle);
            actual.push(format!(
                "regime={} overload=flags ownNetOnly={}",
                java_angle_name(angle),
                own_net_only
            ));
            board.insert_trace_without_cleaning(
                detour_polyline(),
                0,
                30,
                vec![3],
                1,
                FixedState::Unfixed,
            );
            for id in trace_ids(&board) {
                let changed = <Board as PolylineTraceExt>::pull_tight(
                    &mut board,
                    id,
                    own_net_only,
                    500,
                    &never,
                )
                .expect("a never-tripping stop check");
                actual.push(format!("  trace id={} changed={}", id.0, changed));
            }
            actual.extend(dump_board(&board));
        }

        // (d) the refusals of `:811-828`.
        let mut board = probe_board(angle);
        actual.push(format!(
            "regime={} overload=refusals",
            java_angle_name(angle)
        ));
        let fixed_trace = board
            .insert_trace_without_cleaning(
                detour_polyline(),
                0,
                30,
                vec![3],
                1,
                FixedState::ShoveFixed,
            )
            .expect("the detour inserts");
        let mut c = algo(&mut board, 500);
        actual.push(format!(
            "  shoveFixed changed={}",
            <Board as PolylineTraceExt>::pull_tight_with(&mut board, fixed_trace, &mut c)
        ));
        let free = board
            .insert_trace_without_cleaning(
                Polyline::from_points(&[
                    p(-3000, 3000),
                    p(-3000, 6000),
                    p(-2000, 6000),
                    p(-2000, 3000),
                    p(-1000, 3000),
                ]),
                0,
                30,
                vec![3],
                1,
                FixedState::Unfixed,
            )
            .expect("the second detour inserts");
        let mut other_net =
            TraceTightener::get_instance(&mut board, vec![2], None, 500, None, -1, None, -1);
        actual.push(format!(
            "  otherNetFilter changed={}",
            <Board as PolylineTraceExt>::pull_tight_with(&mut board, free, &mut other_net)
        ));
        board.remove_item(free);
        let mut d = algo(&mut board, 500);
        actual.push(format!(
            "  removed changed={}",
            <Board as PolylineTraceExt>::pull_tight_with(&mut board, free, &mut d)
        ));
        actual.extend(dump_board(&board));
    }
    assert_rows("trace", &expected, &actual);
}

/// Probe mode `trace`, `regime=NINETY_DEGREE overload=refusals`:
///
/// ```text
///   shoveFixed changed=false
///   otherNetFilter changed=false
///   removed changed=false
/// ```
///
/// `PolylineTrace.pullTight:811-823` refuses a trace that is off the board, one that is
/// shove-fixed, and one whose nets do not match a non-empty `onlyNetNoArr`.
#[test]
fn pull_tight_refuses_a_shove_fixed_a_filtered_and_a_removed_trace() {
    let mut board = probe_board(AngleRestriction::NinetyDegree);
    let fixed_trace = board
        .insert_trace_without_cleaning(detour_polyline(), 0, 30, vec![3], 1, FixedState::ShoveFixed)
        .expect("the detour inserts");
    let mut a = algo(&mut board, 500);
    assert!(!<Board as PolylineTraceExt>::pull_tight_with(
        &mut board,
        fixed_trace,
        &mut a
    ));

    let free = board
        .insert_trace_without_cleaning(detour_polyline(), 0, 30, vec![3], 1, FixedState::Unfixed)
        .expect("the detour inserts");
    let mut other_net =
        TraceTightener::get_instance(&mut board, vec![2], None, 500, None, -1, None, -1);
    assert!(!<Board as PolylineTraceExt>::pull_tight_with(
        &mut board,
        free,
        &mut other_net
    ));

    board.remove_item(free);
    let mut b = algo(&mut board, 500);
    assert!(!<Board as PolylineTraceExt>::pull_tight_with(
        &mut board, free, &mut b
    ));
}

#[test]
fn pull_tight_honours_the_net_class_flag() {
    let mut board = probe_board(AngleRestriction::NinetyDegree);
    let trace = board
        .insert_trace_without_cleaning(detour_polyline(), 0, 30, vec![3], 1, FixedState::Unfixed)
        .expect("the detour inserts");
    let class = board
        .rules
        .nets
        .get(3)
        .expect("net 3 exists")
        .get_net_class();
    assert!(board.rules.net_classes.get(class).get_pull_tight());
    board.rules.net_classes.get_mut(class).set_pull_tight(false);

    let mut a = algo(&mut board, 500);
    assert!(!<Board as PolylineTraceExt>::pull_tight_with(
        &mut board, trace, &mut a
    ));
    assert_eq!(
        polyline_of(&board, trace).expect("a live trace").lines(),
        detour_polyline().lines(),
        "the polyline is untouched"
    );
}

// =================================================================================================
// `smoothenEndCornersAtTrace` — probe mode `smooth`
// =================================================================================================

#[test]
fn smoothen_end_corners_at_trace_matches_the_jvm() {
    let expected = section_for("smooth");
    let mut actual: Vec<String> = Vec::new();
    for angle in [
        AngleRestriction::NinetyDegree,
        AngleRestriction::FortyFiveDegree,
        AngleRestriction::None,
    ] {
        let mut board = probe_board(angle);
        actual.push(format!("regime={}", java_angle_name(angle)));
        insert_smoothen_fixture(&mut board);
        let mut a = algo(&mut board, 500);
        for id in trace_ids(&board) {
            let (layer, half_width, nets, cl, polyline) = {
                let trace = polyline_trace_of(&board, id).expect("a live trace");
                (
                    trace.get_layer(),
                    trace.get_half_width(),
                    trace.hdr.net_nos.clone(),
                    trace.hdr.clearance_class(),
                    trace.polyline().clone(),
                )
            };
            a.pull_tight_polyline(&mut board, &polyline, layer, half_width, &nets, cl, None);
            let start = a.smoothen_start_corner_at_trace(&mut board, id);
            let end = a.smoothen_end_corner_at_trace(&mut board, id);
            actual.push(format!(
                "  trace id={} start={}",
                id.0,
                start.as_ref().map_or("null".to_string(), dump_polyline)
            ));
            actual.push(format!(
                "  trace id={} end=  {}",
                id.0,
                end.as_ref().map_or("null".to_string(), dump_polyline)
            ));
        }
        for id in trace_ids(&board) {
            let changed = a
                .smoothen_end_corners_at_trace(&mut board, id)
                .expect("a never-tripping stop check");
            actual.push(format!(
                "  trace id={} smoothenEndCorners={}",
                id.0, changed
            ));
        }
        actual.extend(dump_board(&board));
    }
    // Re-cuts the port-lane golden: `T11_DUMP_SMOOTH=<path>` writes the rows this run produced,
    // for pasting under `data/p9t11-tightener-smooth.txt`'s `#` provenance header, where the
    // command is recorded. Writing rather than printing keeps 143 rows out of a captured stdout.
    if let Ok(path) = std::env::var("T11_DUMP_SMOOTH") {
        std::fs::write(path, actual.join("\n")).expect("the dump path is writable");
    }
    assert_rows("smooth", &expected, &actual);
}

#[test]
fn the_ninety_degree_regime_never_smoothens_an_end_corner() {
    let mut board = probe_board(AngleRestriction::NinetyDegree);
    let mut a = algo(&mut board, 500);
    for id in trace_ids(&board) {
        assert!(a.smoothen_start_corner_at_trace(&mut board, id).is_none());
        assert!(a.smoothen_end_corner_at_trace(&mut board, id).is_none());
    }
}

/// #210 — three traces share the corner being smoothed. Two of them (`far`, `near`) satisfy the
/// acute-angle branch; the third (`middle`) points the wrong way and satisfies neither. `far`
/// gets the smallest item id and `near` the largest, so an id-ordered walk that keeps whichever
/// match it visits last would keep `far`. The winning contact is instead the one whose own far
/// corner lands closest to the corner being smoothed — `near`, at distance 500 against `far`'s
/// 5000 — regardless of id.
#[test]
fn the_start_corner_contact_is_chosen_by_geometry() {
    let mut rules = rules_with_wide_class(AngleRestriction::FortyFiveDegree);
    let default_class = rules.get_default_net_class();
    rules.nets.add("N1", 1, false, default_class);
    let mut board = Board::new(
        Vec::new(),
        0,
        BOUNDING_BOX,
        rules,
        BoardLibrary::new(Padstacks::new(layers()), Packages::new()),
        Components::new(),
        Communication::default(),
    );

    let main = board
        .insert_trace_without_cleaning(
            Polyline::from_points(&[p(0, 0), p(1000, 1000), p(2000, 1000)]),
            0,
            30,
            vec![1],
            1,
            FixedState::Unfixed,
        )
        .expect("insert");
    let far = board
        .insert_trace_without_cleaning(
            Polyline::from_points(&[p(0, 0), p(5000, 0), p(5000, -500)]),
            0,
            30,
            vec![1],
            1,
            FixedState::Unfixed,
        )
        .expect("insert");
    board
        .insert_trace_without_cleaning(
            Polyline::from_points(&[p(0, 0), p(-500, 0), p(-500, -500)]),
            0,
            30,
            vec![1],
            1,
            FixedState::Unfixed,
        )
        .expect("insert");
    let near = board
        .insert_trace_without_cleaning(
            Polyline::from_points(&[p(0, 0), p(500, 0), p(500, -500)]),
            0,
            30,
            vec![1],
            1,
            FixedState::Unfixed,
        )
        .expect("insert");
    assert!(
        far.0 < near.0,
        "far must have the smaller id for this test to distinguish the two rules"
    );

    let near_first_line = polyline_of(&board, near).expect("near trace").lines()[1];
    let far_first_line = polyline_of(&board, far).expect("far trace").lines()[1];

    let main_polyline = polyline_of(&board, main).expect("main trace").clone();
    let mut a = algo(&mut board, 500);
    a.pull_tight_polyline(&mut board, &main_polyline, 0, 30, &[1], 1, None);
    let smoothed = a
        .smoothen_start_corner_at_trace(&mut board, main)
        .expect("the acute branch matches");

    assert_eq!(
        smoothed.lines()[0],
        near_first_line,
        "the nearest contact's own line shapes the new corner"
    );
    assert_ne!(
        smoothed.lines()[0],
        far_first_line,
        "not the farther contact, even though it has the smaller id"
    );
}

// =================================================================================================
// `PolylineTrace.pullTight:841-861` — probe mode `pinedge`
// =================================================================================================

#[test]
fn pin_edge_branch_matches_the_jvm() {
    let expected = section("pinedge");
    let mut actual: Vec<String> = Vec::new();
    for angle in [
        AngleRestriction::NinetyDegree,
        AngleRestriction::FortyFiveDegree,
        AngleRestriction::None,
    ] {
        for edge_to_turn_dist in [0.0_f64, 500.0] {
            let mut board = probe_board(angle);
            board.rules.set_pin_edge_to_turn_dist(edge_to_turn_dist);
            actual.push(format!(
                "regime={} pinEdgeToTurnDist={}",
                java_angle_name(angle),
                java_double_to_string(edge_to_turn_dist)
            ));
            let mut a = algo(&mut board, 500);
            for id in trace_ids(&board) {
                let changed = <Board as PolylineTraceExt>::pull_tight_with(&mut board, id, &mut a);
                actual.push(format!("  trace id={} changed={}", id.0, changed));
            }
            actual.extend(dump_board(&board));
        }
    }
    assert_rows("pinedge", &expected, &actual);
}

// =================================================================================================
// The three random blocks — probe modes `rand90`, `rand45`, `randany`
// =================================================================================================

const RANDOM_COUNT: usize = 256;
const RANDOM_SEED: i64 = 4242;

/// `P6T15aProbe.randomTable`, replayed with [`JavaRandom`] so the two sides draw the same stream.
fn random_rows(angle: AngleRestriction) -> Vec<String> {
    let mut board = probe_board(angle);
    let mut out = vec![format!(
        "regime={} n={} seed={}",
        java_angle_name(angle),
        RANDOM_COUNT,
        RANDOM_SEED
    )];
    let mut rnd = JavaRandom::new(RANDOM_SEED);
    for i in 0..RANDOM_COUNT {
        let corner_count = 2 + rnd.next_int(7);
        let corners: Vec<Point> = (0..corner_count)
            .map(|_| Point::new(rnd.next_int(4001) - 2000, rnd.next_int(4001) - 2000))
            .collect();
        let half_width = 10 + rnd.next_int(60);
        let layer = rnd.next_int(2) as usize;
        let net = 1 + rnd.next_int(3);
        let cl = (1 + rnd.next_int(2)) as usize;
        let mtd = 100 + rnd.next_int(900);
        let before = Polyline::from_points(&corners);
        if before.lines().len() < 3 {
            out.push(format!("row {i} degenerate"));
            continue;
        }
        let mut a = algo(&mut board, mtd);
        let after = a.pull_tight_polyline(&mut board, &before, layer, half_width, &[net], cl, None);
        out.push(format!(
            "row {} layer={} hw={} net={} cl={} mtd={} same={}",
            i,
            layer,
            half_width,
            net,
            cl,
            mtd,
            same_polyline(&before, &after)
        ));
        out.push(format!("  in  {}", dump_polyline(&before)));
        out.push(format!("  out {}", dump_polyline(&after)));
    }
    out
}

#[test]
fn random_block_matches_the_jvm_in_the_ninety_degree_regime() {
    assert_rows(
        "rand90",
        &section("rand90"),
        &random_rows(AngleRestriction::NinetyDegree),
    );
}

#[test]
fn random_block_matches_the_jvm_in_the_forty_five_degree_regime() {
    assert_rows(
        "rand45",
        &section("rand45"),
        &random_rows(AngleRestriction::FortyFiveDegree),
    );
}

#[test]
fn random_block_matches_the_jvm_in_the_any_angle_regime() {
    assert_rows(
        "randany",
        &section("randany"),
        &random_rows(AngleRestriction::None),
    );
}

#[test]
fn pull_tight_stops_when_the_stop_check_trips() {
    let mut board = probe_board(AngleRestriction::None);
    let stopped = std::cell::Cell::new(true);
    let check = || stopped.get();
    let before = detour_polyline();
    let mut a = TraceTightener::get_instance(
        &mut board,
        Vec::new(),
        None,
        500,
        Some(&check),
        -1,
        None,
        -1,
    );
    let after = a.pull_tight_polyline(&mut board, &before, 0, 30, &[3], 1, None);
    assert_eq!(
        after.lines(),
        before.lines(),
        "an already-stopped run leaves the polyline alone"
    );

    // The same run without the stop shortens it, which is what makes the assertion above a
    // statement about the stop check rather than about this polyline.
    let mut b = algo(&mut board, 500);
    let tightened = b.pull_tight_polyline(&mut board, &before, 0, 30, &[3], 1, None);
    assert_ne!(tightened.lines(), before.lines());
}

#[test]
fn smoothen_end_corners_stops_when_the_stop_check_trips() {
    let mut board = probe_board(AngleRestriction::FortyFiveDegree);
    insert_smoothen_fixture(&mut board);
    let trip = || true;
    let mut a =
        TraceTightener::get_instance(&mut board, Vec::new(), None, 500, Some(&trip), -1, None, -1);
    let mut sawstop = false;
    for id in trace_ids(&board) {
        match a.smoothen_end_corners_at_trace(&mut board, id) {
            Err(BoardError::Stopped) => sawstop = true,
            Err(other) => panic!("unexpected error {other:?}"),
            Ok(_) => {}
        }
    }
    assert!(
        sawstop,
        "the trip must surface as BoardError::Stopped, never as a hang"
    );
}

// =================================================================================================
// Helpers
// =================================================================================================

/// `P6T15aProbe.smoothen`'s six extra net-3 traces plus the net-1 trace that starts on the SMD
/// pin: three junctions that reach the `acuteAngle` arm, the `bend` arm (and through it
/// `repositionLine`) and the `else { return null; }` arm of the contact loop.
fn insert_smoothen_fixture(board: &mut Board) {
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[p(-500, 0), p(-1200, 700)]),
        0,
        30,
        vec![1],
        1,
        FixedState::Unfixed,
    );
    for corners in [
        vec![p(1000, 1000), p(2000, 2000)],
        vec![p(1000, 1000), p(1000, 2000)],
        vec![p(3000, 1000), p(4000, 1000), p(4000, 2000)],
        vec![p(3000, 1000), p(3000, 2000)],
        vec![p(5000, 1000), p(6000, 2000)],
        vec![p(6000, 2000), p(6000, 1000)],
        vec![p(9500, 5000), p(6000, 5000), p(6000, 6000)],
        vec![p(6000, 6000), p(9500, 6000), p(9500, 7000)],
    ] {
        board.insert_trace_without_cleaning(
            Polyline::from_points(&corners),
            0,
            30,
            vec![3],
            1,
            FixedState::Unfixed,
        );
    }
}

fn polyline_trace_of(board: &Board, id: ItemId) -> Option<&PolylineTrace> {
    match board.items.get(&id) {
        Some(Item::Trace(trace)) => Some(trace),
        _ => None,
    }
}

fn detour_polyline() -> Polyline {
    Polyline::from_points(&[
        p(-3000, -3000),
        p(-3000, 0),
        p(-2000, 0),
        p(-2000, -3000),
        p(-1000, -3000),
    ])
}

fn trace_ids(board: &Board) -> Vec<ItemId> {
    board
        .get_items()
        .filter(|item| item.is_trace())
        .map(|item| item.id())
        .collect()
}

fn polyline_of(board: &Board, id: ItemId) -> Option<&Polyline> {
    match board.items.get(&id) {
        Some(Item::Trace(trace)) => Some(trace.polyline()),
        _ => None,
    }
}

/// Compare two row lists and report the first difference with its line number, so a diff in a
/// 771-row random block names the row rather than dumping the block.
///
/// `expected` is the JVM transcript's section, except for a [`PORT_LANE`] mode, where it is the
/// port's own re-cut golden — see [`section_for`].
fn assert_rows(mode: &str, expected: &[&str], actual: &[String]) {
    let lane = if PORT_LANE.iter().any(|(name, _, _)| *name == mode) {
        "port-lane golden"
    } else {
        "JVM transcript"
    };
    for (i, (want, got)) in expected.iter().zip(actual.iter()).enumerate() {
        assert_eq!(
            *want,
            got.as_str(),
            "mode `{mode}` row {i} differs from the {lane}"
        );
    }
    assert_eq!(
        expected.len(),
        actual.len(),
        "mode `{mode}` row count differs from the {lane}"
    );
}

const T11_SMOOTH: &str = include_str!("data/p9t11-tightener-smooth.txt");

const PORT_LANE: &[(&str, &str, &str)] = &[(
    "smooth",
    "#183",
    "`TraceTightenerAnyAngle.smoothenEndCornerAtTrace` read `prevLineDirection` from the same \
     line as `lineDirection`, so the `bend` arm — which needs the two to differ — was unreachable \
     for every input. Reading `lines[endLineNo - 1]` makes it reachable: measured 0 executions \
     before and 3 after, over this very fixture.",
)];

/// The port-lane golden's rows, with its `#` provenance header stripped.
fn port_lane_section(mode: &str) -> Vec<&'static str> {
    let text = match mode {
        "smooth" => T11_SMOOTH,
        _ => panic!("no port-lane golden for mode `{mode}`"),
    };
    let rows: Vec<&str> = text
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(str::trim_end)
        .collect();
    assert!(!rows.is_empty(), "port-lane golden `{mode}` is empty");
    rows
}

/// [`section`] for a jar-lane mode, [`port_lane_section`] for a port-lane one.
fn section_for(mode: &str) -> Vec<&'static str> {
    if PORT_LANE.iter().any(|(name, _, _)| *name == mode) {
        port_lane_section(mode)
    } else {
        section(mode)
    }
}

/// Every [`PORT_LANE`] mode must **still** differ from the jar. Without this, a fix that was
/// quietly reverted would go green against its own re-cut golden and nothing would notice.
#[test]
fn the_port_lane_modes_still_differ_from_the_jar() {
    for (mode, row, reason) in PORT_LANE {
        assert_ne!(
            port_lane_section(mode),
            section(mode),
            "port-lane mode `{mode}` now MATCHES the jar — delete its PORT_LANE entry \
             (register {row}: {reason})"
        );
    }
}
