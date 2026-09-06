use std::collections::{BTreeMap, BTreeSet};

use fr_board::ids::TreeObject;
use fr_board::prelude::*;
use fr_geometry::{Area, IntBox, IntOctagon, IntVector, Point, Polyline, Shape, TileShape};
use fr_router::autoroute::expansion::sorted_neighbours::{
    SortedRoomNeighbour, SortedRoomNeighbours,
};
use fr_router::autoroute::expansion::{
    ExpansionRoomStore, IncompleteFreeSpaceExpansionRoom, RoomRef,
};
use fr_router::autoroute::item_info;
use fr_router::autoroute::tree_ext::AutorouteSearchTreeExt;

fn corner_neighbour(
    room_shape: &TileShape,
    neighbour: IntBox,
    tsr: i32,
    tsn: i32,
    ntc: bool,
    object_id: i32,
) -> SortedRoomNeighbour {
    let neighbour_shape = TileShape::Box(neighbour);
    let intersection = room_shape.intersection(&neighbour_shape);
    SortedRoomNeighbour::new(
        TreeObject::Room(RoomId(object_id as u32)),
        object_id,
        neighbour_shape,
        intersection,
        tsr,
        tsn,
        true,
        ntc,
        room_shape.clone(),
    )
}

#[test]
fn five_neighbours_at_one_corner_are_all_kept_and_sort() {
    let room = TileShape::Box(IntBox::from_coords(1730, -384, 3188, 78));
    let inputs: [(IntBox, i32, i32, bool, i32); 5] = [
        (
            IntBox::from_coords(-1481, -2678, -737, -2434),
            1,
            3,
            true,
            2,
        ),
        (
            IntBox::from_coords(-2604, 1412, -1661, 1887),
            1,
            3,
            false,
            1,
        ),
        (IntBox::from_coords(1645, -2924, 3049, -2217), 1, 1, true, 4),
        (
            IntBox::from_coords(-1447, -2108, -661, -1705),
            1,
            2,
            true,
            5,
        ),
        (IntBox::from_coords(-2652, 377, -1753, 1070), 1, 0, false, 5),
    ];
    let built: Vec<SortedRoomNeighbour> = inputs
        .iter()
        .map(|&(b, tsr, tsn, ntc, id)| corner_neighbour(&room, b, tsr, tsn, ntc, id))
        .collect();

    #[allow(clippy::mutable_key_type)]
    let mut set = BTreeSet::new();
    for (i, neighbour) in built.iter().enumerate() {
        assert!(
            set.insert(neighbour.clone()),
            "insert[{i}] must succeed: all five compare distinct"
        );
        assert_eq!(set.len(), i + 1);
    }
    assert_eq!(
        set.len(),
        5,
        "the comparator is a total order; the BTreeSet keeps all five"
    );

    let mut by_compare_to = built.clone();
    by_compare_to.sort_by(SortedRoomNeighbour::compare_to);
    assert_eq!(
        set.iter().collect::<Vec<_>>(),
        by_compare_to.iter().collect::<Vec<_>>(),
        "the BTreeSet's Ord-driven order matches sorting by compare_to directly"
    );

    use std::cmp::Ordering;
    assert_ne!(
        built[3].compare_to(&built[4]),
        Ordering::Equal,
        "two neighbours with different touching sides are two doors"
    );
    assert_eq!(
        built[3].compare_to(&built[4]).reverse(),
        built[4].compare_to(&built[3]),
        "antisymmetry"
    );
    for (a, b, c) in [(0usize, 3usize, 4usize), (3, 4, 0), (4, 0, 3)] {
        let (ab, bc, ac) = (
            built[a].compare_to(&built[b]),
            built[b].compare_to(&built[c]),
            built[a].compare_to(&built[c]),
        );
        if ab == Ordering::Less && bc == Ordering::Less {
            assert_eq!(ac, Ordering::Less, "transitivity on ({a}, {b}, {c})");
        }
    }
}

