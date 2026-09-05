use std::collections::BTreeMap;

use fr_board::ids::{ItemId, RoomId, TreeId, TreeObject};
use fr_board::prelude::*;
use fr_geometry::{
    Area, IntBox, IntOctagon, IntPoint, IntVector, Point, Polyline, Shape, TileShape, Vector,
};
use fr_router::autoroute::expansion::{ExpansionRoomStore, IncompleteFreeSpaceExpansionRoom};
use fr_router::autoroute::tree_ext::AutorouteSearchTreeExt;

const BOARD: IntBox = IntBox {
    ll: IntPoint { x: 0, y: 0 },
    ur: IntPoint { x: 1000, y: 1000 },
};

struct TestBoard {
    library: BoardLibrary,
    components: Components,
    rules: BoardRules,
    bounding_box: IntBox,
    items: BTreeMap<ItemId, Item>,
    tree: ShapeSearchTree,
    rooms: ExpansionRoomStore,
}

impl TestBoard {
    fn new(angle: AngleRestriction, obstacles: &[(ItemId, TileShape, usize, Vec<i32>)]) -> Self {
        let layers =
            || LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)]);
        let clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 0);
        let mut rules = BoardRules::new(layers(), clearance_matrix);
        rules.trace_angle_restriction = angle;

        let mut items = BTreeMap::new();
        for (id, shape, layer, nets) in obstacles {
            let area = Area::Shape(Shape::Tile(shape.clone()));
            items.insert(
                *id,
                Item::ObstacleArea(ObstacleArea::new(
                    ItemHeader::new(*id, nets.clone(), 1, 0, FixedState::Unfixed),
                    ObstacleAreaData::new(area, *layer, Vector::ZERO, 0.0, false, None),
                )),
            );
        }

        let mut board = TestBoard {
            library: BoardLibrary::new(Padstacks::new(layers()), Packages::new()),
            components: Components::new(),
            rules,
            bounding_box: BOARD,
            items,
            tree: ShapeSearchTree::new(TreeId(1), angle, 0),
            rooms: ExpansionRoomStore::new(),
        };
        let mut taken = std::mem::take(&mut board.items);
        {
            let ctx = ItemCtx {
                library: &board.library,
                components: &board.components,
                rules: &board.rules,
                bounding_box: &board.bounding_box,
                max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
            };
            for item in taken.values_mut().rev() {
                board.tree.insert_item(item, &ctx);
            }
        }
        board.items = taken;
        board
    }

    fn ctx(&self) -> ItemCtx<'_> {
        ItemCtx {
            library: &self.library,
            components: &self.components,
            rules: &self.rules,
            bounding_box: &self.bounding_box,
            max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
        }
    }

    fn insert_room(&mut self, shape: TileShape, layer: usize) -> RoomId {
        let id_no = self.rooms.next_room_id_no();
        let room = self.rooms.new_complete_room(Some(shape), layer, id_no);
        self.rooms.insert_complete_room(&mut self.tree, room);
        room
    }

    fn complete(
        &self,
        room: &IncompleteFreeSpaceExpansionRoom,
        net_no: i32,
        ignore_object: Option<TreeObject>,
        ignore_shape: Option<&TileShape>,
    ) -> Vec<IncompleteFreeSpaceExpansionRoom> {
        self.tree.complete_shape(
            room,
            net_no,
            ignore_object,
            ignore_shape,
            &self.items,
            &self.rooms,
            &self.ctx(),
        )
    }
}

fn boxed(llx: i32, lly: i32, urx: i32, ury: i32) -> TileShape {
    TileShape::Box(IntBox::from_coords(llx, lly, urx, ury))
}

fn describe(rooms: &[IncompleteFreeSpaceExpansionRoom]) -> Vec<(usize, i32, IntBox, IntBox)> {
    rooms
        .iter()
        .map(|room| {
            let shape = room.get_shape().expect("a completed room has a shape");
            (
                room.get_layer(),
                shape.dimension(),
                shape.bounding_box(),
                room.get_contained_shape()
                    .map(TileShape::bounding_box)
                    .unwrap_or(IntBox::EMPTY),
            )
        })
        .collect()
}

