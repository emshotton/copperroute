//! Port of `autoroute.drill.ExpansionDrill` (ExpansionDrill.java:17-139) — "layer change
//! expansion object in the maze search algorithm".

use fr_board::{Board, TreeObject};
use fr_geometry::{Point, TileShape};

use crate::autoroute::expansion::RoomRef;
use crate::autoroute::maze::AutorouteEngine;
use crate::autoroute::maze::engine::tree_of;
use crate::autoroute::maze::search_element::MazeSearchElement;

/// Port of `ExpansionDrill` (ExpansionDrill.java:17-139), one of the four `ExpandableObject`
/// implementors.
///
/// # Identity and ordering
///
/// [`get_id`](Self::get_id) is the fifth member of the `getId()` family
/// ([`crate::autoroute::expansion`]'s table): `31 * (31 * location.getId() + firstLayer) +
/// lastLayer` (`:127-130`), a hash that wraps. It reaches `MazeListElement.compareTo` through
/// `ExpandableObject.getId()` (plan-6 ruling 4), so it may not be "cleaned up" before parity.
///
/// Unlike [`super::DrillPage`]'s, this id is **stable**: every input is `final`.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpansionDrill {
    /// `public final Point location` (`:20`): "the location, where the drill is checked".
    pub location: Point,
    /// `public final int firstLayer` (`:23`).
    pub first_layer: usize,
    /// `public final int lastLayer` (`:26`).
    pub last_layer: usize,
    /// `public final CompleteExpansionRoom[] roomArr` (`:29`): "array of dimension
    /// `lastLayer - firstLayer + 1`", one room per layer.
    ///
    /// `None` is Java's `null` slot, which is what every slot holds until
    /// [`calculate_expansion_rooms`](Self::calculate_expansion_rooms) fills it — and what the
    /// slots from the failing layer onwards keep when that method answers `false`.
    ///
    /// Named `rooms` rather than `room_arr` per the crate's naming convention; the Java field is
    /// on this line.
    // renamed: ExpansionDrill.roomArr -> rooms
    pub rooms: Vec<Option<RoomRef>>,
    /// `private final MazeSearchElement[] mazeSearchElements` (`:31`), one per layer.
    maze_search_elements: Vec<MazeSearchElement>,
    /// `private final TileShape shape` (`:34`): "the shape of the drill".
    ///
    /// Private, as Java's is — the task brief lists it as a public field, but
    /// `ExpansionDrill.shape` is `private` at `:34` and read through `getShape()` (`:94-97`).
    /// (`DrillPage.shape`, by contrast, really is public.)
    shape: TileShape,
}

impl ExpansionDrill {
    /// Port of `ExpansionDrill(TileShape, Point, int, int)` (ExpansionDrill.java:36-48).
    ///
    /// Both arrays are `lastLayer - firstLayer + 1` long (`:42-47`), so a single-layer drill has
    /// one room slot and one `MazeSearchElement`.
    ///
    /// # Panics
    /// If `last_layer < first_layer`. Java would compute a negative `layerCount` and throw
    /// `NegativeArraySizeException` at `:43`; no caller can produce one
    /// (`DrillPage.getDrills:106-107` passes `0` and `layerCount - 1`, and
    /// `Via.getAutorouteDrillInfo` the via's own layer range).
    pub fn new(
        shape: TileShape,
        location: Point,
        first_layer: usize,
        last_layer: usize,
    ) -> ExpansionDrill {
        assert!(
            last_layer >= first_layer,
            "ExpansionDrill: lastLayer {last_layer} is below firstLayer {first_layer} — Java \
             throws NegativeArraySizeException at ExpansionDrill.java:43"
        );
        // :42.
        let layer_count = last_layer - first_layer + 1;
        ExpansionDrill {
            shape,
            location,
            first_layer,
            last_layer,
            // :43.
            rooms: vec![None; layer_count],
            // :44-47.
            maze_search_elements: vec![MazeSearchElement::default(); layer_count],
        }
    }

