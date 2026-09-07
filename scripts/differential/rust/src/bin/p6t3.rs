//! Rust twin of `scripts/differential/java/P6T3.java` (Plan 6 Task 4).
//!
//! `SortedRoomNeighbours` — the any-angle neighbour sorter: the sorted neighbour list its
//! comparator produces, the own-net list it defers, and the doors `calculateNeighbours` builds.
//!
//! * **mode 0** builds the `P2T10` board plus `n` random obstacle areas, gets the autoroute tree,
//!   seeds it with three complete expansion rooms, and then for `rooms` random seed rooms — a
//!   `complete_shape` output room, or an obstacle room over a random item shape — calls
//!   `SortedRoomNeighbours::calculate_neighbours` and prints everything it produced.
//! * **mode 1** is the hazard-F probe: `SortedRoomNeighbour`s built directly and inserted into the
//!   ordered set, printing what survives.
//!
//! args: `mode seed n rooms`.

use copper_board::ids::TreeObject;
use copper_board::prelude::*;
use copper_dsn::format::format_double;
use copper_geometry::{
    Area, IntBox, IntOctagon, IntPoint, IntVector, Point, Polyline, PolylineShapeRef, Shape,
    TileShape,
};
use copper_router::IncompleteRoomId;
use copper_router::autoroute::expansion::sorted_neighbours::{
    SortedRoomNeighbour, SortedRoomNeighbours,
};
use copper_router::autoroute::expansion::sorted_neighbours_45::Sorted45DegreeRoomNeighbours;
use copper_router::autoroute::expansion::sorted_neighbours_orthogonal::SortedOrthogonalRoomNeighbours;
use copper_router::autoroute::expansion::{
    ExpansionRoomStore, IncompleteFreeSpaceExpansionRoom, RoomRef,
};
use copper_router::autoroute::item_info;
use copper_router::autoroute::tree_ext::AutorouteSearchTreeExt;

const RANGE: i32 = 9000;
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

// -------------------------------------------------------------------------------------------
// The shared xorshift stream
// -------------------------------------------------------------------------------------------

struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: i64) -> Rng {
        Rng {
            state: if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed as u64
            },
        }
    }

    fn next(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }

    fn rnd(&mut self, bound: i32) -> i32 {
        (self.next() % bound as u64) as i32
    }

    fn coord(&mut self, range: i32) -> i32 {
        self.rnd(2 * range + 1) - range
    }

    fn box_(&mut self, range: i32, min_size: i32, max_size: i32) -> IntBox {
        let w = min_size + self.rnd(max_size - min_size + 1);
        let h = min_size + self.rnd(max_size - min_size + 1);
        let x = self.coord(range);
        let y = self.coord(range);
        IntBox::from_coords(x, y, x + w, y + h)
    }

    /// A box whose four ordinates are multiples of 500.
    fn grid_box(&mut self) -> IntBox {
        let w = 500 * (1 + self.rnd(4));
        let h = 500 * (1 + self.rnd(4));
        let x = 500 * (self.rnd(33) - 16);
        let y = 500 * (self.rnd(33) - 16);
        IntBox::from_coords(x, y, x + w, y + h)
    }
}

// -------------------------------------------------------------------------------------------
// The board — `P2T10.build`, verbatim, in the any-angle regime
// -------------------------------------------------------------------------------------------

