//! Plan 6 Task 7: the drill pages, the page array and the expansion drills
//! (`autoroute/drill/{DrillPageArray,DrillPage,ExpansionDrill}.java`).
//!
//! # Where the numbers come from
//!
//! Every literal below — the page grids, the overlapping-page sets, the drill counts, the drill
//! shapes and locations, the `getId()` hashes and the room ids each drill binds — is **read off
//! the HEAD jar**, not off this port. The probe is
//! `scripts/differential/java/probes/P6T7Probe.java`, which is committed with the exact
//! `javac`/`java` invocation in its header; it reflects into `DrillPageArray`'s private
//! `pages`/`columnCount`/`rowCount`/`pageWidth`/`pageHeight` and into `DrillPage`'s private
//! `drills`/`netNumber`, which are the state these tests assert on.
//!
//! Each test names its probe mode and pastes the stdout it asserts against.
//!
//! # The seeded incomplete-room list
//!
//! Every drill test calls [`seed_incomplete_list`] first, and that is not decoration:
//! `AutorouteEngine.removeIncompleteExpansionRoom` (`:368-371`) dereferences
//! `incompleteExpansionRooms` with no null guard, and the list is created lazily by
//! `addIncompleteExpansionRoom` (`:344`) — so on an engine that has never had an incomplete room,
//! **every** drill dies with a `NullPointerException` that `completeExpansionRoom`'s catch turns
//! into an empty room list. `a_virgin_engine_yields_no_drills_at_all` pins that (quirk #169); the
//! rest of the file pins the state a real routing run is in by the time the maze reaches a drill
//! page.

use fr_board::prelude::*;
use fr_geometry::{
    Area, IntBox, IntOctagon, IntPoint, IntVector, Point, Polyline, Shape, TileShape,
};
use fr_router::autoroute::drill::{DrillPage, DrillPageArray, ExpansionDrill};
use fr_router::autoroute::expansion::RoomRef;
use fr_router::autoroute::maze::engine::AutorouteEngine;

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

/// `P6T7Probe.build`, which is `P6T3.build`'s any-angle board verbatim: two layers, a 200-unit
/// clearance matrix with a "wide" class, a two-pin component (an **SMD** pad at (-500, 0) on
/// layer 0 only and a **through** pad at (500, 0) on both layers) and two traces, one on net 1
/// and one on net 2.
fn probe_board(bounds: IntBox) -> Board {
    let layers = || LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)]);
    let mut clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
    assert!(clearance_matrix.append_class("wide"));
    clearance_matrix.set_value_on_all_layers(2, 1, 600);
    clearance_matrix.set_value_on_all_layers(2, 2, 800);
    let mut rules = BoardRules::new(layers(), clearance_matrix);
    rules.trace_angle_restriction = AngleRestriction::None;

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
        bounds,
        rules,
        BoardLibrary::new(padstacks, packages),
        components,
        Communication::default(),
    );
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

/// `board.insertObstacle(new IntBox(...), layer, 1, FixedState.UNFIXED)`.
fn insert_obstacle(board: &mut Board, llx: i32, lly: i32, urx: i32, ury: i32, layer: usize) {
    board.insert_obstacle(
        Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            llx, lly, urx, ury,
        )))),
        layer,
        1,
        FixedState::Unfixed,
    );
}

/// `P6T7Probe.seedList`: creates `incompleteExpansionRooms` and leaves it empty. See the module
/// docs — without it every drill dies on quirk #169.
fn seed_incomplete_list(engine: &mut AutorouteEngine) {
    let seed = engine.add_incomplete_expansion_room(
        None,
        0,
        Some(TileShape::Box(IntBox::from_coords(9000, 9000, 9100, 9100))),
    );
    engine.remove_incomplete_expansion_room(seed);
}

/// A ready engine on `board`: `new AutorouteEngine(board, 1, true)` + `initConnection(net, …)` +
/// the seeded list.
fn engine_on(board: &mut Board, net: i32) -> AutorouteEngine {
    let mut engine = AutorouteEngine::new(board, 1, true);
    engine.init_connection(board, net, None);
    seed_incomplete_list(&mut engine);
    engine
}

/// `P6T7Probe.componentPage`: the page around the two-pin component.
fn component_page(board: &Board) -> DrillPage {
    DrillPage::new(IntBox::from_coords(-1000, -1000, 1000, 1000), board)
}

const NEVER: &dyn Fn() -> bool = &|| false;
const ALWAYS: &dyn Fn() -> bool = &|| true;

/// `(llx, lly, urx, ury)`, the shape the probe prints a page box in.
type Box4 = (i32, i32, i32, i32);

