//! Plan 6 Task 11: `MazeSearchEngine`'s construction, `init` and pop loop
//! (`autoroute/maze/MazeSearchEngine.java:75-152`, `:300-384`, `:763-789`, `:969-1103`).
//!
//! # Where the numbers come from
//!
//! Every literal below — the item ids, the completed-room boxes, the `getId()` hashes of the
//! target doors, the seeded elements' `sortingValue`s, the stop-call counts at each of the five
//! cancellation sites, and `doorIsSmall`'s answers under the three angle restrictions — is **read
//! off the HEAD jar**, not off this port. The probe is
//! `scripts/differential/java/probes/P6T11Probe.java`, which is committed with the exact
//! `javac`/`java` invocation in its header; its whole stdout is committed as
//! `tests/data/p6t11-maze-search.txt`. Each test names its probe mode and pastes the lines it
//! asserts against.
//!
//! # The fixture, and why it is not `tests/drill.rs`'s board
//!
//! The board is `P6T7Probe.build`'s two-pin board with the two traces replaced by a single
//! obstacle box at `[700,-1000..900,1000]`. The traces had to go: with the "wide" clearance class
//! the foreign-net trace's *compensated* tree shape swallows the start pin's centre, and
//! `ShapeSearchTree.completeShape` then answers **no room at all** for the start pin — JVM-pinned,
//! and the reason the first draft of the probe reported `init=false`. A pin's
//! `getTraceConnectionShape` is a bare point (`DrillItem.java:359-361`), so a start room only
//! exists where that point is outside every foreign obstacle.

use std::cell::Cell;
use std::collections::BTreeSet;

use fr_board::ids::ItemId;
use fr_board::prelude::*;
use fr_board::rules::ViaRule;
use fr_geometry::{
    Area, FloatLine, FloatPoint, IntBox, IntOctagon, IntPoint, IntVector, Point, Polyline, Shape,
    TileShape,
};
use fr_router::autoroute::expansion::{ExpandableRef, RoomRef};
use fr_router::autoroute::item_info;
use fr_router::autoroute::maze::engine::AutorouteEngine;
use fr_router::autoroute::maze::search::{
    MazeSearchEngine, segment_projection, to_impacted_points,
};
use fr_router::autoroute::maze::{AutorouteControl, MazeListElement};
use fr_settings::RouterSettings;

// =================================================================================================
// The probe's board, rebuilt from scratch
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

/// `P6T11Probe.tiePin`'s board: one pin carrying **two** nets, a foreign-net trace and an own-net
/// trace, both ending at the pin centre.
fn tie_pin_board() -> Board {
    let layers = || LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)]);
    let clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
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
    let pad = Shape::Tile(TileShape::Box(IntBox::from_coords(-200, -200, 200, 200)));
    let through = padstacks.add("thru", vec![Some(pad.clone()), Some(pad)], true, false);
    let mut packages = Packages::new();
    let package = packages.add(
        "pkg",
        vec![PackagePin::new(
            "P1",
            through,
            IntVector::new(0, 0).into(),
            0.0,
        )],
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
    board.insert_pin(1, 0, vec![1, 2], 1, FixedState::Unfixed);
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(0, 0), Point::new(0, 2000)]),
        0,
        60,
        vec![2],
        1,
        FixedState::Unfixed,
    );
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(0, 0), Point::new(2000, 0)]),
        0,
        60,
        vec![1],
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

/// A `StopCheck` that answers `true` from its `trip`-th call onwards; `0` never fires. Java's is
/// `P6T11Probe.Counter`.
struct Counter {
    trip: u32,
    calls: Cell<u32>,
}

impl Counter {
    fn new(trip: u32) -> Counter {
        Counter {
            trip,
            calls: Cell::new(0),
        }
    }

    fn check(&self) -> bool {
        self.calls.set(self.calls.get() + 1);
        self.trip > 0 && self.calls.get() >= self.trip
    }

    fn calls(&self) -> u32 {
        self.calls.get()
    }
}

/// `[ItemId(2), …]` — the probe's `setOf`, which is a `TreeSet<Item>`.
fn set_of(ids: &[u32]) -> BTreeSet<ItemId> {
    ids.iter().map(|id| ItemId(*id)).collect()
}

/// `(java room id, layer, bounding box, target-door count)` for every room in
/// `completeExpansionRooms` (AutorouteEngine.java:74), in list order — the shape
/// `P6T11Probe.dumpRooms` prints.
type RoomRow = (i32, usize, (i32, i32, i32, i32), usize);

