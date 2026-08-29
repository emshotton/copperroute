//! `settings/OptimizerSettings.java` (121 lines) and the two strategy enums it references,
//! `autoroute/{BoardUpdateStrategy,ItemSelectionStrategy}.java`.

use serde::{Deserialize, Serialize};

/// Strategy for updating board state during routing passes (`autoroute/BoardUpdateStrategy.java`,
/// verbatim). Declaration order is preserved even though nothing in this crate's `copy_fields`
/// reaches it (`ReflectionUtil.getDefaultValue` takes `getEnumConstants()[0]` at
/// `ReflectionUtil.java:367-368` for *some* enum field elsewhere; the survey's §8 note asks that
/// every ported Java enum keep its declaration order on principle).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoardUpdateStrategy {
    /// Update immediately on any improvement.
    Greedy,
    /// Calculate updates in parallel and apply the single best improvement.
    GlobalOptimal,
    /// Combine `Greedy` and `GlobalOptimal`.
    Hybrid,
}

impl BoardUpdateStrategy {
    /// The exact Java enum constant name, for `copy_fields`'s (Task 2) `Enum.valueOf`-equivalent
    /// case-sensitive match (`ReflectionUtil.java:260-266`).
    pub fn java_name(self) -> &'static str {
        match self {
            Self::Greedy => "GREEDY",
            Self::GlobalOptimal => "GLOBAL_OPTIMAL",
            Self::Hybrid => "HYBRID",
        }
    }
}

/// Strategy for selecting and ordering items to optimize (`autoroute/ItemSelectionStrategy.java`,
/// verbatim). Declaration order preserved — see [`BoardUpdateStrategy`]'s doc comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemSelectionStrategy {
    /// Process items in board order.
    Sequential,
    /// Process items in a randomised order.
    Random,
    /// Process items in a priority order.
    Prioritized,
}

impl ItemSelectionStrategy {
    /// The exact Java enum constant name — see [`BoardUpdateStrategy::java_name`].
    pub fn java_name(self) -> &'static str {
        match self {
            Self::Sequential => "SEQUENTIAL",
            Self::Random => "RANDOM",
            Self::Prioritized => "PRIORITIZED",
        }
    }
}

/// Settings for the route optimizer which runs after auto-routing to reduce vias and trace
/// length (`OptimizerSettings.java:10-95`).
///
/// Every field follows the `RouterSettings` nullable-field contract (plan ruling 4).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct OptimizerSettings {
    /// Whether the route optimizer is enabled. `OptimizerSettings.java:13-14`.
    #[serde(rename = "enabled", default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,

    /// The identifier of the optimization algorithm to use (e.g. `"freerouting-optimizer"`).
    /// `OptimizerSettings.java:17-18`.
    #[serde(rename = "algorithm", default, skip_serializing_if = "Option::is_none")]
    pub algorithm: Option<String>,

    /// The maximum number of full optimization passes (sweeps over the board's items) to run.
    /// `OptimizerSettings.java:20-21`.
    #[serde(
        rename = "max_passes",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub max_passes: Option<i32>,

    /// The maximum number of item optimization attempts allowed. `OptimizerSettings.java:26-27`.
    #[serde(rename = "max_items", default, skip_serializing_if = "Option::is_none")]
    pub max_items: Option<i32>,

    /// The maximum number of threads to use for parallel route optimization.
    /// `OptimizerSettings.java:30-31`.
    #[serde(
        rename = "max_threads",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub max_threads: Option<i32>,

    /// The improvement threshold (as a fraction) below which the optimizer terminates.
    /// `OptimizerSettings.java:37-38`. `Float` in Java.
    #[serde(
        rename = "improvement_threshold",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub optimization_improvement_threshold: Option<f32>,

    /// The maximum number of consecutive item optimization failures allowed before aborting the
    /// current pass. `OptimizerSettings.java:42-43`.
    #[serde(
        rename = "max_consecutive_failures",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub max_consecutive_failures: Option<i32>,

    /// A multiplier applied to the base ripup cost at the start of optimization.
    /// `OptimizerSettings.java:48-49`.
    #[serde(
        rename = "additional_ripup_cost_factor_at_start",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub additional_ripup_cost_factor_at_start: Option<i32>,

    /// A cost discount factor applied when ripping up trace items (as opposed to vias).
    /// `OptimizerSettings.java:54-55`. `Float` in Java.
    #[serde(
        rename = "trace_ripup_cost_factor",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub trace_ripup_cost_factor: Option<f32>,

    /// The maximum number of autoroute passes allowed when ripping up and rerouting a single item
    /// during optimization. `OptimizerSettings.java:60-61`.
    #[serde(
        rename = "max_autoroute_passes",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub max_autoroute_passes: Option<i32>,

    /// The strategy to update the board. `OptimizerSettings.java:68-70`.
    ///
    /// `transient` in Java, and unlike `RouterSettings.layers` there is no custom
    /// `TypeAdapterFactory` for `OptimizerSettings` to re-read it — verified: `GsonProvider.GSON`
    /// never emits `board_update_strategy` for a `RouterSettings` whose `optimizer.boardUpdateStrategy`
    /// is set (JDK 25, `freerouting-current-executable.jar` built 2026-08-27, task-1-report.md
    /// Probe2). Full skip, both directions.
    #[serde(skip)]
    pub board_update_strategy: Option<BoardUpdateStrategy>,

    /// The ratio of `GlobalOptimal` to `Greedy` updates when using the `Hybrid` strategy (e.g.
    /// `"1:1"`). `OptimizerSettings.java:73-74`. `transient` — see
    /// [`Self::board_update_strategy`]'s doc comment; same treatment.
    #[serde(skip)]
    pub hybrid_ratio: Option<String>,

    /// The strategy for selecting and ordering the items to be optimized.
    /// `OptimizerSettings.java:79-80`. `transient` — same treatment.
    #[serde(skip)]
    pub item_selection_strategy: Option<ItemSelectionStrategy>,

    /// Timeout for the optimizer stage (e.g. `"5m"`, `"300s"`). `OptimizerSettings.java:83-84`.
    #[serde(rename = "timeout", default, skip_serializing_if = "Option::is_none")]
    pub timeout_string: Option<String>,
}

impl OptimizerSettings {
    /// Rust field names in `OptimizerSettings.getDeclaredFields()` source order.
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
        "board_update_strategy",
        "hybrid_ratio",
        "item_selection_strategy",
        "timeout_string",
    ];
}

// renamed: clone -> derive(Clone) (OptimizerSettings.java:98-113). Java's clone() falls back from
// Object.clone() and then explicitly re-copies the three transient fields defensively; Object.clone()
// (a raw memberwise copy) already copies transient fields regardless — transient only affects
// serialization, not Object.clone() — so the explicit re-copy is redundant, and a derived Clone
// over Copy fields reproduces the same result.
