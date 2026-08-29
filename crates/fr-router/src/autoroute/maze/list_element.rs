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
    /// # Java bug: `MazeListElement.compareTo`
    ///
    /// `:81-93` compare two `double`s with raw `<` and `>`. A `NaN` makes **both** false, so the
    /// comparison **falls through to the next key** instead of ordering — `total_cmp` (NaN last)
    /// and `partial_cmp().unwrap()` (a panic) both get it wrong. `sortingValue` is
    /// `expansionValue + destinationDistance.calculate(…)`, and a degenerate
    /// `IntBox.weightedDistance` can produce a NaN, so this is reachable in principle. Quirk
    /// #170, pinned by `tests/maze_list_element.rs`'s
    /// `nan_sorting_value_falls_through_to_the_next_key`.
    ///
    /// The other half is `:110-112`: a full tie answers `0`, and `TreeMap.put` then keeps the
    /// element already in the tree and drops the new one whole — including its `backtrackDoor`,
    /// `shapeEntry`, `roomRipped` and `ripupCost`, none of which is compared. Quirk #171.
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
        // "If truly equal (same door, same section, same values), return 0 to avoid duplicates
        // in the set." (:110-112)
        Ordering::Equal
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
// The comparator is also **not a total order** even before the resolver: quirk #170's NaN
// fall-through means `a < b`, `b < a` and `a == b` can all be false-ish in the `PartialOrd`
// sense, and `Ord` promises a total order. Declaring one would be a lie the compiler cannot
// catch. `PartialEq` is derived and is *not* the comparator: it is field-by-field equality, used
// only by tests.