#[test]
fn an_empty_board_returns_the_seed_room_unchanged() {
    let board = TestBoard::new(
        AngleRestriction::None,
        &[(ItemId(1), boxed(200, 200, 300, 300), 1, vec![])],
    );
    let seed = IncompleteFreeSpaceExpansionRoom::new(
        Some(boxed(100, 100, 400, 400)),
        0,
        Some(boxed(200, 200, 250, 250)),
    );
    let result = board.complete(&seed, 1, None, None);
    assert_eq!(
        describe(&result),
        vec![(
            0,
            2,
            IntBox::from_coords(100, 100, 400, 400),
            IntBox::from_coords(200, 200, 250, 250)
        )]
    );
}

#[test]
fn an_obstacle_fully_containing_the_seed_returns_nothing() {
    let board = TestBoard::new(
        AngleRestriction::None,
        &[(ItemId(1), boxed(0, 0, 1000, 1000), 0, vec![])],
    );
    let seed = IncompleteFreeSpaceExpansionRoom::new(
        Some(boxed(100, 100, 400, 400)),
        0,
        Some(boxed(200, 200, 250, 250)),
    );
    assert!(board.complete(&seed, 1, None, None).is_empty());
}

#[test]
fn divide_large_room_splits_at_the_board_bounds() {
    let tree = ShapeSearchTree::new(TreeId(1), AngleRestriction::None, 0);
    let room = IncompleteFreeSpaceExpansionRoom::new(
        Some(boxed(0, 0, 1000, 1000)),
        0,
        Some(boxed(400, 400, 600, 600)),
    );
    let sections = tree.divide_large_room(vec![room], &BOARD);
    assert_eq!(
        describe(&sections),
        vec![
            (
                0,
                2,
                IntBox::from_coords(0, 0, 500, 500),
                IntBox::from_coords(400, 400, 500, 500)
            ),
            (
                0,
                2,
                IntBox::from_coords(500, 0, 1000, 500),
                IntBox::from_coords(500, 400, 600, 500)
            ),
            (
                0,
                2,
                IntBox::from_coords(0, 500, 500, 1000),
                IntBox::from_coords(400, 500, 500, 600)
            ),
            (
                0,
                2,
                IntBox::from_coords(500, 500, 1000, 1000),
                IntBox::from_coords(500, 500, 600, 600)
            ),
        ]
    );

    let small = IncompleteFreeSpaceExpansionRoom::new(
        Some(boxed(0, 0, 1000, 400)),
        0,
        Some(boxed(400, 100, 600, 300)),
    );
    assert_eq!(tree.divide_large_room(vec![small], &BOARD).len(), 1);
}

#[test]
fn the_45_degree_override_is_not_the_base_algorithm() {
    let obstacle = TileShape::Octagon(IntOctagon::new(300, 300, 700, 700, -200, 1200, 0, 1000));
    let obstacles = [(ItemId(1), obstacle, 0, vec![])];
    let seed = || IncompleteFreeSpaceExpansionRoom::new(None, 0, Some(boxed(900, 50, 960, 110)));

    let base = TestBoard::new(AngleRestriction::None, &obstacles);
    let base_rooms = base.complete(&seed(), 1, None, None);
    let deg45 = TestBoard::new(AngleRestriction::FortyFiveDegree, &obstacles);
    let deg45_rooms = deg45.complete(&seed(), 1, None, None);

    assert_eq!(
        describe(&base_rooms),
        vec![(
            0,
            2,
            IntBox::from_coords(0, 0, 1000, 300),
            IntBox::from_coords(900, 50, 960, 110)
        )],
        "the base class cuts below the obstacle's lower-right diagonal"
    );
    assert_eq!(
        describe(&deg45_rooms),
        vec![(
            0,
            2,
            IntBox::from_coords(700, 0, 1000, 1000),
            IntBox::from_coords(900, 50, 960, 110)
        )],
        "the 45-degree override cuts to the right of it"
    );
    assert!(
        matches!(base_rooms[0].get_shape(), Some(TileShape::Simplex(_))),
        "the base algorithm answers half-plane intersections"
    );
    assert!(
        matches!(deg45_rooms[0].get_shape(), Some(TileShape::Octagon(_))),
        "the 45-degree override answers octagons only"
    );
}

