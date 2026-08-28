//! Specctra DSN/SES scope parsers (`io/specctra/parser/**`).
//!
// added in Plan 3 Task 4+: `ReadScopeParameter`/`WriteScopeParameter`, and one module per scope
// (structure, network, library, wiring, ...). This module currently holds only the placeholder
// `DsnRouterSettings` type that `BoardMetadata` (Task 1) needs in order to compile.
pub mod autoroute_settings;

pub use autoroute_settings::DsnRouterSettings;
