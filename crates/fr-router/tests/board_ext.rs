use fr_board::BoardError;
use fr_board::ids::{ItemId, PadstackId};
use fr_board::items::Item;
use fr_board::prelude::*;
use fr_dsn::format::double::java_double_to_string;
use fr_geometry::{
    IntBox, IntOctagon, IntPoint, IntVector, JavaRandom, Point, Polyline, Shape, TileShape, Vector,
};
use fr_router::PageId;
use fr_router::autoroute::maze::engine::AutorouteEngine;
use fr_router::board_ext::{DrillItemMover, RoutingBoardExt, TraceShover};


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

fn bare_board_with_a_net_two_trace() -> Board {
    bare_board_bounded(BOUNDING_BOX)
}

fn bare_board_bounded(bounds: IntBox) -> Board {
    let mut board = Board::new(
        Vec::new(),
        0,
        bounds,
        rules_with_wide_class(AngleRestriction::None),
        BoardLibrary::new(Padstacks::new(layers()), Packages::new()),
        Components::new(),
        Communication::default(),
    );
    let default_class = board.rules.get_default_net_class();
    board.rules.nets.add("N1", 1, false, default_class);
    board.rules.nets.add("N2", 1, false, default_class);
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(0, -400), Point::new(0, 400)]),
        0,
        30,
        vec![2],
        1,
        FixedState::Unfixed,
    );
    board
}

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

fn probe_lines() -> Vec<(&'static str, Polyline)> {
    vec![
        (
            "acrossNet1",
            Polyline::from_points(&[Point::new(-300, -600), Point::new(-300, 600)]),
        ),
        (
            "intoNet1",
            Polyline::from_points(&[Point::new(-300, -600), Point::new(-300, 0)]),
        ),
        (
            "acrossNet2",
            Polyline::from_points(&[Point::new(-1200, 600), Point::new(-400, 600)]),
        ),
        (
            "throughPin",
            Polyline::from_points(&[Point::new(500, -600), Point::new(500, 600)]),
        ),
        (
            "freeSpace",
            Polyline::from_points(&[Point::new(2000, 2000), Point::new(3000, 2000)]),
        ),
        (
            "twoSegments",
            Polyline::from_points(&[
                Point::new(-1200, 600),
                Point::new(-300, 600),
                Point::new(-300, -600),
            ]),
        ),
        (
            "offBoard",
            Polyline::from_points(&[Point::new(9800, 9800), Point::new(12000, 9800)]),
        ),
    ]
}


#[test]
fn init_autoroute_reuses_the_engine_only_on_a_matching_clearance_class() {
    let mut board = bare_board_with_a_net_two_trace();
    let engine = board.init_autoroute(None, 1, 1, None, true);
    let tree = engine.tree;

    let engine = board.init_autoroute(Some(engine), 2, 1, None, true);
    assert_eq!(
        engine.tree, tree,
        "the engine was rebuilt on a matching class"
    );

    let rebuilt = board.init_autoroute(Some(engine), 2, 2, None, true);
    assert_ne!(rebuilt.tree, tree, "the class change did not rebuild");
    assert_eq!(
        rebuilt.complete_expansion_rooms().len(),
        0,
        "a rebuilt engine starts with no rooms — probe `afterClassChange complete=null`"
    );
    assert_eq!(compensated_class(&board, &rebuilt), 2);
}

#[test]
fn init_autoroute_never_reuses_when_retain_is_false() {
    let mut board = bare_board_with_a_net_two_trace();
    let engine = board.init_autoroute(None, 1, 1, None, true);
    let rooms_before = seed_and_complete(&mut board, engine);
    assert_eq!(rooms_before.1, 1, "probe `completed n=1`");
    let engine = rooms_before.0;

    let fresh = board.init_autoroute(Some(engine), 1, 1, None, false);
    assert_eq!(
        fresh.complete_expansion_rooms().len(),
        0,
        "retain = false must have thrown the room database away"
    );
    assert!(!fresh.maintain_database, "`maintainDatabase` is `retain`");
}

#[test]
fn finish_autoroute_clears_the_room_database_before_dropping_the_engine() {
    let mut board = bare_board_with_a_net_two_trace();
    let engine = board.init_autoroute(None, 1, 1, None, true);
    let (engine, completed) = seed_and_complete(&mut board, engine);
    assert_eq!(completed, 1);
    let tree_size_with_rooms = tree_size(&board, &engine);
    board.finish_autoroute(engine);
    assert!(
        tree_size_with_rooms > 1,
        "the rooms should have been in the tree before finishAutoroute"
    );
}


#[test]
fn additional_update_after_change_removes_the_overlapping_rooms() {
    let mut board = bare_board_with_a_net_two_trace();
    let engine = board.init_autoroute(None, 1, 1, None, true);
    let (engine, completed) = seed_and_complete(&mut board, engine);
    assert_eq!(completed, 1, "probe `completed n=1`");
    assert_eq!(engine.complete_expansion_rooms().len(), 1);
    let room = engine.complete_expansion_rooms()[0];
    let room_ref = engine
        .rooms
        .complete_room(room)
        .expect("the completed room");
    assert_eq!(
        room_ref.get_id(),
        6,
        "port id; the jar's probe says `complete id=1`"
    );
    assert!(
        !room_ref.is_net_dependent(),
        "probe `netDependent=false` — the removal below can only be :111-117's"
    );
    assert_eq!(room_ref.get_doors().len(), 6, "probe `doors=6`");
    assert_eq!(tree_size(&board, &engine), 2, "probe `treeSize=2`");
    assert_eq!(
        engine.rooms.incomplete_rooms.len(),
        5,
        "probe `afterComplete incomplete=5`"
    );

    let engine = board.init_autoroute(Some(engine), 2, 1, None, true);
    assert_eq!(
        engine.complete_expansion_rooms().len(),
        0,
        "probe `afterInitNet2 complete=0`"
    );
    assert_eq!(
        engine.rooms.incomplete_rooms.len(),
        1,
        "probe `afterInitNet2 incomplete=1`"
    );
    assert_eq!(
        tree_size(&board, &engine),
        1,
        "probe `afterInitNet2 treeSize=1`"
    );
}

#[test]
fn additional_update_after_change_invalidates_the_drill_pages_of_every_tree_shape() {
    let mut board = bare_board_with_a_net_two_trace();
    let mut engine = board.init_autoroute(None, 1, 1, None, true);
    let trace = board
        .items_in_board_order()
        .into_iter()
        .find(|id| board.get_item(*id).is_some_and(Item::is_trace))
        .expect("the net-2 trace");

    let tree = engine.tree;
    let shape = board
        .item_tree_shape(trace, tree, 0)
        .expect("the trace's tree shape");
    let overlapping: Vec<PageId> = engine.drill_pages().overlapping_pages(&shape);
    assert!(!overlapping.is_empty());

    let populated = populate_drill_pages(&mut board, &mut engine);
    assert!(
        populated > 0,
        "the fixture needs at least one page holding drills"
    );
    let held: usize = overlapping
        .iter()
        .map(|page| {
            engine
                .drill_pages()
                .page(*page)
                .drills()
                .map_or(0, <[_]>::len)
        })
        .sum();
    assert_eq!(
        held, 7,
        "every overlapping page should be holding real drills before the invalidation, and on \
         this board that is seven of them (quirk #169 fixed; before it, zero)"
    );

    board.additional_update_after_change(&mut engine, trace);

    assert!(
        overlapping
            .iter()
            .all(|page| engine.drill_pages().page(*page).drills().is_none()),
        "`:107` should have invalidated every page the trace's tree shape overlaps"
    );
}


