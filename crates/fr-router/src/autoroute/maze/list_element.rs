//! Port of `autoroute.maze.MazeListElement` (MazeListElement.java:11-114) — "information for the
//! maze expand algorithm contained in expansion doors and drills while the maze expanding
//! algorithm is in progress", and the queue's sort key.

use std::cmp::Ordering;

use fr_geometry::FloatLine;

use crate::autoroute::expansion::{ExpandableRef, RoomRef};
use crate::autoroute::maze::MazeAdjustment;

/// Port of `maze.MazeListElement` (MazeListElement.java:11-114).
///
/// Every field is Java's, in Java's declaration order. They are `pub` because Java's are
/// package-private and `MazeSearchEngine`/`MazeExpansionEngine` read them directly.
///
/// # Java-vs-brief
///
/// The task brief omits `ripupCost` (`:51`). HEAD has it, and it is not decoration:
/// `MazeSearchEngine.java:546` and `:905` write it **after** construction, and `:342` copies it
/// into the door section the search occupies. It is carried here.
///
/// # The door is a reference, not a value
///
/// Java's `door` is an `ExpandableObject` and `compareTo` calls `door.getId()` on it. The port
/// holds an [`ExpandableRef`] — an arena index — because none of the four `getId()`s is a
/// property of the object alone: `ExpansionDoor.getId` hashes the two rooms' ids
/// (`ExpansionDoor.java:184-190`), `TargetItemExpansionDoor.getId` the item's and the room's
/// (`TargetItemExpansionDoor.java:70-74`), and `DrillPage.getId` the shape's *and the mutable
/// `netNumber`* (`DrillPage.java:189-193`). So [`MazeListElement::compare_to`] takes a resolver
/// and calls it exactly where Java performs the virtual call — see the note on the method.
#[derive(Debug, Clone, PartialEq)]
pub struct MazeListElement {
    /// `final ExpandableObject door` (`:14`): "the door or drill belonging to this
    /// MazeListElement".
    pub door: ExpandableRef,
    /// `final int sectionNoOfDoor` (`:17`): "the section number of the door (or the layer of the
    /// drill)".
    pub section_no_of_door: i32,
    /// `final ExpandableObject backtrackDoor` (`:20`): "the door, from which this door was
    /// expanded". `None` is Java's `null` (`MazeSearchEngine.java:1070`).
    pub backtrack_door: Option<ExpandableRef>,
    /// `final int sectionNoOfBacktrackDoor` (`:23`).
    pub section_no_of_backtrack_door: i32,
    /// `final double expansionValue` (`:26`): "the weighted distance to the start of the
    /// expansion".
    pub expansion_value: f64,
    /// `final double sortingValue` (`:32`): "the expansion value plus the shortest distance to a
    /// destination. The list is sorted in ascending order by this value."
    pub sorting_value: f64,
    /// `final CompleteExpansionRoom nextRoom` (`:35`): "the next room, which will be expanded
    /// from this maze search element". `None` is Java's `null`
    /// (`MazeExpansionEngine.java:96`, a drill element).
    pub next_room: Option<RoomRef>,
    /// `final FloatLine shapeEntry` (`:41`): "point of the region of the expansion door, which
    /// has the shortest distance to the backtrack door".
    ///
    /// Not an `Option`: every production construction site passes a real line, and
    /// `MazeSearchEngine.java:99` dereferences it with no guard. `MazeListElementTest` passes
    /// `null`, but never reaches a method that reads it.
    pub shape_entry: FloatLine,
    /// `final boolean roomRipped` (`:43`).
    pub room_ripped: bool,
    /// `final MazeSearchElement.Adjustment adjustment` (`:44`).
    pub adjustment: MazeAdjustment,
    /// `final boolean alreadyChecked` (`:45`).
    pub already_checked: bool,
    /// `int ripupCost` (`:51`): "the ripup cost paid to enter the nextRoom through this door.
    /// Non-zero only when roomRipped is true and this element was directly created by
    /// expand_to_door_section with a positive add_costs."
    ///
    /// The one non-`final` field, written after construction at `MazeSearchEngine.java:546` and
    /// `:905`.
    pub ripup_cost: i32,
}

