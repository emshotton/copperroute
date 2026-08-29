//! Port of `autoroute.expansion.ExpansionDoor` (ExpansionDoor.java:11-201) — "an ExpansionDoor
//! is a common edge between two ExpansionRooms".

use fr_geometry::TileShape;

use crate::autoroute::expansion::RoomRef;
use crate::autoroute::maze::MazeSearchElement;

/// Port of `ExpansionDoor` (ExpansionDoor.java:11-201).
///
/// # Java-vs-brief
///
/// The task brief lists a `precalculated_shape: Option<TileShape>` memo at "`:30`". HEAD has no
/// such field: `:30` is inside the three-argument constructor, and `getShape()` (`:42-47`)
/// recomputes `firstRoom.getShape().intersection(secondRoom.getShape())` on **every** call.
/// Java wins, so there is no memo here either — adding one would change how often the
/// intersection is recomputed, and the shape moves whenever either room's shape is replaced
/// (`FreeSpaceExpansionRoom.setShape`, hazard C).
#[derive(Debug, Clone, PartialEq)]
pub struct ExpansionDoor {
    /// `public final ExpansionRoom firstRoom` (:14).
    pub first_room: RoomRef,
    /// `public final ExpansionRoom secondRoom` (:17).
    pub second_room: RoomRef,
    /// `public final int dimension` (:20): "the dimension of a door may be 1 or 2.
    /// 2-dimensional doors can only exist between ObstacleExpansionRooms" (`:50-52`).
    pub dimension: i32,
    /// `public MazeSearchElement[] sectionArr` (:25): "each section of the following array can
    /// be expanded separately by the maze search algorithm."
    ///
    /// **`None` is Java's `null`, and the distinction is observable.** The field has no
    /// initialiser and only [`allocate_sections`](Self::allocate_sections) — reached from
    /// `getSectionSegments` (`:141`) — ever fills it, so a door that the maze search has not yet
    /// sectioned has a null array: `reset()` guards it (`:177`) and
    /// `mazeSearchElementCount()` (`:95-97`) does not, i.e. it throws. A `Vec` would conflate
    /// "not allocated" with "allocated to length 0" and turn that throw into a `0`.
    sections: Option<Vec<MazeSearchElement>>,
}

impl ExpansionDoor {
    /// Port of the three-argument constructor (ExpansionDoor.java:28-32).
    pub fn new(first_room: RoomRef, second_room: RoomRef, dimension: i32) -> ExpansionDoor {
        ExpansionDoor {
            first_room,
            second_room,
            dimension,
            sections: None,
        }
    }

    /// Port of the two-argument constructor (ExpansionDoor.java:35-39), whose `dimension` is
    /// `firstRoom.getShape().intersection(secondRoom.getShape()).dimension()`.
    ///
    /// The caller resolves the two shapes, because a room is reached through the arena here.
    /// Java would NPE on a null room shape; the port's caller has the same choice, and
    /// [`super::ExpansionRoomStore::new_door_from_shapes`] makes it explicit.
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

    /// Port of `getShape` (ExpansionDoor.java:41-47): "calculates the intersection of the shapes
    /// of the 2 rooms belonging to this door" — recomputed each call, see the type doc.
    ///
    /// The caller resolves the two room shapes out of the arenas;
    /// [`super::ExpansionRoomStore::door_shape`] is the convenience form.
    pub fn get_shape(&self, first_shape: &TileShape, second_shape: &TileShape) -> TileShape {
        first_shape.intersection(second_shape)
    }

    /// Port of `getDimension` (ExpansionDoor.java:53-56).
    pub fn get_dimension(&self) -> i32 {
        self.dimension
    }

    /// Port of `otherRoom(ExpansionRoom)` (ExpansionDoor.java:62-72): the other side, or `None`
    /// for a room that is neither ("null if room is neither equal to this.firstRoom nor to
    /// this.secondRoom").
    ///
    /// Java's `==` is reference identity, which is `RoomRef` equality here.
    pub fn other_room(&self, room: RoomRef) -> Option<RoomRef> {
        if room == self.first_room {
            Some(self.second_room)
        } else if room == self.second_room {
            Some(self.first_room)
        } else {
            None
        }
    }

