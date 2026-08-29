//! `settings/sources/DefaultSettings.java` (169): the hardcoded defaults, priority 0.

use crate::{
    BoardUpdateStrategy, HostEnvironment, ItemSelectionStrategy, RouterSettings, SettingsSource,
    SourceKind, merger::priority,
};

/// `settings/sources/DefaultSettings.java`: the lowest-priority source, and the base every merge
/// starts from.
///
/// # Every default is JVM-pinned
///
/// The table in [`Self::new`] is `DefaultSettings.getSettings()` (`:96-155`) transcribed in
/// Java's own assignment order, and `tests/sources.rs::default_settings_pins_every_java_value`
/// checks it field by field against the `SProbe A` transcript.
///
/// # What is deliberately left absent
///
/// `layers`, `scoring.preferredDirectionTraceCost` and `scoring.undesiredDirectionTraceCost`
/// stay `None`, per the comment at `:88-93`: their sizes depend on the board, so they come from
/// [`super::DsnFileSettings`] and are finalised by `apply_board_specific_optimizations`.
/// `resultJsonPath` is never assigned at all. This is what makes quirk Q18 consequential — the
/// DSN source at priority 20 is the *first* writer of both cost arrays, and
/// `copy_fields` rule 5 then freezes them.
///
/// # One deviation, and why it is unobservable
///
/// Java's `getSettings()` builds a **fresh** `RouterSettings` on every call (JVM-verified,
/// `SProbe A.sameInstance = false`); [`SettingsSource::get_settings`] returns a borrow, so this
/// port computes the table once in [`Self::new`]. Nothing in the merge path mutates a source's
/// settings — [`crate::SettingsMerger::merge`] clones the base and applies the others on top —
/// so a shared instance and a fresh one are indistinguishable, which
/// `default_settings_are_not_mutated_by_a_merge` pins.
#[derive(Debug, Clone)]
pub struct DefaultSettings {
    settings: RouterSettings,
}

impl DefaultSettings {
    /// Penalty subtracted from the board score for each unrouted connection
    /// (`DefaultSettings.java:27`).
    pub const DEFAULT_UNROUTED_NET_PENALTY: f32 = 5_000_000.0;

    /// Penalty subtracted from the board score for each clearance (DRC) violation (`:34`).
    pub const DEFAULT_CLEARANCE_VIOLATION_PENALTY: f32 = 1_000_000.0;

    /// Penalty per bend in any trace (`:40`).
    pub const DEFAULT_BEND_PENALTY: f32 = 10.0;

    /// Cost per via on a regular (non-plane) net (`:46`).
    pub const DEFAULT_VIA_COSTS: i32 = 50;

    /// Reduced via cost for vias into a copper pour / power plane (`:52`).
    pub const DEFAULT_PLANE_VIA_COSTS: i32 = 5;

    /// Base ripup cost at the start of each ripup-and-reroute pass (`:60`).
    pub const DEFAULT_START_RIPUP_COSTS: i32 = 100;

    /// Cost multiplier per millimetre of trace in the preferred direction (`:67`).
    pub const DEFAULT_PREFERRED_DIRECTION_TRACE_COST: f64 = 1.0;

    /// Cost multiplier per millimetre of trace against the preferred direction (`:75`).
    pub const DEFAULT_UNDESIRED_DIRECTION_TRACE_COST: f64 = 1.0;

    /// Default copper-to-board-edge clearance in micrometres, 0.5 mm (`:78`).
    pub const DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM: f64 = 500.0;

    /// Default drill-hole-to-copper clearance in micrometres (`:81`).
    pub const DEFAULT_HOLE_CLEARANCE_UM: f64 = 0.0;

    /// `DefaultSettings.PRIORITY` (`:83`).
    const PRIORITY: i32 = priority::DEFAULT;