#[test]
fn check_forced_trace_polyline_uses_the_default_tree_and_the_bounding_box_in_ninety_degree_mode() {
    let expected: &[(&str, i32, bool)] = &[
        ("acrossNet1", 30, false),
        ("acrossNet1", 120, false),
        ("intoNet1", 30, false),
        ("intoNet1", 120, false),
        ("acrossNet2", 30, true),
        ("acrossNet2", 120, true),
        ("throughPin", 30, false),
        ("throughPin", 120, false),
        ("freeSpace", 30, true),
        ("freeSpace", 120, true),
        ("twoSegments", 30, false),
        ("twoSegments", 120, false),
        ("offBoard", 30, false),
        ("offBoard", 120, false),
    ];
    for angle in [AngleRestriction::None, AngleRestriction::NinetyDegree] {
        let mut board = probe_board(angle);
        let lines = probe_lines();
        let mut actual = Vec::new();
        for (name, polyline) in &lines {
            for half_width in [30, 120] {
                let ok =
                    board.check_forced_trace_polyline(polyline, half_width, 0, &[3], 1, 20, 5, 20);
                actual.push((*name, half_width, ok));
            }
        }
        assert_eq!(actual, expected, "angle regime {angle:?}");
    }
}


#[test]
fn trace_shover_check_refuses_at_the_recursion_limit() {
    let mut board = probe_board(AngleRestriction::None);
    let rows = instance_check_rows(&mut board);
    let two_segments: Vec<_> = rows
        .iter()
        .filter(|(name, shape, kind, _, _)| {
            *name == "twoSegments" && *shape == 0 && *kind == "maxRecursionDepth"
        })
        .map(|(_, _, _, value, ok)| (*value, *ok))
        .collect();
    assert_eq!(
        two_segments,
        vec![(0, false), (1, true), (2, true), (20, true)]
    );
}

#[test]
fn trace_shover_check_agrees_with_the_jvm_on_every_probe_row() {
    let expected: &[(&str, usize, &str, i32, bool)] = &[
        ("acrossNet1", 0, "maxRecursionDepth", 0, false),
        ("acrossNet1", 0, "maxRecursionDepth", 1, false),
        ("acrossNet1", 0, "maxRecursionDepth", 2, false),
        ("acrossNet1", 0, "maxRecursionDepth", 20, false),
        ("acrossNet1", 0, "maxSpringOver", 0, false),
        ("acrossNet1", 0, "maxSpringOver", 1, false),
        ("acrossNet1", 0, "maxSpringOver", 20, false),
        ("intoNet1", 0, "maxRecursionDepth", 0, false),
        ("intoNet1", 0, "maxRecursionDepth", 1, false),
        ("intoNet1", 0, "maxRecursionDepth", 2, false),
        ("intoNet1", 0, "maxRecursionDepth", 20, false),
        ("intoNet1", 0, "maxSpringOver", 0, false),
        ("intoNet1", 0, "maxSpringOver", 1, false),
        ("intoNet1", 0, "maxSpringOver", 20, false),
        ("acrossNet2", 0, "maxRecursionDepth", 0, true),
        ("acrossNet2", 0, "maxRecursionDepth", 1, true),
        ("acrossNet2", 0, "maxRecursionDepth", 2, true),
        ("acrossNet2", 0, "maxRecursionDepth", 20, true),
        ("acrossNet2", 0, "maxSpringOver", 0, true),
        ("acrossNet2", 0, "maxSpringOver", 1, true),
        ("acrossNet2", 0, "maxSpringOver", 20, true),
        ("throughPin", 0, "maxRecursionDepth", 0, false),
        ("throughPin", 0, "maxRecursionDepth", 1, false),
        ("throughPin", 0, "maxRecursionDepth", 2, false),
        ("throughPin", 0, "maxRecursionDepth", 20, false),
        ("throughPin", 0, "maxSpringOver", 0, false),
        ("throughPin", 0, "maxSpringOver", 1, false),
        ("throughPin", 0, "maxSpringOver", 20, false),
        ("freeSpace", 0, "maxRecursionDepth", 0, true),
        ("freeSpace", 0, "maxRecursionDepth", 1, true),
        ("freeSpace", 0, "maxRecursionDepth", 2, true),
        ("freeSpace", 0, "maxRecursionDepth", 20, true),
        ("freeSpace", 0, "maxSpringOver", 0, true),
        ("freeSpace", 0, "maxSpringOver", 1, true),
        ("freeSpace", 0, "maxSpringOver", 20, true),
        ("twoSegments", 0, "maxRecursionDepth", 0, false),
        ("twoSegments", 0, "maxRecursionDepth", 1, true),
        ("twoSegments", 0, "maxRecursionDepth", 2, true),
        ("twoSegments", 0, "maxRecursionDepth", 20, true),
        ("twoSegments", 0, "maxSpringOver", 0, true),
        ("twoSegments", 0, "maxSpringOver", 1, true),
        ("twoSegments", 0, "maxSpringOver", 20, true),
        ("twoSegments", 1, "maxRecursionDepth", 0, false),
        ("twoSegments", 1, "maxRecursionDepth", 1, false),
        ("twoSegments", 1, "maxRecursionDepth", 2, false),
        ("twoSegments", 1, "maxRecursionDepth", 20, false),
        ("twoSegments", 1, "maxSpringOver", 0, false),
        ("twoSegments", 1, "maxSpringOver", 1, false),
        ("twoSegments", 1, "maxSpringOver", 20, false),
        ("offBoard", 0, "maxRecursionDepth", 0, false),
        ("offBoard", 0, "maxRecursionDepth", 1, false),
        ("offBoard", 0, "maxRecursionDepth", 2, false),
        ("offBoard", 0, "maxRecursionDepth", 20, false),
        ("offBoard", 0, "maxSpringOver", 0, false),
        ("offBoard", 0, "maxSpringOver", 1, false),
        ("offBoard", 0, "maxSpringOver", 20, false),
    ];
    let mut board = probe_board(AngleRestriction::None);
    assert_eq!(instance_check_rows(&mut board), expected);
}

