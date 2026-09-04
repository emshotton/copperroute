//! Tests for the two angle-restricted neighbour sorters (Plan 6 Task 5):
//! `crates/fr-router/src/autoroute/expansion/sorted_neighbours_45.rs` and
//! `.../sorted_neighbours_orthogonal.rs`, plus the dispatch in
//! [`SortedRoomNeighbours::complete`] that now reaches three different implementations.
//!
//! The fixed scripts are **not** replays of a generator: the board, its obstacles, the three seed
//! rooms and every call's inputs are literals read off `scripts/differential/java/P6T3.java`'s own
//! stdout (`run.sh p6t3 6 42 30 1000`, `7 42 30 1000`, `8 42 20 1000`, `9 42 20 1000`), and the
//! expected lines are that stdout verbatim. So each one reproduces the Java output from scratch.

use fr_board::ids::TreeObject;
use fr_board::prelude::*;
use fr_dsn::format::java_double_to_string;
use fr_geometry::{Area, IntBox, IntOctagon, IntVector, Point, Polyline, Shape, TileShape};
use fr_router::JavaTreeSet;
use fr_router::autoroute::expansion::sorted_neighbours::SortedRoomNeighbours;
use fr_router::autoroute::expansion::sorted_neighbours_45::Sorted45DegreeRoomNeighbours;
use fr_router::autoroute::expansion::sorted_neighbours_orthogonal::SortedOrthogonalRoomNeighbours;
use fr_router::autoroute::expansion::{
    ExpansionRoomStore, IncompleteFreeSpaceExpansionRoom, RoomRef,
};
use fr_router::autoroute::item_info;
use fr_router::autoroute::tree_ext::AutorouteSearchTreeExt;

// =================================================================================================
// The `p6t3` board, in whichever angle regime the caller asks for
// =================================================================================================

/// The thirty grid obstacles seed 42 draws for `p6t3 4/6/7 42 30 …` — the same numbers in all
/// three regimes, because the obstacle stream is drawn before the tree is built. Read off the
/// `item id=` lines of `p6t3 6 42 30 1000`.
const GRID_OBSTACLES: [(i32, i32, i32, i32, usize); 30] = [
    (-3500, 0, -2000, 2000, 0),
    (-500, 7000, 1500, 9000, 1),
    (3000, -7000, 5000, -6500, 1),
    (5500, 6500, 7500, 7000, 0),
    (8000, 6000, 10000, 7500, 1),
    (6500, -8000, 7500, -7000, 0),
    (-3000, 6500, -2000, 8500, 0),
    (4500, -5500, 5000, -3500, 1),
    (-1500, 0, -500, 1000, 1),
    (6500, 4500, 8500, 5500, 0),
    (3000, 1500, 5000, 2000, 0),
    (-5500, 0, -3500, 500, 0),
    (7500, -8000, 8500, -6500, 1),
    (6000, 8000, 6500, 8500, 0),
    (7500, 4000, 8000, 4500, 1),
    (3000, 3000, 5000, 3500, 1),
    (4000, 7500, 5000, 9500, 1),
    (-6000, -5000, -5000, -4000, 1),
    (5000, 2500, 6500, 3500, 0),
    (-6500, -500, -6000, 1000, 1),
    (-8000, 5500, -7500, 7500, 1),
    (-500, 7000, 1500, 8500, 0),
    (500, 7500, 1500, 9500, 0),
    (-8000, -8000, -7500, -6500, 0),
    (5500, 1500, 7000, 3500, 0),
    (0, -6000, 1000, -4500, 0),
    (3000, 1500, 4000, 2000, 0),
    (1000, 7500, 2000, 9000, 1),
    (6000, 7000, 8000, 8000, 0),
    (-3000, -6500, -1500, -6000, 0),
];

/// The twenty ungridded obstacles seed 42 draws for `p6t3 5/8/9 42 20 …`, read off the `item id=`
/// lines of `p6t3 8 42 20 1000`.
const RANDOM_OBSTACLES: [(i32, i32, i32, i32, usize); 20] = [
    (8496, 4150, 9056, 6521, 0),
    (-3980, -333, -1897, 700, 1),
    (5966, 4944, 8451, 6759, 1),
    (-5966, 5831, -5138, 7132, 0),
    (-4976, 2554, -3810, 3700, 1),
    (-4147, 7504, -3492, 8411, 0),
    (-3968, 8539, -2604, 10079, 0),
    (-5816, 5285, -4137, 7397, 1),
    (4427, 432, 6018, 2925, 1),
    (-5859, 1509, -5383, 3660, 0),
    (-209, 6836, 449, 7816, 0),
    (7485, 2949, 7672, 3378, 0),
    (8407, 5395, 9709, 7594, 1),
    (8739, 2522, 10425, 2733, 0),
    (-1504, 7993, 926, 9633, 1),
    (-4656, 4358, -4386, 6625, 1),
    (-6393, 705, -5996, 1847, 1),
    (-4849, -7918, -3659, -7284, 1),
    (-5955, -3513, -5309, -1234, 0),
    (4098, -8459, 5378, -6908, 1),
];

/// `seedRoom 1..3` of `p6t3 6 42 30 1000` (and of modes 4 and 7 — the same stream).
const GRID_SEED_ROOMS: [(i32, i32, i32, i32, usize); 3] = [
    (-2188, 4586, 247, 6855, 0),
    (-1858, 6349, -1018, 7935, 1),
    (-5674, 8222, -3634, 10595, 0),
];

/// `seedRoom 1..3` of `p6t3 8 42 20 1000` (and of modes 5 and 9).
const RANDOM_SEED_ROOMS: [(i32, i32, i32, i32, usize); 3] = [
    (1386, -2517, 2807, 458, 1),
    (2811, 8147, 3647, 10698, 0),
    (-1114, 1885, 790, 4531, 0),
];

/// The `P2T10` board of `P6T3.java` (two layers, a two-pin component, two traces, an empty
/// outline) in the given angle regime, plus the named obstacle list and the autoroute tree.
///
/// This duplicates `tests/sorted_neighbours.rs`'s builder rather than sharing it, because that one
/// hard-codes the any-angle regime and its own thirty obstacles; both are literal transcriptions
/// of the same Java driver, and the angle is the whole point here.
fn p6t3_board(
    angle: AngleRestriction,
    obstacles: &[(i32, i32, i32, i32, usize)],
) -> (Board, TreeId) {
    let layers = || LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)]);
    let mut clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
    assert!(clearance_matrix.append_class("wide"));
    clearance_matrix.set_value_on_all_layers(2, 1, 600);
    clearance_matrix.set_value_on_all_layers(2, 2, 800);
    let mut rules = BoardRules::new(layers(), clearance_matrix);
    rules.trace_angle_restriction = angle;

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
        IntBox::from_coords(-10_000, -10_000, 10_000, 10_000),
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

    for &(llx, lly, urx, ury, layer) in obstacles {
        board.insert_obstacle(
            Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
                llx, lly, urx, ury,
            )))),
            layer,
            1,
            FixedState::Unfixed,
        );
    }

    // `searchTreeManager.getAutorouteTree(1)`, over the item list in board order (descending id).
    let tree_id = {
        let mut items = std::mem::take(&mut board.items);
        let mut manager = std::mem::take(&mut board.trees);
        let id = {
            let ctx = board.ctx();
            let mut refs: Vec<&mut Item> = items.values_mut().rev().collect();
            manager.get_autoroute_tree(1, &mut refs, &ctx).id()
        };
        board.items = items;
        board.trees = manager;
        id
    };
    (board, tree_id)
}

/// The three `CompleteFreeSpaceExpansionRoom`s `P6T3.insertSeedRooms` puts in the tree. The two
/// obstacle sets draw different ones, so the caller names them.
fn p6t3_seed_rooms(
    board: &mut Board,
    tree_id: TreeId,
    seeds: [(i32, i32, i32, i32, usize); 3],
) -> ExpansionRoomStore {
    let mut rooms = ExpansionRoomStore::new();
    for (index, (llx, lly, urx, ury, layer)) in seeds.into_iter().enumerate() {
        let id_no = rooms.next_room_id_no();
        assert_eq!(id_no, index as i32 + 1);
        let room = rooms.new_complete_room(
            Some(TileShape::Box(IntBox::from_coords(llx, lly, urx, ury))),
            layer,
            id_no,
        );
        let tree = board
            .trees
            .trees_mut()
            .find(|tree| tree.id() == tree_id)
            .expect("the autoroute tree");
        rooms.insert_complete_room(tree, room);
    }
    rooms
}

