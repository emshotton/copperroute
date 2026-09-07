//! A pour-splitting reroute must not survive the optimizer: the static plane rule still calls
//! every pad inside the pour connected, so only the refilled-copper check can catch it.
use fr_board::prelude::*;
use fr_drc::PlaneConnectivity;
use fr_geometry::{
    Area, Circle, IntBox, IntPoint, Point, Polyline, PolylineShapeRef, Shape, TileShape,
};
use fr_router::pipeline::{BatchOptimizer, NoopProgressSink, RouterBudget, RouterStop, TaskState};
use fr_settings::sources::DefaultSettings;
use fr_settings::{HostEnvironment, RouterSettings, SettingsSource};

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

struct Pad {
    at: (i32, i32),
    through_hole: bool,
}

const PADS: &[Pad] = &[
    Pad {
        at: (30_000, 30_000),
        through_hole: true,
    },
    Pad {
        at: (0, -42_000),
        through_hole: false,
    },
    Pad {
        at: (-46_000, -30_000),
        through_hole: false,
    },
    Pad {
        at: (46_000, -30_000),
        through_hole: false,
    },
];

const GROUND_THROUGH_HOLE: i32 = 0;
const GROUND_TOP_ONLY: i32 = 1;
const SIGNAL_LEFT: i32 = 2;
const SIGNAL_RIGHT: i32 = 3;

fn layers() -> LayerStructure {
    LayerStructure::new(vec![Layer::new("F.Cu", true), Layer::new("B.Cu", true)])
}

fn board() -> Board {
    let ls = layers();
    let mut padstacks = Padstacks::new(ls.clone());
    let mut pins = Vec::new();
    for (index, pad) in PADS.iter().enumerate() {
        let shape = Shape::Tile(TileShape::Box(IntBox::from_coords(
            -PAD_HALF, -PAD_HALF, PAD_HALF, PAD_HALF,
        )));
        let shapes = if pad.through_hole {
            vec![Some(shape.clone()), Some(shape)]
        } else {
            vec![Some(shape), None]
        };
        let name = format!("pad{index}");
        let padstack = padstacks.add(&name, shapes, pad.through_hole, false);
        pins.push(PackagePin::new(
            name,
            padstack,
            Point::new(pad.at.0, pad.at.1).difference_by(&Point::ZERO),
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
    board.insert_pin(1, GROUND_THROUGH_HOLE, vec![GND], 1, FixedState::Unfixed);
    board.insert_pin(1, GROUND_TOP_ONLY, vec![GND], 1, FixedState::Unfixed);
    board.insert_pin(1, SIGNAL_LEFT, vec![SIGNAL], 1, FixedState::Unfixed);
    board.insert_pin(1, SIGNAL_RIGHT, vec![SIGNAL], 1, FixedState::Unfixed);

    let detour = [
        PADS[SIGNAL_LEFT as usize].at,
        (-46_000, -47_000),
        (46_000, -47_000),
        PADS[SIGNAL_RIGHT as usize].at,
    ];
    let corners: Vec<Point> = detour.iter().map(|(x, y)| Point::new(*x, *y)).collect();
    board
        .insert_trace_without_cleaning(
            Polyline::from_points(&corners),
            0,
            HALF_WIDTH,
            vec![SIGNAL],
            1,
            FixedState::Unfixed,
        )
        .expect("the detour has four distinct corners");
    board
}

fn settings(board: &Board) -> RouterSettings {
    let mut settings = DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    settings.set_layer_count(board.get_layer_count());
    settings.apply_board_specific_optimizations(board);
    let fanout = settings.fanout.get_or_insert_with(Default::default);
    fanout.enabled = Some(false);
    settings.set_run_router(true);
    settings.set_run_optimizer(true);
    settings.save_intermediate_stages = Some(false);
    settings
}

#[test]
fn the_optimizer_keeps_a_detour_whose_shortcut_would_cut_a_pad_off_the_pour() {
    let mut board = board();
    let before = PlaneConnectivity::of(&board);
    assert_eq!(
        before.cluster_count(GND),
        Some(1),
        "the detour leaves the pour whole"
    );
    let detour_length = board.cumulative_trace_length();

    let settings = settings(&board);
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    let result = BatchOptimizer::new(&settings)
        .run_batch_loop(&mut board, &stop, RouterBudget::disabled(), &mut sink)
        .expect("the optimizer runs");

    assert_eq!(result.state, TaskState::Finished);
    let after = PlaneConnectivity::of(&board);
    assert_eq!(
        after.cluster_count(GND),
        Some(1),
        "the shortcut along y = -30000 walls the top-only ground pad off the front pour"
    );
    assert!(
        board.cumulative_trace_length() >= detour_length - 1.0 || board.get_vias().len() >= 2,
        "the signal either keeps its detour or changes layer to spare the pour"
    );
}