fn complete_rooms(maze: &MazeSearchEngine<'_>) -> Vec<RoomRow> {
    maze.engine
        .complete_expansion_rooms()
        .iter()
        .map(|id| {
            let room = maze.engine.rooms.complete_room(*id).expect("live room");
            let b = room.get_shape().expect("a shape").bounding_box();
            (
                room.get_id(),
                room.get_layer(),
                (b.ll.x, b.ll.y, b.ur.x, b.ur.y),
                room.get_target_doors().len(),
            )
        })
        .collect()
}

/// One queue row, in the shape `P6T11Probe.dumpQueue` prints it:
/// `(door id, section, expansionValue, sortingValue, next-room id, shape entry a)`.
type QueueRow = (i32, i32, f64, f64, Option<i32>, (f64, f64));

fn queue_rows(maze: &MazeSearchEngine<'_>) -> Vec<QueueRow> {
    maze.queue
        .iter()
        .map(|e| row_of(maze, e))
        .collect::<Vec<_>>()
}

fn row_of(maze: &MazeSearchEngine<'_>, e: &MazeListElement) -> QueueRow {
    (
        maze.engine.expandable_id_no(e.door),
        e.section_no_of_door,
        e.expansion_value,
        e.sorting_value,
        e.next_room
            .and_then(|room| maze.engine.rooms.room_id_no(room)),
        (e.shape_entry.a.x, e.shape_entry.a.y),
    )
}

// =================================================================================================
// The board itself
// =================================================================================================

/// Probe mode `items`:
/// ```text
/// item id=4 ObstacleArea nets=[] treeShapeCount=1
/// item id=3 Pin nets=[1] treeShapeCount=2
/// item id=2 Pin nets=[1] treeShapeCount=1
/// item id=1 BoardOutline nets=[] treeShapeCount=0
/// ```
#[test]
fn the_boards_item_ids_are_the_probes() {
    let mut board = probe_board();
    let tree = board.default_tree_id();
    let ids: Vec<u32> = board
        .items_in_board_order()
        .into_iter()
        .map(|id| id.0)
        .collect();
    assert_eq!(ids, vec![4, 3, 2, 1], "getItems() is descending id");
    assert_eq!(board.item_tree_shape_count(ItemId(3), tree), 2);
    assert_eq!(board.item_tree_shape_count(ItemId(2), tree), 1);
}

// =================================================================================================
// `init` (:969-1103)
// =================================================================================================

/// Probe mode `init`:
/// ```text
/// init=true
/// stopCalls=7
/// startInfo item2=true
/// startInfo item3=false
/// startInfo item4=false
/// completeRooms n=3
/// room id=1 layer=0 box=[-10000,-10000..-4700,0] doors=2 targetDoors=0
/// room id=2 layer=0 box=[-4700,-10000..600,0] doors=4 targetDoors=2
///     target TargetItemExpansionDoor id=64 item=2 treeEntryNo=0 room=2 dest=false
///     target TargetItemExpansionDoor id=95 item=3 treeEntryNo=0 room=2 dest=true
/// room id=4 layer=0 box=[-4700,0..600,1041] doors=2 targetDoors=2
///     target TargetItemExpansionDoor id=66 item=2 treeEntryNo=0 room=4 dest=false
///     target TargetItemExpansionDoor id=97 item=3 treeEntryNo=0 room=4 dest=true
/// queue n=2
///   [0] door=… id=64 … expansion=0.000000000 sorting=830.000000000 nextRoom=2 shapeEntry=(-500,0)-(-500,0)
///   [1] door=… id=66 … expansion=0.000000000 sorting=830.000000000 nextRoom=4 shapeEntry=(-500,0)-(-500,0)
/// ```
#[test]
fn init_seeds_one_element_per_non_destination_target_door() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let counter = Counter::new(0);
    let mut maze = MazeSearchEngine::new(&mut engine, &ctrl);
    assert!(maze.init(&mut board, &set_of(&[2]), &set_of(&[3]), &|| {
        counter.check()
    }));
    assert_eq!(counter.calls(), 7);

    assert!(item_info::is_start_info(&mut board, ItemId(2)));
    assert!(!item_info::is_start_info(&mut board, ItemId(3)));
    assert!(!item_info::is_start_info(&mut board, ItemId(4)));

    // PORT-REGRESSION PIN, `accepted at plan9-t7t8 (ruling CC)`: the jar's room ids `1, 2, 4`
    // read `6, 7, 13` here, because room ids come from ONE shared counter across the engine's
    // room kinds (#156/#167/#158). **Every other column is the jar's** — the same three rooms, in
    // the same order, on the same layers, with the same boxes and the same target-door counts
    // `0, 2, 2`, which is what "one element per non-destination target door" is read off.
    assert_eq!(
        complete_rooms(&maze),
        vec![
            (6, 0, (-10_000, -10_000, -4700, 0), 0),
            (7, 0, (-4700, -10_000, 600, 0), 2),
            (13, 0, (-4700, 0, 600, 1041), 2),
        ]
    );

    // Same pin, same cause: the jar's door ids `64, 66` over rooms `2, 4` are the port's
    // `69, 75` over rooms `7, 13`. The costs `0.0/830.0` and the entry point `(-500,0)` are the
    // jar's, and so is the row count — one element per non-destination target door.
    assert_eq!(
        queue_rows(&maze),
        vec![
            (69, 0, 0.0, 830.0, Some(7), (-500.0, 0.0)),
            (75, 0, 0.0, 830.0, Some(13), (-500.0, 0.0)),
        ]
    );
    assert_eq!(maze.destination_door(), None);
}