#[test]
fn the_90_degree_override_splits_around_a_straddling_obstacle() {
    let board = TestBoard::new(
        AngleRestriction::NinetyDegree,
        &[(ItemId(1), boxed(200, 200, 800, 800), 0, vec![])],
    );
    let seed = IncompleteFreeSpaceExpansionRoom::new(None, 0, Some(boxed(100, 100, 900, 900)));
    assert_eq!(
        describe(&board.complete(&seed, 1, None, None)),
        vec![
            (
                0,
                2,
                IntBox::from_coords(0, 0, 200, 1000),
                IntBox::from_coords(100, 100, 200, 900)
            ),
            (
                0,
                2,
                IntBox::from_coords(800, 0, 1000, 1000),
                IntBox::from_coords(800, 100, 900, 900)
            ),
            (
                0,
                2,
                IntBox::from_coords(200, 0, 800, 200),
                IntBox::from_coords(200, 100, 800, 200)
            ),
            (
                0,
                2,
                IntBox::from_coords(200, 800, 800, 1000),
                IntBox::from_coords(200, 800, 800, 900)
            ),
        ]
    );
}

#[test]
fn the_90_degree_override_never_calls_divide_large_room() {
    let board = TestBoard::new(
        AngleRestriction::NinetyDegree,
        &[(ItemId(1), boxed(200, 200, 300, 300), 1, vec![])],
    );
    let seed = IncompleteFreeSpaceExpansionRoom::new(
        Some(boxed(0, 0, 1000, 1000)),
        0,
        Some(boxed(400, 400, 600, 600)),
    );
    let result = board.complete(&seed, 1, None, None);
    assert_eq!(
        describe(&result),
        vec![(
            0,
            2,
            IntBox::from_coords(0, 0, 1000, 1000),
            IntBox::from_coords(400, 400, 600, 600)
        )]
    );

    let base = TestBoard::new(
        AngleRestriction::None,
        &[(ItemId(1), boxed(200, 200, 300, 300), 1, vec![])],
    );
    assert_eq!(base.complete(&seed, 1, None, None).len(), 4);
}

#[test]
fn an_octagon_obstacle_restrains_the_45_degree_room_on_a_diagonal() {
    let obstacle = TileShape::Octagon(IntOctagon::new(200, 200, 800, 800, -300, 1300, -300, 1300));
    let board = TestBoard::new(
        AngleRestriction::FortyFiveDegree,
        &[(ItemId(1), obstacle, 0, vec![])],
    );
    let seed = IncompleteFreeSpaceExpansionRoom::new(None, 0, Some(boxed(10, 10, 60, 60)));
    let result = board.complete(&seed, 1, None, None);
    assert!(!result.is_empty());
    for room in &result {
        let shape = room.get_shape().expect("a shape");
        assert!(matches!(shape, TileShape::Octagon(_)));
        assert_eq!(
            shape.intersection(&boxed(400, 400, 600, 600)).dimension(),
            -1
        );
    }
}

