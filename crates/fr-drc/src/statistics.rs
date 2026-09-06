use serde::Serialize;

use crate::DrcViolation;

#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct BoardStatisticsClearanceViolations {
    #[serde(rename = "total_count", skip_serializing_if = "Option::is_none")]
    pub total_count: Option<i32>,
    #[serde(rename = "min_violation_um", skip_serializing_if = "Option::is_none")]
    pub min_violation_um: Option<f64>,
    #[serde(rename = "max_violation_um", skip_serializing_if = "Option::is_none")]
    pub max_violation_um: Option<f64>,
    #[serde(rename = "avg_violation_um", skip_serializing_if = "Option::is_none")]
    pub avg_violation_um: Option<f64>,
}

impl BoardStatisticsClearanceViolations {
    pub fn from_violations(
        violations: &[DrcViolation],
        board_unit_to_um_factor: f64,
    ) -> BoardStatisticsClearanceViolations {
        let total_count = violations.len() as i32;

        let (min_violation, max_violation, avg_violation) = if violations.is_empty() {
            (0.0, 0.0, 0.0)
        } else {
            let mut minimum = f64::MAX;
            let mut maximum = 0.0_f64;
            let mut sum = 0.0;
            for violation in violations {
                let shortfall_um = violation.shortfall() * board_unit_to_um_factor;
                minimum = (minimum).min(shortfall_um);
                maximum = (maximum).max(shortfall_um);
                sum += shortfall_um;
            }
            (minimum, maximum, sum / violations.len() as f64)
        };

        BoardStatisticsClearanceViolations {
            total_count: Some(total_count),
            min_violation_um: Some(min_violation),
            max_violation_um: Some(max_violation),
            avg_violation_um: Some(avg_violation),
        }
    }
}