/// Probe mode `startorder`. A **two-item** start set, so that the order `init` walks it in is
/// observable: Java's `Set<Item>` is a `TreeSet` ordered by `Item.compareTo` = `other.id - this.id`
/// (Item.java:95-101), i.e. **descending id**, so the through pin (item 3, two tree shapes) seeds
/// its two incomplete rooms before the SMD pin (item 2, one). Walking the `BTreeSet` forwards
/// would swap the room ids and the queue's first three rows.
/// ```text
/// init=true
/// stopCalls=11
/// room id=1 layer=0 box=[-10000,-10000..-4700,0]  targetDoors=0
/// room id=2 layer=0 box=[-4700,-10000..600,0]     targetDoors=2   (64, 95)
/// room id=4 layer=0 box=[-4700,0..600,1041]       targetDoors=2   (66, 97)
/// room id=5 layer=1 box=[-10000,-10000..0,0]      targetDoors=0
/// room id=9 layer=1 box=[0,0..10000,10000]        targetDoors=1   (102)
/// queue n=5: 95@0.0/room2, 97@0.0/room4, 102@0.0/room9, 64@830.0/room2, 66@830.0/room4
/// ```
#[test]
fn init_walks_the_start_set_in_javas_descending_item_order() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let counter = Counter::new(0);
    let mut maze = MazeSearchEngine::new(&mut engine, &ctrl);
    assert!(maze.init(&mut board, &set_of(&[2, 3]), &set_of(&[3]), &|| {
        counter.check()
    }));
    assert_eq!(counter.calls(), 11);

    assert_eq!(
        complete_rooms(&maze),
        // PORT-REGRESSION PIN, same cause and same wave: jar `1, 2, 4, 5, 9`, port
        // `8, 9, 15, 17, 25`. Layers, boxes and target-door counts are the jar's, and so is the
        // descending item order this test is named for.
        vec![
            (8, 0, (-10_000, -10_000, -4700, 0), 0),
            (9, 0, (-4700, -10_000, 600, 0), 2),
            (15, 0, (-4700, 0, 600, 1041), 2),
            (17, 1, (-10_000, -10_000, 0, 0), 0),
            (25, 1, (0, 0, 10_000, 10_000), 1),
        ]
    );
    // Same pin, same cause. The five rows, their costs and their two entry points are the jar's;
    // only the door and room ids moved with the shared counter.
    assert_eq!(
        queue_rows(&maze),
        vec![
            (102, 0, 0.0, 0.0, Some(9), (500.0, 0.0)),
            (108, 0, 0.0, 0.0, Some(15), (500.0, 0.0)),
            (118, 0, 0.0, 0.0, Some(25), (500.0, 0.0)),
            (71, 0, 0.0, 830.0, Some(9), (-500.0, 0.0)),
            (77, 0, 0.0, 830.0, Some(15), (-500.0, 0.0)),
        ]
    );
}

