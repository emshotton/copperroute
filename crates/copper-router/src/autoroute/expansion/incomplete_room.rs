use copper_geometry::TileShape;

use crate::Arena;
use crate::arena::{DoorId, TargetDoorId};
use crate::autoroute::expansion::{ExpansionDoor, FreeSpaceExpansionRoom, RoomRef};

#[derive(Debug, Clone, PartialEq)]
pub struct IncompleteFreeSpaceExpansionRoom {
    pub base: FreeSpaceExpansionRoom,
    contained_shape: Option<TileShape>,
    id_no: i32,
}

impl IncompleteFreeSpaceExpansionRoom {
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

    pub fn set_id_no(&mut self, id_no: i32) {
        self.id_no = id_no;
    }

    pub fn get_contained_shape(&self) -> Option<&TileShape> {
        self.contained_shape.as_ref()
    }

    pub fn set_contained_shape(&mut self, shape: Option<TileShape>) {
        self.contained_shape = shape;
    }

    pub fn get_target_doors(&self) -> &[TargetDoorId] {
        &[]
    }

    pub fn get_id(&self) -> i32 {
        self.id_no
    }

    pub fn id(&self) -> i32 {
        let shape = self.base.get_shape().unwrap_or_else(|| {
            panic!(
                "IncompleteFreeSpaceExpansionRoom.getId: the room has no shape — Java NPEs here \
                 too (IncompleteFreeSpaceExpansionRoom.java:40)"
            )
        });
        shape
            .get_id()
            .wrapping_mul(31)
            .wrapping_add(self.base.get_layer() as i32)
    }

    pub fn get_shape(&self) -> Option<&TileShape> {
        self.base.get_shape()
    }

    pub fn set_shape(&mut self, shape: Option<TileShape>) {
        self.base.set_shape(shape);
    }

    pub fn get_layer(&self) -> usize {
        self.base.get_layer()
    }

    pub fn add_door(&mut self, door: DoorId) {
        self.base.add_door(door);
    }

    pub fn get_doors(&self) -> &[DoorId] {
        self.base.get_doors()
    }

    pub fn clear_doors(&mut self) {
        self.base.clear_doors();
    }

    pub fn remove_door(&mut self, door: DoorId) -> bool {
        self.base.remove_door(door)
    }

    pub fn reset_doors(&self, doors: &mut Arena<ExpansionDoor>) {
        self.base.reset_doors(doors);
    }

    pub fn door_exists(&self, doors: &Arena<ExpansionDoor>, other: RoomRef) -> bool {
        self.base.door_exists(doors, other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use copper_geometry::IntBox;

    fn boxed(llx: i32, lly: i32, urx: i32, ury: i32) -> TileShape {
        TileShape::Box(IntBox::from_coords(llx, lly, urx, ury))
    }

    #[test]
    fn the_id_is_the_engine_counter_and_javas_is_the_shape_hash() {
        let shape = boxed(1, 2, 3, 4);
        let mut room = IncompleteFreeSpaceExpansionRoom::new(Some(shape.clone()), 5, None);
        assert_eq!(room.id(), shape.get_id().wrapping_mul(31).wrapping_add(5));
        assert_eq!(room.get_id(), 0, "a candidate that never reached the arena");
        room.set_id_no(7);
        assert_eq!(room.get_id(), 7);
    }

    #[test]
    fn the_id_no_longer_moves_when_the_shape_is_replaced() {
        let mut room = IncompleteFreeSpaceExpansionRoom::new(Some(boxed(0, 0, 1, 1)), 0, None);
        room.set_id_no(3);
        let before = room.get_id();
        let id_before = room.id();
        room.set_shape(Some(boxed(0, 0, 2, 2)));
        assert_eq!(room.get_id(), before);
        assert_ne!(room.id(), id_before, "Java's does move — the defect");
    }

    #[test]
    fn the_whole_plane_room_has_an_id() {
        let mut room = IncompleteFreeSpaceExpansionRoom::new(None, 0, Some(boxed(0, 0, 1, 1)));
        room.set_id_no(11);
        assert_eq!(room.get_id(), 11);
    }

    #[test]
    #[should_panic(expected = "the room has no shape")]
    fn javas_id_of_a_whole_plane_room_still_npes() {
        IncompleteFreeSpaceExpansionRoom::new(None, 0, Some(boxed(0, 0, 1, 1))).id();
    }

    #[test]
    fn target_doors_are_always_empty() {
        let room = IncompleteFreeSpaceExpansionRoom::new(Some(boxed(0, 0, 1, 1)), 0, None);
        assert!(room.get_target_doors().is_empty());
    }
}
