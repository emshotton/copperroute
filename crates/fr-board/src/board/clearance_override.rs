use fr_geometry::{Area, Shape};

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
        (Unit::scale(
            clearance_um * f64::from(board_resolution),
            Unit::Um,
            self.communication.unit,
        ))
        .round() as i64 as i32
    }

    pub fn apply_copper_to_edge_clearance_override(&mut self, clearance_um: f64) -> bool {
        if clearance_um < 0.0 {
            return false;
        }
        // :488-494: no outline, nothing to re-point.
        let Some(outline_id) = self.get_outline() else {
            return false;
        };

        // :509-516.
        let configured_clearance_board_units = self.clearance_override_board_units(clearance_um);

        // :518-528: reuse a `board_edge` class the DSN already declares, else append one. No
        // corpus board declares one, so this always appends and the new index is
        // board-dependent (3, 4 or 10 across the 16 corpus boards) — a port must append, not
        // assume 3.
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

        // :536-541. Both the row and the column, on every layer, unconditionally — unlike the
        // hole path below, which floors against the existing value. The two are deliberately
        // not factored together. The loop starts at class 1, so column/row 0 (the `"null"`
        // class) keeps its zeros, and it ends at the *new* class count, so the
        // `[board_edge][board_edge]` diagonal is written too.
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

        // :543-549: remove from the trees, re-point, clear the derived data, insert again. Note
        // this is *not* `Item.changeClearanceClassIndex` — the remove/insert pair runs whether
        // or not clearance compensation is on, and the `clearDerivedData` sits between the
        // class write and the insert.
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
        // :354-359.
        if clearance_um < 0.0 {
            return false;
        }
        // :365-372.
        let configured_clearance_board_units = self.clearance_override_board_units(clearance_um);
        // :373-374: `changed` is read *before* the write, and the write is unconditional.
        let changed = configured_clearance_board_units != self.rules.get_hole_clearance();
        self.rules
            .set_hole_clearance(configured_clearance_board_units);
        // :375-378: the `> 0` gate lives here, not inside the reclassifier.
        let mut hole_keepouts = 0;
        if configured_clearance_board_units > 0 {
            hole_keepouts = self
                .assign_hole_keepout_clearance_class_board_units(configured_clearance_board_units);
        }
        if changed || hole_keepouts > 0 {
            self.reinsert_tree_items();
        }
        // :387-395 is an `FRLogger.info` only.
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
                    // :419-420: a package keepout belongs to a component, and a circular one is
                    // a drilled hole in the footprint.
                    Item::ObstacleArea(keepout) => {
                        keepout.hdr.get_component_id() > 0
                            && matches!(keepout.get_area(&ctx), Area::Shape(Shape::Circle(_)))
                    }
                    _ => false,
                })
                .map(|(id, _)| *id)
                .collect()
        };
        // :424-426.
        if hole_keepouts.is_empty() {
            return 0;
        }
        // :427-436, as for `board_edge` above.
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
        // :437-442.
        let default_net_class = self.rules.get_default_net_class();
        let default_area_class_no = self
            .rules
            .net_classes
            .get(default_net_class)
            .default_item_clearance_classes
            .get(ItemClass::Area);

        // :443-454. Two things are load-bearing and must not be "cleaned up":
        //
        //   * the value is a **floor**, `max(holeClearance, the AREA row's existing value)` —
        //     never reduce an existing requirement (:445-450). At 100 µm on the corpus the
        //     existing copper clearance always wins, so only the item reclassification is
        //     observable; at 500 µm the floor bites on the lower-valued columns.
        //   * the read and the writes **interleave**. The loop runs to the *new* class count,
        //     so on the last iteration `class_no == hole_edge_class_no` and the read
        //     `get_value(default_area_class_no, hole_edge_class_no, …)` sees the cell the
        //     `class_no == default_area_class_no` iteration already wrote through its symmetric
        //     `set_value(class_no, hole_edge_class_no, …)`. Hoisting the reads out of the loop
        //     would change the diagonal.
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

        // :455-462. Note there is no search-tree remove/insert here — the caller's
        // `reinsertTreeItems` covers it — and `clearDerivedData` runs only for a keepout whose
        // class actually changes.
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
