use fr_geometry::{FloatPoint, IntBox, java_max, java_min};
use fr_settings::ExpansionCostFactor;

#[derive(Debug, Clone, PartialEq)]
pub struct DestinationDistance {
        trace_costs: Vec<ExpansionCostFactor>,
            #[allow(dead_code)]
    layer_active: Vec<bool>,
        layer_count: usize,
        active_layer_count: usize,
            min_cheap_via_cost: f64,
        pub min_component_side_trace_cost: f64,
        pub max_component_side_trace_cost: f64,
        pub min_solder_side_trace_cost: f64,
        pub max_solder_side_trace_cost: f64,
            pub max_inner_side_trace_cost: f64,
            pub min_component_inner_trace_cost: f64,
            pub min_solder_inner_trace_cost: f64,
            pub min_component_solder_inner_trace_cost: f64,
                        min_normal_via_cost: f64,
        component_side_box: IntBox,
        solder_side_box: IntBox,
        inner_side_box: IntBox,
        box_is_empty: bool,
        component_side_box_is_empty: bool,
        solder_side_box_is_empty: bool,
        inner_side_box_is_empty: bool,
}

impl DestinationDistance {
            pub fn new(
        trace_costs: &[ExpansionCostFactor],
        layer_active: &[bool],
        min_normal_via_cost: f64,
        min_cheap_via_cost: f64,
    ) -> DestinationDistance {
        let layer_count = layer_active.len(); 
        let active_layer_count = layer_active.iter().filter(|a| **a).count(); 

        let (min_component_side_trace_cost, max_component_side_trace_cost) = if layer_active[0] {
            let c = trace_costs[0];
            if c.horizontal < c.vertical {
                (c.horizontal, c.vertical)
            } else {
                (c.vertical, c.horizontal)
            }
        } else {
            (0.0, 0.0)
        };

        let (min_solder_side_trace_cost, max_solder_side_trace_cost) =
            if layer_active[layer_count - 1] {
                let c = trace_costs[layer_count - 1];
                if c.horizontal < c.vertical {
                    (c.horizontal, c.vertical)
                } else {
                    (c.vertical, c.horizontal)
                }
            } else {
                (0.0, 0.0)
            };

        let mut max_inner_side_trace_cost =
            java_min(max_component_side_trace_cost, max_solder_side_trace_cost);
        for ind2 in 1..layer_count.saturating_sub(1) {
            if !layer_active[ind2] {
                continue; 
            }
            let current_max_cost =
                java_max(trace_costs[ind2].horizontal, trace_costs[ind2].vertical);
            max_inner_side_trace_cost = java_min(max_inner_side_trace_cost, current_max_cost);
        }
        let min_component_inner_trace_cost =
            java_min(min_component_side_trace_cost, max_inner_side_trace_cost);
        let min_solder_inner_trace_cost =
            java_min(min_solder_side_trace_cost, max_inner_side_trace_cost);
        let min_component_solder_inner_trace_cost =
            java_min(min_component_inner_trace_cost, min_solder_inner_trace_cost);

        DestinationDistance {
            trace_costs: trace_costs.to_vec(),
            layer_active: layer_active.to_vec(),
            layer_count,
            active_layer_count,
            min_cheap_via_cost,
            min_component_side_trace_cost,
            max_component_side_trace_cost,
            min_solder_side_trace_cost,
            max_solder_side_trace_cost,
            max_inner_side_trace_cost,
            min_component_inner_trace_cost,
            min_solder_inner_trace_cost,
            min_component_solder_inner_trace_cost,
            min_normal_via_cost,
            component_side_box: IntBox::EMPTY,
            solder_side_box: IntBox::EMPTY,
            inner_side_box: IntBox::EMPTY,
            box_is_empty: true,
            component_side_box_is_empty: true,
            solder_side_box_is_empty: true,
            inner_side_box_is_empty: true,
        }
    }

