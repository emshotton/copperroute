//! Specctra DSN/SES scope parsers (`io/specctra/parser/**`).
//!
//! [`scope_parameter`] holds `ReadScopeParameter`/`WriteScopeParameter` (`ReadScopeParameter.java`
//! / `WriteScopeParameter.java`) plus the generic `ScopeKeyword.readScope`/`skipScope` dispatch
//! loop (`ScopeKeyword.java`); [`dsn_file`] holds `DsnFile`'s scalar scope helpers
//! (`DsnFile.java`). Everything else in this module is one file per Specctra scope class,
//! matching the Java package's own one-class-per-file layout: [`structure`], [`network`],
//! [`wiring`], [`library`], [`part_library`], [`placement`], [`header`] (the last groups
//! `Parser.java`/`Resolution.java`/`PlaceControl.java` — three small, closely related header
//! scopes with no dedicated file of their own in the plan's file structure).
//!
//! Every scope-reader function below except the ones `autoroute_settings.rs` and this task's
//! `dsn_file.rs`/`scope_parameter.rs` already implement is a **stub**
// added in Plan 3: ScopeKeyword.readScope dispatch table (this module's scope-reader stubs)
//! that just calls [`scope_parameter::skip_scope`] — later tasks replace the body, not the
//! signature, so [`scope_parameter::read_scope`]'s dispatch `match` does not change shape again.
pub mod autoroute_settings;
pub mod dsn_file;
pub mod header;
pub mod library;
pub mod network;
pub mod part_library;
pub mod placement;
pub mod scope_parameter;
pub mod structure;
pub mod wiring;

pub use autoroute_settings::DsnRouterSettings;