/// `(columnCount, rowCount, pageWidth, pageHeight)` plus every page box in row-major order, in
/// the shape the probe prints it.
fn grid(array: &DrillPageArray) -> (Box4, Vec<Box4>) {
    let head = (
        array.column_count(),
        array.row_count(),
        array.page_width(),
        array.page_height(),
    );
    let mut boxes = Vec::new();
    for j in 0..array.row_count() {
        for i in 0..array.column_count() {
            let b = array.page(array.page_id(i, j)).shape;
            boxes.push((b.ll.x, b.ll.y, b.ur.x, b.ur.y));
        }
    }
    (head, boxes)
}

fn page_boxes(array: &DrillPageArray, pages: &[fr_router::arena::PageId]) -> Vec<Box4> {
    pages
        .iter()
        .map(|id| {
            let b = array.page(*id).shape;
            (b.ll.x, b.ll.y, b.ur.x, b.ur.y)
        })
        .collect()
}

/// `(location x, location y, first layer, last layer, getId(), shape bounding box)` for every
/// drill of a page, in list order.
type DrillRow = (i32, i32, usize, usize, i32, Box4);

fn drill_rows(engine: &AutorouteEngine, drills: &[fr_router::arena::DrillId]) -> Vec<DrillRow> {
    drills
        .iter()
        .map(|id| {
            let drill = engine.rooms.drills.get(id.0).expect("a live drill");
            let location = drill.location.to_float().round();
            let b = drill.get_shape().bounding_box();
            (
                location.x,
                location.y,
                drill.first_layer,
                drill.last_layer,
                drill.get_id(),
                (b.ll.x, b.ll.y, b.ur.x, b.ur.y),
            )
        })
        .collect()
}

/// The Java room id (`CompleteFreeSpaceExpansionRoom.getId()`) each layer of a drill binds.
fn drill_room_ids(engine: &AutorouteEngine, drill: &ExpansionDrill) -> Vec<Option<i32>> {
    drill
        .rooms
        .iter()
        .map(|room| match room {
            Some(RoomRef::Complete(id)) => Some(
                engine
                    .rooms
                    .complete_room(*id)
                    .expect("a live room")
                    .get_id(),
            ),
            Some(other) => panic!("a drill bound a non-free-space room: {other:?}"),
            None => None,
        })
        .collect()
}

// =================================================================================================
// `DrillPageArray` (DrillPageArray.java:34-62)
// =================================================================================================

/// Probe mode 0, verbatim:
///
/// ```text
/// square20000 maxPageWidth=7000 columnCount=3 rowCount=3 pageWidth=6667 pageHeight=6667
///     page[0][0] [-10000,-10000..-3333,-3333]
///     …
/// defaultViaDiameter=0.0 maxDrillPageWidth=10000
/// square20000 engineWidth columnCount=2 rowCount=2 pageWidth=10000 pageHeight=10000
/// wide30000x4000 maxPageWidth=10000 columnCount=3 rowCount=1 pageWidth=10000 pageHeight=4000
/// small3000x5000 maxPageWidth=10000 columnCount=1 rowCount=1 pageWidth=3000 pageHeight=5000
/// ```
///
/// The 20 000-unit board over a 7 000-unit page is the one that pins the `ceil` chain of
/// `:37-41`: `columnCount = ceil(20000/7000) = 3`, and `pageWidth` is then **recomputed** as
/// `ceil(20000/3) = 6667` rather than reused, so the last column is 6 666 wide and the first two
/// are 6 667.
#[test]
fn page_grid_matches_java_for_a_known_bounding_box() {
    let board = probe_board(BOUNDING_BOX);
    let array = DrillPageArray::new(&board, 7000);
    let (head, boxes) = grid(&array);
    assert_eq!(head, (3, 3, 6667, 6667));
    assert_eq!(
        boxes,
        vec![
            (-10000, -10000, -3333, -3333),
            (-3333, -10000, 3334, -3333),
            (3334, -10000, 10000, -3333),
            (-10000, -3333, -3333, 3334),
            (-3333, -3333, 3334, 3334),
            (3334, -3333, 10000, 3334),
            (-10000, 3334, -3333, 10000),
            (-3333, 3334, 3334, 10000),
            (3334, 3334, 10000, 10000),
        ]
    );

    // The width `AutorouteEngine`'s constructor computes for this board (`:89-90`): the default
    // via diameter is 0.0, so the `max(…, 10000)` floor wins.
    let array = DrillPageArray::new(&board, 10_000);
    let (head, boxes) = grid(&array);
    assert_eq!(head, (2, 2, 10000, 10000));
    assert_eq!(
        boxes,
        vec![
            (-10000, -10000, 0, 0),
            (0, -10000, 10000, 0),
            (-10000, 0, 0, 10000),
            (0, 0, 10000, 10000),
        ]
    );

    // A wide, short board: `columnCount` and `rowCount` differ, and `pageHeight` is the whole
    // height because one row covers it.
    let board = probe_board(IntBox::from_coords(-15_000, -1000, 15_000, 3000));
    let array = DrillPageArray::new(&board, 10_000);
    let (head, boxes) = grid(&array);
    assert_eq!(head, (3, 1, 10000, 4000));
    assert_eq!(
        boxes,
        vec![
            (-15000, -1000, -5000, 3000),
            (-5000, -1000, 5000, 3000),
            (5000, -1000, 15000, 3000),
        ]
    );

    // A board smaller than one page: one page, and `pageWidth`/`pageHeight` are the board's own.
    let board = probe_board(IntBox::from_coords(0, 0, 3000, 5000));
    let array = DrillPageArray::new(&board, 10_000);
    let (head, boxes) = grid(&array);
    assert_eq!(head, (1, 1, 3000, 5000));
    assert_eq!(boxes, vec![(0, 0, 3000, 5000)]);
}

