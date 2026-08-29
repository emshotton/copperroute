//! Plan 6 Task 8: the port of `src/test/java/app/freerouting/autoroute/maze/MazeListElementTest`
//! (plan-6 ruling 12), plus the two quirk tests ruling 4 asks for.
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

// =================================================================================================
// The two Java test methods, ported one for one
// =================================================================================================

/// `MazeListElementTest.compareToReturnsZeroForSameInstance` (:14-20).
#[test]
fn compare_to_returns_zero_for_same_instance() {
    let ids = test_doors(&[1]);
    let e = element(1, 0, 0.0, 1.0);

    assert_eq!(e.compare_to(&e, &ids), Ordering::Equal);
}

/// `MazeListElementTest.compareToSortsBySortingValue` (:22-35): "lower sortingValue must be
/// expanded first".
///
/// Java's `SortedSet<MazeListElement> queue = new TreeSet<>()` is [`JavaTreeSet`] here (plan-6
/// ruling Y/Z), and `queue.first()` is its first in-order entry.
#[test]
fn compare_to_sorts_by_sorting_value() {
    let ids = test_doors(&[1, 2]);
    let lower_cost = element(1, 0, 0.0, 1.0);
    let higher_cost = element(2, 0, 0.0, 2.0);

    let mut queue: JavaTreeSet<MazeListElement> = JavaTreeSet::new();
    queue.add_by(higher_cost.clone(), |a, b| a.compare_to(b, &ids));
    queue.add_by(lower_cost.clone(), |a, b| a.compare_to(b, &ids));

    let first = queue.iter().next().expect("a non-empty queue");
    assert_eq!(
        first, &lower_cost,
        "Lower sortingValue must be expanded first"
    );
}

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
// The two quirks plan-6 ruling 4 names
// =================================================================================================

/// Quirk #170: `compareTo` compares two `double`s with raw `<`/`>`, so a `NaN` makes **both**
/// tests false and the comparison **falls through to the next key** instead of ordering.
///
/// `f64::total_cmp` (NaN sorts after every number) and `partial_cmp().unwrap()` (a panic) both
/// get this wrong; the port transcribes the fall-through.
#[test]
fn nan_sorting_value_falls_through_to_the_next_key() {
    let ids = test_doors(&[1, 2]);

    // A NaN sortingValue against a number: `<` and `>` are both false, so `expansionValue`
    // decides — and it decides *for* the NaN element, which a NaN-sorts-last comparator would
    // put at the end of the queue instead.
    let nan = element(1, 0, 1.0, f64::NAN);
    let number = element(2, 0, 5.0, 1.0);
    assert_eq!(nan.compare_to(&number, &ids), Ordering::Less);
    assert_eq!(number.compare_to(&nan, &ids), Ordering::Greater);

    // Two NaN sorting values fall through to expansionValue as well.
    assert_eq!(
        element(1, 0, 1.0, f64::NAN).compare_to(&element(2, 0, 2.0, f64::NAN), &ids),
        Ordering::Less
    );

    // A NaN expansionValue falls through one further, to the door id.
    assert_eq!(
        element(1, 0, f64::NAN, 1.0).compare_to(&element(2, 0, 3.0, 1.0), &ids),
        Ordering::Less
    );

    // And a NaN on both keys leaves the door id and section number as the whole order.
    assert_eq!(
        element(2, 0, f64::NAN, f64::NAN).compare_to(&element(1, 0, f64::NAN, f64::NAN), &ids),
        Ordering::Greater
    );

    // The comparator is therefore *not* reflexive on NaN in the total-order sense — but it does
    // answer `Equal` for a NaN element against itself, because every key falls through.
    let n = element(1, 4, f64::NAN, f64::NAN);
    assert_eq!(n.compare_to(&n, &ids), Ordering::Equal);
}

/// Quirk #171: a full tie answers `0` (`:110-112`) and `TreeMap.put` then **keeps the element
/// already in the tree** and answers `false` — the new one is silently dropped, values and all.
///
/// Two elements can tie on all four keys while differing in `expansionValue`'s *consumers*:
/// `backtrackDoor`, `nextRoom`, `shapeEntry`, `roomRipped`, `adjustment` and `ripupCost` are not
/// compared at all, so the queue can drop the cheaper backtrack path.
#[test]
fn a_full_tie_is_dropped_by_the_set() {
    let ids = test_doors(&[1]);

    let first = element(1, 2, 3.0, 4.0);
    let mut second = element(1, 2, 3.0, 4.0);
    second.room_ripped = true;
    second.ripup_cost = 999;
    second.adjustment = MazeAdjustment::Left;
    second.backtrack_door = Some(ExpandableRef::Door(DoorId(1)));

    let mut queue: JavaTreeSet<MazeListElement> = JavaTreeSet::new();
    assert!(queue.add_by(first.clone(), |a, b| a.compare_to(b, &ids)));
    assert!(
        !queue.add_by(second, |a, b| a.compare_to(b, &ids)),
        "TreeSet.add answers false for a key that compares Equal"
    );

    assert_eq!(queue.len(), 1);
    let kept = queue.iter().next().expect("the one element");
    assert_eq!(
        kept, &first,
        "the element already in the tree is kept; the new one is dropped whole"
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