/// Probe mode `startrooms`. The start-set order reaches the *incomplete* room list before the
/// completion loop can hide it: aborting on the fourth stop call leaves
/// ```text
/// init=false
/// stopCalls=4
/// incompleteRooms n=3
///   [0] layer=0 contained=[500,0..500,0]
///   [1] layer=1 contained=[500,0..500,0]
///   [2] layer=0 contained=[-500,0..-500,0]
/// ```
/// — the **through** pin (item 3, the higher id, two tree shapes) first. Walking the `BTreeSet`
/// forwards puts item 2's room at index 0, which is what this test rules out; the completed rooms
/// of `init_walks_the_start_set_in_javas_descending_item_order` happen to come out the same either
/// way on this board, because both pin centres fall in the same `completeShape` partition.
#[test]
fn init_creates_the_start_rooms_in_javas_descending_item_order() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let counter = Counter::new(4);
    let mut maze = MazeSearchEngine::new(&mut engine, &ctrl);
    assert!(
        !maze.init(&mut board, &set_of(&[2, 3]), &set_of(&[3]), &|| counter
            .check())
    );
    assert_eq!(counter.calls(), 4);

    let rooms: Vec<(usize, (i32, i32, i32, i32))> = maze
        .engine
        .rooms
        .incomplete_rooms
        .iter()
        .map(|(_, room)| {
            let b = room
                .get_contained_shape()
                .expect("the start room's contained shape")
                .bounding_box();
            (room.get_layer(), (b.ll.x, b.ll.y, b.ur.x, b.ur.y))
        })
        .collect();
    assert_eq!(
        rooms,
        vec![
            (0, (500, 0, 500, 0)),
            (1, (500, 0, 500, 0)),
            (0, (-500, 0, -500, 0)),
        ]
    );
}

/// Probe mode `nostart`:
/// ```text
/// emptyStart instance=null
/// emptyStart stopCalls=1
/// emptyDest instance=null
/// emptyDest stopCalls=0
/// ```
#[test]
fn init_returns_none_when_no_start_door_exists() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let counter = Counter::new(0);
    assert!(
        MazeSearchEngine::get_instance(
            &BTreeSet::new(),
            &set_of(&[3]),
            &mut engine,
            &mut board,
            &ctrl,
            &|| counter.check(),
        )
        .is_none()
    );
    assert_eq!(counter.calls(), 1);

    // The empty *destination* set never reaches a stop check at all: `:975` is inside the loop.
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let counter = Counter::new(0);
    assert!(
        MazeSearchEngine::get_instance(
            &set_of(&[2]),
            &BTreeSet::new(),
            &mut engine,
            &mut board,
            &ctrl,
            &|| counter.check(),
        )
        .is_none()
    );
    assert_eq!(counter.calls(), 0);
}

// =================================================================================================
// The five cancellation sites (plan-6 ruling 6)
// =================================================================================================

/// Runs `init` with a stop check that trips on its `trip`-th call and answers
/// `(init's answer, stop calls, item 2's startInfo, complete-room count, queue length)`.
fn init_with_trip(trip: u32) -> (bool, u32, bool, usize, usize) {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let counter = Counter::new(trip);
    let mut maze = MazeSearchEngine::new(&mut engine, &ctrl);
    let ok = maze.init(&mut board, &set_of(&[2]), &set_of(&[3]), &|| {
        counter.check()
    });
    let rooms = maze.engine.complete_expansion_rooms().len();
    let queued = maze.queue.len();
    let start_info = item_info::is_start_info(&mut board, ItemId(2));
    (ok, counter.calls(), start_info, rooms, queued)
}

/// Probe modes `stop1`..`stop8`. Site 1 is `:975`, the destination loop; site 2 is `:1009`, the
/// start loop; site 3 is `:1035`, the room-completion loop; sites 4-7 are `:1051`, the target-door
/// loop, once per door **including** the destination doors it is about to `continue` past.
#[test]
fn each_of_the_four_init_stop_sites_aborts_where_java_does() {
    // stop1: `trip=1 init=false / stopCalls=1 / startInfo item2=false / completeRooms n=-1`
    assert_eq!(init_with_trip(1), (false, 1, false, 0, 0));
    // stop2: `trip=2 init=false / stopCalls=2 / startInfo item2=false / completeRooms n=-1`
    assert_eq!(init_with_trip(2), (false, 2, false, 0, 0));
    // stop3: `trip=3 init=false / stopCalls=3 / startInfo item2=true / completeRooms n=-1`
    assert_eq!(init_with_trip(3), (false, 3, true, 0, 0));
    // stop4: `trip=4 init=false / stopCalls=4 / completeRooms n=3 / queue n=0`
    assert_eq!(init_with_trip(4), (false, 4, true, 3, 0));
    // stop5 and stop6: one element seeded, then the abort.
    assert_eq!(init_with_trip(5), (false, 5, true, 3, 1));
    assert_eq!(init_with_trip(6), (false, 6, true, 3, 1));
    // stop7: both seeded, and the abort still wins over `startOk`.
    assert_eq!(init_with_trip(7), (false, 7, true, 3, 2));
    // stop8: the check never fires — seven calls, `init` succeeds.
    assert_eq!(init_with_trip(8), (true, 7, true, 3, 2));
}