/// Probe mode 1, verbatim:
///
/// ```text
/// probe [-10000,-10000..1000,1000] minJ=0 maxJ=1.6499175041247938 minI=0 maxI=1.6499175041247938
///   n=4  [-10000,-10000..-3333,-3333] [-3333,-10000..3334,-3333]
///        [-10000,-3333..-3333,3334]   [-3333,-3333..3334,3334]
/// probe [-10000,-10000..-3000,-3000] minJ=0 maxJ=1.0499475026248688 …            n=4
/// probe [-10000,-10000..10000,10000] minJ=0 maxJ=2.999850007499625 …             n=9
/// probe [-1000,-1000..-900,-900]     minJ=1 maxJ=1.3649317534123293 minI=1 …     n=1
/// probe [-3000,-10000..-3000,10000]  minJ=0 maxJ=2.999850007499625 minI=1 maxI=1.0499475026248688
///                                                                                n=0
/// probe [5000,5000..30000,30000]     minJ=2 maxJ=2.999850007499625 …             n=1
/// ```
///
/// The loop bounds are Java's `for (int j = minJ; j < maxJ; j++)` with `maxJ` a **`double`**
/// (DrillPageArray.java:82, `:87`), so `j` is widened per comparison. Truncating `maxJ` to an
/// `int` — the obvious "cleanup" — loses the page the fractional bound sits in: the first probe
/// would answer 1 page instead of 4, and the whole-board probe 4 instead of 9. The fourth probe
/// is the one that also pins `minI`/`minJ` being `floor`ed rather than truncated.
#[test]
fn overlapping_pages_uses_javas_mixed_loop_bounds() {
    let board = probe_board(BOUNDING_BOX);
    let array = DrillPageArray::new(&board, 7000);

    let probe = |llx, lly, urx, ury| {
        page_boxes(
            &array,
            &array.overlapping_pages(&TileShape::Box(IntBox::from_coords(llx, lly, urx, ury))),
        )
    };

    assert_eq!(
        probe(-10000, -10000, 1000, 1000),
        vec![
            (-10000, -10000, -3333, -3333),
            (-3333, -10000, 3334, -3333),
            (-10000, -3333, -3333, 3334),
            (-3333, -3333, 3334, 3334),
        ]
    );
    assert_eq!(probe(-10000, -10000, -3000, -3000).len(), 4);
    assert_eq!(probe(-10000, -10000, 10000, 10000).len(), 9);
    assert_eq!(
        probe(-1000, -1000, -900, -900),
        vec![(-3333, -3333, 3334, 3334)]
    );
    // A 1-dimensional shape: the `intersection.dimension() > 1` guard at `:91` drops every page.
    assert!(probe(-3000, -10000, -3000, 10000).is_empty());
    // A shape sticking out of the board: `:79` intersects the bounding box first.
    assert_eq!(
        probe(5000, 5000, 30000, 30000),
        vec![(3334, 3334, 10000, 10000)]
    );
}

// =================================================================================================
// `DrillPage.getDrills` (DrillPage.java:63-131)
// =================================================================================================

