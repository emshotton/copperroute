//! Tests for `crates/fr-router/src/autoroute/expansion/sorted_neighbours.rs` — the port of
//! `autoroute.expansion.SortedRoomNeighbours` (Plan 6 Task 4).
//!
//! The fixed scripts at the foot of the file are **not** replays of a generator: the board, the
//! thirty grid obstacles, the three seed rooms and every call's inputs are literals read off
//! `scripts/differential/java/P6T3.java`'s own stdout (`run.sh p6t3 4 42 30 1000`), and the
//! expected neighbour and door lines are that stdout verbatim. So each one reproduces the Java
//! output from scratch.

use std::collections::BTreeMap;

use fr_board::ids::TreeObject;
use fr_board::prelude::*;
use fr_geometry::{Area, IntBox, IntOctagon, IntVector, Point, Polyline, Shape, TileShape};
use fr_router::JavaTreeSet;
use fr_router::autoroute::expansion::sorted_neighbours::{
    SortedRoomNeighbour, SortedRoomNeighbours,
};
use fr_router::autoroute::expansion::{
    ExpansionRoomStore, IncompleteFreeSpaceExpansionRoom, RoomRef,
};
use fr_router::autoroute::item_info;
use fr_router::autoroute::tree_ext::AutorouteSearchTreeExt;

// =================================================================================================
// The factory dispatch — moved to `tests/java_ports.rs` (Task 18)
// =================================================================================================
//
// `SortedRoomNeighboursFactoryTest`'s three methods (`:15-34`) live in
// `crates/fr-router/tests/java_ports.rs`, which plan-6 ruling 12 makes the one named home of every
// ported Java suite. `select_calculation_mode` is still exercised from here indirectly, through
// every `SortedRoomNeighbours::calculate` call below.

// =================================================================================================
// Hazard F and hazard G — the comparator that was not a total order (quirks #160, #161)
//
// fixed: T8 (#160, #161). `SortedRoomNeighbour::compare_to` is now a lexicographic comparison of
// Java's own keys in Java's own order, with three defects removed: the last-corner refinement runs
// whenever the first-corner distances tie (Java needed the two first *corners* to be the same
// point), `c_dist_tolerance` no longer selects which key answers, and the final tie-break compares
// the object **kind** before the id. Past Java's last key the remaining value fields are compared,
// so `Equal` means "equal as a value" and a set can no longer drop a door the room really has.
//
// The three tests below used to pin the drops. They now pin their absence, and each one keeps the
// jar's verbatim transcript beside the port's answer.
// =================================================================================================