/// `AutorouteEngine.completeExpansionRoom`'s seed: `completeShape` of a whole-plane room around a
/// small contained box, of which the driver picks candidate `pick`.
#[allow(clippy::too_many_arguments)]
fn seed_free_space_room(
    board: &Board,
    rooms: &mut ExpansionRoomStore,
    tree_id: TreeId,
    layer: usize,
    contained: IntBox,
    net_no: i32,
    pick: usize,
    expected_candidates: usize,
) -> RoomRef {
    let seed = IncompleteFreeSpaceExpansionRoom::new(None, layer, Some(TileShape::Box(contained)));
    let completed = {
        let ctx = board.ctx();
        let tree = board
            .trees
            .trees()
            .find(|tree| tree.id() == tree_id)
            .expect("the autoroute tree");
        tree.complete_shape(&seed, net_no, None, None, &board.items, &*rooms, &ctx)
    };
    assert_eq!(completed.len(), expected_candidates, "candidates=");
    let chosen = &completed[pick];
    RoomRef::Incomplete(rooms.new_incomplete_room(
        chosen.get_shape().cloned(),
        chosen.get_layer(),
        chosen.get_contained_shape().cloned(),
    ))
}

// =================================================================================================
// Rendering, byte-for-byte with `scripts/differential/rust/src/bin/p6t3.rs`
// =================================================================================================

fn shp(s: &TileShape) -> String {
    match s {
        TileShape::Box(x) => format!("Box[{},{}..{},{}]", x.ll.x, x.ll.y, x.ur.x, x.ur.y),
        TileShape::Octagon(o) => format!(
            "Oct[{},{},{},{},{},{},{},{}]",
            o.left_x,
            o.bottom_y,
            o.right_x,
            o.top_y,
            o.upper_left_diagonal_x,
            o.lower_right_diagonal_x,
            o.lower_left_diagonal_x,
            o.upper_right_diagonal_x
        ),
        TileShape::Simplex(sx) => {
            let mut out = String::from("Simplex{");
            for i in 0..sx.border_line_count() {
                if let Some(line) = sx.border_line(i) {
                    out.push_str(&format!("({}->{})", line.a, line.b));
                }
            }
            out.push('}');
            out
        }
    }
}

fn opt_shp(s: Option<&TileShape>) -> String {
    s.map_or("null".to_string(), shp)
}

fn corners(shape: Option<&TileShape>) -> String {
    let Some(shape) = shape else {
        return "null".to_string();
    };
    let mut out = String::from("(");
    for (i, corner) in shape.corner_approx_arr().iter().enumerate() {
        if i > 0 {
            out.push(';');
        }
        out.push_str(&java_double_to_string(corner.x));
        out.push(',');
        out.push_str(&java_double_to_string(corner.y));
    }
    out.push(')');
    out
}

fn desc(room: RoomRef, rooms: &ExpansionRoomStore) -> String {
    match room {
        RoomRef::Complete(id) => format!(
            "cfsr{}",
            rooms.complete_room(id).expect("a live room").get_id()
        ),
        RoomRef::Obstacle(id) => {
            let r = rooms.obstacle_room(id).expect("a live room");
            format!("obs{}/{}", r.get_item().0, r.get_index_in_item())
        }
        RoomRef::Incomplete(id) => format!(
            "inc{}",
            rooms.incomplete_room(id).expect("a live room").get_id()
        ),
    }
}

fn describe_object(object: TreeObject, rooms: &ExpansionRoomStore) -> String {
    match object {
        TreeObject::Item(id) => format!("item{}", id.0),
        TreeObject::Room(id) => format!(
            "cfsr{}",
            rooms
                .complete_room(id)
                .map_or(i32::from(id.0 as u16), |room| room.get_id())
        ),
    }
}

/// `P6T3.dumpRegimeNeighbours` for the 45-degree inner class, plus everything the call left on the
/// completed room.
fn dump_45(
    result: &Sorted45DegreeRoomNeighbours,
    rooms: &ExpansionRoomStore,
    out: &mut Vec<String>,
) {
    out.push(format!("  neighbours n={}", result.sorted_neighbours.len()));
    for (i, n) in result.sorted_neighbours.iter().enumerate() {
        out.push(format!(
            "    [{i}] fts={} lts={} obj={} nshape={} nshapeCorners={} isect={} isectCorners={}",
            n.first_touching_side,
            n.last_touching_side,
            describe_object(n.search_tree_object, rooms),
            shp(&TileShape::Octagon(n.shape)),
            corners(Some(&TileShape::Octagon(n.shape))),
            shp(&TileShape::Octagon(n.intersection)),
            corners(Some(&TileShape::Octagon(n.intersection)))
        ));
    }
    out.push(format!(
        "  edgeTouches={}",
        flags(&result.edge_interior_touches_obstacle)
    ));
    out.push(format!(
        "  completedRoom={}",
        desc(result.completed_room, rooms)
    ));
    dump_doors(result.completed_room, rooms, out);
    dump_target_doors(result.completed_room, rooms, out);
}

/// `P6T3.dumpRegimeNeighbours` for the orthogonal inner class.
fn dump_orthogonal(
    result: &SortedOrthogonalRoomNeighbours,
    rooms: &ExpansionRoomStore,
    out: &mut Vec<String>,
) {
    out.push(format!("  neighbours n={}", result.sorted_neighbours.len()));
    for (i, n) in result.sorted_neighbours.iter().enumerate() {
        out.push(format!(
            "    [{i}] fts={} lts={} obj={} nshape={} nshapeCorners={} isect={} isectCorners={}",
            n.first_touching_side,
            n.last_touching_side,
            describe_object(n.search_tree_object, rooms),
            shp(&TileShape::Box(n.shape)),
            corners(Some(&TileShape::Box(n.shape))),
            shp(&TileShape::Box(n.intersection)),
            corners(Some(&TileShape::Box(n.intersection)))
        ));
    }
    out.push(format!(
        "  edgeTouches={}",
        flags(&result.edge_interior_touches_obstacle)
    ));
    out.push(format!(
        "  completedRoom={}",
        desc(result.completed_room, rooms)
    ));
    dump_doors(result.completed_room, rooms, out);
    dump_target_doors(result.completed_room, rooms, out);
}

fn dump_doors(room: RoomRef, rooms: &ExpansionRoomStore, out: &mut Vec<String>) {
    let doors = rooms.room_doors(room).to_vec();
    out.push(format!("  doors n={}", doors.len()));
    for (i, door_id) in doors.iter().enumerate() {
        let door = rooms.door(*door_id).expect("a live door");
        let (first, second, dimension) = (door.first_room, door.second_room, door.dimension);
        let shape = rooms.door_shape(*door_id);
        out.push(format!(
            "    [{i}] first={} second={} dim={dimension} shape={} corners={}",
            desc(first, rooms),
            desc(second, rooms),
            opt_shp(shape.as_ref()),
            corners(shape.as_ref())
        ));
    }
}

fn dump_target_doors(room: RoomRef, rooms: &ExpansionRoomStore, out: &mut Vec<String>) {
    let target_doors = rooms.room_target_doors(room).to_vec();
    out.push(format!("  targetDoors n={}", target_doors.len()));
    for (i, door_id) in target_doors.iter().enumerate() {
        let door = rooms.target_door(*door_id).expect("a live target door");
        out.push(format!(
            "    [{i}] item={} entry={} dim={} shape={}",
            door.item.0,
            door.tree_entry_no,
            door.get_dimension(),
            shp(door.get_shape())
        ));
    }
}

/// `P6T3.dumpIncompleteRooms`, minus the arena slots the driver filters out as its own seed.
fn dump_incomplete_rooms(rooms: &ExpansionRoomStore, skip: &[u32], out: &mut Vec<String>) {
    let engine_rooms: Vec<u32> = rooms
        .incomplete_rooms
        .iter()
        .map(|(index, _)| index)
        .filter(|index| !skip.contains(index))
        .collect();
    out.push(format!("  incompleteRooms n={}", engine_rooms.len()));
    for (i, index) in engine_rooms.iter().enumerate() {
        let current = rooms
            .incomplete_room(fr_router::IncompleteRoomId(*index))
            .expect("a live room");
        out.push(format!(
            "    [{i}] layer={} shape={} corners={} contained={} doors={}",
            current.get_layer(),
            opt_shp(current.get_shape()),
            corners(current.get_shape()),
            opt_shp(current.get_contained_shape()),
            current.get_doors().len()
        ));
    }
}

