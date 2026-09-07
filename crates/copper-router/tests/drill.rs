use copper_board::prelude::*;
use copper_geometry::{
    Area, IntBox, IntOctagon, IntPoint, IntVector, Point, Polyline, Shape, TileShape,
};
use copper_router::autoroute::drill::{DrillPage, DrillPageArray, ExpansionDrill};
use copper_router::autoroute::expansion::{ExpansionRoomStore, RoomRef};
use copper_router::autoroute::maze::engine::AutorouteEngine;

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

fn seed_incomplete_list(engine: &mut AutorouteEngine) {
    let seed = engine.add_incomplete_expansion_room(
        None,
        0,
        Some(TileShape::Box(IntBox::from_coords(9000, 9000, 9100, 9100))),
    );
    engine.remove_incomplete_expansion_room(seed);
}

fn engine_on(board: &mut Board, net: i32) -> AutorouteEngine {
    let mut engine = AutorouteEngine::new(board, 1, true);
    engine.init_connection(board, net, None);
    seed_incomplete_list(&mut engine);
    engine
}

fn component_page(board: &Board) -> DrillPage {
    DrillPage::new(IntBox::from_coords(-1000, -1000, 1000, 1000), board, 1)
}

const NEVER: &dyn Fn() -> bool = &|| false;
const ALWAYS: &dyn Fn() -> bool = &|| true;

type Box4 = (i32, i32, i32, i32);

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

fn page_boxes(array: &DrillPageArray, pages: &[copper_router::arena::PageId]) -> Vec<Box4> {
    pages
        .iter()
        .map(|id| {
            let b = array.page(*id).shape;
            (b.ll.x, b.ll.y, b.ur.x, b.ur.y)
        })
        .collect()
}

type DrillRow = (i32, i32, usize, usize, i32, Box4);

fn drill_rows(engine: &AutorouteEngine, drills: &[copper_router::arena::DrillId]) -> Vec<DrillRow> {
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

#[test]
fn page_grid_matches_java_for_a_known_bounding_box() {
    let board = probe_board(BOUNDING_BOX);
    let array = DrillPageArray::new(&board, 7000, &mut ExpansionRoomStore::new());
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

    let array = DrillPageArray::new(&board, 10_000, &mut ExpansionRoomStore::new());
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

    let board = probe_board(IntBox::from_coords(-15_000, -1000, 15_000, 3000));
    let array = DrillPageArray::new(&board, 10_000, &mut ExpansionRoomStore::new());
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

    let board = probe_board(IntBox::from_coords(0, 0, 3000, 5000));
    let array = DrillPageArray::new(&board, 10_000, &mut ExpansionRoomStore::new());
    let (head, boxes) = grid(&array);
    assert_eq!(head, (1, 1, 3000, 5000));
    assert_eq!(boxes, vec![(0, 0, 3000, 5000)]);
}

#[test]
fn overlapping_pages_include_boundary_contacts() {
    let board = probe_board(BOUNDING_BOX);
    let array = DrillPageArray::new(&board, 7000, &mut ExpansionRoomStore::new());

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
    assert_eq!(
        probe(-3000, -10000, -3000, 10000),
        vec![
            (-3333, -10000, 3334, -3333),
            (-3333, -3333, 3334, 3334),
            (-3333, 3334, 3334, 10000),
        ]
    );
    assert_eq!(
        probe(5000, 5000, 30000, 30000),
        vec![(3334, 3334, 10000, 10000)]
    );
}

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
        (-379, -122, 0, 1, -11408030, (-576, -240, -260, 76))
    );
    assert_eq!(rows[..10], without_rows[..10]);
}

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
    let first_rooms = drill_room_ids(&engine, first);
    assert_eq!(first_rooms, vec![Some(29), Some(23)]);
    let third = engine.rooms.drills.get(drills[2].0).expect("a live drill");
    let third_rooms = drill_room_ids(&engine, third);
    assert_eq!(third_rooms, vec![Some(29), Some(34)]);
    assert_eq!(
        first_rooms[0], third_rooms[0],
        "probe: both drills bind the SAME layer-0 room"
    );
    assert_ne!(
        first_rooms[1], third_rooms[1],
        "probe: and different layer-1 rooms"
    );

    assert_eq!(page.id(), -29_759_999);
    assert_eq!(page.get_dimension(), 2);
    assert_eq!(page.maze_search_element_count(), 2);
}

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
            (3, 1, false, false),
            (2, 0, false, true),
        ]
    );
    assert_eq!(entries.iter().filter(|e| e.cut_out).count(), 4);

    let entries = page.obstacle_cutout_trace(&engine, &mut board, true);
    assert_eq!(entries.iter().filter(|e| e.cut_out).count(), 3);
    assert!(entries.last().expect("the SMD pin's entry").skipped);
}