/// One [`SortedRoomNeighbour`] with `roomTouchIsCorner`, which is what makes both corners equal to
/// the room's own corner and every distance delta exactly 0 — see the module docs.
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
    // `run.sh p6t3 3 42 0 2000`, `corner c=520`. Five neighbours of the same room, all with
    // `roomTouchIsCorner`, all touching side 1, so the first two comparison keys tie for every
    // pair and only the `Direction.compareFrom` branch (both `ntc`) and the id difference are
    // left. The jar, verbatim:
    //
    //   corner c=520 room=Box[1730,-384..3188,78] n=5
    //       add[0] added=true size=1 tsr=1 tsn=3 rtc=true ntc=true  obj=cfsr2
    //       add[1] added=true size=2 tsr=1 tsn=3 rtc=true ntc=false obj=cfsr1
    //       add[2] added=true size=3 tsr=1 tsn=1 rtc=true ntc=true  obj=cfsr4
    //       add[3] added=true size=4 tsr=1 tsn=2 rtc=true ntc=true  obj=cfsr5
    //       add[4] added=true size=5 tsr=1 tsn=0 rtc=true ntc=false obj=cfsr5
    //     survivors n=5
    //       [0] tsn=3 ntc=false obj=cfsr1
    //       [1] tsn=2 ntc=true  obj=cfsr5
    //       [2] tsn=3 ntc=true  obj=cfsr2
    //       [3] tsn=1 ntc=true  obj=cfsr4
    //       [4] tsn=0 ntc=false obj=cfsr5
    //
    // **The jar keeps all five and so does the port** — that half never moved. What moved is the
    // rest of the row: the jar's five survivors are not in any consistent order (an in-order walk
    // of a red-black tree built by a non-transitive comparator is not a sorted sequence), and a
    // `BTreeSet` over the same `Ord` used to keep only **four**, because its binary search inside
    // one B-tree node hits an `Equal` Java's root-to-leaf walk never reaches. That difference is
    // what made the container a `JavaTreeSet`.
    //
    // Post-fix the comparator is total, so both containers keep five and agree on the order, and
    // the order is genuinely sorted. **KNOWN DIVERGENCE from the jar, authorized by #160**: the
    // survivor sequence is the port's, and the jar's is kept above so a reader sees both.
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

    let mut set = JavaTreeSet::new();
    for (i, neighbour) in built.iter().enumerate() {
        assert!(
            set.add(neighbour.clone()),
            "the jar's add[{i}] answered true and so must the port's"
        );
        assert_eq!(set.len(), i + 1);
    }
    assert_eq!(set.len(), 5);

    // A `BTreeSet` over the same `Ord` now keeps all five too, which is the point of the fix and
    // is what lets Task 24 collect the `JavaTreeSet`. (`mutable_key_type` fires because
    // `SortedRoomNeighbour` memoizes its two corners in `OnceCell`s exactly as Java's
    // `precalculatedFirstCorner`/`precalculatedLastCorner` do; neither cell is read by the
    // comparator's keys, only filled by them.)
    #[allow(clippy::mutable_key_type)]
    let mut btree = std::collections::BTreeSet::new();
    for neighbour in &built {
        btree.insert(neighbour.clone());
    }
    assert_eq!(
        btree.len(),
        5,
        "a BTreeSet dropped one of the five before the fix; it keeps all five now"
    );
    let describe = |n: &SortedRoomNeighbour| {
        (
            n.touching_side_no_of_neighbour_room,
            n.neighbour_room_touch_is_corner,
            n.object_id,
        )
    };
    assert_eq!(
        set.iter().map(describe).collect::<Vec<_>>(),
        btree.iter().map(describe).collect::<Vec<_>>(),
        "the two containers agree once the comparator is a total order"
    );

    // And the two elements the jar could not order — `add[3]` and `add[4]`, whose `Signum.asInt`
    // deltas were both 0 and whose object ids were both 5 — are ordered now, consistently with
    // where they each sit relative to `add[0]`. Before the fix:
    //     built[3] == built[4],  built[0] > built[3],  built[0] < built[4]
    // which no equivalence can permit.
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
    // `run.sh p6t3 3 42 0 2000`, `corner c=521`: four neighbours, and `add[3]` is the one the
    // jar's `TreeSet` silently drops, because it compares `Equal` to `add[1]` — same touching
    // side, same (corner) first and last corners, neither is a `neighbourRoomTouchIsCorner` pair,
    // and the two objects carry the **same id**. The jar, verbatim:
    //
    //   corner c=521 room=Box[656,685..1281,2671] n=4
    //       add[0] added=true  size=1 tsr=1 tsn=1 rtc=true ntc=true  obj=cfsr4
    //       add[1] added=true  size=2 tsr=3 tsn=3 rtc=true ntc=false obj=cfsr2
    //       add[2] added=true  size=3 tsr=0 tsn=2 rtc=true ntc=true  obj=cfsr3
    //       add[3] added=false size=3 tsr=3 tsn=1 rtc=true ntc=false obj=cfsr2
    //     survivors n=3
    //
    // **KNOWN DIVERGENCE from the jar, authorized by #160**: `add[3]` now answers `true` and the
    // set holds **four**. The two are separated at `touchingSideNoOfNeighbourRoom` — 3 against 1,
    // two different sides of two different neighbour boxes, which is to say two different doors.
    let room = TileShape::Box(IntBox::from_coords(656, 685, 1281, 2671));
    let mut set = JavaTreeSet::new();
    assert!(set.add(corner_neighbour(
        &room,
        IntBox::from_coords(-2928, 954, -2418, 1931),
        1,
        1,
        true,
        4
    )));
    assert!(set.add(corner_neighbour(
        &room,
        IntBox::from_coords(756, 1893, 2108, 2443),
        3,
        3,
        false,
        2
    )));
    assert!(set.add(corner_neighbour(
        &room,
        IntBox::from_coords(1639, 1220, 1818, 1470),
        0,
        2,
        true,
        3
    )));
    assert!(
        set.add(corner_neighbour(
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
    // quirk #161. `:759` is `this.searchTreeObject.getId() - other.searchTreeObject.getId()`, and
    // the two objects can be a board item and an expansion room — a `BasicBoard.ItemIdGenerator`
    // number against an `AutorouteEngine.expansionRoomInstanceCount` number. Both start at 1, so
    // "item 3" and "room 3" tied and the `TreeSet` dropped one of them.
    //
    // fixed: T8 (#161) — the object **kind** is compared before the id, so the two id spaces never
    // meet. `Item` before `Room`; the direction is arbitrary, the consistency is not.
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
    let mut set = JavaTreeSet::new();
    assert!(set.add(as_item));
    assert!(
        set.add(as_room),
        "the room is no longer dropped for colliding with the item"
    );
    assert_eq!(set.len(), 2);
    // A different room id still separates them, in the id's own direction, inside the room space.
    let as_room_4 = make(TreeObject::Room(RoomId(0)), 4);
    assert!(set.add(as_room_4));
    assert_eq!(set.len(), 3);
}

// =================================================================================================
// The total-order property, over `p6t3` mode 3's own 2 000 cases
//
// BL7: `p6t2` retires with this task and `p6t3` mode 3's Java half is not needed to keep this
// assertion alive — the generator is reproduced here from `scripts/differential/java/P6T3.java`'s
// own source (the xorshift64 stream at `:132-141`, `randomBox` at `:147-153` and
// `cornerTouchProbe` at `:1084-1136`), so the 2 000 cases this asserts over are the same 2 000
// cases the jar was measured on.
// =================================================================================================

/// `P6T3.next()` (`:132-137`) — xorshift64, seeded exactly as `:204` seeds it.
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

    /// `P6T3.rnd(int)` (`:139-141`): `Long.remainderUnsigned(next(), bound)`.
    fn rnd(&mut self, bound: u64) -> i32 {
        (self.next() % bound) as i32
    }

    /// `P6T3.randCoord(int)` (`:143-145`).
    fn coord(&mut self, range: i32) -> i32 {
        self.rnd(2 * range as u64 + 1) - range
    }

    /// `P6T3.randomBox(int, int, int)` (`:147-153`).
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
    // `run.sh p6t3 3 42 0 2000` — 2 000 rooms, each with 3 to 5 neighbours that all carry
    // `roomTouchIsCorner`, so both corners collapse onto the room's own corner and every distance
    // delta is exactly 0. What is left to decide the order is `Direction.compareFrom` (reached
    // only when *both* neighbours are `neighbourRoomTouchIsCorner`) and the id — the narrowest
    // hazard-F probe there is, and the one the **481 drops in 2 000 cases** headline was measured
    // on.
    //
    // Three assertions, and the first is the headline:
    //
    //   1. **0 drops.** A `JavaTreeSet` and a `BTreeSet` both hold exactly as many elements as
    //      there are distinct *values*, so no door a room really has is lost. Before the fix this
    //      run drops 481 of 6 991 neighbours across the 2 000 cases.
    //   2. **Antisymmetry**, over every pair of every case.
    //   3. **Transitivity**, over every ordered triple of every case.
    let mut rng = Xorshift64::new(42);
    let mut drops_java = 0usize;
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

        // "Distinct as a value" is the full constructor argument list: everything
        // `cornerTouchProbe` varies, plus the two flags it fixes.
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

        let mut java = JavaTreeSet::new();
        for neighbour in &built {
            java.add(neighbour.clone());
        }
        drops_java += distinct.len() - java.len();

        #[allow(clippy::mutable_key_type)]
        let mut btree = std::collections::BTreeSet::new();
        for neighbour in &built {
            btree.insert(neighbour.clone());
        }
        drops_btree += distinct.len() - btree.len();

        // Antisymmetry over every pair, transitivity over every ordered triple.
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
    // Provenance: the stream really is the jar's. `corner c=520` printed
    // `room=Box[1730,-384..3188,78]` and `n=5`, and this reproduction draws the same box at the
    // same index — so the 2 000 cases asserted over are the 2 000 the jar was measured on, and
    // `five_neighbours_at_one_corner_are_all_kept_and_sort`'s five literals are case 520's.
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
        (drops_java, drops_btree),
        (0, 0),
        "the pre-fix tree drops 481 of the 8 000 in a JavaTreeSet and 482 in a BTreeSet — \
         measured, by stashing the comparator and running this same test; the post-fix number \
         is 0 in both, which is what makes the JavaTreeSet replaceable"
    );
}