fn build(mode: i32) -> Board {
    let layers = || LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)]);
    let mut clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
    assert!(clearance_matrix.append_class("wide"));
    clearance_matrix.set_value_on_all_layers(2, 1, 600);
    clearance_matrix.set_value_on_all_layers(2, 2, 800);
    let mut rules = BoardRules::new(layers(), clearance_matrix);
    // Modes 6 and 8 drive the 45-degree tree, modes 7 and 9 the 90-degree one — see `P6T3.java`.
    rules.trace_angle_restriction = match mode {
        6 | 8 => AngleRestriction::FortyFiveDegree,
        7 | 9 => AngleRestriction::NinetyDegree,
        _ => AngleRestriction::None,
    };

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

    let outline: Vec<PolylineShapeRef> = Vec::new();
    let mut board = Board::new(
        outline,
        0,
        BOUNDING_BOX,
        rules,
        library,
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

// -------------------------------------------------------------------------------------------

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode: i32 = args.first().map_or(0, |s| s.parse().expect("mode"));
    let seed: i64 = args.get(1).map_or(42, |s| s.parse().expect("seed"));
    let obstacle_count: i32 = args.get(2).map_or(20, |s| s.parse().expect("n"));
    let room_count: i32 = args.get(3).map_or(1000, |s| s.parse().expect("rooms"));

    println!("mode=p6t3 submode={mode} seed={seed} obstacles={obstacle_count} rooms={room_count}");

    let mut rng = Rng::new(seed);
    if mode == 1 || mode == 2 {
        comparator_probe(&mut rng, room_count, mode == 2);
        return;
    }
    if mode == 3 {
        corner_touch_probe(&mut rng, room_count);
        return;
    }

    let mut board = build(mode);
    for _ in 0..obstacle_count {
        // modes 4, 6 and 7 snap every obstacle to a 500-unit grid — see `P6T3.java` for why:
        // without it `calculateNeighbours`' whole dimension-0 branch stays dead.
        let shape = if mode == 4 || mode == 6 || mode == 7 {
            rng.grid_box()
        } else {
            rng.box_(RANGE, 100, 2500)
        };
        let layer = rng.rnd(2) as usize;
        board.insert_obstacle(
            Area::Shape(Shape::Tile(TileShape::Box(shape))),
            layer,
            1,
            FixedState::Unfixed,
        );
    }

    // `SearchTreeManager.getAutorouteTree(1)` — the item list is walked in board order
    // (descending id, quirk #63).
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
    {
        let tree = tree_of(&board, tree_id);
        println!(
            "regime={} angle={} size={} items={}",
            regime_of(mode),
            angle_name(board.rules.trace_angle_restriction),
            tree.size(),
            board.items.len()
        );
    }
    {
        let ctx = board.ctx();
        for id in board.items.keys().rev() {
            let item = &board.items[id];
            println!(
                "  item id={} class={} layers={}..{} tileShapeCount={} routable={} bbox={}",
                id.0,
                class_name(item),
                item.first_layer(&ctx),
                item.last_layer(&ctx),
                item.tile_shape_count(&ctx),
                item.is_routable(),
                b(&item.bounding_box(&ctx))
            );
        }
    }

    let mut rooms = ExpansionRoomStore::new();
    let mut room_id_counter = 0i32;
    // Three complete expansion rooms, inserted exactly as `AutorouteEngine.addCompleteRoom` does
    // (AutorouteEngine.java:534).
    for _ in 0..3 {
        let shape = rng.box_(RANGE, 500, 3000);
        let layer = rng.rnd(2) as usize;
        // In mode 5 the ids come from the store's own counter — the one
        // `SortedRoomNeighbours::calculate` then draws from — so that it stays in lockstep with
        // Java's `AutorouteEngine.expansionRoomInstanceCount`.
        room_id_counter = if mode == 5 || mode == 8 || mode == 9 {
            rooms.next_room_id_no()
        } else {
            room_id_counter + 1
        };
        let room = rooms.new_complete_room(Some(TileShape::Box(shape)), layer, room_id_counter);
        {
            let tree = board
                .trees
                .trees_mut()
                .find(|tree| tree.id() == tree_id)
                .expect("the autoroute tree");
            rooms.insert_complete_room(tree, room);
        }
        println!(
            "  seedRoom {room_id_counter} layer={layer} shape={}",
            shp(&TileShape::Box(shape))
        );
    }
    println!("  treeSizeWithRooms={}", tree_of(&board, tree_id).size());

    let mut seed_incomplete: Vec<IncompleteRoomId> = Vec::new();
    for index in 0..room_count {
        if mode == 5 || mode == 8 || mode == 9 {
            run_one_complete(
                &mut board,
                &mut rooms,
                tree_id,
                &mut rng,
                &mut seed_incomplete,
                index,
                mode,
            );
        } else {
            run_one(
                &mut board,
                &mut rooms,
                tree_id,
                &mut rng,
                &mut room_id_counter,
                index,
                mode,
            );
        }
    }
    if mode == 6 || mode == 7 {
        run_overlap_probe(&mut board, &mut rooms, tree_id, &mut room_id_counter, mode);
    }
}

