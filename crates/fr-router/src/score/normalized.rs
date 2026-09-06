use fr_settings::ScoringSettings;

use super::statistics::BoardStatistics;

impl BoardStatistics {
    pub fn calculate_score(&self, scoring: &ScoringSettings) -> f32 {
        let maximum_score = self.maximum_score(scoring);

        let unrouted_net_penalty = scoring
            .unrouted_net_penalty
            .expect("ScoringSettings.unroutedNetPenalty is null (Java: NullPointerException)");
        let clearance_violation_penalty = scoring.clearance_violation_penalty.expect(
            "ScoringSettings.clearanceViolationPenalty is null (Java: NullPointerException)",
        );
        let bend_penalty = scoring
            .bend_penalty
            .expect("ScoringSettings.bendPenalty is null (Java: NullPointerException)");

        let incomplete_count = self
            .connections
            .incomplete_count
            .expect("connections.incompleteCount is null (Java: NullPointerException)");
        let violation_count = self
            .clearance_violations
            .total_count
            .expect("clearanceViolations.totalCount is null (Java: NullPointerException)");
        let bend_count = self
            .bends
            .total_count
            .expect("bends.totalCount is null (Java: NullPointerException)");
        let penalties = incomplete_count as f32 * unrouted_net_penalty
            + violation_count as f32 * clearance_violation_penalty
            + bend_count as f32 * bend_penalty;

        let trace_length_for_cost = self
            .traces
            .total_length_mm
            .or(self.traces.total_length)
            .expect("traces.totalLength is null (Java: NullPointerException)");

        let trace_cost = scoring.default_preferred_direction_trace_cost.expect(
            "ScoringSettings.defaultPreferredDirectionTraceCost is null \
             (Java: NullPointerException)",
        );
        let via_costs = scoring
            .via_costs
            .expect("ScoringSettings.viaCosts is null (Java: NullPointerException)");
        let via_count = self
            .vias
            .total_count
            .expect("vias.totalCount is null (Java: NullPointerException)");
        let via_term = f64::from(via_count.wrapping_mul(via_costs));
        let costs = (f64::from(trace_length_for_cost) * trace_cost + via_term) as f32;

        maximum_score - penalties - costs
    }

    pub fn maximum_score(&self, scoring: &ScoringSettings) -> f32 {
        let maximum_count = self
            .connections
            .maximum_count
            .expect("connections.maximumCount is null (Java: NullPointerException)");
        let unrouted_net_penalty = scoring
            .unrouted_net_penalty
            .expect("ScoringSettings.unroutedNetPenalty is null (Java: NullPointerException)");
        maximum_count as f32 * unrouted_net_penalty
    }

    pub fn normalized_score(&self, scoring: &ScoringSettings) -> f32 {
        let maximum_score = self.maximum_score(scoring);
        if maximum_score <= 0.0 {
            return 0.0;
        }
        (self.calculate_score(scoring) / maximum_score).max(0.0) * 1000.0
    }

    /// The score's cost term — trace length, vias and bends at the scoring weights — in `f64`,
    /// so that one via on a large board is still a visible difference.
    pub fn routing_cost(&self, scoring: &ScoringSettings) -> f64 {
        let length_mm = self
            .traces
            .total_length_mm
            .or(self.traces.total_length)
            .unwrap_or(0.0);
        let trace_cost = scoring
            .default_preferred_direction_trace_cost
            .unwrap_or(1.0);
        let via_costs = scoring.via_costs.unwrap_or(0);
        let bend_penalty = scoring.bend_penalty.unwrap_or(0.0);
        f64::from(length_mm) * trace_cost
            + f64::from(via_costs) * f64::from(self.vias.total_count.unwrap_or(0))
            + f64::from(bend_penalty) * f64::from(self.bends.total_count.unwrap_or(0))
    }

    /// Everything the score subtracts from its maximum, in `f64`: the unrouted and violation
    /// penalties plus [`routing_cost`](Self::routing_cost). Lower is better.
    pub fn routing_penalty(&self, scoring: &ScoringSettings) -> f64 {
        let unrouted_net_penalty = scoring.unrouted_net_penalty.unwrap_or(0.0);
        let clearance_violation_penalty = scoring.clearance_violation_penalty.unwrap_or(0.0);
        f64::from(unrouted_net_penalty) * f64::from(self.connections.incomplete_count.unwrap_or(0))
            + f64::from(clearance_violation_penalty)
                * f64::from(self.clearance_violations.total_count.unwrap_or(0))
            + self.routing_cost(scoring)
    }
}