#[test]
fn get_drills_recomputes_when_the_net_changes_and_mutates_javas_id() {
    let mut board = probe_board(BOUNDING_BOX);
    let mut engine = AutorouteEngine::new(&mut board, 1, false);
    let mut page = component_page(&board);

    assert_eq!(page.shape.get_id(), -960_000);
    let fresh_id = page.id();
    assert_eq!(fresh_id, -29_760_001);
    let stable_id = page.get_id();

    engine.init_connection(&mut board, 1, None);
    seed_incomplete_list(&mut engine);
    assert_eq!(
        page.get_drills(&mut engine, &mut board, false, NEVER).len(),
        13
    );
    let net1_id = page.id();
    assert_eq!(net1_id, -29_759_999);
    assert_ne!(net1_id, fresh_id);
    assert_eq!(
        page.get_id(),
        stable_id,
        "fixed: T8 (#167) — the port's id does not move"
    );

    engine.init_connection(&mut board, 2, None);
    assert_eq!(
        page.get_drills(&mut engine, &mut board, false, NEVER).len(),
        30
    );
    let net2_id = page.id();
    assert_eq!(net2_id, -29_759_998);
    assert_ne!(net2_id, net1_id);
    assert_eq!(
        page.get_id(),
        stable_id,
        "fixed: T8 (#167) — nor on a second net"
    );

    page.reset(&mut engine.rooms.drills);
    assert_eq!(page.drills().map(<[_]>::len), Some(30));
    page.invalidate(&mut engine.rooms.drills);
    assert_eq!(page.drills(), None);
    assert_eq!(page.id(), -29_759_998);
    assert_eq!(
        page.get_id(),
        stable_id,
        "fixed: T8 (#167) — nor across invalidate"
    );
}

#[test]
fn a_stopped_split_does_not_memoise_an_empty_page() {
    let mut board = probe_board(BOUNDING_BOX);
    let mut engine = engine_on(&mut board, 1);
    let mut page = component_page(&board);

    assert_eq!(page.drills(), None);
    let before = page.net_number();

    let cancelled = page.get_drills(&mut engine, &mut board, false, ALWAYS);
    assert!(
        cancelled.is_empty(),
        "a cancelled page has no drills to report"
    );

    assert_eq!(
        page.drills(),
        None,
        "`:65-66`'s writes are deferred past `:103`, so a cancelled split memoises nothing"
    );
    assert_eq!(
        page.net_number(),
        before,
        "the net number is written with the list, not before the work"
    );

    assert_eq!(
        page.get_drills(&mut engine, &mut board, false, NEVER).len(),
        13,
        "the page recomputes instead of answering the memo — mode 2's count"
    );
    assert_eq!(page.net_number(), 1);
}

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
    assert_eq!(drill_room_ids(&engine, &drill), vec![Some(33), None]);
}

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
        blocked.other_room(RoomRef::Complete(copper_board::RoomId(0))),
        None
    );

    let mut layer0 = ExpansionDrill::new(
        TileShape::Box(IntBox::from_coords(-50, -50, 50, 50)),
        Point::new(0, 0),
        0,
        0,
    );
    assert!(layer0.calculate_expansion_rooms(&mut engine, &mut board));
    assert_eq!(drill_room_ids(&engine, &layer0), vec![Some(11)]);
    assert_eq!(layer0.get_id(), 0);
    assert_eq!(layer0.maze_search_element_count(), 1);

    let free_shape = TileShape::Box(IntBox::from_coords(785, 75, 885, 175));
    let mut free = ExpansionDrill::new(free_shape.clone(), Point::new(835, 125), 0, 1);
    assert!(free.calculate_expansion_rooms(&mut engine, &mut board));
    assert_eq!(drill_room_ids(&engine, &free), vec![Some(22), Some(25)]);
    assert_eq!(free.get_id(), 24_995_611);

    let mut again = ExpansionDrill::new(free_shape, Point::new(835, 125), 0, 1);
    assert!(again.calculate_expansion_rooms(&mut engine, &mut board));
    assert_eq!(drill_room_ids(&engine, &again), vec![Some(22), Some(25)]);
}