/// Probe modes 2 and 3. With `attachSmd = false` the page yields **13** drills; with
/// `attachSmd = true` it yields **11**, because `:80-84` skips the SMD pin (`drillAllowed()` is
/// true for a padstack that lives on one layer, Pin.java:344-350) and the three little drill
/// shapes wedged around its pad collapse into one.
///
/// Probe mode 2, the four rows that change:
///
/// ```text
/// drill loc=(-481,-206) shape=IntOctagon[-576,-240..-409,-150]
/// drill loc=(-327,-173) shape=IntOctagon[-409,-240..-260,-91]
/// drill loc=(-294,-19)  shape=IntOctagon[-350,-91..-260,76]
/// ```
///
/// Probe mode 3, in their place:
///
/// ```text
/// drill loc=(-378,-121) shape=IntOctagon[-576,-240..-260,76] id=-11377278
/// ```
///
/// The through pin at (500, 0) is **not** skipped in either run: `drillAllowed()` is false for a
/// padstack on two layers, so its pad is cut out whatever `attachSmd` says.
#[test]
fn an_smd_pin_is_cut_out_unless_attach_smd_and_drill_allowed() {
    let mut board = probe_board(BOUNDING_BOX);
    let mut engine = engine_on(&mut board, 1);
    let mut page = component_page(&board);
    let without = page.get_drills(&mut engine, &mut board, false, NEVER);
    assert_eq!(without.len(), 13);
    let without_rows = drill_rows(&engine, &without);
    assert_eq!(
        without_rows[10..],
        [
            (-481, -206, 0, 1, -14527436, (-576, -240, -409, -150)),
            (-327, -173, 0, 1, -9907909, (-409, -240, -260, -91)),
            (-294, -19, 0, 1, -8776812, (-350, -91, -260, 76)),
        ]
    );

    let mut board = probe_board(BOUNDING_BOX);
    let mut engine = engine_on(&mut board, 1);
    let mut page = component_page(&board);
    let with = page.get_drills(&mut engine, &mut board, true, NEVER);
    assert_eq!(with.len(), 11);
    let rows = drill_rows(&engine, &with);
    assert_eq!(
        rows[10],
        (-378, -121, 0, 1, -11377278, (-576, -240, -260, 76))
    );
    // The first ten drills are the same in both runs, so the difference is the SMD pad alone.
    assert_eq!(rows[..10], without_rows[..10]);
}

/// Probe mode 2's first three rows, verbatim:
///
/// ```text
/// drill loc=(35.0,95.0)   layers=0..1 id=1133981   shape=IntBox[-260,-170..330,360]
///     rooms: CompleteFreeSpaceExpansionRoom#7@0 CompleteFreeSpaceExpansionRoom#6@1
/// drill loc=(835.0,125.0) layers=0..1 id=24995611  shape=IntBox[670,-111..1000,360]
///     rooms: CompleteFreeSpaceExpansionRoom#7@0 CompleteFreeSpaceExpansionRoom#6@1
/// drill loc=(370.0,-585.0) layers=0..1 id=10460486 shape=IntBox[-260,-1000..1000,-170]
///     rooms: CompleteFreeSpaceExpansionRoom#7@0 CompleteFreeSpaceExpansionRoom#8@1
/// pageId=-29759999 pageDim=2 mazeElements=2
/// ```
///
/// The `getId()` of the first drill is `31 * (31 * location.getId() + firstLayer) + lastLayer`
/// (`:127-130`) over `IntPoint.getId() = 31 * x + y` — `31 * (31 * (31*35 + 95) + 0) + 1`.
#[test]
fn get_drills_binds_one_room_per_layer_and_hashes_its_location() {
    let mut board = probe_board(BOUNDING_BOX);
    let mut engine = engine_on(&mut board, 1);
    let mut page = component_page(&board);
    let drills = page.get_drills(&mut engine, &mut board, false, NEVER);

    let rows = drill_rows(&engine, &drills);
    assert_eq!(
        rows[..3],
        [
            (35, 95, 0, 1, 1133981, (-260, -170, 330, 360)),
            (835, 125, 0, 1, 24995611, (670, -111, 1000, 360)),
            (370, -585, 0, 1, 10460486, (-260, -1000, 1000, -170)),
        ]
    );
    let first = engine.rooms.drills.get(drills[0].0).expect("a live drill");
    assert_eq!(drill_room_ids(&engine, first), vec![Some(7), Some(6)]);
    let third = engine.rooms.drills.get(drills[2].0).expect("a live drill");
    assert_eq!(drill_room_ids(&engine, third), vec![Some(7), Some(8)]);

    // The `ExpandableObject` half of the page (`:133-151`, `:190-193`).
    assert_eq!(page.get_id(), -29_759_999);
    assert_eq!(page.get_dimension(), 2);
    assert_eq!(page.maze_search_element_count(), 2);
}

