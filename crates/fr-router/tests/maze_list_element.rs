//! Plan 6 Task 8: the port of `src/test/java/app/freerouting/autoroute/maze/MazeListElementTest`
//! (plan-6 ruling 12), plus the two quirk tests ruling 4 asks for.
//!
//! **The two Java test methods themselves moved to `crates/fr-router/tests/java_ports.rs`** in
//! Task 18, which is ruling 12's single named home for the three ported suites; what stays here is
//! the tie-break, NaN and dropped-element family that suite does not cover.
//!
//! # What the Java test does, and what the port has to do instead
//!
//! `MazeListElementTest` builds a `TestDoor implements ExpandableObject` whose only live method is
//! `getId()`, and compares two `MazeListElement`s. The port cannot: `ExpandableObject.getId()` is
//! **not** a property of the reference the element holds — for an `ExpansionDoor` it is a hash of
//! the two rooms' ids (`ExpansionDoor.java:184-190`), for a `DrillPage` a hash of its shape and
//! its *mutable* `netNumber` (`DrillPage.java:189-193`, quirk #167). The port's
//! [`fr_router::autoroute::maze::MazeListElement`] therefore holds an
//! [`ExpandableRef`](fr_router::autoroute::expansion::ExpandableRef) (an arena index) and
//! `compare_to` takes a **resolver**, exactly where Java performs a virtual call. The resolver
//! here is the test's `TestDoor.getId()`: a map from index to id.

use std::cmp::Ordering;
use std::collections::HashMap;

use fr_geometry::{FloatLine, FloatPoint};
use fr_router::JavaTreeSet;
use fr_router::arena::DoorId;
use fr_router::autoroute::expansion::ExpandableRef;
use fr_router::autoroute::maze::{MazeAdjustment, MazeListElement};

// =================================================================================================
// `MazeListElementTest.TestDoor` (:37-78): an ExpandableObject whose only live method is getId()
// =================================================================================================

/// The `door.getId()` resolver `compare_to` consults, standing in for the Java test's `TestDoor`.
///
/// `TestDoor(id)` becomes `ExpandableRef::Door(DoorId(id))` plus a row here, so that a door's id
/// stays a *lookup at comparison time* rather than a field of the element — which is what quirk
/// #167's moving `DrillPage.getId` needs.
fn test_doors(ids: &[i32]) -> impl Fn(ExpandableRef) -> i32 + use<> {
    let map: HashMap<ExpandableRef, i32> = ids
        .iter()
        .map(|id| (ExpandableRef::Door(DoorId(*id as u32)), *id))
        .collect();
    move |door| *map.get(&door).expect("a TestDoor the test registered")
}

/// `new MazeListElement(new TestDoor(door), 0, null, 0, expansion, sorting, null, null, false,
/// null, false)` — the Java test's constructor call, with the two arguments it passes `null`
/// that the port makes non-optional (`shapeEntry`, `adjustment`) at their production values.
fn element(door: i32, section_no: i32, expansion: f64, sorting: f64) -> MazeListElement {
    MazeListElement {
        door: ExpandableRef::Door(DoorId(door as u32)),
        section_no_of_door: section_no,
        backtrack_door: None,
        section_no_of_backtrack_door: 0,
        expansion_value: expansion,
        sorting_value: sorting,
        next_room: None,
        shape_entry: FloatLine::new(FloatPoint::new(0.0, 0.0), FloatPoint::new(0.0, 0.0)),
        room_ripped: false,
        adjustment: MazeAdjustment::None,
        already_checked: false,
        ripup_cost: 0,
    }
}

