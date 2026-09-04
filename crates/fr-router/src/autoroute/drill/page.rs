use fr_board::{Board, ItemId, StopCheck, TreeObject};
use fr_geometry::{IntBox, Point, PolylineArea, TileShape};

use crate::arena::DrillId;
use crate::autoroute::expansion::RoomRef;
use crate::autoroute::maze::AutorouteEngine;
use crate::autoroute::maze::engine::tree_of;
use crate::autoroute::maze::search_element::MazeSearchElement;
use crate::{Arena, ExpansionDrill};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CutoutEntry {
    pub item: ItemId,
    pub shape_index: usize,
    pub skipped: bool,
    pub cut_out: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DrillPage {
    pub shape: IntBox,
    id_no: i32,
    maze_search_elements: Vec<MazeSearchElement>,
    drills: Option<Vec<DrillId>>,
    net_number: i32,
}

impl DrillPage {
    pub fn new(shape: IntBox, board: &Board, id_no: i32) -> DrillPage {
        DrillPage {
            shape,
            id_no,
            maze_search_elements: vec![MazeSearchElement::default(); board.get_layer_count()],
            drills: None,
            net_number: -1,
        }
    }

    pub fn get_drills(
        &mut self,
        engine: &mut AutorouteEngine,
        board: &mut Board,
        attach_smd: bool,
        stop: StopCheck<'_>,
    ) -> Vec<DrillId> {
        if self.drills.is_some() && engine.get_net_number() == self.net_number {
            return self.drills.clone().unwrap_or_default();
        }
        let new_net_number = engine.get_net_number();

        let mut trace = Vec::new();
        let cutout_shapes =
            self.cutout_shapes(engine, board, new_net_number, attach_smd, &mut trace);

        let shape_with_holes = PolylineArea::new(
            TileShape::Box(self.shape).into(),
            cutout_shapes.into_iter().map(Into::into).collect(),
        );
        let Some(drill_shapes) = shape_with_holes.split_to_convex(Some(stop)) else {
            return Vec::new();
        };

        self.net_number = new_net_number;
        for old_drill in self.drills.take().into_iter().flatten() {
            engine.rooms.drills.remove(old_drill.0);
        }
        self.drills = Some(Vec::new());

        let drill_first_layer = 0usize;
        let drill_last_layer = board.get_layer_count() - 1;
        for current_drill_shape in drill_shapes {
            let mut current_drill_location: Option<Point> = None;
            if attach_smd {
                current_drill_location =
                    calc_pin_center_in_drill(&current_drill_shape, drill_first_layer, board);
                if current_drill_location.is_none() {
                    current_drill_location =
                        calc_pin_center_in_drill(&current_drill_shape, drill_last_layer, board);
                }
            }
            let current_drill_location = current_drill_location
                .unwrap_or_else(|| Point::Int(current_drill_shape.centre_of_gravity().round()));
            let mut new_drill = ExpansionDrill::new(
                current_drill_shape,
                current_drill_location,
                drill_first_layer,
                drill_last_layer,
            );
            if new_drill.calculate_expansion_rooms(engine, board) {
                let id = DrillId(engine.rooms.drills.insert(new_drill));
                self.drills.get_or_insert_with(Vec::new).push(id);
            }
        }
        self.drills.clone().unwrap_or_default()
    }

    pub fn obstacle_cutout_trace(
        &self,
        engine: &AutorouteEngine,
        board: &mut Board,
        attach_smd: bool,
    ) -> Vec<CutoutEntry> {
        let mut trace = Vec::new();
        self.cutout_shapes(
            engine,
            board,
            engine.get_net_number(),
            attach_smd,
            &mut trace,
        );
        trace
    }

    fn cutout_shapes(
        &self,
        engine: &AutorouteEngine,
        board: &mut Board,
        net_number: i32,
        attach_smd: bool,
        trace: &mut Vec<CutoutEntry>,
    ) -> Vec<TileShape> {
        let page_shape = TileShape::Box(self.shape);
        let overlaps = {
            let ctx = board.ctx();
            tree_of(board, engine.tree).overlapping_tree_entries_with_rooms(
                &page_shape,
                None,
                &[],
                &board.items,
                &engine.rooms,
                &ctx,
            )
        };

        let mut cutout_shapes: Vec<TileShape> = Vec::new();
        let mut prev_obstacle_shape = TileShape::Box(IntBox::EMPTY);
        for current_entry in overlaps {
            let TreeObject::Item(item) = current_entry.object else {
                continue;
            };
            let drillable = board
                .get_item(item)
                .is_some_and(|i| i.is_drillable(net_number));
            if drillable {
                trace.push(CutoutEntry {
                    item,
                    shape_index: current_entry.shape_index,
                    skipped: true,
                    cut_out: false,
                });
                continue;
            }
            let smd_skip = attach_smd && {
                let ctx = board.ctx();
                board.get_item(item).is_some_and(|i| match i {
                    fr_board::Item::Pin(pin) => pin.drill_allowed(&ctx),
                    _ => false,
                })
            };
            if smd_skip {
                trace.push(CutoutEntry {
                    item,
                    shape_index: current_entry.shape_index,
                    skipped: true,
                    cut_out: false,
                });
                continue;
            }

            let Some(current_obstacle_shape) =
                board.item_tree_shape(item, engine.tree, current_entry.shape_index)
            else {
                continue;
            };
            let mut cut_out = false;
            if !prev_obstacle_shape.contains_tile(&current_obstacle_shape) {
                let current_cutout_shape = current_obstacle_shape.intersection(&page_shape);
                if current_cutout_shape.dimension() == 2 {
                    cutout_shapes.push(current_cutout_shape);
                    cut_out = true;
                }
            }
            trace.push(CutoutEntry {
                item,
                shape_index: current_entry.shape_index,
                skipped: false,
                cut_out,
            });
            prev_obstacle_shape = current_obstacle_shape;
        }
        cutout_shapes
    }

    pub fn get_shape(&self) -> TileShape {
        TileShape::Box(self.shape)
    }

    pub fn get_dimension(&self) -> i32 {
        2
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

    pub fn reset(&mut self, drills: &mut Arena<ExpansionDrill>) {
        if let Some(ids) = &self.drills {
            for id in ids {
                if let Some(drill) = drills.get_mut(id.0) {
                    drill.reset();
                }
            }
        }
        for element in &mut self.maze_search_elements {
            element.reset();
        }
    }

    pub fn invalidate(&mut self, drills: &mut Arena<ExpansionDrill>) {
        for id in self.drills.take().into_iter().flatten() {
            drills.remove(id.0);
        }
    }

    pub fn other_room(&self, _room: RoomRef) -> Option<RoomRef> {
        None
    }

    pub fn get_id(&self) -> i32 {
        self.id_no
    }

    pub fn java_id(&self) -> i32 {
        31i32
            .wrapping_mul(self.shape.get_id())
            .wrapping_add(self.net_number)
    }

    pub fn drills(&self) -> Option<&[DrillId]> {
        self.drills.as_deref()
    }

    pub fn net_number(&self) -> i32 {
        self.net_number
    }
}

fn calc_pin_center_in_drill(drill_shape: &TileShape, layer: usize, board: &Board) -> Option<Point> {
    let overlapping_items = board.overlapping_items(
        &fr_geometry::Area::Shape(fr_geometry::Shape::Tile(drill_shape.clone())),
        Some(layer),
    );
    let ctx = board.ctx();
    let mut result = None;
    for item in overlapping_items.into_iter().rev() {
        let Some(fr_board::Item::Pin(pin)) = board.get_item(item) else {
            continue;
        };
        if pin.drill_allowed(&ctx) && drill_shape.contains_inside(&pin.get_center(&ctx)) {
            result = Some(pin.get_center(&ctx));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_pages_id_is_the_engine_counter_and_javas_hashes_the_minus_one_net() {
        let shape = IntBox::from_coords(-1000, -1000, 1000, 1000);
        assert_eq!(shape.get_id(), -960_000);
        let page = DrillPage {
            id_no: 1,
            shape,
            maze_search_elements: vec![MazeSearchElement::default(); 2],
            drills: None,
            net_number: -1,
        };
        assert_eq!(page.get_id(), 1);
        assert_eq!(page.java_id(), -29_760_001);
        assert_eq!(page.net_number(), -1);
        assert_eq!(page.drills(), None);
        assert_eq!(page.get_dimension(), 2);
        assert_eq!(page.get_shape(), TileShape::Box(shape));
    }
}
