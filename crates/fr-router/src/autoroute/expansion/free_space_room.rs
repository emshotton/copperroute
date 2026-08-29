//! Port of the abstract `autoroute.expansion.FreeSpaceExpansionRoom`
//! (FreeSpaceExpansionRoom.java:8-91) — the state and behaviour
//! `CompleteFreeSpaceExpansionRoom` and `IncompleteFreeSpaceExpansionRoom` share.
//!
//! Java uses inheritance; the port uses **composition**: both subclasses hold one of these as
//! their `base` field and delegate. That keeps the audit map's one-file-per-class split honest
//! (this file is `FreeSpaceExpansionRoom`'s and nothing else's) and it keeps the two subclasses'
//! extra state — the tree leaf and the target doors on one side, the contained shape on the
//! other — out of the shared struct.

use fr_geometry::TileShape;

use crate::Arena;
use crate::arena::DoorId;
use crate::autoroute::expansion::{ExpansionDoor, RoomRef};

/// Port of `FreeSpaceExpansionRoom` (FreeSpaceExpansionRoom.java:8-91).
#[derive(Debug, Clone, PartialEq)]
pub struct FreeSpaceExpansionRoom {
    /// `private TileShape shape` (:14) — **nullable**, and the null is load-bearing: the
    /// constructor's own javadoc says "if shape == null, it means shape is the whole plane"
    /// (IncompleteFreeSpaceExpansionRoom.java:14-16), and
    /// `ExpansionDrill.calculateExpansionRooms` builds exactly that
    /// (`autoroute/drill/ExpansionDrill.java:77`, `new IncompleteFreeSpaceExpansionRoom(null, i,
    /// searchShape)`). So this is `Option<TileShape>`, not `TileShape`.
    shape: Option<TileShape>,
    /// `private final int layer` (:11).
    layer: usize,
    /// `private List<ExpansionDoor> doors` (:20), an `ArrayList` "for better cache locality and
    /// O(1) indexed access" — a `Vec` of arena indices here.
    doors: Vec<DoorId>,
}

impl FreeSpaceExpansionRoom {
    /// Port of the protected constructor (FreeSpaceExpansionRoom.java:27-31).
    pub fn new(shape: Option<TileShape>, layer: usize) -> FreeSpaceExpansionRoom {
        FreeSpaceExpansionRoom {
            shape,
            layer,
            doors: Vec::new(),
        }
    }

    /// Port of `addDoor` (FreeSpaceExpansionRoom.java:34-37).
    pub fn add_door(&mut self, door: DoorId) {
        self.doors.push(door);
    }

    /// Port of `getDoors` (FreeSpaceExpansionRoom.java:40-43). Java hands back the live list,
    /// which its callers mutate; the port hands back a slice and mutates through the other
    /// methods of this type.
    pub fn get_doors(&self) -> &[DoorId] {
        &self.doors
    }

    /// Port of `clearDoors` (FreeSpaceExpansionRoom.java:46-49). Java replaces the list with a
    /// fresh `ArrayList` rather than clearing it, which matters only to a caller holding the old
    /// one — none does.
    pub fn clear_doors(&mut self) {
        self.doors = Vec::new();
    }

    /// Port of `removeDoor` (FreeSpaceExpansionRoom.java:58-61): `List.remove(Object)` removes
    /// the **first** equal element and answers whether it found one.
    pub fn remove_door(&mut self, door: DoorId) -> bool {
        match self.doors.iter().position(|d| *d == door) {
            Some(index) => {
                self.doors.remove(index);
                true
            }
            None => false,
        }
    }

    /// Port of `resetDoors` (FreeSpaceExpansionRoom.java:51-56): `currentDoor.reset()` for every
    /// door of this room.
    ///
    /// Java reaches the door objects through the list; the port reaches them through the door
    /// arena, which is why this takes it. A door id with no live door is skipped — Java cannot
    /// have one, since a removed door is unlinked from both its rooms first.
    pub fn reset_doors(&self, doors: &mut Arena<ExpansionDoor>) {
        for door in &self.doors {
            if let Some(door) = doors.get_mut(door.0) {
                door.reset();
            }
        }
    }

