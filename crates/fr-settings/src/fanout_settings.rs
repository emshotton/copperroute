use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FanoutSettings {
        #[serde(rename = "enabled", default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,

        #[serde(
        rename = "max_passes",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub max_passes: Option<i32>,

            #[serde(rename = "max_items", default, skip_serializing_if = "Option::is_none")]
    pub max_items: Option<i32>,

            #[serde(
        rename = "max_milliseconds_per_pin",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub max_milliseconds_per_pin: Option<i64>,

            #[serde(
        rename = "ripup_allowed",
        alias = "ripupAllowed",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub ripup_allowed: Option<bool>,

        #[serde(
        rename = "min_escape_length_mm",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub min_escape_length_mm: Option<f64>,

        #[serde(
        rename = "max_escape_length_mm",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub max_escape_length_mm: Option<f64>,

            #[serde(
        rename = "start_via_diameter_mm",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub start_via_diameter_mm: Option<f64>,

            #[serde(
        rename = "end_via_diameter_mm",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub end_via_diameter_mm: Option<f64>,

            #[serde(
        rename = "pin_sorting_order",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub pin_sorting_order: Option<String>,

            #[serde(
        rename = "fallback_to_board_vias",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub fallback_to_board_vias: Option<bool>,

            #[serde(rename = "timeout", default, skip_serializing_if = "Option::is_none")]
    pub timeout_string: Option<String>,
}

impl FanoutSettings {
        pub const FIELD_NAMES: &'static [&'static str] = &[
        "enabled",
        "max_passes",
        "max_items",
        "max_milliseconds_per_pin",
        "ripup_allowed",
        "min_escape_length_mm",
        "max_escape_length_mm",
        "start_via_diameter_mm",
        "end_via_diameter_mm",
        "pin_sorting_order",
        "fallback_to_board_vias",
        "timeout_string",
    ];
}