    /// Port of `calculateExpansionRooms(AutorouteEngine)` (ExpansionDrill.java:55-92): "looks for
    /// the expansion room of this drill on each layer. Creates a `CompleteFreeSpaceExpansionRoom`
    /// if no expansion room is found. Returns false if that was not possible because of an
    /// obstacle at location on some layer in the compensated search tree."
    ///
    /// # The mixed room/item set, and its cross-layer erasure
    ///
    /// `:57-58` queries `overlappingObjects`, whose `TreeSet<SearchTreeObject>` holds **both**
    /// board items and complete expansion rooms — one of the three live mixed-set sites Task 2's
    /// ordering test covers. The `Iterator.remove` calls at `:65` and `:70` mutate that set, and
    /// the removals **persist across the layer loop**: every item is erased on the first layer's
    /// scan and each room is erased by the layer that claims it, so a later layer never
    /// re-examines either. The port keeps the set's order in a `Vec` and removes by index, which
    /// is the same traversal.
    ///
    /// # `false` means "not exactly one room", not only "blocked"
    ///
    /// `:80-83` rejects `newRooms.size() != 1`, and the comment there names only the zero case.
    /// More than one is just as common — a location where `completeShape` splits the free space —
    /// and `crates/fr-router/tests/drill.rs`'s
    /// `calculate_expansion_rooms_reuses_the_rooms_that_are_already_in_the_tree` pins both arms
    /// off the JVM.
    ///
    /// The `completeExpansionRoom` call at `:79` is consumed with `.unwrap_or_default()`, which
    /// is the obligation on `AutorouteEngine::complete_expansion_room`: its `Err` **is** Java's
    /// empty collection (`docs/java-quirks.md` #166), and `?` would abort a connection Java
    /// completes.
    pub fn calculate_expansion_rooms(
        &mut self,
        engine: &mut AutorouteEngine,
        board: &mut Board,
    ) -> bool {
        // :56.
        let search_shape = TileShape::Box(TileShape::get_instance_from_point(&self.location));

        // :57-58. The three-argument `overlappingObjects(shape, layer)` with `layer < 0` is the
        // "ignore the layer" query; the room-aware twin is needed because the autoroute tree
        // holds `TreeObject::Room` leaves by the time a drill is built.
        let mut overlaps: Vec<TreeObject> = {
            let ctx = board.ctx();
            tree_of(board, engine.tree)
                .overlapping_objects_with_rooms(
                    &search_shape,
                    None,
                    &[],
                    &board.items,
                    &engine.rooms,
                    &ctx,
                )
                .into_iter()
                .collect()
        };

        // :59.
        for layer in self.first_layer..=self.last_layer {
            // :60-73.
            let mut found_room: Option<RoomRef> = None;
            let mut index = 0;
            while index < overlaps.len() {
                // :63-67. `!(currentObject instanceof CompleteExpansionRoom)` is `it.remove()`
                // and `continue`: only a `CompleteFreeSpaceExpansionRoom` ever reaches the
                // autoroute tree as a room, so this drops every board item from the set for
                // good.
                let TreeObject::Room(room) = overlaps[index] else {
                    overlaps.remove(index);
                    continue;
                };
                // :68-72. Java NPEs on a room the arena no longer holds; `None` never matches a
                // layer, so a stale id is skipped rather than claimed.
                if engine.rooms.complete_room(room).map(|r| r.get_layer()) == Some(layer) {
                    found_room = Some(RoomRef::Complete(room));
                    overlaps.remove(index);
                    break;
                }
                index += 1;
            }

            let found_room = match found_room {
                Some(room) => room,
                None => {
                    // :74-77. "Create a new expansion room on this layer." Java calls the
                    // **constructor**, not `addIncompleteExpansionRoom`, so the room never joins
                    // `incompleteExpansionRooms` — and on an engine whose list is still null that
                    // is what makes `:79` throw (quirk #169).
                    //
                    // fixed: T6 (#169) — the second half of the register's suggested fix: build
                    // the room with `addIncompleteExpansionRoom` (`new_incomplete_room`) rather
                    // than the bare constructor, "so the room is in the database it is about to be
                    // completed out of". The first half is the field guard in
                    // `ExpansionRoomStore::remove_incomplete_expansion_room`; both are needed,
                    // because the guard alone would leave `ExpansionDrill` completing a room the
                    // engine has never heard of, and this alone would leave every other bare
                    // constructor (there are none today) able to reinstate the crash.
                    let new_incomplete_room =
                        engine
                            .rooms
                            .new_incomplete_room(None, layer, Some(search_shape.clone()));
                    // :78-79.
                    let new_rooms = engine
                        .complete_expansion_room(board, new_incomplete_room)
                        .unwrap_or_default();
                    // :80-83. "The size may be 0 because of an obstacle in the compensated tree
                    // at this.location."
                    if new_rooms.len() != 1 {
                        return false;
                    }
                    // :84-87.
                    RoomRef::Complete(new_rooms[0])
                }
            };

            // :89.
            self.rooms[layer - self.first_layer] = Some(found_room);
        }
        // :91.
        true
    }