// =================================================================================================
// The `p6t3` mode-4 board, and three fixed scripts read off its Java output
// =================================================================================================

/// The `P2T10` board of `P6T3.java` (two layers, a two-pin component, two traces, an empty
/// outline) plus the thirty grid obstacles seed 42 draws, read off `p6t3 4 42 30 1000`'s own
/// `item id=` lines.
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

    // Items 6..35 of the Java dump: `(llx, lly, urx, ury, layer)`.
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

/// The three `CompleteFreeSpaceExpansionRoom`s `P6T3.insertSeedRooms` puts in the tree, from the
/// same Java output (`seedRoom 1..3`).
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

fn corners_of(shape: &TileShape) -> Vec<(f64, f64)> {
    shape
        .corner_approx_arr()
        .iter()
        .map(|c| (c.x, c.y))
        .collect()
}

#[test]
fn one_neighbour_yields_one_door() {
    // `run.sh p6t3 4 42 30 1000`, `call i=646`, verbatim:
    //
    //   call i=646 kind=freeSpace layer=1 contained=Box[-1102,7787..-804,7980] candidates=2 pick=1
    //        net=3 roomIdNo=650
    //     neighbours n=1
    //       [0] tsr=0 tsn=2 rtc=false ntc=false obj=cfsr2
    //           first=(-1858.0,7935.0) last=(-1018.0,7935.0) nshape=Box[-1858,6349..-1018,7935]
    //     ownNet n=0
    //     completedRoom=cfsr650
    //     doors n=1
    //       [0] first=cfsr650 second=cfsr2 dim=1
    //           corners=(-1858.0,7935.0;-1018.0,7935.0;-1018.0,7935.0;-1858.0,7935.0)
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
    // `obj=cfsr2` — the second seed room, which is a `TreeObject::Room` in the tree. This is the
    // arm `fr-board`'s `tree_shape_of`/`ignore_object` used to panic on.
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
    // `run.sh p6t3 4 42 30 1000`, `call i=14` — five neighbours, of which `[3]` is a
    // **dimension-0** touch (`SortedRoomNeighbours.java:286-326`), the branch a non-grid board
    // never reaches. Java, verbatim:
    //
    //   call i=14 kind=freeSpace layer=1 contained=Box[4177,3527..4399,3816] candidates=1 pick=0
    //        net=3 roomIdNo=18
    //     neighbours n=5
    //       [0] tsr=0 tsn=4 rtc=false ntc=false obj=item21
    //       [1] tsr=1 tsn=6 rtc=false ntc=false obj=item20
    //       [2] tsr=2 tsn=0 rtc=false ntc=false obj=item22
    //       [3] tsr=3 tsn=0 rtc=true  ntc=true  obj=item33 first=(2041.0,7400.0) last=(2041.0,7400.0)
    //       [4] tsr=3 tsn=1 rtc=false ntc=false obj=item7
    //     doors n=0
    //
    // Note the door count: the brief expected a corner touch to yield a one-dimensional door, but
    // Java creates a door only in the `dimension == 1` arm (`:279-285`). A corner touch is
    // recorded as a neighbour and nothing else.
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
    // The corner touch: both corners collapse to the room's own corner.
    let corner = neighbours[3];
    assert_eq!(corner.first_corner(), corner.last_corner());
    assert_eq!(corner.first_corner().to_float().x, 2041.0);
    assert_eq!(corner.first_corner().to_float().y, 7400.0);
    assert!(rooms.room_doors(result.completed_room).is_empty());
}