/// The free-space **2-dimensional overlap** arm of `Sorted45DegreeRoomNeighbours.java:132` /
/// `SortedOrthogonalRoomNeighbours.java:168` — the `&&` the base class does not have. See
/// `P6T3.java`'s `runOverlapProbe` for why the random loop cannot reach it and why this arm is
/// the only place a `dimension == 2` door to a free-space room is ever built.
fn run_overlap_probe(
    board: &mut Board,
    rooms: &mut ExpansionRoomStore,
    tree_id: TreeId,
    room_id_counter: &mut i32,
    mode: i32,
) {
    let overlap_box = IntBox::from_coords(-2500, -8500, 500, -6500);
    *room_id_counter += 1;
    let overlap_id = *room_id_counter;
    let overlap_room = rooms.new_complete_room(Some(TileShape::Box(overlap_box)), 1, overlap_id);
    {
        let tree = board
            .trees
            .trees_mut()
            .find(|tree| tree.id() == tree_id)
            .expect("the autoroute tree");
        rooms.insert_complete_room(tree, overlap_room);
    }
    let room_box = IntBox::from_coords(-4000, -9500, -1000, -7500);
    *room_id_counter += 1;
    let room_id_no = *room_id_counter;
    let room = RoomRef::Incomplete(rooms.new_incomplete_room(
        Some(TileShape::Box(room_box)),
        1,
        Some(TileShape::Box(room_box)),
    ));
    println!(
        "overlap regime={} overlapRoom={overlap_id} overlapShape={} room={} net=1 \
         roomIdNo={room_id_no}",
        regime_of(mode),
        shp(&TileShape::Box(overlap_box)),
        shp(&TileShape::Box(room_box))
    );
    if mode == 6 {
        let Some(result) = Sorted45DegreeRoomNeighbours::calculate_neighbours(
            room, 1, board, rooms, tree_id, room_id_no,
        ) else {
            println!("  result=null");
            return;
        };
        dump_45_neighbours(&result, rooms);
        println!("  completedRoom={}", desc(result.completed_room, rooms));
        dump_doors(result.completed_room, rooms);
        dump_target_doors(result.completed_room, rooms);
    } else {
        let Some(result) = SortedOrthogonalRoomNeighbours::calculate_neighbours(
            room, 1, board, rooms, tree_id, room_id_no,
        ) else {
            println!("  result=null");
            return;
        };
        dump_orthogonal_neighbours(&result, rooms);
        println!("  completedRoom={}", desc(result.completed_room, rooms));
        dump_doors(result.completed_room, rooms);
        dump_target_doors(result.completed_room, rooms);
    }
}

/// `P6T3.dumpRegimeNeighbours` for the 45-degree inner class, plus its `edgeTouches` line.
fn dump_45_neighbours(result: &Sorted45DegreeRoomNeighbours, rooms: &ExpansionRoomStore) {
    println!("  neighbours n={}", result.sorted_neighbours.len());
    for (i, n) in result.sorted_neighbours.iter().enumerate() {
        println!(
            "    [{i}] fts={} lts={} obj={} nshape={} nshapeCorners={} isect={} isectCorners={}",
            n.first_touching_side,
            n.last_touching_side,
            describe_object(n.search_tree_object, rooms),
            shp(&TileShape::Octagon(n.shape)),
            corners(Some(&TileShape::Octagon(n.shape))),
            shp(&TileShape::Octagon(n.intersection)),
            corners(Some(&TileShape::Octagon(n.intersection)))
        );
    }
    println!(
        "  edgeTouches={}",
        flags(&result.edge_interior_touches_obstacle)
    );
}

/// `P6T3.dumpRegimeNeighbours` for the orthogonal inner class, plus its `edgeTouches` line.
fn dump_orthogonal_neighbours(result: &SortedOrthogonalRoomNeighbours, rooms: &ExpansionRoomStore) {
    println!("  neighbours n={}", result.sorted_neighbours.len());
    for (i, n) in result.sorted_neighbours.iter().enumerate() {
        println!(
            "    [{i}] fts={} lts={} obj={} nshape={} nshapeCorners={} isect={} isectCorners={}",
            n.first_touching_side,
            n.last_touching_side,
            describe_object(n.search_tree_object, rooms),
            shp(&TileShape::Box(n.shape)),
            corners(Some(&TileShape::Box(n.shape))),
            shp(&TileShape::Box(n.intersection)),
            corners(Some(&TileShape::Box(n.intersection)))
        );
    }
    println!(
        "  edgeTouches={}",
        flags(&result.edge_interior_touches_obstacle)
    );
}

