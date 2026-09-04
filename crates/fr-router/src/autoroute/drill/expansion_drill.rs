use fr_board::{Board, TreeObject};
use fr_geometry::{Point, TileShape};

use crate::autoroute::expansion::RoomRef;
use crate::autoroute::maze::AutorouteEngine;
use crate::autoroute::maze::engine::tree_of;
use crate::autoroute::maze::search_element::MazeSearchElement;

#[derive(Debug, Clone, PartialEq)]
pub struct ExpansionDrill {
        pub location: Point,
        pub first_layer: usize,
        pub last_layer: usize,
                                        pub rooms: Vec<Option<RoomRef>>,
        maze_search_elements: Vec<MazeSearchElement>,
                        shape: TileShape,
}

impl ExpansionDrill {
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
        let layer_count = last_layer - first_layer + 1;
        ExpansionDrill {
            shape,
            location,
            first_layer,
            last_layer,
            rooms: vec![None; layer_count],
            maze_search_elements: vec![MazeSearchElement::default(); layer_count],
        }
    }

                                                                                                                pub fn calculate_expansion_rooms(
        &mut self,
        engine: &mut AutorouteEngine,
        board: &mut Board,
    ) -> bool {
        let search_shape = TileShape::Box(TileShape::get_instance_from_point(&self.location));

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

        for layer in self.first_layer..=self.last_layer {
            let mut found_room: Option<RoomRef> = None;
            let mut index = 0;
            while index < overlaps.len() {
                let TreeObject::Room(room) = overlaps[index] else {
                    overlaps.remove(index);
                    continue;
                };
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
                    let new_incomplete_room =
                        engine
                            .rooms
                            .new_incomplete_room(None, layer, Some(search_shape.clone()));
                    let new_rooms =
                        engine.complete_expansion_room_or_committed(board, new_incomplete_room);
                    if new_rooms.len() != 1 {
                        return false;
                    }
                    RoomRef::Complete(new_rooms[0])
                }
            };

            self.rooms[layer - self.first_layer] = Some(found_room);
        }
        true
    }

        pub fn get_shape(&self) -> &TileShape {
        &self.shape
    }

        pub fn get_dimension(&self) -> i32 {
        2
    }

                pub fn other_room(&self, _room: RoomRef) -> Option<RoomRef> {
        None
    }

        pub fn maze_search_element_count(&self) -> usize {
        self.maze_search_elements.len()
    }

                    pub fn get_maze_search_element(&self, index: usize) -> &MazeSearchElement {
        &self.maze_search_elements[index]
    }

            pub fn get_maze_search_element_mut(&mut self, index: usize) -> &mut MazeSearchElement {
        &mut self.maze_search_elements[index]
    }

            pub fn reset(&mut self) {
        for element in &mut self.maze_search_elements {
            element.reset();
        }
    }

                            pub fn get_id(&self) -> i32 {
        let inner = 31i32
            .wrapping_mul(self.location.get_id())
            .wrapping_add(self.first_layer as i32);
        31i32
            .wrapping_mul(inner)
            .wrapping_add(self.last_layer as i32)
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use fr_geometry::IntBox;

    #[test]
    fn the_id_is_javas_hash_of_the_location_and_the_two_layers() {
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