/// Probe mode 4: the eight tree entries the page overlaps, and what the cut-out loop
/// (`:73-96`) does with each.
///
/// ```text
/// netNumber=1 overlaps n=8
///     entry PolylineTrace#5/0 shape=IntOctagon[-1340,-240..-260,1440]  CUTOUT
///     entry PolylineTrace#5/1 shape=IntOctagon[-1340,360..840,1440]    CUTOUT
///     entry PolylineTrace#4/0 -> drillable, skipped
///     entry PolylineTrace#4/1 -> drillable, skipped
///     entry PolylineTrace#4/2 -> drillable, skipped
///     entry Pin#3/0 drillAllowed=false shape=IntOctagon[330,-170..670,170] CUTOUT
///     entry Pin#3/1 drillAllowed=false shape=IntOctagon[330,-170..670,170]
///                                                             prevContains -> no cutout
///     entry Pin#2/0 drillAllowed=true  shape=IntOctagon[-650,-150..-350,150] CUTOUT
/// cutouts=4
/// ```
///
/// The through pin's padstack is the **same octagon on both layers**, so its second tree entry is
/// suppressed by `prevObstacleShape.contains(currentObstacleShape)` at `:87` — the carry the
/// comment at `:88-89` explains ("to avoid multiple cutout for example for vias with the same
/// shape on all layers"). The carry starts at `IntBox.EMPTY` (`:72`), which contains nothing, so
/// the first entry is never suppressed.
#[test]
fn the_prev_obstacle_carry_suppresses_duplicate_cutouts_for_a_through_via() {
    let mut board = probe_board(BOUNDING_BOX);
    let engine = engine_on(&mut board, 1);
    let page = component_page(&board);

    let entries = page.obstacle_cutout_trace(&engine, &mut board, false);
    let rows: Vec<(u32, usize, bool, bool)> = entries
        .iter()
        .map(|e| (e.item.0, e.shape_index, e.skipped, e.cut_out))
        .collect();
    assert_eq!(
        rows,
        vec![
            (5, 0, false, true),
            (5, 1, false, true),
            (4, 0, true, false),
            (4, 1, true, false),
            (4, 2, true, false),
            (3, 0, false, true),
            // The through pin's second layer: the same octagon, suppressed by the carry.
            (3, 1, false, false),
            (2, 0, false, true),
        ]
    );
    assert_eq!(entries.iter().filter(|e| e.cut_out).count(), 4);

    // With `attachSmd`, the SMD pin (item 2) is skipped as well and there are three cut-outs.
    let entries = page.obstacle_cutout_trace(&engine, &mut board, true);
    assert_eq!(entries.iter().filter(|e| e.cut_out).count(), 3);
    assert!(entries.last().expect("the SMD pin's entry").skipped);
}

/// Probe mode 7, verbatim:
///
/// ```text
/// fresh    netNumber=-1 id=-29760001 shapeId=-960000
/// afterNet1 netNumber=1 id=-29759999 drills=13
/// afterNet2 netNumber=2 id=-29759998 drills=30
/// afterReset netNumber=2 drills=30
/// afterInvalidate drills=null netNumber=2 id=-29759998
/// ```
///
/// `getDrills` writes `this.netNumber` at `:65` **before** it recomputes, and `getId()`
/// (`:190-193`) is `31 * shape.getId() + netNumber` — so recomputing a page mutates its id. A
/// page already sitting in the maze's `TreeSet<MazeListElement>` (plan-6 ruling 4, hazard B)
/// would therefore sort by a key that no longer matches where it is stored, and the set would
/// neither find nor remove it. Nothing in Plan 6 may "fix" this before parity.
///
/// `reset()` (`:154-164`) resets the maze scratch and each drill's, but leaves the memoised list
/// alone; only `invalidate()` (`:170-172`) drops it — and it does **not** restore `netNumber`, so
/// an invalidated page keeps the mutated id.
#[test]
fn get_drills_recomputes_when_the_net_changes_and_mutates_the_id() {
    let mut board = probe_board(BOUNDING_BOX);
    let mut engine = AutorouteEngine::new(&mut board, 1, false);
    let mut page = component_page(&board);

    // :33 — the field initialiser, before any `getDrills`.
    assert_eq!(page.shape.get_id(), -960_000);
    let fresh_id = page.get_id();
    assert_eq!(fresh_id, -29_760_001);

    engine.init_connection(&mut board, 1, None);
    seed_incomplete_list(&mut engine);
    assert_eq!(
        page.get_drills(&mut engine, &mut board, false, NEVER).len(),
        13
    );
    let net1_id = page.get_id();
    assert_eq!(net1_id, -29_759_999);
    assert_ne!(net1_id, fresh_id);

    engine.init_connection(&mut board, 2, None);
    assert_eq!(
        page.get_drills(&mut engine, &mut board, false, NEVER).len(),
        30
    );
    let net2_id = page.get_id();
    assert_eq!(net2_id, -29_759_998);
    assert_ne!(net2_id, net1_id);

    // `reset` keeps the memo; `invalidate` drops it and keeps the mutated id.
    page.reset(&mut engine.rooms.drills);
    assert_eq!(page.drills().map(<[_]>::len), Some(30));
    page.invalidate();
    assert_eq!(page.drills(), None);
    assert_eq!(page.get_id(), -29_759_998);
}

