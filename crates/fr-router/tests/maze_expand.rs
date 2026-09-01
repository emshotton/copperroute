//! Plan 6 Task 12: `MazeSearchEngine`'s room-door expansion and cost model
//! (`autoroute/maze/MazeSearchEngine.java:390-966`, `:1105-1215`) and the check-only
//! `MazeTraceShover` (`autoroute/maze/MazeTraceShover.java:32-314`).
//!
//! # Where the numbers come from
//!
//! Every literal below — the expansion and sorting values, the bend threshold, the door ids, the
//! `roomShapeIsThick` boundary, the neckdown half width and every `checkShoveTraceLine` answer —
//! is **read off the HEAD jar**. The probe is
//! `scripts/differential/java/probes/P6T12Probe.java`, committed with the exact `javac`/`java`
//! invocation in its header; its whole stdout is committed as
//! `tests/data/p6t12-maze-expand.txt`. Each test names its probe mode and pastes the lines it
//! asserts against.
//!
//! # The fixture, and the two switches it throws
//!
//! The board is Task 11's — `P6T11Probe.build`'s two-pin board with the two traces replaced by an
//! obstacle box (see `tests/maze_search.rs` for why the traces had to go). Two control fields are
//! set by hand, exactly as the probe sets them:
//!
//! * `viasAllowed = false`, because the drill-page block of `MazeSearchEngine.java:601-623` calls
//!   `MazeExpansionEngine`, which is Task 13's;
//! * `bendCosts[0] = 100.0` in the cost-model tests, because `RouterSettings` answers `0.0` for
//!   this board and the whole of `:856-873` would be dead.
//!
//! `ripupAllowed` is already `false` (AutorouteControl.java:184), which is what keeps
//! `MazeRipupResolver` — also Task 13's — out of the way.

#![allow(clippy::too_many_lines)]

use std::cell::Cell;
use std::collections::BTreeSet;

use fr_board::ids::{ItemId, ObstacleRoomId};
use fr_board::prelude::*;
use fr_board::rules::ViaRule;
use fr_geometry::{
    Area, FloatLine, FloatPoint, IntBox, IntOctagon, IntPoint, IntVector, Point, Polyline, Shape,
    TileShape,
};
use fr_router::arena::DoorId;
use fr_router::autoroute::expansion::{ExpandableRef, RoomRef};
use fr_router::autoroute::maze::engine::AutorouteEngine;
use fr_router::autoroute::maze::search::MazeSearchEngine;
use fr_router::autoroute::maze::trace_shover::{DoorSection, MazeTraceShover};
use fr_router::autoroute::maze::{AutorouteControl, MazeAdjustment, MazeListElement};
use fr_settings::RouterSettings;

// =================================================================================================
// The probe's board, rebuilt from scratch (`P6T11Probe.build`, shared with `tests/maze_search.rs`)
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

/// `P6T11Probe.build`: two layers, a 200-unit clearance matrix with a "wide" class, an empty via
/// rule on the default net class (`AutorouteControl.rebuildViaInfo:235` dereferences it), two
/// declared nets (quirk #173), a two-pin component — an SMD pad at (-500, 0) on layer 0 and a
/// through pad at (500, 0) on both layers, both on net 1 — and one obstacle box.
fn probe_board() -> Board {
    let layers = || LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)]);
    let mut clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
    assert!(clearance_matrix.append_class("wide"));
    clearance_matrix.set_value_on_all_layers(2, 1, 600);
    clearance_matrix.set_value_on_all_layers(2, 2, 800);
    let mut rules = BoardRules::new(layers(), clearance_matrix);
    rules.trace_angle_restriction = AngleRestriction::None;
    rules.via_rules.push(ViaRule::new("empty"));
    let default_class = rules.get_default_net_class();
    rules
        .net_classes
        .get_mut(default_class)
        .set_via_rule(Some(rules.via_rules[0].clone()));
    rules.nets.add("N1", 1, false, default_class);
    rules.nets.add("N2", 1, false, default_class);

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
        rules,
        BoardLibrary::new(padstacks, packages),
        components,
        Communication::default(),
    );
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    board.insert_pin(1, 1, vec![1], 1, FixedState::Unfixed);
    board.insert_obstacle(
        Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            700, -1000, 900, 1000,
        )))),
        0,
        1,
        FixedState::Unfixed,
    );
    board
}

/// `new RouterSettings(board)` (RouterSettings.java:127-131).
fn probe_settings(board: &Board) -> RouterSettings {
    let mut settings = RouterSettings::new();
    settings.set_layer_count(board.get_layer_count());
    settings.apply_board_specific_optimizations(board);
    settings
}

/// `P6T11Probe.control`.
fn probe_control(board: &Board, net_no: i32) -> AutorouteControl {
    let settings = probe_settings(board);
    let trace_costs = settings.get_trace_costs();
    AutorouteControl::new(
        board,
        net_no,
        &settings,
        settings.get_via_costs(),
        &trace_costs,
    )
}

/// `P6T11Probe.engine`: `new AutorouteEngine(board, 1, false)` then `initConnection(net, …)`.
fn probe_engine(board: &mut Board, net_no: i32) -> AutorouteEngine {
    let mut engine = AutorouteEngine::new(board, 1, false);
    engine.init_connection(board, net_no, None);
    engine
}

/// A `StopCheck` that never fires — every test here is past `occupyNextElement`'s only check.
struct Counter {
    calls: Cell<u32>,
}

impl Counter {
    fn new() -> Counter {
        Counter {
            calls: Cell::new(0),
        }
    }

    fn check(&self) -> bool {
        self.calls.set(self.calls.get() + 1);
        false
    }
}

/// `[ItemId(2), …]` — the probe's `setOf`, which is a `TreeSet<Item>`.
fn set_of(ids: &[u32]) -> BTreeSet<ItemId> {
    ids.iter().map(|id| ItemId(*id)).collect()
}

/// One queue row, in the shape `P6T11Probe.dumpQueue` prints it:
/// `(door id, section, expansionValue, sortingValue, next-room id, shape entry a, shape entry b,
/// roomRipped, adjustment, alreadyChecked, ripupCost)`.
type QueueRow = (
    i32,
    i32,
    f64,
    f64,
    Option<i32>,
    (f64, f64),
    (f64, f64),
    bool,
    MazeAdjustment,
    bool,
    i32,
);

/// `P6T11Probe.dumpQueue` prints `expansionValue`/`sortingValue` with `%.9f` and every
/// `FloatPoint` with `%.6f`, so the transcript pins those values to nine and six decimals. The
/// assertions below are made at exactly that precision — anything finer would be asserting digits
/// the ground truth does not contain.
fn r9(x: f64) -> f64 {
    (x * 1e9).round() / 1e9
}

fn r6(p: &FloatPoint) -> (f64, f64) {
    ((p.x * 1e6).round() / 1e6, (p.y * 1e6).round() / 1e6)
}

fn queue_rows(maze: &MazeSearchEngine<'_>) -> Vec<QueueRow> {
    maze.queue
        .iter()
        .map(|e| {
            (
                maze.engine.expandable_id_no(e.door),
                e.section_no_of_door,
                r9(e.expansion_value),
                r9(e.sorting_value),
                e.next_room
                    .and_then(|room| maze.engine.rooms.room_id_no(room)),
                r6(&e.shape_entry.a),
                r6(&e.shape_entry.b),
                e.room_ripped,
                e.adjustment,
                e.already_checked,
                e.ripup_cost,
            )
        })
        .collect()
}

