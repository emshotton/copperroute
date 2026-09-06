use fr_geometry::TileShape;

use crate::Board;
use crate::board::item_ctx;
use crate::ids::{ItemId, TreeObject};
use crate::items::{ClearanceViolation, Item, ItemCtx};

impl Board {
    pub fn clearance_violation_count(&mut self, id: ItemId) -> usize {
        self.clearance_violations(id).len()
    }

    pub fn clearance_violations(&mut self, id: ItemId) -> Vec<ClearanceViolation> {
        let mut result: Vec<ClearanceViolation> = Vec::new();
        let Some(item) = self.items.get(&id) else {
            return result;
        };
        let this_clearance_class = item.header().clearance_class();
        let this_is_trace = matches!(item, Item::Trace(_));
        let shape_layers: Vec<usize> = {
            let ctx = item_ctx!(self);
            (0..item.tile_shape_count(&ctx))
                .map(|i| item.shape_layer(i, &ctx))
                .collect()
        };
        let (first_corner, last_corner) = match item {
            Item::Trace(trace) => (trace.first_corner(), trace.last_corner()),
            _ => (None, None),
        };
        let contacts_at_first =
            first_corner.map(|point| self.trace_normal_contacts_at(id, &point, true));
        let contacts_at_last =
            last_corner.map(|point| self.trace_normal_contacts_at(id, &point, true));

        for (i, &layer) in shape_layers.iter().enumerate() {
            let Some(current_tile_shape) = self.item_tile_shape(id, i) else {
                continue;
            };
            let entries = {
                let Board {
                    items,
                    trees,
                    library,
                    components,
                    rules,
                    bounding_box,
                    max_tree_shape_width,
                    ..
                } = self;
                let ctx = ItemCtx {
                    library,
                    components,
                    rules,
                    bounding_box,
                    max_tree_shape_width: *max_tree_shape_width,
                };
                let (tree, counter) = trees.default_tree_and_counter_mut();
                tree.overlapping_tree_entries_with_clearance(
                    &current_tile_shape,
                    Some(layer),
                    &[],
                    this_clearance_class,
                    &*items,
                    &ctx,
                    counter,
                )
            };

            for entry in entries {
                let TreeObject::Item(other_id) = entry.object else {
                    continue;
                };
                if other_id == id {
                    continue;
                }
                let Some(other) = self.items.get(&other_id) else {
                    continue;
                };
                let mut is_obstacle = {
                    let ctx = item_ctx!(self);
                    other.is_obstacle(&self.items[&id], &ctx)
                };

                if is_obstacle && this_is_trace && matches!(other, Item::Trace(_)) {
                    let current_contacts = match (&contacts_at_first, &contacts_at_last) {
                        (Some(first), _) if first.contains(&other_id) => Some(first),
                        (_, Some(last)) if last.contains(&other_id) => Some(last),
                        _ => None,
                    };
                    if let Some(current_contacts) = current_contacts {
                        for contact_id in current_contacts {
                            let Some(contact @ Item::Pin(_)) = self.items.get(contact_id) else {
                                continue;
                            };
                            if contact.shares_net(&self.items[&id])
                                && contact.shares_net(&self.items[&other_id])
                            {
                                is_obstacle = false;
                                break;
                            }
                        }
                    }
                }

                if !is_obstacle {
                    continue;
                }
                let Some(shape2) = self.item_tile_shape(other_id, entry.shape_index) else {
                    continue;
                };
                let other_clearance_class = self.items[&other_id].header().clearance_class();
                let minimum_clearance = f64::from(self.rules.clearance_matrix.get_value(
                    other_clearance_class,
                    this_clearance_class,
                    layer,
                    false,
                ));

                let (cl_comp1, cl_comp2) = if self.trees.is_clearance_compensation_used() {
                    let tree = self.trees.get_default_tree();
                    (
                        tree.clearance_compensation_value(this_clearance_class, layer, &self.rules),
                        tree.clearance_compensation_value(
                            other_clearance_class,
                            layer,
                            &self.rules,
                        ),
                    )
                } else {
                    let cl_comp1 = (0.5 * minimum_clearance).round() as i64 as i32;
                    (
                        cl_comp1,
                        (minimum_clearance - f64::from(cl_comp1)).round() as i64 as i32,
                    )
                };

                let enlarged_shape1 = if cl_comp1 > 0 {
                    current_tile_shape.enlarge(f64::from(cl_comp1))
                } else {
                    current_tile_shape.clone()
                };
                let enlarged_shape2 = if cl_comp2 > 0 {
                    shape2.enlarge(f64::from(cl_comp2))
                } else {
                    shape2.clone()
                };
                let intersection = enlarged_shape1.intersection(&enlarged_shape2);
                if intersection.dimension() != 2 {
                    continue;
                }
                let actual_clearance = Board::calculate_clearance_between_two_shapes(
                    &current_tile_shape,
                    &shape2,
                    minimum_clearance,
                    cl_comp1,
                    cl_comp2,
                );
                let header = self
                    .items
                    .get_mut(&id)
                    .expect("Board::clearance_violations: present, just read")
                    .header_mut();
                if header.smallest_clearance < 0.0 || actual_clearance < header.smallest_clearance {
                    header.smallest_clearance = actual_clearance;
                }
                result.push(ClearanceViolation {
                    first_item: id,
                    second_item: other_id,
                    shape: intersection,
                    layer,
                    expected_clearance: minimum_clearance,
                    actual_clearance,
                });
            }
        }

        if let Some(Item::Via(via)) = self.items.get(&id)
            && via.is_escape_via
            && via.escape_via_smd_layer >= 0
        {
            let smd_layer = via.escape_via_smd_layer as usize;
            let this_item = &self.items[&id];
            result.retain(|violation| {
                if violation.layer != smd_layer {
                    return true;
                }
                let other = if violation.first_item == id {
                    Some(violation.second_item)
                } else if violation.second_item == id {
                    Some(violation.first_item)
                } else {
                    None
                };
                match other.and_then(|other| self.items.get(&other)) {
                    Some(other) => !other.shares_net(this_item),
                    None => true,
                }
            });
        }
        result
    }