#[test]
fn trace_shover_check_does_not_mutate_the_board() {
    let mut board = probe_board(AngleRestriction::None);
    let before = board.structural_hash();
    let _ = instance_check_rows(&mut board);
    let mut board2 = probe_board(AngleRestriction::None);
    for (_, polyline) in probe_lines() {
        let _ = board2.check_forced_trace_polyline(&polyline, 120, 0, &[3], 1, 20, 5, 20);
    }
    assert_eq!(board.structural_hash(), before);
    assert_eq!(board2.structural_hash(), before);
}


#[test]
fn trace_shover_check_segment_agrees_with_the_jvm() {
    let max = f64::from(i32::MAX);
    let expected: &[(&str, bool, i32, f64)] = &[
        ("acrossNet1", false, 30, 0.0),
        ("acrossNet1", false, 120, 0.0),
        ("acrossNet1", true, 30, 0.0),
        ("acrossNet1", true, 120, 0.0),
        ("intoNet1", false, 30, 0.0),
        ("intoNet1", false, 120, 0.0),
        ("intoNet1", true, 30, 0.0),
        ("intoNet1", true, 120, 0.0),
        ("acrossNet2", false, 30, 0.0),
        ("acrossNet2", false, 120, 0.0),
        ("acrossNet2", true, 30, 0.0),
        ("acrossNet2", true, 120, 0.0),
        ("throughPin", false, 30, 0.0),
        ("throughPin", false, 120, 0.0),
        ("throughPin", true, 30, 0.0),
        ("throughPin", true, 120, 0.0),
        ("freeSpace", false, 30, max),
        ("freeSpace", false, 120, max),
        ("freeSpace", true, 30, max),
        ("freeSpace", true, 120, max),
        ("offBoard", false, 30, 0.0),
        ("offBoard", false, 120, 0.0),
        ("offBoard", true, 30, 0.0),
        ("offBoard", true, 120, 0.0),
    ];
    let mut board = probe_board(AngleRestriction::None);
    let mut actual = Vec::new();
    for (name, polyline) in probe_lines() {
        if polyline.corner_count() != 2 {
            continue;
        }
        let segment = fr_geometry::LineSegment::from_polyline(&polyline, 1).expect("a segment");
        for left in [false, true] {
            for half_width in [30, 120] {
                let result = TraceShover::check_segment(
                    &mut board,
                    &segment,
                    left,
                    0,
                    &[3],
                    half_width,
                    1,
                    20,
                    5,
                );
                actual.push((name, left, half_width, result));
            }
        }
    }
    assert_eq!(actual, expected);
}


#[test]
fn ignore_items_at_tie_pins_answers_the_contacts_of_the_own_net_pins() {
    let board = probe_board(AngleRestriction::None);
    let shapes = [
        TileShape::Box(IntBox::from_coords(-600, -100, -400, 100)),
        TileShape::Box(IntBox::from_coords(400, -100, 600, 100)),
        TileShape::Box(IntBox::from_coords(2000, 2000, 2200, 2200)),
    ];
    let mut actual = Vec::new();
    for (i, shape) in shapes.iter().enumerate() {
        for nets in [vec![1], vec![3], vec![]] {
            let ignore = TraceShover::ignore_items_at_tie_pins(&board, shape, 0, &nets);
            actual.push((i, nets, ignore));
        }
    }
    assert_eq!(
        actual,
        vec![
            (0, vec![1], vec![ItemId(4)]),
            (0, vec![3], vec![]),
            (0, vec![], vec![]),
            (1, vec![1], vec![]),
            (1, vec![3], vec![]),
            (1, vec![], vec![]),
            (2, vec![1], vec![]),
            (2, vec![3], vec![]),
            (2, vec![], vec![]),
        ]
    );
}


#[test]
fn drill_item_mover_check_agrees_with_the_jvm() {
    for angle in [AngleRestriction::None, AngleRestriction::NinetyDegree] {
        let mut board = probe_board(angle);
        let (_free, fixed, on_pin) = insert_probe_vias(&mut board);
        assert_eq!(fixed, ItemId(7), "probe `fixedViaId=7`");
        assert_eq!(on_pin, ItemId(8), "probe `onPinViaId=8`");
        assert_eq!(
            board
                .normal_contacts(on_pin)
                .into_iter()
                .collect::<Vec<_>>(),
            vec![ItemId(3)],
            "probe `onPinViaId=8 contacts=1 kinds=Pin#3`"
        );

        let delta = Vector::from(IntVector::new(300, 0));
        for via in [fixed, on_pin] {
            let mut ignore = Vec::new();
            let ok = DrillItemMover::check(&mut board, via, &delta, 20, 5, Some(&mut ignore), None);
            assert!(
                !ok,
                "probe `check viaId={} result=false` ({angle:?})",
                via.0
            );
            assert!(
                ignore.is_empty(),
                "probe `ignoreSize=0` — both arms return before `:63` adds the drill item"
            );
        }
    }
}