/// `P6T12Probe`'s control: the shared one with `viasAllowed` switched off.
fn expand_control(board: &Board, net_no: i32) -> AutorouteControl {
    let mut ctrl = probe_control(board, net_no);
    ctrl.vias_allowed = false;
    ctrl
}

/// Drain the queue, so a hand-driven `expandToDoorSection` case starts from an empty one — the
/// probe's `maze.mazeExpansionList.clear()`.
fn drain(maze: &mut MazeSearchEngine<'_>) {
    while maze.queue.pop_first().is_some() {}
}

/// `P6T12Probe.doorAt`: two complete free-space rooms meeting on the vertical segment `x`,
/// `y ± 100`, and the 1-dimensional door between them — so the door's
/// `getShape().centreOfGravity()` is `(x, y)` and `getSectionSegments` allocates exactly one
/// section.
fn door_at(maze: &mut MazeSearchEngine<'_>, x: i32, y: i32, id1: i32, id2: i32) -> DoorId {
    let a = maze.engine.rooms.new_complete_room(
        Some(TileShape::Box(IntBox::from_coords(
            x - 500,
            y - 100,
            x,
            y + 100,
        ))),
        0,
        id1,
    );
    let b = maze.engine.rooms.new_complete_room(
        Some(TileShape::Box(IntBox::from_coords(
            x,
            y - 100,
            x + 500,
            y + 100,
        ))),
        0,
        id2,
    );
    maze.engine
        .rooms
        .new_door(RoomRef::Complete(a), RoomRef::Complete(b), 1)
}

// =================================================================================================
// The pop loop's tail — the first test that runs `expandToRoomDoors` for real
// =================================================================================================

/// Probe mode `pop`. `init` seeds two elements; the first `occupyNextElement` pops door 64 and
/// runs the whole of `expandToRoomDoors` on room 2 — `completeNeighbourRooms` (room count 3 → 4),
/// `expandToTargetDoors` (the destination door 95), and `expandToDoor` over the **post-completion**
/// door list (66, 67, 33).
/// ```text
/// --- after pop 1 occupyNextElement=true
/// completeRooms n=4
/// queue n=5
///   [0] 66  expansion=0.000000000     sorting=830.000000000    nextRoom=4
///   [1] 95  expansion=1000.000000000  sorting=1000.000000000   nextRoom=null
///   [2] 66  expansion=1550.000000000  sorting=3930.000000000   nextRoom=4
///   [3] 67  expansion=11149.268182262 sorting=20379.268182262  nextRoom=5
///   [4] 33  expansion=10846.197490365 sorting=21737.317726594  nextRoom=1
/// ```
/// Rows [0] and [2] share the id 66 by coincidence: the first is the `TargetItemExpansionDoor`
/// `31 * item(2) + room(4)`, the second the `ExpansionDoor` `min(2,4) * 31 + max(2,4)`.
#[test]
fn the_first_pop_expands_the_start_room_through_every_door_of_it() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = expand_control(&board, 1);
    let counter = Counter::new();
    let mut maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut engine,
        &mut board,
        &ctrl,
        &|| counter.check(),
    )
    .expect("init succeeds on this board");

    assert_eq!(maze.queue.len(), 2, "init seeded two elements");
    assert_eq!(maze.engine.complete_expansion_rooms().len(), 3);

    assert!(maze.occupy_next_element(&mut board, &|| counter.check()));
    assert_eq!(
        maze.engine.complete_expansion_rooms().len(),
        4,
        "completeNeighbourRooms added room 5"
    );

    assert_eq!(
        queue_rows(&maze),
        vec![
            (
                66,
                0,
                0.0,
                830.0,
                Some(4),
                (-500.0, 0.0),
                (-500.0, 0.0),
                false,
                MazeAdjustment::None,
                false,
                0
            ),
            (
                95,
                0,
                1000.0,
                1000.0,
                None,
                (500.0, 0.0),
                (500.0, 0.0),
                false,
                MazeAdjustment::None,
                false,
                0
            ),
            (
                66,
                0,
                1550.0,
                3930.0,
                Some(4),
                (-3098.0, 0.0),
                (-1002.0, 0.0),
                false,
                MazeAdjustment::None,
                false,
                0
            ),
            (
                67,
                0,
                11_149.268_182_262,
                20_379.268_182_262,
                Some(5),
                (-3_884.326_137, -8_621.203_369),
                (-215.673_863, -2_419.796_631),
                false,
                MazeAdjustment::None,
                false,
                0
            ),
            (
                33,
                0,
                10_846.197_490_365,
                21_737.317_726_594,
                Some(1),
                (-4700.0, -8398.0),
                (-4700.0, -1602.0),
                false,
                MazeAdjustment::None,
                false,
                0
            ),
        ]
    );
    assert_eq!(maze.destination_door(), None);
}

/// Probe mode `pop2`. The second pop takes door 66's element, whose room 4 is only 1041 units tall
/// — `nextRoomShape.minWidth() < 2 * halfWidth` (`:458`), so `nextRoomIsThick` is **false** and
/// every candidate is refused: the destination target door by `checkForcedTracePolyline` (`:690`),
/// the doors by `segmentProjection` or the small-door `continue` of `:743-748`. The queue simply
/// loses its head.
/// ```text
/// --- after pop 2 occupyNextElement=true
/// completeRooms n=5
/// queue n=4   (rows [1]..[4] of the previous dump, unchanged)
/// ```
#[test]
fn the_second_pop_into_a_thin_room_expands_nothing() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = expand_control(&board, 1);
    let counter = Counter::new();
    let mut maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut engine,
        &mut board,
        &ctrl,
        &|| counter.check(),
    )
    .expect("init succeeds on this board");

    assert!(maze.occupy_next_element(&mut board, &|| counter.check()));
    let after_first = queue_rows(&maze);
    assert!(maze.occupy_next_element(&mut board, &|| counter.check()));
    assert_eq!(
        maze.engine.complete_expansion_rooms().len(),
        5,
        "room 4's neighbours were completed in their turn"
    );
    assert_eq!(
        queue_rows(&maze),
        after_first[1..].to_vec(),
        "nothing was added, only the head removed"
    );
}

/// Probe mode `inactive`. `:396-401`: an inactive **signal** layer answers `true` — "the from door
/// section has to be occupied" — with nothing expanded at all, before `completeNeighbourRooms` even
/// runs.
/// ```text
/// layer0isSignal=true
/// expandToRoomDoors=true
/// queue n=2   (the two seeded elements, untouched)
/// ```
#[test]
fn an_inactive_layer_is_never_expanded_into() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let mut ctrl = expand_control(&board, 1);
    let counter = Counter::new();
    assert!(
        board.layer_structure().layers[0].is_signal,
        "layer0isSignal=true"
    );
    ctrl.layer_active[0] = false;
    let mut maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut engine,
        &mut board,
        &ctrl,
        &|| counter.check(),
    )
    .expect("init succeeds on this board");
    let before = queue_rows(&maze);
    let first = maze.queue.iter().next().expect("two seeded").clone();
    assert!(maze.expand_to_room_doors(&mut board, &first));
    assert_eq!(queue_rows(&maze), before);
    assert_eq!(maze.engine.complete_expansion_rooms().len(), 3);
}

// =================================================================================================
// The cost model (`expandToDoorSection`, :791-966)
// =================================================================================================