/// Probe mode 6, verbatim:
///
/// ```text
/// threw java.lang.NullPointerException
///     at app.freerouting.autoroute.drill.DrillPage.getDrills(DrillPage.java:108)
/// afterThrow netNumber(field)=1 drills=0
/// secondCall drills n=0
/// ```
///
/// This is ruling 6's sixth and last cancellation site: `:103` passes
/// `autorouteEngine.stoppableThread` — the raw flag, **not** `isStopRequested()`, so the time
/// limit is not consulted here — to `PolylineArea.splitToConvex`, which returns `null` when the
/// flag trips (PolylineArea.java:189-191). `:108` then dereferences `drillShapes.length` with no
/// null check and throws.
///
/// The damage outlives the throw, which is quirk #168: `:65-66` has already written the new net
/// number and installed a **fresh empty** `drills` list, so the memo now says "this page has no
/// drills on net 1" and `:64`'s guard sends every later call straight past the recomputation.
#[test]
fn split_to_convex_stops_when_the_stop_check_trips() {
    let mut board = probe_board(BOUNDING_BOX);
    let mut engine = engine_on(&mut board, 1);
    let mut page = component_page(&board);

    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        page.get_drills(&mut engine, &mut board, false, ALWAYS)
    }));
    let payload = caught.expect_err("Java throws a NullPointerException at DrillPage.java:108");
    let message = payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .unwrap_or("");
    assert!(
        message.contains("DrillPage.java:108"),
        "the panic must name Java's throw site, got {message:?}"
    );

    // The page is left memoised as empty on the new net, and the next call trusts the memo.
    assert_eq!(page.net_number(), 1);
    assert_eq!(page.drills(), Some(&[][..]));
    assert!(
        page.get_drills(&mut engine, &mut board, false, NEVER)
            .is_empty()
    );
}

// =================================================================================================
// `ExpansionDrill.calculateExpansionRooms` (ExpansionDrill.java:55-92)
// =================================================================================================

/// Probe mode 9, verbatim:
///
/// ```text
/// warm drills n=11
/// upperBlocked calculateExpansionRooms=false
///     upperBlocked roomArr[0]=CompleteFreeSpaceExpansionRoom#10@0
///     upperBlocked roomArr[1]=null
/// ```
///
/// A keepout on layer 1 alone covers the drill location, so layer 0 binds a room and layer 1's
/// `completeExpansionRoom` answers **no** rooms — `newRooms.size() != 1` at `:80-83` — and the
/// method returns false with the layers it already bound still in `roomArr`.
#[test]
fn calculate_expansion_rooms_fails_when_one_layer_is_blocked() {
    let mut board = probe_board(BOUNDING_BOX);
    insert_obstacle(&mut board, 700, -300, 1000, 300, 1);
    let mut engine = engine_on(&mut board, 1);
    let mut page = component_page(&board);
    assert_eq!(
        page.get_drills(&mut engine, &mut board, false, NEVER).len(),
        11
    );

    let mut drill = ExpansionDrill::new(
        TileShape::Box(IntBox::from_coords(785, 75, 885, 175)),
        Point::new(835, 125),
        0,
        1,
    );
    assert!(!drill.calculate_expansion_rooms(&mut engine, &mut board));
    assert_eq!(drill_room_ids(&engine, &drill), vec![Some(10), None]);
}