/// 0 for the any-angle regime, 1 for 90 degrees, 2 for 45 degrees — `p6t2`'s numbering.
fn regime_of(mode: i32) -> i32 {
    match mode {
        6 | 8 => 2,
        7 | 9 => 1,
        _ => 0,
    }
}

/// mode 5: the **whole** of `SortedRoomNeighbours::complete` — `try_remove_edge`,
/// `calculate_new_incomplete_rooms`, `calculate_incomplete_rooms_with_empty_neighbours` and
/// `calculate_target_doors`, none of which mode 0 reaches. See `P6T3.java`'s `runOneComplete`.
#[allow(clippy::too_many_arguments)]
fn run_one_complete(
    board: &mut Board,
    rooms: &mut ExpansionRoomStore,
    tree_id: TreeId,
    rng: &mut Rng,
    seed_incomplete: &mut Vec<IncompleteRoomId>,
    index: i32,
    mode: i32,
) {
    let kind = rng.rnd(4);
    let net_number = 1 + rng.rnd(3);
    let layer = rng.rnd(2) as usize;
    let contained_box = rng.box_(RANGE, 10, 300);
    let item_id = if rng.rnd(4) == 0 {
        2 + rng.rnd(4)
    } else {
        4 + rng.rnd(2)
    };

    let (room, description) = if kind == 3 {
        let item_id = ItemId(item_id as u32);
        let shape_count = board.item_tree_shape_count(item_id, tree_id);
        if shape_count == 0 {
            println!(
                "call i={index} kind=obstacle item={} skipped=noShapes",
                item_id.0
            );
            return;
        }
        let index_in_item = rng.rnd(shape_count as i32) as usize;
        let obstacle = {
            let (b, r) = (&mut *board, &mut *rooms);
            item_info::get_expansion_room(b, item_id, index_in_item, tree_id, |b, i, idx, t| {
                r.new_obstacle_room(b, i, idx, t)
            })
        }
        .expect("the index came from treeShapeCount");
        (
            RoomRef::Obstacle(obstacle),
            format!("obstacle item={} indexInItem={index_in_item}", item_id.0),
        )
    } else {
        let seed =
            IncompleteFreeSpaceExpansionRoom::new(None, layer, Some(TileShape::Box(contained_box)));
        let completed = {
            let ctx = board.ctx();
            tree_of(board, tree_id).complete_shape(
                &seed,
                net_number,
                None,
                None,
                &board.items,
                &*rooms,
                &ctx,
            )
        };
        if completed.is_empty() {
            println!(
                "call i={index} kind=freeSpace layer={layer} contained={} skipped=noCompletedShape",
                shp(&TileShape::Box(contained_box))
            );
            return;
        }
        let pick = rng.rnd(completed.len() as i32) as usize;
        let chosen = &completed[pick];
        let description = format!(
            "freeSpace layer={layer} contained={} candidates={} pick={pick}",
            shp(&TileShape::Box(contained_box)),
            completed.len()
        );
        let id = rooms.new_incomplete_room(
            chosen.get_shape().cloned(),
            chosen.get_layer(),
            chosen.get_contained_shape().cloned(),
        );
        // Java's seed room is a local `completeShape` output that never reaches
        // `addIncompleteExpansionRoom`, so it is not on the engine's list; the port has to put it
        // in the arena to have a `RoomRef` for it, and the dump below filters it back out.
        seed_incomplete.push(id);
        (RoomRef::Incomplete(id), description)
    };

    // See `P6T3.java`: the any-angle `calculateNewIncompleteRooms` does not terminate when the
    // room's shape has more border lines than its `toSimplex()` does (quirk #162), so both
    // drivers skip those calls — in **mode 5 only**, because neither angle-restricted sorter
    // walks a simplex.
    //
    // **The port no longer needs the skip.** Plan 9 Task 8 fixed #162: `SortedRoomNeighbours`
    // derives its simplex once, in the constructor, so every `touchingSideNoOfRoom` indexes the
    // shape the walk walks and the walk's exit is reachable by construction. The skip stays
    // because the **jar** is not fixed and this driver is a differential: dropping it here would
    // make the two sides disagree on a call the JVM cannot survive at all.
    //
    // `P9T8_NO_SKIP=1` runs the calls the skip would have refused. It is the port-side
    // full-coverage measurement #162's acceptance asks for — the number of calls mode 5 executes
    // must rise to the number it enumerates — and it is deliberately **not** the default, because
    // the run it produces has no Java half to be compared against.
    let no_skip = std::env::var_os("P9T8_NO_SKIP").is_some();
    let from_shape = rooms
        .room_shape(room)
        .expect("a live room with a shape")
        .clone();
    if mode == 5
        && !no_skip
        && from_shape.border_line_count() != from_shape.to_simplex().border_line_count()
    {
        println!(
            "call i={index} kind={description} net={net_number} \
             skipped=simplexSideCountDiffers borderLines={} simplexLines={}",
            from_shape.border_line_count(),
            from_shape.to_simplex().border_line_count()
        );
        rooms.incomplete_rooms.clear();
        seed_incomplete.clear();
        return;
    }

    println!(
        "call i={index} kind={description} net={net_number} shape={} roomLayer={}",
        opt_shp(rooms.room_shape(room)),
        rooms.room_layer(board, room).expect("a live room")
    );

    let result = SortedRoomNeighbours::complete(room, net_number, board, rooms, tree_id)
        .expect("an incomplete or obstacle room completes");
    println!(
        "  result={} shape={} corners={}",
        desc(result, rooms),
        opt_shp(rooms.room_shape(result)),
        corners(rooms.room_shape(result))
    );
    println!(
        "  fromRoom={} shape={}",
        desc(room, rooms),
        opt_shp(rooms.room_shape(room))
    );
    dump_doors(result, rooms);
    dump_target_doors(result, rooms);
    let engine_rooms: Vec<IncompleteRoomId> = rooms
        .incomplete_rooms
        .iter()
        .map(|(index, _)| IncompleteRoomId(index))
        .filter(|id| !seed_incomplete.contains(id))
        .collect();
    println!("  incompleteRooms n={}", engine_rooms.len());
    for (i, id) in engine_rooms.iter().enumerate() {
        let current = rooms.incomplete_room(*id).expect("a live room");
        println!(
            "    [{i}] layer={} shape={} corners={} contained={} doors={}",
            current.get_layer(),
            opt_shp(current.get_shape()),
            corners(current.get_shape()),
            opt_shp(current.get_contained_shape()),
            current.get_doors().len()
        );
    }
    // The engine's incomplete-room list is reset between calls — see `P6T3.java`.
    rooms.incomplete_rooms.clear();
    seed_incomplete.clear();
}

