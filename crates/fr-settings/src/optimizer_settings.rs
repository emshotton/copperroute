use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoardUpdateStrategy {
    Greedy,
    GlobalOptimal,
    Hybrid,
}

impl BoardUpdateStrategy {
    pub fn java_name(self) -> &'static str {
        match self {
            Self::Greedy => "GREEDY",
            Self::GlobalOptimal => "GLOBAL_OPTIMAL",
            Self::Hybrid => "HYBRID",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemSelectionStrategy {
    Sequential,
    Random,
    Prioritized,
}

impl ItemSelectionStrategy {
    pub fn java_name(self) -> &'static str {
        match self {
            Self::Sequential => "SEQUENTIAL",
            Self::Random => "RANDOM",
            Self::Prioritized => "PRIORITIZED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct OptimizerSettings {
    #[serde(rename = "enabled", default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,

    #[serde(rename = "algorithm", default, skip_serializing_if = "Option::is_none")]
    pub algorithm: Option<String>,

    #[serde(
        rename = "max_passes",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub max_passes: Option<i32>,

    #[serde(rename = "max_items", default, skip_serializing_if = "Option::is_none")]
    pub max_items: Option<i32>,

    #[serde(
        rename = "max_threads",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub max_threads: Option<i32>,

    #[serde(
        rename = "improvement_threshold",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub optimization_improvement_threshold: Option<f32>,

    #[serde(
        rename = "max_consecutive_failures",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub max_consecutive_failures: Option<i32>,

    #[serde(
        rename = "additional_ripup_cost_factor_at_start",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub additional_ripup_cost_factor_at_start: Option<i32>,

    #[serde(
        rename = "trace_ripup_cost_factor",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub trace_ripup_cost_factor: Option<f32>,

    #[serde(
        rename = "max_autoroute_passes",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub max_autoroute_passes: Option<i32>,

    #[serde(
        rename = "max_search_steps",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub max_search_steps: Option<i64>,

    #[serde(skip)]
    pub board_update_strategy: Option<BoardUpdateStrategy>,

    #[serde(skip)]
    pub hybrid_ratio: Option<String>,

    #[serde(skip)]
    pub item_selection_strategy: Option<ItemSelectionStrategy>,

    #[serde(rename = "timeout", default, skip_serializing_if = "Option::is_none")]
    pub timeout_string: Option<String>,
}

impl OptimizerSettings {
    pub const FIELD_NAMES: &'static [&'static str] = &[
        "enabled",
        "algorithm",
        "max_passes",
        "max_items",
        "max_threads",
        "optimization_improvement_threshold",
        "max_consecutive_failures",
        "additional_ripup_cost_factor_at_start",
        "trace_ripup_cost_factor",
        "max_autoroute_passes",
        "max_search_steps",
        "board_update_strategy",
        "hybrid_ratio",
        "item_selection_strategy",
        "timeout_string",
    ];
}
