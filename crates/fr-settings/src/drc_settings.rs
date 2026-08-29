//! `settings/DesignRulesCheckerSettings.java` (30 lines) and `settings/DebugSettings.java` (56
//! lines).
//!
//! Unlike `RouterSettings` and its four nested value types, neither of these participates in the
//! `SettingsMerger`/`copy_fields` nullable-field protocol (plan ruling 4 is scoped to
//! `RouterSettings`'s own tree): both hold plain Java primitives (`boolean`, `int`) with hard-coded
//! field initialisers, not boxed nullable wrappers, so the port keeps them as plain (non-`Option`)
//! fields with a `Default` impl mirroring those initialisers, exactly as the brief specifies for
//! `DesignRulesCheckerSettings`. `DebugSettings`'s field shape follows the same non-`Option`
//! precedent since Task 1's brief does not give it a distinct one.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Configuration controlling design-rule-check reporting
/// (`DesignRulesCheckerSettings.java:6-29`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignRulesCheckerSettings {
    /// Whether the checker is enabled. `DesignRulesCheckerSettings.java:9-11`.
    ///
    /// `transient boolean` in Java — a **primitive**, not `Boolean`. Gson's default reflective
    /// adapter excludes `transient` fields from both directions (no custom adapter is registered
    /// for this class), so this is never round-tripped through JSON; skipped here to match.
    #[serde(skip)]
    pub enabled: bool,

    /// Whether warning-level violations are included in reports. Defaults to `true`
    /// (`DesignRulesCheckerSettings.java:14`).
    #[serde(rename = "include_warnings")]
    pub include_warnings: bool,

    /// Whether error-level violations are included in reports. Defaults to `true`
    /// (`DesignRulesCheckerSettings.java:17`).
    #[serde(rename = "include_errors")]
    pub include_errors: bool,
}

impl Default for DesignRulesCheckerSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            include_warnings: true,
            include_errors: true,
        }
    }
}

impl DesignRulesCheckerSettings {
    /// Rust field names in `DesignRulesCheckerSettings.getDeclaredFields()` source order.
    pub const FIELD_NAMES: &'static [&'static str] =
        &["enabled", "include_warnings", "include_errors"];
}

// renamed: clone -> derive(Clone) (DesignRulesCheckerSettings.java:20-27). Java's clone() is a
// manual field-by-field copy of three primitive booleans (no super.clone() call at all — the
// class does not implement Cloneable's usual pattern); a derived Clone over Copy fields is the
// same operation.

/// Settings for debugging the routing engine (`DebugSettings.java:9-55`).
///
/// Out of the `RouterSettings` merge tree entirely (see module doc); fields are plain Java
/// primitives/collections with hard-coded initialisers, ported as plain (non-`Option`) fields
/// with a matching [`Default`] impl.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugSettings {
    /// `DebugSettings.java:11-12`.
    #[serde(rename = "enable_detailed_logging")]
    pub enable_detailed_logging: bool,

    /// `DebugSettings.java:14-15`.
    #[serde(rename = "single_step_execution")]
    pub single_step_execution: bool,

    /// `DebugSettings.java:17-18`.
    #[serde(rename = "trace_insertion_delay")]
    pub trace_insertion_delay: i32,

    /// Net numbers/names permitted by [`Self::is_net_permitted`]; an empty set permits every net.
    /// `DebugSettings.java:20-21`.
    #[serde(rename = "filter_by_net")]
    pub filter_by_net: HashSet<String>,

    /// Which router/optimizer operations are logged when [`Self::enable_detailed_logging`] is
    /// set. `DebugSettings.java:23-32`.
    #[serde(rename = "operation_filters")]
    pub operation_filters: Vec<String>,
}

impl Default for DebugSettings {
    /// `DebugSettings.java:23-32`'s field initialiser, verbatim order.
    fn default() -> Self {
        Self {
            enable_detailed_logging: false,
            single_step_execution: false,
            trace_insertion_delay: 0,
            filter_by_net: HashSet::new(),
            operation_filters: vec![
                "insert_trace_segment".to_string(),
                "remove_trace_segment".to_string(),
                "insert_trace_failure".to_string(),
                "remove_tail".to_string(),
                "insert_trace".to_string(),
                "remove_trace".to_string(),
                "insert_via".to_string(),
                "remove_via".to_string(),
            ],
        }
    }
}

impl DebugSettings {
    /// Rust field names in `DebugSettings.getDeclaredFields()` source order.
    pub const FIELD_NAMES: &'static [&'static str] = &[
        "enable_detailed_logging",
        "single_step_execution",
        "trace_insertion_delay",
        "filter_by_net",
        "operation_filters",
    ];

    /// `DebugSettings()`: the no-arg constructor is a no-op beyond the field initialisers already
    /// captured by [`Default`] (`DebugSettings.java:35`).
    pub fn new() -> Self {
        Self::default()
    }

    /// Checks whether the given net number or name is permitted by [`Self::filter_by_net`]. An
    /// empty filter permits every net (`DebugSettings.java:38-49`).
    ///
    /// Reproduces Java's asymmetric case handling verbatim: `net_name` is lower-cased before the
    /// lookup, but entries already in `filter_by_net` are matched as stored — Java's own comment
    /// (`DebugSettings.java:50-51`) notes external input should be lower-cased *before insertion*
    /// for case-insensitive matching to actually work, i.e. this is a documented caller
    /// responsibility, not a bug this port fixes.
    pub fn is_net_permitted(&self, net_number: i32, net_name: Option<&str>) -> bool {
        if self.filter_by_net.is_empty() {
            return true;
        }
        let net_no_str = net_number.to_string();
        self.filter_by_net.contains(&net_no_str)
            || self.filter_by_net.contains(&format!("Net #{net_number}"))
            || self.filter_by_net.contains(&format!("Net#{net_number}"))
            || net_name.is_some_and(|name| self.filter_by_net.contains(&name.to_lowercase()))
    }
}