type ShoveRow = (AngleRestriction, &'static str, bool, Vec<(i32, i32)>);
type ShoveRowRef<'a> = (AngleRestriction, &'a str, bool, &'a [(i32, i32)]);

#[test]
fn try_shove_via_points_agrees_with_the_jvm() {
    let box_shape = TileShape::Box(IntBox::from_coords(1900, 1900, 2400, 2400));
    let octagon_shape = TileShape::Octagon(IntOctagon::new(
        1900, 1900, 2400, 2400, -600, 4600, -600, 4600,
    ));
    let expected: &[ShoveRowRef<'_>] = &[
        (AngleRestriction::None, "box", false, &[(1612, 2000)]),
        (
            AngleRestriction::None,
            "box",
            true,
            &[(1612, 2000), (2000, 1612), (1696, 1696), (1612, 2388)],
        ),
        (AngleRestriction::None, "octagon", false, &[(1612, 2000)]),
        (
            AngleRestriction::None,
            "octagon",
            true,
            &[(1612, 2000), (2000, 1612), (1612, 2388), (1612, 1612)],
        ),
        (
            AngleRestriction::NinetyDegree,
            "box",
            false,
            &[(1612, 2000)],
        ),
        (
            AngleRestriction::NinetyDegree,
            "box",
            true,
            &[(1612, 2000), (2000, 1612)],
        ),
        (
            AngleRestriction::NinetyDegree,
            "octagon",
            false,
            &[(1612, 2000)],
        ),
        (
            AngleRestriction::NinetyDegree,
            "octagon",
            true,
            &[(1612, 2000), (2000, 1612)],
        ),
    ];
    let mut actual: Vec<ShoveRow> = Vec::new();
    for angle in [AngleRestriction::None, AngleRestriction::NinetyDegree] {
        let mut board = probe_board(angle);
        let (free, _, _) = insert_probe_vias(&mut board);
        assert_eq!(free, ItemId(6), "probe `freeViaId=6`");
        for (label, shape) in [("box", &box_shape), ("octagon", &octagon_shape)] {
            for extended in [false, true] {
                let points =
                    DrillItemMover::try_shove_via_points(&mut board, shape, 0, free, 1, extended);
                actual.push((
                    angle,
                    label,
                    extended,
                    points.iter().map(|p| (p.x, p.y)).collect(),
                ));
            }
        }
    }
    let expected: Vec<ShoveRow> = expected
        .iter()
        .map(|(a, l, e, ps)| (*a, *l, *e, ps.to_vec()))
        .collect();
    assert_eq!(actual, expected);
}


fn insert_probe_vias(board: &mut Board) -> (ItemId, ItemId, ItemId) {
    let through = PadstackId(
        board
            .library
            .padstacks
            .get_by_name("thru")
            .expect("the thru padstack")
            .no,
    );
    let free = board
        .insert_via(
            through,
            Point::new(2000, 2000),
            vec![3],
            1,
            FixedState::Unfixed,
            false,
        )
        .expect("the free via");
    let fixed = board
        .insert_via(
            through,
            Point::new(3000, 2000),
            vec![3],
            1,
            FixedState::ShoveFixed,
            false,
        )
        .expect("the shove-fixed via");
    let on_pin = board
        .insert_via(
            through,
            Point::new(500, 0),
            vec![1],
            1,
            FixedState::Unfixed,
            false,
        )
        .expect("the via on the pin");
    (free, fixed, on_pin)
}

fn instance_check_rows(board: &mut Board) -> Vec<(&'static str, usize, &'static str, i32, bool)> {
    let mut rows = Vec::new();
    for (name, polyline) in probe_lines() {
        let shapes = polyline.offset_shapes_between(120, 0, polyline.lines().len() - 1);
        for (s, shape) in shapes.iter().enumerate() {
            let from_side = ShapeEntrySide::from_polyline(&polyline, s + 1, shape);
            for depth in [0, 1, 2, 20] {
                let ok = TraceShover::check(
                    board,
                    shape,
                    Some(&from_side),
                    None,
                    0,
                    &[3],
                    1,
                    depth,
                    5,
                    20,
                    None,
                );
                rows.push((name, s, "maxRecursionDepth", depth, ok));
            }
            for spring_over in [0, 1, 20] {
                let ok = TraceShover::check(
                    board,
                    shape,
                    Some(&from_side),
                    None,
                    0,
                    &[3],
                    1,
                    20,
                    5,
                    spring_over,
                    None,
                );
                rows.push((name, s, "maxSpringOver", spring_over, ok));
            }
        }
    }
    rows
}

fn seed_and_complete(board: &mut Board, mut engine: AutorouteEngine) -> (AutorouteEngine, usize) {
    let seed = engine.add_incomplete_expansion_room(
        None,
        0,
        Some(TileShape::Box(IntBox::from_coords(-600, -100, -400, 100))),
    );
    let completed = engine
        .complete_expansion_room(board, seed)
        .unwrap_or_default()
        .len();
    (engine, completed)
}

fn compensated_class(board: &Board, engine: &AutorouteEngine) -> usize {
    board
        .trees
        .trees()
        .find(|tree| tree.id() == engine.tree)
        .expect("the autoroute tree")
        .compensated_clearance_class()
}

fn tree_size(board: &Board, engine: &AutorouteEngine) -> usize {
    board
        .trees
        .trees()
        .find(|tree| tree.id() == engine.tree)
        .expect("the autoroute tree")
        .size()
}

fn populate_drill_pages(board: &mut Board, engine: &mut AutorouteEngine) -> usize {
    for page in all_pages(engine) {
        let _ = engine.drill_page_drills(board, page, false, &|| false);
    }
    pages_holding_drills(engine)
}

fn all_pages(engine: &AutorouteEngine) -> Vec<PageId> {
    let array = engine.drill_pages();
    let mut result = Vec::new();
    for j in 0..array.row_count() {
        for i in 0..array.column_count() {
            result.push(array.page_id(i, j));
        }
    }
    result
}

fn pages_holding_drills(engine: &AutorouteEngine) -> usize {
    all_pages(engine)
        .into_iter()
        .filter(|page| engine.drill_pages().page(*page).drills().is_some())
        .count()
}


const T15B: &str = include_str!("data/p6t15b-insert-forced.txt");

fn t15b_section(mode: &str) -> Vec<&'static str> {
    let header = format!("######## {mode}");
    let mut rows = Vec::new();
    let mut inside = false;
    for line in T15B.lines() {
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

fn assert_rows_match(mode: &str, actual: &[String]) {
    let expected = t15b_section(mode);
    let mut diffs = Vec::new();
    for i in 0..expected.len().max(actual.len()) {
        let want = expected.get(i).copied().unwrap_or("<missing>");
        let got = actual.get(i).map(String::as_str).unwrap_or("<missing>");
        if want != got {
            diffs.push(format!("row {i}\n  jvm:  {want}\n  rust: {got}"));
        }
    }
    assert!(
        diffs.is_empty(),
        "mode `{mode}`: {} of {} rows differ\n{}",
        diffs.len(),
        expected.len().max(actual.len()),
        diffs
            .iter()
            .take(12)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}


fn t15b_line(line: &fr_geometry::Line) -> String {
    format!("({},{})->({},{})", line.a.x, line.a.y, line.b.x, line.b.y)
}

fn t15b_corner(polyline: &Polyline, no: usize) -> String {
    match polyline.corner(no) {
        Some(Point::Int(p)) => format!("({},{})", p.x, p.y),
        _ => {
            let f = polyline.corner_approx(no).expect("no is below cornerCount");
            format!(
                "~({},{})",
                java_double_to_string(f.x),
                java_double_to_string(f.y)
            )
        }
    }
}

fn t15b_polyline(polyline: Option<&Polyline>) -> String {
    let Some(polyline) = polyline else {
        return "null".to_string();
    };
    let lines: Vec<String> = polyline.lines().iter().map(t15b_line).collect();
    let corners: Vec<String> = (0..polyline.corner_count())
        .map(|i| t15b_corner(polyline, i))
        .collect();
    format!(
        "n={} lines=[{}] corners=[{}]",
        polyline.lines().len(),
        lines.join(","),
        corners.join(",")
    )
}

fn t15b_answer(point: Option<&Point>) -> String {
    match point {
        None => "null".to_string(),
        Some(Point::Int(p)) => format!("({},{})", p.x, p.y),
        Some(other) => {
            let f = other.to_float();
            format!(
                "~({},{})",
                java_double_to_string(f.x),
                java_double_to_string(f.y)
            )
        }
    }
}

fn t15b_point(point: &Point) -> String {
    let rounded = point.to_float().round();
    format!("({},{})", rounded.x, rounded.y)
}

fn t15b_nets(net_nos: &[i32]) -> String {
    let inner: Vec<String> = net_nos.iter().map(i32::to_string).collect();
    format!("[{}]", inner.join(","))
}

fn t15b_type_name(item: &Item) -> &'static str {
    match item {
        Item::Trace(_) => "PolylineTrace",
        Item::Via(_) => "Via",
        Item::Pin(_) => "Pin",
        Item::ObstacleArea(_) => "ObstacleArea",
        Item::ConductionArea(_) => "ConductionArea",
        Item::ViaObstacleArea(_) => "ViaObstacleArea",
        Item::ComponentObstacleArea(_) => "ComponentObstacleArea",
        Item::ComponentOutline(_) => "ComponentOutline",
        Item::BoardOutline(_) => "BoardOutline",
    }
}

fn t15b_failing(board: &Board) -> String {
    let obstacle = match board.get_shove_failing_obstacle() {
        None => "null".to_string(),
        Some(id) => match board.get_item(id) {
            Some(item) => format!("{}#{}", t15b_type_name(item), id.0),
            None => format!("removed#{}", id.0),
        },
    };
    format!(
        "failing={} failingLayer={}",
        obstacle,
        board.get_shove_failing_layer()
    )
}

fn t15b_board_dump(board: &Board) -> Vec<String> {
    let mut out = vec![format!(
        "    maxId={}",
        board.communication.id_gen.max_generated_id()
    )];
    for item in board.get_items() {
        let mut line = format!(
            "    item id={} type={} nets={} cl={}",
            item.id().0,
            t15b_type_name(item),
            t15b_nets(item.net_nos()),
            item.clearance_class()
        );
        match item {
            Item::Trace(trace) => line.push_str(&format!(
                " layer={} hw={} {}",
                trace.get_layer(),
                trace.get_half_width(),
                t15b_polyline(Some(trace.polyline()))
            )),
            Item::Pin(_) => {
                let center = board.drill_center(item.id()).expect("a pin has a centre");
                line.push_str(&format!(" center={}", t15b_point(&center)));
            }
            _ => {}
        }
        out.push(line);
    }
    out
}

fn t15b_regime_name(angle: AngleRestriction) -> &'static str {
    match angle {
        AngleRestriction::NinetyDegree => "90",
        AngleRestriction::FortyFiveDegree => "45",
        AngleRestriction::None => "any",
    }
}

const T15B_REGIMES: [AngleRestriction; 3] = [
    AngleRestriction::NinetyDegree,
    AngleRestriction::FortyFiveDegree,
    AngleRestriction::None,
];

const T15B_MAXV: i32 = i32::MAX;

const LADDER_Y: i32 = -3000;

fn thru_padstack(board: &Board) -> PadstackId {
    PadstackId(
        board
            .library
            .padstacks
            .get_by_name("thru")
            .expect("the thru padstack")
            .no,
    )
}

fn obstacles_board(angle: AngleRestriction) -> Board {
    let mut board = probe_board(angle);
    let through = thru_padstack(&board);
    board
        .insert_via(
            through,
            Point::new(2400, 2000),
            vec![3],
            1,
            FixedState::UserFixed,
            false,
        )
        .expect("the user-fixed via");
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(-1500, -900), Point::new(1500, -900)]),
        0,
        30,
        vec![2],
        1,
        FixedState::ShoveFixed,
    );
    board
}