                        pub fn join(&mut self, box_to_join: &IntBox, layer: usize) {
        if layer == 0 {
            self.component_side_box = self.component_side_box.union(box_to_join);
            self.component_side_box_is_empty = false;
        } else if layer == self.layer_count - 1 {
            self.solder_side_box = self.solder_side_box.union(box_to_join);
            self.solder_side_box_is_empty = false;
        } else {
            self.inner_side_box = self.inner_side_box.union(box_to_join);
            self.inner_side_box_is_empty = false;
        }
        self.box_is_empty = false;
    }

                            pub fn calculate_from_point(&self, point: &FloatPoint, layer: usize) -> f64 {
        self.calculate(&point.bounding_box(), layer)
    }

            pub fn calculate(&self, box_: &IntBox, layer: usize) -> f64 {
        self.calculate_with_via_cost(box_, layer, self.min_normal_via_cost)
    }

                                                        pub fn calculate_cheap_distance(&self, box_: &IntBox, layer: usize) -> f64 {
        self.calculate_with_via_cost(box_, layer, self.min_cheap_via_cost)
    }

            fn calculate_with_via_cost(
        &self,
        box_: &IntBox,
        layer: usize,
        min_normal_via_cost: f64,
    ) -> f64 {
        if self.box_is_empty {
            return f64::from(i32::MAX);
        }

        let (component_side_delta_x, component_side_delta_y) =
            deltas(box_, &self.component_side_box);
        let (solder_side_delta_x, solder_side_delta_y) = deltas(box_, &self.solder_side_box);
        let (inner_side_delta_x, inner_side_delta_y) = deltas(box_, &self.inner_side_box);

        let (component_side_max_delta, component_side_min_delta) =
            max_min(component_side_delta_x, component_side_delta_y);
        let (solder_side_max_delta, solder_side_min_delta) =
            max_min(solder_side_delta_x, solder_side_delta_y);
        let (inner_side_max_delta, inner_side_min_delta) =
            max_min(inner_side_delta_x, inner_side_delta_y);

        let mut result = f64::from(i32::MAX); 

        if layer == 0 {
            if !self.component_side_box_is_empty {
                result = box_.weighted_distance(
                    &self.component_side_box,
                    self.trace_costs[0].horizontal,
                    self.trace_costs[0].vertical,
                );
            }
            if self.active_layer_count <= 1 {
                return result; 
            }

            let mut tmp_distance =
                if self.min_solder_side_trace_cost < self.min_component_side_trace_cost {
                    self.min_solder_side_trace_cost * solder_side_max_delta
                        + self.min_component_side_trace_cost * solder_side_min_delta
                        + min_normal_via_cost
                } else {
                    self.min_component_side_trace_cost * solder_side_max_delta
                        + self.min_solder_side_trace_cost * solder_side_min_delta
                        + min_normal_via_cost
                };
            result = java_min(result, tmp_distance); 

            tmp_distance = component_side_max_delta
                + component_side_min_delta * self.min_component_inner_trace_cost
                + 2.0 * min_normal_via_cost;
            result = java_min(result, tmp_distance); 

            if self.active_layer_count == 2 {
                return result; 
            }

            tmp_distance = inner_side_max_delta
                + inner_side_min_delta * self.min_component_inner_trace_cost
                + min_normal_via_cost;
            result = java_min(result, tmp_distance); 

            tmp_distance = solder_side_max_delta
                + self.min_component_solder_inner_trace_cost * solder_side_min_delta
                + 2.0 * min_normal_via_cost;
            result = java_min(result, tmp_distance);

            tmp_distance =
                component_side_max_delta + component_side_min_delta + 2.0 * min_normal_via_cost;
            result = java_min(result, tmp_distance);

            if self.active_layer_count == 3 {
                return result; 
            }

            tmp_distance = inner_side_max_delta + inner_side_min_delta + 2.0 * min_normal_via_cost;
            result = java_min(result, tmp_distance);

            tmp_distance =
                solder_side_max_delta + solder_side_min_delta + 3.0 * min_normal_via_cost;
            return java_min(result, tmp_distance);
        }

        if layer == self.layer_count - 1 {
            if !self.solder_side_box_is_empty {
                result = box_.weighted_distance(
                    &self.solder_side_box,
                    self.trace_costs[layer].horizontal,
                    self.trace_costs[layer].vertical,
                );
            }

            let mut tmp_distance =
                if self.min_component_side_trace_cost < self.min_solder_side_trace_cost {
                    self.min_component_side_trace_cost * component_side_max_delta
                        + self.min_solder_side_trace_cost * component_side_min_delta
                        + min_normal_via_cost
                } else {
                    self.min_solder_side_trace_cost * component_side_max_delta
                        + self.min_component_side_trace_cost * component_side_min_delta
                        + min_normal_via_cost
                };
            result = java_min(result, tmp_distance); 

            tmp_distance = solder_side_max_delta
                + solder_side_min_delta * self.min_solder_inner_trace_cost
                + 2.0 * min_normal_via_cost;
            result = java_min(result, tmp_distance);

            if self.active_layer_count <= 2 {
                return result; 
            }

            tmp_distance = inner_side_min_delta * self.min_solder_inner_trace_cost
                + inner_side_max_delta
                + min_normal_via_cost;
            result = java_min(result, tmp_distance);

            tmp_distance = component_side_max_delta
                + self.min_component_solder_inner_trace_cost * component_side_min_delta
                + 2.0 * min_normal_via_cost;
            result = java_min(result, tmp_distance);

            tmp_distance =
                solder_side_max_delta + solder_side_min_delta + 2.0 * min_normal_via_cost;
            result = java_min(result, tmp_distance);

            if self.active_layer_count == 3 {
                return result; 
            }

            tmp_distance = inner_side_max_delta + inner_side_min_delta + 2.0 * min_normal_via_cost;
            result = java_min(result, tmp_distance);

            tmp_distance =
                component_side_max_delta + component_side_min_delta + 3.0 * min_normal_via_cost;
            return java_min(result, tmp_distance);
        }

        if !self.inner_side_box_is_empty {
            result = box_.weighted_distance(
                &self.inner_side_box,
                self.trace_costs[layer].horizontal,
                self.trace_costs[layer].vertical,
            );
        }

        let mut tmp_distance = inner_side_max_delta + inner_side_min_delta + min_normal_via_cost;
        result = java_min(result, tmp_distance);
        tmp_distance = component_side_max_delta
            + component_side_min_delta * self.min_component_inner_trace_cost
            + min_normal_via_cost;
        result = java_min(result, tmp_distance);
        tmp_distance = solder_side_max_delta
            + solder_side_min_delta * self.min_solder_inner_trace_cost
            + min_normal_via_cost;
        result = java_min(result, tmp_distance);

        tmp_distance =
            component_side_max_delta + component_side_min_delta + 2.0 * min_normal_via_cost;
        result = java_min(result, tmp_distance);
        tmp_distance = solder_side_max_delta + solder_side_min_delta + 2.0 * min_normal_via_cost;
        java_min(result, tmp_distance)
    }
}

fn deltas(box_: &IntBox, other: &IntBox) -> (f64, f64) {
    let delta_x = if box_.ll.x > other.ur.x {
        f64::from(box_.ll.x.wrapping_sub(other.ur.x))
    } else if box_.ur.x < other.ll.x {
        f64::from(other.ll.x.wrapping_sub(box_.ur.x))
    } else {
        0.0
    };
    let delta_y = if box_.ll.y > other.ur.y {
        f64::from(box_.ll.y.wrapping_sub(other.ur.y))
    } else if box_.ur.y < other.ll.y {
        f64::from(other.ll.y.wrapping_sub(box_.ur.y))
    } else {
        0.0
    };
    (delta_x, delta_y)
}

fn max_min(delta_x: f64, delta_y: f64) -> (f64, f64) {
    if delta_x > delta_y {
        (delta_x, delta_y)
    } else {
        (delta_y, delta_x)
    }
}