/// Build the probe's `bend` fixture: a room on layer 0 for the `from` element, a backtrack door
/// whose shape is centred on the origin, and a fresh `to` door centred on `(2000, dy)` whose one
/// section is already allocated. Returns `(from element, to door)`.
fn bend_fixture(
    maze: &mut MazeSearchEngine<'_>,
    dy: i32,
    room_ripped_parent: bool,
) -> (MazeListElement, DoorId) {
    let from_room = maze.engine.rooms.new_complete_room(
        Some(TileShape::Box(IntBox::from_coords(
            -5000, -5000, 5000, 5000,
        ))),
        0,
        900,
    );
    let backtrack_door = door_at(maze, 0, 0, 901, 902);
    let to_door = door_at(maze, 2000, dy, 903, 904);
    assert_eq!(
        maze.engine
            .rooms
            .door_section_segments(to_door, 1600.0)
            .len(),
        1,
        "sections=1"
    );
    let from = MazeListElement {
        door: ExpandableRef::Door(backtrack_door),
        section_no_of_door: 0,
        backtrack_door: Some(ExpandableRef::Door(backtrack_door)),
        section_no_of_backtrack_door: 0,
        expansion_value: 0.0,
        sorting_value: 0.0,
        next_room: Some(RoomRef::Complete(from_room)),
        shape_entry: FloatLine::new(FloatPoint::new(1000.0, 0.0), FloatPoint::new(1000.0, 0.0)),
        room_ripped: room_ripped_parent,
        adjustment: MazeAdjustment::None,
        already_checked: room_ripped_parent,
        ripup_cost: 0,
    };
    (from, to_door)
}

/// Probe mode `bend`. `:856-873` charges `ctrl.bendCosts[layer]` when the cross product of the
/// incoming and outgoing directions satisfies `crossProduct² > 0.01 · |prev|² · |next|²`, i.e.
/// `sin² > 0.01`. With `prev = (1000, 0)` and `next = (1000, dy)` that reduces to `99·dy² > 1000²`,
/// so the threshold sits between `dy = 100` and `dy = 101` — and the four rows below straddle it
/// by one unit:
/// ```text
/// bendCosts[0]=100.0    traceCosts[0] horizontal=1.0 vertical=2.0    backtrackCog=(0.000000,0.000000)
/// dy=0    expansion=1000.000000000  (= √(1000² + 0²)      , no bend)
/// dy=100  expansion=1019.803902719  (= √(1000² + 200²)    , no bend)
/// dy=101  expansion=1120.198019994  (= √(1000² + 202²) + 100)
/// dy=1000 expansion=2336.067977500  (= √(1000² + 2000²) + 100)
/// ```
/// The `vertical = 2.0` cost is why `dy` is doubled inside the root: the distance is
/// `FloatPoint.weightedDistance`, not the Euclidean one.
#[test]
fn the_bend_penalty_fires_exactly_above_sin_squared_one_percent() {
    for (dy, expected_expansion, expected_sorting, bend_charged) in [
        (0, 1000.0, 2330.0, false),
        (100, 1_019.803_902_719, 2_349.803_902_719, false),
        (101, 1_120.198_019_994, 2_450.198_019_994, true),
        (1000, 2_336.067_977_5, 4_463.155_187_748, true),
    ] {
        let mut board = probe_board();
        let mut engine = probe_engine(&mut board, 1);
        let mut ctrl = probe_control(&board, 1);
        // The board's `RouterSettings` answer `bendCosts = [0.0, 0.0]`, which would make the whole
        // of `:856-873` dead; the probe sets the same field.
        ctrl.bend_costs[0] = 100.0;
        let counter = Counter::new();
        let mut maze = MazeSearchEngine::get_instance(
            &set_of(&[2]),
            &set_of(&[3]),
            &mut engine,
            &mut board,
            &ctrl,
            &|| counter.check(),
        )
        .expect("init succeeds on this board");
        drain(&mut maze);

        let (from, to_door) = bend_fixture(&mut maze, dy, false);
        let shape_entry = FloatLine::new(
            FloatPoint::new(2000.0, f64::from(dy)),
            FloatPoint::new(2000.0, f64::from(dy)),
        );
        assert!(maze.expand_to_door_section(
            &mut board,
            ExpandableRef::Door(to_door),
            0,
            Some(&shape_entry),
            &from,
            0,
            MazeAdjustment::None,
        ));
        assert_eq!(
            queue_rows(&maze),
            vec![(
                28_897,
                0,
                expected_expansion,
                expected_sorting,
                None,
                (2000.0, f64::from(dy)),
                (2000.0, f64::from(dy)),
                false,
                MazeAdjustment::None,
                false,
                0
            )],
            "dy={dy}"
        );
        // The unweighted straight-line term, so the assertion above cannot pass by accident.
        let straight = (1000.0f64 * 1000.0 + 4.0 * f64::from(dy) * f64::from(dy)).sqrt();
        let charged = r9(expected_expansion) - r9((straight * 1e9).round() / 1e9);
        assert!(
            (charged - if bend_charged { 100.0 } else { 0.0 }).abs() < 1e-6,
            "dy={dy}: bend penalty {charged}"
        );
    }
}

/// Probe mode `bend`, the `addCosts` × `adjustment` matrix. `:885-887` and `:904-906`:
/// `roomRipped` and `ripupCost` are set only by a **positive `addCosts` with `Adjustment.NONE`**;
/// a `LEFT`/`RIGHT` adjustment carries the cost into the values but leaves both flags clear. The
/// second clause of `:887` — `fromElement.alreadyChecked && fromElement.roomRipped` — sets
/// `roomRipped` and leaves `ripupCost` at 0.
/// ```text
/// dy=0 addCosts=0 adjustment=NONE  => expansion=1000.0 roomRipped=false ripupCost=0
/// dy=0 addCosts=7 adjustment=NONE  => expansion=1007.0 roomRipped=true  ripupCost=7
/// dy=0 addCosts=7 adjustment=RIGHT => expansion=1007.0 roomRipped=false ripupCost=0
/// dy=0 addCosts=7 adjustment=LEFT  => expansion=1007.0 roomRipped=false ripupCost=0
/// expandToDoorSection rippedParent => expansion=1000.0 roomRipped=true  ripupCost=0
/// ```
#[test]
fn room_ripped_and_the_ripup_cost_follow_add_costs_and_the_adjustment() {
    let cases: [(i32, MazeAdjustment, f64, bool, i32); 4] = [
        (0, MazeAdjustment::None, 1000.0, false, 0),
        (7, MazeAdjustment::None, 1007.0, true, 7),
        (7, MazeAdjustment::Right, 1007.0, false, 0),
        (7, MazeAdjustment::Left, 1007.0, false, 0),
    ];
    for (add_costs, adjustment, expansion, room_ripped, ripup_cost) in cases {
        let mut board = probe_board();
        let mut engine = probe_engine(&mut board, 1);
        let mut ctrl = probe_control(&board, 1);
        ctrl.bend_costs[0] = 100.0;
        let counter = Counter::new();
        let mut maze = MazeSearchEngine::get_instance(
            &set_of(&[2]),
            &set_of(&[3]),
            &mut engine,
            &mut board,
            &ctrl,
            &|| counter.check(),
        )
        .expect("init succeeds on this board");
        drain(&mut maze);
        let (from, to_door) = bend_fixture(&mut maze, 0, false);
        let shape_entry =
            FloatLine::new(FloatPoint::new(2000.0, 0.0), FloatPoint::new(2000.0, 0.0));
        assert!(maze.expand_to_door_section(
            &mut board,
            ExpandableRef::Door(to_door),
            0,
            Some(&shape_entry),
            &from,
            add_costs,
            adjustment,
        ));
        assert_eq!(
            queue_rows(&maze),
            vec![(
                28_897,
                0,
                expansion,
                expansion + 1330.0,
                None,
                (2000.0, 0.0),
                (2000.0, 0.0),
                room_ripped,
                adjustment,
                false,
                ripup_cost
            )],
            "addCosts={add_costs} adjustment={adjustment:?}"
        );
    }
}