    pub fn calculate_clearance_between_two_shapes(
        raw_shape1: &TileShape,
        raw_shape2: &TileShape,
        minimum_clearance: f64,
        cl_comp1: i32,
        cl_comp2: i32,
    ) -> f64 {
        if raw_shape1.intersection(raw_shape2).dimension() == 2 {
            return 0.0;
        }
        let mut low = 0.0;
        let mut high = minimum_clearance;
        let sum_comp = f64::from(cl_comp1.wrapping_add(cl_comp2));
        let factor1 = if sum_comp > 0.0 {
            f64::from(cl_comp1) / sum_comp
        } else {
            0.5
        };
        let factor2 = if sum_comp > 0.0 {
            f64::from(cl_comp2) / sum_comp
        } else {
            0.5
        };
        for _ in 0..16 {
            let mid = (low + high) * 0.5;
            let s1 = raw_shape1.enlarge(mid * factor1);
            let s2 = raw_shape2.enlarge(mid * factor2);
            if s1.intersection(&s2).dimension() == 2 {
                high = mid;
            } else {
                low = mid;
            }
        }
        low
    }

    pub fn aggregate_violations_sorted_by_severity(&mut self) -> Vec<ClearanceViolation> {
        let mut violations = Vec::new();
        for id in self.items_in_board_order() {
            violations.extend(self.clearance_violations(id));
        }
        violations.sort_by(|a, b| {
            (b.expected_clearance - b.actual_clearance)
                .total_cmp(&(a.expected_clearance - a.actual_clearance))
        });
        violations
    }

    pub fn smallest_clearance(&self) -> f64 {
        let mut smallest = f64::MAX;
        for item in self.get_items() {
            let item_smallest = item.header().smallest_clearance;
            if item_smallest >= 0.0 && item_smallest < smallest {
                smallest = item_smallest;
            }
        }
        smallest
    }
}