#[allow(clippy::too_many_arguments)]
fn run_one(
    board: &mut Board,
    rooms: &mut ExpansionRoomStore,
    tree_id: TreeId,
    rng: &mut Rng,
    room_id_counter: &mut i32,
    index: i32,
    mode: i32,
) {
    let kind = rng.rnd(4);
    let net_number = 1 + rng.rnd(3);
    let layer = rng.rnd(2) as usize;
    let contained_box = rng.box_(RANGE, 10, 300);
    let item_id = if rng.rnd(4) == 0 {
        2 + rng.rnd(4)
    } else {
        4 + rng.rnd(2)
    };
    *room_id_counter += 1;
    let room_id_no = *room_id_counter;

    let (room, description) = if kind == 3 {
        let item_id = ItemId(item_id as u32);
        let shape_count = board.item_tree_shape_count(item_id, tree_id);
        if shape_count == 0 {
            println!(
                "call i={index} kind=obstacle item={} skipped=noShapes",
                item_id.0
            );
            return;
        }
        let index_in_item = rng.rnd(shape_count as i32) as usize;
        let obstacle = {
            let (b, r) = (&mut *board, &mut *rooms);
            item_info::get_expansion_room(b, item_id, index_in_item, tree_id, |b, i, idx, t| {
                r.new_obstacle_room(b, i, idx, t)
            })
        }
        .expect("the index came from treeShapeCount");
        (
            RoomRef::Obstacle(obstacle),
            format!("obstacle item={} indexInItem={index_in_item}", item_id.0),
        )
    } else {
        // The seed room the engine actually hands to `SortedRoomNeighbours`: the output of
        // `completeShape` over a whole-plane room around a small contained box
        // (`AutorouteEngine.java:449-450`).
        let seed =
            IncompleteFreeSpaceExpansionRoom::new(None, layer, Some(TileShape::Box(contained_box)));
        let completed = {
            let ctx = board.ctx();
            tree_of(board, tree_id).complete_shape(
                &seed,
                net_number,
                None,
                None,
                &board.items,
                &*rooms,
                &ctx,
            )
        };
        if completed.is_empty() {
            println!(
                "call i={index} kind=freeSpace layer={layer} contained={} skipped=noCompletedShape",
                shp(&TileShape::Box(contained_box))
            );
            return;
        }
        let pick = rng.rnd(completed.len() as i32) as usize;
        let chosen = &completed[pick];
        let description = format!(
            "freeSpace layer={layer} contained={} candidates={} pick={pick} chosenContained={}",
            shp(&TileShape::Box(contained_box)),
            completed.len(),
            opt_shp(chosen.get_contained_shape())
        );
        let id = rooms.new_incomplete_room(
            chosen.get_shape().cloned(),
            chosen.get_layer(),
            chosen.get_contained_shape().cloned(),
        );
        (RoomRef::Incomplete(id), description)
    };

    println!(
        "call i={index} kind={description} net={net_number} roomIdNo={room_id_no} shape={} \
         roomLayer={}",
        opt_shp(rooms.room_shape(room)),
        rooms.room_layer(board, room).expect("a live room")
    );

    if mode == 6 {
        let Some(result) = Sorted45DegreeRoomNeighbours::calculate_neighbours(
            room, net_number, board, rooms, tree_id, room_id_no,
        ) else {
            println!("  result=null");
            return;
        };
        dump_45_neighbours(&result, rooms);
        println!("  completedRoom={}", desc(result.completed_room, rooms));
        dump_doors(result.completed_room, rooms);
        dump_target_doors(result.completed_room, rooms);
        return;
    }
    if mode == 7 {
        let Some(result) = SortedOrthogonalRoomNeighbours::calculate_neighbours(
            room, net_number, board, rooms, tree_id, room_id_no,
        ) else {
            println!("  result=null");
            return;
        };
        dump_orthogonal_neighbours(&result, rooms);
        println!("  completedRoom={}", desc(result.completed_room, rooms));
        dump_doors(result.completed_room, rooms);
        dump_target_doors(result.completed_room, rooms);
        return;
    }

    let Some(result) = SortedRoomNeighbours::calculate_neighbours(
        room, net_number, board, rooms, tree_id, room_id_no,
    ) else {
        println!("  result=null");
        return;
    };

    println!("  neighbours n={}", result.sorted_neighbours.len());
    for (i, neighbour) in result.sorted_neighbours.iter().enumerate() {
        println!("    [{i}] {}", describe_neighbour(neighbour, rooms));
    }
    println!("  ownNet n={}", result.own_net_objects.len());
    for (i, entry) in result.own_net_objects.iter().enumerate() {
        println!(
            "    [{i}] obj={} idx={}",
            describe_object(entry.object, rooms),
            entry.shape_index
        );
    }
    println!("  completedRoom={}", desc(result.completed_room, rooms));
    dump_doors(result.completed_room, rooms);
}

