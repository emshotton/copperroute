//! Port of `autoroute.expansion.IncompleteFreeSpaceExpansionRoom`
//! (IncompleteFreeSpaceExpansionRoom.java:8-42) — "an expansion room, whose shape is not yet
//! completely calculated".

use fr_geometry::TileShape;

use crate::Arena;
use crate::arena::{DoorId, TargetDoorId};
use crate::autoroute::expansion::{ExpansionDoor, FreeSpaceExpansionRoom, RoomRef};

/// Port of `IncompleteFreeSpaceExpansionRoom` (IncompleteFreeSpaceExpansionRoom.java:8-42).
///
/// The `FreeSpaceExpansionRoom` half is held by composition rather than inheritance; the
/// delegating accessors below are the `super` calls.
#[derive(Debug, Clone, PartialEq)]
pub struct IncompleteFreeSpaceExpansionRoom {
    /// The inherited `FreeSpaceExpansionRoom` state (`extends FreeSpaceExpansionRoom`, :8).
    pub base: FreeSpaceExpansionRoom,
    /// `private TileShape containedShape` (:11): "a shape which should be contained in the
    /// completed shape". Nullable in Java, like the inherited shape.
    contained_shape: Option<TileShape>,
}

impl IncompleteFreeSpaceExpansionRoom {
    /// Port of the constructor (IncompleteFreeSpaceExpansionRoom.java:17-20). "If shape ==
    /// null, it means shape is the whole plane" (:14-16).
    pub fn new(
        shape: Option<TileShape>,
        layer: usize,
        contained_shape: Option<TileShape>,
    ) -> IncompleteFreeSpaceExpansionRoom {
        IncompleteFreeSpaceExpansionRoom {
            base: FreeSpaceExpansionRoom::new(shape, layer),
            contained_shape,
        }
    }

    /// Port of `getContainedShape` (IncompleteFreeSpaceExpansionRoom.java:23-25).
    pub fn get_contained_shape(&self) -> Option<&TileShape> {
        self.contained_shape.as_ref()
    }

    /// Port of `setContainedShape` (IncompleteFreeSpaceExpansionRoom.java:28-30).
    pub fn set_contained_shape(&mut self, shape: Option<TileShape>) {
        self.contained_shape = shape;
    }

    /// Port of `getTargetDoors` (IncompleteFreeSpaceExpansionRoom.java:33-35): "returns an empty
    /// list of target doors for incomplete rooms". Java allocates a fresh `ArrayList` each call;
    /// the port answers an empty slice, which is indistinguishable — nothing writes to it.
    pub fn get_target_doors(&self) -> &[TargetDoorId] {
        &[]
    }

    /// Port of `getId` (IncompleteFreeSpaceExpansionRoom.java:37-41):
    /// `31 * getShape().getId() + getLayer()`, a "stable hash of shape and layer".
    ///
    /// # Two hazards, both reproduced
    ///
    /// * **Hazard C** (plan-6 ruling 4): the shape is *mutable*
    ///   ([`FreeSpaceExpansionRoom::set_shape`], `:70`), so this id changes under any sorted
    ///   container holding the room. Java has the same defect and the port does not fix it —
    ///   the id reaches [`super::ExpansionDoor::get_id`], which is a sort key of
    ///   `MazeListElement` (Task 8).
    /// * **The null shape NPEs.** Java calls `getShape().getId()` with no guard, so an
    ///   incomplete room built with a null shape — which
    ///   `ExpansionDrill.calculateExpansionRooms` does at
    ///   `autoroute/drill/ExpansionDrill.java:77` — throws a `NullPointerException` here. That
    ///   is a crash in both languages, so the port panics rather than inventing a value
    ///   (`docs/java-quirks.md` #162). It is unreachable today: nothing asks a drill's seed room
    ///   for its id before `completeExpansionRoom` replaces it.
    ///
    /// The arithmetic is Java `int`, so it wraps.
    ///
    /// # Panics
    /// If the room's shape is `None` — Java's `NullPointerException` at `:40`.
    pub fn get_id(&self) -> i32 {
        let shape = self.base.get_shape().unwrap_or_else(|| {
            // Java bug: IncompleteFreeSpaceExpansionRoom.getId dereferences a shape its own
            // constructor documents as nullable (:14-16, and ExpansionDrill.java:77 supplies a
            // null). Java throws NullPointerException; so does this.
            panic!(
                "IncompleteFreeSpaceExpansionRoom.getId: the room has no shape — Java NPEs here \
                 too (IncompleteFreeSpaceExpansionRoom.java:40)"
            )
        });
        shape
            .get_id()
            .wrapping_mul(31)
            .wrapping_add(self.layer_i32())
    }

