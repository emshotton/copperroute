use crate::autoroute::expansion::ExpandableRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum MazeAdjustment {
        #[default]
    None,
        Right,
        Left,
}

impl MazeAdjustment {
            pub const ALL: [MazeAdjustment; 3] = [
        MazeAdjustment::None,
        MazeAdjustment::Right,
        MazeAdjustment::Left,
    ];
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MazeSearchElement {
            pub is_occupied: bool,
            pub backtrack_door: Option<ExpandableRef>,
        pub section_no_of_backtrack_door: i32,
        pub room_ripped: bool,
        pub adjustment: MazeAdjustment,
            pub ripup_cost: i32,
}

impl MazeSearchElement {
        pub fn new() -> MazeSearchElement {
        MazeSearchElement::default()
    }

                            pub fn reset(&mut self) {
        self.is_occupied = false; 
        self.backtrack_door = None; 
        self.section_no_of_backtrack_door = 0; 
        self.room_ripped = false; 
        self.adjustment = MazeAdjustment::None; 
        self.ripup_cost = 0; 
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::DoorId;
    use crate::autoroute::expansion::ExpandableRef;

    #[test]
    fn reset_clears_every_one_of_the_six_fields() {
        let mut e = MazeSearchElement::new();
        e.is_occupied = true;
        e.backtrack_door = Some(ExpandableRef::Door(DoorId(3)));
        e.section_no_of_backtrack_door = 9;
        e.room_ripped = true;
        e.adjustment = MazeAdjustment::Left;
        e.ripup_cost = 1234;
        e.reset();
        assert_eq!(e, MazeSearchElement::default());
    }

    #[test]
    fn the_adjustment_default_is_none_like_the_field_initialiser() {
        assert_eq!(MazeAdjustment::default(), MazeAdjustment::None);
        assert_eq!(MazeAdjustment::ALL[0], MazeAdjustment::None);
        assert_eq!(MazeAdjustment::ALL[1], MazeAdjustment::Right);
        assert_eq!(MazeAdjustment::ALL[2], MazeAdjustment::Left);
    }
}
