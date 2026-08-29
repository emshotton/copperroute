//! Port of `autoroute.expansion.ExpansionDoor` (ExpansionDoor.java:11-201) — "an ExpansionDoor
//! is a common edge between two ExpansionRooms".

use fr_geometry::{FloatLine, Point, TileShape};

use crate::autoroute::expansion::RoomRef;
use crate::autoroute::maze::{MazeSearchElement, TRACE_WIDTH_TOLERANCE};

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

    /// Port of `getSectionSegments(double)` (ExpansionDoor.java:104-143): "calculates the line
    /// segments of the sections of this door", and **allocates the section array** on the way
    /// (`:141`) — it is `allocateSections`' only caller.
    ///
    /// The two room shapes are resolved by the caller, because a room is reached through the
    /// arena here; [`super::ExpansionRoomStore::door_section_segments`] is the convenience form.
    ///
    /// The three branches, in Java's order:
    ///
    /// 1. **dimension 1** (`:115-117`) — the door is an edge, so the segment is the door shape's
    ///    diagonal corner segment.
    /// 2. **dimension 2 between two `CompleteFreeSpaceExpansionRoom`s** (`:118-132`) — the
    ///    overlapping-corner case of 90- and 45-degree routing. The restraint line is computed by
    ///    [`calc_door_line_segment`](Self::calc_door_line_segment), and the door is dropped
    ///    entirely if there is none (`:124-127`, "CompleteFreeSpaceExpansionRoom inside other
    ///    room") or if it is shorter than `2 * offset` (`:128-131`, "2 dimensional small doors
    ///    are not yet expanded"). The `instanceof` is on
    ///    `CompleteFreeSpaceExpansionRoom` specifically, so an `ObstacleExpansionRoom` — the
    ///    other `CompleteExpansionRoom` — falls to branch 3.
    /// 3. **everything else** (`:133-137`) — a degenerate segment at the door's centre of
    ///    gravity, which divides into exactly one zero-length section.
    ///
    /// `offset_param` is the caller's trace half-width; `TRACE_WIDTH_TOLERANCE` is added to it
    /// at `:106`.
    ///
    /// # Panics
    /// If the computed section count is negative — `new MazeSearchElement[sectionCount]` throws
    /// `NegativeArraySizeException` there (`:197`). It needs a non-positive `offset`, which no
    /// caller produces: all four pass a trace half-width
    /// (`MazeSearchEngine.java:715`, `MazeTraceShover.java:254,294`,
    /// `FoundConnectionLocator45Degree.java:244`).
    pub fn get_section_segments(
        &mut self,
        first_shape: &TileShape,
        second_shape: &TileShape,
        offset_param: f64,
    ) -> Vec<FloatLine> {
        // :106
        let offset = offset_param + f64::from(TRACE_WIDTH_TOLERANCE);
        // :107
        let door_shape = self.get_shape(first_shape, second_shape);
        // :108-112
        if door_shape.is_empty() {
            return Vec::new();
        }

        let door_line_segment: FloatLine;
        let shrinked_line_segment: FloatLine;
        if self.dimension == 1 {
            // :115-117. `diagonalCornerSegment` returns null only for an empty shape
            // (TileShape.java:469-471), which `:109` has already returned for — so Java's
            // unguarded dereference at `:117` is unreachable, and so is this `expect`.
            door_line_segment = door_shape.diagonal_corner_segment().expect(
                "TileShape.diagonalCornerSegment is null only when isEmpty, tested at :109",
            );
            shrinked_line_segment = door_line_segment.shrink_segment(offset);
        } else if self.dimension == 2
            && matches!(self.first_room, RoomRef::Complete(_))
            && matches!(self.second_room, RoomRef::Complete(_))
        {
            // :118-132
            let Some(segment) = self.calc_door_line_segment(&door_shape, first_shape, second_shape)
            else {
                // :124-127 — CompleteFreeSpaceExpansionRoom inside the other room.
                return Vec::new();
            };
            // :128-131 — the door is small; 2-dimensional small doors are not yet expanded.
            if segment.b.distance_square(&segment.a) < 4.0 * offset * offset {
                return Vec::new();
            }
            door_line_segment = segment;
            shrinked_line_segment = door_line_segment.shrink_segment(offset); // :132
        } else {
            // :133-137
            let gravity_point = door_shape.centre_of_gravity();
            door_line_segment = FloatLine::new(gravity_point, gravity_point);
            shrinked_line_segment = door_line_segment;
        }

        // :138-140. Java's `(int)` cast truncates toward zero and saturates on an infinity,
        // which is exactly what Rust's `as i32` does; the `+ 1` is `wrapping_add` because Java's
        // `int` addition wraps rather than panicking in a debug build.
        let max_door_section_width = 10.0 * offset;
        let section_count = ((door_line_segment.b.distance(&door_line_segment.a)
            / max_door_section_width) as i32)
            .wrapping_add(1);

        // :141
        self.allocate_sections(usize::try_from(section_count).unwrap_or_else(|_| {
            panic!(
                "ExpansionDoor.getSectionSegments: section count {section_count} is negative — \
                 Java throws NegativeArraySizeException at ExpansionDoor.java:197"
            )
        }));
        // :142
        shrinked_line_segment.divide_segment_into_sections(section_count)
    }

    /// Port of the private `calcDoorLineSegment(TileShape)` (ExpansionDoor.java:145-172):
    /// "calculates a diagonal line of the 2-dimensional doorShape which represents the restraint
    /// line between the shapes of this.firstRoom and this.secondRoom."
    ///
    /// It walks the door shape's corners and keeps the first two **distinct** ones that lie on
    /// the border of *both* rooms — i.e. inside neither (`:157-158`) — then stops (`:164`).
    /// `None` is Java's `null` for fewer than two such corners (`:168-170`).
    ///
    /// Java's loop bound is named `cornerCount` but reads `borderLineCount()` (`:154`); for a
    /// convex tile shape the two are equal, so the name is the only thing wrong with it.
    fn calc_door_line_segment(
        &self,
        door_shape: &TileShape,
        first_room_shape: &TileShape,
        second_room_shape: &TileShape,
    ) -> Option<FloatLine> {
        let mut first_corner: Option<Point> = None;
        let mut second_corner: Option<Point> = None;
        let corner_count = door_shape.border_line_count(); // :154
        for i in 0..corner_count {
            let current_corner = door_shape.corner(i); // :156
            // :157-159 — on the border of both room shapes.
            if first_room_shape.contains_inside(&current_corner)
                || second_room_shape.contains_inside(&current_corner)
            {
                continue;
            }
            match &first_corner {
                // :160-161
                None => first_corner = Some(current_corner),
                // :162-165 — a *distinct* second corner ends the walk.
                Some(first) if *first != current_corner => {
                    second_corner = Some(current_corner);
                    break;
                }
                // :162's `else if` is false: a repeat of the first corner is ignored.
                Some(_) => {}
            }
        }
        // :168-171
        Some(FloatLine::new(
            first_corner?.to_float(),
            second_corner?.to_float(),
        ))
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
