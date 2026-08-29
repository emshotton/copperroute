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
//! and the error types. Task 2 adds the merge engine ([`copy_fields`] — `ReflectionUtil.copyFields`
//! as a hand-written per-struct field table, plus `RouterSettings::apply_new_values_from` and its
//! inverse `fill_absent_from`). Task 3 adds [`field_path`] — `ReflectionUtil.setFieldValue`, the string-keyed half of the
//! same Java class, which Task 7's environment-variable and CLI sources drive. Task 4 adds
//! `RouterSettings`'s null-coalescing accessors, their setter clamps, `java_clone` and
//! `validate`. Task 5 adds [`board_optimizations`] — `applyBoardSpecificOptimizations` and its
//! two companions, the only part of `RouterSettings` that reads a `fr_board::Board`.
//! Task 6 adds [`merger`] — `SettingsSource`, the priority ladder and `SettingsMerger` — and
//! [`sources`], the five in-scope `settings/sources/**` classes plus the
//! `DsnRouterSettings` ⇄ `RouterSettings` conversion pair Plan 3 ruling 5 parked here. Task 7
//! adds the environment-variable and CLI sources; Task 8 adds `resolve_headless`.

pub mod board_optimizations;
pub mod copy_fields;
pub mod drc_settings;
pub mod error;
pub mod fanout_settings;
pub mod field_path;
pub mod host;
pub mod layer_settings;
pub mod merger;
pub mod optimizer_settings;
pub mod router_settings;
pub mod scoring_settings;
pub mod sources;

pub use copy_fields::{CopyFields, JavaEnum, MergeMode};
pub use drc_settings::{DebugSettings, DesignRulesCheckerSettings};
pub use error::{MergeError, MergeReport, SettingsError};
pub use fanout_settings::FanoutSettings;
pub use field_path::{FieldKind, FieldSpec, set_field_value};
pub use host::HostEnvironment;
pub use layer_settings::LayerSettings;
pub use merger::{SettingsMerger, SettingsSource, SourceKind, priority};
pub use optimizer_settings::{BoardUpdateStrategy, ItemSelectionStrategy, OptimizerSettings};
pub use router_settings::{ExpansionCostFactor, RouterSettings};
pub use scoring_settings::ScoringSettings;

/// Re-exports every public type of the crate, for `use fr_settings::prelude::*;`.
pub mod prelude {
    pub use crate::sources::{
        ApiSettings, DefaultSettings, DsnFileSettings, RulesFileSettings, SesFileSettings,
    };
    pub use crate::{
        BoardUpdateStrategy, CopyFields, DebugSettings, DesignRulesCheckerSettings,
        ExpansionCostFactor, FanoutSettings, FieldKind, FieldSpec, HostEnvironment,
        ItemSelectionStrategy, JavaEnum, LayerSettings, MergeError, MergeMode, MergeReport,
        OptimizerSettings, RouterSettings, ScoringSettings, SettingsError, SettingsMerger,
        SettingsSource, SourceKind, priority, set_field_value,
    };
}