fn flags(values: &[bool]) -> String {
    let mut out = String::from("[");
    for (i, value) in values.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(if *value { "true" } else { "false" });
    }
    out.push(']');
    out
}

/// Compares one produced script with the expected one, line by line.
///
/// `FR_DUMP_SCRIPT=1` prints what was actually produced, prefixed `DUMP|`, instead of only saying
/// which line differs. It exists because these scripts are re-cut by hand whenever an authorized
/// divergence moves one, and doing that from a first-differing-line message is how a re-cut goes
/// wrong. Added at Plan 9 Task 8 (it cut `an_eight_sided_obstacle_room_gets_eight_doors`); named
/// for what it does rather than for the task, because the next re-cut will want it too.
///
/// It is a **read** — it prints and changes no assertion — so an unset variable and a set one
/// compare exactly the same thing.
fn assert_script(actual: &[String], expected: &str) {
    if std::env::var_os("FR_DUMP_SCRIPT").is_some() {
        for line in actual {
            eprintln!("DUMP|{line}");
        }
    }
    let expected: Vec<&str> = expected.trim_matches('\n').lines().collect();
    for (i, line) in expected.iter().enumerate() {
        assert_eq!(
            actual.get(i).map(String::as_str),
            Some(*line),
            "line {i} of the Java script"
        );
    }
    assert_eq!(actual.len(), expected.len(), "line count");
}

// =================================================================================================
// The 45-degree sorter — `p6t3 6 42 30 1000`, `call i=7`
// =================================================================================================

#[test]
fn the_45_degree_sorter_matches_the_p6t3_script() {
    // `run.sh p6t3 6 42 30 1000`, `call i=7`, verbatim:
    //
    //   call i=7 kind=freeSpace layer=0 contained=Box[-5996,7255..-5783,7396] candidates=1 pick=0
    //     chosenContained=Oct[-5996,7255,-5783,7396,-13392,-13038,1259,1613] net=3 roomIdNo=11
    //     shape=Oct[-10000,2100,-3100,8222,-18222,-5200,-7900,5122] roomLayer=0
    //
    // Three neighbours, two board items and one of the seed rooms; every shape and intersection an
    // `IntOctagon`, and `edgeInteriorTouchesObstacle` with five of its eight flags set — the array
    // `tryRemoveEdgeLine` reads.
    let (mut board, tree_id) = p6t3_board(AngleRestriction::FortyFiveDegree, &GRID_OBSTACLES);
    let mut rooms = p6t3_seed_rooms(&mut board, tree_id, GRID_SEED_ROOMS);
    let room = seed_free_space_room(
        &board,
        &mut rooms,
        tree_id,
        0,
        IntBox::from_coords(-5996, 7255, -5783, 7396),
        3,
        0,
        1,
    );
    assert_eq!(
        opt_shp(rooms.room_shape(room)),
        "Oct[-10000,2100,-3100,8222,-18222,-5200,-7900,5122]"
    );

    let result = Sorted45DegreeRoomNeighbours::calculate_neighbours(
        room, 3, &mut board, &mut rooms, tree_id, 11,
    )
    .expect("an incomplete room completes");
    let mut actual = Vec::new();
    dump_45(&result, &rooms, &mut actual);
    assert_script(
        &actual,
        r#"
  neighbours n=3
    [0] fts=0 lts=2 obj=item6 nshape=Oct[-3600,-100,-1900,2100,-5641,-1859,-3641,141] nshapeCorners=(-3541.0,-100.0;-1959.0,-100.0;-1900.0,-41.0;-1900.0,2041.0;-1959.0,2100.0;-3541.0,2100.0;-3600.0,2041.0;-3600.0,-41.0) isect=Oct[-3541,2100,-3100,2100,-5641,-5200,-1441,-1000] isectCorners=(-3541.0,2100.0;-3100.0,2100.0;-3100.0,2100.0;-3100.0,2100.0;-3100.0,2100.0;-3541.0,2100.0;-3541.0,2100.0;-3541.0,2100.0)
    [1] fts=2 lts=4 obj=item12 nshape=Oct[-3100,6400,-1900,8600,-11641,-8359,3359,6641] nshapeCorners=(-3041.0,6400.0;-1959.0,6400.0;-1900.0,6459.0;-1900.0,8541.0;-1959.0,8600.0;-3041.0,8600.0;-3100.0,8541.0;-3100.0,6459.0) isect=Oct[-3100,6459,-3100,8222,-11322,-9559,3359,5122] isectCorners=(-3100.0,6459.0;-3100.0,6459.0;-3100.0,6459.0;-3100.0,8222.0;-3100.0,8222.0;-3100.0,8222.0;-3100.0,8222.0;-3100.0,6459.0)
    [2] fts=4 lts=4 obj=cfsr3 nshape=Oct[-5674,8222,-3634,10595,-16269,-11856,2548,6961] nshapeCorners=(-5674.0,8222.0;-3634.0,8222.0;-3634.0,8222.0;-3634.0,10595.0;-3634.0,10595.0;-5674.0,10595.0;-5674.0,10595.0;-5674.0,8222.0) isect=Oct[-5674,8222,-3634,8222,-13896,-11856,2548,4588] isectCorners=(-5674.0,8222.0;-3634.0,8222.0;-3634.0,8222.0;-3634.0,8222.0;-3634.0,8222.0;-5674.0,8222.0;-5674.0,8222.0;-5674.0,8222.0)
  edgeTouches=[true,true,true,true,true,false,false,false]
  completedRoom=cfsr11
  doors n=1
    [0] first=cfsr11 second=cfsr3 dim=1 shape=Oct[-5674,8222,-3634,8222,-13896,-11856,2548,4588] corners=(-5674.0,8222.0;-3634.0,8222.0;-3634.0,8222.0;-3634.0,8222.0;-3634.0,8222.0;-5674.0,8222.0;-5674.0,8222.0;-5674.0,8222.0)
  targetDoors n=0
"#,
    );
}

// =================================================================================================
// The orthogonal sorter — `p6t3 7 42 30 1000`, `call i=4` and `call i=62`
// =================================================================================================

#[test]
fn the_orthogonal_sorter_matches_the_p6t3_script() {
    // `run.sh p6t3 7 42 30 1000`, `call i=4`, verbatim:
    //
    //   call i=4 kind=freeSpace layer=1 contained=Box[-5947,599..-5730,882] candidates=1 pick=0
    //     chosenContained=Box[-5900,599..-5730,882] net=3 roomIdNo=8
    //     shape=Box[-5900,-3900..-1600,6349] roomLayer=1
    //
    // Four neighbours, every shape an `IntBox`: two touch one side (`fts == lts`), one wraps side 1
    // to side 2 and the last wraps side 3 round to side 0 — which is what the comparator's
    // `(lastTouchingSide - firstTouchingSide + 4) % 4` span is for.
    let (mut board, tree_id) = p6t3_board(AngleRestriction::NinetyDegree, &GRID_OBSTACLES);
    let mut rooms = p6t3_seed_rooms(&mut board, tree_id, GRID_SEED_ROOMS);
    let room = seed_free_space_room(
        &board,
        &mut rooms,
        tree_id,
        1,
        IntBox::from_coords(-5947, 599, -5730, 882),
        3,
        0,
        1,
    );
    assert_eq!(
        opt_shp(rooms.room_shape(room)),
        "Box[-5900,-3900..-1600,6349]"
    );

    let result = SortedOrthogonalRoomNeighbours::calculate_neighbours(
        room, 3, &mut board, &mut rooms, tree_id, 8,
    )
    .expect("an incomplete room completes");
    let mut actual = Vec::new();
    dump_orthogonal(&result, &rooms, &mut actual);
    assert_script(
        &actual,
        r#"
  neighbours n=4
    [0] fts=1 lts=1 obj=item14 nshape=Box[-1600,-100..-400,1100] nshapeCorners=(-1600.0,-100.0;-400.0,-100.0;-400.0,1100.0;-1600.0,1100.0) isect=Box[-1600,-100..-1600,1100] isectCorners=(-1600.0,-100.0;-1600.0,-100.0;-1600.0,1100.0;-1600.0,1100.0)
    [1] fts=1 lts=2 obj=cfsr2 nshape=Box[-1858,6349..-1018,7935] nshapeCorners=(-1858.0,6349.0;-1018.0,6349.0;-1018.0,7935.0;-1858.0,7935.0) isect=Box[-1858,6349..-1600,6349] isectCorners=(-1858.0,6349.0;-1600.0,6349.0;-1600.0,6349.0;-1858.0,6349.0)
    [2] fts=3 lts=3 obj=item25 nshape=Box[-6600,-600..-5900,1100] nshapeCorners=(-6600.0,-600.0;-5900.0,-600.0;-5900.0,1100.0;-6600.0,1100.0) isect=Box[-5900,-600..-5900,1100] isectCorners=(-5900.0,-600.0;-5900.0,-600.0;-5900.0,1100.0;-5900.0,1100.0)
    [3] fts=3 lts=0 obj=item23 nshape=Box[-6100,-5100..-4900,-3900] nshapeCorners=(-6100.0,-5100.0;-4900.0,-5100.0;-4900.0,-3900.0;-6100.0,-3900.0) isect=Box[-5900,-3900..-4900,-3900] isectCorners=(-5900.0,-3900.0;-4900.0,-3900.0;-4900.0,-3900.0;-5900.0,-3900.0)
  edgeTouches=[true,true,true,true]
  completedRoom=cfsr8
  doors n=1
    [0] first=cfsr8 second=cfsr2 dim=1 shape=Box[-1858,6349..-1600,6349] corners=(-1858.0,6349.0;-1600.0,6349.0;-1600.0,6349.0;-1858.0,6349.0)
  targetDoors n=0
"#,
    );
}