#[test]
fn the_ninety_degree_override_keeps_the_room_it_ignores_by_shape() {
    let ignore = boxed(300, 300, 700, 700);
    let mut results = Vec::new();
    for angle in [
        AngleRestriction::None,
        AngleRestriction::FortyFiveDegree,
        AngleRestriction::NinetyDegree,
    ] {
        let mut board = TestBoard::new(angle, &[(ItemId(1), boxed(10, 900, 40, 950), 1, vec![])]);
        board.insert_room(boxed(400, 400, 600, 600), 0);
        let seed = IncompleteFreeSpaceExpansionRoom::new(None, 0, Some(boxed(100, 100, 200, 200)));
        results.push(board.complete(&seed, 1, None, Some(&ignore)).len());
    }
    assert_eq!(
        results,
        vec![4, 4, 1],
        "all three regimes now KEEP the ignored room (was [4, 4, 0]); the 1 against the 4s is \
         the third, unregistered difference — the 90-degree override does not divide"
    );
}

const P6T2_BOUNDING_BOX: IntBox = IntBox {
    ll: IntPoint {
        x: -10_000,
        y: -10_000,
    },
    ur: IntPoint {
        x: 10_000,
        y: 10_000,
    },
};

const P6T2_OBSTACLES: [(u32, usize, [i32; 4]); 20] = [
    (6, 0, [8496, 4150, 9056, 6521]),
    (7, 1, [-3980, -333, -1897, 700]),
    (8, 1, [5966, 4944, 8451, 6759]),
    (9, 0, [-5966, 5831, -5138, 7132]),
    (10, 1, [-4976, 2554, -3810, 3700]),
    (11, 0, [-4147, 7504, -3492, 8411]),
    (12, 0, [-3968, 8539, -2604, 10079]),
    (13, 1, [-5816, 5285, -4137, 7397]),
    (14, 1, [4427, 432, 6018, 2925]),
    (15, 0, [-5859, 1509, -5383, 3660]),
    (16, 0, [-209, 6836, 449, 7816]),
    (17, 0, [7485, 2949, 7672, 3378]),
    (18, 1, [8407, 5395, 9709, 7594]),
    (19, 0, [8739, 2522, 10425, 2733]),
    (20, 1, [-1504, 7993, 926, 9633]),
    (21, 1, [-4656, 4358, -4386, 6625]),
    (22, 1, [-6393, 705, -5996, 1847]),
    (23, 1, [-4849, -7918, -3659, -7284]),
    (24, 0, [-5955, -3513, -5309, -1234]),
    (25, 1, [4098, -8459, 5378, -6908]),
];

const P6T2_SEED_ROOMS: [(usize, [i32; 4]); 3] = [
    (1, [1386, -2517, 2807, 458]),
    (0, [2811, 8147, 3647, 10698]),
    (0, [-1114, 1885, 790, 4531]),
];

struct P6t2Board {
    library: BoardLibrary,
    components: Components,
    rules: BoardRules,
    bounding_box: IntBox,
    items: BTreeMap<ItemId, Item>,
    manager: SearchTreeManager,
    rooms: ExpansionRoomStore,
    seed_rooms: Vec<RoomId>,
    tree_id: TreeId,
}

impl P6t2Board {
    fn ctx(&self) -> ItemCtx<'_> {
        ItemCtx {
            library: &self.library,
            components: &self.components,
            rules: &self.rules,
            bounding_box: &self.bounding_box,
            max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
        }
    }

    fn tree(&self) -> &ShapeSearchTree {
        self.manager
            .trees()
            .find(|tree| tree.id() == self.tree_id)
            .expect("the autoroute tree")
    }

    fn complete(
        &self,
        room: &IncompleteFreeSpaceExpansionRoom,
        net_no: i32,
        ignore_object: Option<TreeObject>,
        ignore_shape: Option<&TileShape>,
    ) -> Vec<IncompleteFreeSpaceExpansionRoom> {
        self.tree().complete_shape(
            room,
            net_no,
            ignore_object,
            ignore_shape,
            &self.items,
            &self.rooms,
            &self.ctx(),
        )
    }
}