/// One `MazeQueue::push` on a bare board, so the #170 refusal can be asserted where it lives.
///
/// The guard `push` applies before `super.add` is the fanout window, and `isFanout` is false
/// here, so what this measures is exactly the non-finite check and the insert.
fn push_for_test(element: MazeListElement) -> bool {
    use fr_board::prelude::*;
    use fr_geometry::{IntBox, TileShape};
    use fr_router::autoroute::expansion::RoomRef;
    use fr_router::autoroute::maze::{AutorouteControl, AutorouteEngine, MazeQueue};
    use fr_settings::RouterSettings;

    let layers = || LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)]);
    let clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
    let mut rules = BoardRules::new(layers(), clearance_matrix);
    rules.trace_angle_restriction = AngleRestriction::None;
    let class = rules.net_classes.append("default", &layers(), false);
    rules.nets.add("n1", 1, false, class);
    let mut board = Board::new(
        Vec::new(),
        0,
        IntBox::from_coords(-100_000, -100_000, 100_000, 100_000),
        rules,
        BoardLibrary::new(Padstacks::new(layers()), Packages::new()),
        Components::new(),
        Communication::default(),
    );
    let settings = RouterSettings::new();
    // The literal control block `tests/maze_queue.rs::fresh_control` builds, for the same reason
    // it does not go through `AutorouteControl::new`: this board has no via rule, and `:235`
    // dereferences one. The guard `push` applies never touches a via.
    let ctrl = AutorouteControl {
        trace_costs: settings.get_trace_costs(),
        bend_costs: vec![0.0, 0.0],
        with_neckdown: false,
        layer_active: vec![true, true],
        layer_count: 2,
        trace_half_width: vec![100, 100],
        compensated_trace_half_width: vec![100, 100],
        via_radii: vec![0.0, 0.0],
        add_via_costs: vec![vec![0, 0], vec![0, 0]],
        trace_clearance_class_index: 1,
        vias_allowed: true,
        attach_smd_allowed: false,
        min_normal_via_cost: 0.0,
        ripup_allowed: false,
        ripup_costs: 1000,
        ripup_pass_no: 1,
        is_fanout: false,
        fanout_start_pin_name: None,
        fanout_start_pin_center: None,
        fanout_start_pin_layer: -1,
        remove_unconnected_vias: true,
        via_rule: None,
        net_number: 1,
        via_clearance_class: 1,
        via_infos: Vec::new(),
        via_lower_bound: 0,
        via_upper_bound: 2,
        max_via_radius: 0.0,
        tidy_region_width: i32::MAX,
        pull_tight_accuracy: 500,
        max_shove_trace_recursion_depth: 20,
        max_shove_via_recursion_depth: 5,
        max_spring_over_recursion_depth: 5,
        min_cheap_via_cost: 0.0,
        fanout_max_escape_length: 3000.0,
        fanout_min_escape_length: 500.0,
        // `RouterSettings.getStartRipupCosts`'s default (RouterSettings.java:537-548).
        start_ripup_costs: 1,
    };
    let mut engine = AutorouteEngine::new(&mut board, 1, false);
    let room = engine.rooms.new_complete_room(
        Some(TileShape::Box(IntBox::from_coords(0, 0, 10, 10))),
        0,
        1,
    );
    let mut element = element;
    element.next_room = Some(RoomRef::Complete(room));
    let mut queue = MazeQueue::new();
    queue.push(element, &ctrl, &engine, &board)
}

// =================================================================================================
// The two Java test methods, ported one for one — moved to `tests/java_ports.rs` (Task 18)
// =================================================================================================
//
// `MazeListElementTest.compareToReturnsZeroForSameInstance` (:14-20) and
// `compareToSortsBySortingValue` (:22-35) live in `crates/fr-router/tests/java_ports.rs`, which
// plan-6 ruling 12 makes the one named home of every ported Java suite, so that
// `grep -rn MazeListElementTest crates/` gives one answer. Everything below is the quirk family
// this task added around them, which stays here.

// =================================================================================================
// The four tie-breaks, in Java's order (MazeListElement.java:80-113)
// =================================================================================================

/// `:81-86` then `:88-93` then `:95-102` then `:104-109` then `:112`.
#[test]
fn the_four_tie_breaks_are_taken_in_javas_order() {
    let ids = test_doors(&[1, 2]);

    // sortingValue first (:81-86)
    assert_eq!(
        element(1, 0, 9.0, 1.0).compare_to(&element(2, 5, 0.0, 2.0), &ids),
        Ordering::Less
    );
    // then expansionValue (:88-93)
    assert_eq!(
        element(1, 0, 5.0, 1.0).compare_to(&element(2, 5, 4.0, 1.0), &ids),
        Ordering::Greater
    );
    // then the door id (:95-102)
    assert_eq!(
        element(1, 7, 1.0, 1.0).compare_to(&element(2, 0, 1.0, 1.0), &ids),
        Ordering::Less
    );
    // then sectionNoOfDoor (:104-109)
    assert_eq!(
        element(1, 7, 1.0, 1.0).compare_to(&element(1, 0, 1.0, 1.0), &ids),
        Ordering::Greater
    );
    // and then Equal (:112)
    assert_eq!(
        element(1, 3, 1.0, 1.0).compare_to(&element(1, 3, 1.0, 1.0), &ids),
        Ordering::Equal
    );
}

// =================================================================================================
// The two quirks plan-6 ruling 4 names — both fixed at Plan 9 Task 8
// =================================================================================================