    /// Builds the default table (`DefaultSettings.getSettings`, `:86-158`).
    ///
    /// The two `Math.max(1, Runtime.getRuntime().availableProcessors() - 1)` sites (`:106`,
    /// `:134`) read `host` instead (plan ruling 6 — no machine-dependent default anywhere but
    /// [`HostEnvironment::detect`]).
    #[must_use]
    pub fn new(host: &HostEnvironment) -> Self {
        let mut settings = RouterSettings::new();

        settings.enabled = Some(true);
        settings.algorithm = Some(RouterSettings::ALGORITHM_CURRENT.to_string());
        settings.job_timeout_string = Some("12:00:00".to_string());
        settings.max_passes = Some(9999);
        settings.max_items = Some(i32::MAX);
        settings.trace_pull_tight_accuracy = Some(500);
        settings.vias_allowed = Some(true);
        settings.automatic_neckdown = Some(true);
        settings.save_intermediate_stages = Some(false);
        settings.ignore_net_classes = Some(Vec::new());
        settings.max_threads = Some(host.default_max_threads());
        settings.copper_to_edge_clearance_um = Some(Self::DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM);
        settings.hole_clearance_um = Some(Self::DEFAULT_HOLE_CLEARANCE_UM);
        settings.neck_width_um = Some(0.0);
        settings.strict_drc = Some(false);

        // `layers` is left absent intentionally (:112-114).

        let fanout = settings
            .fanout
            .as_mut()
            .expect("RouterSettings::new sets it");
        fanout.enabled = Some(true);
        fanout.max_passes = Some(20);
        fanout.max_milliseconds_per_pin = Some(10_000);
        fanout.ripup_allowed = Some(true);
        fanout.min_escape_length_mm = Some(2.5);
        fanout.max_escape_length_mm = Some(4.5);
        fanout.start_via_diameter_mm = Some(0.250);
        fanout.end_via_diameter_mm = Some(0.250);
        fanout.pin_sorting_order = Some("outer_first".to_string());
        fanout.max_items = Some(i32::MAX);
        fanout.fallback_to_board_vias = Some(true);

        let optimizer = settings
            .optimizer
            .as_mut()
            .expect("RouterSettings::new sets it");
        optimizer.enabled = Some(true);
        optimizer.algorithm = Some("freerouting-optimizer".to_string());
        optimizer.max_passes = Some(100);
        optimizer.max_items = Some(i32::MAX);
        optimizer.max_threads = Some(host.default_max_threads());
        optimizer.optimization_improvement_threshold = Some(0.01);
        optimizer.board_update_strategy = Some(BoardUpdateStrategy::Greedy);
        optimizer.hybrid_ratio = Some("1:1".to_string());
        optimizer.item_selection_strategy = Some(ItemSelectionStrategy::Prioritized);
        optimizer.additional_ripup_cost_factor_at_start = Some(10);
        optimizer.trace_ripup_cost_factor = Some(0.6);
        optimizer.max_autoroute_passes = Some(6);
        optimizer.max_consecutive_failures = Some(50);

        // The two per-layer cost arrays are omitted for the same reason `layers` is (:144-145).
        let scoring = settings
            .scoring
            .as_mut()
            .expect("RouterSettings::new sets it");
        scoring.default_preferred_direction_trace_cost =
            Some(Self::DEFAULT_PREFERRED_DIRECTION_TRACE_COST);
        scoring.default_undesired_direction_trace_cost =
            Some(Self::DEFAULT_UNDESIRED_DIRECTION_TRACE_COST);
        scoring.via_costs = Some(Self::DEFAULT_VIA_COSTS);
        scoring.plane_via_costs = Some(Self::DEFAULT_PLANE_VIA_COSTS);
        scoring.start_ripup_costs = Some(Self::DEFAULT_START_RIPUP_COSTS);
        scoring.unrouted_net_penalty = Some(Self::DEFAULT_UNROUTED_NET_PENALTY);
        scoring.clearance_violation_penalty = Some(Self::DEFAULT_CLEARANCE_VIOLATION_PENALTY);
        scoring.bend_penalty = Some(Self::DEFAULT_BEND_PENALTY);
        scoring.default_bend_cost = Some(0.0);

        Self { settings }
    }
}

impl SettingsSource for DefaultSettings {
    fn get_settings(&self) -> Option<&RouterSettings> {
        Some(&self.settings)
    }

    fn get_source_name(&self) -> String {
        "Default Settings".to_string()
    }

    fn get_priority(&self) -> i32 {
        Self::PRIORITY
    }

    fn kind(&self) -> SourceKind {
        SourceKind::Default
    }
}