fn p6t2_board(angle: AngleRestriction) -> P6t2Board {
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
    let library = BoardLibrary::new(padstacks, packages);
    let mut components = Components::new();
    components.add_with_generated_name(Some(Point::new(0, 0)), 0.0, true, package);

    let mut items = BTreeMap::new();
    items.insert(
        ItemId(1),
        Item::BoardOutline(BoardOutline::new(
            ItemHeader::new(ItemId(1), Vec::new(), 0, 0, FixedState::SystemFixed),
            Vec::new(),
        )),
    );
    items.insert(
        ItemId(2),
        Item::Pin(Pin::new(
            ItemHeader::new(ItemId(2), vec![1], 1, 1, FixedState::Unfixed),
            0,
        )),
    );
    items.insert(
        ItemId(3),
        Item::Pin(Pin::new(
            ItemHeader::new(ItemId(3), vec![1], 1, 1, FixedState::Unfixed),
            1,
        )),
    );
    items.insert(
        ItemId(4),
        Item::Trace(PolylineTrace::new(
            ItemHeader::new(ItemId(4), vec![1], 1, 0, FixedState::Unfixed),
            Polyline::from_points(&[
                Point::new(-500, 0),
                Point::new(0, 0),
                Point::new(0, 400),
                Point::new(500, 400),
            ]),
            0,
            30,
            None,
        )),
    );
    items.insert(
        ItemId(5),
        Item::Trace(PolylineTrace::new(
            ItemHeader::new(ItemId(5), vec![2], 2, 0, FixedState::Unfixed),
            Polyline::from_points(&[
                Point::new(-800, 300),
                Point::new(-800, 900),
                Point::new(300, 900),
            ]),
            0,
            40,
            None,
        )),
    );
    for (id, layer, c) in P6T2_OBSTACLES {
        let id = ItemId(id);
        items.insert(
            id,
            Item::ObstacleArea(ObstacleArea::new(
                ItemHeader::new(id, Vec::new(), 1, 0, FixedState::Unfixed),
                ObstacleAreaData::new(
                    Area::Shape(Shape::Tile(boxed(c[0], c[1], c[2], c[3]))),
                    layer,
                    Vector::ZERO,
                    0.0,
                    false,
                    None,
                ),
            )),
        );
    }

    let mut manager = SearchTreeManager::new();
    let bounding_box = P6T2_BOUNDING_BOX;
    let tree_id = {
        let ctx = ItemCtx {
            library: &library,
            components: &components,
            rules: &rules,
            bounding_box: &bounding_box,
            max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
        };
        for item in items.values_mut().rev() {
            manager.insert(item, &ctx);
        }
        let mut refs: Vec<&mut Item> = items.values_mut().rev().collect();
        manager.get_autoroute_tree(1, &mut refs, &ctx).id()
    };

    let mut rooms = ExpansionRoomStore::new();
    let mut seed_rooms = Vec::new();
    {
        let tree = manager
            .trees_mut()
            .find(|tree| tree.id() == tree_id)
            .expect("the autoroute tree");
        for (layer, c) in P6T2_SEED_ROOMS {
            let id_no = rooms.next_room_id_no();
            let room = rooms.new_complete_room(Some(boxed(c[0], c[1], c[2], c[3])), layer, id_no);
            rooms.insert_complete_room(tree, room);
            seed_rooms.push(room);
        }
    }

    P6t2Board {
        library,
        components,
        rules,
        bounding_box,
        items,
        manager,
        rooms,
        seed_rooms,
        tree_id,
    }
}

fn render(rooms: &[IncompleteFreeSpaceExpansionRoom]) -> Vec<String> {
    rooms
        .iter()
        .map(|room| {
            format!(
                "layer={} dim={} shape={} contained={}",
                room.get_layer(),
                room.get_shape()
                    .map_or("null".to_string(), |s| s.dimension().to_string()),
                opt_shp(room.get_shape()),
                opt_shp(room.get_contained_shape())
            )
        })
        .collect()
}

