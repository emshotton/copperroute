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
//! adds the environment-variable and CLI sources, plus the dead `LegacyBridge` (plan ruling 8)
//! and `classify_de_arguments` (plan ruling 10); Task 8 adds [`resolve`] — `resolve_headless`, the linear
//! form of Java's two-merge headless precedence, and `resolve_scheduler_rules_path`. Task 10
//! adds [`json`] — `RouterSettings::from_json_str`/`to_json_string_pretty`, the Gson-compatible
//! JSON in and out `util/gson/{GsonProvider,RouterSettingsTypeAdapterFactory}.java` define.
//! Task 11 adds this crate's `README.md` and the out-of-scope roster at the foot of this file.
//!
//! `README.md` is the entry point for a reader: the API surface, the precedence Java actually
//! runs, the ported-vs-deferred table, the tests and the `p4t1` differential.

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

/// Re-exports every public type of the crate, for `use fr_settings::prelude::*;`.
pub mod prelude {
    pub use crate::sources::cli::{
        DeSlots, LegacyBridge, apply_command_line_arguments, classify_de_arguments,
    };
    pub use crate::sources::rules_file::apply_rules_file_against_board;
    pub use crate::sources::{
        ApiSettings, CliSettings, DefaultSettings, DsnFileSettings, EnvironmentVariablesSource,
        RulesFileSettings, SesFileSettings,
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

// ===============================================================================================
// The out-of-scope roster (plan ruling 9, Task 11)
// ===============================================================================================
//
// Everything below is a Java class or method that a reader of `settings/**` will expect to find
// in this crate and will not. Each line says why it is absent, so nobody re-derives the reason —
// and the `// not ported:` / `// added in Plan N:` markers are what `scripts/audit-port.sh`
// checks, so a deferral is a gate rather than a silence. Line numbers are the clone's HEAD
// (plan ruling 7). `scripts/audit-map/fr-settings.map` maps each class named here to this file.
//
// The audit matches **per method**, so a class with public methods gets one line per method.
//
// -----------------------------------------------------------------------------------------------
// `settings/sources/**` — the two tiers of the priority ladder that have no source here
// -----------------------------------------------------------------------------------------------
//
// `JsonFileSettings` (priority 10, `sources/JsonFileSettings.java:22`) reads `router` out of
// `~/…/freerouting.json` through `GsonProvider.GSON` (`:40-62`). Spec §2 gives this port no
// persistent configuration file, so the tier is reserved-and-empty: [`merger::priority::JSON_FILE`]
// keeps the number so nobody reuses it, [`merger::SourceKind::JsonFile`] keeps the identity, and
// `scripts/differential/java/P4T1.java` *proves* the tier contributes nothing by constructing the
// real class on an empty temporary directory and aborting with `JSON_SOURCE_NOT_EMPTY` if any
// leaf of its `getSettings()` is non-null. Its file-absent arm returns `new RouterSettings()`
// (`:41-44`), which is exactly the no-op this port hard-codes.
// not ported: JsonFileSettings.getSettings (:65-68) — spec §2, no persistent config file.
// not ported: JsonFileSettings.getSourceName (:70-73) — same; the name would be "freerouting.json".
// not ported: JsonFileSettings.getPriority (:75-78) — same; the number lives on as
//   `priority::JSON_FILE == 10`.
//
// `GuiSettingsSource` (priority **65**, `sources/GuiSettingsSource.java:36` — not the 50 the
// `SettingsSource` javadoc claims, quirk #138) is the Swing settings dialog's tier. There is no
// GUI in this port (plan Global Constraints), so it has no source; the number and the identity
// are kept for the same reason as above ([`merger::priority::GUI`], [`merger::SourceKind::Gui`]).
// not ported: GuiSettingsSource.getSettings (:47-51) — GUI, priority 65.
// not ported: GuiSettingsSource.setSettings (:53-60) — GUI, priority 65.
// not ported: GuiSettingsSource.getSourceName (:62-65) — GUI, priority 65.
// not ported: GuiSettingsSource.getPriority (:67-70) — GUI, priority 65.
//
// -----------------------------------------------------------------------------------------------
// `settings/**` — the fifteen non-router settings classes
// -----------------------------------------------------------------------------------------------
//
// `GlobalSettings` holds thirteen sibling settings objects beside its `RouterSettings` and
// `DesignRulesCheckerSettings`. None of them is a router setting, none is read by any code this
// port contains, and each is out of scope for the reason on its line. They are plain data
// classes (fields, no public methods), so they carry no audit obligation — the roster exists so
// that "why is there no `LoggingSettings` in `fr-settings`?" has a written answer.
//
// not ported: RuntimeEnvironment (settings/RuntimeEnvironment.java) — captured Java version, CPU
//   count and RAM, for diagnostics. The one datum this crate needs from it, the processor count,
//   is [`HostEnvironment`] (plan ruling 6).
// not ported: AppPaths (settings/AppPaths.java) — OS-standard config/data/log/cache directories.
//   Spec §2: no persistent config file and no user-data directory; see also
//   `GlobalSettings.getUserDataPath` in `sources/cli.rs`'s roster.
// not ported: LoggingSettings (settings/LoggingSettings.java) — console and file logging. This
//   crate must not depend on `tracing` and drops every `FRLogger` call (plan Global Constraints).
// not ported: StatisticsSettings (settings/StatisticsSettings.java) — activity counters and
//   timestamps (telemetry, spec §2).
// not ported: UserProfileSettings (settings/UserProfileSettings.java) — user identity and
//   e-mail preferences (telemetry, spec §2).
// not ported: UsageAndDiagnosticDataSettings (settings/UsageAndDiagnosticDataSettings.java) —
//   usage analytics and diagnostic-data upload (telemetry, spec §2).
// not ported: GoogleSheetsProviderSettings (settings/GoogleSheetsProviderSettings.java) — the
//   credentials and endpoint the analytics upload posts to (telemetry, spec §2).
// not ported: FeatureFlagsSettings (settings/FeatureFlagsSettings.java) — toggles for optional
//   application capabilities, none of them a router setting (GUI/server surface).
// not ported: GuiApplicationSettings (settings/GuiApplicationSettings.java) — the Swing session
//   (window geometry, input directory). No GUI (plan Global Constraints).
// not ported: ApiServerSettings (settings/ApiServerSettings.java) — the REST API server's host,
//   port and lifecycle (spec §2: no REST API in this port).
// not ported: ApiAuthenticationSettings (settings/ApiAuthenticationSettings.java) — the
//   authentication shared by the REST API and the HTTP MCP server (spec §2).
// not ported: NetworkSettings (settings/NetworkSettings.java) — HTTP proxy and custom TLS
//   truststore, used only by the API/telemetry clients (spec §2).
// not ported: RateLimitSettings (settings/RateLimitSettings.java) — the fixed-window rate limit
//   the REST API and MCP servers share (spec §2).
// not ported: McpServerSettings (settings/McpServerSettings.java) — the **HTTP** MCP transport's
//   host, port and authentication. Plan 8 builds the *stdio* MCP transport, which needs none of
//   this: no listener, no port, no auth.
//
// -----------------------------------------------------------------------------------------------
// `management/**` — the job queue that *runs* the merge sequence
// -----------------------------------------------------------------------------------------------
//
// not ported: management/jobs/RoutingJobScheduler.java — the job queue itself (thread pool, job
//   states, session bookkeeping) is out of scope per spec §2. What *is* ported is the settings
//   composition that happens inside its worker: `:103-170` is merge #2 and `:172-186` is the
//   post-merge `RulesReader.read` plus `applyBoardSpecificOptimizations`, and
//   [`resolve::resolve_headless`] reproduces that sequence exactly (plan ruling 1, and ruling N's
//   two `.rules` parses — quirk #142). [`resolve::resolve_scheduler_rules_path`] is `:113-152`'s
//   `job.rules ?? -dr ?? adjacent <design>.rules` chain on its own.
// not ported: management/jobs/RoutingJobSchedulerActionThread.java — the worker thread; see the
//   `parseTimespanString` note below for the one line of it this crate's data touches.
// not ported: management/jobs/ThreadActionListener.java — job lifecycle callbacks (no job queue).
// not ported: management/sessions/SessionManager.java — session ownership and the job lists
//   hanging off it (spec §2: no sessions, no REST API).
//
// -----------------------------------------------------------------------------------------------
// `util/**` — the two timeout strings, and the four Gson adapters
// -----------------------------------------------------------------------------------------------
//
// added in Plan 8: TextManager.parseTimespanString (util/TextManager.java:83) — `RouterSettings`
//   carries three timeout values as **strings** (`jobTimeoutString`, `optimizer.timeoutString`,
//   `fanout.timeoutString`) and Java parses them nowhere in the settings path. The only reader is
//   `RoutingJobSchedulerActionThread.threadAction` (`:44`), which parses `jobTimeoutString` and
//   then caps the result at `MAX_TIMEOUT` — 24 hours, `:24` — in `:45-52`. This crate therefore
//   carries the strings verbatim, as Java does, and Plan 8 parses them where Java parses them.
//
// `util/gson/**` is the JSON configuration [`json`] reproduces. `GsonProvider` and
// `RouterSettingsTypeAdapterFactory` are ported there; the factory's two methods and the three
// unrelated adapters are not, for the reasons below.
// not ported: RouterSettingsTypeAdapterFactory.create (util/gson/RouterSettingsTypeAdapterFactory
//   .java:21-69) — Gson's `TypeAdapterFactory` dispatch: it answers `null` for every type but
//   `RouterSettings` (`:24-27`) and otherwise wraps the delegate reflective adapter. `serde` binds
//   the (de)serialiser to the type at compile time, so there is no factory to port; what the
//   returned adapter *does* is [`json`], and its `write` half — pretty printing, Java number
//   rendering, non-finite refusal, the two extra escapes — is
//   `fr_dsn::format::json::JavaNumberFormatter`, moved there in Plan 5 (ruling 7) so `fr-drc` can
//   reuse it without depending on this crate; see `crates/fr-dsn/src/format/json.rs`.
// not ported: RouterSettingsTypeAdapterFactory.read (:48-67) — the two-pass read (lenient textual
//   pass, then a fresh `JsonTreeReader` at default strictness) is described and reproduced in
//   [`json`]; the entry point is [`RouterSettings::from_json_str`], and the `layers` re-read at
//   `:59-64` is Task 1's `#[serde(skip)]` split. Named separately from `create` because the audit
//   matches per method.
// not ported: InstantTypeAdapter.write (util/gson/InstantTypeAdapter.java:12-21) — `java.time
//   .Instant` as an ISO-8601 string. No `RouterSettings` field is an `Instant`; the type appears
//   only on `RoutingJob`/`StatisticsSettings`, both out of scope.
// not ported: InstantTypeAdapter.read (:22-31) — the same, reading.
// not ported: PathTypeAdapter.write (util/gson/PathTypeAdapter.java:13-23) — `java.nio.file.Path`
//   as a string. No `RouterSettings` field is a `Path` (`resultJsonPath` is a `String`).
// not ported: PathTypeAdapter.read (:24-33) — the same, reading.
// not ported: ByteArrayToBase64TypeAdapter.serialize (util/gson/ByteArrayToBase64TypeAdapter
//   .java:17-20) — `byte[]` as Base64. No `RouterSettings` field is a `byte[]`; the adapter
//   exists for `BoardFileDetails`, which is out of scope.
// not ported: ByteArrayToBase64TypeAdapter.deserialize (:22-27) — the same, reading.