#[test]
fn a_tie_on_geometry_no_longer_drops_the_neighbour() {
    let room = TileShape::Box(IntBox::from_coords(656, 685, 1281, 2671));
    #[allow(clippy::mutable_key_type)]
    let mut set = BTreeSet::new();
    assert!(set.insert(corner_neighbour(
        &room,
        IntBox::from_coords(-2928, 954, -2418, 1931),
        1,
        1,
        true,
        4
    )));
    assert!(set.insert(corner_neighbour(
        &room,
        IntBox::from_coords(756, 1893, 2108, 2443),
        3,
        3,
        false,
        2
    )));
    assert!(set.insert(corner_neighbour(
        &room,
        IntBox::from_coords(1639, 1220, 1818, 1470),
        0,
        2,
        true,
        3
    )));
    assert!(
        set.insert(corner_neighbour(
            &room,
            IntBox::from_coords(-2824, -2164, -2591, -1506),
            3,
            1,
            false,
            2
        )),
        "the jar's id tie-break answered 0 and its TreeSet dropped this neighbour; \
         the port keeps it"
    );
    assert_eq!(set.len(), 4);
}

#[test]
fn a_room_id_is_never_subtracted_from_an_item_id() {
    let room = TileShape::Box(IntBox::from_coords(0, 0, 1000, 1000));
    let neighbour = IntBox::from_coords(-500, -500, -100, -100);
    let neighbour_shape = TileShape::Box(neighbour);
    let intersection = room.intersection(&neighbour_shape);
    let make = |object: TreeObject, id: i32| {
        SortedRoomNeighbour::new(
            object,
            id,
            neighbour_shape.clone(),
            intersection.clone(),
            0,
            0,
            true,
            false,
            room.clone(),
        )
    };
    let as_item = make(TreeObject::Item(ItemId(3)), 3);
    let as_room = make(TreeObject::Room(RoomId(0)), 3);
    assert_eq!(
        as_item.compare_to(&as_room),
        std::cmp::Ordering::Less,
        "an item sorts before a room; the two ids are never subtracted from one another"
    );
    assert_eq!(
        as_room.compare_to(&as_item),
        std::cmp::Ordering::Greater,
        "and the relation is antisymmetric"
    );
    #[allow(clippy::mutable_key_type)]
    let mut set = BTreeSet::new();
    assert!(set.insert(as_item));
    assert!(
        set.insert(as_room),
        "the room is no longer dropped for colliding with the item"
    );
    assert_eq!(set.len(), 2);
    let as_room_4 = make(TreeObject::Room(RoomId(0)), 4);
    assert!(set.insert(as_room_4));
    assert_eq!(set.len(), 3);
}

struct Xorshift64(u64);

impl Xorshift64 {
    fn new(seed: u64) -> Xorshift64 {
        Xorshift64(if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        })
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn rnd(&mut self, bound: u64) -> i32 {
        (self.next() % bound) as i32
    }

    fn coord(&mut self, range: i32) -> i32 {
        self.rnd(2 * range as u64 + 1) - range
    }

    fn box_(&mut self, range: i32, min_size: i32, max_size: i32) -> IntBox {
        let w = min_size + self.rnd((max_size - min_size + 1) as u64);
        let h = min_size + self.rnd((max_size - min_size + 1) as u64);
        let x = self.coord(range);
        let y = self.coord(range);
        IntBox::from_coords(x, y, x + w, y + h)
    }
}

