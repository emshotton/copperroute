use copper_board::items::{Item, ItemCtx};
use copper_board::prelude::*;
use copper_geometry::{
    Area, IntBox, IntVector, Line, Point, PolygonShape, Polyline, PolylineShapeRef, Shape,
    TileShape, Vector,
};

struct Fixture {
    board: Board,
    smd_pad: PadstackId,
    thru_pad: PadstackId,
}

fn fixture() -> Fixture {
    let ls = LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)]);
    let clearance_matrix = ClearanceMatrix::get_default_instance(&ls, 200);
    let mut rules = BoardRules::new(ls.clone(), clearance_matrix);
    rules.create_default_net_class();
    let default_class = rules.get_default_net_class();

    let mut padstacks = Padstacks::new(ls.clone());
    let smd_pad = padstacks.add(
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
    let thru_shape = Shape::Tile(TileShape::Box(IntBox::from_coords(-70, -70, 70, 70)));
    let thru_pad = padstacks.add(
        "thru",
        vec![Some(thru_shape.clone()), Some(thru_shape)],
        true,
        false,
    );
    let mut packages = Packages::new();
    let pkg = packages.add(
        "pkg",
        vec![
            PackagePin::new("P1", smd_pad, IntVector::new(-1000, 0).into(), 0.0),
            PackagePin::new("P2", thru_pad, IntVector::new(1000, 1000).into(), 0.0),
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
    components.add_with_generated_name(Some(Point::new(0, 0)), 0.0, true, pkg);

    let outline = vec![PolylineShapeRef::Polygon(PolygonShape::from_points(&[
        Point::new(-5000, -5000),
        Point::new(5000, -5000),
        Point::new(5000, 5000),
        Point::new(-5000, 5000),
    ]))];
    let mut board = Board::new(
        outline,
        1,
        IntBox::from_coords(-10_000, -10_000, 10_000, 10_000),
        rules,
        BoardLibrary::new(padstacks, packages),
        components,
        Communication::default(),
    );
    board.rules.nets.add("N1", 1, false, default_class);
    board.rules.nets.add("N2", 1, false, default_class);

    Fixture {
        board,
        smd_pad,
        thru_pad,
    }
}

fn box_area(x1: i32, y1: i32, x2: i32, y2: i32) -> Area {
    Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
        x1, y1, x2, y2,
    ))))
}

fn straight_trace(board: &mut Board, layer: usize, x1: i32, x2: i32, net_no: i32) -> ItemId {
    board
        .insert_trace_without_cleaning(
            Polyline::from_points(&[Point::new(x1, 100), Point::new(x2, 100)]),
            layer,
            30,
            vec![net_no],
            1,
            FixedState::Unfixed,
        )
        .expect("a straight two-corner trace")
}

#[test]
fn trace_free_boards_with_different_items_no_longer_collide() {
    let a = fixture().board;
    let mut b = fixture().board;
    b.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    let mut c = fixture().board;
    c.insert_obstacle(box_area(2000, 2000, 3000, 3000), 0, 1, FixedState::Unfixed);

    assert!(a.get_traces().is_empty() && a.get_vias().is_empty());
    assert!(b.get_traces().is_empty() && b.get_vias().is_empty());
    assert!(c.get_traces().is_empty() && c.get_vias().is_empty());

    assert_ne!(a.structural_hash(), b.structural_hash());
    assert_ne!(a.structural_hash(), c.structural_hash());
    assert_ne!(b.structural_hash(), c.structural_hash());
}

#[test]
fn the_item_id_reaches_the_hash() {
    let mut a = fixture().board;
    straight_trace(&mut a, 0, 100, 500, 1);

    let mut b = fixture().board;
    let burnt = straight_trace(&mut b, 0, 100, 500, 1);
    b.remove_item(burnt);
    straight_trace(&mut b, 0, 100, 500, 1);

    assert_eq!(a.get_traces().len(), b.get_traces().len());
    assert_ne!(a.get_traces()[0], b.get_traces()[0], "different ids");
    assert_ne!(a.structural_hash(), b.structural_hash());
}

#[test]
fn the_net_numbers_reach_the_hash() {
    let mut a = fixture().board;
    straight_trace(&mut a, 0, 100, 500, 1);
    let mut b = fixture().board;
    straight_trace(&mut b, 0, 100, 500, 2);
    assert_ne!(a.structural_hash(), b.structural_hash());
}

