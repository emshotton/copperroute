use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ScoringSettings {
    #[serde(skip)]
    pub preferred_direction_trace_cost: Option<Vec<f64>>,

    #[serde(skip)]
    pub undesired_direction_trace_cost: Option<Vec<f64>>,

    #[serde(
        rename = "default_preferred_direction_trace_cost",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub default_preferred_direction_trace_cost: Option<f64>,

    #[serde(
        rename = "default_undesired_direction_trace_cost",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub default_undesired_direction_trace_cost: Option<f64>,

    #[serde(
        rename = "via_costs",
        alias = "viaCosts",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub via_costs: Option<i32>,

    #[serde(
        rename = "plane_via_costs",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub plane_via_costs: Option<i32>,

    #[serde(
        rename = "start_ripup_costs",
        alias = "startRipupCosts",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub start_ripup_costs: Option<i32>,

    #[serde(
        rename = "unrouted_net_penalty",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub unrouted_net_penalty: Option<f32>,

    #[serde(
        rename = "clearance_violation_penalty",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub clearance_violation_penalty: Option<f32>,

    #[serde(
        rename = "bend_penalty",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub bend_penalty: Option<f32>,

    #[serde(
        rename = "default_bend_cost",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub default_bend_cost: Option<f64>,

    #[serde(
        rename = "smd_via_cost_factor",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub smd_via_cost_factor: Option<f64>,
}

impl ScoringSettings {
    pub const FIELD_NAMES: &'static [&'static str] = &[
        "preferred_direction_trace_cost",
        "undesired_direction_trace_cost",
        "default_preferred_direction_trace_cost",
        "default_undesired_direction_trace_cost",
        "via_costs",
        "plane_via_costs",
        "start_ripup_costs",
        "unrouted_net_penalty",
        "clearance_violation_penalty",
        "bend_penalty",
        "default_bend_cost",
        "smd_via_cost_factor",
    ];
}