#[test]
fn the_neighbour_comparator_is_a_total_order() {
    let mut rng = Xorshift64::new(42);
    let mut case_520_room = None;
    let mut drops_btree = 0usize;
    let mut neighbours_built = 0usize;
    for case in 0..2000 {
        let room_box = rng.box_(2000, 400, 2000);
        if case == 520 {
            case_520_room = Some(room_box);
        }
        let room = TileShape::Box(room_box);
        let count = 3 + rng.rnd(3);
        let mut built = Vec::new();
        for _ in 0..count {
            let neighbour_box = rng.box_(3000, 100, 1500);
            let tsr = rng.rnd(4);
            let tsn = rng.rnd(4);
            let ntc = rng.rnd(2) == 0;
            let object_id = 1 + rng.rnd(5);
            built.push(corner_neighbour(
                &room,
                neighbour_box,
                tsr,
                tsn,
                ntc,
                object_id,
            ));
        }
        neighbours_built += built.len();

        let value_of = |n: &SortedRoomNeighbour| {
            (
                n.touching_side_no_of_room,
                n.touching_side_no_of_neighbour_room,
                n.room_touch_is_corner,
                n.neighbour_room_touch_is_corner,
                n.object_id,
                {
                    let b = n.neighbour_shape.bounding_box();
                    (b.ll.x, b.ll.y, b.ur.x, b.ur.y)
                },
            )
        };
        let distinct: std::collections::BTreeSet<_> = built.iter().map(value_of).collect();

        #[allow(clippy::mutable_key_type)]
        let mut btree = std::collections::BTreeSet::new();
        for neighbour in &built {
            btree.insert(neighbour.clone());
        }
        drops_btree += distinct.len() - btree.len();

        use std::cmp::Ordering;
        for i in 0..built.len() {
            for j in 0..built.len() {
                assert_eq!(
                    built[i].compare_to(&built[j]).reverse(),
                    built[j].compare_to(&built[i]),
                    "case {case}: compare_to({i}, {j}) is not antisymmetric"
                );
                for k in 0..built.len() {
                    let (ij, jk) = (
                        built[i].compare_to(&built[j]),
                        built[j].compare_to(&built[k]),
                    );
                    if ij != Ordering::Greater && jk != Ordering::Greater {
                        assert_ne!(
                            built[i].compare_to(&built[k]),
                            Ordering::Greater,
                            "case {case}: {i} <= {j} <= {k} but {i} > {k}"
                        );
                    }
                }
            }
        }
    }
    assert_eq!(
        case_520_room,
        Some(IntBox::from_coords(1730, -384, 3188, 78)),
        "the xorshift stream must reproduce the jar's `corner c=520 room=`"
    );
    assert_eq!(
        neighbours_built, 8000,
        "3 + rnd(3) over 2 000 cases; the jar's own draw split 665/670/665"
    );
    assert_eq!(
        drops_btree, 0,
        "the comparator is a total order: the BTreeSet drops none of the 8 000 distinct \
         neighbours built (the pre-fix comparator dropped 482 of them)"
    );
}