/// `dumpDoors` — every door of one room, in insertion order.
fn dump_doors(room: RoomRef, rooms: &ExpansionRoomStore) {
    let doors = rooms.room_doors(room).to_vec();
    println!("  doors n={}", doors.len());
    for (i, door_id) in doors.iter().enumerate() {
        let door = rooms.door(*door_id).expect("a live door");
        let (first, second, dimension) = (door.first_room, door.second_room, door.dimension);
        let shape = rooms.door_shape(*door_id);
        println!(
            "    [{i}] first={} second={} dim={dimension} shape={} corners={}",
            desc(first, rooms),
            desc(second, rooms),
            opt_shp(shape.as_ref()),
            corners(shape.as_ref())
        );
    }
}

/// `dumpTargetDoors` — the target doors the two angle-restricted sorters build inside their
/// neighbour loop, and the base class builds at the end of `calculate`.
fn dump_target_doors(room: RoomRef, rooms: &ExpansionRoomStore) {
    let target_doors = rooms.room_target_doors(room).to_vec();
    println!("  targetDoors n={}", target_doors.len());
    for (i, door_id) in target_doors.iter().enumerate() {
        let door = rooms.target_door(*door_id).expect("a live target door");
        println!(
            "    [{i}] item={} entry={} dim={} shape={}",
            door.item.0,
            door.tree_entry_no,
            door.get_dimension(),
            shp(door.get_shape())
        );
    }
}