/// The fifth site is `:323`, the top of `occupyNextElement`'s pop loop: a stop there answers
/// `false` with the queue **untouched**, because the check runs before `iterator().next()`.
#[test]
fn the_pop_loops_stop_check_aborts_before_the_queue_is_touched() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let never = Counter::new(0);
    let mut maze = MazeSearchEngine::new(&mut engine, &ctrl);
    assert!(maze.init(&mut board, &set_of(&[2]), &set_of(&[3]), &|| never.check()));
    assert_eq!(maze.queue.len(), 2);

    let always = Counter::new(1);
    assert!(!maze.occupy_next_element(&mut board, &|| always.check()));
    assert_eq!(always.calls(), 1);
    assert_eq!(maze.queue.len(), 2, "nothing was popped");
    assert_eq!(maze.destination_door(), None);
}

// =================================================================================================
// `occupyNextElement` (:314-384)
// =================================================================================================

/// The destination target door of room 2, and its centre.
fn destination_door_of_room(
    maze: &MazeSearchEngine<'_>,
    board: &mut Board,
    room: RoomRef,
) -> ExpandableRef {
    let doors = maze.engine.rooms.room_target_doors(room).to_vec();
    for door in doors {
        let is_dest = maze
            .engine
            .rooms
            .target_door(door)
            .expect("live target door")
            .is_destination_door(board);
        if is_dest {
            return ExpandableRef::TargetDoor(door);
        }
    }
    panic!("the room has no destination door");
}

/// Probe mode `pops`:
/// ```text
/// destDoor=TargetItemExpansionDoor id=95 item=3 treeEntryNo=0 room=2 dest=true
/// afterPush n=3
/// occupyNextElement=false
/// afterPop n=2
/// destinationDoor=TargetItemExpansionDoor id=95 …
/// sectionNoOfDestinationDoor=0
/// destDoorSectionOccupied=false
/// findConnection=TargetItemExpansionDoor id=95 … section=0
/// ```
#[test]
fn the_queue_pops_the_lowest_sorting_value_and_removes_it() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let never = Counter::new(0);
    let mut maze = MazeSearchEngine::new(&mut engine, &ctrl);
    assert!(maze.init(&mut board, &set_of(&[2]), &set_of(&[3]), &|| never.check()));

    let room = maze
        .queue
        .iter()
        .next()
        .expect("a seeded element")
        .next_room;
    let room = room.expect("the seeded element has a next room");
    let dest_door = destination_door_of_room(&maze, &mut board, room);
    // PORT-REGRESSION PIN, same wave: the jar's destination-door id is `95`, the port's `100`.
    // Door ids are derived from room ids (`ExpansionDoor.getId`), so the shared room-id counter
    // (#156/#167/#158) moves them too. The claim — that the queue pops the LOWEST sorting value
    // and removes it — is asserted below on the queue's own contents, not on this number.
    assert_eq!(maze.engine.expandable_id_no(dest_door), 100);

    let centre = maze
        .engine
        .rooms
        .target_door(match dest_door {
            ExpandableRef::TargetDoor(d) => d,
            _ => unreachable!(),
        })
        .expect("live")
        .get_shape()
        .centre_of_gravity();
    // `sortingValue = -1.0` puts it in front of the two seeded elements at 830.0.
    maze.push(
        MazeListElement {
            door: dest_door,
            section_no_of_door: 0,
            backtrack_door: None,
            section_no_of_backtrack_door: 0,
            expansion_value: 0.0,
            sorting_value: -1.0,
            next_room: Some(room),
            shape_entry: FloatLine::new(centre, centre),
            room_ripped: false,
            adjustment: fr_router::autoroute::maze::MazeAdjustment::None,
            already_checked: false,
            ripup_cost: 0,
        },
        &board,
    );
    assert_eq!(maze.queue.len(), 3);

    assert!(!maze.occupy_next_element(&mut board, &|| never.check()));
    assert_eq!(maze.queue.len(), 2);
    assert_eq!(maze.destination_door(), Some(dest_door));
    assert_eq!(maze.section_no_of_destination_door(), 0);
    // The early `return false` at `:357-358` happens *before* `:382`, so the section stays free.
    let occupied = match dest_door {
        ExpandableRef::TargetDoor(d) => {
            maze.engine
                .rooms
                .target_door(d)
                .expect("live")
                .get_maze_search_element(0)
                .is_occupied
        }
        _ => unreachable!(),
    };
    assert!(!occupied);

    let result = maze
        .find_connection(&mut board, &|| never.check())
        .expect("a result");
    assert_eq!(result.destination_door, dest_door);
    assert_eq!(result.section_no_of_door, 0);
}