/// Quirk #170, **fixed: T8** — by refusing the element, not by ordering it.
///
/// `compareTo` compares two `double`s with raw `<`/`>`, so a `NaN` makes both tests false and the
/// comparison **falls through to the next key** instead of ordering. The relation then stops
/// being transitive, and a red-black tree built on it can find or not find the same element
/// depending on its shape. The counter-example, from
/// `docs/plan-9-prep/fixtures/task-8/expected-outcomes.md`:
///
/// ```text
/// A = { sorting: NaN, expansion: 1.0 }   A vs B: NaN<5 false, NaN>5 false -> 1.0 < 2.0 -> A < B
/// B = { sorting: 5.0, expansion: 2.0 }   B vs C: 5.0 > 3.0                            -> B > C
/// C = { sorting: 3.0, expansion: 9.0 }   A vs C: NaN<3 false, NaN>3 false -> 1.0 < 9.0 -> A < C
/// ```
///
/// **The fix is not in the comparator.** `total_cmp` would silently sort NaN last and keep the
/// bug alive; a non-finite cost is an upstream defect, not a thing to sort. `MazeQueue::push`
/// refuses it — Java's own `false`, the same answer its `:104`/`:120` fanout refusals give — so
/// the comparator never sees one. The fall-through below is therefore still transcribed and still
/// asserted: it is what a NaN *would* do, and this test's job is now to show that the queue does
/// not let one in.
#[test]
fn a_non_finite_sorting_value_is_refused_at_add() {
    let ids = test_doors(&[1, 2]);

    // The comparator is unchanged and still falls through, because it is not where the fix is.
    let nan = element(1, 0, 1.0, f64::NAN);
    let number = element(2, 0, 5.0, 1.0);
    assert_eq!(nan.compare_to(&number, &ids), Ordering::Less);
    assert_eq!(number.compare_to(&nan, &ids), Ordering::Greater);

    // And the three-element counter-example really is non-transitive, which is the reason the
    // element has to be refused rather than ordered.
    let a = element(1, 0, 1.0, f64::NAN);
    let b = element(2, 0, 2.0, 5.0);
    let c = element(1, 1, 9.0, 3.0);
    assert_eq!(a.compare_to(&b, &ids), Ordering::Less);
    assert_eq!(b.compare_to(&c, &ids), Ordering::Greater);
    assert_eq!(a.compare_to(&c, &ids), Ordering::Less);
    assert_eq!(
        c.compare_to(&b, &ids),
        Ordering::Less,
        "so A < B, C < B and A < C — and nothing says where A and C sit relative to each other \
         in a way B agrees with"
    );

    // The queue refuses every non-finite sorting value, at the port's single `add` site — which
    // stands for Java's six (`MazeSearchEngine.java:547`, `:964`, `:1079` and
    // `MazeExpansionEngine.java:101`, `:142`, `:373`), all of which go through the container's
    // overridden `add`.
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(
            !push_for_test(element(1, 0, 1.0, bad)),
            "a sorting value of {bad} is an upstream bug and is refused"
        );
    }
    assert!(
        push_for_test(element(1, 0, 1.0, 4.0)),
        "a finite one is accepted"
    );
}