#[test]
fn an_own_net_object_becomes_a_target_door_inside_the_neighbour_loop() {
    // `run.sh p6t3 7 42 30 1000`, `call i=62`, verbatim. This is the structural difference the two
    // angle-restricted sorters have from the base class:
    // `CompleteFreeSpaceExpansionRoom.calculateTargetDoors` runs **inside** the neighbour loop
    // (`SortedOrthogonalRoomNeighbours.java:155`), one tree entry at a time, where the base class
    // defers every own-net object to a list and processes it after `calculate`
    // (`SortedRoomNeighbours.java:132-134`, `:158-185`). Item 3 — the through-hole pin on the
    // routed net 1 — therefore never becomes a neighbour and yields a target door instead.
    let (mut board, tree_id) = p6t3_board(AngleRestriction::NinetyDegree, &GRID_OBSTACLES);
    let mut rooms = p6t3_seed_rooms(&mut board, tree_id, GRID_SEED_ROOMS);
    let room = seed_free_space_room(
        &board,
        &mut rooms,
        tree_id,
        1,
        IntBox::from_coords(2478, 1079, 2529, 1235),
        1,
        0,
        1,
    );
    assert_eq!(
        opt_shp(rooms.room_shape(room)),
        "Box[-400,-3400..10000,2900]"
    );

    let result = SortedOrthogonalRoomNeighbours::calculate_neighbours(
        room, 1, &mut board, &mut rooms, tree_id, 66,
    )
    .expect("an incomplete room completes");
    let mut actual = Vec::new();
    dump_orthogonal(&result, &rooms, &mut actual);
    assert_script(
        &actual,
        r#"
  neighbours n=3
    [0] fts=0 lts=0 obj=item13 nshape=Box[4400,-5600..5100,-3400] nshapeCorners=(4400.0,-5600.0;5100.0,-5600.0;5100.0,-3400.0;4400.0,-3400.0) isect=Box[4400,-3400..5100,-3400] isectCorners=(4400.0,-3400.0;5100.0,-3400.0;5100.0,-3400.0;4400.0,-3400.0)
    [1] fts=2 lts=2 obj=item21 nshape=Box[2900,2900..5100,3600] nshapeCorners=(2900.0,2900.0;5100.0,2900.0;5100.0,3600.0;2900.0,3600.0) isect=Box[2900,2900..5100,2900] isectCorners=(2900.0,2900.0;5100.0,2900.0;5100.0,2900.0;2900.0,2900.0)
    [2] fts=3 lts=3 obj=item14 nshape=Box[-1600,-100..-400,1100] nshapeCorners=(-1600.0,-100.0;-400.0,-100.0;-400.0,1100.0;-1600.0,1100.0) isect=Box[-400,-100..-400,1100] isectCorners=(-400.0,-100.0;-400.0,-100.0;-400.0,1100.0;-400.0,1100.0)
  edgeTouches=[true,false,true,true]
  completedRoom=cfsr66
  doors n=0
  targetDoors n=1
    [0] item=3 entry=1 dim=2 shape=Box[330,-170..670,170]
"#,
    );
    // And the room is net-dependent, because `calculateTargetDoors` sets that unconditionally
    // (`CompleteFreeSpaceExpansionRoom.java:134`) — the half of the method the base class's static
    // namesake only performs when its deferred list is non-empty (`SortedRoomNeighbours.java:162`).
    let RoomRef::Complete(completed) = result.completed_room else {
        panic!("a free-space room completes to a CompleteFreeSpaceExpansionRoom")
    };
    assert!(
        rooms
            .complete_room(completed)
            .expect("a live room")
            .is_net_dependent()
    );
}

// =================================================================================================
// Quirk #163 — `calculateEdgeIncompleteRoomsOfObstacleExpansionRoom` skipped its last side
// =================================================================================================