fn ladder_board(count: i32) -> Board {
    let mut board = probe_board(AngleRestriction::None);
    let through = thru_padstack(&board);
    for i in 0..count {
        board
            .insert_via(
                through,
                Point::new(-2000 + 400 * i, LADDER_Y),
                vec![1],
                1,
                FixedState::UserFixed,
                false,
            )
            .expect("a ladder via");
    }
    board
}

fn overlap_board(gap: i32) -> Board {
    let mut board = probe_board(AngleRestriction::None);
    let through = thru_padstack(&board);
    for x in [0, gap] {
        board
            .insert_via(
                through,
                Point::new(x, LADDER_Y),
                vec![1],
                1,
                FixedState::UserFixed,
                false,
            )
            .expect("an overlap via");
    }
    board
}

struct T15bCase {
    name: &'static str,
    corners: Vec<Point>,
    layer: usize,
    half_width: i32,
    nets: Vec<i32>,
    cl: usize,
}

fn t15b_cases() -> Vec<T15bCase> {
    let case = |name, corners: Vec<Point>, nets: Vec<i32>| T15bCase {
        name,
        corners,
        layer: 0,
        half_width: 30,
        nets,
        cl: 1,
    };
    vec![
        case(
            "free",
            vec![Point::new(2000, 2000), Point::new(3000, 2000)],
            vec![3],
        ),
        case(
            "acrossNet1",
            vec![Point::new(-300, -600), Point::new(-300, 600)],
            vec![3],
        ),
        case(
            "intoNet1",
            vec![Point::new(-300, -600), Point::new(-300, 0)],
            vec![3],
        ),
        case(
            "acrossNet2",
            vec![Point::new(-1200, 600), Point::new(-400, 600)],
            vec![3],
        ),
        case(
            "throughPin",
            vec![Point::new(500, -600), Point::new(500, 600)],
            vec![3],
        ),
        case(
            "twoSegments",
            vec![
                Point::new(-1200, 600),
                Point::new(-300, 600),
                Point::new(-300, -600),
            ],
            vec![3],
        ),
        case(
            "ownNetEnd",
            vec![Point::new(500, 400), Point::new(900, 400)],
            vec![1],
        ),
        case(
            "offBoard",
            vec![Point::new(9800, 9800), Point::new(12000, 9800)],
            vec![3],
        ),
        case(
            "degenerate",
            vec![Point::new(2000, 2000), Point::new(2000, 2000)],
            vec![3],
        ),
        case(
            "ownNetTee",
            vec![Point::new(300, -600), Point::new(300, 400)],
            vec![1],
        ),
        case(
            "ownNetCross",
            vec![Point::new(300, -600), Point::new(300, 600)],
            vec![1],
        ),
        case(
            "foreignTee",
            vec![Point::new(-1400, 600), Point::new(-800, 600)],
            vec![3],
        ),
        case(
            "sharpTurn",
            vec![
                Point::new(-1000, -800),
                Point::new(200, -800),
                Point::new(0, -700),
            ],
            vec![3],
        ),
        T15bCase {
            name: "net2Tee",
            corners: vec![Point::new(-1400, 600), Point::new(-800, 600)],
            layer: 0,
            half_width: 40,
            nets: vec![2],
            cl: 2,
        },
    ]
}

