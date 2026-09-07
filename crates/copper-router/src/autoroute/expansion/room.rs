use copper_board::{ObstacleRoomId, RoomId};

use crate::arena::{DoorId, DrillId, IncompleteRoomId, PageId, TargetDoorId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RoomRef {
    Complete(RoomId),
    Obstacle(ObstacleRoomId),
    Incomplete(IncompleteRoomId),
}

impl RoomRef {
    pub fn is_complete(self) -> bool {
        matches!(self, RoomRef::Complete(_) | RoomRef::Obstacle(_))
    }

    pub fn is_free_space(self) -> bool {
        matches!(self, RoomRef::Complete(_) | RoomRef::Incomplete(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExpandableRef {
    Door(DoorId),
    TargetDoor(TargetDoorId),
    Drill(DrillId),
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
        assert_ne!(RoomRef::Complete(RoomId(1)), RoomRef::Complete(RoomId(2)));
        assert_ne!(
            RoomRef::Complete(RoomId(1)),
            RoomRef::Incomplete(IncompleteRoomId(1))
        );
    }
}