/// `flags` — Java's `boolean[]` rendered as `[true,false,...]`.
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

// -------------------------------------------------------------------------------------------
// mode 1 — the hazard-F comparator probe
// -------------------------------------------------------------------------------------------

fn comparator_probe(rng: &mut Rng, case_count: i32, stress: bool) {
    let rooms = ExpansionRoomStore::new();
    for c in 0..case_count {
        let room_box = rng.box_(2000, 400, 2000);
        let room_shape = TileShape::Box(room_box);
        let count = 2 + rng.rnd(4);
        // In stress mode every neighbour of a probe sits on the **same** side of the room — see
        // `P6T3.java` for why that is where the comparator stops being a total order.
        let fixed_side = rng.rnd(4);
        println!(
            "probe c={c} room={} n={count} stress={stress}",
            shp(&room_shape)
        );
        let mut set = std::collections::BTreeSet::new();
        for i in 0..count {
            let side = if stress { fixed_side } else { rng.rnd(4) };
            let span = 20 + rng.rnd(400);
            let offset = rng.rnd((room_box.ur.x - room_box.ll.x - span).max(1));
            let offset_y = rng.rnd((room_box.ur.y - room_box.ll.y - span).max(1));
            let neighbour_box = match side {
                0 => IntBox::from_coords(
                    room_box.ll.x + offset,
                    room_box.ll.y - 300,
                    room_box.ll.x + offset + span,
                    room_box.ll.y,
                ),
                1 => IntBox::from_coords(
                    room_box.ur.x,
                    room_box.ll.y + offset_y,
                    room_box.ur.x + 300,
                    room_box.ll.y + offset_y + span,
                ),
                2 => IntBox::from_coords(
                    room_box.ll.x + offset,
                    room_box.ur.y,
                    room_box.ll.x + offset + span,
                    room_box.ur.y + 300,
                ),
                _ => IntBox::from_coords(
                    room_box.ll.x - 300,
                    room_box.ll.y + offset_y,
                    room_box.ll.x,
                    room_box.ll.y + offset_y + span,
                ),
            };
            let neighbour_shape = TileShape::Box(neighbour_box);
            let intersection = room_shape.intersection(&neighbour_shape);
            let touching_sides = room_shape.touching_sides(&neighbour_shape);
            let (tsr, tsn) = match touching_sides {
                Some([a, b]) => (a as i32, b as i32),
                None => (0, 0),
            };
            let rtc = if stress {
                rng.rnd(2) == 0
            } else {
                rng.rnd(4) == 0
            };
            let ntc = if stress {
                rng.rnd(2) == 0
            } else {
                rng.rnd(3) == 0
            };
            let object_id = if stress {
                1 + rng.rnd(4)
            } else {
                1 + rng.rnd(6)
            };
            let neighbour = SortedRoomNeighbour::new(
                TreeObject::Room(RoomId(object_id as u32)),
                object_id,
                neighbour_shape,
                intersection,
                tsr,
                tsn,
                rtc,
                ntc,
                room_shape.clone(),
            );
            let description = describe_neighbour(&neighbour, &rooms);
            let added = set.insert(neighbour);
            println!(
                "    add[{i}] added={added} size={} {description}",
                set.len()
            );
        }
        println!("  survivors n={}", set.len());
        for (j, neighbour) in set.iter().enumerate() {
            println!("    [{j}] {}", describe_neighbour(neighbour, &rooms));
        }
    }
}

