//! `fr-settings`: the router configuration data model and merge engine — a faithful port of
//! `settings/{RouterSettings,LayerSettings,ScoringSettings,OptimizerSettings,FanoutSettings,
//! DesignRulesCheckerSettings,DebugSettings,SettingsSource,SettingsMerger}.java`, the in-scope
//! `settings/sources/**` classes, `util/ReflectionUtil.java` and
//! `autoroute/{BoardUpdateStrategy,ItemSelectionStrategy}.java` (freerouting, clone HEAD — see
//! the plan's Global Constraints for the jar-pinning exception, which does not apply here).
//!
//! See the plan doc (`docs/superpowers/plans/2026-08-28-plan-4-settings.md`) for scope,
//! architecture and rulings; Task 1's report (`.superpowers/sdd/2026-08-28-plan-4-settings/
//! task-1-report.md`) for this task's JVM verification evidence.
//!
//! This crate must not depend on `tracing`: `FRLogger` calls from the Java source are dropped or
//! become [`error::MergeReport`] entries, mirroring `fr-dsn`'s and `fr-board`'s convention (plan
//! Global Constraints).
//!
//! Task 1 scope: the crate skeleton, the data model (`RouterSettings` and its four nested value
//! types, `DesignRulesCheckerSettings`, `DebugSettings`), the two strategy enums, [`HostEnvironment`]
//! and the error types. The merge engine (`copy_fields`, `SettingsMerger`, the settings sources)
//! is Tasks 2 and onward.

pub mod drc_settings;
pub mod error;
pub mod fanout_settings;
pub mod host;
pub mod layer_settings;
pub mod optimizer_settings;
pub mod router_settings;
pub mod scoring_settings;

pub use drc_settings::{DebugSettings, DesignRulesCheckerSettings};
pub use error::{MergeError, MergeReport, SettingsError};
pub use fanout_settings::FanoutSettings;
pub use host::HostEnvironment;
pub use layer_settings::LayerSettings;
pub use optimizer_settings::{BoardUpdateStrategy, ItemSelectionStrategy, OptimizerSettings};
pub use router_settings::RouterSettings;
pub use scoring_settings::ScoringSettings;

/// Re-exports every public type of the crate, for `use fr_settings::prelude::*;`.
pub mod prelude {
    pub use crate::{
        BoardUpdateStrategy, DebugSettings, DesignRulesCheckerSettings, FanoutSettings,
        HostEnvironment, ItemSelectionStrategy, LayerSettings, MergeError, MergeReport,
        OptimizerSettings, RouterSettings, ScoringSettings, SettingsError,
    };
}