#[test]
fn the_clearance_class_reaches_the_hash() {
    let mut a = fixture().board;
    let trace = straight_trace(&mut a, 0, 100, 500, 1);
    let before = a.structural_hash();
    let rules = a.rules.clone();
    a.get_item_mut(trace)
        .expect("the trace")
        .header_mut()
        .set_clearance_class(0, &rules);
    assert_ne!(a.structural_hash(), before);
}

#[test]
fn the_fixed_state_reaches_the_hash() {
    let mut a = fixture().board;
    let trace = straight_trace(&mut a, 0, 100, 500, 1);
    let before = a.structural_hash();
    a.get_item_mut(trace)
        .expect("the trace")
        .header_mut()
        .set_fixed_state(FixedState::UserFixed);
    assert_ne!(a.structural_hash(), before);
}

#[test]
fn the_pin_index_reaches_the_hash() {
    let mut a = fixture().board;
    a.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    let mut b = fixture().board;
    b.insert_pin(1, 1, vec![1], 1, FixedState::Unfixed);
    assert_ne!(a.structural_hash(), b.structural_hash());
}

#[test]
fn the_component_id_reaches_the_hash() {
    let mut a = fixture().board;
    let obstacle = a.insert_obstacle(box_area(2000, 2000, 3000, 3000), 0, 1, FixedState::Unfixed);
    let before = a.structural_hash();
    a.get_item_mut(obstacle)
        .expect("the obstacle")
        .header_mut()
        .assign_component_id(7);
    assert_ne!(a.structural_hash(), before);
}

#[test]
fn the_on_the_board_flag_reaches_the_hash() {
    let mut a = fixture().board;
    let trace = straight_trace(&mut a, 0, 100, 500, 1);
    let before = a.structural_hash();
    a.get_item_mut(trace)
        .expect("the trace")
        .header_mut()
        .set_on_the_board(false);
    assert_ne!(a.structural_hash(), before);
}

#[test]
fn the_trace_layer_reaches_the_hash() {
    let mut a = fixture().board;
    straight_trace(&mut a, 0, 100, 500, 1);
    let mut b = fixture().board;
    straight_trace(&mut b, 1, 100, 500, 1);
    assert_ne!(a.structural_hash(), b.structural_hash());
}

#[test]
fn the_trace_half_width_reaches_the_hash() {
    let mut a = fixture().board;
    a.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(100, 100), Point::new(500, 100)]),
        0,
        30,
        vec![1],
        1,
        FixedState::Unfixed,
    );
    let mut b = fixture().board;
    b.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(100, 100), Point::new(500, 100)]),
        0,
        31,
        vec![1],
        1,
        FixedState::Unfixed,
    );
    assert_ne!(a.structural_hash(), b.structural_hash());
}

#[test]
fn two_polylines_with_equal_corners_but_different_lines_hash_differently() {
    let long_lines = vec![
        Line::from_coords(-1000, 100, 1000, 100),
        Line::from_coords(0, 0, 0, 1000),
        Line::from_coords(-1000, 900, 1000, 900),
    ];
    let short_lines = vec![
        Line::from_coords(-500, 100, 500, 100),
        Line::from_coords(0, 0, 0, 400),
        Line::from_coords(-500, 900, 500, 900),
    ];
    let long = Polyline::from_lines(long_lines).expect("three non-parallel consecutive lines");
    let short = Polyline::from_lines(short_lines).expect("three non-parallel consecutive lines");
    assert_eq!(long.corners(), short.corners(), "same corners");
    assert_ne!(long.lines(), short.lines(), "different defining lines");

    let mut a = fixture().board;
    a.insert_trace_without_cleaning(long, 0, 30, vec![1], 1, FixedState::Unfixed)
        .expect("a trace");
    let mut b = fixture().board;
    b.insert_trace_without_cleaning(short, 0, 30, vec![1], 1, FixedState::Unfixed)
        .expect("a trace");

    assert_ne!(a.structural_hash(), b.structural_hash());
}