/// Probe mode `bend`, the `rippedParent` row: `:887`'s second clause propagates `roomRipped` from
/// an **already-checked** parent and leaves `ripupCost` at 0, because no ripup was paid here.
/// ```text
/// expandToDoorSection rippedParent => true
///   [0] door=ExpansionDoor id=28961 … expansion=1000.000000000 sorting=2330.000000000 roomRipped=true … ripupCost=0
/// ```
#[test]
fn an_already_checked_ripped_parent_propagates_room_ripped_but_not_the_cost() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let mut ctrl = probe_control(&board, 1);
    ctrl.bend_costs[0] = 100.0;
    let counter = Counter::new();
    let mut maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut engine,
        &mut board,
        &ctrl,
        &|| counter.check(),
    )
    .expect("init succeeds on this board");
    drain(&mut maze);
    let (from, to_door) = bend_fixture(&mut maze, 0, true);
    let shape_entry = FloatLine::new(FloatPoint::new(2000.0, 0.0), FloatPoint::new(2000.0, 0.0));
    assert!(maze.expand_to_door_section(
        &mut board,
        ExpandableRef::Door(to_door),
        0,
        Some(&shape_entry),
        &from,
        0,
        MazeAdjustment::None,
    ));
    assert_eq!(
        queue_rows(&maze),
        vec![(
            28_897,
            0,
            1000.0,
            2330.0,
            None,
            (2000.0, 0.0),
            (2000.0, 0.0),
            true,
            MazeAdjustment::None,
            false,
            0
        )]
    );
}

/// Probe mode `bend`, the tail. `:798-849`: an already-occupied section and a `null` shape entry
/// are both refused, and neither touches the queue.
/// ```text
/// expandToDoorSection occupied  => false
/// expandToDoorSection nullEntry => false
/// queue n=0
/// ```
#[test]
fn an_occupied_section_and_a_null_shape_entry_are_both_refused() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let counter = Counter::new();
    let mut maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut engine,
        &mut board,
        &ctrl,
        &|| counter.check(),
    )
    .expect("init succeeds on this board");
    drain(&mut maze);
    let (from, to_door) = bend_fixture(&mut maze, 0, true);
    let shape_entry = FloatLine::new(FloatPoint::new(2000.0, 0.0), FloatPoint::new(2000.0, 0.0));

    maze.engine
        .rooms
        .door_mut(to_door)
        .expect("live door")
        .get_maze_search_element_mut(0)
        .expect("one section")
        .is_occupied = true;
    assert!(!maze.expand_to_door_section(
        &mut board,
        ExpandableRef::Door(to_door),
        0,
        Some(&shape_entry),
        &from,
        0,
        MazeAdjustment::None,
    ));
    maze.engine
        .rooms
        .door_mut(to_door)
        .expect("live door")
        .get_maze_search_element_mut(0)
        .expect("one section")
        .is_occupied = false;
    assert!(!maze.expand_to_door_section(
        &mut board,
        ExpandableRef::Door(to_door),
        0,
        None,
        &from,
        0,
        MazeAdjustment::None,
    ));
    assert_eq!(maze.queue.len(), 0);
}

// =================================================================================================
// roomShapeIsThick (:1105-1123) and checkNeckDownAtDestPin (:1207-1215)
// =================================================================================================

/// Probe mode `thick`. `:1122` compares `traceHalfWidth + clearanceCompensationValue` against
/// `ctrl.compensatedTraceHalfWidth[layer]`, and the compensation is **the trace's own**, not the
/// control's — which is why a trace of half width 1500 is already "thick" against a compensated
/// 1600. The two non-trace, non-via arms take `:1118-1121`'s `obstacleHalfWidth = 0` and answer
/// false.
/// ```text
/// compensatedTraceHalfWidth[0]=1600   clearanceCompensationValue(1,0)=100
/// obstacleArea => false
/// pin => false
/// trace id=8 halfWidth=2000 roomShapeIsThick=true
/// trace id=7 halfWidth=1600 roomShapeIsThick=true
/// trace id=6 halfWidth=1500 roomShapeIsThick=true
/// trace id=5 halfWidth=100  roomShapeIsThick=false
/// ```
#[test]
fn room_shape_is_thick_at_the_compensated_half_width_boundary() {
    let mut board = probe_board();
    {
        let mut engine = probe_engine(&mut board, 1);
        let ctrl = probe_control(&board, 1);
        let maze = MazeSearchEngine::new(&mut engine, &ctrl);
        assert_eq!(ctrl.compensated_trace_half_width[0], 1600);
        let tree = maze.search_tree;
        drop(maze);
        // The obstacle area (item 4) is neither a trace nor a via, and the through pin (item 3) is
        // a `DrillItem` but not a `Via`: both take the `FRLogger.warn` arm.
        for item in [ItemId(4), ItemId(3)] {
            let room = engine.rooms.new_obstacle_room(&mut board, item, 0, tree);
            let maze = MazeSearchEngine::new(&mut engine, &ctrl);
            assert!(
                !maze.room_shape_is_thick(&board, room),
                "item {item:?} takes the unexpected-obstacle arm"
            );
            drop(maze);
        }
    }

    for half_width in [100, 1500, 1600, 2000] {
        board.insert_trace_without_cleaning(
            Polyline::from_points(&[
                Point::new(-9000, 5000 + half_width),
                Point::new(-3000, 5000 + half_width),
            ]),
            0,
            half_width,
            vec![2],
            1,
            FixedState::Unfixed,
        );
    }
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let tree = engine.tree;
    // `getItems()` is descending id, which is the order the probe prints.
    let mut rows = Vec::new();
    for id in board.items_in_board_order() {
        if !matches!(board.items.get(&id), Some(Item::Trace(_))) {
            continue;
        }
        let room = engine.rooms.new_obstacle_room(&mut board, id, 0, tree);
        let maze = MazeSearchEngine::new(&mut engine, &ctrl);
        let half_width = match board.items.get(&id) {
            Some(Item::Trace(trace)) => trace.get_half_width(),
            _ => unreachable!(),
        };
        rows.push((id.0, half_width, maze.room_shape_is_thick(&board, room)));
        drop(maze);
    }
    assert_eq!(
        rows,
        vec![
            (8, 2000, true),
            (7, 1600, true),
            (6, 1500, true),
            (5, 100, false),
        ]
    );
    let _ = tree;
}