impl MazeListElement {
    /// Port of `compareTo(MazeListElement)` (MazeListElement.java:79-113), transcribed as Java's
    /// chain of raw `<`/`>` tests rather than as `f64::total_cmp` or `partial_cmp().unwrap()`
    /// (plan-6 ruling 4).
    ///
    /// `door_id` is the `door.getId()` virtual call of `:95-96`. It is a parameter — resolved at
    /// *comparison* time, never snapshotted into the struct — because
    /// `DrillPage.getId` (`DrillPage.java:189-193`) hashes the page's `netNumber`, which
    /// `DrillPage.getDrills` overwrites at `:65` while the page may already be an element of the
    /// queue (quirk #167, hazard B). A cached id would freeze a key Java re-reads.
    ///
    /// # fixed: T8 (#171, #170)
    ///
    /// **#170 — the NaN.** `:81-93` compare two `double`s with raw `<` and `>`. A `NaN` makes
    /// **both** false, so the comparison falls through to the next key instead of ordering, and
    /// the relation stops being transitive: with `A.sorting = NaN`, `B.sorting = 5.0`,
    /// `C.sorting = 3.0` and expansion values `1.0`, `2.0`, `9.0`, Java answers `A < B`, `B > C`
    /// and `A < C`, which no strict weak ordering permits — and a red-black tree built on it can
    /// find or not find the same element depending on its shape. The fix is **not** here: a
    /// non-finite cost is an upstream bug, not a thing to sort, so [`MazeQueue::push`] refuses the
    /// element and this method never sees one. `total_cmp` would have been the wrong fix — it
    /// silently sorts NaN last and keeps the bug alive.
    ///
    /// [`MazeQueue::push`]: crate::autoroute::maze::MazeQueue::push
    ///
    /// **#171 — the four-key tie.** `:110-112` answers `0` for a full tie and `TreeMap.put` then
    /// keeps the element already in the tree and **drops the newcomer whole** — including its
    /// `backtrackDoor`, which is the entire path, and its `ripupCost`, `shapeEntry` and
    /// `roomRipped`, none of which Java compares. Two genuinely different routes arriving at the
    /// same door section at the same cost were collapsed to one, and the survivor was whichever
    /// was inserted first: not the cheaper, not the one with the lower ripup cost.
    ///
    /// **The policy, written down** (the invariant asks for one): *both are kept*. The comparison
    /// continues past `:110-112` through the remaining eight fields, in the struct's — that is,
    /// Java's — declaration order, so `Equal` means the two elements are equal as values and a
    /// set cannot lose a path. The four keys Java compares keep their meaning and their order:
    /// what follows only separates elements Java could not tell apart.
    ///
    /// Note that `door.getId()` is a **hash** even over injective room ids — `ExpansionDoor::id`
    /// is `min * 31 + max`, so doors between rooms `(1, 63)` and `(2, 32)` both answer 94 — so
    /// key 3 can tie for two *different* doors. That is why the door itself is compared below and
    /// not only its id.
    pub fn compare_to<F>(&self, other: &MazeListElement, door_id: F) -> Ordering
    where
        F: Fn(ExpandableRef) -> i32,
    {
        // :81-86
        if self.sorting_value < other.sorting_value {
            return Ordering::Less;
        }
        if self.sorting_value > other.sorting_value {
            return Ordering::Greater;
        }
        // Tie-break 1: expansionValue (:88-93)
        if self.expansion_value < other.expansion_value {
            return Ordering::Less;
        }
        if self.expansion_value > other.expansion_value {
            return Ordering::Greater;
        }
        // Tie-break 2: door id (:95-102)
        let id1 = door_id(self.door);
        let id2 = door_id(other.door);
        if id1 < id2 {
            return Ordering::Less;
        }
        if id1 > id2 {
            return Ordering::Greater;
        }
        // Tie-break 3: sectionIndex (:104-109)
        if self.section_no_of_door < other.section_no_of_door {
            return Ordering::Less;
        }
        if self.section_no_of_door > other.section_no_of_door {
            return Ordering::Greater;
        }
        // Java stops here: "If truly equal (same door, same section, same values), return 0 to
        // avoid duplicates in the set." (:110-112) — but "the same values" is only true of the
        // four keys above, and the eight fields below are the ones a dropped element takes with
        // it. fixed: T8 (#171).
        self.door
            .cmp(&other.door)
            .then_with(|| self.backtrack_door.cmp(&other.backtrack_door))
            .then_with(|| {
                self.section_no_of_backtrack_door
                    .cmp(&other.section_no_of_backtrack_door)
            })
            .then_with(|| self.next_room.cmp(&other.next_room))
            .then_with(|| line_key(&self.shape_entry).cmp(&line_key(&other.shape_entry)))
            .then_with(|| self.room_ripped.cmp(&other.room_ripped))
            .then_with(|| self.adjustment.cmp(&other.adjustment))
            .then_with(|| self.already_checked.cmp(&other.already_checked))
            .then_with(|| self.ripup_cost.cmp(&other.ripup_cost))
    }
}

