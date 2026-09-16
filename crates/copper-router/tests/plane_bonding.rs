//! A pad inside a pour's outline is not connected to the plane if the refill strands it on an
//! island of its own. The static plane rule cannot tell the two apart, so the router used to
//! drop such a pad from the pass entirely.
use copper_board::prelude::*;
use copper_drc::PlaneConnectivity;
use copper_geometry::{
    Area, Circle, IntBox, IntPoint, Point, Polyline, PolylineShapeRef, Shape, TileShape,
};
use copper_router::pipeline::{BatchAutorouter, RouterBudget};
use copper_settings::sources::DefaultSettings;
use copper_settings::{HostEnvironment, RouterSettings, SettingsSource};

const GND: i32 = 1;
const SIGNAL: i32 = 2;
const CLEARANCE: i32 = 2000;
const HALF_WIDTH: i32 = 1250;
const PAD_HALF: i32 = 1500;

const BOUNDING_BOX: IntBox = IntBox {
    ll: IntPoint {
        x: -100_000,
        y: -100_000,
    },
    ur: IntPoint {
        x: 100_000,
        y: 100_000,
    },
};

const CAGED: usize = 0;
const OPEN: usize = 1;

const PADS: &[(i32, i32)] = &[(0, 0), (30_000, 30_000), (-30_000, 30_000)];

/// A ring of signal track close enough around the caged pad that the pour's clearance from it
/// severs the copper inside the ring from the rest of the pour.
const CAGE: i32 = 5_000;

fn board() -> Board {
    let ls = LayerStructure::new(vec![Layer::new("F.Cu", true), Layer::new("B.Cu", true)]);
    let mut padstacks = Padstacks::new(ls.clone());
    let mut pins = Vec::new();
    for (index, at) in PADS.iter().enumerate() {
        let shape = Shape::Tile(TileShape::Box(IntBox::from_coords(
            -PAD_HALF, -PAD_HALF, PAD_HALF, PAD_HALF,
        )));
        let name = format!("pad{index}");
        let padstack = padstacks.add(&name, vec![Some(shape.clone()), Some(shape)], true, false);
        pins.push(PackagePin::new(
            name,
            padstack,
            Point::new(at.0, at.1).difference_by(&Point::ZERO),
            0.0,
        ));
    }
    let via_shape = Shape::Circle(Circle::new(IntPoint::new(0, 0), 3000));
    let via = padstacks.add(
        "Via[0-1]_600:300_um",
        vec![Some(via_shape.clone()), Some(via_shape)],
        true,
        false,
    );
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

    let matrix = ClearanceMatrix::get_default_instance(&ls, CLEARANCE);
    let mut rules = BoardRules::new(ls, matrix);
    rules.create_default_net_class();
    rules.set_default_trace_half_widths(HALF_WIDTH);
    let default_class = rules.get_default_net_class();
    rules.via_infos.add(ViaInfo::new("via", via, 1, false));
    let mut via_rule = ViaRule::new("default");
    via_rule.append_via(rules.via_infos.get(ViaInfoId(0)).clone());
    rules.via_rules.push(via_rule);
    rules
        .net_classes
        .get_mut(default_class)
        .set_via_rule(Some(rules.via_rules[0].clone()));

    let mut board = Board::new(
        Vec::new(),
        1,
        BOUNDING_BOX,
        rules,
        BoardLibrary::new(padstacks, packages),
        components,
        Communication::default(),
    );
    board.rules.nets.add("GND", 1, true, default_class);
    board.rules.nets.add("SIG", 1, false, default_class);
    board.insert_outline(
        vec![PolylineShapeRef::Tile(TileShape::Box(IntBox::from_coords(
            -50_000, -50_000, 50_000, 50_000,
        )))],
        1,
    );
    for layer in [0, 1] {
        board.insert_conduction_area(
            Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
                -48_000, -48_000, 48_000, 48_000,
            )))),
            layer,
            vec![GND],
            1,
            false,
            FixedState::SystemFixed,
        );
    }
    for index in 0..PADS.len() {
        board.insert_pin(1, index as i32, vec![GND], 1, FixedState::Unfixed);
    }
    board
}

fn cage(board: &mut Board) {
    let sides = [
        [(-CAGE, -CAGE), (-CAGE, CAGE)],
        [(-CAGE, CAGE), (CAGE, CAGE)],
        [(CAGE, CAGE), (CAGE, -CAGE)],
        [(CAGE, -CAGE), (-CAGE, -CAGE)],
    ];
    for layer in [0, 1] {
        for side in &sides {
            let corners: Vec<Point> = side.iter().map(|(x, y)| Point::new(*x, *y)).collect();
            board
                .insert_trace_without_cleaning(
                    Polyline::from_points(&corners),
                    layer,
                    HALF_WIDTH,
                    vec![SIGNAL],
                    1,
                    FixedState::Unfixed,
                )
                .expect("each side has two distinct corners");
        }
    }
}

fn settings(board: &Board) -> RouterSettings {
    let mut settings = DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    settings.set_layer_count(board.get_layer_count());
    settings.apply_board_specific_optimizations(board);
    settings
}

fn pin_of(board: &Board, index: usize) -> ItemId {
    board
        .get_connectable_items(GND)
        .into_iter()
        .find(|id| {
            matches!(board.get_item(*id), Some(Item::Pin(pin))
                if pin.get_pin_index() == index as i32)
        })
        .expect("the ground pin is on the board")
}

#[test]
fn an_uncaged_pad_is_not_stranded() {
    let board = board();

    let connectivity = PlaneConnectivity::of(&board);

    assert!(connectivity.stranded_items().is_empty());
    assert_eq!(connectivity.cluster_count(GND), Some(1));
}

#[test]
fn a_caged_pad_is_stranded_inside_the_pour() {
    let mut board = board();
    cage(&mut board);
    let caged = pin_of(&board, CAGED);

    let connectivity = PlaneConnectivity::of(&board);

    assert!(
        connectivity.stranded_items().contains(&caged),
        "the refill leaves the caged pad on an island of its own"
    );
    assert!(
        !connectivity
            .stranded_items()
            .contains(&pin_of(&board, OPEN))
    );
}

#[test]
fn the_pass_queues_a_pad_the_pour_does_not_reach() {
    let mut board = board();
    cage(&mut board);
    let caged = pin_of(&board, CAGED);
    let settings = settings(&board);

    let mut router = BatchAutorouter::new(
        &board,
        &settings,
        false,
        true,
        100,
        500,
        RouterBudget::disabled(),
    );

    let queued_on_the_static_rule: Vec<ItemId> = router
        .autoroute_items(&board)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert!(
        !queued_on_the_static_rule.contains(&caged),
        "the pour's outline covers the pad, so the static rule calls it connected"
    );

    router.refresh_plane_bonding(&board);
    let queued: Vec<ItemId> = router
        .autoroute_items(&board)
        .into_iter()
        .map(|(id, _)| id)
        .collect();

    assert!(
        queued.contains(&caged),
        "the refill model says the pad is off the pour, so the pass must route it"
    );
}