#[test]
fn an_eight_sided_obstacle_room_gets_eight_doors() {
    // `run.sh p6t3 8 42 20 1000`, `call i=4` — the whole of
    // `Sorted45DegreeRoomNeighbours.calculate` for an obstacle room over the second trace's first
    // segment. Its shape has **eight distinct corners** and no touching neighbour at all, so
    // `calculate:73` runs `calculateEdgeIncompleteRoomsOfObstacleExpansionRoom(0, 7)` — which in
    // the jar produces **seven** incomplete rooms, not eight.
    //
    // That is quirk #163: `:264` computes `currentCorner = roomShape.corner(fromSideIndex)` and
    // the loop body never reassigns it, so the `!currentCorner.equals(nextCorner)` guard at `:269`
    // compares each side's *end* corner against the corner the walk started at rather than against
    // the side's own start corner. On a full 0..7 walk that is true for every side but the last,
    // whose end corner **is** `corner(0)`. The jar's doors run round sides 0..6 —
    // `(-1024,-240)→(-576,-240)`, `…→(-260,76)`, `…→(-260,1124)`, `…→(-576,1440)`,
    // `…→(-1024,1440)`, `…→(-1340,1124)`, `…→(-1340,76)` — and the eighth side,
    // `(-1340,76)→(-1024,-240)`, got no room at all.
    //
    // **fixed: T8 (#163)** — `currentCorner = nextCorner` at the foot of the loop. **7 -> 8**
    // incomplete rooms and **8 -> 9** doors (the extra one beside the dimension-2 door to
    // `obs5/1`). This is a **KNOWN DIVERGENCE from the jar authorized by #163**: the jar's script
    // said `doors n=8` / `incompleteRooms n=7` and its seven room literals are the seven below,
    // unchanged; `doors[8]` and `incompleteRooms[7]` are the port's, and they are the two shapes
    // `docs/plan-9-prep/fixtures/task-8/expected-outcomes.md` derives **by hand** from the
    // octagon's own geometry, without the jar:
    //
    //     eighth door  Oct[-1340,-240,-1024,76,-1416,-784,-1264,-1264]   dim 1
    //     eighth room  Oct[-10000,-10000,8736,8736,-18736,18736,-20000,-1264]
    //
    // Both match the port to the unit, which is an independent confirmation of the fix rather
    // than a re-cut of its output.
    //
    // The invariant behind the literal, so a future octagon needs no new derivation: an obstacle
    // expansion room whose octagon has **k distinct corners** and no touching neighbour gets **k**
    // edge incomplete rooms, one per side, and each side's door is that side's segment. No side of
    // a non-degenerate octagon is unreachable. `k -> k`; `7 -> 8` is this instance.
    let (mut board, tree_id) = p6t3_board(AngleRestriction::FortyFiveDegree, &RANDOM_OBSTACLES);
    let mut rooms = p6t3_seed_rooms(&mut board, tree_id, RANDOM_SEED_ROOMS);
    let obstacle = {
        let (b, r) = (&mut board, &mut rooms);
        item_info::get_expansion_room(b, ItemId(5), 0, tree_id, |b, i, idx, t| {
            r.new_obstacle_room(b, i, idx, t)
        })
    }
    .expect("the second trace has a first tree shape");
    let room = RoomRef::Obstacle(obstacle);
    assert_eq!(
        opt_shp(rooms.room_shape(room)),
        "Oct[-1340,-240,-260,1440,-2464,-336,-1264,864]"
    );

    let result = SortedRoomNeighbours::complete(room, 1, &mut board, &mut rooms, tree_id)
        .expect("an obstacle room completes");
    let mut actual = vec![format!(
        "  result={} shape={} corners={}",
        desc(result, &rooms),
        opt_shp(rooms.room_shape(result)),
        corners(rooms.room_shape(result))
    )];
    dump_doors(result, &rooms, &mut actual);
    dump_target_doors(result, &rooms, &mut actual);
    dump_incomplete_rooms(&rooms, &[], &mut actual);
    // PORT-REGRESSION PINS in the script below, `accepted at plan9-t7t8 (ruling CC)`: the
    // `incN` tokens. Java's `IncompleteFreeSpaceExpansionRoom.getId` is the hash
    // `31 * shape.getId() + layer` over a MUTABLE shape (quirk #158, hazard C), so the jar printed
    // eight wide hash values here. #158's fix at Task 8 gives the room an
    // `id_no` drawn from the engine's shared counter as it enters the arena, and the port prints
    // the counter's small consecutive ids instead. Nothing else in the script moved: the door
    // count, every door's dimension, every shape and every corner list is the jar's to the digit,
    // and so is the `incompleteRooms` block below.
    //
    // Java's arithmetic is not lost — it survives as `IncompleteFreeSpaceExpansionRoom::java_id`,
    // which still panics on the whole-plane room exactly where Java NPEs, with its own tests
    // beside it. The jar's tokens, for the record:
    // `inc-475676864 inc943169552 inc1803269220 inc-1549944256 inc-1656480944 inc184106368`
    // `inc-1738579036 inc-1129576944` -> `inc8 inc9 inc10 inc11 inc12 inc13 inc14 inc15`.
    assert_script(
        &actual,
        r#"
  result=obs5/0 shape=Oct[-1340,-240,-260,1440,-2464,-336,-1264,864] corners=(-1024.0,-240.0;-576.0,-240.0;-260.0,76.0;-260.0,1124.0;-576.0,1440.0;-1024.0,1440.0;-1340.0,1124.0;-1340.0,76.0)
  doors n=9
    [0] first=obs5/0 second=obs5/1 dim=2 shape=Oct[-1340,360,-260,1440,-2464,-620,-664,864] corners=(-1024.0,360.0;-260.0,360.0;-260.0,360.0;-260.0,1124.0;-576.0,1440.0;-1024.0,1440.0;-1340.0,1124.0;-1340.0,676.0)
    [1] first=obs5/0 second=inc8 dim=1 shape=Oct[-1024,-240,-576,-240,-784,-336,-1264,-816] corners=(-1024.0,-240.0;-576.0,-240.0;-576.0,-240.0;-576.0,-240.0;-576.0,-240.0;-1024.0,-240.0;-1024.0,-240.0;-1024.0,-240.0)
    [2] first=obs5/0 second=inc9 dim=1 shape=Oct[-576,-240,-260,76,-336,-336,-816,-184] corners=(-576.0,-240.0;-576.0,-240.0;-260.0,76.0;-260.0,76.0;-260.0,76.0;-260.0,76.0;-576.0,-240.0;-576.0,-240.0)
    [3] first=obs5/0 second=inc10 dim=1 shape=Oct[-260,76,-260,1124,-1384,-336,-184,864] corners=(-260.0,76.0;-260.0,76.0;-260.0,76.0;-260.0,1124.0;-260.0,1124.0;-260.0,1124.0;-260.0,1124.0;-260.0,76.0)
    [4] first=obs5/0 second=inc11 dim=1 shape=Oct[-576,1124,-260,1440,-2016,-1384,864,864] corners=(-260.0,1124.0;-260.0,1124.0;-260.0,1124.0;-260.0,1124.0;-576.0,1440.0;-576.0,1440.0;-576.0,1440.0;-576.0,1440.0)
    [5] first=obs5/0 second=inc12 dim=1 shape=Oct[-1024,1440,-576,1440,-2464,-2016,416,864] corners=(-1024.0,1440.0;-576.0,1440.0;-576.0,1440.0;-576.0,1440.0;-576.0,1440.0;-1024.0,1440.0;-1024.0,1440.0;-1024.0,1440.0)
    [6] first=obs5/0 second=inc13 dim=1 shape=Oct[-1340,1124,-1024,1440,-2464,-2464,-216,416] corners=(-1340.0,1124.0;-1340.0,1124.0;-1024.0,1440.0;-1024.0,1440.0;-1024.0,1440.0;-1024.0,1440.0;-1340.0,1124.0;-1340.0,1124.0)
    [7] first=obs5/0 second=inc14 dim=1 shape=Oct[-1340,76,-1340,1124,-2464,-1416,-1264,-216] corners=(-1340.0,76.0;-1340.0,76.0;-1340.0,76.0;-1340.0,1124.0;-1340.0,1124.0;-1340.0,1124.0;-1340.0,1124.0;-1340.0,76.0)
    [8] first=obs5/0 second=inc15 dim=1 shape=Oct[-1340,-240,-1024,76,-1416,-784,-1264,-1264] corners=(-1024.0,-240.0;-1024.0,-240.0;-1024.0,-240.0;-1024.0,-240.0;-1340.0,76.0;-1340.0,76.0;-1340.0,76.0;-1340.0,76.0)
  targetDoors n=0
  incompleteRooms n=8
    [0] layer=0 shape=Oct[-10000,-10000,10000,-240,-9760,20000,-20000,9760] corners=(-10000.0,-10000.0;10000.0,-10000.0;10000.0,-10000.0;10000.0,-240.0;10000.0,-240.0;-10000.0,-240.0;-10000.0,-240.0;-10000.0,-10000.0) contained=Oct[-1024,-240,-576,-240,-784,-336,-1264,-816] doors=1
    [1] layer=0 shape=Oct[-10000,-10000,10000,10000,-336,20000,-20000,20000] corners=(-10000.0,-10000.0;10000.0,-10000.0;10000.0,-10000.0;10000.0,10000.0;10000.0,10000.0;9664.0,10000.0;-10000.0,-9664.0;-10000.0,-10000.0) contained=Oct[-576,-240,-260,76,-336,-336,-816,-184] doors=1
    [2] layer=0 shape=Oct[-260,-10000,10000,10000,-10260,20000,-10260,20000] corners=(-260.0,-10000.0;10000.0,-10000.0;10000.0,-10000.0;10000.0,10000.0;10000.0,10000.0;-260.0,10000.0;-260.0,10000.0;-260.0,-10000.0) contained=Oct[-260,76,-260,1124,-1384,-336,-184,864] doors=1
    [3] layer=0 shape=Oct[-9136,-9136,10000,10000,-19136,19136,864,20000] corners=(10000.0,-9136.0;10000.0,-9136.0;10000.0,-9136.0;10000.0,10000.0;10000.0,10000.0;-9136.0,10000.0;-9136.0,10000.0;-9136.0,10000.0) contained=Oct[-576,1124,-260,1440,-2016,-1384,864,864] doors=1
    [4] layer=0 shape=Oct[-10000,1440,10000,10000,-20000,8560,-8560,20000] corners=(-10000.0,1440.0;10000.0,1440.0;10000.0,1440.0;10000.0,10000.0;10000.0,10000.0;-10000.0,10000.0;-10000.0,10000.0;-10000.0,1440.0) contained=Oct[-1024,1440,-576,1440,-2464,-2016,416,864] doors=1
    [5] layer=0 shape=Oct[-10000,-7536,7536,10000,-20000,-2464,-17536,17536] corners=(-10000.0,-7536.0;-10000.0,-7536.0;7536.0,10000.0;7536.0,10000.0;7536.0,10000.0;-10000.0,10000.0;-10000.0,10000.0;-10000.0,-7536.0) contained=Oct[-1340,1124,-1024,1440,-2464,-2464,-216,416] doors=1
    [6] layer=0 shape=Oct[-10000,-10000,-1340,10000,-20000,8660,-20000,8660] corners=(-10000.0,-10000.0;-1340.0,-10000.0;-1340.0,-10000.0;-1340.0,10000.0;-1340.0,10000.0;-10000.0,10000.0;-10000.0,10000.0;-10000.0,-10000.0) contained=Oct[-1340,76,-1340,1124,-2464,-1416,-1264,-216] doors=1
    [7] layer=0 shape=Oct[-10000,-10000,8736,8736,-18736,18736,-20000,-1264] corners=(-10000.0,-10000.0;8736.0,-10000.0;8736.0,-10000.0;8736.0,-10000.0;-10000.0,8736.0;-10000.0,8736.0;-10000.0,8736.0;-10000.0,-10000.0) contained=Oct[-1340,-240,-1024,76,-1416,-784,-1264,-1264] doors=1
"#,
    );

    // The invariant, asserted rather than only described.
    let shape = rooms.room_shape(result).expect("a shape").clone();
    let distinct: std::collections::BTreeSet<(u64, u64)> = shape
        .corner_approx_arr()
        .iter()
        .map(|corner| (corner.x.to_bits(), corner.y.to_bits()))
        .collect();
    assert_eq!(distinct.len(), 8, "a non-degenerate octagon");
    let edge_doors = rooms
        .room_doors(result)
        .iter()
        .filter(|door| rooms.door(**door).is_some_and(|door| door.dimension == 1))
        .count();
    assert_eq!(
        edge_doors,
        distinct.len(),
        "k distinct corners -> k edge doors; the jar answers k - 1"
    );
}