/// Probe mode `neck`, and quirk #179. `checkNeckDownAtDestPin`'s name and javadoc both say
/// *destination* pin, and its loop never asks: it answers the neckdown half width of the **first**
/// target door whose item is a `Pin`, and `return`s from inside the loop. Room 2's target doors are
/// `(item 2 = the start pin, item 3 = the destination pin)`, and the answer is the *start* pin's.
/// ```text
/// room=2 layer=0 targetDoors=2 checkNeckDownAtDestPin=49.0
/// room=4 layer=0 targetDoors=2 checkNeckDownAtDestPin=49.0
/// bareRoom checkNeckDownAtDestPin=0.0
/// ```
#[test]
fn check_neck_down_at_dest_pin_answers_the_first_pin_target_door_whichever_it_is() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let counter = Counter::new();
    let maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut engine,
        &mut board,
        &ctrl,
        &|| counter.check(),
    )
    .expect("init succeeds on this board");

    let rooms: Vec<RoomRef> = maze
        .queue
        .iter()
        .map(|e| e.next_room.expect("a seeded element has a room"))
        .collect();
    let answers: Vec<(i32, f64)> = rooms
        .iter()
        .map(|room| {
            (
                maze.engine.rooms.room_id_no(*room).expect("a live room"),
                maze.check_neck_down_at_dest_pin(&board, *room),
            )
        })
        .collect();
    assert_eq!(answers, vec![(2, 49.0), (4, 49.0)]);
    // The first target door of room 2 is item **2**, the start pin — so this is not the
    // destination pin's answer, whatever the method is called.
    let first_target_item = {
        let door = maze.engine.rooms.room_target_doors(rooms[0])[0];
        maze.engine.rooms.target_door(door).expect("live").item
    };
    assert_eq!(first_target_item, ItemId(2));

    let bare = maze.engine.rooms.new_complete_room(
        Some(TileShape::Box(IntBox::from_coords(0, 0, 10, 10))),
        0,
        950,
    );
    assert_eq!(
        maze.check_neck_down_at_dest_pin(&board, RoomRef::Complete(bare)),
        0.0
    );
}

// =================================================================================================
// The small door, the snapshot and the stale tree entry
// =================================================================================================

/// Probe mode `smalldoor`. Entering through an **`ExpansionDoor`** rather than a target door makes
/// `:405` bite and `doorIsSmall` (`:415`) decide the round. At the control's own half width the
/// door is big enough and four elements are expanded; at an absurd half width the door is small,
/// `:501-511` returns `somethingExpanded` — still `false`, because `expandToTargetDoors` refused at
/// `:634-646` too — and nothing is expanded.
/// ```text
/// room=2 door=33 dimension=1 doorShape=[-4700,-10000..-4700,0] shapeEntry=(-4700.000000,-5000.000000)
/// halfWidth=1600   doorIsSmall=false expandToRoomDoors=true    queue n=4  (95, 64, 67, 66)
/// halfWidth=100000 doorIsSmall=true  expandToRoomDoors=false   queue n=0
/// ```
#[test]
fn a_small_door_refuses_the_whole_round() {
    for (half_width, door_is_small, expanded) in [(1600, false, true), (100_000, true, false)] {
        let mut board = probe_board();
        let mut engine = probe_engine(&mut board, 1);
        let mut ctrl = expand_control(&board, 1);
        // The probe sets this **after** `getInstance`; `init` never reads
        // `compensatedTraceHalfWidth`, so setting it first leaves the seeded queue identical —
        // which the assertion below checks before anything else happens.
        ctrl.compensated_trace_half_width[0] = half_width;
        let counter = Counter::new();
        let mut maze = MazeSearchEngine::get_instance(
            &set_of(&[2]),
            &set_of(&[3]),
            &mut engine,
            &mut board,
            &ctrl,
            &|| counter.check(),
        )
        .expect("init succeeds on this board");
        assert_eq!(maze.queue.len(), 2, "init is unaffected by the half width");

        let seed = maze.queue.iter().next().expect("two seeded").clone();
        let room = seed.next_room.expect("a seeded element has a room");
        let door = maze.engine.rooms.room_doors(room)[0];
        assert_eq!(maze.engine.rooms.door_id_no(door), Some(33));
        assert_eq!(
            maze.engine.rooms.door(door).expect("live").dimension,
            1,
            "dimension=1"
        );
        let door_shape = maze.engine.rooms.door_shape(door).expect("live");
        let b = door_shape.bounding_box();
        assert_eq!((b.ll.x, b.ll.y, b.ur.x, b.ur.y), (-4700, -10_000, -4700, 0));
        let centre = door_shape.centre_of_gravity();
        assert_eq!(r6(&centre), (-4700.0, -5000.0));

        assert_eq!(
            maze.door_is_small(&board, door, 2.0 * (f64::from(half_width) + 2.0)),
            door_is_small
        );

        let element = MazeListElement {
            door: ExpandableRef::Door(door),
            section_no_of_door: 0,
            backtrack_door: None,
            section_no_of_backtrack_door: 0,
            expansion_value: 0.0,
            sorting_value: 0.0,
            next_room: Some(room),
            shape_entry: FloatLine::new(centre, centre),
            room_ripped: false,
            adjustment: MazeAdjustment::None,
            already_checked: false,
            ripup_cost: 0,
        };
        drain(&mut maze);
        assert_eq!(maze.expand_to_room_doors(&mut board, &element), expanded);
        let ids: Vec<i32> = maze
            .queue
            .iter()
            .map(|e| maze.engine.expandable_id_no(e.door))
            .collect();
        if expanded {
            assert_eq!(ids, vec![95, 64, 67, 66]);
        } else {
            assert!(ids.is_empty(), "a small door expands nothing");
        }
    }
}

/// Probe mode `snapshot`, and a **correction to the task brief**. The brief asks for
/// "`the_door_snapshot_is_taken_before_completing_neighbours`"; Java takes it at `:559`, which is
/// *after* `completeNeighbourRooms` (`:419`). The distinction is observable: completing room 2's
/// neighbours **removes one door and adds another**, and the round below expands through the
/// post-completion list — door 67, which did not exist when the pop began, is visited, and the
/// door that completion dropped is not.
/// ```text
/// room=2 doorsBefore=4
/// doorsAfterCompletion=3
///   door[0] id=33 dimension=1 inSnapshot=true  shape=[-4700,-10000..-4700,0]
///   door[1] id=66 dimension=1 inSnapshot=true  shape=[-4700,0..600,0]
///   door[2] id=67 dimension=1 inSnapshot=false shape=[-4700,-10000..600,-1041]
/// ```
#[test]
fn the_door_list_is_snapshotted_after_completing_neighbours() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = expand_control(&board, 1);
    let counter = Counter::new();
    let maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut engine,
        &mut board,
        &ctrl,
        &|| counter.check(),
    )
    .expect("init succeeds on this board");
    let room = maze
        .queue
        .iter()
        .next()
        .expect("two seeded")
        .next_room
        .expect("a room");
    let before: Vec<DoorId> = maze.engine.rooms.room_doors(room).to_vec();
    assert_eq!(before.len(), 4, "doorsBefore=4");

    maze.engine.complete_neighbour_rooms(&mut board, room);
    let after: Vec<DoorId> = maze.engine.rooms.room_doors(room).to_vec();
    assert_eq!(after.len(), 3, "doorsAfterCompletion=3");
    /// `(door id, dimension, was in the pre-completion list, bounding box)`.
    type DoorRow = (i32, i32, bool, (i32, i32, i32, i32));
    let rows: Vec<DoorRow> = after
        .iter()
        .map(|door| {
            let d = maze.engine.rooms.door(*door).expect("live");
            let b = maze
                .engine
                .rooms
                .door_shape(*door)
                .expect("live")
                .bounding_box();
            (
                maze.engine.rooms.door_id_no(*door).expect("live"),
                d.dimension,
                before.contains(door),
                (b.ll.x, b.ll.y, b.ur.x, b.ur.y),
            )
        })
        .collect();
    assert_eq!(
        rows,
        vec![
            (33, 1, true, (-4700, -10_000, -4700, 0)),
            (66, 1, true, (-4700, 0, 600, 0)),
            (67, 1, false, (-4700, -10_000, 600, -1041)),
        ]
    );
    // And probe mode `pop` shows the round expanding 66, 67 and 33 — the post-completion list.
}