/// Probe mode `occupied`:
/// ```text
/// beforePop n=3
/// occupyNextElement=false
/// afterPop n=0
/// destinationDoor=TargetItemExpansionDoor id=95 …
/// skippedSectionBacktrack=null
/// ```
#[test]
fn an_already_occupied_section_is_skipped_without_expanding() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let never = Counter::new(0);
    let mut maze = MazeSearchEngine::new(&mut engine, &ctrl);
    assert!(maze.init(&mut board, &set_of(&[2]), &set_of(&[3]), &|| never.check()));

    let seeded: Vec<(ExpandableRef, i32, Option<RoomRef>)> = maze
        .queue
        .iter()
        .map(|e| (e.door, e.section_no_of_door, e.next_room))
        .collect();
    for (door, section, _) in &seeded {
        maze.engine
            .maze_search_element_mut(*door, *section)
            .expect("a live door section")
            .is_occupied = true;
    }

    let room = seeded[0].2.expect("a next room");
    let dest_door = destination_door_of_room(&maze, &mut board, room);
    let centre = maze
        .engine
        .rooms
        .target_door(match dest_door {
            ExpandableRef::TargetDoor(d) => d,
            _ => unreachable!(),
        })
        .expect("live")
        .get_shape()
        .centre_of_gravity();
    maze.push(
        MazeListElement {
            door: dest_door,
            section_no_of_door: 0,
            backtrack_door: None,
            section_no_of_backtrack_door: 0,
            expansion_value: 0.0,
            sorting_value: 1.0e9,
            next_room: Some(room),
            shape_entry: FloatLine::new(centre, centre),
            room_ripped: false,
            adjustment: fr_router::autoroute::maze::MazeAdjustment::None,
            already_checked: false,
            ripup_cost: 0,
        },
        &board,
    );
    assert_eq!(maze.queue.len(), 3);

    assert!(!maze.occupy_next_element(&mut board, &|| never.check()));
    assert_eq!(maze.queue.len(), 0, "all three were popped");
    assert_eq!(maze.destination_door(), Some(dest_door));
    // The two skipped sections never had the backtrack fields copied onto them (`:342-346` runs
    // only for the element the loop breaks on).
    for (door, section, _) in &seeded {
        assert_eq!(
            maze.engine
                .maze_search_element(*door, *section)
                .expect("a live door section")
                .backtrack_door,
            None
        );
    }
}

/// `findConnection` (`:300-312`) on an exhausted queue is Java's `null`.
#[test]
fn find_connection_answers_none_when_the_queue_is_empty() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let never = Counter::new(0);
    let mut maze = MazeSearchEngine::new(&mut engine, &ctrl);
    assert!(
        maze.find_connection(&mut board, &|| never.check())
            .is_none()
    );
}

// =================================================================================================
// The fanout window on `init`'s own `add` (:84-125 through :1079)
// =================================================================================================

/// Probe mode `fanout`, and quirk **#178**, **fixed: T8**:
/// ```text
/// resolution=0.03937007874015748
/// maxEscapeLengthMm=null
/// instance=ok
/// queue n=0
/// ```
///
/// The jar's queue is **empty** and its `getInstance` still hands back an engine: `:1079-1080`
/// is `mazeExpansionList.add(newListElement);` on one line and `startOk = true;` on the next, so
/// the `boolean` the overridden `add` (`:86-124`) just answered is discarded. The caller then
/// pays for a whole engine construction, a `reduceTraceShapesAtTiePins` pass and a full round of
/// room completion to learn that the queue was empty and `findConnection` can only answer `null`.
///
/// `startOk` is now exactly what `add` answered on at least one element, so a fanout window too
/// tight for any start element makes `getInstance` answer `None` - which **is** what
/// "initialisation failed" means, and is what Java's own `:1083-1102` reads `startOk` for.
///
/// **KNOWN DIVERGENCE from the jar, authorized by #178**: the jar's `instance=ok` with `queue
/// n=0` is kept above, beside the port's `None`.
#[test]
fn an_empty_queue_makes_get_instance_answer_none() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let mut ctrl = probe_control(&board, 1);
    ctrl.is_fanout = true;
    ctrl.fanout_start_pin_center = Some(Point::new(9000, 9000));
    ctrl.fanout_start_pin_layer = 0;
    assert_eq!(ctrl.fanout_max_escape_length, 3000.0);

    let never = Counter::new(0);
    let maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut engine,
        &mut board,
        &ctrl,
        &|| never.check(),
    );
    assert!(
        maze.is_none(),
        "every seeded element was refused by the escape window, so initialisation failed - \
         the jar answers an engine here whose queue is empty (`instance=ok`, `queue n=0`)"
    );
}

