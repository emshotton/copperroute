use copper_geometry::{Area, Shape, ShapeOps};

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
    pub fn raise_copper_clearances_to(&mut self, minimum: i32) -> bool {
        if minimum <= 0 {
            return false;
        }
        let mut copper_classes = std::collections::BTreeSet::new();
        for class in self.rules.net_classes.iter() {
            copper_classes.insert(class.get_trace_clearance_class());
            for kind in [
                ItemClass::Trace,
                ItemClass::Via,
                ItemClass::Pin,
                ItemClass::Smd,
                ItemClass::Area,
            ] {
                copper_classes.insert(class.default_item_clearance_classes.get(kind));
            }
        }
        for via in self.rules.via_infos.iter() {
            copper_classes.insert(via.get_clearance_class_index());
        }
        for item in self.items.values() {
            if matches!(
                item,
                Item::Trace(_) | Item::Via(_) | Item::Pin(_) | Item::ConductionArea(_)
            ) {
                copper_classes.insert(item.clearance_class());
            }
        }
        copper_classes.remove(&0);
        let matrix = &mut self.rules.clearance_matrix;
        let mut changed = false;
        for &a in &copper_classes {
            for &b in &copper_classes {
                for layer in 0..matrix.get_layer_count() {
                    if matrix.get_value(a, b, layer, false) < minimum {
                        matrix.set_value(a, b, layer, minimum);
                        changed = true;
                    }
                }
            }
        }
        if changed {
            let mut items = std::mem::take(&mut self.items);
            let ctx = item_ctx!(self);
            let mut refs: Vec<&mut Item> = items.values_mut().rev().collect();
            self.trees.clearance_value_changed(&mut refs, &ctx);
            drop(refs);
            self.items = items;
        }
        changed
    }

    pub fn raise_solder_mask_clearances(&mut self) -> bool {
        let clearance = self
            .rules
            .drc_constraints
            .as_ref()
            .and_then(|rules| rules.solder_mask_to_copper_clearance)
            .unwrap_or(0)
            .max(0);
        let mut changed = false;
        for id in self.get_pins() {
            let Some(Item::Pin(pin)) = self.get_item(id) else {
                continue;
            };
            if pin.solder_mask_expansion.is_empty() {
                continue;
            }
            let mut minimum = vec![0; self.get_layer_count()];
            for (&layer, &margin) in &pin.solder_mask_expansion {
                if let Some(value) = minimum.get_mut(layer) {
                    *value = self.solder_mask_clearance_limit(
                        id,
                        layer,
                        margin.saturating_add(clearance).max(0),
                    );
                }
            }
            changed |= self.raise_pin_clearance(id, &minimum);
        }
        changed
    }

    pub fn solder_mask_clearance_limit(&self, id: ItemId, layer: usize, requested: i32) -> i32 {
        let Some(Item::Pin(pin)) = self.get_item(id) else {
            return requested;
        };
        let ctx = self.ctx();
        let Some(shape) = pin.get_shape_on_layer(layer, &ctx) else {
            return requested;
        };
        let copper = shape.bounding_tile();
        let search = copper.enlarge(f64::from(requested));
        let mut limit = requested;
        for object in self.overlapping_objects(&search, Some(layer)) {
            let crate::ids::TreeObject::Item(other_id) = object else {
                continue;
            };
            let Some(Item::Pin(other)) = self.get_item(other_id) else {
                continue;
            };
            if other_id == id
                || other.hdr.net_count() == 0
                || pin.hdr.shares_net_no(&other.hdr.net_nos)
            {
                continue;
            }
            let Some(other_shape) = other.get_shape_on_layer(layer, &ctx) else {
                continue;
            };
            let other_copper = other_shape.bounding_tile();
            if search.intersection(&other_copper).dimension() != 2 {
                continue;
            }
            let gap = Self::calculate_clearance_between_two_shapes(
                &copper,
                &other_copper,
                f64::from(requested),
                0,
                0,
            );
            limit = limit.min(gap.floor() as i32);
        }
        limit.max(0)
    }

    pub fn raise_pin_clearance(&mut self, id: ItemId, minimum_by_layer: &[i32]) -> bool {
        let Some(Item::Pin(pin)) = self.items.get(&id) else {
            return false;
        };
        let base = pin.hdr.clearance_class();
        let matrix = &mut self.rules.clearance_matrix;
        let layers = matrix.get_layer_count();
        if minimum_by_layer.len() != layers || minimum_by_layer.iter().any(|v| *v < 0) {
            return false;
        }
        let count = matrix.get_class_count();
        if (1..count).all(|other| {
            minimum_by_layer.iter().enumerate().all(|(layer, minimum)| {
                matrix.get_value(base, other, layer, false) >= *minimum
                    && matrix.get_value(other, base, layer, false) >= *minimum
            })
        }) {
            return false;
        }
        let mut row = Vec::with_capacity(count * layers);
        let mut column = Vec::with_capacity(count * layers);
        for other in 0..count {
            for (layer, minimum) in minimum_by_layer.iter().enumerate() {
                let floor = if other == 0 { 0 } else { *minimum };
                row.push(matrix.get_value(base, other, layer, false).max(floor));
                column.push(matrix.get_value(other, base, layer, false).max(floor));
            }
        }
        let equivalent = (1..count).find(|candidate| {
            (0..count).all(|other| {
                (0..layers).all(|layer| {
                    matrix.get_value(*candidate, other, layer, false) == row[other * layers + layer]
                        && matrix.get_value(other, *candidate, layer, false)
                            == column[other * layers + layer]
                })
            })
        });
        let class = match equivalent {
            Some(class) if class == base => return false,
            Some(class) => class,
            None => {
                let mut name = format!("pad_clearance_{count}");
                while matrix.get_no(&name).is_some() {
                    name.push('_');
                }
                matrix.append_class(&name);
                for other in 0..count {
                    for layer in 0..layers {
                        matrix.set_value(count, other, layer, row[other * layers + layer]);
                        matrix.set_value(other, count, layer, column[other * layers + layer]);
                    }
                }
                for layer in 0..layers {
                    matrix.set_value(count, count, layer, row[base * layers + layer]);
                }
                count
            }
        };
        self.change_clearance_class_index(id, class)
    }

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
        let configured_clearance_board_units = self.clearance_override_board_units(clearance_um);
        self.set_copper_to_edge_clearance(|_| configured_clearance_board_units)
    }

    pub fn raise_copper_to_edge_clearance_to(&mut self, minimum_board_units: i32) -> bool {
        if minimum_board_units <= 0 {
            return false;
        }
        let matrix = &self.rules.clearance_matrix;
        let below_minimum = match matrix.get_no(BOARD_EDGE_CLEARANCE_CLASS_NAME) {
            None => true,
            Some(board_edge_class_no) => (0..matrix.get_layer_count()).any(|layer| {
                (1..matrix.get_class_count()).any(|class_no| {
                    matrix.get_value(board_edge_class_no, class_no, layer, false)
                        < minimum_board_units
                        || matrix.get_value(class_no, board_edge_class_no, layer, false)
                            < minimum_board_units
                })
            }),
        };
        below_minimum
            && self.set_copper_to_edge_clearance(|existing| existing.max(minimum_board_units))
    }

    fn set_copper_to_edge_clearance(&mut self, clearance_for: impl Fn(i32) -> i32) -> bool {
        // :488-494: no outline, nothing to re-point.
        let Some(outline_id) = self.get_outline() else {
            return false;
        };

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

        // :536-541. Both the row and the column, on every layer. The loop starts at class 1,
        // so column/row 0 (the `"null"` class) keeps its zeros, and it ends at the *new* class
        // count, so the `[board_edge][board_edge]` diagonal is written too.
        for layer in 0..matrix.get_layer_count() {
            for class_no in 1..matrix.get_class_count() {
                let clearance =
                    clearance_for(matrix.get_value(board_edge_class_no, class_no, layer, false));
                let reverse_clearance =
                    clearance_for(matrix.get_value(class_no, board_edge_class_no, layer, false));
                matrix.set_value(board_edge_class_no, class_no, layer, clearance);
                matrix.set_value(class_no, board_edge_class_no, layer, reverse_clearance);
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
        self.set_hole_clearance_board_units(configured_clearance_board_units)
    }

    pub fn raise_hole_clearance_to(&mut self, minimum_board_units: i32) -> bool {
        if self.rules.get_hole_clearance() >= minimum_board_units {
            return false;
        }
        self.set_hole_clearance_board_units(minimum_board_units)
    }

    fn set_hole_clearance_board_units(&mut self, configured_clearance_board_units: i32) -> bool {
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