    /// Port of `doorExists` (FreeSpaceExpansionRoom.java:80-91): "checks if this room already
    /// has a door to other".
    ///
    /// Java's `doors == null` guard (:82-84) cannot happen here — the field is a `Vec` and the
    /// constructor fills it — and its `==` on room references is `RoomRef`'s equality, since an
    /// arena index is the identity.
    pub fn door_exists(&self, doors: &Arena<ExpansionDoor>, other: RoomRef) -> bool {
        self.doors.iter().any(|door| {
            doors
                .get(door.0)
                .is_some_and(|d| d.first_room == other || d.second_room == other)
        })
    }

    /// Port of `getShape` (FreeSpaceExpansionRoom.java:64-67). `None` is Java's null shape,
    /// i.e. "the whole plane".
    pub fn get_shape(&self) -> Option<&TileShape> {
        self.shape.as_ref()
    }

    /// Port of `setShape` (FreeSpaceExpansionRoom.java:70-72).
    ///
    /// **Hazard C** (plan-6 ruling 4): the shape is mutable and
    /// `IncompleteFreeSpaceExpansionRoom.getId` (`:38-41`) hashes it, so a room's id changes
    /// under a sorted container that holds it. Reproduced, not fixed; see
    /// [`super::IncompleteFreeSpaceExpansionRoom::get_id`].
    pub fn set_shape(&mut self, shape: Option<TileShape>) {
        self.shape = shape;
    }

    /// Port of `getLayer` (FreeSpaceExpansionRoom.java:74-77).
    pub fn get_layer(&self) -> usize {
        self.layer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fr_board::RoomId;
    use fr_geometry::IntBox;

    fn boxed(llx: i32, lly: i32, urx: i32, ury: i32) -> TileShape {
        TileShape::Box(IntBox::from_coords(llx, lly, urx, ury))
    }

    #[test]
    fn remove_door_takes_the_first_match_and_reports_whether_it_found_one() {
        let mut room = FreeSpaceExpansionRoom::new(Some(boxed(0, 0, 1, 1)), 0);
        room.add_door(DoorId(1));
        room.add_door(DoorId(2));
        room.add_door(DoorId(1));
        assert!(room.remove_door(DoorId(1)));
        assert_eq!(room.get_doors(), &[DoorId(2), DoorId(1)]);
        assert!(room.remove_door(DoorId(1)));
        assert!(!room.remove_door(DoorId(1)));
    }

    #[test]
    fn clear_doors_empties_the_list() {
        let mut room = FreeSpaceExpansionRoom::new(None, 3);
        room.add_door(DoorId(7));
        room.clear_doors();
        assert!(room.get_doors().is_empty());
        assert_eq!(room.get_layer(), 3);
    }

    #[test]
    fn a_null_shape_is_the_whole_plane() {
        // ExpansionDrill.java:77 builds exactly this.
        let mut room = FreeSpaceExpansionRoom::new(None, 0);
        assert_eq!(room.get_shape(), None);
        room.set_shape(Some(boxed(0, 0, 5, 5)));
        assert_eq!(room.get_shape(), Some(&boxed(0, 0, 5, 5)));
    }

    #[test]
    fn door_exists_finds_either_side() {
        let mut doors: Arena<ExpansionDoor> = Arena::new();
        let a = RoomRef::Complete(RoomId(0));
        let b = RoomRef::Complete(RoomId(1));
        let id = DoorId(doors.insert(ExpansionDoor::new(a, b, 1)));

        let mut room = FreeSpaceExpansionRoom::new(Some(boxed(0, 0, 1, 1)), 0);
        assert!(!room.door_exists(&doors, b));
        room.add_door(id);
        assert!(room.door_exists(&doors, a));
        assert!(room.door_exists(&doors, b));
        assert!(!room.door_exists(&doors, RoomRef::Complete(RoomId(9))));
    }
}
