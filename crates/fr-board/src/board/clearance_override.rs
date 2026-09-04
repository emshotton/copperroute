use fr_geometry::{Area, Shape, java_round};

use crate::Board;
use crate::board::item_ctx;
use crate::ids::ItemId;
use crate::items::Item;
use crate::rules::ItemClass;
use crate::structure::Unit;

pub const BOARD_EDGE_CLEARANCE_CLASS_NAME: &str = "board_edge";

pub const HOLE_EDGE_CLEARANCE_CLASS_NAME: &str = "hole_edge";

pub const DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM: f64 = 500.0;

impl Board {
                                                            pub fn clearance_override_board_units(&self, clearance_um: f64) -> i32 {
        let board_resolution = self.communication.resolution.max(1);
        java_round(Unit::scale(
            clearance_um * f64::from(board_resolution),
            Unit::Um,
            self.communication.unit,
        )) as i32
    }

                                                                                pub fn apply_copper_to_edge_clearance_override(&mut self, clearance_um: f64) -> bool {
        if clearance_um < 0.0 {
            return false;
        }
        let Some(outline_id) = self.get_outline() else {
            return false;
        };
        let default_net_class = self.rules.get_default_net_class();
        let default_area_class_no = self
            .rules
            .net_classes
            .get(default_net_class)
            .default_item_clearance_classes
            .get(ItemClass::Area);
        let outline_class_no = self
            .items
            .get(&outline_id)
            .expect("Board::apply_copper_to_edge_clearance_override: get_outline just found it")
            .clearance_class();
        let uses_fallback_outline_class = outline_class_no == default_area_class_no;
        let uses_default_edge_clearance_value =
            (clearance_um - DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM).abs() < 1e-9;
        if uses_default_edge_clearance_value && !uses_fallback_outline_class {
            return false;
        }

        let configured_clearance_board_units = self.clearance_override_board_units(clearance_um);

        let matrix = &mut self.rules.clearance_matrix;
        let board_edge_class_no = match matrix.get_no(BOARD_EDGE_CLEARANCE_CLASS_NAME) {
            Some(class_no) => class_no,
            None => {
                matrix.append_class(BOARD_EDGE_CLEARANCE_CLASS_NAME);
                matrix
                    .get_no(BOARD_EDGE_CLEARANCE_CLASS_NAME)
                    .expect("ClearanceMatrix::append_class of an absent name always takes")
            }
        };

        for layer in 0..matrix.get_layer_count() {
            for class_no in 1..matrix.get_class_count() {
                matrix.set_value(
                    board_edge_class_no,
                    class_no,
                    layer,
                    configured_clearance_board_units,
                );
                matrix.set_value(
                    class_no,
                    board_edge_class_no,
                    layer,
                    configured_clearance_board_units,
                );
            }
        }

        let mut outline = self
            .items
            .remove(&outline_id)
            .expect("Board::apply_copper_to_edge_clearance_override: present, just read");
        self.trees.remove(&mut outline);
        outline.set_clearance_class(board_edge_class_no, &self.rules);
        outline.clear_derived_data();
        let ctx = item_ctx!(self);
        self.trees.insert(&mut outline, &ctx);
        self.items.insert(outline_id, outline);
        true
    }

                                                                pub fn apply_hole_clearance_override(&mut self, clearance_um: f64) -> bool {
        if clearance_um < 0.0 {
            return false;
        }
        let configured_clearance_board_units = self.clearance_override_board_units(clearance_um);
        let changed = configured_clearance_board_units != self.rules.get_hole_clearance();
        self.rules
            .set_hole_clearance(configured_clearance_board_units);
        let mut hole_keepouts = 0;
        if configured_clearance_board_units > 0 {
            hole_keepouts = self
                .assign_hole_keepout_clearance_class_board_units(configured_clearance_board_units);
        }
        if changed || hole_keepouts > 0 {
            self.reinsert_tree_items();
        }
        changed || hole_keepouts > 0
    }

                                                        pub fn assign_hole_keepout_clearance_class(&mut self, clearance_um: f64) -> bool {
        let board_units = self.clearance_override_board_units(clearance_um);
        self.assign_hole_keepout_clearance_class_board_units(board_units) > 0
    }

            fn assign_hole_keepout_clearance_class_board_units(
        &mut self,
        hole_clearance_board_units: i32,
    ) -> usize {
        let hole_keepouts: Vec<ItemId> = {
            let ctx = item_ctx!(self);
            self.items
                .iter()
                .rev()
                .filter(|(_, item)| match item {
                    Item::ObstacleArea(keepout) => {
                        keepout.hdr.get_component_id() > 0
                            && matches!(keepout.get_area(&ctx), Area::Shape(Shape::Circle(_)))
                    }
                    _ => false,
                })
                .map(|(id, _)| *id)
                .collect()
        };
        if hole_keepouts.is_empty() {
            return 0;
        }
        let matrix = &mut self.rules.clearance_matrix;
        let hole_edge_class_no = match matrix.get_no(HOLE_EDGE_CLEARANCE_CLASS_NAME) {
            Some(class_no) => class_no,
            None => {
                matrix.append_class(HOLE_EDGE_CLEARANCE_CLASS_NAME);
                matrix
                    .get_no(HOLE_EDGE_CLEARANCE_CLASS_NAME)
                    .expect("ClearanceMatrix::append_class of an absent name always takes")
            }
        };
        let default_net_class = self.rules.get_default_net_class();
        let default_area_class_no = self
            .rules
            .net_classes
            .get(default_net_class)
            .default_item_clearance_classes
            .get(ItemClass::Area);

        let matrix = &mut self.rules.clearance_matrix;
        for layer in 0..matrix.get_layer_count() {
            for class_no in 1..matrix.get_class_count() {
                let value = hole_clearance_board_units.max(matrix.get_value(
                    default_area_class_no,
                    class_no,
                    layer,
                    false,
                ));
                matrix.set_value(hole_edge_class_no, class_no, layer, value);
                matrix.set_value(class_no, hole_edge_class_no, layer, value);
            }
        }

        let mut reclassified = 0;
        for id in hole_keepouts {
            let rules = &self.rules;
            let keepout = self
                .items
                .get_mut(&id)
                .expect("Board::assign_hole_keepout_clearance_class: collected from self.items");
            if keepout.clearance_class() != hole_edge_class_no {
                keepout.set_clearance_class(hole_edge_class_no, rules);
                keepout.clear_derived_data();
                reclassified += 1;
            }
        }
        reclassified
    }
}