/// The narrowest hazard-F probe — see `P6T3.java`'s `cornerTouchProbe` for what it isolates.
fn corner_touch_probe(rng: &mut Rng, case_count: i32) {
    let rooms = ExpansionRoomStore::new();
    for c in 0..case_count {
        let room_box = rng.box_(2000, 400, 2000);
        let room_shape = TileShape::Box(room_box);
        let count = 3 + rng.rnd(3);
        println!("corner c={c} room={} n={count}", shp(&room_shape));
        let mut set = std::collections::BTreeSet::new();
        for i in 0..count {
            let neighbour_box = rng.box_(3000, 100, 1500);
            let neighbour_shape = TileShape::Box(neighbour_box);
            let intersection = room_shape.intersection(&neighbour_shape);
            let tsr = rng.rnd(4);
            let tsn = rng.rnd(4);
            let ntc = rng.rnd(2) == 0;
            let object_id = 1 + rng.rnd(5);
            let neighbour = SortedRoomNeighbour::new(
                TreeObject::Room(RoomId(object_id as u32)),
                object_id,
                neighbour_shape,
                intersection,
                tsr,
                tsn,
                true,
                ntc,
                room_shape.clone(),
            );
            let description = describe_neighbour(&neighbour, &rooms);
            let added = set.insert(neighbour);
            println!(
                "    add[{i}] added={added} size={} {description}",
                set.len()
            );
        }
        println!("  survivors n={}", set.len());
        for (j, neighbour) in set.iter().enumerate() {
            println!("    [{j}] {}", describe_neighbour(neighbour, &rooms));
        }
    }
}

// -------------------------------------------------------------------------------------------
// Formatting, byte-for-byte with `P6T3.java`
// -------------------------------------------------------------------------------------------

fn tree_of(board: &Board, tree_id: TreeId) -> &ShapeSearchTree {
    board
        .trees
        .trees()
        .find(|tree| tree.id() == tree_id)
        .expect("the autoroute tree")
}

fn describe_neighbour(n: &SortedRoomNeighbour, rooms: &ExpansionRoomStore) -> String {
    format!(
        "tsr={} tsn={} rtc={} ntc={} obj={} first={} last={} nshape={} nshapeCorners={} \
         isect={} isectCorners={}",
        n.touching_side_no_of_room,
        n.touching_side_no_of_neighbour_room,
        n.room_touch_is_corner,
        n.neighbour_room_touch_is_corner,
        describe_object(n.search_tree_object, rooms),
        pt(n.first_corner()),
        pt(n.last_corner()),
        shp(&n.neighbour_shape),
        corners(Some(&n.neighbour_shape)),
        shp(&n.intersection),
        corners(Some(&n.intersection))
    )
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

fn pt(p: &Point) -> String {
    let f = p.to_float();
    format!(
        "({},{})",
        format_double(f.x),
        format_double(f.y)
    )
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
        out.push_str(&format_double(corner.x));
        out.push(',');
        out.push_str(&format_double(corner.y));
    }
    out.push(')');
    out
}

fn b(x: &IntBox) -> String {
    format!("[{},{}..{},{}]", x.ll.x, x.ll.y, x.ur.x, x.ur.y)
}

fn opt_shp(s: Option<&TileShape>) -> String {
    s.map_or("null".to_string(), shp)
}

fn shp(s: &TileShape) -> String {
    match s {
        TileShape::Box(x) => format!("Box{}", b(x)),
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

fn angle_name(angle: AngleRestriction) -> &'static str {
    match angle {
        AngleRestriction::None => "NONE",
        AngleRestriction::FortyFiveDegree => "FORTYFIVE_DEGREE",
        AngleRestriction::NinetyDegree => "NINETY_DEGREE",
    }
}

fn class_name(item: &Item) -> &'static str {
    match item {
        Item::Trace(_) => "PolylineTrace",
        Item::Via(_) => "Via",
        Item::Pin(_) => "Pin",
        Item::ObstacleArea(_) => "ObstacleArea",
        Item::ConductionArea(_) => "ConductionArea",
        Item::ViaObstacleArea(_) => "ViaObstacleArea",
        Item::ComponentObstacleArea(_) => "ComponentObstacleArea",
        Item::ComponentOutline(_) => "ComponentOutline",
        Item::BoardOutline(_) => "BoardOutline",
    }
}