#[test]
fn the_line_identity_token_does_not_reach_the_hash() {
    let original = Polyline::from_points(&[
        Point::new(100, 100),
        Point::new(500, 100),
        Point::new(500, 900),
    ]);
    let reminted: Vec<Line> = original
        .lines()
        .iter()
        .map(|line| Line::new(line.a, line.b))
        .collect();
    assert!(
        original
            .lines()
            .iter()
            .zip(&reminted)
            .all(|(old, new)| !old.is_same_object(new)),
        "every line must carry a fresh identity token"
    );
    let copy = Polyline::from_lines(reminted).expect("the same lines");

    let mut a = fixture().board;
    a.insert_trace_without_cleaning(original, 0, 30, vec![1], 1, FixedState::Unfixed)
        .expect("a trace");
    let mut b = fixture().board;
    b.insert_trace_without_cleaning(copy, 0, 30, vec![1], 1, FixedState::Unfixed)
        .expect("a trace");

    assert_eq!(a.structural_hash(), b.structural_hash());
}

#[test]
fn the_via_centre_and_padstack_reach_the_hash() {
    let mut fa = fixture();
    fa.board
        .insert_via(
            fa.thru_pad,
            Point::new(0, 0),
            vec![1],
            1,
            FixedState::Unfixed,
            true,
        )
        .expect("a via");
    let mut fb = fixture();
    fb.board
        .insert_via(
            fb.thru_pad,
            Point::new(10, 0),
            vec![1],
            1,
            FixedState::Unfixed,
            true,
        )
        .expect("a via");
    assert_ne!(fa.board.structural_hash(), fb.board.structural_hash());

    let mut fc = fixture();
    fc.board
        .insert_via(
            fc.smd_pad,
            Point::new(0, 0),
            vec![1],
            1,
            FixedState::Unfixed,
            true,
        )
        .expect("a via");
    assert_ne!(fa.board.structural_hash(), fc.board.structural_hash());
}

#[test]
fn the_via_attach_allowed_flag_reaches_the_hash() {
    let mut fa = fixture();
    fa.board
        .insert_via(
            fa.thru_pad,
            Point::new(0, 0),
            vec![1],
            1,
            FixedState::Unfixed,
            true,
        )
        .expect("a via");
    let mut fb = fixture();
    fb.board
        .insert_via(
            fb.thru_pad,
            Point::new(0, 0),
            vec![1],
            1,
            FixedState::Unfixed,
            false,
        )
        .expect("a via");
    assert_ne!(fa.board.structural_hash(), fb.board.structural_hash());
}

#[test]
fn the_via_escape_flags_reach_the_hash() {
    let mut fa = fixture();
    let via = fa
        .board
        .insert_via(
            fa.thru_pad,
            Point::new(0, 0),
            vec![1],
            1,
            FixedState::Unfixed,
            true,
        )
        .expect("a via");
    let before = fa.board.structural_hash();
    let Some(Item::Via(v)) = fa.board.get_item_mut(via) else {
        panic!("the via");
    };
    v.is_escape_via = true;
    let after_escape = fa.board.structural_hash();
    assert_ne!(after_escape, before);

    let Some(Item::Via(v)) = fa.board.get_item_mut(via) else {
        panic!("the via");
    };
    v.escape_via_smd_layer = 1;
    assert_ne!(fa.board.structural_hash(), after_escape);
}

#[test]
fn the_obstacle_area_geometry_and_layer_reach_the_hash() {
    let mut a = fixture().board;
    a.insert_obstacle(box_area(2000, 2000, 3000, 3000), 0, 1, FixedState::Unfixed);
    let mut b = fixture().board;
    b.insert_obstacle(box_area(2000, 2000, 3001, 3000), 0, 1, FixedState::Unfixed);
    let mut c = fixture().board;
    c.insert_obstacle(box_area(2000, 2000, 3000, 3000), 1, 1, FixedState::Unfixed);

    assert_ne!(a.structural_hash(), b.structural_hash(), "relativeArea");
    assert_ne!(a.structural_hash(), c.structural_hash(), "layer");
}