    /// Port of the `otherRoom(CompleteExpansionRoom)` overload (ExpansionDoor.java:78-92) — the
    /// `ExpandableObject` interface method. Same walk as [`other_room`](Self::other_room), and
    /// then `null` again if the answer is not itself a `CompleteExpansionRoom` (`:88-90`), i.e.
    /// if it is an incomplete free-space room.
    ///
    /// `AutorouteEngine.completeNeighbourRooms` (:567-592) casts its argument back to
    /// `ExpansionRoom` precisely to avoid this narrowing, and says so in a comment at `:577-578`.
    pub fn other_complete_room(&self, room: RoomRef) -> Option<RoomRef> {
        self.other_room(room).filter(|other| other.is_complete())
    }

    /// Port of `mazeSearchElementCount` (ExpansionDoor.java:94-97): `sectionArr.length`.
    ///
    /// `None` is the unallocated array Java throws a `NullPointerException` on. Returning an
    /// `Option` rather than panicking is the one place this port declines to reproduce a crash:
    /// the caller (`MazeSearchEngine`, Task 11) has the guard Java lacks, and a `0` would be
    /// wrong.
    pub fn maze_search_element_count(&self) -> Option<usize> {
        self.sections.as_ref().map(|s| s.len())
    }

    /// Port of `getMazeSearchElement(int)` (ExpansionDoor.java:99-102): `sectionArr[index]`.
    /// `None` is Java's `NullPointerException` (unallocated) or
    /// `ArrayIndexOutOfBoundsException` (past the end).
    pub fn get_maze_search_element(&self, index: usize) -> Option<&MazeSearchElement> {
        self.sections.as_ref()?.get(index)
    }

    /// [`get_maze_search_element`](Self::get_maze_search_element), mutably — Java's callers hold
    /// the element and write its public fields.
    pub fn get_maze_search_element_mut(&mut self, index: usize) -> Option<&mut MazeSearchElement> {
        self.sections.as_mut()?.get_mut(index)
    }

    /// Port of `reset` (ExpansionDoor.java:174-182): "resets this ExpandableObject for
    /// autorouting the next connection." Java's `sectionArr != null` guard (`:177`) is the
    /// `Option`.
    pub fn reset(&mut self) {
        if let Some(sections) = self.sections.as_mut() {
            for section in sections.iter_mut() {
                section.reset();
            }
        }
    }

    /// Port of `getId` (ExpansionDoor.java:184-190):
    /// `Math.min(id1, id2) * 31 + Math.max(id1, id2)` over the two rooms' `getId()`s — "a stable
    /// combination of room IDs. Note: min/max ensures order-independence."
    ///
    /// A hash, not an identity: two different pairs of rooms can produce it, and the `int`
    /// arithmetic wraps. The caller resolves the two room ids, because they come from three
    /// different `getId()` implementations (see [`super::ExpansionRoomStore::room_id_no`]).
    pub fn get_id(&self, first_id: i32, second_id: i32) -> i32 {
        ExpansionDoor::id(first_id, second_id)
    }

    /// [`get_id`](Self::get_id)'s arithmetic over the two room ids.
    pub fn id(first_id: i32, second_id: i32) -> i32 {
        first_id
            .min(second_id)
            .wrapping_mul(31)
            .wrapping_add(first_id.max(second_id))
    }

    /// Port of `allocateSections(int)` (ExpansionDoor.java:192-201), which is package-private in
    /// Java and reached only from `getSectionSegments` (`:141`): "allocates and initialises
    /// sectionCount sections", and returns early when the array is already that length
    /// (`:194-195`) — so an existing section's maze state survives a re-section of the same
    /// width, and is thrown away by one of a different width.
    pub fn allocate_sections(&mut self, section_count: usize) {
        if self
            .sections
            .as_ref()
            .is_some_and(|s| s.len() == section_count)
        {
            return; // :194-195, "already allocated"
        }
        self.sections = Some(
            std::iter::repeat_with(MazeSearchElement::new)
                .take(section_count)
                .collect(),
        );
    }
}

// added in Task 12: `ExpansionDoor.getSectionSegments`
// (ExpansionDoor.java:104-143, with its private helper `calcDoorLineSegment` at :145-172) — it
// needs `AutorouteEngine.TRACE_WIDTH_TOLERANCE` (AutorouteEngine.java:40) and is reached only
// from the room-door expansion of `MazeSearchEngine`, which is Task 12's. It is the sole caller
// of `allocateSections` above, which is why that one is public here.

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
        d.reset(); // ExpansionDoor.java:177 — the null guard.
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