fn p6t3_board() -> (Board, TreeId) {
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

    const OBSTACLES: [(i32, i32, i32, i32, usize); 30] = [
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
    for (llx, lly, urx, ury, layer) in OBSTACLES {
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

fn p6t3_seed_rooms(board: &mut Board, tree_id: TreeId) -> ExpansionRoomStore {
    let mut rooms = ExpansionRoomStore::new();
    for (id, (llx, lly, urx, ury, layer)) in [
        (-2188, 4586, 247, 6855, 0usize),
        (-1858, 6349, -1018, 7935, 1),
        (-5674, 8222, -3634, 10595, 0),
    ]
    .into_iter()
    .enumerate()
    {
        let id_no = rooms.next_room_id_no();
        assert_eq!(id_no, id as i32 + 1);
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

fn corners_of(shape: &TileShape) -> Vec<(f64, f64)> {
    shape
        .corner_approx_arr()
        .iter()
        .map(|c| (c.x, c.y))
        .collect()
}

#[test]
fn one_neighbour_yields_one_door() {
    let (mut board, tree_id) = p6t3_board();
    let mut rooms = p6t3_seed_rooms(&mut board, tree_id);
    let room = seed_free_space_room(
        &board,
        &mut rooms,
        tree_id,
        1,
        IntBox::from_coords(-1102, 7787, -804, 7980),
        3,
        1,
        2,
    );

    let result =
        SortedRoomNeighbours::calculate_neighbours(room, 3, &mut board, &mut rooms, tree_id, 650)
            .expect("an incomplete room completes");

    let neighbours: Vec<&SortedRoomNeighbour> = result.sorted_neighbours.iter().collect();
    assert_eq!(neighbours.len(), 1);
    let n = neighbours[0];
    assert_eq!(n.touching_side_no_of_room, 0);
    assert_eq!(n.touching_side_no_of_neighbour_room, 2);
    assert!(!n.room_touch_is_corner);
    assert!(!n.neighbour_room_touch_is_corner);
    assert_eq!(n.search_tree_object, TreeObject::Room(RoomId(1)));
    assert_eq!(n.object_id, 2);
    assert_eq!(n.first_corner().to_float().x, -1858.0);
    assert_eq!(n.first_corner().to_float().y, 7935.0);
    assert_eq!(n.last_corner().to_float().x, -1018.0);
    assert_eq!(n.last_corner().to_float().y, 7935.0);
    assert_eq!(
        n.neighbour_shape,
        TileShape::Box(IntBox::from_coords(-1858, 6349, -1018, 7935))
    );
    assert!(result.own_net_objects.is_empty());

    assert_eq!(rooms.room_id_no(result.completed_room), Some(650));
    let doors = rooms.room_doors(result.completed_room).to_vec();
    assert_eq!(doors.len(), 1);
    let door = rooms.door(doors[0]).expect("a live door");
    assert_eq!(door.first_room, result.completed_room);
    assert_eq!(door.second_room, RoomRef::Complete(RoomId(1)));
    assert_eq!(door.dimension, 1);
    assert_eq!(
        corners_of(&rooms.door_shape(doors[0]).expect("a door shape")),
        vec![
            (-1858.0, 7935.0),
            (-1018.0, 7935.0),
            (-1018.0, 7935.0),
            (-1858.0, 7935.0)
        ]
    );
}

#[test]
fn a_corner_touch_is_recorded_with_both_corner_flags_and_yields_no_door() {
    let (mut board, tree_id) = p6t3_board();
    let mut rooms = p6t3_seed_rooms(&mut board, tree_id);
    let room = seed_free_space_room(
        &board,
        &mut rooms,
        tree_id,
        1,
        IntBox::from_coords(4177, 3527, 4399, 3816),
        3,
        0,
        1,
    );

    let result =
        SortedRoomNeighbours::calculate_neighbours(room, 3, &mut board, &mut rooms, tree_id, 18)
            .expect("an incomplete room completes");

    let neighbours: Vec<&SortedRoomNeighbour> = result.sorted_neighbours.iter().collect();
    let summary: Vec<(i32, i32, bool, bool, TreeObject)> = neighbours
        .iter()
        .map(|n| {
            (
                n.touching_side_no_of_room,
                n.touching_side_no_of_neighbour_room,
                n.room_touch_is_corner,
                n.neighbour_room_touch_is_corner,
                n.search_tree_object,
            )
        })
        .collect();
    assert_eq!(
        summary,
        vec![
            (0, 4, false, false, TreeObject::Item(ItemId(21))),
            (1, 6, false, false, TreeObject::Item(ItemId(20))),
            (2, 0, false, false, TreeObject::Item(ItemId(22))),
            (3, 0, true, true, TreeObject::Item(ItemId(33))),
            (3, 1, false, false, TreeObject::Item(ItemId(7))),
        ]
    );
    let corner = neighbours[3];
    assert_eq!(corner.first_corner(), corner.last_corner());
    assert_eq!(corner.first_corner().to_float().x, 2041.0);
    assert_eq!(corner.first_corner().to_float().y, 7400.0);
    assert!(rooms.room_doors(result.completed_room).is_empty());
}

#[test]
fn a_two_dimensional_overlap_yields_an_overlap_door_between_obstacle_rooms() {
    let (mut board, tree_id) = p6t3_board();
    let mut rooms = p6t3_seed_rooms(&mut board, tree_id);
    let item = ItemId(4);
    assert_eq!(board.item_tree_shape_count(item, tree_id), 3);
    let obstacle = {
        let (b, r) = (&mut board, &mut rooms);
        item_info::get_expansion_room(b, item, 1, tree_id, |b, i, idx, t| {
            r.new_obstacle_room(b, i, idx, t)
        })
    }
    .expect("the item has three tree shapes");
    let room = RoomRef::Obstacle(obstacle);
    assert_eq!(
        rooms.room_shape(room),
        Some(&TileShape::Octagon(IntOctagon::new(
            -130, -130, 130, 530, -584, 184, -184, 584
        )))
    );

    let result =
        SortedRoomNeighbours::calculate_neighbours(room, 2, &mut board, &mut rooms, tree_id, 4)
            .expect("an obstacle room completes");

    assert_eq!(
        result.completed_room, room,
        "an obstacle room is its own completion"
    );
    assert!(result.sorted_neighbours.is_empty());
    assert!(result.own_net_objects.is_empty());
    let doors = rooms.room_doors(room).to_vec();
    assert_eq!(doors.len(), 2);
    let described: Vec<(usize, i32)> = doors
        .iter()
        .map(|d| {
            let door = rooms.door(*d).expect("a live door");
            let RoomRef::Obstacle(other) = door.second_room else {
                panic!("both sides are obstacle rooms")
            };
            (
                rooms
                    .obstacle_room(other)
                    .expect("a live room")
                    .get_index_in_item(),
                door.dimension,
            )
        })
        .collect();
    assert_eq!(described, vec![(0, 2), (2, 2)]);
}

#[test]
fn calculate_new_incomplete_rooms_terminates_on_the_pinned_trigger() {
    let shape = TileShape::Octagon(IntOctagon::new(
        -5209, -4057, 1764, 1885, -7094, 5821, -9266, -1264,
    ));
    assert_eq!(shape.border_line_count(), 8);
    assert_eq!(
        shape.to_simplex().border_line_count(),
        5,
        "three of the octagon's eight constraints are redundant and `toSimplex()` drops them"
    );

    let (mut board, tree_id) = p6t3_board();
    let mut rooms = p6t3_seed_rooms(&mut board, tree_id);
    let incomplete = rooms.new_incomplete_room(Some(shape.clone()), 0, Some(shape.clone()));
    let room = RoomRef::Incomplete(incomplete);

    let result =
        SortedRoomNeighbours::calculate_neighbours(room, 3, &mut board, &mut rooms, tree_id, 900)
            .expect("an incomplete room completes");

    assert_eq!(
        result.room_shape.border_line_count(),
        5,
        "the room shape the side numbers index is the simplex, not the octagon"
    );
    assert!(
        !result.sorted_neighbours.is_empty(),
        "the trigger room must have neighbours, or the loop under test is never entered"
    );
    for neighbour in &result.sorted_neighbours {
        let side = neighbour.touching_side_no_of_room;
        assert!(
            side >= 0 && (side as usize) < result.room_shape.border_line_count(),
            "every touching side number is a line of the shape the loop walks; got {side} \
             against {} lines",
            result.room_shape.border_line_count()
        );
    }

    let (sender, receiver) = std::sync::mpsc::channel();
    let worker = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            let completed =
                SortedRoomNeighbours::complete(room, 3, &mut board, &mut rooms, tree_id);
            let doors = completed.map_or(0, |r| rooms.room_doors(r).len());
            sender.send(doors).expect("the receiver is alive");
        })
        .expect("a worker thread");
    let doors = receiver
        .recv_timeout(std::time::Duration::from_secs(30))
        .expect(
            "SortedRoomNeighbours::complete must terminate on quirk #162's trigger room; \
             it did not finish in 30 s, which is the unfixed behaviour (the loop allocates a \
             room and a door per turn until the heap is gone)",
        );
    worker.join().expect("the worker did not panic");
    assert!(
        doors > 0,
        "the completed room keeps the doors the loop built"
    );
}

#[test]
fn a_non_obstacle_of_the_routed_net_is_deferred_to_the_own_net_list() {
    let (mut board, tree_id) = p6t3_board();
    let mut rooms = p6t3_seed_rooms(&mut board, tree_id);
    let contained = IntBox::from_coords(1571, 885, 1716, 1165);

    let room = seed_free_space_room(&board, &mut rooms, tree_id, 0, contained, 2, 0, 1);
    let deferred =
        SortedRoomNeighbours::calculate_neighbours(room, 2, &mut board, &mut rooms, tree_id, 7)
            .expect("an incomplete room completes");
    assert_eq!(deferred.own_net_objects.len(), 1);
    assert_eq!(
        deferred.own_net_objects[0].object,
        TreeObject::Item(ItemId(5))
    );
    assert_eq!(deferred.own_net_objects[0].shape_index, 1);
    assert_eq!(deferred.sorted_neighbours.len(), 4);
    assert_eq!(
        deferred
            .sorted_neighbours
            .iter()
            .map(|n| n.search_tree_object)
            .collect::<Vec<_>>(),
        vec![
            TreeObject::Item(ItemId(16)),
            TreeObject::Item(ItemId(32)),
            TreeObject::Room(RoomId(0)),
            TreeObject::Item(ItemId(4)),
        ]
    );

    let room = seed_free_space_room(&board, &mut rooms, tree_id, 0, contained, 3, 0, 1);
    let not_deferred =
        SortedRoomNeighbours::calculate_neighbours(room, 3, &mut board, &mut rooms, tree_id, 8)
            .expect("an incomplete room completes");
    assert!(
        not_deferred.own_net_objects.is_empty(),
        "on net 3 every object is a trace obstacle, so nothing is deferred"
    );
}

#[test]
fn remove_all_doors_unlinks_both_sides_and_drops_incomplete_neighbours() {
    let mut rooms = ExpansionRoomStore::new();
    let complete = RoomRef::Complete(rooms.new_complete_room(
        Some(TileShape::Box(IntBox::from_coords(0, 0, 100, 100))),
        0,
        1,
    ));
    let incomplete = RoomRef::Incomplete(rooms.new_incomplete_room(
        Some(TileShape::Box(IntBox::from_coords(100, 0, 200, 100))),
        0,
        None,
    ));
    let other_complete = RoomRef::Complete(rooms.new_complete_room(
        Some(TileShape::Box(IntBox::from_coords(-100, 0, 0, 100))),
        0,
        2,
    ));
    for neighbour in [incomplete, other_complete] {
        let door = rooms.new_door(complete, neighbour, 1);
        rooms.add_door(complete, door);
        rooms.add_door(neighbour, door);
    }
    assert_eq!(rooms.room_doors(complete).len(), 2);

    rooms.remove_all_doors(complete);
    assert!(rooms.room_doors(complete).is_empty());
    assert!(rooms.room_doors(other_complete).is_empty(), "unlinked");
    let RoomRef::Incomplete(id) = incomplete else {
        unreachable!()
    };
    assert!(
        rooms.incomplete_room(id).is_none(),
        "an incomplete neighbour is removed from the engine's list as well"
    );
}

#[test]
fn a_room_bearing_tree_answers_queries_through_the_room_lookup() {
    let mut items: BTreeMap<ItemId, Item> = BTreeMap::new();
    items.insert(
        ItemId(1),
        Item::BoardOutline(BoardOutline::new(
            ItemHeader::new(ItemId(1), Vec::new(), 0, 0, FixedState::SystemFixed),
            Vec::new(),
        )),
    );
    let layers = LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)]);
    let rules = BoardRules::new(
        layers.clone(),
        ClearanceMatrix::get_default_instance(&layers, 200),
    );
    let library = BoardLibrary::new(Padstacks::new(layers.clone()), Packages::new());
    let components = Components::new();
    let bounding_box = IntBox::from_coords(-1000, -1000, 1000, 1000);
    let ctx = ItemCtx {
        library: &library,
        components: &components,
        rules: &rules,
        bounding_box: &bounding_box,
        max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
    };

    let mut tree = ShapeSearchTree::new(TreeId(7), AngleRestriction::None, 0);
    let mut rooms = ExpansionRoomStore::new();
    let id_no = rooms.next_room_id_no();
    let room = rooms.new_complete_room(
        Some(TileShape::Box(IntBox::from_coords(0, 0, 100, 100))),
        1,
        id_no,
    );
    rooms.insert_complete_room(&mut tree, room);

    let probe = TileShape::Box(IntBox::from_coords(-10, -10, 10, 10));
    let hits = tree.overlapping_tree_entries_with_rooms(&probe, Some(1), &[], &items, &rooms, &ctx);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].object, TreeObject::Room(room));
    assert!(
        tree.overlapping_tree_entries_with_rooms(&probe, Some(0), &[], &items, &rooms, &ctx)
            .is_empty()
    );
    assert_eq!(
        tree.overlapping_tree_entries_with_rooms(&probe, Some(1), &[1, 2], &items, &rooms, &ctx)
            .len(),
        1
    );

    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        tree.overlapping_tree_entries(&probe, Some(1), &[], &items, &ctx)
    }));
    assert!(
        panicked.is_err(),
        "the room-free overload passes NoRooms and still panics on a room leaf"
    );
}