#[test]
fn the_obstacle_area_placement_fields_reach_the_hash() {
    let make = |translation: Vector, rotation: f64, side_changed: bool, name: Option<String>| {
        let mut board = fixture().board;
        board.insert_obstacle_of_component(
            box_area(2000, 2000, 3000, 3000),
            0,
            translation,
            rotation,
            side_changed,
            1,
            1,
            name,
            FixedState::Unfixed,
        );
        board.structural_hash()
    };
    let base = make(Vector::ZERO, 0.0, false, Some("keepout".to_string()));
    assert_ne!(
        base,
        make(
            IntVector::new(1, 0).into(),
            0.0,
            false,
            Some("keepout".into())
        ),
        "translation"
    );
    assert_ne!(
        base,
        make(Vector::ZERO, 90.0, false, Some("keepout".into())),
        "rotationInDegree"
    );
    assert_ne!(
        base,
        make(Vector::ZERO, 0.0, true, Some("keepout".into())),
        "sideChanged"
    );
    assert_ne!(
        base,
        make(Vector::ZERO, 0.0, false, Some("other".into())),
        "name"
    );
    assert_ne!(base, make(Vector::ZERO, 0.0, false, None), "a null name");
}

#[test]
fn the_four_obstacle_area_kinds_are_distinguishable() {
    let area = || box_area(2000, 2000, 3000, 3000);
    let mut a = fixture().board;
    a.insert_obstacle(area(), 0, 1, FixedState::Unfixed);
    let mut b = fixture().board;
    b.insert_via_obstacle(area(), 0, 1, FixedState::Unfixed);
    let mut c = fixture().board;
    c.insert_component_obstacle(area(), 0, 1, FixedState::Unfixed);
    let mut d = fixture().board;
    d.insert_conduction_area(area(), 0, Vec::new(), 1, true, FixedState::Unfixed);

    let hashes = [
        a.structural_hash(),
        b.structural_hash(),
        c.structural_hash(),
        d.structural_hash(),
    ];
    for i in 0..hashes.len() {
        for j in i + 1..hashes.len() {
            assert_ne!(hashes[i], hashes[j], "kinds {i} and {j} collide");
        }
    }
}

#[test]
fn the_conduction_area_flags_reach_the_hash() {
    let mut a = fixture().board;
    let area = a.insert_conduction_area(
        box_area(-3000, -3000, -2000, -2000),
        0,
        vec![2],
        1,
        true,
        FixedState::Unfixed,
    );
    let before = a.structural_hash();
    let Some(Item::ConductionArea(c)) = a.get_item_mut(area) else {
        panic!("the conduction area");
    };
    c.set_is_obstacle(false);
    let after = a.structural_hash();
    assert_ne!(after, before, "isObstacle");

    let Some(Item::ConductionArea(c)) = a.get_item_mut(area) else {
        panic!("the conduction area");
    };
    c.set_is_filled(false);
    assert_ne!(a.structural_hash(), after, "isFilled");
}

#[test]
fn the_component_outline_fields_reach_the_hash() {
    let make = |is_front: bool,
                translation: Vector,
                rotation: f64,
                courtyard: bool,
                fabrication: bool,
                closed: bool| {
        let mut board = fixture().board;
        board
            .insert_component_outline(
                box_area(-500, -500, 500, 500),
                is_front,
                translation,
                rotation,
                1,
                courtyard,
                fabrication,
                closed,
                FixedState::Unfixed,
            )
            .expect("a bounded area");
        board.structural_hash()
    };
    let base = make(true, Vector::ZERO, 0.0, false, false, true);
    assert_ne!(
        base,
        make(false, Vector::ZERO, 0.0, false, false, true),
        "isFront"
    );
    assert_ne!(
        base,
        make(true, IntVector::new(1, 0).into(), 0.0, false, false, true),
        "translation"
    );
    assert_ne!(
        base,
        make(true, Vector::ZERO, 90.0, false, false, true),
        "rotationInDegree"
    );
    assert_ne!(
        base,
        make(true, Vector::ZERO, 0.0, true, false, true),
        "isCourtyard"
    );
    assert_ne!(
        base,
        make(true, Vector::ZERO, 0.0, false, true, true),
        "isFabrication"
    );
    assert_ne!(
        base,
        make(true, Vector::ZERO, 0.0, false, false, false),
        "isClosed"
    );
}