fn opt_shp(shape: Option<&TileShape>) -> String {
    shape.map_or("null".to_string(), shp)
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

#[test]
fn the_base_regime_matches_the_p6t2_script() {
    let board = p6t2_board(AngleRestriction::None);
    let room =
        IncompleteFreeSpaceExpansionRoom::new(None, 0, Some(boxed(3456, -3011, 3469, -2803)));
    let expected = vec![
        "layer=0 dim=2 shape=Oct[-5209,-10000,2396,-4057,781,12396,-15209,-1661] \
         contained=Simplex{}"
            .to_string(),
        "layer=0 dim=2 shape=Box[2396,-10000..10000,-4057] contained=Simplex{}".to_string(),
        "layer=0 dim=2 shape=Oct[-3276,-4057,2396,1615,781,6453,-7333,4011] \
         contained=Simplex{}"
            .to_string(),
        "layer=0 dim=2 shape=Oct[2396,-4057,10000,1885,781,14057,-1661,11885] \
         contained=Simplex{((3456,-3011)->(3457,-3011))((3469,-2803)->(3469,-2802))\
         ((3469,-2803)->(3468,-2803))((3456,-3011)->(3456,-3012))}"
            .to_string(),
    ];
    let completed = board.complete(&room, 3, Some(TreeObject::Room(board.seed_rooms[1])), None);
    assert_eq!(render(&completed), expected);
    let divided = board
        .tree()
        .divide_large_room(completed, &P6T2_BOUNDING_BOX);
    assert_eq!(render(&divided), expected);
}

#[test]
fn the_90_degree_regime_matches_the_p6t2_script() {
    let board = p6t2_board(AngleRestriction::NinetyDegree);
    let room =
        IncompleteFreeSpaceExpansionRoom::new(None, 0, Some(boxed(-6067, -1181, -5767, -1030)));
    let expected = vec![
        "layer=0 dim=2 shape=Box[-10000,-10000..-6055,1409] \
         contained=Box[-6067,-1181..-6055,-1030]"
            .to_string(),
        "layer=0 dim=2 shape=Box[-6055,-1134..-1340,1409] \
         contained=Box[-6055,-1134..-5767,-1030]"
            .to_string(),
    ];
    let completed = board.complete(
        &room,
        3,
        Some(TreeObject::Room(board.seed_rooms[2])),
        Some(&boxed(-1233, 1766, 909, 4650)),
    );
    assert_eq!(render(&completed), expected);
    let divided = board
        .tree()
        .divide_large_room(completed, &P6T2_BOUNDING_BOX);
    assert_eq!(render(&divided), expected);
}

#[test]
fn the_45_degree_regime_matches_the_p6t2_script() {
    let board = p6t2_board(AngleRestriction::FortyFiveDegree);
    let room =
        IncompleteFreeSpaceExpansionRoom::new(None, 0, Some(boxed(-6067, -1181, -5767, -1030)));
    let expected = vec![
        "layer=0 dim=2 shape=Oct[-10000,-1134,-1340,1409,-11409,-206,-11134,69] \
         contained=Oct[-6067,-1134,-5767,-1030,-5037,-4633,-7201,-6797]"
            .to_string(),
        "layer=0 dim=2 shape=Oct[-10000,-5138,-5996,-1134,-8866,-4862,-15138,-7130] \
         contained=Oct[-6067,-1181,-5996,-1134,-4933,-4862,-7248,-7130]"
            .to_string(),
    ];
    let completed = board.complete(
        &room,
        3,
        Some(TreeObject::Room(board.seed_rooms[2])),
        Some(&boxed(-1233, 1766, 909, 4650)),
    );
    assert_eq!(render(&completed), expected);
    let divided = board
        .tree()
        .divide_large_room(completed, &P6T2_BOUNDING_BOX);
    assert_eq!(render(&divided), expected);
}

const NINETY_DEGREE_STEM: &str = "p9t8-ninety-degree";

#[test]
fn the_ninety_degree_fixture_routes_and_every_segment_is_axis_aligned() {
    let ses = route_ninety_degree_fixture();

    for net in ["NA", "NB"] {
        assert!(
            ses.contains(&format!("(net {net}")),
            "net {net} must be routed on the 90-degree fixture; SES was:\n{ses}"
        );
    }

    let mut segments = 0usize;
    for chunk in ses.split("(path ").skip(1) {
        let body = &chunk[..chunk.find(')').unwrap_or(chunk.len())];
        let mut tokens = body.split_whitespace();
        let _layer = tokens.next();
        let _width = tokens.next();
        let coords: Vec<f64> = tokens.filter_map(|t| t.parse().ok()).collect();
        assert!(
            coords.len() >= 4 && coords.len().is_multiple_of(2),
            "a path has corners"
        );
        let points: Vec<&[f64]> = coords.chunks_exact(2).collect();
        for pair in points.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            assert!(
                a[0] == b[0] || a[1] == b[1],
                "a 90-degree board must emit only axis-aligned segments, but \
                 ({},{}) -> ({},{}) is diagonal",
                a[0],
                a[1],
                b[0],
                b[1]
            );
            segments += 1;
        }
    }
    assert!(segments > 0, "the fixture routes something");
}