#[test]
fn a_virgin_engine_yields_thirteen_drills() {
    let mut board = probe_board(BOUNDING_BOX);
    let mut engine = AutorouteEngine::new(&mut board, 1, true);
    engine.init_connection(&mut board, 1, None);
    assert!(
        !engine.rooms.incomplete_list_created(),
        "the engine is virgin: `incompleteExpansionRooms` is still Java's null"
    );

    let mut page = component_page(&board);
    let drills = page.get_drills(&mut engine, &mut board, false, NEVER);
    assert_eq!(
        drills.len(),
        13,
        "mode 2's seeded count, now reached without seeding — the fix's headline, 0 -> 13"
    );
    assert!(
        engine.rooms.incomplete_list_created(),
        "`ExpansionDrill.calculateExpansionRooms` now goes through addIncompleteExpansionRoom, \
         so the list exists by the time the first drill is built"
    );

    let mut seeded_board = probe_board(BOUNDING_BOX);
    let mut seeded_engine = engine_on(&mut seeded_board, 1);
    let mut seeded_page = component_page(&seeded_board);
    let seeded_drills = seeded_page.get_drills(&mut seeded_engine, &mut seeded_board, false, NEVER);
    assert_eq!(seeded_drills.len(), 13);
    assert_eq!(
        drill_rows(&engine, &drills),
        drill_rows(&seeded_engine, &seeded_drills),
        "a virgin engine now builds the same thirteen drills a seeded one does — same locations, \
         same layers, same getId() hashes, same shapes"
    );
    assert_eq!(
        drill_rows(&engine, &drills)[10..],
        [
            (-481, -206, 0, 1, -14527436, (-576, -240, -409, -150)),
            (-327, -173, 0, 1, -9907909, (-409, -240, -260, -91)),
            (-294, -19, 0, 1, -8776812, (-350, -91, -260, 76)),
        ]
    );

    let mut cold_board = probe_board(BOUNDING_BOX);
    let mut cold = AutorouteEngine::new(&mut cold_board, 1, true);
    cold.init_connection(&mut cold_board, 1, None);
    let mut drill = ExpansionDrill::new(
        TileShape::Box(IntBox::from_coords(785, 75, 885, 175)),
        Point::new(835, 125),
        0,
        1,
    );
    assert!(!drill.calculate_expansion_rooms(&mut cold, &mut cold_board));

    let mut seeded_board = probe_board(BOUNDING_BOX);
    let mut seeded = AutorouteEngine::new(&mut seeded_board, 1, true);
    seeded.init_connection(&mut seeded_board, 1, None);
    seed_incomplete_list(&mut seeded);
    let mut after = ExpansionDrill::new(
        TileShape::Box(IntBox::from_coords(785, 75, 885, 175)),
        Point::new(835, 125),
        0,
        1,
    );
    assert!(!after.calculate_expansion_rooms(&mut seeded, &mut seeded_board));
}

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
    engine.invalidate_drill_pages(&TileShape::Box(IntBox::from_coords(
        -9000, -9000, -8000, -8000,
    )));
    assert_eq!(
        engine.drill_pages().page(page).drills().map(<[_]>::len),
        Some(10)
    );
}

#[test]
fn reset_all_doors_resets_the_pages_but_keeps_their_drills() {
    let mut board = probe_board(IntBox::from_coords(-1000, -1000, 1000, 1000));
    let mut engine = engine_on(&mut board, 1);
    let page = engine.drill_pages().page_id(0, 0);
    let drills = engine.drill_page_drills(&mut board, page, false, NEVER);
    assert_eq!(drills.len(), 9);

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

#[test]
fn invalidating_a_page_frees_its_drills_arena_slots() {
    let mut board = probe_board(IntBox::from_coords(-1000, -1000, 1000, 1000));
    let mut engine = engine_on(&mut board, 1);
    let page = engine.drill_pages().page_id(0, 0);
    let small = TileShape::Box(IntBox::from_coords(-900, -900, -800, -800));

    assert_eq!(engine.rooms.drills.len(), 0);
    assert_eq!(
        engine
            .drill_page_drills(&mut board, page, false, NEVER)
            .len(),
        9
    );
    assert_eq!(engine.rooms.drills.len(), 9);

    engine.invalidate_drill_pages(&small);
    assert_eq!(engine.drill_pages().page(page).drills(), None);
    assert_eq!(engine.rooms.drills.len(), 0);

    assert_eq!(
        engine
            .drill_page_drills(&mut board, page, false, NEVER)
            .len(),
        10
    );
    assert_eq!(engine.rooms.drills.len(), 10);

    engine.invalidate_drill_pages(&small);
    assert_eq!(
        engine
            .drill_page_drills(&mut board, page, false, NEVER)
            .len(),
        10
    );
    assert_eq!(engine.rooms.drills.len(), 10);
    assert_eq!(engine.rooms.drills.slot_count(), 29);

    let first = engine.drill_page_drills(&mut board, page, false, NEVER);
    let second = engine.drill_page_drills(&mut board, page, false, NEVER);
    assert_eq!(first, second);
    assert_eq!(engine.rooms.drills.len(), 10);
}

#[test]
fn recomputing_for_a_new_net_frees_the_previous_nets_drills() {
    let mut board = probe_board(BOUNDING_BOX);
    let mut engine = AutorouteEngine::new(&mut board, 1, false);
    seed_incomplete_list(&mut engine);
    let mut page = component_page(&board);

    engine.init_connection(&mut board, 1, None);
    assert_eq!(
        page.get_drills(&mut engine, &mut board, false, NEVER).len(),
        13
    );
    assert_eq!(engine.rooms.drills.len(), 13);

    engine.init_connection(&mut board, 2, None);
    assert_eq!(
        page.get_drills(&mut engine, &mut board, false, NEVER).len(),
        30
    );
    assert_eq!(engine.rooms.drills.len(), 30);
    assert_eq!(engine.rooms.drills.slot_count(), 43);
}
