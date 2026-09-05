#![forbid(unsafe_code)]

pub mod board_optimizations;
pub mod copy_fields;
pub mod drc_settings;
pub mod error;
pub mod fanout_settings;
pub mod field_path;
pub mod host;
pub mod json;
pub mod layer_settings;
pub mod merger;
pub mod optimizer_settings;
pub mod resolve;
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
pub use resolve::{
    SettingsInputs, resolve_headless, resolve_scheduler_rules_path,
    resolve_scheduler_rules_path_with,
};
pub use router_settings::{ExpansionCostFactor, RouterSettings};
pub use scoring_settings::ScoringSettings;

pub mod prelude {
    pub use crate::sources::cli::{
        DeSlots, LegacyBridge, MULTIPLE_DSN_FILES, MULTIPLE_RULES_FILES, MULTIPLE_SES_FILES,
        MULTIPLE_SESSION_FILES, UNKNOWN_COMMAND_LINE_ARGUMENT_PREFIX, UNKNOWN_FILE_TYPE_PREFIX,
        UNKNOWN_FILE_TYPE_SUFFIX, UNKNOWN_SETTINGS_PROPERTY_PREFIX, apply_command_line_arguments,
        classify_de_arguments, classify_de_arguments_reporting, legacy_flag_value_is_consumed,
    };
    pub use crate::sources::rules_file::apply_rules_file_against_board;
    pub use crate::sources::{
        ApiSettings, CliSettings, DefaultSettings, DsnFileSettings, EnvironmentVariablesSource,
        JsonFileSettings, RulesFileSettings, SesFileSettings,
    };
    pub use crate::{
        BoardUpdateStrategy, CopyFields, DebugSettings, DesignRulesCheckerSettings,
        ExpansionCostFactor, FanoutSettings, FieldKind, FieldSpec, HostEnvironment,
        ItemSelectionStrategy, JavaEnum, LayerSettings, MergeError, MergeMode, MergeReport,
        OptimizerSettings, RouterSettings, ScoringSettings, SettingsError, SettingsInputs,
        SettingsMerger, SettingsSource, SourceKind, priority, resolve_headless,
        resolve_scheduler_rules_path, resolve_scheduler_rules_path_with, set_field_value,
    };
}

//   `:59-64` is Task 1's `#[serde(skip)]` split. Named separately from `create` because the audit
