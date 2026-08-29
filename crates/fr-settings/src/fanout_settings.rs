//! `settings/FanoutSettings.java` (123 lines): configuration for the SMD-pin fanout pre-pass that
//! runs before the main batch autorouter.

use serde::{Deserialize, Serialize};

/// Configuration for the SMD-pin fanout pre-pass (`FanoutSettings.java:22-105`). During fanout
/// each single-layer (SMD) pin is given a short escape trace and a via so that the main router
/// can work pin-to-via rather than pin-to-pin.
///
/// Every field follows the `RouterSettings` nullable-field contract (plan ruling 4): a `None`
/// value means "this source has no opinion" and is skipped by the merge engine (Task 2). None of
/// this struct's fields are `transient` in Java, so none are skipped by serde either.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FanoutSettings {
    /// Whether to run the fanout pre-pass at all. `FanoutSettings.java:24-25`.
    #[serde(rename = "enabled", default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,

    /// Maximum number of fanout passes. `FanoutSettings.java:31-32`.
    #[serde(
        rename = "max_passes",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub max_passes: Option<i32>,

    /// Maximum number of escape/fanout routing attempts allowed during the fanout stage.
    /// `FanoutSettings.java:38-39`.
    #[serde(rename = "max_items", default, skip_serializing_if = "Option::is_none")]
    pub max_items: Option<i32>,

    /// Base time budget (in milliseconds) that each individual SMD pin may consume in pass 1.
    /// `FanoutSettings.java:47-48`. `Long` in Java — `i64` here.
    #[serde(
        rename = "max_milliseconds_per_pin",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub max_milliseconds_per_pin: Option<i64>,

    /// Whether the fanout router is allowed to rip up and re-route existing traces to make room
    /// for a new escape via. `FanoutSettings.java:56-59`.
    #[serde(
        rename = "ripup_allowed",
        alias = "ripupAllowed",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub ripup_allowed: Option<bool>,

    /// The minimum physical escape wire length (in millimeters). `FanoutSettings.java:62-64`.
    #[serde(
        rename = "min_escape_length_mm",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub min_escape_length_mm: Option<f64>,

    /// The maximum physical escape wire length (in millimeters). `FanoutSettings.java:67-69`.
    #[serde(
        rename = "max_escape_length_mm",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub max_escape_length_mm: Option<f64>,

    /// The diameter of starting/escape vias used inside the pins during the fanout/escape stage
    /// (in millimeters). Default is 0.250 mm. `FanoutSettings.java:73-75`.
    #[serde(
        rename = "start_via_diameter_mm",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub start_via_diameter_mm: Option<f64>,

    /// The diameter of landing/end vias used at the end of escaping wires during the
    /// fanout/escape stage (in millimeters). Default is 0.250 mm. `FanoutSettings.java:79-81`.
    #[serde(
        rename = "end_via_diameter_mm",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub end_via_diameter_mm: Option<f64>,

    /// The sorting order for SMD pins within a component: `"inner_first"` (v1.9 default),
    /// `"outer_first"`, or `"unsorted"`. `FanoutSettings.java:87-89`.
    #[serde(
        rename = "pin_sorting_order",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub pin_sorting_order: Option<String>,

    /// Whether to fall back to board-wide via rules if a net has no via rules defined or an empty
    /// via list during fanout. `FanoutSettings.java:93-95`.
    #[serde(
        rename = "fallback_to_board_vias",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub fallback_to_board_vias: Option<bool>,

    /// Timeout for the fanout stage (e.g. `"5m"`, `"300s"`). Default is `None` (no timeout).
    /// `FanoutSettings.java:98-99`.
    #[serde(rename = "timeout", default, skip_serializing_if = "Option::is_none")]
    pub timeout_string: Option<String>,
}

impl FanoutSettings {
    /// Rust field names in `FanoutSettings.getDeclaredFields()` source order.
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

// renamed: clone -> derive(Clone) (FanoutSettings.java:110-118). Java's clone() is a plain
// Object.clone() (shallow copy) with no array fields to re-clone; a derived Clone over immutable
// wrapper/String fields is the same operation.
