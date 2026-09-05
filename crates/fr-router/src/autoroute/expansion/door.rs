use fr_geometry::{FloatLine, Point, TileShape};

use crate::autoroute::expansion::RoomRef;
use crate::autoroute::maze::{MazeSearchElement, TRACE_WIDTH_TOLERANCE};

#[derive(Debug, Clone, PartialEq)]
pub struct ExpansionDoor {
    pub first_room: RoomRef,
    pub second_room: RoomRef,
    pub dimension: i32,
    sections: Option<Vec<MazeSearchElement>>,
}

impl ExpansionDoor {
    pub fn new(first_room: RoomRef, second_room: RoomRef, dimension: i32) -> ExpansionDoor {
        ExpansionDoor {
            first_room,
            second_room,
            dimension,
            sections: None,
        }
    }

    pub fn new_with_computed_dimension(
        first_room: RoomRef,
        second_room: RoomRef,
        first_shape: &TileShape,
        second_shape: &TileShape,
    ) -> ExpansionDoor {
        ExpansionDoor::new(
            first_room,
            second_room,
            first_shape.intersection(second_shape).dimension(),
        )
    }

    pub fn get_shape(&self, first_shape: &TileShape, second_shape: &TileShape) -> TileShape {
        first_shape.intersection(second_shape)
    }

    pub fn get_dimension(&self) -> i32 {
        self.dimension
    }

    pub fn other_room(&self, room: RoomRef) -> Option<RoomRef> {
        if room == self.first_room {
            Some(self.second_room)
        } else if room == self.second_room {
            Some(self.first_room)
        } else {
            None
        }
    }

    pub fn other_complete_room(&self, room: RoomRef) -> Option<RoomRef> {
        self.other_room(room).filter(|other| other.is_complete())
    }

    pub fn maze_search_element_count(&self) -> Option<usize> {
        self.sections.as_ref().map(|s| s.len())
    }

    pub fn get_maze_search_element(&self, index: usize) -> Option<&MazeSearchElement> {
        self.sections.as_ref()?.get(index)
    }

    pub fn get_maze_search_element_mut(&mut self, index: usize) -> Option<&mut MazeSearchElement> {
        self.sections.as_mut()?.get_mut(index)
    }

    pub fn reset(&mut self) {
        if let Some(sections) = self.sections.as_mut() {
            for section in sections.iter_mut() {
                section.reset();
            }
        }
    }

    pub fn get_id(&self, first_id: i32, second_id: i32) -> i32 {
        ExpansionDoor::id(first_id, second_id)
    }

    pub fn id(first_id: i32, second_id: i32) -> i32 {
        first_id
            .min(second_id)
            .wrapping_mul(31)
            .wrapping_add(first_id.max(second_id))
    }

    pub fn get_section_segments(
        &mut self,
        first_shape: &TileShape,
        second_shape: &TileShape,
        offset_param: f64,
    ) -> Vec<FloatLine> {
        let offset = offset_param + f64::from(TRACE_WIDTH_TOLERANCE);
        let door_shape = self.get_shape(first_shape, second_shape);
        if door_shape.is_empty() {
            return Vec::new();
        }

        let door_line_segment: FloatLine;
        let shrinked_line_segment: FloatLine;
        if self.dimension == 1 {
            door_line_segment = door_shape.diagonal_corner_segment().expect(
                "TileShape.diagonalCornerSegment is null only when isEmpty, tested at :109",
            );
            shrinked_line_segment = door_line_segment.shrink_segment(offset);
        } else if self.dimension == 2
            && matches!(self.first_room, RoomRef::Complete(_))
            && matches!(self.second_room, RoomRef::Complete(_))
        {
            let Some(segment) = self.calc_door_line_segment(&door_shape, first_shape, second_shape)
            else {
                return Vec::new();
            };
            if segment.b.distance_square(&segment.a) < 4.0 * offset * offset {
                return Vec::new();
            }
            door_line_segment = segment;
            shrinked_line_segment = door_line_segment.shrink_segment(offset);
        } else {
            let gravity_point = door_shape.centre_of_gravity();
            door_line_segment = FloatLine::new(gravity_point, gravity_point);
            shrinked_line_segment = door_line_segment;
        }

        let max_door_section_width = 10.0 * offset;
        let section_count = ((door_line_segment.b.distance(&door_line_segment.a)
            / max_door_section_width) as i32)
            .wrapping_add(1);

        self.allocate_sections(usize::try_from(section_count).unwrap_or_else(|_| {
            panic!(
                "ExpansionDoor.getSectionSegments: section count {section_count} is negative — \
                 Java throws NegativeArraySizeException at ExpansionDoor.java:197"
            )
        }));
        shrinked_line_segment.divide_segment_into_sections(section_count)
    }