#[test]
fn a_two_dimensional_overlap_yields_an_overlap_door_between_obstacle_rooms() {
    // `run.sh p6t3 4 42 30 1000`, `call i=0`, verbatim:
    //
    //   call i=0 kind=obstacle item=4 indexInItem=1 net=2 roomIdNo=4
    //        shape=Oct[-130,-130,130,530,-584,184,-184,584] roomLayer=0
    //     neighbours n=0
    //     ownNet n=0
    //     completedRoom=obs4/1
    //     doors n=2
    //       [0] first=obs4/1 second=obs4/0 dim=2
    //       [1] first=obs4/1 second=obs4/2 dim=2
    //
    // The middle tile shape of trace 4 overlaps its two neighbours two-dimensionally, so
    // `createOverlapDoor` (`ObstacleExpansionRoom.java:77-100`) builds one door per consecutive
    // segment — and no *sorted* neighbour is recorded at all.
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
    // quirk #162, **fixed: T8**. `SortedRoomNeighbours.java:512` built `roomSimplex =
    // this.fromRoom.getShape().toSimplex()` and then indexed it with `touchingSideNoOfRoom`, a
    // side number of the **un-simplified** shape. `Simplex.getInstance` drops redundant lines, so
    // the two did not have the same number of sides — and when `firstTouchingSideNo` named a line
    // the simplex does not have, the `for (;;)` at `:562` never reached it and allocated an
    // incomplete room per turn until the heap was gone.
    //
    // The octagon below is `run.sh p6t3 4 42 30 1000`'s `call i=2` seed room, and mode 5 reports
    // it as `skipped=simplexSideCountDiffers borderLines=8 simplexLines=5`. This test used to be
    // called `the_room_shapes_that_make_calculate_new_incomplete_rooms_loop_for_ever` and asserted
    // the *trigger* without running the loop, because running it was what the JVM could not
    // survive either. It now runs it.
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

    // **The fix, as an invariant.** The shape every `touchingSideNoOfRoom` is an index into is the
    // shape the loop walks, because there is only one of them now. Before the fix
    // `result.room_shape` was the 8-line octagon and the loop walked a 5-line simplex, so a
    // neighbour on side 5, 6 or 7 handed the loop a `firstTouchingSideNo` `prevNo` could never
    // reach: 4, 3, 2, 1, 0, 4, … for ever.
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

    // And the loop itself terminates. `calculate` is the caller Java has (`:117-130`), so this
    // drives `tryRemoveEdge` and `calculateNewIncompleteRooms` exactly as the engine does. The
    // work is done on a worker thread with a wall-clock join, because the pre-fix answer to this
    // call is not "wrong" but "never" — a bounded assertion is the only kind that can be written
    // about non-termination.
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

// =================================================================================================
// The own-net split and the target doors
// =================================================================================================

#[test]
fn a_non_obstacle_of_the_routed_net_is_deferred_to_the_own_net_list() {
    // `run.sh p6t3 4 42 30 1000`, `call i=3`: net 2, layer 0, contained `Box[1571,885..1716,1165]`
    // — four neighbours and, verbatim,
    //
    //     ownNet n=1
    //       [0] obj=item5 idx=1
    //
    // `SortedRoomNeighbours.java:222-227` puts an object that is *not* a trace obstacle for the
    // routed net aside rather than making it a neighbour, "to delay processing the target doors
    // until the room shape will not change anymore". Trace 5 carries net 2, so routing net 2
    // defers it and routing net 3 does not — and on net 3 it becomes a *fifth* neighbour instead.
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

// =================================================================================================
// `ExpansionRoomStore`'s two new engine-list operations
// =================================================================================================

#[test]
fn remove_all_doors_unlinks_both_sides_and_drops_incomplete_neighbours() {
    // `AutorouteEngine.removeAllDoors` (`:603-615`): every door is removed from the room on its
    // other side, and an incomplete room on that side is removed outright.
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

// =================================================================================================
// The `RoomLookup` `fr-board` gained for this task
// =================================================================================================

#[test]
fn a_room_bearing_tree_answers_queries_through_the_room_lookup() {
    // The obligation Task 2 recorded and this task discharges: `ShapeSearchTree`'s two private
    // helpers used to panic on a `TreeObject::Room`. With a `RoomLookup` they resolve, and the
    // room-free overload still panics — deliberately.
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
    // `shapeLayer` is the room's own layer, so a query on the other layer ignores it.
    assert!(
        tree.overlapping_tree_entries_with_rooms(&probe, Some(0), &[], &items, &rooms, &ctx)
            .is_empty()
    );
    // `isObstacle(int)` is the constant `true`, so no ignored net can hide a room.
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