// =================================================================================================
// `doorIsSmall` (:763-789)
// =================================================================================================

/// Probe mode `small`. Every row of the transcript, as
/// `(restriction, door box, trace width, answer)`.
#[test]
fn door_is_small_matches_the_three_angle_restrictions() {
    // `[llx, lly, urx, ury]` of the two rooms' overlap, and the seven widths the probe tries.
    const GEOMS: [(i32, i32, i32, i32); 3] = [(0, 0, 100, 40), (0, 0, 40, 100), (0, 0, 30, 30)];
    const WIDTHS: [f64; 7] = [10.0, 42.0, 45.0, 101.0, 105.0, 108.0, 200.0];
    // Read off `tests/data/p6t11-maze-search.txt`, mode `small`.
    const EXPECTED: [[bool; 7]; 9] = [
        // NINETY_DEGREE
        [false, false, false, true, true, true, true],
        [false, false, false, true, true, true, true],
        [false, true, true, true, true, true, true],
        // FORTYFIVE_DEGREE
        [false, false, false, true, true, true, true],
        [false, false, false, true, true, true, true],
        [false, false, true, true, true, true, true],
        // NONE
        [false, false, false, false, false, true, true],
        [false, false, false, false, false, true, true],
        [false, false, true, true, true, true, true],
    ];

    let mut row = 0;
    for restriction in [
        AngleRestriction::NinetyDegree,
        AngleRestriction::FortyFiveDegree,
        AngleRestriction::None,
    ] {
        for geom in GEOMS {
            let mut board = probe_board();
            board.rules.trace_angle_restriction = restriction;
            let mut engine = probe_engine(&mut board, 1);
            let ctrl = probe_control(&board, 1);
            let first = engine.rooms.new_complete_room(
                Some(TileShape::Box(IntBox::from_coords(
                    geom.0 - 500,
                    geom.1 - 500,
                    geom.2,
                    geom.3,
                ))),
                0,
                1,
            );
            let second = engine.rooms.new_complete_room(
                Some(TileShape::Box(IntBox::from_coords(
                    geom.0,
                    geom.1,
                    geom.2 + 500,
                    geom.3 + 500,
                ))),
                0,
                2,
            );
            let door =
                engine
                    .rooms
                    .new_door(RoomRef::Complete(first), RoomRef::Complete(second), 2);
            let maze = MazeSearchEngine::new(&mut engine, &ctrl);
            for (col, width) in WIDTHS.iter().enumerate() {
                assert_eq!(
                    maze.door_is_small(&board, door, *width),
                    EXPECTED[row][col],
                    "restriction={restriction:?} geom={geom:?} width={width}"
                );
            }
            row += 1;
        }
    }
}

/// The two clauses of `:775-777` that answer `false` with no shape at all:
/// ```text
/// doorIsSmall mixedRooms dimension=1 width=1.0e9 => true
/// doorIsSmall mixedRooms dimension=2 width=1.0e9 => false
/// ```
#[test]
fn door_is_small_answers_false_for_a_two_dimensional_door_onto_an_obstacle_room() {
    for (dimension, expected) in [(1, true), (2, false)] {
        let mut board = probe_board();
        let mut engine = probe_engine(&mut board, 1);
        let ctrl = probe_control(&board, 1);
        let tree = engine.tree;
        let free = engine.rooms.new_complete_room(
            Some(TileShape::Box(IntBox::from_coords(0, 0, 1000, 1000))),
            0,
            7,
        );
        let obstacle = engine
            .rooms
            .new_obstacle_room(&mut board, ItemId(4), 0, tree);
        let door = engine.rooms.new_door(
            RoomRef::Complete(free),
            RoomRef::Obstacle(obstacle),
            dimension,
        );
        let maze = MazeSearchEngine::new(&mut engine, &ctrl);
        assert_eq!(maze.door_is_small(&board, door, 1.0e9), expected);
    }
}