/// Probe mode 5, verbatim:
///
/// ```text
/// blocked calculateExpansionRooms=false
///     blocked roomArr[0]=null
///     blocked roomArr[1]=null
/// blocked getId=1 dim=2 mazeElements=2 otherRoom=null
/// layer0 calculateExpansionRooms=true
///     layer0 roomArr[0]=CompleteFreeSpaceExpansionRoom#4@0
/// layer0 getId=0 mazeElements=1
/// free calculateExpansionRooms=true
///     free roomArr[0]=CompleteFreeSpaceExpansionRoom#6@0
///     free roomArr[1]=CompleteFreeSpaceExpansionRoom#7@1
/// free getId=24995611
/// again calculateExpansionRooms=true
///     again roomArr[0]=CompleteFreeSpaceExpansionRoom#6@0
///     again roomArr[1]=CompleteFreeSpaceExpansionRoom#7@1
/// ```
///
/// Three facts in one run: a location where `completeExpansionRoom` answers more than one room
/// fails on the **first** layer with nothing bound; `roomArr` is sized `lastLayer - firstLayer +
/// 1`, so a single-layer drill has one slot and one `MazeSearchElement`; and a second drill at
/// the same location finds the rooms the first one created, through `overlappingObjects`
/// (`:57-73`) rather than by building new ones.
#[test]
fn calculate_expansion_rooms_reuses_the_rooms_that_are_already_in_the_tree() {
    let mut board = probe_board(BOUNDING_BOX);
    insert_obstacle(&mut board, -200, -200, 200, 200, 1);
    let mut engine = engine_on(&mut board, 1);

    let mut blocked = ExpansionDrill::new(
        TileShape::Box(IntBox::from_coords(-50, -50, 50, 50)),
        Point::new(0, 0),
        0,
        1,
    );
    assert!(!blocked.calculate_expansion_rooms(&mut engine, &mut board));
    assert_eq!(drill_room_ids(&engine, &blocked), vec![None, None]);
    assert_eq!(blocked.get_id(), 1);
    assert_eq!(blocked.get_dimension(), 2);
    assert_eq!(blocked.maze_search_element_count(), 2);
    assert_eq!(
        blocked.other_room(RoomRef::Complete(fr_board::RoomId(0))),
        None
    );

    let mut layer0 = ExpansionDrill::new(
        TileShape::Box(IntBox::from_coords(-50, -50, 50, 50)),
        Point::new(0, 0),
        0,
        0,
    );
    assert!(layer0.calculate_expansion_rooms(&mut engine, &mut board));
    assert_eq!(drill_room_ids(&engine, &layer0), vec![Some(4)]);
    assert_eq!(layer0.get_id(), 0);
    assert_eq!(layer0.maze_search_element_count(), 1);

    let free_shape = TileShape::Box(IntBox::from_coords(785, 75, 885, 175));
    let mut free = ExpansionDrill::new(free_shape.clone(), Point::new(835, 125), 0, 1);
    assert!(free.calculate_expansion_rooms(&mut engine, &mut board));
    assert_eq!(drill_room_ids(&engine, &free), vec![Some(6), Some(7)]);
    assert_eq!(free.get_id(), 24_995_611);

    let mut again = ExpansionDrill::new(free_shape, Point::new(835, 125), 0, 1);
    assert!(again.calculate_expansion_rooms(&mut engine, &mut board));
    assert_eq!(drill_room_ids(&engine, &again), vec![Some(6), Some(7)]);
}

/// Probe mode 8, verbatim:
///
/// ```text
/// virgin drills n=0
/// virgin calculateExpansionRooms=false
/// seeded calculateExpansionRooms=false
/// ```
///
/// Quirk #169. `AutorouteEngine.removeIncompleteExpansionRoom` (`:368-371`) is
/// `removeAllDoors(room); incompleteExpansionRooms.remove(room);` with **no null guard**, and
/// `incompleteExpansionRooms` is created lazily by `addIncompleteExpansionRoom` (`:342-345`). So
/// on an engine that has never had an incomplete room added, `completeExpansionRoom`'s `:469`
/// throws a `NullPointerException`, its own `catch` at `:518-521` turns that into an empty room
/// collection, and `ExpansionDrill.calculateExpansionRooms:80-83` reads the empty collection as
/// "blocked" and drops the drill. Every drill on the page dies the same way and the page memoises
/// an empty list.
///
/// The `seeded` line is the control: the same location still answers false once the list exists,
/// because on a tree with no rooms in it yet `completeExpansionRoom` answers more than one room
/// there — which is why every other test in this file warms the database with `getDrills` first.
#[test]
fn a_virgin_engine_yields_no_drills_at_all() {
    let mut board = probe_board(BOUNDING_BOX);
    let mut engine = AutorouteEngine::new(&mut board, 1, true);
    engine.init_connection(&mut board, 1, None);
    // No `seed_incomplete_list` here — that is the whole point.
    let mut page = component_page(&board);
    assert!(
        page.get_drills(&mut engine, &mut board, false, NEVER)
            .is_empty()
    );

    let mut drill = ExpansionDrill::new(
        TileShape::Box(IntBox::from_coords(785, 75, 885, 175)),
        Point::new(835, 125),
        0,
        1,
    );
    assert!(!drill.calculate_expansion_rooms(&mut engine, &mut board));

    seed_incomplete_list(&mut engine);
    let mut after = ExpansionDrill::new(
        TileShape::Box(IntBox::from_coords(785, 75, 885, 175)),
        Point::new(835, 125),
        0,
        1,
    );
    assert!(!after.calculate_expansion_rooms(&mut engine, &mut board));
}

