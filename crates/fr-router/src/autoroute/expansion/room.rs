//! Ports of the three interfaces of `autoroute.expansion`: `ExpansionRoom`
//! (ExpansionRoom.java:7-34), `CompleteExpansionRoom` (CompleteExpansionRoom.java:8-18) and
//! `ExpandableObject` (ExpandableObject.java:7-34).
//!
//! Java dispatches on the interface; the port dispatches on an **enum of arena indices**
//! (plan-6 ruling 16), because every room and every door is reached through an index anyway and
//! `&dyn ExpansionRoom` would need the lifetime of the arena that owns it — which the engine
//! mutates while the maze search holds the reference.
//!
//! The interface methods themselves are not on these enums. Java's rooms and doors keep their
//! *own* state (a door list, a shape, a section array) and the port keeps it on the concrete
//! structs; what needs the whole database — `doorExists`, `removeDoor`, `getId` across the three
//! room kinds — lives on [`ExpansionRoomStore`](super::ExpansionRoomStore), which is the object
//! Java's `this` plus the heap add up to.

use fr_board::{ObstacleRoomId, RoomId};

use crate::arena::{DoorId, DrillId, IncompleteRoomId, PageId, TargetDoorId};

/// Port of the `ExpansionRoom` interface (ExpansionRoom.java:7-34) and of
/// `CompleteExpansionRoom` (CompleteExpansionRoom.java:8-18), as an enum over the three
/// implementors: `CompleteFreeSpaceExpansionRoom`, `ObstacleExpansionRoom` and
/// `IncompleteFreeSpaceExpansionRoom`.
///
/// Java's two interfaces are one enum here because the distinction is a *predicate* on the
/// variant — [`RoomRef::is_complete`] — not a separate hierarchy: `CompleteExpansionRoom` adds
/// only `getTargetDoors`, `getObject` and `emitDiagnostic`, and the first two are answered by
/// the variant.
///
/// # Identity
///
/// Comparing two `RoomRef`s is Java's `==` on the object references, because an arena index
/// *is* the identity — indices are never reused (see [`crate::Arena`]). That is what
/// `ExpansionDoor.otherRoom` (`:64-68`) and `FreeSpaceExpansionRoom.doorExists` (`:86`) do.
///
/// # Ordering (quirk #161)
///
/// The derived [`Ord`] is **not** a port of any Java comparator and no sorted container may be
/// keyed on it. Java sorts rooms only through `CompleteFreeSpaceExpansionRoom.compareTo`
/// (`:45-53`), which is reproduced by [`fr_board::TreeObject`]'s `Ord` for the rooms that reach
/// a search tree. That method is also **latently broken**: it tests
/// `other instanceof FreeSpaceExpansionRoom` and then casts to `CompleteFreeSpaceExpansionRoom`,
/// so an *incomplete* room in a sorted set would throw `ClassCastException`. It is unreachable
/// today — incomplete rooms never enter a tree, and nothing else sorts rooms — and is recorded,
/// not fixed (`docs/java-quirks.md` #161). The derive exists so `RoomRef` can sit in a
/// `BTreeSet` for the port's own bookkeeping, which Java does not do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RoomRef {
    /// `CompleteFreeSpaceExpansionRoom` — the only room kind that enters the search tree, which
    /// is why its index type is [`fr_board::RoomId`] and lives in `fr-board`.
    Complete(RoomId),
    /// `ObstacleExpansionRoom` — the room an item's tree shape carries.
    Obstacle(ObstacleRoomId),
    /// `IncompleteFreeSpaceExpansionRoom` — a room whose shape is not yet computed.
    Incomplete(IncompleteRoomId),
}

impl RoomRef {
    /// Java's `room instanceof CompleteExpansionRoom`: true for the two complete kinds.
    pub fn is_complete(self) -> bool {
        matches!(self, RoomRef::Complete(_) | RoomRef::Obstacle(_))
    }

    /// Java's `room instanceof FreeSpaceExpansionRoom`: true for the two free-space kinds,
    /// which are the ones that extend `FreeSpaceExpansionRoom` (`:8`).
    pub fn is_free_space(self) -> bool {
        matches!(self, RoomRef::Complete(_) | RoomRef::Incomplete(_))
    }
}

/// Port of the `ExpandableObject` interface (ExpandableObject.java:7-34), as an enum over its
/// four implementors: `ExpansionDoor`, `TargetItemExpansionDoor`, `ExpansionDrill` and
/// `DrillPage`.
///
/// Task 2 supplies the first two; the drill and the page arrive with `autoroute/drill/` in
/// Task 7. The variants exist now because `MazeSearchElement.backtrackDoor` (`:12`) is an
/// `ExpandableObject` and the maze search stores drills and pages in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExpandableRef {
    /// `autoroute.expansion.ExpansionDoor`.
    Door(DoorId),
    /// `autoroute.expansion.TargetItemExpansionDoor`.
    TargetDoor(TargetDoorId),
    /// `autoroute.drill.ExpansionDrill`.
    Drill(DrillId),
    /// `autoroute.drill.DrillPage`.
    Page(PageId),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_complete_matches_javas_two_instanceof_tests() {
        assert!(RoomRef::Complete(RoomId(0)).is_complete());
        assert!(RoomRef::Obstacle(ObstacleRoomId(0)).is_complete());
        assert!(!RoomRef::Incomplete(IncompleteRoomId(0)).is_complete());

        assert!(RoomRef::Complete(RoomId(0)).is_free_space());
        assert!(!RoomRef::Obstacle(ObstacleRoomId(0)).is_free_space());
        assert!(RoomRef::Incomplete(IncompleteRoomId(0)).is_free_space());
    }

    #[test]
    fn a_room_ref_is_an_identity_not_a_value() {
        // Two rooms with identical geometry are still different rooms, because the index is the
        // identity — Java's `==` on references.
        assert_ne!(RoomRef::Complete(RoomId(1)), RoomRef::Complete(RoomId(2)));
        assert_ne!(
            RoomRef::Complete(RoomId(1)),
            RoomRef::Incomplete(IncompleteRoomId(1))
        );
    }
}
