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

const GRID_SEED_ROOMS: [(i32, i32, i32, i32, usize); 3] = [
    (-2188, 4586, 247, 6855, 0),
    (-1858, 6349, -1018, 7935, 1),
    (-5674, 8222, -3634, 10595, 0),
];

const RANDOM_SEED_ROOMS: [(i32, i32, i32, i32, usize); 3] = [
    (1386, -2517, 2807, 458, 1),
    (2811, 8147, 3647, 10698, 0),
    (-1114, 1885, 790, 4531, 0),
];

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


#[test]
fn the_45_degree_sorter_matches_the_p6t3_script() {
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


#[test]
fn the_orthogonal_sorter_matches_the_p6t3_script() {
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


#[test]
fn an_eight_sided_obstacle_room_gets_eight_doors() {
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


#[test]
fn an_orthogonal_obstacle_room_with_no_neighbours_gets_one_room_per_board_side() {
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
    let doors = rooms.room_doors(result.completed_room);
    assert_eq!(doors.len(), 1);
    assert_eq!(rooms.door(doors[0]).expect("a live door").dimension, 2);
}

#[test]
fn a_two_dimensional_overlap_is_an_orthogonal_neighbour_with_a_two_dimensional_door() {
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
    dump_incomplete_rooms(&rooms, &[0], &mut out);
    out
}

#[test]
fn the_three_regimes_disagree_on_the_same_room() {
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

    assert!(any[0].contains("shape=Simplex{"), "{}", any[0]);
    assert_eq!(any[1], "  doors n=4");
    assert!(any[2..6].iter().all(|line| line.contains(" dim=1 ")));

    assert_eq!(deg45[0], "  result=cfsr2 shape=Box[-2000,-1000..0,1000]");
    assert_eq!(deg45[1], "  doors n=2");
    assert!(deg45[2..4].iter().all(|line| line.contains(" dim=2 ")));
    assert!(deg45[2..4].iter().all(|line| line.contains("shape=Oct[")));

    assert_eq!(
        orthogonal[0],
        "  result=cfsr3 shape=Box[-2000,-10000..0,1000]"
    );
    assert_eq!(orthogonal[1], "  doors n=2");
    assert!(orthogonal[2..4].iter().all(|line| line.contains(" dim=2 ")));
    assert!(
        orthogonal[2..4]
            .iter()
            .all(|line| line.contains("shape=Box["))
    );
}


type Neighbour45 = fr_router::autoroute::expansion::sorted_neighbours_45::SortedRoomNeighbour;
type NeighbourOrthogonal =
    fr_router::autoroute::expansion::sorted_neighbours_orthogonal::SortedRoomNeighbour;

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
