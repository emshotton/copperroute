use copper_geometry::TileShape;

use crate::Arena;
use crate::arena::DoorId;
use crate::autoroute::expansion::{ExpansionDoor, RoomRef};

#[derive(Debug, Clone, PartialEq)]
pub struct FreeSpaceExpansionRoom {
    shape: Option<TileShape>,
    layer: usize,
    doors: Vec<DoorId>,
}

impl FreeSpaceExpansionRoom {
    pub fn new(shape: Option<TileShape>, layer: usize) -> FreeSpaceExpansionRoom {
        FreeSpaceExpansionRoom {
            shape,
            layer,
            doors: Vec::new(),
        }
    }

    pub fn add_door(&mut self, door: DoorId) {
        self.doors.push(door);
    }

    pub fn get_doors(&self) -> &[DoorId] {
        &self.doors
    }

    pub fn clear_doors(&mut self) {
        self.doors = Vec::new();
    }

    pub fn remove_door(&mut self, door: DoorId) -> bool {
        match self.doors.iter().position(|d| *d == door) {
            Some(index) => {
                self.doors.remove(index);
                true
            }
            None => false,
        }
    }

    pub fn reset_doors(&self, doors: &mut Arena<ExpansionDoor>) {
        for door in &self.doors {
            if let Some(door) = doors.get_mut(door.0) {
                door.reset();
            }
        }
    }

    pub fn door_exists(&self, doors: &Arena<ExpansionDoor>, other: RoomRef) -> bool {
        self.doors.iter().any(|door| {
            doors
                .get(door.0)
                .is_some_and(|d| d.first_room == other || d.second_room == other)
        })
    }

    pub fn get_shape(&self) -> Option<&TileShape> {
        self.shape.as_ref()
    }

    pub fn set_shape(&mut self, shape: Option<TileShape>) {
        self.shape = shape;
    }

    pub fn get_layer(&self) -> usize {
        self.layer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use copper_board::RoomId;
    use copper_geometry::IntBox;

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