/// Quirk #171, **fixed: T8**.
///
/// A full tie answered `0` (`:110-112`) and `TreeMap.put` then **kept the element already in the
/// tree** and answered `false` — the new one was silently dropped, values and all. Two elements
/// can tie on all four keys while differing in everything the four keys do not cover:
/// `backtrackDoor` (which is the whole path), `nextRoom`, `shapeEntry`, `roomRipped`,
/// `adjustment` and `ripupCost`. So two genuinely different routes arriving at the same door
/// section at the same cost were collapsed to one, and the survivor was whichever was inserted
/// first — not the cheaper, not the one with the lower ripup cost.
///
/// **The policy, written down** (the invariant asks for one): *both are kept*. `compare_to`
/// continues past Java's `:110-112` through the remaining eight fields in the struct's — that is,
/// Java's — declaration order, so `Equal` means the two elements are equal as values. The four
/// keys Java compares keep their meaning and their order; what follows only separates elements
/// Java could not tell apart.
#[test]
fn two_paths_at_the_same_cost_are_both_kept() {
    let ids = test_doors(&[1]);

    let first = element(1, 2, 3.0, 4.0);
    let mut second = element(1, 2, 3.0, 4.0);
    second.room_ripped = true;
    second.ripup_cost = 999;
    second.adjustment = MazeAdjustment::Left;
    second.backtrack_door = Some(ExpandableRef::Door(DoorId(1)));

    // The four keys tie, exactly as Java's do.
    assert_eq!(first.sorting_value, second.sorting_value);
    assert_eq!(first.expansion_value, second.expansion_value);
    assert_eq!(first.door, second.door);
    assert_eq!(first.section_no_of_door, second.section_no_of_door);
    // And the two are ordered anyway, antisymmetrically.
    assert_ne!(first.compare_to(&second, &ids), Ordering::Equal);
    assert_eq!(
        first.compare_to(&second, &ids).reverse(),
        second.compare_to(&first, &ids)
    );

    let mut queue: JavaTreeSet<MazeListElement> = JavaTreeSet::new();
    assert!(queue.add_by(first.clone(), |a, b| a.compare_to(b, &ids)));
    assert!(
        queue.add_by(second.clone(), |a, b| a.compare_to(b, &ids)),
        "the jar's TreeSet answers false here and drops the element whole, ripupCost and all"
    );
    assert_eq!(queue.len(), 2, "both paths are in the queue");
    let held: Vec<&MazeListElement> = queue.iter().collect();
    assert!(held.contains(&&first) && held.contains(&&second));

    // `Equal` now means equal as a value, which is what makes a set safe to hold these.
    assert_eq!(first.compare_to(&first, &ids), Ordering::Equal);
    assert!(
        !queue.add_by(first.clone(), |a, b| a.compare_to(b, &ids)),
        "a genuine duplicate is still a duplicate"
    );
    assert_eq!(queue.len(), 2);

    // The door id is a **hash** even over injective room ids: `ExpansionDoor::id(a, b)` is
    // `min * 31 + max`, so the doors between rooms `(1, 63)` and `(2, 32)` both answer 94 and two
    // *different* doors tie on key 3. That is why the door itself is compared past the id.
    assert_eq!(
        fr_router::autoroute::expansion::ExpansionDoor::id(1, 63),
        fr_router::autoroute::expansion::ExpansionDoor::id(2, 32),
        "1 * 31 + 63 == 2 * 31 + 32 == 94"
    );
    let ids2 = |door: ExpandableRef| {
        let _ = door;
        94
    };
    let door_a = element(7, 0, 1.0, 1.0);
    let mut door_b = element(8, 0, 1.0, 1.0);
    door_b.section_no_of_door = 0;
    assert_ne!(
        door_a.compare_to(&door_b, ids2),
        Ordering::Equal,
        "two different doors that collide on the hash are still two doors"
    );
}

// =================================================================================================
// The queue is a queue: pop-first removes, and the order survives removal
// =================================================================================================

/// `MazeSearchEngine.occupyNextElement` (`MazeSearchEngine.java:327-329`) pops through
/// `iterator().next()` + `it.remove()`, which is `TreeMap.deleteEntry` — not a `BinaryHeap` pop,
/// and not a `BTreeSet::pop_first`.
#[test]
fn pop_first_walks_the_queue_in_sorting_value_order() {
    let ids = test_doors(&[1, 2, 3, 4, 5]);
    let mut queue: JavaTreeSet<MazeListElement> = JavaTreeSet::new();
    for (door, sorting) in [(3, 30.0), (1, 10.0), (5, 50.0), (2, 20.0), (4, 40.0)] {
        assert!(queue.add_by(element(door, 0, 0.0, sorting), |a, b| a.compare_to(b, &ids)));
    }
    assert_eq!(queue.len(), 5);

    let mut popped = Vec::new();
    while let Some(e) = queue.poll_first() {
        popped.push(e.sorting_value);
    }
    assert_eq!(popped, vec![10.0, 20.0, 30.0, 40.0, 50.0]);
    assert_eq!(queue.len(), 0);
    assert!(queue.is_empty());
}

/// Java re-inserts mutated elements while popping (plan-6 ruling 4), so the tree has to stay
/// correct across interleaved `add`/`remove`. 200 rounds of pop-two, push-three.
#[test]
fn interleaved_pops_and_pushes_keep_the_tree_sorted() {
    let ids: HashMap<ExpandableRef, i32> = (0..2000)
        .map(|i| (ExpandableRef::Door(DoorId(i)), i as i32))
        .collect();
    let resolve = |door: ExpandableRef| *ids.get(&door).expect("registered");

    let mut queue: JavaTreeSet<MazeListElement> = JavaTreeSet::new();
    let mut door = 0u32;
    let mut value = 0.0f64;
    for _ in 0..200 {
        for _ in 0..3 {
            // A cheap deterministic spread, so the tree is not built in sorted order.
            value = (value * 7.0 + 13.0) % 1000.0;
            queue.add_by(element(door as i32, 0, 0.0, value), |a, b| {
                a.compare_to(b, resolve)
            });
            door += 1;
        }
        for _ in 0..2 {
            queue.poll_first();
        }
    }
    let remaining: Vec<f64> = std::iter::from_fn(|| queue.poll_first())
        .map(|e| e.sorting_value)
        .collect();
    assert!(!remaining.is_empty());
    assert!(
        remaining.windows(2).all(|w| w[0] <= w[1]),
        "the survivors come out in ascending sortingValue order: {remaining:?}"
    );
}
