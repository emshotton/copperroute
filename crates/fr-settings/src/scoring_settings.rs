//! `settings/ScoringSettings.java` (107 lines): weights that control how the autorouter scores a
//! board state.

use serde::{Deserialize, Serialize};

/// Weights that control how the autorouter scores a board state (`ScoringSettings.java:25-79`).
///
/// Every field follows the `RouterSettings` nullable-field contract (plan ruling 4). Naming
/// conventions, verbatim from the Java doc comment:
/// - **penalty** — subtracted when a quality constraint is violated (unrouted net, DRC violation,
///   bend). These appear in the board-score formula.
/// - **costs** — subtracted as an absolute cost proportional to resource usage (via count, trace
///   length). These also appear in the board-score formula.
/// - **start_ripup_costs** — a routing-control parameter passed to the maze-search engine; it
///   does *not* appear in the board-score formula.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ScoringSettings {
    /// The cost of 1 mm of trace length routed in the preferred direction, per layer.
    /// `ScoringSettings.java:27-29`.
    ///
    /// `transient` in Java; the default Gson reflective adapter excludes `transient` fields from
    /// both serialisation and deserialisation (verified: `GsonProvider.GSON.toJson` on a
    /// `ScoringSettings` with this field set omits the key entirely — JDK 25,
    /// `freerouting-current-executable.jar` built 2026-08-27, see task-1-report.md Probe2). No
    /// custom `TypeAdapterFactory` is registered for `ScoringSettings` (only `RouterSettings`
    /// itself gets one, and it does not special-case this field), so unlike
    /// `RouterSettings.layers` this one is never round-tripped at all.
    #[serde(skip)]
    pub preferred_direction_trace_cost: Option<Vec<f64>>,

    /// The cost of 1 mm of trace length routed in the undesired direction, per layer.
    /// `ScoringSettings.java:31-33`. `transient` — see
    /// [`Self::preferred_direction_trace_cost`]'s doc comment; same treatment.
    #[serde(skip)]
    pub undesired_direction_trace_cost: Option<Vec<f64>>,

    /// The cost of 1 mm of trace length routed in the preferred direction. `ScoringSettings.java:35-37`.
    #[serde(
        rename = "default_preferred_direction_trace_cost",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub default_preferred_direction_trace_cost: Option<f64>,

    /// The cost of 1 mm of trace length routed in the undesired direction. `ScoringSettings.java:39-41`.
    #[serde(
        rename = "default_undesired_direction_trace_cost",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub default_undesired_direction_trace_cost: Option<f64>,

    /// The cost of a via on a regular (non-plane) net. `ScoringSettings.java:43-46`.
    #[serde(
        rename = "via_costs",
        alias = "viaCosts",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub via_costs: Option<i32>,

    /// The cost of a via placed on a plane. `ScoringSettings.java:48-49`.
    #[serde(
        rename = "plane_via_costs",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub plane_via_costs: Option<i32>,

    /// Base ripup cost for the first ripup-and-reroute pass. A routing-control parameter
    /// multiplied by the pass number inside `BatchAutorouter`; does NOT appear in the board-score
    /// formula. `ScoringSettings.java:51-58`.
    #[serde(
        rename = "start_ripup_costs",
        alias = "startRipupCosts",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub start_ripup_costs: Option<i32>,

    /// The penalty for an unrouted net. `ScoringSettings.java:60-61`.
    ///
    /// `Float` in Java (not `Double`) — `f32` here. `fr_dsn::java_float_to_string` is the
    /// formatter of record if this ever reaches text (plan ruling reference, brief).
    #[serde(
        rename = "unrouted_net_penalty",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub unrouted_net_penalty: Option<f32>,

    /// The penalty for a clearance violation. `ScoringSettings.java:63-64`. `Float` in Java.
    #[serde(
        rename = "clearance_violation_penalty",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub clearance_violation_penalty: Option<f32>,

    /// The penalty for a bend. `ScoringSettings.java:66-67`. `Float` in Java.
    #[serde(
        rename = "bend_penalty",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub bend_penalty: Option<f32>,

    /// Default bend cost/penalty per direction change on a layer. `ScoringSettings.java:69-70`.
    #[serde(
        rename = "default_bend_cost",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub default_bend_cost: Option<f64>,
}

impl ScoringSettings {
    /// Rust field names in `ScoringSettings.getDeclaredFields()` source order.
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
    ];
}

// renamed: clone -> derive(Clone) (ScoringSettings.java:82-97). Java's clone() falls back from
// Object.clone() (a shallow copy) to explicitly re-cloning the two array fields, because a shallow
// copy would otherwise share the same backing array between original and clone. A derived Clone
// on `Option<Vec<f64>>` always allocates a new Vec, so it already has the deep-copy semantics
// Java's override exists to add.