fn t15b_spring_cases() -> Vec<T15bCase> {
    let case = |name, corners: Vec<Point>, nets: Vec<i32>| T15bCase {
        name,
        corners,
        layer: 0,
        half_width: 30,
        nets,
        cl: 1,
    };
    vec![
        case(
            "free",
            vec![Point::new(4000, 4000), Point::new(5000, 4000)],
            vec![3],
        ),
        case(
            "throughPin",
            vec![Point::new(500, -600), Point::new(500, 600)],
            vec![3],
        ),
        case(
            "throughSmd",
            vec![Point::new(-500, -600), Point::new(-500, 600)],
            vec![3],
        ),
        case(
            "throughVia",
            vec![Point::new(2400, 1400), Point::new(2400, 2600)],
            vec![1],
        ),
        case(
            "acrossFixed",
            vec![Point::new(0, -1400), Point::new(0, -400)],
            vec![3],
        ),
        case(
            "acrossNet1",
            vec![Point::new(-300, -600), Point::new(-300, 600)],
            vec![3],
        ),
        case(
            "twoObstacles",
            vec![
                Point::new(-900, -600),
                Point::new(900, -600),
                Point::new(900, 600),
            ],
            vec![3],
        ),
        case(
            "offBoard",
            vec![Point::new(9800, 9800), Point::new(12000, 9800)],
            vec![3],
        ),
    ]
}

fn never_stop() -> bool {
    false
}


#[test]
fn spring_over_obstacles_agrees_with_the_jvm_on_every_probe_row() {
    let mut rows = Vec::new();
    for angle in T15B_REGIMES {
        for case in t15b_spring_cases() {
            for half_width in [case.half_width, 100] {
                for net_arr in [case.nets.clone(), vec![1], Vec::new()] {
                    for with_contact_pins in [false, true] {
                        let mut board = obstacles_board(angle);
                        let contact_pins: Option<std::collections::BTreeSet<ItemId>> =
                            with_contact_pins.then(|| {
                                board
                                    .get_items()
                                    .filter(|item| matches!(item, Item::Pin(_)))
                                    .map(Item::id)
                                    .collect()
                            });
                        let polyline = Polyline::from_points(&case.corners);
                        let result = TraceShover::spring_over_obstacles(
                            &mut board,
                            &polyline,
                            half_width,
                            case.layer,
                            &net_arr,
                            case.cl,
                            contact_pins.as_ref(),
                        );
                        let same_val = result.as_ref().is_some_and(|r| {
                            t15b_polyline(Some(r)) == t15b_polyline(Some(&polyline))
                        });
                        rows.push(format!(
                            "  regime={} case={} hw={} nets={} contactPins={} same={} sameVal={} -> {} {}",
                            t15b_regime_name(angle),
                            case.name,
                            half_width,
                            t15b_nets(&net_arr),
                            match &contact_pins {
                                None => "null".to_string(),
                                Some(pins) => pins.len().to_string(),
                            },
                            same_val,
                            same_val,
                            t15b_polyline(result.as_ref()),
                            t15b_failing(&board),
                        ));
                    }
                }
            }
        }
    }
    assert_rows_match("spring", &rows);
}


#[test]
fn spring_over_obstacles_stops_at_the_recursion_limit() {
    let mut rows = Vec::new();
    for via_count in [1, 2, 3, 4, 5, 6, 8, 10, 15, 18, 19, 20, 21, 22, 25] {
        let mut board = ladder_board(via_count);
        let along = Polyline::from_points(&[
            Point::new(-2600, LADDER_Y),
            Point::new(-2000 + 400 * via_count + 600, LADDER_Y),
        ]);
        let result = TraceShover::spring_over_obstacles(&mut board, &along, 30, 0, &[3], 1, None);
        let same_val = result
            .as_ref()
            .is_some_and(|r| t15b_polyline(Some(r)) == t15b_polyline(Some(&along)));
        rows.push(format!(
            "  ladder vias={} same={} sameVal={} lines={} len={} {}",
            via_count,
            same_val,
            same_val,
            result.as_ref().map_or(-1, |r| r.lines().len() as i64),
            result
                .as_ref()
                .map_or("null".to_string(), |r| java_round_half_up(
                    r.length_approx()
                )
                .to_string()),
            t15b_failing(&board),
        ));
    }
    for gap in [100, 140, 200, 400] {
        let mut board = overlap_board(gap);
        let across =
            Polyline::from_points(&[Point::new(-800, LADDER_Y), Point::new(800, LADDER_Y)]);
        let result = TraceShover::spring_over_obstacles(&mut board, &across, 30, 0, &[3], 1, None);
        let same_val = result
            .as_ref()
            .is_some_and(|r| t15b_polyline(Some(r)) == t15b_polyline(Some(&across)));
        rows.push(format!(
            "  overlap gap={} same={} sameVal={} lines={} {}",
            gap,
            same_val,
            same_val,
            result.as_ref().map_or(-1, |r| r.lines().len() as i64),
            t15b_failing(&board),
        ));
    }
    let expected: Vec<&str> = t15b_section("ladder")
        .into_iter()
        .filter(|line| !line.starts_with("  springOver "))
        .collect();
    assert_eq!(expected.len(), rows.len());
    let mut diffs = Vec::new();
    for (i, (want, got)) in expected.iter().zip(rows.iter()).enumerate() {
        if want != got {
            diffs.push(format!("row {i}\n  jvm:  {want}\n  rust: {got}"));
        }
    }
    assert!(diffs.is_empty(), "{}", diffs.join("\n"));
    assert!(expected[11].contains("vias=20 same=false sameVal=false lines=66"));
    assert!(
        expected[12].contains("vias=21 same=false sameVal=false lines=-1 len=null failing=Via#6")
    );
}

fn java_round_half_up(value: f64) -> i64 {
    (value + 0.5).floor() as i64
}


#[test]
fn insert_forced_trace_polyline_agrees_with_the_jvm_on_every_probe_row() {
    let mut rows = Vec::new();
    for angle in T15B_REGIMES {
        for case in t15b_cases() {
            for max_rec in [0, 20] {
                for with_check in [false, true] {
                    for tidy_width in [0, T15B_MAXV] {
                        let mut board = probe_board(angle);
                        let polyline = Polyline::from_points(&case.corners);
                        let result = board
                            .insert_forced_trace_polyline(
                                None,
                                &polyline,
                                case.half_width,
                                case.layer,
                                &case.nets,
                                case.cl,
                                max_rec,
                                max_rec,
                                max_rec,
                                tidy_width,
                                500,
                                with_check,
                                None,
                                &never_stop,
                            )
                            .expect("no stop check trips here");
                        let first = result.is_some() && result == polyline.first_corner();
                        let last = result.is_some() && result == polyline.last_corner();
                        rows.push(format!(
                            "  regime={} case={} maxRec={} withCheck={} tidy={} -> {} first={} last={} {}",
                            t15b_regime_name(angle),
                            case.name,
                            max_rec,
                            with_check,
                            if tidy_width == T15B_MAXV {
                                "MAX".to_string()
                            } else {
                                tidy_width.to_string()
                            },
                            t15b_answer(result.as_ref()),
                            first,
                            last,
                            t15b_failing(&board),
                        ));
                        rows.extend(t15b_board_dump(&board));
                    }
                }
            }
        }
    }
    assert_rows_match("poly", &rows);
}