// =================================================================================================
// The orthogonal empty-neighbour path — `p6t3 9 42 20 1000`, `call i=83`
// =================================================================================================

#[test]
fn an_orthogonal_obstacle_room_with_no_neighbours_gets_one_room_per_board_side() {
    // `run.sh p6t3 9 42 20 1000`, `call i=83`, verbatim.
    // `SortedOrthogonalRoomNeighbours.calculateIncompleteRoomsWithEmptyNeighbours` (`:79-108`)
    // builds **four** rooms, one per side of the board's bounding box, and a `dimension == 1` door
    // to each — with no `insertDoorOk` test, no dimension test and no `isEmpty` guard, all three
    // of which the base class's namesake (SortedRoomNeighbours.java:138-156) applies per room.
    let (mut board, tree_id) = p6t3_board(AngleRestriction::NinetyDegree, &RANDOM_OBSTACLES);
    let mut rooms = p6t3_seed_rooms(&mut board, tree_id, RANDOM_SEED_ROOMS);
    let obstacle = {
        let (b, r) = (&mut board, &mut rooms);
        item_info::get_expansion_room(b, ItemId(2), 0, tree_id, |b, i, idx, t| {
            r.new_obstacle_room(b, i, idx, t)
        })
    }
    .expect("the first pin has a first tree shape");
    let room = RoomRef::Obstacle(obstacle);
    assert_eq!(opt_shp(rooms.room_shape(room)), "Box[-650,-150..-350,150]");

    let result = SortedRoomNeighbours::complete(room, 3, &mut board, &mut rooms, tree_id)
        .expect("an obstacle room completes");
    let mut actual = vec![format!(
        "  result={} shape={} corners={}",
        desc(result, &rooms),
        opt_shp(rooms.room_shape(result)),
        corners(rooms.room_shape(result))
    )];
    dump_doors(result, &rooms, &mut actual);
    dump_target_doors(result, &rooms, &mut actual);
    dump_incomplete_rooms(&rooms, &[], &mut actual);
    // PORT-REGRESSION PINS in the script below, `accepted at plan9-t7t8 (ruling CC)`: the
    // `incN` tokens. Java's `IncompleteFreeSpaceExpansionRoom.getId` is the hash
    // `31 * shape.getId() + layer` over a MUTABLE shape (quirk #158, hazard C), so the jar printed
    // four wide hash values here. #158's fix at Task 8 gives the room an
    // `id_no` drawn from the engine's shared counter as it enters the arena, and the port prints
    // the counter's small consecutive ids instead. Nothing else in the script moved: the door
    // count, every door's dimension, every shape and every corner list is the jar's to the digit,
    // and so is the `incompleteRooms` block below.
    //
    // Java's arithmetic is not lost — it survives as `IncompleteFreeSpaceExpansionRoom::java_id`,
    // which still panics on the whole-plane room exactly where Java NPEs, with its own tests
    // beside it. The jar's tokens, for the record:
    // `inc-297914650 inc-10116850 inc-287845850 inc-307834650` -> `inc8 inc9 inc10 inc11`.
    assert_script(
        &actual,
        r#"
  result=obs2/0 shape=Box[-650,-150..-350,150] corners=(-650.0,-150.0;-350.0,-150.0;-350.0,150.0;-650.0,150.0)
  doors n=4
    [0] first=obs2/0 second=inc8 dim=1 shape=Box[-650,-150..-350,-150] corners=(-650.0,-150.0;-350.0,-150.0;-350.0,-150.0;-650.0,-150.0)
    [1] first=obs2/0 second=inc9 dim=1 shape=Box[-350,-150..-350,150] corners=(-350.0,-150.0;-350.0,-150.0;-350.0,150.0;-350.0,150.0)
    [2] first=obs2/0 second=inc10 dim=1 shape=Box[-650,150..-350,150] corners=(-650.0,150.0;-350.0,150.0;-350.0,150.0;-650.0,150.0)
    [3] first=obs2/0 second=inc11 dim=1 shape=Box[-650,-150..-650,150] corners=(-650.0,-150.0;-650.0,-150.0;-650.0,150.0;-650.0,150.0)
  targetDoors n=0
  incompleteRooms n=4
    [0] layer=0 shape=Box[-10000,-10000..10000,-150] corners=(-10000.0,-10000.0;10000.0,-10000.0;10000.0,-150.0;-10000.0,-150.0) contained=Box[-650,-150..-350,-150] doors=1
    [1] layer=0 shape=Box[-350,-10000..10000,10000] corners=(-350.0,-10000.0;10000.0,-10000.0;10000.0,10000.0;-350.0,10000.0) contained=Box[-350,-150..-350,150] doors=1
    [2] layer=0 shape=Box[-10000,150..10000,10000] corners=(-10000.0,150.0;10000.0,150.0;10000.0,10000.0;-10000.0,10000.0) contained=Box[-650,150..-350,150] doors=1
    [3] layer=0 shape=Box[-10000,-10000..-650,10000] corners=(-10000.0,-10000.0;-650.0,-10000.0;-650.0,10000.0;-10000.0,10000.0) contained=Box[-650,-150..-650,150] doors=1
"#,
    );
}

// =================================================================================================
// The free-space 2-dimensional overlap arm — `p6t3 6/7 42 30 1000`, the `overlap` probe
// =================================================================================================

/// The `overlap` probe of `p6t3` modes 6 and 7: a `CompleteFreeSpaceExpansionRoom` that overlaps
/// the room under test 2-dimensionally, on an otherwise empty part of layer 1.
///
/// The random part of the driver cannot produce this: its seed rooms are `completeShape` output,
/// restrained against everything already in the tree. But it is **not** structurally unreachable
/// in production — `tryRemoveEdgeLine`/`tryRemoveEdge` deliberately look for a `dimension == 2`
/// door to a free-space room to use as `completeShape`'s `ignoreObject`
/// (`Sorted45DegreeRoomNeighbours.java:373-394`, `SortedOrthogonalRoomNeighbours.java:459-526`),
/// which is exactly a room the next pass may then overlap; and
/// `CompleteFreeSpaceExpansionRoom.isTraceObstacle` is the constant `true`
/// (`CompleteFreeSpaceExpansionRoom.java:81-84`), so nothing diverts such an overlap to
/// `calculateTargetDoors`.
fn overlap_probe_rooms(
    board: &mut Board,
    rooms: &mut ExpansionRoomStore,
    tree_id: TreeId,
) -> RoomRef {
    let overlap_room = rooms.new_complete_room(
        Some(TileShape::Box(IntBox::from_coords(
            -2500, -8500, 500, -6500,
        ))),
        1,
        1004,
    );
    let tree = board
        .trees
        .trees_mut()
        .find(|tree| tree.id() == tree_id)
        .expect("the autoroute tree");
    rooms.insert_complete_room(tree, overlap_room);
    let room_box = IntBox::from_coords(-4000, -9500, -1000, -7500);
    RoomRef::Incomplete(rooms.new_incomplete_room(
        Some(TileShape::Box(room_box)),
        1,
        Some(TileShape::Box(room_box)),
    ))
}