/// Probe mode `stale`. `expandToTargetDoors`' guard at `:656-658` compares the door's
/// `treeEntryNo` against the item's **current** `treeShapeCount`, and skips silently when the
/// trace or pad has been rebuilt underneath it. With a healthy fixture one destination door is
/// expanded; with every `treeEntryNo` forced to 99 the method answers false and the queue stays
/// empty — no warning, no throw.
/// ```text
/// expandToTargetDoors healthy=true    queue n=1  (door 95, expansion=1000.0, sorting=1000.0)
/// targetDoor item=2 treeEntryNo=99 treeShapeCount=1
/// targetDoor item=3 treeEntryNo=99 treeShapeCount=2
/// expandToTargetDoors stale=false     queue n=0
/// ```
#[test]
fn a_stale_tree_entry_is_skipped_silently() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = expand_control(&board, 1);
    let counter = Counter::new();
    let mut maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut engine,
        &mut board,
        &ctrl,
        &|| counter.check(),
    )
    .expect("init succeeds on this board");
    let seed = maze.queue.iter().next().expect("two seeded").clone();
    let room = seed.next_room.expect("a room");
    let mid = seed.shape_entry.a.middle_point(&seed.shape_entry.b);

    drain(&mut maze);
    assert!(maze.expand_to_target_doors(&mut board, &seed, true, false, &mid));
    assert_eq!(
        queue_rows(&maze),
        vec![(
            95,
            0,
            1000.0,
            1000.0,
            None,
            (500.0, 0.0),
            (500.0, 0.0),
            false,
            MazeAdjustment::None,
            false,
            0
        )]
    );

    drain(&mut maze);
    for door in maze.engine.rooms.room_target_doors(room).to_vec() {
        maze.engine
            .rooms
            .target_door_mut(door)
            .expect("live")
            .tree_entry_no = 99;
    }
    assert!(!maze.expand_to_target_doors(&mut board, &seed, true, false, &mid));
    assert_eq!(maze.queue.len(), 0);
}

// =================================================================================================
// MazeTraceShover (MazeTraceShover.java:32-314) and shoveTraceRoom (:1130-1201)
// =================================================================================================

/// Probe mode `shove`, first half. `shoveTraceRoom` and `checkShoveTraceLine` are **check-only**:
/// the board's item count is the same before and after, and the two early `true`s of `:39-44` —
/// a from-door that is not an `ExpansionDoor`, and an obstacle item that is not a `PolylineTrace`
/// — answer with an empty door list.
/// ```text
/// itemCountBefore=4
/// shoveTraceRoom(obstacleArea)=true
/// itemCountAfter=4
/// checkShoveTraceLine targetDoorFrom=true sections=0
/// ```
#[test]
fn shove_trace_room_does_not_mutate_the_board() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = expand_control(&board, 1);
    let counter = Counter::new();
    let mut maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut engine,
        &mut board,
        &ctrl,
        &|| counter.check(),
    )
    .expect("init succeeds on this board");
    let tree = maze.search_tree;
    let area_room = maze
        .engine
        .rooms
        .new_obstacle_room(&mut board, ItemId(4), 0, tree);
    let seed = maze.queue.iter().next().expect("two seeded").clone();

    let before = board.items.len();
    assert_eq!(before, 4, "itemCountBefore=4");
    assert!(maze.shove_trace_room(&mut board, &seed, area_room));
    assert_eq!(board.items.len(), 4, "itemCountAfter=4");

    let mut out: Vec<DoorSection> = Vec::new();
    assert!(MazeTraceShover::check_shove_trace_line(
        &seed,
        area_room,
        &mut maze.engine.rooms,
        &mut board,
        &ctrl,
        false,
        &mut out,
    ));
    assert!(out.is_empty(), "sections=0");
}

/// The probe's `shove` fixture: the shared board plus a net-2 obstacle trace whose half width and
/// clearance class match the control's, so the `:48-51` guard lets the algorithm through. Its
/// three tree shapes are the probe's:
/// ```text
/// trace id=5 halfWidth=1500 clearanceClass=1 lines=5 treeShapes=3
///   treeShape[0]=[-8600,-9600..1600,-6400]
///   treeShape[1]=[-1600,-9600..1600,-2900]
///   treeShape[2]=[-1600,-6100..7600,-2900]
/// ```
fn shove_board() -> Board {
    let mut board = probe_board();
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[
            Point::new(-7000, -8000),
            Point::new(0, -8000),
            Point::new(0, -4500),
            Point::new(6000, -4500),
        ]),
        0,
        1500,
        vec![2],
        1,
        FixedState::Unfixed,
    );
    board
}

/// `P6T12Probe.shoveOne`'s four doors: `fromDoor` on the upper edge's left half, one more on the
/// upper edge's right half, one on the lower edge and one on the left edge — so the collector of
/// `:236-312` has a candidate on either side of the shove line.
fn shove_doors(
    maze: &mut MazeSearchEngine<'_>,
    board: &mut Board,
    obstacle_room: ObstacleRoomId,
    corner_no: i32,
) -> DoorId {
    let room_ref = RoomRef::Obstacle(obstacle_room);
    let b = maze
        .engine
        .rooms
        .room_shape(room_ref)
        .expect("the room has a shape")
        .bounding_box();
    // Java's `int` division truncates toward zero, and so does Rust's.
    let mid_x = (b.ll.x + b.ur.x) / 2;
    let mid_y = (b.ll.y + b.ur.y) / 2;
    let mut make = |llx, lly, urx, ury, id| {
        let free = maze.engine.rooms.new_complete_room(
            Some(TileShape::Box(IntBox::from_coords(llx, lly, urx, ury))),
            0,
            id,
        );
        let door = maze
            .engine
            .rooms
            .new_door(RoomRef::Complete(free), room_ref, 1);
        maze.engine.rooms.add_door(room_ref, door);
        door
    };
    let from_door = make(b.ll.x, b.ur.y, mid_x, b.ur.y + 6000, 8000 + corner_no);
    make(mid_x, b.ur.y, b.ur.x, b.ur.y + 6000, 8100 + corner_no);
    make(mid_x, b.ll.y - 6000, b.ur.x, b.ll.y, 8200 + corner_no);
    make(b.ll.x - 6000, mid_y, b.ll.x, b.ur.y, 8300 + corner_no);
    let _ = board;
    from_door
}