#[test]
fn insert_forced_trace_polyline_pull_tightens_its_tail() {
    let mut rows = Vec::new();
    for angle in T15B_REGIMES {
        for case in t15b_cases() {
            for tidy_width in [0, 1, 400, T15B_MAXV] {
                for accuracy in [0, 100, 500, 5000] {
                    let mut board = probe_board(angle);
                    let polyline = Polyline::from_points(&case.corners);
                    let result = board
                        .insert_forced_trace_polyline(
                            None,
                            &polyline,
                            case.half_width,
                            case.layer,
                            &case.nets,
                            case.cl,
                            20,
                            20,
                            20,
                            tidy_width,
                            accuracy,
                            true,
                            None,
                            &never_stop,
                        )
                        .expect("no stop check trips here");
                    rows.push(format!(
                        "  regime={} case={} tidy={} acc={} -> {} {}",
                        t15b_regime_name(angle),
                        case.name,
                        if tidy_width == T15B_MAXV {
                            "MAX".to_string()
                        } else {
                            tidy_width.to_string()
                        },
                        accuracy,
                        t15b_answer(result.as_ref()),
                        t15b_failing(&board),
                    ));
                    rows.extend(t15b_board_dump(&board));
                }
            }
            for net_arr in [case.nets.clone(), vec![2]] {
                let mut board = probe_board(angle);
                let polyline = Polyline::from_points(&case.corners);
                let result = board
                    .insert_forced_trace_polyline(
                        None,
                        &polyline,
                        case.half_width,
                        case.layer,
                        &net_arr,
                        case.cl,
                        0,
                        0,
                        0,
                        T15B_MAXV,
                        500,
                        true,
                        None,
                        &never_stop,
                    )
                    .expect("no stop check trips here");
                rows.push(format!(
                    "  regime={} case={} optNet={} -> {} {}",
                    t15b_regime_name(angle),
                    case.name,
                    t15b_nets(&net_arr),
                    t15b_answer(result.as_ref()),
                    t15b_failing(&board),
                ));
                rows.extend(t15b_board_dump(&board));
            }
        }
    }
    assert_rows_match("tail", &rows);

    let mut tightened = probe_board(AngleRestriction::NinetyDegree);
    let mut untightened = probe_board(AngleRestriction::NinetyDegree);
    let polyline = Polyline::from_points(&[Point::new(-300, -600), Point::new(-300, 600)]);
    for (board, tidy_width) in [(&mut tightened, T15B_MAXV), (&mut untightened, 0)] {
        board
            .insert_forced_trace_polyline(
                None,
                &polyline,
                30,
                0,
                &[3],
                1,
                20,
                20,
                20,
                tidy_width,
                500,
                true,
                None,
                &never_stop,
            )
            .expect("no stop check trips here");
    }
    assert_ne!(
        t15b_board_dump(&tightened),
        t15b_board_dump(&untightened),
        "the `tidyWidth > 0` tail at RoutingBoard.java:860-862 must change the board"
    );
}


#[test]
fn insert_forced_trace_segment_agrees_with_the_jvm_on_every_probe_row() {
    let mut rows = Vec::new();
    for angle in T15B_REGIMES {
        for case in t15b_cases() {
            let from = case.corners[0].clone();
            let to = case.corners[case.corners.len() - 1].clone();
            for max_rec in [0, 20] {
                for tidy_width in [0, T15B_MAXV] {
                    let mut board = probe_board(angle);
                    let result = board
                        .insert_forced_trace_segment(
                            None,
                            &from,
                            &to,
                            case.half_width,
                            case.layer,
                            &case.nets,
                            case.cl,
                            max_rec,
                            max_rec,
                            max_rec,
                            tidy_width,
                            500,
                            true,
                            None,
                            &never_stop,
                        )
                        .expect("no stop check trips here");
                    let is_to = result.as_ref() == Some(&to);
                    let is_from = !is_to && result.as_ref() == Some(&from);
                    rows.push(format!(
                        "  regime={} case={} maxRec={} tidy={} -> {} isFrom={} isTo={} {}",
                        t15b_regime_name(angle),
                        case.name,
                        max_rec,
                        if tidy_width == T15B_MAXV {
                            "MAX".to_string()
                        } else {
                            tidy_width.to_string()
                        },
                        t15b_answer(result.as_ref()),
                        is_from,
                        is_to,
                        t15b_failing(&board),
                    ));
                    rows.extend(t15b_board_dump(&board));
                }
            }
        }
    }
    assert_rows_match("seg", &rows);
}


#[test]
fn insert_forced_trace_segment_necks_down_like_the_jvm() {
    let pairs: [(&str, [Point; 2]); 5] = [
        ("intoPin", [Point::new(500, -600), Point::new(500, 0)]),
        ("throughPin", [Point::new(500, -600), Point::new(500, 600)]),
        ("intoSmd", [Point::new(-500, -600), Point::new(-500, 0)]),
        (
            "acrossNet1",
            [Point::new(-300, -600), Point::new(-300, 600)],
        ),
        ("intoNet2", [Point::new(-800, 1400), Point::new(-800, 600)]),
    ];
    let mut rows = Vec::new();
    for angle in T15B_REGIMES {
        for (name, pair) in &pairs {
            for half_width in [1, 5, 10, 20, 30, 60, 100] {
                let mut board = probe_board(angle);
                let result = board
                    .insert_forced_trace_segment(
                        None,
                        &pair[0],
                        &pair[1],
                        half_width,
                        0,
                        &[3],
                        1,
                        20,
                        20,
                        20,
                        T15B_MAXV,
                        500,
                        true,
                        None,
                        &never_stop,
                    )
                    .expect("no stop check trips here");
                rows.push(format!(
                    "  regime={} case={} hw={} -> {} isTo={} {}",
                    t15b_regime_name(angle),
                    name,
                    half_width,
                    t15b_answer(result.as_ref()),
                    result.as_ref() == Some(&pair[1]),
                    t15b_failing(&board),
                ));
                rows.extend(t15b_board_dump(&board));
            }
        }
    }
    assert_rows_match("neck", &rows);

    for regime in ["45", "any"] {
        let wide = format!("  regime={regime} case=acrossNet1 hw=100 -> (-175,224) isTo=false");
        let narrow = format!("  regime={regime} case=acrossNet1 hw=60 -> (-300,600) isTo=true");
        assert!(
            rows.iter().any(|row| row.starts_with(&wide)),
            "the wide row is missing: {wide}"
        );
        assert!(
            rows.iter().any(|row| row.starts_with(&narrow)),
            "the narrow row is missing: {narrow}"
        );
    }
}


fn t15b_string_hash(text: &str) -> i32 {
    let mut hash: i32 = 0;
    for c in text.chars() {
        hash = hash.wrapping_mul(31).wrapping_add(c as i32);
    }
    hash
}