#[test]
fn a_two_dimensional_overlap_is_a_45_degree_neighbour_with_a_two_dimensional_door() {
    // `run.sh p6t3 6 42 30 1000`, the `overlap regime=2` block, verbatim. Two things are pinned:
    //
    // * `Sorted45DegreeRoomNeighbours.java:132` is `dimension > 1 && completedRoom instanceof
    //   ObstacleExpansionRoom`, so a 2-dimensional overlap with a **free-space** completed room
    //   falls through to `addSortedNeighbour` — where the base class `continue`s for every
    //   `dimension > 1` (`SortedRoomNeighbours.java:232`);
    // * `:164` is the **two**-argument `new ExpansionDoor(completedRoom, neighbourRoom)`
    //   (`ExpansionDoor.java:35-39`), which *computes* the dimension from the two rooms' shapes —
    //   `dim=2` here. Only the base class hard-codes 1 at that spot
    //   (`SortedRoomNeighbours.java:281`). This is the only site in the class that can build a
    //   `dimension == 2` door, and `:381` scans for exactly one when it picks `completeShape`'s
    //   `ignoreObject`.
    let (mut board, tree_id) = p6t3_board(AngleRestriction::FortyFiveDegree, &GRID_OBSTACLES);
    let mut rooms = p6t3_seed_rooms(&mut board, tree_id, GRID_SEED_ROOMS);
    let room = overlap_probe_rooms(&mut board, &mut rooms, tree_id);

    let result = Sorted45DegreeRoomNeighbours::calculate_neighbours(
        room, 1, &mut board, &mut rooms, tree_id, 1005,
    )
    .expect("an incomplete room completes");
    let mut actual = Vec::new();
    dump_45(&result, &rooms, &mut actual);
    assert_script(
        &actual,
        r#"
  neighbours n=1
    [0] fts=2 lts=4 obj=cfsr1004 nshape=Oct[-2500,-8500,500,-6500,4000,9000,-11000,-6000] nshapeCorners=(-2500.0,-8500.0;500.0,-8500.0;500.0,-8500.0;500.0,-6500.0;500.0,-6500.0;-2500.0,-6500.0;-2500.0,-6500.0;-2500.0,-8500.0) isect=Oct[-2500,-8500,-1000,-7500,5000,7500,-11000,-8500] isectCorners=(-2500.0,-8500.0;-1000.0,-8500.0;-1000.0,-8500.0;-1000.0,-7500.0;-1000.0,-7500.0;-2500.0,-7500.0;-2500.0,-7500.0;-2500.0,-8500.0)
  edgeTouches=[false,false,true,true,true,false,false,false]
  completedRoom=cfsr1005
  doors n=1
    [0] first=cfsr1005 second=cfsr1004 dim=2 shape=Box[-2500,-8500..-1000,-7500] corners=(-2500.0,-8500.0;-1000.0,-8500.0;-1000.0,-7500.0;-2500.0,-7500.0)
  targetDoors n=0
"#,
    );
    // Spelled out, because a hard-coded `1` here would still have matched every other fixture in
    // this file and every configuration of `p6t3` modes 6-9.
    let doors = rooms.room_doors(result.completed_room);
    assert_eq!(doors.len(), 1);
    assert_eq!(rooms.door(doors[0]).expect("a live door").dimension, 2);
}

#[test]
fn a_two_dimensional_overlap_is_an_orthogonal_neighbour_with_a_two_dimensional_door() {
    // `run.sh p6t3 7 42 30 1000`, the `overlap regime=1` block, verbatim — the same two facts for
    // `SortedOrthogonalRoomNeighbours.java:168` and `:201`, whose `dimension == 2` scan is at
    // `:468`.
    let (mut board, tree_id) = p6t3_board(AngleRestriction::NinetyDegree, &GRID_OBSTACLES);
    let mut rooms = p6t3_seed_rooms(&mut board, tree_id, GRID_SEED_ROOMS);
    let room = overlap_probe_rooms(&mut board, &mut rooms, tree_id);

    let result = SortedOrthogonalRoomNeighbours::calculate_neighbours(
        room, 1, &mut board, &mut rooms, tree_id, 1005,
    )
    .expect("an incomplete room completes");
    let mut actual = Vec::new();
    dump_orthogonal(&result, &rooms, &mut actual);
    assert_script(
        &actual,
        r#"
  neighbours n=1
    [0] fts=1 lts=2 obj=cfsr1004 nshape=Box[-2500,-8500..500,-6500] nshapeCorners=(-2500.0,-8500.0;500.0,-8500.0;500.0,-6500.0;-2500.0,-6500.0) isect=Box[-2500,-8500..-1000,-7500] isectCorners=(-2500.0,-8500.0;-1000.0,-8500.0;-1000.0,-7500.0;-2500.0,-7500.0)
  edgeTouches=[false,true,true,false]
  completedRoom=cfsr1005
  doors n=1
    [0] first=cfsr1005 second=cfsr1004 dim=2 shape=Box[-2500,-8500..-1000,-7500] corners=(-2500.0,-8500.0;-1000.0,-8500.0;-1000.0,-7500.0;-2500.0,-7500.0)
  targetDoors n=0
"#,
    );
    let doors = rooms.room_doors(result.completed_room);
    assert_eq!(doors.len(), 1);
    assert_eq!(rooms.door(doors[0]).expect("a live door").dimension, 2);
}

// =================================================================================================
// The dispatch reaches three different implementations
// =================================================================================================

/// A two-layer board with the named box obstacles, in the given angle regime, plus its autoroute
/// tree — the smallest board on which the three sorters visibly disagree.
fn tiny_board(angle: AngleRestriction, obstacles: &[IntBox]) -> (Board, TreeId) {
    let layers = || LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)]);
    let clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 0);
    let mut rules = BoardRules::new(layers(), clearance_matrix);
    rules.trace_angle_restriction = angle;

    let mut board = Board::new(
        Vec::new(),
        0,
        IntBox::from_coords(-10_000, -10_000, 10_000, 10_000),
        rules,
        BoardLibrary::new(Padstacks::new(layers()), Packages::new()),
        Components::new(),
        Communication::default(),
    );
    for shape in obstacles {
        board.insert_obstacle(
            Area::Shape(Shape::Tile(TileShape::Box(*shape))),
            0,
            1,
            FixedState::Unfixed,
        );
    }
    let tree_id = {
        let mut items = std::mem::take(&mut board.items);
        let mut manager = std::mem::take(&mut board.trees);
        let id = {
            let ctx = board.ctx();
            let mut refs: Vec<&mut Item> = items.values_mut().rev().collect();
            manager.get_autoroute_tree(1, &mut refs, &ctx).id()
        };
        board.items = items;
        board.trees = manager;
        id
    };
    (board, tree_id)
}

/// Runs the whole of `SortedRoomNeighbours::complete` for one regime over the same input room and
/// renders everything the call left behind.
fn complete_in_regime(
    angle: AngleRestriction,
    room_box: IntBox,
    obstacles: &[IntBox],
) -> Vec<String> {
    let (mut board, tree_id) = tiny_board(angle, obstacles);
    let mut rooms = ExpansionRoomStore::new();
    let room = RoomRef::Incomplete(rooms.new_incomplete_room(
        Some(TileShape::Box(room_box)),
        0,
        Some(TileShape::Box(room_box)),
    ));
    let result = SortedRoomNeighbours::complete(room, 1, &mut board, &mut rooms, tree_id)
        .expect("an incomplete room completes");
    let mut out = vec![format!(
        "  result={} shape={}",
        desc(result, &rooms),
        opt_shp(rooms.room_shape(result))
    )];
    dump_doors(result, &rooms, &mut out);
    // Arena slot 0 is the seed room itself, which Java never puts on the engine's list.
    dump_incomplete_rooms(&rooms, &[0], &mut out);
    out
}

