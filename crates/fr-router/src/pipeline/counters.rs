//! `core/RouterCounters.java` — the per-pass counter DTO.

/// Port of `core.RouterCounters` (`core/RouterCounters.java:1-48`) — the per-pass counter DTO
/// `AutoroutePassRunner.updateProgress` fills and Java fires at every board-updated event
/// (`NamedAlgorithm.fireBoardUpdatedEvent`, `NamedAlgorithm.java:104-110`).
///
/// Ported as a **value**: Task 16's per-pass trace asserts it. The *firing* is
/// [`crate::pipeline::ProgressSink`]'s (ruling AK).
///
/// # The field list is the JVM's, in declaration order
///
/// Nine fields, and `phase` and `fanoutExtraViasCount` are the two an eyeball transcription
/// misses. The order below is `RouterCounters.class.getDeclaredFields()`' as reflected by
/// `scripts/differential/java/probes/P7T4Probe.java` and committed to
/// `crates/fr-router/tests/data/p7t4-stop-and-counters.txt`; `router_counters_field_list_matches_java`
/// asserts the port against that transcript rather than against this comment.
///
/// # Why the fields are `Option`s, and why the type is not `Copy`
///
/// Every Java field is a **boxed** `Integer` (or a `String`), and a fresh `new RouterCounters()`
/// leaves all nine `null` — the probe's nine `default=null` lines. The nullability is not
/// incidental: the two trailing fields are documented as optional (`:41`, `:45`), Gson omits
/// `null`s from the serialized progress payload, and a counter that has never been set is
/// distinguishable from one that is `0`. So the port's fields are `Option<i32>` /
/// `Option<String>` and its `Default` is all-`None`, which costs the type [`Copy`].
///
/// not ported: the `@SerializedName` annotations (`:10`, `:14`, `:18-20`, `:24`, `:28`, `:32`,
/// `:36-38`, `:42`, `:46`) and `Serializable` — this crate has no serialization surface, and the
/// snake_case JSON names plus the two `alternate` spellings (`routedCount`, `incompleteCount`)
/// belong to Plan 8's API layer.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RouterCounters {
    /// `passCount` (`:11`) — the current pass number.
    ///
    /// Java's comment above it (`:9`) is a copy-paste of the *next* field's ("items on the board
    /// that are in the queue to be routed in the current pass"). Harmless — no code reads a
    /// comment — so it is noted here rather than given a quirk row, per `docs/java-quirks.md`
    /// §Process notes' rule that only reachable or loop-bearing divergences earn one.
    pub pass_count: Option<i32>,
    /// `queuedToBeRoutedCount` (`:15`) — items queued to be routed in the current pass.
    pub queued_to_be_routed_count: Option<i32>,
    /// `routedCount` (`:21`) — items successfully routed in this pass.
    pub routed_count: Option<i32>,
    /// `skippedCount` (`:25`) — items skipped in this pass.
    pub skipped_count: Option<i32>,
    /// `rippedCount` (`:29`) — items ripped in this pass.
    pub ripped_count: Option<i32>,
    /// `failedToBeRoutedCount` (`:33`) — items that failed to be routed in this pass.
    pub failed_to_be_routed_count: Option<i32>,
    /// `incompleteCount` (`:39`) — items still in the ratsnest.
    pub incomplete_count: Option<i32>,
    /// `phase` (`:43`) — the optional phase marker, `"autoroute"` or `"fanout"`.
    pub phase: Option<String>,
    /// `fanoutExtraViasCount` (`:47`) — the optional fanout-only extra-via counter.
    pub fanout_extra_vias_count: Option<i32>,
}

impl RouterCounters {
    /// Java's declaration order, which `router_counters_field_list_matches_java` checks against
    /// the reflected list in `tests/data/p7t4-stop-and-counters.txt`. The Java spellings are kept
    /// verbatim so the assertion compares like with like.
    pub const JAVA_FIELD_NAMES: [&'static str; 9] = [
        "passCount",
        "queuedToBeRoutedCount",
        "routedCount",
        "skippedCount",
        "rippedCount",
        "failedToBeRoutedCount",
        "incompleteCount",
        "phase",
        "fanoutExtraViasCount",
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_counter_set_is_all_null() {
        // The probe's nine `default=null` lines.
        let counters = RouterCounters::default();
        assert_eq!(counters.pass_count, None);
        assert_eq!(counters.phase, None);
        assert_eq!(counters.fanout_extra_vias_count, None);
    }

    #[test]
    fn the_java_field_name_list_has_nine_entries() {
        assert_eq!(RouterCounters::JAVA_FIELD_NAMES.len(), 9);
    }
}