/// Probe mode `shove`, the 24-cell table over the trace's three segments. Every cell is Java's
/// answer for `checkShoveTraceLine` on a 1-dimensional from-door, with the shape entry anchored at
/// either end of the door segment and the shove going either way. The `false`s are `:131-133`'s
/// `sectionOk` refusal — "shove only from the right most section to the right or from the left most
/// section to the left" — and every cell collects **no** door section, because
/// `TraceShover::check_segment` cannot open one on this board.
/// ```text
/// --- cornerNo=0 roomBox=[-8600,-9600..1600,-6400] doorSections=1
///   left=false entryAtA=true => false | left=false entryAtA=false => true
///   left=true  entryAtA=true => true  | left=true  entryAtA=false => false
/// --- cornerNo=1 roomBox=[-1600,-9600..1600,-2900] doorSections=1   (all four true)
/// --- cornerNo=2 roomBox=[-1600,-6100..7600,-2900] doorSections=1   (as cornerNo=0)
/// ```
#[test]
fn the_shover_answers_javas_table_over_the_three_trace_segments() {
    let expected: [[bool; 4]; 3] = [
        // (left=false, atA=true), (false, false), (true, true), (true, false)
        [false, true, true, false],
        [true, true, true, true],
        [false, true, true, false],
    ];
    let boxes = [
        (-8600, -9600, 1600, -6400),
        (-1600, -9600, 1600, -2900),
        (-1600, -6100, 7600, -2900),
    ];
    for corner_no in 0..3usize {
        let mut board = shove_board();
        let mut engine = probe_engine(&mut board, 1);
        let ctrl = probe_control(&board, 1);
        let tree = engine.tree;
        let mut maze = MazeSearchEngine::new(&mut engine, &ctrl);
        let obstacle_room =
            maze.engine
                .rooms
                .new_obstacle_room(&mut board, ItemId(5), corner_no, tree);
        let b = maze
            .engine
            .rooms
            .room_shape(RoomRef::Obstacle(obstacle_room))
            .expect("a shape")
            .bounding_box();
        assert_eq!(
            (b.ll.x, b.ll.y, b.ur.x, b.ur.y),
            boxes[corner_no],
            "treeShape[{corner_no}]"
        );
        let from_door = shove_doors(
            &mut maze,
            &mut board,
            obstacle_room,
            i32::try_from(corner_no).expect("0..3"),
        );
        assert_eq!(
            maze.engine
                .rooms
                .door_section_segments(from_door, 1600.0)
                .len(),
            1,
            "doorSections=1"
        );
        let door_segment = maze
            .engine
            .rooms
            .door_shape(from_door)
            .expect("live")
            .diagonal_corner_segment()
            .expect("a non-empty door");

        let mut answers = Vec::new();
        for (left, at_a) in [(false, true), (false, false), (true, true), (true, false)] {
            let anchor = if at_a { door_segment.a } else { door_segment.b };
            let element = MazeListElement {
                door: ExpandableRef::Door(from_door),
                section_no_of_door: 0,
                backtrack_door: None,
                section_no_of_backtrack_door: 0,
                expansion_value: 0.0,
                sorting_value: 0.0,
                next_room: Some(RoomRef::Obstacle(obstacle_room)),
                shape_entry: FloatLine::new(anchor, anchor),
                room_ripped: false,
                adjustment: MazeAdjustment::None,
                already_checked: false,
                ripup_cost: 0,
            };
            let mut out: Vec<DoorSection> = Vec::new();
            let items_before = board.items.len();
            let ok = MazeTraceShover::check_shove_trace_line(
                &element,
                obstacle_room,
                &mut maze.engine.rooms,
                &mut board,
                &ctrl,
                left,
                &mut out,
            );
            assert!(out.is_empty(), "sections=0 at cornerNo={corner_no}");
            assert_eq!(items_before, board.items.len(), "the board is not written");
            answers.push(ok);
        }
        assert_eq!(
            answers,
            expected[corner_no].to_vec(),
            "cornerNo={corner_no}"
        );
    }
}

/// Probe mode `shove`, the link-door half. A 2-dimensional door between the obstacle rooms of two
/// consecutive segments of the **same** trace takes `:73-96`, where `endPointsMatching` is the
/// identity arm of `:321-322`. Both shove directions answer `true` with no sections, and
/// `shoveTraceRoom` — whose `:1131-1137` guard passes because the door has one section — pushes
/// nothing onto the queue.
/// ```text
/// --- linkDoor id=163873 shape=[-1600,-6100..1600,-2900] maxWidth=3862.924345622109 sections=1
///   link left=false => true sections=0    shoveTraceRoom left=false => true    queue n=0
///   link left=true  => true sections=0    shoveTraceRoom left=true  => true    queue n=0
/// ```
#[test]
fn a_two_dimensional_link_door_takes_javas_other_shove_branch() {
    let mut board = shove_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let tree = engine.tree;
    let mut maze = MazeSearchEngine::new(&mut engine, &ctrl);
    let seg1 = maze
        .engine
        .rooms
        .new_obstacle_room(&mut board, ItemId(5), 1, tree);
    let seg2 = maze
        .engine
        .rooms
        .new_obstacle_room(&mut board, ItemId(5), 2, tree);
    let link_door = maze
        .engine
        .rooms
        .new_door(RoomRef::Obstacle(seg1), RoomRef::Obstacle(seg2), 2);
    maze.engine
        .rooms
        .add_door(RoomRef::Obstacle(seg2), link_door);
    for (llx, lly, urx, ury, id) in [
        (3000, -2900, 7600, 3000, 8500),
        (3000, -9000, 7600, -6100, 8600),
    ] {
        let free = maze.engine.rooms.new_complete_room(
            Some(TileShape::Box(IntBox::from_coords(llx, lly, urx, ury))),
            0,
            id,
        );
        let door = maze
            .engine
            .rooms
            .new_door(RoomRef::Complete(free), RoomRef::Obstacle(seg2), 1);
        maze.engine.rooms.add_door(RoomRef::Obstacle(seg2), door);
    }
    assert_eq!(maze.engine.rooms.door_id_no(link_door), Some(163_873));
    let link_shape = maze.engine.rooms.door_shape(link_door).expect("live");
    let b = link_shape.bounding_box();
    assert_eq!(
        (b.ll.x, b.ll.y, b.ur.x, b.ur.y),
        (-1600, -6100, 1600, -2900)
    );
    assert!((link_shape.max_width() - 3_862.924_345_622_109).abs() < 1e-9);
    assert_eq!(
        maze.engine
            .rooms
            .door_section_segments(link_door, 1600.0)
            .len(),
        1,
        "sections=1"
    );
    let centre = link_shape.centre_of_gravity();

    for left in [false, true] {
        let element = MazeListElement {
            door: ExpandableRef::Door(link_door),
            section_no_of_door: 0,
            backtrack_door: None,
            section_no_of_backtrack_door: 0,
            expansion_value: 0.0,
            sorting_value: 0.0,
            next_room: Some(RoomRef::Obstacle(seg2)),
            shape_entry: FloatLine::new(centre, centre),
            room_ripped: false,
            adjustment: MazeAdjustment::None,
            already_checked: false,
            ripup_cost: 0,
        };
        let mut out: Vec<DoorSection> = Vec::new();
        assert!(MazeTraceShover::check_shove_trace_line(
            &element,
            seg2,
            &mut maze.engine.rooms,
            &mut board,
            &ctrl,
            left,
            &mut out,
        ));
        assert!(out.is_empty(), "sections=0");
        drain(&mut maze);
        assert!(maze.shove_trace_room(&mut board, &element, seg2));
        assert_eq!(maze.queue.len(), 0, "queue n=0");
    }
}