    /// `getLayer()` as the `int` Java's `getId` adds.
    fn layer_i32(&self) -> i32 {
        i32::try_from(self.base.get_layer()).unwrap_or(i32::MAX)
    }

    // --- the delegating `super` calls ----------------------------------------------------------

    /// `super.getShape()` (FreeSpaceExpansionRoom.java:64-67).
    pub fn get_shape(&self) -> Option<&TileShape> {
        self.base.get_shape()
    }

    /// `super.setShape(TileShape)` (FreeSpaceExpansionRoom.java:70-72).
    pub fn set_shape(&mut self, shape: Option<TileShape>) {
        self.base.set_shape(shape);
    }

    /// `super.getLayer()` (FreeSpaceExpansionRoom.java:74-77).
    pub fn get_layer(&self) -> usize {
        self.base.get_layer()
    }

    /// `super.addDoor(ExpansionDoor)` (FreeSpaceExpansionRoom.java:34-37).
    pub fn add_door(&mut self, door: DoorId) {
        self.base.add_door(door);
    }

    /// `super.getDoors()` (FreeSpaceExpansionRoom.java:40-43).
    pub fn get_doors(&self) -> &[DoorId] {
        self.base.get_doors()
    }

    /// `super.clearDoors()` (FreeSpaceExpansionRoom.java:46-49).
    pub fn clear_doors(&mut self) {
        self.base.clear_doors();
    }

    /// `super.removeDoor(ExpandableObject)` (FreeSpaceExpansionRoom.java:58-61).
    pub fn remove_door(&mut self, door: DoorId) -> bool {
        self.base.remove_door(door)
    }

    /// `super.resetDoors()` (FreeSpaceExpansionRoom.java:51-56).
    pub fn reset_doors(&self, doors: &mut Arena<ExpansionDoor>) {
        self.base.reset_doors(doors);
    }

    /// `super.doorExists(ExpansionRoom)` (FreeSpaceExpansionRoom.java:80-91).
    pub fn door_exists(&self, doors: &Arena<ExpansionDoor>, other: RoomRef) -> bool {
        self.base.door_exists(doors, other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fr_geometry::IntBox;

    fn boxed(llx: i32, lly: i32, urx: i32, ury: i32) -> TileShape {
        TileShape::Box(IntBox::from_coords(llx, lly, urx, ury))
    }

    #[test]
    fn the_id_is_the_shape_hash_times_31_plus_the_layer() {
        let shape = boxed(1, 2, 3, 4);
        let room = IncompleteFreeSpaceExpansionRoom::new(Some(shape.clone()), 5, None);
        assert_eq!(
            room.get_id(),
            shape.get_id().wrapping_mul(31).wrapping_add(5)
        );
    }

    #[test]
    fn the_id_moves_when_the_shape_is_replaced() {
        // Hazard C: the sort key mutates under the container. Reproduced, not fixed.
        let mut room = IncompleteFreeSpaceExpansionRoom::new(Some(boxed(0, 0, 1, 1)), 0, None);
        let before = room.get_id();
        room.set_shape(Some(boxed(0, 0, 2, 2)));
        assert_ne!(room.get_id(), before);
    }

    #[test]
    #[should_panic(expected = "the room has no shape")]
    fn the_id_of_a_whole_plane_room_panics_like_javas_npe() {
        IncompleteFreeSpaceExpansionRoom::new(None, 0, Some(boxed(0, 0, 1, 1))).get_id();
    }

    #[test]
    fn target_doors_are_always_empty() {
        let room = IncompleteFreeSpaceExpansionRoom::new(Some(boxed(0, 0, 1, 1)), 0, None);
        assert!(room.get_target_doors().is_empty());
    }
}