fn route_ninety_degree_fixture() -> String {
    use fr_dsn::{BoardReadResult, DsnReadOptions};
    use fr_router::pipeline::{
        NoopProgressSink, RouterBudget, RouterStop, prepare_board, run_pipeline,
    };
    use fr_settings::sources::{CliSettings, DsnFileSettings, EnvironmentVariablesSource};
    use fr_settings::{HostEnvironment, SettingsInputs, SettingsSource, resolve_headless};

    let root = parity::workspace_root();
    let dsn = root.join(format!(
        "crates/fr-router/tests/data/{NINETY_DEGREE_STEM}.dsn"
    ));
    let bytes =
        std::fs::read(&dsn).unwrap_or_else(|e| panic!("cannot read {}: {e}", dsn.display()));
    let file_name = format!("{NINETY_DEGREE_STEM}.dsn");

    let (mut board, transform) = match fr_dsn::read_board(
        std::io::Cursor::new(&bytes[..]),
        None,
        Some(&file_name),
        &DsnReadOptions::default(),
    ) {
        BoardReadResult::Success {
            board,
            coordinate_transform,
            ..
        } => (
            *board.expect("the fixture produces a board"),
            coordinate_transform.expect("the fixture produces a coordinate transform"),
        ),
        other => panic!("{NINETY_DEGREE_STEM} did not read: {other:?}"),
    };
    assert_eq!(
        board.rules.trace_angle_restriction,
        AngleRestriction::NinetyDegree,
        "the fixture's `(snap_angle ninety_degree)` must reach the board rules, or the regime \
         under test is not the one being exercised"
    );

    let argv = vec!["-de".to_string(), dsn.display().to_string()];
    let dsn_source = DsnFileSettings::new(&bytes[..], &file_name);
    let env_map: std::collections::BTreeMap<String, String> = std::env::vars().collect();
    let env_source = EnvironmentVariablesSource::new(&env_map);
    let cli_source = CliSettings::new(&argv);
    let inputs = SettingsInputs {
        json_file: None,
        dsn: dsn_source.get_settings(),
        cli_rules: None,
        scheduler_rules: None,
        env: env_source.get_settings(),
        cli: cli_source.get_settings(),
    };
    let settings = resolve_headless(&inputs, Some(&board), &HostEnvironment::detect());
    prepare_board(&mut board, &settings);

    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    run_pipeline(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut sink,
    )
    .expect("the fixture has a routable signal layer");

    let mut ses = Vec::new();
    fr_dsn::ses_writer::write(&board, &transform, &mut ses, NINETY_DEGREE_STEM)
        .expect("the SES writer cannot fail on a Vec");
    String::from_utf8(ses).expect("the SES writer emits UTF-8")
}