    /// Port of `getShape()` (ExpansionDrill.java:94-97).
    pub fn get_shape(&self) -> &TileShape {
        &self.shape
    }

    /// Port of `getDimension()` (ExpansionDrill.java:99-102): the constant 2.
    pub fn get_dimension(&self) -> i32 {
        2
    }

    /// Port of `otherRoom(CompleteExpansionRoom)` (ExpansionDrill.java:104-107): the constant
    /// `null`. A drill is not a door between two rooms — it binds one room per layer — so the
    /// `ExpandableObject` method has no answer.
    pub fn other_room(&self, _room: RoomRef) -> Option<RoomRef> {
        None
    }

    /// Port of `mazeSearchElementCount()` (ExpansionDrill.java:109-112).
    pub fn maze_search_element_count(&self) -> usize {
        self.maze_search_elements.len()
    }

    /// Port of `getMazeSearchElement(int)` (ExpansionDrill.java:114-117).
    ///
    /// # Panics
    /// On an out-of-range index, which is Java's `ArrayIndexOutOfBoundsException` at `:116`.
    pub fn get_maze_search_element(&self, index: usize) -> &MazeSearchElement {
        &self.maze_search_elements[index]
    }

    /// [`get_maze_search_element`](Self::get_maze_search_element), mutably — Java hands back the
    /// array element itself and the maze search writes through it.
    pub fn get_maze_search_element_mut(&mut self, index: usize) -> &mut MazeSearchElement {
        &mut self.maze_search_elements[index]
    }

    /// Port of `reset()` (ExpansionDrill.java:119-124): resets every `MazeSearchElement`. The
    /// bound rooms are **not** cleared — a reset drill keeps its layer bindings.
    pub fn reset(&mut self) {
        for element in &mut self.maze_search_elements {
            element.reset();
        }
    }

    /// Port of `getId()` (ExpansionDrill.java:126-130):
    /// `31 * (31 * location.getId() + firstLayer) + lastLayer`.
    ///
    /// A Java `int` hash, so every step wraps (quirk #8's family). The layers are `usize` here
    /// and Java's are `int`; a board with more than `i32::MAX` layers is not representable, so
    /// the casts are lossless.
    pub fn get_id(&self) -> i32 {
        let inner = 31i32
            .wrapping_mul(self.location.get_id())
            .wrapping_add(self.first_layer as i32);
        31i32
            .wrapping_mul(inner)
            .wrapping_add(self.last_layer as i32)
    }
}

// =================================================================================================
// The deferral roster for `autoroute/drill/ExpansionDrill.java`
// =================================================================================================

// not ported: `ExpansionDrill.emitDiagnostic` — it feeds `AutorouteDiagnostic.Sink`, a GUI overlay
// (`global-constraints.md`: no GUI, no observers), and no routing decision reads it.

#[cfg(test)]
mod tests {
    use super::*;
    use fr_geometry::IntBox;

    #[test]
    fn the_id_is_javas_hash_of_the_location_and_the_two_layers() {
        // `P6T7Probe` mode 5: `free getId=24995611` for location (835, 125) over layers 0..1,
        // `blocked getId=1` for (0, 0) over 0..1 and `layer0 getId=0` for (0, 0) over 0..0.
        let shape = TileShape::Box(IntBox::from_coords(0, 0, 1, 1));
        let drill = ExpansionDrill::new(shape.clone(), Point::new(835, 125), 0, 1);
        assert_eq!(drill.get_id(), 24_995_611);
        assert_eq!(
            ExpansionDrill::new(shape.clone(), Point::new(0, 0), 0, 1).get_id(),
            1
        );
        assert_eq!(
            ExpansionDrill::new(shape, Point::new(0, 0), 0, 0).get_id(),
            0
        );
    }

    #[test]
    fn the_arrays_are_one_slot_per_layer() {
        let shape = TileShape::Box(IntBox::from_coords(0, 0, 1, 1));
        let drill = ExpansionDrill::new(shape.clone(), Point::new(0, 0), 0, 3);
        assert_eq!(drill.rooms.len(), 4);
        assert_eq!(drill.maze_search_element_count(), 4);
        assert!(drill.rooms.iter().all(Option::is_none));

        let single = ExpansionDrill::new(shape, Point::new(0, 0), 2, 2);
        assert_eq!(single.rooms.len(), 1);
        assert_eq!(single.maze_search_element_count(), 1);
        assert_eq!(single.get_dimension(), 2);
    }
}