#[test]
fn the_two_random_blocks_agree_with_the_jvm() {
    let mut rnd = JavaRandom::new(4242);
    let mut rows = vec!["  block=polyline".to_string()];
    for row in 0..128 {
        let regime_no = rnd.next_int(3) as usize;
        let corner_count = 2 + rnd.next_int(3);
        let corners: Vec<Point> = (0..corner_count)
            .map(|_| Point::new(rnd.next_int(3001) - 1500, rnd.next_int(3001) - 1500))
            .collect();
        let half_width = 10 + rnd.next_int(60);
        let layer = rnd.next_int(2) as usize;
        let net = 1 + rnd.next_int(3);
        let cl = (1 + rnd.next_int(2)) as usize;
        let max_rec = rnd.next_int(3) * 10;
        let with_check = rnd.next_int(2) == 0;
        let tidy_width = if rnd.next_int(2) == 0 { 0 } else { T15B_MAXV };
        let accuracy = rnd.next_int(1000);
        let angle = T15B_REGIMES[regime_no];
        let mut board = probe_board(angle);
        let polyline = Polyline::from_points(&corners);
        let result = board
            .insert_forced_trace_polyline(
                None,
                &polyline,
                half_width,
                layer,
                &[net],
                cl,
                max_rec,
                max_rec,
                max_rec,
                tidy_width,
                accuracy,
                with_check,
                None,
                &never_stop,
            )
            .expect("no stop check trips here");
        let dump = t15b_board_dump(&board).join("\n");
        rows.push(format!(
            "  row={} regime={} n={} hw={} layer={} net={} cl={} maxRec={} check={} tidy={} acc={} -> {} maxId={} items={} hash={} {}",
            row,
            t15b_regime_name(angle),
            corner_count,
            half_width,
            layer,
            net,
            cl,
            max_rec,
            with_check,
            if tidy_width == T15B_MAXV { "MAX" } else { "0" },
            accuracy,
            t15b_answer(result.as_ref()),
            board.communication.id_gen.max_generated_id(),
            board.get_items().count(),
            t15b_string_hash(&dump),
            t15b_failing(&board),
        ));
    }
    rows.push("  block=segment".to_string());
    for row in 0..128 {
        let regime_no = rnd.next_int(3) as usize;
        let from = Point::new(rnd.next_int(3001) - 1500, rnd.next_int(3001) - 1500);
        let to = Point::new(rnd.next_int(3001) - 1500, rnd.next_int(3001) - 1500);
        let half_width = 10 + rnd.next_int(60);
        let layer = rnd.next_int(2) as usize;
        let net = 1 + rnd.next_int(3);
        let cl = (1 + rnd.next_int(2)) as usize;
        let max_rec = rnd.next_int(3) * 10;
        let with_check = rnd.next_int(2) == 0;
        let tidy_width = if rnd.next_int(2) == 0 { 0 } else { T15B_MAXV };
        let accuracy = rnd.next_int(1000);
        let angle = T15B_REGIMES[regime_no];
        let mut board = probe_board(angle);
        let result = board
            .insert_forced_trace_segment(
                None,
                &from,
                &to,
                half_width,
                layer,
                &[net],
                cl,
                max_rec,
                max_rec,
                max_rec,
                tidy_width,
                accuracy,
                with_check,
                None,
                &never_stop,
            )
            .expect("no stop check trips here");
        let is_to = result.as_ref() == Some(&to);
        let is_from = !is_to && result.as_ref() == Some(&from);
        let dump = t15b_board_dump(&board).join("\n");
        rows.push(format!(
            "  row={} regime={} from={} to={} hw={} layer={} net={} cl={} maxRec={} check={} tidy={} acc={} -> {} isFrom={} isTo={} maxId={} items={} hash={} {}",
            row,
            t15b_regime_name(angle),
            t15b_answer(Some(&from)),
            t15b_answer(Some(&to)),
            half_width,
            layer,
            net,
            cl,
            max_rec,
            with_check,
            if tidy_width == T15B_MAXV { "MAX" } else { "0" },
            accuracy,
            t15b_answer(result.as_ref()),
            is_from,
            is_to,
            board.communication.id_gen.max_generated_id(),
            board.get_items().count(),
            t15b_string_hash(&dump),
            t15b_failing(&board),
        ));
    }
    assert_rows_match("rand", &rows);
}


#[test]
fn the_shove_loop_entry_side_index_is_one_below_the_check_loops() {
    let mut rows = Vec::new();
    for angle in T15B_REGIMES {
        for case in t15b_cases() {
            let board = probe_board(angle);
            let polyline = Polyline::from_points(&case.corners);
            if polyline.lines().len() < 3 {
                rows.push(format!(
                    "  regime={} case={} empty=true",
                    t15b_regime_name(angle),
                    case.name
                ));
                continue;
            }
            let compensated_half_width = case.half_width
                + board.trees.get_default_tree().clearance_compensation_value(
                    case.cl,
                    case.layer,
                    &board.rules,
                );
            let combined = polyline;
            let trace_shapes = combined.offset_shapes_between(
                compensated_half_width,
                0,
                combined.lines().len() - 1,
            );
            let orthogonal_mode =
                board.rules.trace_angle_restriction == AngleRestriction::NinetyDegree;
            for (i, shape) in trace_shapes.iter().enumerate() {
                let shape = if orthogonal_mode {
                    TileShape::Box(shape.bounding_box())
                } else {
                    shape.clone()
                };
                let insert_index = combined.corner_count() - trace_shapes.len() - 1 + i;
                let check_index = i + 1;
                let insert_side = ShapeEntrySide::from_polyline(&combined, insert_index, &shape);
                let check_side = ShapeEntrySide::from_polyline(&combined, check_index, &shape);
                rows.push(format!(
                    "  regime={} case={} shape={} insertIndex={} insertNo={} checkIndex={} checkNo={} agree={}",
                    t15b_regime_name(angle),
                    case.name,
                    i,
                    insert_index,
                    insert_side.no,
                    check_index,
                    check_side.no,
                    insert_side.no == check_side.no,
                ));
            }
        }
    }
    assert_rows_match("side", &rows);
    assert!(
        rows.iter().all(|row| !row.contains("agree=false")),
        "the two indices answer the same entry side on every row of this fixture"
    );
}


#[test]
fn insert_stops_when_the_stop_check_trips() {
    let mut board = probe_board(AngleRestriction::None);
    let calls = std::cell::Cell::new(0u32);
    let stop = || {
        calls.set(calls.get() + 1);
        true
    };
    let polyline = Polyline::from_points(&[Point::new(-300, -600), Point::new(-300, 600)]);
    let result = board.insert_forced_trace_polyline(
        None,
        &polyline,
        30,
        0,
        &[3],
        1,
        20,
        20,
        20,
        T15B_MAXV,
        500,
        true,
        None,
        &stop,
    );
    assert!(
        matches!(result, Err(BoardError::Stopped)),
        "expected Err(Stopped), got {result:?}"
    );
    assert!(calls.get() > 0, "the stop check was never consulted");
}
