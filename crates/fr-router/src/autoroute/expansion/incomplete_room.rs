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
    /// The engine's own id for this room — fixed: T8 (#158), see [`Self::get_id`]. No Java
    /// field: Java hashes the room's own (mutable, nullable) shape on every call.
    id_no: i32,
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
            id_no: 0,
        }
    }

    /// Give this room the engine's own id — fixed: T8 (#158).
    ///
    /// `ExpansionRoomStore::new_incomplete_room` and its unlisted sibling call this as they put
    /// the room in the arena, which is the moment the room becomes something the engine can name.
    /// A room `ShapeSearchTree::complete_shape` builds and hands back as a *candidate* never
    /// reaches the arena and keeps the placeholder `0`; nothing asks such a room for its id, and
    /// [`Self::get_id`] says so.
    pub fn set_id_no(&mut self, id_no: i32) {
        self.id_no = id_no;
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
    /// fixed: T8 (#158). It is the engine's own counter now, for two reasons Java's formula
    /// cannot answer:
    ///
    /// * **The shape is mutable.** [`FreeSpaceExpansionRoom::set_shape`] (`:70`) replaces it, and
    ///   `AutorouteEngine.completeExpansionRoom` calls it — so the id *moved* under any sorted
    ///   container holding the room, and the id reaches [`super::ExpansionDoor::get_id`], which
    ///   is a sort key of `MazeListElement`. An object's id must not change while it is an
    ///   element of an ordered collection; a counter cannot.
    /// * **The null shape NPEs.** Java calls `getShape().getId()` with no guard, so an
    ///   incomplete room built with a null shape — the **whole-plane** room, which
    ///   `ExpansionDrill.calculateExpansionRooms` supplies at
    ///   `autoroute/drill/ExpansionDrill.java:77` — throws a `NullPointerException` here. The
    ///   port used to panic with Java's message; a counter has nothing to dereference, so the
    ///   whole-plane room has an id like every other room.
    ///
    /// A room that never reached the arena — a *candidate* `ShapeSearchTree::complete_shape`
    /// built and handed back — has the placeholder `0` and is not an object the engine can name.
    pub fn get_id(&self) -> i32 {
        self.id_no
    }

    /// **Java's** `getId` arithmetic, kept so the hazard #158 fixed can be pinned. Nothing in the
    /// port reads it, and it still panics on the whole-plane room exactly where Java NPEs.
    ///
    /// # Panics
    /// If the room's shape is `None` — Java's `NullPointerException` at `:40`.
    pub fn java_id(&self) -> i32 {
        let shape = self.base.get_shape().unwrap_or_else(|| {
            panic!(
                "IncompleteFreeSpaceExpansionRoom.getId: the room has no shape — Java NPEs here \
                 too (IncompleteFreeSpaceExpansionRoom.java:40)"
            )
        });
        // `getLayer()` is a Java `int`, so the port's `usize` truncates rather than saturating:
        // `as i32` *is* Java's arithmetic.
        shape
            .get_id()
            .wrapping_mul(31)
            .wrapping_add(self.base.get_layer() as i32)
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
    fn the_id_is_the_engine_counter_and_javas_is_the_shape_hash() {
        // fixed: T8 (#158). Java's `getId` is `31 * getShape().getId() + getLayer()`, which
        // `java_id` keeps; the port's is the id the store handed the room.
        let shape = boxed(1, 2, 3, 4);
        let mut room = IncompleteFreeSpaceExpansionRoom::new(Some(shape.clone()), 5, None);
        assert_eq!(
            room.java_id(),
            shape.get_id().wrapping_mul(31).wrapping_add(5)
        );
        assert_eq!(room.get_id(), 0, "a candidate that never reached the arena");
        room.set_id_no(7);
        assert_eq!(room.get_id(), 7);
    }

    #[test]
    fn the_id_no_longer_moves_when_the_shape_is_replaced() {
        // Hazard C, **fixed: T8 (#158)**: the sort key used to mutate under the container, because
        // `FreeSpaceExpansionRoom::set_shape` replaces the very shape Java hashes — and
        // `AutorouteEngine.completeExpansionRoom` calls it. An object's id must not change while
        // it is an element of an ordered collection.
        let mut room = IncompleteFreeSpaceExpansionRoom::new(Some(boxed(0, 0, 1, 1)), 0, None);
        room.set_id_no(3);
        let before = room.get_id();
        let java_before = room.java_id();
        room.set_shape(Some(boxed(0, 0, 2, 2)));
        assert_eq!(room.get_id(), before);
        assert_ne!(room.java_id(), java_before, "Java's does move — the defect");
    }

    #[test]
    fn the_whole_plane_room_has_an_id() {
        // fixed: T8 (#158). Java calls `getShape().getId()` with no guard, so the whole-plane
        // room — the one `ExpansionDrill.calculateExpansionRooms` builds at
        // `autoroute/drill/ExpansionDrill.java:77` — throws a `NullPointerException` here. A
        // counter has nothing to dereference.
        let mut room = IncompleteFreeSpaceExpansionRoom::new(None, 0, Some(boxed(0, 0, 1, 1)));
        room.set_id_no(11);
        assert_eq!(room.get_id(), 11);
    }

    #[test]
    #[should_panic(expected = "the room has no shape")]
    fn javas_id_of_a_whole_plane_room_still_npes() {
        IncompleteFreeSpaceExpansionRoom::new(None, 0, Some(boxed(0, 0, 1, 1))).java_id();
    }

    #[test]
    fn target_doors_are_always_empty() {
        let room = IncompleteFreeSpaceExpansionRoom::new(Some(boxed(0, 0, 1, 1)), 0, None);
        assert!(room.get_target_doors().is_empty());
    }
}
