use copper_board::Board;

use crate::{LayerSettings, RouterSettings, ScoringSettings};

impl RouterSettings {
    pub fn apply_board_specific_optimizations_if_needed(&mut self, board: &Board) {
        let board_layer_count = board.get_layer_count();
        if self.get_layer_count() != board_layer_count
            || !self.are_board_specific_trace_costs_applied()
        {
            self.apply_board_specific_optimizations(board);
        }
    }

    pub fn are_board_specific_trace_costs_applied(&self) -> bool {
        matches!(self.board_specific_trace_costs_applied, Some(true))
    }

    pub fn apply_board_specific_optimizations(&mut self, board: &Board) {
        let bounding_box = board.get_bounding_box();
        let horizontal_width = f64::from(bounding_box.width());
        let vertical_width = f64::from(bounding_box.height());

        let layer_count = board.get_layer_count();

        if self.scoring.is_none() {
            self.scoring = Some(ScoringSettings::default());
        }

        let horizontal_add_costs_against_preferred_dir =
            0.1 * (10.0 * horizontal_width / vertical_width).round() as i64 as f64;
        let vertical_add_costs_against_preferred_dir =
            0.1 * (10.0 * vertical_width / horizontal_width).round() as i64 as f64;

        if !matches!(&self.layers, Some(layers) if layers.len() == layer_count) {
            self.board_specific_trace_costs_applied = Some(false);
            let old_layers = self.layers.take();
            let mut layers = Vec::with_capacity(layer_count);
            for i in 0..layer_count {
                layers.push(match old_layers.as_ref().and_then(|old| old.get(i)) {
                    Some(existing) => *existing,
                    None => LayerSettings::default(),
                });
            }
            self.layers = Some(layers);
        }

        let scoring = self.scoring.as_mut().expect("instantiated above");
        let mut costs_reallocated = false;
        if !matches!(&scoring.preferred_direction_trace_cost, Some(a) if a.len() == layer_count) {
            scoring.preferred_direction_trace_cost = Some(vec![0.0; layer_count]);
            costs_reallocated = true;
        }
        if !matches!(&scoring.undesired_direction_trace_cost, Some(a) if a.len() == layer_count) {
            scoring.undesired_direction_trace_cost = Some(vec![0.0; layer_count]);
            costs_reallocated = true;
        }
        if scoring.default_preferred_direction_trace_cost.is_none() {
            scoring.default_preferred_direction_trace_cost = Some(1.0);
        }
        if scoring.default_undesired_direction_trace_cost.is_none() {
            scoring.default_undesired_direction_trace_cost = Some(1.0);
        }
        let default_preferred_direction_trace_cost = scoring
            .default_preferred_direction_trace_cost
            .expect("set above");
        let default_undesired_direction_trace_cost = scoring
            .default_undesired_direction_trace_cost
            .expect("set above");
        let default_bend_cost = scoring.default_bend_cost.unwrap_or(0.0);
        if costs_reallocated {
            self.board_specific_trace_costs_applied = Some(false);
        }

        let mut current_preferred_direction_is_horizontal = horizontal_width < vertical_width;
        let initialize_trace_costs = !self.are_board_specific_trace_costs_applied();

        let layer_structure = board.layer_structure();
        let layers = self.layers.as_mut().expect("allocated above");
        let scoring = self.scoring.as_mut().expect("instantiated above");
        let preferred_costs = scoring
            .preferred_direction_trace_cost
            .as_mut()
            .expect("allocated above");
        let undesired_costs = scoring
            .undesired_direction_trace_cost
            .as_mut()
            .expect("allocated above");
        for i in 0..layer_count {
            let is_signal = layer_structure.layers[i].is_signal;
            if is_signal {
                current_preferred_direction_is_horizontal =
                    !current_preferred_direction_is_horizontal;
            }
            if !is_signal {
                layers[i].routable = Some(false);
            } else if layers[i].routable.is_none() {
                layers[i].routable = Some(true);
            }
            if layers[i].bend_cost.is_none() {
                layers[i].bend_cost = Some(default_bend_cost);
            }
            if layers[i].preferred_direction_horizontal.is_none() {
                layers[i].preferred_direction_horizontal =
                    Some(current_preferred_direction_is_horizontal);
            }
            if initialize_trace_costs {
                preferred_costs[i] = default_preferred_direction_trace_cost;
                undesired_costs[i] = default_undesired_direction_trace_cost;
                if current_preferred_direction_is_horizontal {
                    undesired_costs[i] += horizontal_add_costs_against_preferred_dir;
                } else {
                    undesired_costs[i] += vertical_add_costs_against_preferred_dir;
                }
            }
        }

        if initialize_trace_costs {
            let signal_layer_count = layer_structure.signal_layer_count();
            if signal_layer_count > 2 {
                let outer_add_costs = 0.2 * signal_layer_count as f64;
                preferred_costs[0] += outer_add_costs;
                preferred_costs[layer_count - 1] += outer_add_costs;
                undesired_costs[0] += outer_add_costs;
                undesired_costs[layer_count - 1] += outer_add_costs;
            }
            self.board_specific_trace_costs_applied = Some(true);
        }
    }
}