#[test]
fn the_board_outline_shapes_and_keepout_flag_reach_the_hash() {
    let mut a = fixture().board;
    a.insert_outline(
        vec![PolylineShapeRef::Tile(TileShape::Box(IntBox::from_coords(
            -100, -100, 100, 100,
        )))],
        1,
    );
    let mut b = fixture().board;
    b.insert_outline(
        vec![PolylineShapeRef::Tile(TileShape::Box(IntBox::from_coords(
            -100, -100, 101, 100,
        )))],
        1,
    );
    assert_ne!(a.structural_hash(), b.structural_hash(), "shapes");

    let mut c = fixture().board;
    let outline = c.get_outline().expect("the constructor inserted one");
    let before = c.structural_hash();
    let Some(Item::BoardOutline(o)) = c.get_item_mut(outline) else {
        panic!("the outline");
    };
    o.generate_keepout_outside(true);
    assert_ne!(c.structural_hash(), before, "keepoutOutsideOutline");
}

#[test]
fn reordering_the_item_list_changes_the_hash() {
    let mut a = fixture().board;
    straight_trace(&mut a, 0, 100, 500, 1);
    straight_trace(&mut a, 1, 600, 900, 2);

    let mut b = fixture().board;
    straight_trace(&mut b, 1, 600, 900, 2);
    straight_trace(&mut b, 0, 100, 500, 1);

    assert_eq!(a.get_traces().len(), b.get_traces().len());
    assert_ne!(a.structural_hash(), b.structural_hash());
}

#[test]
fn filling_the_pin_centre_cache_does_not_move_the_hash() {
    let mut fx = fixture();
    let pin = fx.board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    let before = fx.board.structural_hash();

    let ctx: ItemCtx<'_> = fx.board.ctx();
    let Some(Item::Pin(p)) = fx.board.get_item(pin) else {
        panic!("the pin");
    };
    let centre = p.get_center(&ctx);
    assert_eq!(centre, Point::new(-1000, 0));

    assert_eq!(fx.board.structural_hash(), before);
}

#[test]
fn the_smallest_clearance_by_product_does_not_move_the_hash() {
    let mut a = fixture().board;
    let trace = straight_trace(&mut a, 0, 100, 500, 1);
    let before = a.structural_hash();
    a.get_item_mut(trace)
        .expect("the trace")
        .header_mut()
        .smallest_clearance = 42.0;
    assert_eq!(a.structural_hash(), before);
}

#[test]
fn filling_the_via_layer_caches_does_not_move_the_hash() {
    let mut fx = fixture();
    fx.board
        .insert_via(
            fx.thru_pad,
            Point::new(0, 0),
            vec![1],
            1,
            FixedState::Unfixed,
            true,
        )
        .expect("a via");
    let before = fx.board.structural_hash();

    let ctx: ItemCtx<'_> = fx.board.ctx();
    for id in fx.board.get_vias() {
        let Some(Item::Via(v)) = fx.board.get_item(id) else {
            panic!("a via");
        };
        v.first_layer(&ctx);
        v.last_layer(&ctx);
    }

    assert_eq!(fx.board.structural_hash(), before);
}

#[test]
fn diff_traces_is_the_symmetric_difference() {
    let mut a = fixture().board;
    let shared = straight_trace(&mut a, 0, 100, 500, 1);
    let only_a = straight_trace(&mut a, 0, 600, 900, 1);

    let mut b = a.clone();
    b.remove_item(only_a);
    let only_b = straight_trace(&mut b, 1, 1000, 1400, 2);

    assert_eq!(a.diff_traces(&a), 0, "a board against itself");
    assert_eq!(a.diff_traces(&b), 2, "one each way");
    assert_eq!(b.diff_traces(&a), 2, "symmetric");

    assert!(a.get_traces().contains(&shared) && b.get_traces().contains(&shared));
    assert!(a.get_traces().contains(&only_a) && !b.get_traces().contains(&only_a));
    assert!(b.get_traces().contains(&only_b) && !a.get_traces().contains(&only_b));

    let mut c = a.clone();
    c.remove_item(only_a);
    assert_eq!(a.diff_traces(&c), 1);
    assert_eq!(c.diff_traces(&a), 1);

    let ids = a.get_traces();
    let unique: std::collections::BTreeSet<ItemId> = ids.iter().copied().collect();
    assert_eq!(ids.len(), unique.len());
}