/// Probe mode `shove`, the tail — **hazard N**. `ObstacleExpansionRoom` snapshots its shape but
/// keeps its `indexInItem`, so shortening the trace underneath it leaves the index past the end of
/// the current polyline. `:65-66` answers `false` with an empty door list: no warning, no throw,
/// and no attempt to index `lines[traceCornerNo + 2]`.
/// ```text
/// --- stale indexInItem=2 lines=3
///   checkShoveTraceLine=false sections=0
/// ```
#[test]
fn a_stale_trace_index_is_refused_silently_by_the_shover() {
    let mut board = shove_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let tree = engine.tree;
    let maze = MazeSearchEngine::new(&mut engine, &ctrl);
    let stale_room = maze
        .engine
        .rooms
        .new_obstacle_room(&mut board, ItemId(5), 2, tree);
    let b = maze
        .engine
        .rooms
        .room_shape(RoomRef::Obstacle(stale_room))
        .expect("a shape")
        .bounding_box();
    let free = maze
        .engine
        .rooms
        .new_complete_room(Some(TileShape::Box(b.offset(1000.0))), 0, 8900);
    let stale_door =
        maze.engine
            .rooms
            .new_door(RoomRef::Complete(free), RoomRef::Obstacle(stale_room), 1);
    maze.engine
        .rooms
        .add_door(RoomRef::Obstacle(stale_room), stale_door);
    maze.engine.rooms.door_section_segments(stale_door, 1600.0);
    let anchor = maze
        .engine
        .rooms
        .door_shape(stale_door)
        .expect("live")
        .centre_of_gravity();
    let element = MazeListElement {
        door: ExpandableRef::Door(stale_door),
        section_no_of_door: 0,
        backtrack_door: None,
        section_no_of_backtrack_door: 0,
        expansion_value: 0.0,
        sorting_value: 0.0,
        next_room: Some(RoomRef::Obstacle(stale_room)),
        shape_entry: FloatLine::new(anchor, anchor),
        room_ripped: false,
        adjustment: MazeAdjustment::None,
        already_checked: false,
        ripup_cost: 0,
    };

    // The trace shrinks underneath the room, exactly as pull-tight or a shove would leave it.
    match board.items.get_mut(&ItemId(5)) {
        Some(Item::Trace(trace)) => trace.set_polyline(Polyline::from_points(&[
            Point::new(-7000, -8000),
            Point::new(0, -8000),
        ])),
        _ => panic!("item 5 is the obstacle trace"),
    }
    assert_eq!(
        board.items.get(&ItemId(5)).map(|item| match item {
            Item::Trace(trace) => trace.polyline().lines().len(),
            _ => 0,
        }),
        Some(3),
        "lines=3"
    );
    assert_eq!(
        maze.engine
            .rooms
            .obstacle_room(stale_room)
            .expect("live")
            .get_index_in_item(),
        2,
        "indexInItem=2"
    );

    let mut out: Vec<DoorSection> = Vec::new();
    assert!(
        !MazeTraceShover::check_shove_trace_line(
            &element,
            stale_room,
            &mut maze.engine.rooms,
            &mut board,
            &ctrl,
            false,
            &mut out,
        ),
        "checkShoveTraceLine=false"
    );
    assert!(out.is_empty(), "sections=0");
}

/// Probe mode `neck2` — the two `withNeckdown` call sites, which is where quirk #179 *bites*.
/// `:407-414` runs only for an `ExpansionDoor` and narrows **both** `halfWidthAdd` and `halfWidth`;
/// `:442-451` runs only for a `TargetItemExpansionDoor` and narrows `halfWidth` alone. Either way
/// the value comes from `checkNeckDownAtDestPin`, i.e. the *start* pin's `49.0`.
///
/// The round is run on **room 4**, whose shape has `minWidth() = 687.49`: `:458`'s
/// `minWidth() < 2 * halfWidth` makes it **thin** against the control's 1600 and **thick** against
/// the pin's 49, so each site flips.
/// ```text
/// --- site=targetDoor    withNeckdown=false targetDoor=66     room=4 roomMinWidth=687.4942085903301 checkNeckDownAtDestPin=49.0
///   expandToRoomDoors=false   queue n=0
/// --- site=targetDoor    withNeckdown=true  targetDoor=66
///   expandToRoomDoors=true    queue n=1   (97, expansion=1000.0, sorting=1000.0)
/// --- site=expansionDoor withNeckdown=false expansionDoor=66 dimension=1
///   expandToRoomDoors=true    queue n=1   (ExpansionDoor 129, expansion=1007.153180628, sorting=3572.056297706)
/// --- site=expansionDoor withNeckdown=true  expansionDoor=66 dimension=1
///   expandToRoomDoors=true    queue n=2   (66 @1550.0/2380.0, 97 @2550.0/2550.0)
/// ```
#[test]
fn the_neckdown_call_sites_narrow_the_half_width_for_the_whole_round() {
    type Expected = (bool, Vec<(i32, f64, f64)>);
    let cases: [(&str, bool, Expected); 4] = [
        ("targetDoor", false, (false, vec![])),
        ("targetDoor", true, (true, vec![(97, 1000.0, 1000.0)])),
        (
            "expansionDoor",
            false,
            (true, vec![(129, 1_007.153_180_628, 3_572.056_297_706)]),
        ),
        (
            "expansionDoor",
            true,
            (true, vec![(66, 1550.0, 2380.0), (97, 2550.0, 2550.0)]),
        ),
    ];
    for (site, neckdown, (expanded, rows)) in cases {
        let mut board = probe_board();
        let mut engine = probe_engine(&mut board, 1);
        let mut settings = probe_settings(&board);
        settings.set_automatic_neckdown(neckdown);
        let trace_costs = settings.get_trace_costs();
        let mut ctrl =
            AutorouteControl::new(&board, 1, &settings, settings.get_via_costs(), &trace_costs);
        ctrl.vias_allowed = false;
        assert_eq!(ctrl.with_neckdown, neckdown);
        let counter = Counter::new();
        let mut maze = MazeSearchEngine::get_instance(
            &set_of(&[2]),
            &set_of(&[3]),
            &mut engine,
            &mut board,
            &ctrl,
            &|| counter.check(),
        )
        .expect("init succeeds on this board");

        // The **second** seeded element: room 4, 1041 units tall.
        let seed = maze.queue.iter().last().expect("two seeded").clone();
        let room = seed.next_room.expect("a room");
        assert_eq!(maze.engine.rooms.room_id_no(room), Some(4));
        let room_min_width = maze
            .engine
            .rooms
            .room_shape(room)
            .expect("a shape")
            .min_width();
        assert!((room_min_width - 687.494_208_590_330_1).abs() < 1e-9);
        assert_eq!(ctrl.compensated_trace_half_width[0], 1600);
        assert_eq!(maze.check_neck_down_at_dest_pin(&board, room), 49.0);

        let element = if site == "targetDoor" {
            assert_eq!(maze.engine.expandable_id_no(seed.door), 66);
            seed
        } else {
            let door = maze.engine.rooms.room_doors(room)[0];
            assert_eq!(maze.engine.rooms.door_id_no(door), Some(66));
            assert_eq!(maze.engine.rooms.door(door).expect("live").dimension, 1);
            let centre = maze
                .engine
                .rooms
                .door_shape(door)
                .expect("live")
                .centre_of_gravity();
            MazeListElement {
                door: ExpandableRef::Door(door),
                section_no_of_door: 0,
                backtrack_door: None,
                section_no_of_backtrack_door: 0,
                expansion_value: 0.0,
                sorting_value: 0.0,
                next_room: Some(room),
                shape_entry: FloatLine::new(centre, centre),
                room_ripped: false,
                adjustment: MazeAdjustment::None,
                already_checked: false,
                ripup_cost: 0,
            }
        };

        drain(&mut maze);
        assert_eq!(
            maze.expand_to_room_doors(&mut board, &element),
            expanded,
            "site={site} withNeckdown={neckdown}"
        );
        let got: Vec<(i32, f64, f64)> = maze
            .queue
            .iter()
            .map(|e| {
                (
                    maze.engine.expandable_id_no(e.door),
                    r9(e.expansion_value),
                    r9(e.sorting_value),
                )
            })
            .collect();
        assert_eq!(got, rows, "site={site} withNeckdown={neckdown}");
    }
}