// =================================================================================================
// The two static helpers
// =================================================================================================

/// Probe mode `project`.
#[test]
fn segment_projection_matches_the_jvm() {
    let line = |ax: f64, ay: f64, bx: f64, by: f64| {
        FloatLine::new(FloatPoint::new(ax, ay), FloatPoint::new(bx, by))
    };
    let cases: [(FloatLine, FloatLine, Option<FloatLine>); 6] = [
        (
            line(0.0, 0.0, 100.0, 0.0),
            line(0.0, 10.0, 100.0, 10.0),
            Some(line(0.0, 10.0, 100.0, 10.0)),
        ),
        (
            line(0.0, 0.0, 100.0, 0.0),
            line(50.0, 10.0, 200.0, 10.0),
            Some(line(50.0, 10.0, 100.0, 10.0)),
        ),
        (
            line(0.0, 0.0, 100.0, 0.0),
            line(200.0, 10.0, 300.0, 10.0),
            None,
        ),
        (
            line(100.0, 0.0, 0.0, 0.0),
            line(0.0, 10.0, 100.0, 10.0),
            Some(line(0.0, 10.0, 100.0, 10.0)),
        ),
        (
            line(20.0, 0.0, 80.0, 0.0),
            line(0.0, 10.0, 100.0, 10.0),
            Some(line(20.0, 10.0, 80.0, 10.0)),
        ),
        (
            line(0.0, 0.0, 100.0, 100.0),
            line(0.0, 10.0, 100.0, 110.0),
            Some(line(0.0, 10.0, 95.0, 105.0)),
        ),
    ];
    for (from, to, expected) in cases {
        let got = segment_projection(&from, &to);
        match (&got, &expected) {
            (None, None) => {}
            (Some(g), Some(e)) => {
                assert!(
                    (g.a.x - e.a.x).abs() < 1e-9
                        && (g.a.y - e.a.y).abs() < 1e-9
                        && (g.b.x - e.b.x).abs() < 1e-9
                        && (g.b.y - e.b.y).abs() < 1e-9,
                    "from={from:?} to={to:?} got={got:?} expected={expected:?}"
                );
            }
            _ => panic!("from={from:?} to={to:?} got={got:?} expected={expected:?}"),
        }
    }
}

/// `toImpactedPoints` (`:287-298`): `null` in, `null` out; otherwise both ends rounded.
#[test]
fn to_impacted_points_rounds_both_ends_and_passes_null_through() {
    assert_eq!(to_impacted_points(None), None);
    let line = FloatLine::new(
        FloatPoint::new(1.4, -1.4),
        FloatPoint::new(2.5, f64::from(-2.5f32)),
    );
    assert_eq!(
        to_impacted_points(Some(&line)),
        Some([Point::new(1, -1), Point::new(3, -2)])
    );
}

// =================================================================================================
// `reduceTraceShapesAtTiePins` (:154-172)
// =================================================================================================

/// Probe mode `tiepin`:
/// ```text
/// item id=4 PolylineTrace nets=[1] netCount=1
/// item id=3 PolylineTrace nets=[2] netCount=1
/// item id=2 Pin nets=[1, 2] netCount=2
/// before trace id=4 n=1 [-160,-160..2160,160]
/// before trace id=3 n=1 [-160,-160..160,2160]
/// after  trace id=4 n=1 [-160,-160..2160,160]
/// after  trace id=3 n=1 [-160,300..160,2160]
/// ```
#[test]
fn reduce_trace_shapes_at_tie_pins_cuts_only_the_foreign_net_contact() {
    let mut board = tie_pin_board();
    let mut engine = probe_engine(&mut board, 1);
    let tree = engine.tree;

    let shape_of = |board: &mut Board, id: u32| {
        let b = board
            .item_tree_shape(ItemId(id), tree, 0)
            .expect("a tree shape")
            .bounding_box();
        (b.ll.x, b.ll.y, b.ur.x, b.ur.y)
    };
    assert_eq!(shape_of(&mut board, 4), (-160, -160, 2160, 160));
    assert_eq!(shape_of(&mut board, 3), (-160, -160, 160, 2160));

    MazeSearchEngine::reduce_trace_shapes_at_tie_pins(&mut board, &set_of(&[2]), 1, tree);

    assert_eq!(shape_of(&mut board, 4), (-160, -160, 2160, 160));
    assert_eq!(shape_of(&mut board, 3), (-160, 300, 160, 2160));
    let _ = &mut engine;
}