/// A [`FloatLine`] as a totally-ordered key: its four `f64`s through `total_cmp`.
///
/// `shapeEntry` is `MazeListElement`'s only floating-point field past the two cost keys, and it
/// is never `NaN` in practice — it is a point on a door's region — but `total_cmp` is used rather
/// than `partial_cmp` so that the ordering stays total whatever arrives.
fn line_key(line: &FloatLine) -> [OrderedF64; 4] {
    [
        OrderedF64(line.a.x),
        OrderedF64(line.a.y),
        OrderedF64(line.b.x),
        OrderedF64(line.b.y),
    ]
}

/// An `f64` with a total order (`f64::total_cmp`), so an array of them can be `cmp`ed.
#[derive(Debug, Clone, Copy, PartialEq)]
struct OrderedF64(f64);

impl Eq for OrderedF64 {}

impl PartialOrd for OrderedF64 {
    fn partial_cmp(&self, other: &OrderedF64) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedF64 {
    fn cmp(&self, other: &OrderedF64) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

// =================================================================================================
// Why there is no `impl Ord for MazeListElement`
// =================================================================================================

// renamed: `MazeListElement.compareTo` -> `MazeListElement::compare_to(&self, other, door_id)`.
//
// `Ord::cmp` cannot be the port of this method: `:95-96` needs `door.getId()`, and every one of
// the four implementations of that virtual call reads state the element does not own (two room
// ids, an item id, or a `DrillPage`'s moving `netNumber`). An `impl Ord` would have to snapshot
// the id at construction, which diverges from Java the moment the id moves — which is exactly
// what quirk #167 says it does. `JavaTreeSet::add_by` therefore takes the comparator, and
// `MazeQueue` supplies one closed over the engine's arenas.
//
// fixed: T8 (#171, #170) removed the *second* reason there is no `impl Ord`, but not the first.
// The comparator used not to be a total order at all — quirk #170's NaN fall-through meant
// `a < b`, `b < a` and `a == b` could all be false-ish in the `PartialOrd` sense — and it now is
// one, because `MazeQueue::push` refuses a non-finite `sortingValue` before the comparator can
// see it and because the four keys are followed by the remaining eight fields. What still forbids
// `Ord` is the resolver: `door.getId()` is not a property of the element. Once #156/#167/#158
// make every expandable id a stable counter the resolver can become a snapshot, and Task 24 is
// where that and the `JavaTreeSet` -> `BTreeSet` swap are collected.
//
// `PartialEq` is derived and is *not* the comparator: it is field-by-field equality, used only by
// tests. It is now the same relation the comparator's `Equal` describes, on every field except
// the door id the resolver supplies.