#[test]
fn the_three_regimes_disagree_on_the_same_room() {
    // One room and one board, three sorters — reached only through
    // `SortedRoomNeighbours::complete`, so this is the dispatch under test and not the three
    // entry points called directly. The two obstacles touch the room's right and top sides, so all
    // three implementations have work to do, and they do three different things:
    //
    // * the base class walks the room's own simplex border lines and answers four one-dimensional
    //   doors onto half-plane simplices;
    // * `Sorted45DegreeRoomNeighbours` walks the eight sides of the room's **bounding octagon** and
    //   answers two two-dimensional doors onto octagons — and leaves the room's shape alone,
    //   because its `tryRemoveEdgeLine` insists on an `IntOctagon` (`:320-325`);
    // * `SortedOrthogonalRoomNeighbours` accepts the `IntBox`, so its `tryRemoveEdge` **does**
    //   enlarge the room, and its doors are boxes.
    let obstacles = [
        IntBox::from_coords(0, -500, 1000, 500),
        IntBox::from_coords(-3000, 1000, -500, 2000),
    ];
    let room_box = IntBox::from_coords(-2000, -1000, 0, 1000);
    let any = complete_in_regime(AngleRestriction::None, room_box, &obstacles);
    let deg45 = complete_in_regime(AngleRestriction::FortyFiveDegree, room_box, &obstacles);
    let orthogonal = complete_in_regime(AngleRestriction::NinetyDegree, room_box, &obstacles);

    assert_ne!(any, deg45, "the 45-degree arm must not be the base class");
    assert_ne!(
        any, orthogonal,
        "the orthogonal arm must not be the base class"
    );
    assert_ne!(
        deg45, orthogonal,
        "the two subclasses must not be each other"
    );

    // The base class keeps the room's shape as a simplex and builds one-dimensional doors.
    assert!(any[0].contains("shape=Simplex{"), "{}", any[0]);
    assert_eq!(any[1], "  doors n=4");
    assert!(any[2..6].iter().all(|line| line.contains(" dim=1 ")));

    // The 45-degree class leaves the box alone and answers two-dimensional octagon doors.
    //
    // PORT-REGRESSION PIN, `accepted at plan9-t7t8 (ruling CC)`: the jar's completed room is
    // `cfsr1`, the port's is `cfsr2`. Room ids come from ONE shared counter across the engine's
    // room kinds (#156/#167/#158), and the seed incomplete room this call completes now draws
    // from that counter too, so the complete room it becomes is the second id issued rather than
    // the first — and the orthogonal arm below moves from `cfsr2` to `cfsr3` by the same step.
    // The SHAPES, the door counts and the door dimensions on all three arms are the jar's, and
    // they are what the three-way disagreement this test is named for is read off.
    assert_eq!(deg45[0], "  result=cfsr2 shape=Box[-2000,-1000..0,1000]");
    assert_eq!(deg45[1], "  doors n=2");
    assert!(deg45[2..4].iter().all(|line| line.contains(" dim=2 ")));
    assert!(deg45[2..4].iter().all(|line| line.contains("shape=Oct[")));

    // The orthogonal class enlarges the room, and its doors are boxes.
    //
    // Same pin: jar `cfsr2`, port `cfsr3`, `accepted at plan9-t7t8 (ruling CC)`.
    assert_eq!(
        orthogonal[0],
        "  result=cfsr3 shape=Box[-2000,-10000..0,1000]"
    );
    // The three arms must stay three DIFFERENT answers, which is the whole subject; the
    // `assert_ne!`s above are what hold it, and they are unaffected by the id re-cut.
    assert_eq!(orthogonal[1], "  doors n=2");
    assert!(orthogonal[2..4].iter().all(|line| line.contains(" dim=2 ")));
    assert!(
        orthogonal[2..4]
            .iter()
            .all(|line| line.contains("shape=Box["))
    );
}

// =================================================================================================
// The two comparators — total orders, unlike the base class's (quirks #160 and #161)
// =================================================================================================

type Neighbour45 = fr_router::autoroute::expansion::sorted_neighbours_45::SortedRoomNeighbour;
type NeighbourOrthogonal =
    fr_router::autoroute::expansion::sorted_neighbours_orthogonal::SortedRoomNeighbour;

/// One 45-degree neighbour, built directly through the inner class's constructor exactly as
/// `addSortedNeighbour` does.
fn neighbour_45(
    room: IntOctagon,
    neighbour: IntOctagon,
    edge_flags: &mut [bool; 8],
    object_id: i32,
) -> Neighbour45 {
    Neighbour45::new(
        TreeObject::Room(RoomId(object_id as u32)),
        object_id,
        neighbour,
        room.intersection(&neighbour),
        &room,
        edge_flags,
    )
}

/// One orthogonal neighbour, likewise.
fn neighbour_orthogonal(
    room: IntBox,
    neighbour: IntBox,
    edge_flags: &mut [bool; 4],
    object_id: i32,
) -> NeighbourOrthogonal {
    NeighbourOrthogonal::new(
        TreeObject::Item(ItemId(object_id as u32)),
        object_id,
        neighbour,
        room.intersection(&neighbour),
        &room,
        edge_flags,
    )
}

/// A spread of neighbour boxes that really touch a `[-2000,-2000..2000,2000]` room on each of its
/// four sides, at several offsets and two spans.
fn touching_boxes() -> Vec<IntBox> {
    let mut boxes = Vec::new();
    for (dx, dy) in [(0, -2400), (2000, 0), (0, 2000), (-2400, 0)] {
        for offset in [-1500, -500, 0, 700, 1400] {
            for span in [300, 900] {
                let (x, y) = if dx == 0 { (offset, dy) } else { (dx, offset) };
                boxes.push(IntBox::from_coords(x, y, x + span + 400, y + span + 400));
            }
        }
    }
    boxes
}

#[test]
fn both_regime_comparators_are_total_orders_where_the_base_class_is_not() {
    // The base class's `compareTo` is **not** transitive (quirk #160, pinned by
    // `tests/sorted_neighbours.rs::the_comparator_is_not_transitive_…`): its refinements are
    // entered on different conditions for different pairs, so its `Equal` is not an equivalence
    // and a `TreeSet` and a `BTreeSet` keep different elements. Both of Task 5's comparators are
    // plain lexicographic orders on five Java `int` keys — every refinement is entered on the same
    // `cmpValue == 0` for both operands — so they are antisymmetric and transitive. Checked over
    // every ordered triple of the spread above, which is the property both module docs claim.
    let room_oct = IntOctagon::new(-2000, -2000, 2000, 2000, -8000, 8000, -8000, 8000).normalize();
    let mut oct_flags = [false; 8];
    let built: Vec<Neighbour45> = touching_boxes()
        .iter()
        .enumerate()
        .map(|(i, b)| {
            neighbour_45(
                room_oct,
                b.to_int_octagon().normalize(),
                &mut oct_flags,
                (i % 5) as i32 + 1,
            )
        })
        .collect();
    assert_transitive(&built, |a, b| a.compare_to(b));

    let room_box = IntBox::from_coords(-2000, -2000, 2000, 2000);
    let mut box_flags = [false; 4];
    let built: Vec<NeighbourOrthogonal> = touching_boxes()
        .iter()
        .enumerate()
        .map(|(i, b)| neighbour_orthogonal(room_box, *b, &mut box_flags, (i % 5) as i32 + 1))
        .collect();
    assert_transitive(&built, |a, b| a.compare_to(b));
}

fn assert_transitive<T>(items: &[T], cmp: impl Fn(&T, &T) -> std::cmp::Ordering) {
    use std::cmp::Ordering;
    for a in items {
        for b in items {
            assert_eq!(
                cmp(a, b).reverse(),
                cmp(b, a),
                "the comparator must be antisymmetric"
            );
            for c in items {
                if cmp(a, b) != Ordering::Greater && cmp(b, c) != Ordering::Greater {
                    assert_ne!(
                        cmp(a, c),
                        Ordering::Greater,
                        "a <= b and b <= c must give a <= c"
                    );
                }
            }
        }
    }
}

#[test]
fn identical_geometry_and_a_colliding_id_still_drops_a_neighbour() {
    // Both comparators end in `this.searchTreeObject.getId() - other.searchTreeObject.getId()`
    // (Sorted45DegreeRoomNeighbours.java:977, SortedOrthogonalRoomNeighbours.java:723) over two id
    // spaces that both start at 1 (quirk #161), so an **item** 3 and a **room** 3 with the same
    // geometry compare `Equal` — and `TreeSet.add` then answers false and keeps the first. Being a
    // total order does not save the second element; only an unequal key would.
    let room = IntBox::from_coords(-2000, -2000, 2000, 2000);
    let neighbour = IntBox::from_coords(-500, -2400, 500, -2000);
    let mut edge_flags = [false; 4];
    let item = NeighbourOrthogonal::new(
        TreeObject::Item(ItemId(3)),
        3,
        neighbour,
        room.intersection(&neighbour),
        &room,
        &mut edge_flags,
    );
    let expansion_room = NeighbourOrthogonal::new(
        TreeObject::Room(RoomId(3)),
        3,
        neighbour,
        room.intersection(&neighbour),
        &room,
        &mut edge_flags,
    );
    assert_eq!(item.compare_to(&expansion_room), std::cmp::Ordering::Equal);

    let mut set = JavaTreeSet::new();
    assert!(set.add(item));
    assert!(!set.add(expansion_room), "the TreeSet silently drops it");
    assert_eq!(set.len(), 1);
    assert!(matches!(
        set.last().expect("one survivor").search_tree_object,
        TreeObject::Item(ItemId(3))
    ));
}