// =================================================================================================
// The engine's three drill hooks (AutorouteEngine.java:91, :597-600, :668)
// =================================================================================================

/// Probe mode 10, verbatim:
///
/// ```text
/// engineArray columnCount=1 rowCount=1 pageWidth=2000 pageHeight=2000
///     page[0][0] [-1000,-1000..1000,1000]
/// page00 [-1000,-1000..1000,1000] drills=9
/// page00 memo=9
/// afterInvalidateDrillPages memo=null
/// recomputed drills=10
/// afterReset memo=10
/// afterMissingInvalidate memo=10
/// ```
///
/// The board's bounding box is one page wide here, so the engine's own array is 1x1 and its
/// single page is the component page. On the full -10 000..10 000 board the engine's pages are
/// 10 000 units wide, and completing a room in one of them trips quirk #162's non-terminating
/// `calculateNewIncompleteRooms` — the probe OOMs there, so this is the largest engine-owned page
/// the ground truth can cover.
///
/// Three facts: `invalidateDrillPages` (`:597-600`) reaches the page through the array and drops
/// its memo; a shape that misses the board's bounding box invalidates **nothing**, because
/// `overlappingPages` intersects with the bounds first (`:79`); and the recomputation answers
/// **ten** drills where the first answered nine, because the rooms the first pass created are in
/// the search tree by then and change which drill locations resolve to a single room.
#[test]
fn the_engine_reaches_its_pages_through_invalidate_drill_pages() {
    let mut board = probe_board(IntBox::from_coords(-1000, -1000, 1000, 1000));
    let mut engine = engine_on(&mut board, 1);
    assert_eq!(engine.max_drill_page_width, 10_000);
    let (head, boxes) = grid(engine.drill_pages());
    assert_eq!(head, (1, 1, 2000, 2000));
    assert_eq!(boxes, vec![(-1000, -1000, 1000, 1000)]);

    let page = engine.drill_pages().page_id(0, 0);
    assert_eq!(
        engine
            .drill_page_drills(&mut board, page, false, NEVER)
            .len(),
        9
    );
    assert_eq!(
        engine.drill_pages().page(page).drills().map(<[_]>::len),
        Some(9)
    );

    engine.invalidate_drill_pages(&TileShape::Box(IntBox::from_coords(-900, -900, -800, -800)));
    assert_eq!(engine.drill_pages().page(page).drills(), None);

    assert_eq!(
        engine
            .drill_page_drills(&mut board, page, false, NEVER)
            .len(),
        10
    );
    // A shape outside the board's bounding box selects no page at all.
    engine.invalidate_drill_pages(&TileShape::Box(IntBox::from_coords(
        -9000, -9000, -8000, -8000,
    )));
    assert_eq!(
        engine.drill_pages().page(page).drills().map(<[_]>::len),
        Some(10)
    );
}

/// `resetAllDoors`' last line (`:668`) resets every page and, through `DrillPage.reset:156-160`,
/// every drill on it — the maze scratch only. Probe mode 10's `afterReset memo=10` is the other
/// half: `reset` is not `invalidate`, and the memoised drill list survives it.
#[test]
fn reset_all_doors_resets_the_pages_but_keeps_their_drills() {
    let mut board = probe_board(IntBox::from_coords(-1000, -1000, 1000, 1000));
    let mut engine = engine_on(&mut board, 1);
    let page = engine.drill_pages().page_id(0, 0);
    let drills = engine.drill_page_drills(&mut board, page, false, NEVER);
    assert_eq!(drills.len(), 9);

    // Dirty the maze scratch on the page and on one of its drills.
    engine
        .drill_pages_mut()
        .page_mut(page)
        .get_maze_search_element_mut(0)
        .is_occupied = true;
    engine
        .rooms
        .drills
        .get_mut(drills[0].0)
        .expect("a live drill")
        .get_maze_search_element_mut(0)
        .is_occupied = true;

    engine.reset_all_doors(&mut board);

    assert!(
        !engine
            .drill_pages()
            .page(page)
            .get_maze_search_element(0)
            .is_occupied
    );
    assert!(
        !engine
            .rooms
            .drills
            .get(drills[0].0)
            .expect("a live drill")
            .get_maze_search_element(0)
            .is_occupied
    );
    assert_eq!(
        engine.drill_pages().page(page).drills().map(<[_]>::len),
        Some(9)
    );
}