    fn calc_door_line_segment(
        &self,
        door_shape: &TileShape,
        first_room_shape: &TileShape,
        second_room_shape: &TileShape,
    ) -> Option<FloatLine> {
        let mut first_corner: Option<Point> = None;
        let mut second_corner: Option<Point> = None;
        let corner_count = door_shape.border_line_count();
        for i in 0..corner_count {
            let current_corner = door_shape.corner(i);
            if first_room_shape.contains_inside(&current_corner)
                || second_room_shape.contains_inside(&current_corner)
            {
                continue;
            }
            match &first_corner {
                None => first_corner = Some(current_corner),
                Some(first) if *first != current_corner => {
                    second_corner = Some(current_corner);
                    break;
                }
                Some(_) => {}
            }
        }
        Some(FloatLine::new(
            first_corner?.to_float(),
            second_corner?.to_float(),
        ))
    }

    pub fn allocate_sections(&mut self, section_count: usize) {
        if self
            .sections
            .as_ref()
            .is_some_and(|s| s.len() == section_count)
        {
            return;
        }
        self.sections = Some(
            std::iter::repeat_with(MazeSearchElement::new)
                .take(section_count)
                .collect(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::IncompleteRoomId;
    use crate::autoroute::maze::MazeAdjustment;
    use fr_board::RoomId;
    use fr_geometry::IntBox;

    fn boxed(llx: i32, lly: i32, urx: i32, ury: i32) -> TileShape {
        TileShape::Box(IntBox::from_coords(llx, lly, urx, ury))
    }

    fn door() -> ExpansionDoor {
        ExpansionDoor::new(
            RoomRef::Complete(RoomId(0)),
            RoomRef::Incomplete(IncompleteRoomId(0)),
            1,
        )
    }

    #[test]
    fn the_id_is_symmetric_and_wraps() {
        assert_eq!(ExpansionDoor::id(7, 11), ExpansionDoor::id(11, 7));
        assert_eq!(ExpansionDoor::id(7, 11), 7 * 31 + 11);
        assert_eq!(
            ExpansionDoor::id(i32::MAX, 1),
            1i32.wrapping_mul(31).wrapping_add(i32::MAX)
        );
        assert_eq!(
            ExpansionDoor::id(i32::MIN, i32::MIN),
            i32::MIN.wrapping_mul(31).wrapping_add(i32::MIN)
        );
    }

    #[test]
    fn other_room_answers_either_side_and_none_for_a_stranger() {
        let d = door();
        assert_eq!(
            d.other_room(RoomRef::Complete(RoomId(0))),
            Some(RoomRef::Incomplete(IncompleteRoomId(0)))
        );
        assert_eq!(
            d.other_room(RoomRef::Incomplete(IncompleteRoomId(0))),
            Some(RoomRef::Complete(RoomId(0)))
        );
        assert_eq!(d.other_room(RoomRef::Complete(RoomId(1))), None);
    }

    #[test]
    fn the_complete_overload_drops_an_incomplete_answer() {
        let d = door();
        assert_eq!(d.other_complete_room(RoomRef::Complete(RoomId(0))), None);
        assert_eq!(
            d.other_complete_room(RoomRef::Incomplete(IncompleteRoomId(0))),
            Some(RoomRef::Complete(RoomId(0)))
        );
    }

    #[test]
    fn an_unallocated_section_array_is_javas_null() {
        let mut d = door();
        assert_eq!(d.maze_search_element_count(), None);
        assert_eq!(d.get_maze_search_element(0), None);
        d.reset();
    }

    #[test]
    fn allocate_sections_keeps_an_array_of_the_same_length_and_replaces_another() {
        let mut d = door();
        d.allocate_sections(2);
        d.get_maze_search_element_mut(0).unwrap().is_occupied = true;
        d.allocate_sections(2);
        assert!(d.get_maze_search_element(0).unwrap().is_occupied);
        d.allocate_sections(3);
        assert_eq!(d.maze_search_element_count(), Some(3));
        assert!(!d.get_maze_search_element(0).unwrap().is_occupied);
    }

    #[test]
    fn reset_clears_every_section() {
        let mut d = door();
        d.allocate_sections(2);
        d.get_maze_search_element_mut(1).unwrap().adjustment = MazeAdjustment::Right;
        d.reset();
        assert_eq!(
            d.get_maze_search_element(1).unwrap().adjustment,
            MazeAdjustment::None
        );
    }

    #[test]
    fn the_two_argument_constructor_takes_the_intersections_dimension() {
        let d = ExpansionDoor::new_with_computed_dimension(
            RoomRef::Complete(RoomId(0)),
            RoomRef::Complete(RoomId(1)),
            &boxed(0, 0, 10, 10),
            &boxed(5, 5, 20, 20),
        );
        assert_eq!(d.get_dimension(), 2);
        let edge = ExpansionDoor::new_with_computed_dimension(
            RoomRef::Complete(RoomId(0)),
            RoomRef::Complete(RoomId(1)),
            &boxed(0, 0, 10, 10),
            &boxed(10, 0, 20, 10),
        );
        assert_eq!(edge.get_dimension(), 1);
    }
}
