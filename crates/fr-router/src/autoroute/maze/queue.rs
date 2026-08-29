//! Port of the anonymous `TreeSet<MazeListElement>` subclass `MazeSearchEngine` installs at
//! `MazeSearchEngine.java:84-125` — a sorted set whose `add` refuses elements outside the fanout
//! escape-length window.
//!
//! # Why this is a type and not a `BTreeSet`
//!
//! Plan-6 rulings 4 and Y/Z. Java pops through `mazeExpansionList.iterator().next()` +
//! `it.remove()` (`:327-329`) and **re-inserts mutated elements**, so the container is a sorted
//! set, never a heap; and `MazeListElement.compareTo` is not a total order (quirk #170's NaN
//! fall-through, quirk #171's full-tie drop), on which `java.util.TreeSet` and `BTreeSet` keep
//! and order **different** elements — measured in Task 4 and the reason
//! [`JavaTreeSet`] exists. The queue is a `JavaTreeSet`.
//!
//! # Is the comparator a total order?
//!
//! Almost. On non-NaN inputs the chain `sortingValue`, `expansionValue`, `door.getId()`,
//! `sectionNoOfDoor` is a lexicographic order on four totally ordered keys, so it *is* total
//! there — antisymmetric and transitive — and `Equal` really does mean "the same door section at
//! the same cost". Two things break it anyway, and both are load-bearing:
//!
//! 1. **NaN** (quirk #170). `a.sortingValue = NaN`, `b.sortingValue = 1.0`,
//!    `a.expansionValue < b.expansionValue` gives `a < b`; swap the expansion values and the same
//!    pair gives `a > b` — the relation is decided by a key Java only meant as a tie-break, and
//!    with three elements it is not transitive.
//! 2. **`door.getId()` moves.** `DrillPage.getId` hashes the page's `netNumber`, which
//!    `DrillPage.getDrills` overwrites (`DrillPage.java:65`, quirk #167), so an element already
//!    in the tree can change its third key. No comparator-based container is defined on that.
//!
//! So the container has to be the one whose *undefined* behaviour matches Java's, which is
//! `JavaTreeSet` — including `TreeMap.put`'s tie-drop, which is quirk #171 and is reachable
//! without any NaN at all.

use fr_board::Board;
use fr_board::structure::Unit;

use crate::autoroute::expansion::ExpandableRef;
use crate::autoroute::maze::{AutorouteControl, AutorouteEngine, MazeListElement};
use crate::java_tree_set::JavaTreeSet;

/// Port of the anonymous `TreeSet<MazeListElement>` of `MazeSearchEngine.java:84-125`.
///
/// The guard lives on the container, exactly as Java's override does, so no call site can insert
/// past it — `MazeExpansionEngine.java:101`, `:142`, `:373` and `MazeSearchEngine.java:547`,
/// `:964`, `:1079` all go through `add`.
#[derive(Debug, Clone, Default)]
pub struct MazeQueue {
    set: JavaTreeSet<MazeListElement>,
}

impl MazeQueue {
    /// `new TreeSet<>() { … }` (`:84-125`).
    pub fn new() -> MazeQueue {
        MazeQueue {
            set: JavaTreeSet::new(),
        }
    }

    /// Port of the overridden `add(MazeListElement)` (`MazeSearchEngine.java:86-124`).
    ///
    /// Returns Java's `boolean`: `false` for an element the fanout window rejects (`:104`,
    /// `:120`) **and** for one `super.add` drops because it compares `Equal` to an element
    /// already in the tree (quirk #171). The two are indistinguishable to the caller in Java too
    /// — none of the six call sites reads the answer.
    ///
    /// renamed: `MazeSearchEngine.add` -> `MazeQueue::push`; the method is an override on an
    /// anonymous class that has no name of its own, and `add` is already
    /// [`JavaTreeSet::add`], the `super.add` this delegates to.
    ///
    /// # Note for Tasks 11-13
    ///
    /// `engine` is a parameter, not a field, for the same reason `board` is one on
    /// [`AutorouteEngine`]'s methods: Java's `MazeSearchEngine` holds `autorouteEngine` *and*
    /// `mazeExpansionList` as sibling fields, and a port that did the same could not write
    /// `self.queue.push(e, ctrl, &self.engine, board)` — that is two borrows of `self`. Keep the
    /// engine outside the search struct (Task 6's arrangement) or split the borrow explicitly;
    /// do not "solve" it by snapshotting the door id into the element, which is the one thing
    /// quirk #167 forbids.
    pub fn push(
        &mut self,
        element: MazeListElement,
        ctrl: &AutorouteControl,
        engine: &AutorouteEngine,
        board: &Board,
    ) -> bool {
        // :87
        if ctrl.is_fanout
            && let Some(pin_center) = ctrl.fanout_start_pin_center.as_ref()
        {
            let pin_center_float = pin_center.to_float(); // :88-89

            // :90-92. `element.nextRoom.getLayer()` — Java's `nextRoom` is a
            // `CompleteExpansionRoom`, so the layer comes from the store.
            let on_start_layer = element.next_room.is_some_and(|room| {
                engine.rooms.room_layer(board, room).is_some_and(|layer| {
                    i32::try_from(layer).is_ok_and(|layer| layer == ctrl.fanout_start_pin_layer)
                })
            });
            if on_start_layer {
                // :94-98. `ctrl.settings.fanout.maxEscapeLengthMm * 1000.0` or `3000.0`, copied
                // into the control block by plan-6 ruling 8.
                let max_len = ctrl.fanout_max_escape_length;
                let resolution = board.communication.get_resolution(Unit::Um); // :99-101
                // :102-103
                let entry_point = element.shape_entry.a.middle_point(&element.shape_entry.b);
                let dist = entry_point.distance(&pin_center_float);
                if dist > max_len * resolution {
                    return false; // :104-106
                }
            }
            // :108-122
            if let ExpandableRef::Drill(drill) = element.door {
                let min_len = ctrl.fanout_min_escape_length; // :109-113
                let resolution = board.communication.get_resolution(Unit::Um); // :114-116
                let drill_dist = engine
                    .rooms
                    .drills
                    .get(drill.0)
                    .expect("MazeSearchEngine.add: the element's drill (Java holds a reference)")
                    .location
                    .to_float()
                    .distance(&pin_center_float); // :117-118
                if drill_dist < min_len * resolution {
                    return false; // :119-121
                }
            }
        }
        // :123 — `super.add(element)`, i.e. `TreeMap.put`.
        let door_id = |door: ExpandableRef| engine.expandable_id_no(door);
        self.set.add_by(element, |a, b| a.compare_to(b, door_id))
    }

    /// `mazeExpansionList.iterator().next()` followed by `it.remove()`
    /// (`MazeSearchEngine.java:327-329`) — `TreeMap.getFirstEntry` then `TreeMap.deleteEntry`.
    pub fn pop_first(&mut self) -> Option<MazeListElement> {
        self.set.poll_first()
    }

    /// `mazeExpansionList.isEmpty()` (`MazeSearchEngine.java:322`).
    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }

    /// `mazeExpansionList.size()`.
    pub fn len(&self) -> usize {
        self.set.len()
    }

    /// The in-order traversal, for tests and diagnostics. Java's callers only ever take the
    /// first element.
    pub fn iter(&self) -> impl Iterator<Item = &MazeListElement> {
        self.set.iter()
    }
}
