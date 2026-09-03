//! `settings/RouterSettings.java` (966 lines): mutable router configuration assembled from the
//! configured settings sources.
//!
//! Task 1 ported the data model — the field table below, `new()` (Java's no-arg constructor),
//! and `FIELD_NAMES`. Task 2 added `get_layer_count`/`set_layer_count` and
//! `apply_new_values_from` (the rest of the merge engine lives in [`crate::copy_fields`] and
//! [`crate::field_path`]). Task 4 added the null-coalescing accessors, their setter clamps,
//! [`RouterSettings::java_clone`] and [`RouterSettings::validate`].
//!
//! Task 5 added `applyBoardSpecificOptimizations`, `applyBoardSpecificOptimizationsIfNeeded` and
//! `areBoardSpecificTraceCostsApplied` — in [`crate::board_optimizations`], not here, because
//! they are the only `RouterSettings` methods that need `fr-board`.
//!
//! Five of this class's public methods are still unported, and Task 6 established that **no
//! source in this plan reaches any of them**: `DefaultSettings` assigns `algorithm`,
//! `optimizer.enabled` and `fanout.enabled` as fields, not through setters
//! (`DefaultSettings.java:97,130,117`), and `CliSettings`/`EnvironmentVariablesSource` go through
//! `ReflectionUtil.setFieldValue`, which writes fields directly too. Their real callers, read out
//! of the Java at the clone's HEAD:
//!
//! | Method | Java site | Only callers |
//! |---|---|---|
//! | `setAlgorithm` | `:221-227` | none — no caller anywhere in the Java tree, test or otherwise |
//! | `setOptimizerEnabled` | `:230-238` | `gui/windows/routing/WindowAutorouteParameter.java:515` |
//! | `setFanoutEnabled` | `:583-592` | `WindowAutorouteParameter.java:507` |
//! | `getRunFanout` | `:573-575` | `WindowAutorouteParameter.java:590` |
//! | `isFanoutEnabled` | `:578-580` | `autoroute/pipeline/{RoutingPipeline,BatchAutorouter,AutorouteBatchLoop}.java` — **ported in Task 6 fix round 1** |
//!
//! `isFanoutEnabled` had a real headless caller, so it is ported (see [`Self::is_fanout_enabled`],
//! whose doc comment records the `getRunFanout` default asymmetry, quirk #139). The other four
//! are GUI-only with no headless caller, so none of them is ported; the `// not ported:` markers
//! below record each one for `scripts/audit-port.sh`.
//!
// not ported: setAlgorithm (RouterSettings.java:221-227) — no non-test caller.
// not ported: setOptimizerEnabled (RouterSettings.java:230-238) — GUI only: WindowAutorouteParameter.java:515.
// not ported: getRunFanout (RouterSettings.java:573-575) — GUI only, opposite default from isFanoutEnabled (quirk #139).
// not ported: setFanoutEnabled (RouterSettings.java:583-592) — GUI only: WindowAutorouteParameter.java:507.
//!
//! ## Transient-field serde treatment, JVM-verified
//!
//! Four of this struct's fields are `transient` in Java: `max_items`, `layers`,
//! `save_intermediate_stages`, `ignore_net_classes`. `util/gson/GsonProvider.java` builds its
//! `Gson` with no `excludeFieldsWithModifiers` override, so the *default* exclusion strategy
//! applies: `transient` fields are dropped from **both** serialisation and deserialisation by the
//! delegate reflective adapter. `util/gson/RouterSettingsTypeAdapterFactory.java` registers one
//! custom `TypeAdapterFactory`, scoped to `RouterSettings` itself (`isAssignableFrom` check), whose
//! `read()` explicitly re-populates `settings.layers` from the raw JSON tree after the delegate
//! has already dropped it — but whose `write()` never adds `layers` back to the tree the delegate
//! produced.
//!
//! This was verified against the real JVM, not just read from source (JDK 25,
//! `freerouting-current-executable.jar` built 2026-08-27, `java.awt.headless=true`): serialising a
//! `RouterSettings` with all four transient fields set to non-null values produces JSON containing
//! none of their keys at all; deserialising `{"max_items":99,"layers":[...],
//! "save_intermediate_stages":true,"ignore_net_classes":["x","y"]}` leaves `maxItems`,
//! `saveIntermediateStages` and `ignoreNetClasses` `null` but populates `layers` from the one
//! layer given. See task-1-report.md ("Probe" transcript) for the full driver and output.
//!
//! Consequently:
//! - `max_items`, `save_intermediate_stages`, `ignore_net_classes` get `#[serde(skip)]` — full
//!   skip, matching "never round-tripped".
//! - `layers` gets `#[serde(skip_serializing)]` only — deserialisation stays enabled, matching
//!   the custom re-read.
//!
//! Task 10 finished the picture on the write side: every field also carries
//! `skip_serializing_if = "Option::is_none"` (Gson's default is `serializeNulls = false`), and
//! `fanout`/`optimizer`/`scoring` carry a `default = "…"` that reproduces Gson running
//! `RouterSettings()` before the reflective adapter writes anything. See [`crate::json`] for the
//! whole contract, `Double.toString`/`Float.toString` number formatting included.
//!
//! **Correction to Task 10's brief:** its `emitted_key_set_matches_gson` key list contains
//! `"layers"`. It must not — `layers` is `transient` and the factory's `write()` never adds it
//! back, which `RouterSettingsSerializationTest` asserts directly
//! (`assertFalse(json.contains("\"layers\""))`) and `JProbe.java` block A re-verifies.
//!
//! **Correction to the task brief's illustrative code snippet:** the brief's snippet attaches
//! `skip_serializing` to `max_items`/`save_intermediate_stages`/`ignore_net_classes` (implying
//! they are readable, not writable) and no skip at all to `layers` (implying it round-trips both
//! ways) — the reverse of what the JVM probe above shows. Java wins per the plan's Global
//! Constraints; this file follows the verified behaviour and the correction is called out in
//! task-1-report.md.

use serde::{Deserialize, Serialize};

use crate::{FanoutSettings, HostEnvironment, LayerSettings, OptimizerSettings, ScoringSettings};

/// Mutable router configuration assembled from the configured settings sources
/// (`RouterSettings.java:13-111`).
///
/// Every field is `Option<T>` (plan ruling 4): nullability *is* the merge protocol
/// (`SettingsMerger.java:22-31` — a source's `null` means "no opinion", not "off"). Field
/// declaration order matches Java's `getDeclaredFields()` order exactly, because the merge
/// engine's `copy_fields` (Task 2) iterates it in that order; see [`Self::FIELD_NAMES`].
///
/// **To write this type as JSON, call [`Self::to_json_string_pretty`], never
/// `serde_json::to_string_pretty`**: the latter formats floats with Rust's shortest
/// round-trip formatter, not `Double.toString`/`Float.toString`, and does not escape
/// `U+2028`/`U+2029` — so it is not Gson-compatible. See [`crate::json`].
///
/// **The derived [`Clone`] is not `RouterSettings.clone()`.** Java's `clone()` is a hand-written
/// method that drops `resultJsonPath` (quirk #114) and turns a null `optimizer`/`scoring`/
/// `fanout` into a *fresh default object* — [`Self::java_clone`]. Use `java_clone` wherever the
/// Java call site calls `clone()` (`SettingsMerger.merge`'s choice of merge base,
/// `SettingsMerger.java:162`); the derive is a plain structural copy for everything else.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct RouterSettings {
    /// `RouterSettings.java:20-21`.
    #[serde(rename = "enabled", default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,

    /// `RouterSettings.java:23-24`.
    #[serde(rename = "algorithm", default, skip_serializing_if = "Option::is_none")]
    pub algorithm: Option<String>,

    /// Configuration for the SMD-pin fanout pre-pass. `RouterSettings.java:26-28`.
    #[serde(
        rename = "fanout",
        default = "crate::json::constructed_fanout",
        skip_serializing_if = "Option::is_none"
    )]
    pub fanout: Option<FanoutSettings>,

    /// `RouterSettings.java:30-31`.
    #[serde(
        rename = "copper_to_edge_clearance_um",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub copper_to_edge_clearance_um: Option<f64>,

    /// `RouterSettings.java:33-34`.
    #[serde(
        rename = "hole_clearance_um",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub hole_clearance_um: Option<f64>,

    /// Opt-in width necking: when a connection fails at its net-class trace width, retry it once
    /// with all trace half-widths clamped to this width (in micrometers). 0/absent = off.
    /// `RouterSettings.java:36-43`.
    #[serde(
        rename = "neck_width_um",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub neck_width_um: Option<f64>,

    /// When true, a routed connection whose newly inserted traces/vias carry clearance
    /// violations is ripped up again and counted as not routed for that pass.
    /// `RouterSettings.java:45-52`.
    #[serde(
        rename = "strict_drc",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub strict_drc: Option<bool>,

    /// `RouterSettings.java:54-55`.
    #[serde(
        rename = "job_timeout",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub job_timeout_string: Option<String>,

    /// `RouterSettings.java:57-58`.
    #[serde(
        rename = "max_passes",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub max_passes: Option<i32>,

    /// `transient` — never round-tripped through JSON. See the module doc comment.
    /// `RouterSettings.java:60-61`.
    #[serde(skip)]
    pub max_items: Option<i32>,

    /// `transient`, but specially re-read on deserialize (not on serialize). See the module doc
    /// comment. `RouterSettings.java:63-64`.
    #[serde(rename = "layers", default, skip_serializing)]
    pub layers: Option<Vec<LayerSettings>>,

    /// `transient` — never round-tripped through JSON. See the module doc comment.
    /// `RouterSettings.java:66-67`.
    #[serde(skip)]
    pub save_intermediate_stages: Option<bool>,

    /// `transient` — never round-tripped through JSON. See the module doc comment.
    /// `RouterSettings.java:69-70`.
    #[serde(skip)]
    pub ignore_net_classes: Option<Vec<String>>,

    /// The accuracy of the pull tight algorithm. `RouterSettings.java:72-76`.
    #[serde(
        rename = "trace_pull_tight_accuracy",
        alias = "tracePullTightAccuracy",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub trace_pull_tight_accuracy: Option<i32>,

    /// `Boolean` field under a plural-sounding JSON key: Java's own `@SerializedName` renames it
    /// to `allowed_via_types` (`RouterSettings.java:78-79`) even though the Java field name and
    /// type both read as a single flag, not a collection. Not a bug — just the serialised name —
    /// so no `// Java bug:` marker; this doc note is the record the controller decision asked
    /// for.
    #[serde(
        rename = "allowed_via_types",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub vias_allowed: Option<bool>,

    /// If true, the trace width at static pins smaller than the trace width is lowered
    /// automatically to the pin width, if necessary. `RouterSettings.java:81-88`.
    #[serde(
        rename = "automatic_neckdown",
        alias = "automaticNeckdown",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub automatic_neckdown: Option<bool>,

    /// `RouterSettings.java:90-91`.
    #[serde(
        rename = "optimizer",
        default = "crate::json::constructed_optimizer",
        skip_serializing_if = "Option::is_none"
    )]
    pub optimizer: Option<OptimizerSettings>,

    /// `RouterSettings.java:93-94`.
    #[serde(
        rename = "scoring",
        default = "crate::json::constructed_scoring",
        skip_serializing_if = "Option::is_none"
    )]
    pub scoring: Option<ScoringSettings>,

    /// `RouterSettings.java:96-97`.
    #[serde(
        rename = "max_threads",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub max_threads: Option<i32>,

    /// Optional path for a machine-readable routing result manifest (JSON). Used by benchmark and
    /// autopilot harnesses; ignored when `None` or blank. `RouterSettings.java:99-104`.
    #[serde(
        rename = "result_json",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub result_json_path: Option<String>,

    /// When `Some(true)`, per-layer trace costs were initialized from board geometry (or set
    /// explicitly by the user) and must not be overwritten by subsequent calls to
    /// `apply_board_specific_optimizations` (Task 2/3). `private transient` in Java
    /// (`RouterSettings.java:106-111`) — private, so `copy_fields` must skip it regardless of
    /// order (plan ruling 4: non-`public` fields are never copied); kept non-`pub` here for the
    /// same reason. `transient`, so also never round-tripped through JSON.
    #[serde(skip)]
    pub(crate) board_specific_trace_costs_applied: Option<bool>,

    /// The `optChangedArea` pull-tight budget, in milliseconds. **The port's own field: Java has
    /// no counterpart**, and that is the whole point of it.
    ///
    // Java bug: `TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP` is declared `static final int … = 1000` with
    // a constant initialiser at all four sites, so `javac` inlines it (`javap -c -p` on the
    // shipping jar shows `sipush 1000` before every `optChangedArea` call and no `getstatic`);
    // the fields are dead, reflection cannot reach them, and no flag exists. The only way to
    // observe or disable the limit in the jar is to recompile — quirk #234.
    // fixed: T1 (#234) — this field is the register's own suggested fix, "make the limit a
    // settings field so a reproducible run is expressible", and it is why the port's
    // `RouterBudget::default()` may safely be `0`: the 1000 ms behaviour is not deleted, it is
    // moved from an inlined constant to something a user can ask for.
    ///
    /// `--router.opt_changed_area_ms=1000` (or `{"opt_changed_area_ms": 1000}`) reproduces the
    /// jar's behaviour, including its non-reproducibility. `0` — the default when this is unset —
    /// is Java's own "off" value: `TraceTightener`'s constructor builds a `TimeLimit` only
    /// `if (timeLimit > 0)` (`board/optimize/TraceTightener.java:73-77`), so the pull-tight simply
    /// runs to completion, which is strictly more work and cannot lengthen a trace. A negative
    /// value takes the same branch (`> 0`, not `!= 0`) and is also "off".
    ///
    /// It is deliberately **last** in [`RouterSettings::FIELD_NAMES`], after every Java field, so
    /// that Java's `getDeclaredFields()` order — which the merge engine's `copy_fields` iterates
    /// and `struct_shape.rs` pins — is unchanged by its arrival.
    #[serde(
        rename = "opt_changed_area_ms",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub opt_changed_area_ms: Option<i32>,
    // Java's `private transient PropertyChangeSupport pcs` (RouterSettings.java:114) is GUI
    // bidirectional-binding plumbing with no headless use; the plan's Global Constraints ("No
    // GUI, no PropertyChangeSupport") drop it entirely rather than modeling it as an unused
    // field.
    //
    // not ported: addPropertyChangeListener, removePropertyChangeListener
    // (RouterSettings.java:152-165) — both exist only to register/unregister listeners on `pcs`;
    // with no GUI and no `pcs` field, neither has anything to do in this port.
}

impl RouterSettings {
    /// Rust field names in `RouterSettings.getDeclaredFields()` source order (constants, the two
    /// constructors, and the dropped `pcs` field excluded — see the struct's trailing doc
    /// comments), **followed by the port's own fields**. Pins the order the merge engine's
    /// `copy_fields` (Task 2) must iterate in.
    ///
    /// Java's twenty-one names come first and in Java's order, so `copy_fields` iterates exactly
    /// as `ReflectionUtil` does. The port's own additions are appended after them, where they
    /// cannot shift a Java field's position; there is one today,
    /// [`RouterSettings::opt_changed_area_ms`] (Plan 9 Task 1, #234).
    pub const FIELD_NAMES: &'static [&'static str] = &[
        "enabled",
        "algorithm",
        "fanout",
        "copper_to_edge_clearance_um",
        "hole_clearance_um",
        "neck_width_um",
        "strict_drc",
        "job_timeout_string",
        "max_passes",
        "max_items",
        "layers",
        "save_intermediate_stages",
        "ignore_net_classes",
        "trace_pull_tight_accuracy",
        "vias_allowed",
        "automatic_neckdown",
        "optimizer",
        "scoring",
        "max_threads",
        "result_json_path",
        "board_specific_trace_costs_applied",
        // The port's own, after every Java name. See this const's doc comment.
        "opt_changed_area_ms",
    ];

    /// Mirrors Java's no-arg constructor (`RouterSettings()`, `RouterSettings.java:119-124`),
    /// which allocates the three nested settings objects and leaves every other field `null`.
    /// `SettingsMergerTest.java:34-41` (`emptySourcesList`) pins that `new SettingsMerger().merge()`
    /// gives `maxPasses == null` — i.e. distinct from this constructor's non-null nested objects —
    /// so `Self::default()` (all fields `None`, including the three nested ones) and `Self::new()`
    /// must disagree, and both are pinned by `tests/struct_shape.rs`.
    pub fn new() -> Self {
        Self {
            fanout: Some(FanoutSettings::default()),
            optimizer: Some(OptimizerSettings::default()),
            scoring: Some(ScoringSettings::default()),
            ..Default::default()
        }
    }

    /// `RouterSettings.getLayerCount` (`RouterSettings.java:442-448`): the configured layer count,
    /// `0` when `layers` is absent.
    ///
    /// Ported here rather than with the rest of the per-layer accessors (Task 3/5) because rule
    /// 6's object-array merge is meaningless to test without it — `SettingsMergerTest.java:259-277`
    /// asserts on it directly.
    pub fn get_layer_count(&self) -> usize {
        self.layers.as_ref().map_or(0, Vec::len)
    }

    /// `RouterSettings.setLayerCount` (`RouterSettings.java:455-477`): sizes `layers` and seeds
    /// the two per-layer cost arrays with a neutral `1.0`.
    ///
    /// Reallocating `layers` clears [`Self::board_specific_trace_costs_applied`] (`:456-457`), but
    /// the per-layer reset at `:471-477` runs on **every** call, so an existing `layers` of the
    /// right length still has `routable` forced back to `Some(true)` and the other two fields back
    /// to `None`. `scoring.preferredDirectionTraceCost`/`undesiredDirectionTraceCost` are likewise
    /// replaced unconditionally (`:466-467`), which is the one place a populated cost array is
    /// discarded despite rule 5's first-writer-wins.
    ///
    /// Java takes an `int` and would throw `NegativeArraySizeException` for a negative count; no
    /// caller can reach that (`board.getLayerCount()` and a parsed layer list are both
    /// non-negative), so the port takes a `usize`.
    ///
    /// Ported here for the same reason as [`Self::get_layer_count`] — both Java merge tests build
    /// their fixtures with it.
    ///
    /// Java bug: setLayerCount (RouterSettings.java:455-478) — the asymmetry above is a defect,
    /// not a design: an unchanged layer count still discards every cost the merge just parsed
    /// (`:466-477`) while leaving `boardSpecificTraceCostsApplied` at `true` (`:457` is inside
    /// the reallocation branch), so `applyBoardSpecificOptimizationsIfNeeded` will not recompute
    /// them either. Live via `DsnFileSettings.java:46-48`. JVM-verified in Task 4 against the
    /// clone-HEAD jar; see `docs/java-quirks.md` #126 and the two unit tests at the bottom of
    /// this file.
    pub fn set_layer_count(&mut self, layer_count: usize) {
        if !matches!(&self.layers, Some(layers) if layers.len() == layer_count) {
            self.board_specific_trace_costs_applied = Some(false);
            self.layers = Some(vec![LayerSettings::default(); layer_count]);
        }
        let scoring = self.scoring.get_or_insert_with(ScoringSettings::default);
        scoring.preferred_direction_trace_cost = Some(vec![1.0; layer_count]);
        scoring.undesired_direction_trace_cost = Some(vec![1.0; layer_count]);
        for layer in self.layers.as_mut().expect("set above").iter_mut() {
            layer.routable = Some(true);
            layer.preferred_direction_horizontal = None;
            layer.bend_cost = None;
        }
    }

    // ---- null-coalescing accessors and their setter clamps ----------------------------------
    //
    // Every one of the accessors below is a transcription of a `RouterSettings.java` method whose
    // whole job is to turn a nullable field into a non-null answer. The `FRLogger.warn` calls the
    // Java originals make on an out-of-range layer index are dropped (plan Global Constraints: no
    // `FRLogger`), and the `pcs.firePropertyChange` calls the setters make are dropped with them.
    //
    // not ported: firePropertyChange — `RouterSettings.setMaxPasses` (:167-174),
    // `setMaxThreads` (:176-187), `setJobTimeoutString` (:189-196), `setEnabled` (:198-205),
    // `setViasAllowed` (:207-214), `setAlgorithm` (:221-228), `setOptimizerEnabled` (:230-238),
    // `setFanoutEnabled` (:583-592) and `applyNewValuesFrom` (:907-929) each end by notifying
    // `pcs`. The plan's Global Constraints drop `PropertyChangeSupport` entirely (GUI-only
    // bidirectional binding), so the notification has nothing to notify and the setters are
    // plain field writes.
    //
    // **Layer indices are `usize`, so Java's `layer < 0` half of every range guard is
    // unrepresentable** rather than reproduced. Java answers `0.0`/`false`/no-op for a negative
    // index exactly as it does for a too-large one, so the two arms would agree anyway; the
    // `layer >= getLayerCount()` half is ported literally and is what every out-of-range test
    // below exercises.
    //
    // **`layers` elements are `LayerSettings`, not `Option<LayerSettings>`** (Task 1's shape), so
    // Java's `layers[layer] == null` branches (:657-659, :696, :741-743) are unreachable here.
    // They are *not* dropped behaviour: a null element and a `LayerSettings` with all three
    // fields null give identical answers in every one of those methods (`true`,
    // `defaultBendCost`, `layer % 2 == 1`), which is exactly what a `LayerSettings::default()`
    // produces. The `layers[layer] == null -> new LayerSettings()` instantiation the three
    // setters do (:660-662, :697-699, :744-746) is likewise a no-op here.

    /// `RouterSettings.MIN_BEND_COST` (`RouterSettings.java:17`).
    pub const MIN_BEND_COST: f64 = 0.0;

    /// `RouterSettings.MAX_BEND_COST` (`RouterSettings.java:18`).
    pub const MAX_BEND_COST: f64 = 9.9;

    /// `RouterSettings.ALGORITHM_CURRENT` (`RouterSettings.java:15`).
    pub const ALGORITHM_CURRENT: &'static str = "freerouting-router";

    /// `RouterSettings.ALGORITHM_V19` (`RouterSettings.java:16`).
    pub const ALGORITHM_V19: &'static str = "freerouting-router-v19";

    /// `RouterSettings.getNeckWidthUm` (`:527-529`): the configured necking width, or `0.0` when
    /// absent or non-positive.
    pub fn get_neck_width_um(&self) -> f64 {
        self.neck_width_um.filter(|v| *v > 0.0).unwrap_or(0.0)
    }

    /// `RouterSettings.isStrictDrc` (`:532-534`): `Boolean.TRUE.equals(strictDrc)`.
    pub fn is_strict_drc(&self) -> bool {
        self.strict_drc.unwrap_or(false)
    }

    /// `RouterSettings.getAutomaticNeckdown` (`:892-894`).
    pub fn get_automatic_neckdown(&self) -> bool {
        self.automatic_neckdown.unwrap_or(false)
    }

    /// `RouterSettings.setAutomaticNeckdown` (`:897-899`).
    pub fn set_automatic_neckdown(&mut self, value: bool) {
        self.automatic_neckdown = Some(value);
    }

    /// `RouterSettings.getStartRipupCosts` (`:537-539`): `1` when `scoring` or the field is
    /// absent.
    pub fn get_start_ripup_costs(&self) -> i32 {
        self.scoring
            .as_ref()
            .and_then(|s| s.start_ripup_costs)
            .unwrap_or(1)
    }

    /// `RouterSettings.setStartRipupCosts` (`:542-547`): instantiates `scoring` when absent and
    /// clamps with `Math.max(value, 1)`.
    pub fn set_start_ripup_costs(&mut self, value: i32) {
        self.scoring
            .get_or_insert_with(ScoringSettings::default)
            .start_ripup_costs = Some(value.max(1));
    }

    /// `RouterSettings.getRunRouter` (`:550-552`): an absent `enabled` means "run".
    pub fn get_run_router(&self) -> bool {
        self.enabled.unwrap_or(true)
    }

    /// `RouterSettings.setRunRouter` (`:555-557`).
    pub fn set_run_router(&mut self, value: bool) {
        self.enabled = Some(value);
    }

    /// `RouterSettings.isFanoutEnabled` (`:578-580`):
    /// `fanout != null && Boolean.TRUE.equals(fanout.enabled)` — absent means **false**.
    ///
    /// Its neighbour `getRunFanout` (`:573-575`) carries the identical javadoc ("Returns whether
    /// the fanout pre-pass should run") and the **opposite** default: `fanout != null &&
    /// fanout.enabled != null ? fanout.enabled : true`. So on a `RouterSettings` that never went
    /// through `DefaultSettings`, `getRunFanout()` says run and `isFanoutEnabled()` says do not.
    /// Only `isFanoutEnabled` has headless callers —
    /// `autoroute/pipeline/{RoutingPipeline.java:93,99, BatchAutorouter.java:115,
    /// AutorouteBatchLoop.java:85,89,435}` — which is why it is ported here and `getRunFanout`
    /// (GUI-only, `WindowAutorouteParameter.java:590`) stays deferred. See
    /// `docs/java-quirks.md` #139.
    ///
    /// obligation: **Plan 7** owns `autoroute/pipeline/**` (plan-6 ruling 2 moved it there);
    /// this is the accessor `BatchFanout` gates the fanout pre-pass on. Plan 6 reads it for one
    /// thing only — `AutorouteConnectionRouter.route:46`'s
    /// `removeUnconnectedVias = !isFanoutEnabled()`, which `fr_router::route_connection` takes as
    /// a parameter — so the marker stays open until Plan 7 has the pre-pass itself.
    pub fn is_fanout_enabled(&self) -> bool {
        self.fanout
            .as_ref()
            .and_then(|fanout| fanout.enabled)
            .unwrap_or(false)
    }

    /// `RouterSettings.getRunOptimizer` (`:560-562`): an absent `optimizer` *or* an absent
    /// `optimizer.enabled` means "do not run" — the opposite default from
    /// [`Self::get_run_router`].
    pub fn get_run_optimizer(&self) -> bool {
        self.optimizer
            .as_ref()
            .and_then(|o| o.enabled)
            .unwrap_or(false)
    }

    /// `RouterSettings.setRunOptimizer` (`:565-570`): instantiates `optimizer` when absent.
    pub fn set_run_optimizer(&mut self, value: bool) {
        self.optimizer
            .get_or_insert_with(OptimizerSettings::default)
            .enabled = Some(value);
    }

    /// `RouterSettings.getViasAllowed` (`:595-597`).
    pub fn get_vias_allowed(&self) -> bool {
        self.vias_allowed.unwrap_or(true)
    }

    /// `RouterSettings.setViasAllowed(Boolean)` (`:207-214`). Java's second, primitive overload
    /// (`setViasAllowed(boolean)`, `:216-218`) writes the same field with an auto-boxed value and
    /// deliberately skips the property-change notification; with no `pcs` the two collapse into
    /// this one method, so callers of the primitive form pass `Some(value)`.
    pub fn set_vias_allowed(&mut self, value: Option<bool>) {
        self.vias_allowed = value;
    }

    /// `RouterSettings.getViaCosts` (`:600-602`).
    pub fn get_via_costs(&self) -> i32 {
        self.scoring.as_ref().and_then(|s| s.via_costs).unwrap_or(1)
    }

    /// `RouterSettings.setViaCosts` (`:605-610`): `Math.max(value, 1)`.
    pub fn set_via_costs(&mut self, value: i32) {
        self.scoring
            .get_or_insert_with(ScoringSettings::default)
            .via_costs = Some(value.max(1));
    }

    /// `RouterSettings.getPlaneViaCosts` (`:613-615`).
    pub fn get_plane_via_costs(&self) -> i32 {
        self.scoring
            .as_ref()
            .and_then(|s| s.plane_via_costs)
            .unwrap_or(1)
    }

    /// `RouterSettings.setPlaneViaCosts` (`:618-623`): `Math.max(value, 1)`.
    pub fn set_plane_via_costs(&mut self, value: i32) {
        self.scoring
            .get_or_insert_with(ScoringSettings::default)
            .plane_via_costs = Some(value.max(1));
    }

    /// `RouterSettings.setMaxPasses` (`:167-174`) — a plain field write once `pcs` is gone.
    pub fn set_max_passes(&mut self, value: Option<i32>) {
        self.max_passes = value;
    }

    /// `RouterSettings.setJobTimeoutString` (`:189-196`).
    pub fn set_job_timeout_string(&mut self, value: Option<String>) {
        self.job_timeout_string = value;
    }

    /// `RouterSettings.setEnabled` (`:198-205`).
    pub fn set_enabled(&mut self, value: Option<bool>) {
        self.enabled = value;
    }

    // ---- per-layer accessors -----------------------------------------------------------------

    /// `RouterSettings.setLayerActive` (`:631-651`): out of range is a warn-and-return.
    pub fn set_layer_active(&mut self, layer: usize, value: bool) {
        let Some(entry) = self.layers.as_mut().and_then(|l| l.get_mut(layer)) else {
            return;
        };
        entry.routable = Some(value);
    }

    /// `RouterSettings.getLayerActive` (`:653-673`): `false` out of range, `true` when the layer
    /// has no opinion.
    pub fn get_layer_active(&self, layer: usize) -> bool {
        let Some(entry) = self.layers.as_ref().and_then(|l| l.get(layer)) else {
            return false;
        };
        entry.routable.unwrap_or(true)
    }

    /// `RouterSettings.setBendCost` (`:675-690`): clamped into
    /// `[MIN_BEND_COST, MAX_BEND_COST]`; out of range is a warn-and-return.
    pub fn set_bend_cost(&mut self, layer: usize, value: f64) {
        let Some(entry) = self.layers.as_mut().and_then(|l| l.get_mut(layer)) else {
            return;
        };
        entry.bend_cost = Some(clamp_bend_cost(value));
    }

    /// `RouterSettings.getBendCost` (`:692-703`): `0.0` out of range; the **clamped**
    /// `scoring.defaultBendCost` when the layer carries no explicit cost; `0.0` when there is no
    /// default either.
    ///
    /// The clamp on the fallback path (`:698-699`) has no counterpart on the explicit-value path
    /// (`:702`): a per-layer `bendCost` is clamped by [`Self::set_bend_cost`] on the way in, but
    /// one that arrived through the merge engine (`layers[i].bend_cost` from a `.rules` file) is
    /// returned unclamped. Faithfully reproduced; it is a shape difference, not a defect Java
    /// can be shown to hit, so it gets this note rather than a quirks row.
    pub fn get_bend_cost(&self, layer: usize) -> f64 {
        let Some(entry) = self.layers.as_ref().and_then(|l| l.get(layer)) else {
            return 0.0;
        };
        match entry.bend_cost {
            Some(value) => value,
            None => self
                .scoring
                .as_ref()
                .and_then(|s| s.default_bend_cost)
                .map_or(0.0, clamp_bend_cost),
        }
    }

    /// `RouterSettings.setPreferredDirectionIsHorizontal` (`:711-731`).
    pub fn set_preferred_direction_is_horizontal(&mut self, layer: usize, value: bool) {
        let Some(entry) = self.layers.as_mut().and_then(|l| l.get_mut(layer)) else {
            return;
        };
        entry.preferred_direction_horizontal = Some(value);
    }

    /// `RouterSettings.getPreferredDirectionIsHorizontal` (`:733-755`): `false` out of range;
    /// otherwise the layer's own answer, defaulting to the alternating `layer % 2 == 1`.
    pub fn get_preferred_direction_is_horizontal(&self, layer: usize) -> bool {
        let Some(entry) = self.layers.as_ref().and_then(|l| l.get(layer)) else {
            return false;
        };
        entry
            .preferred_direction_horizontal
            .unwrap_or(layer % 2 == 1)
    }

    /// `RouterSettings.setPreferredDirectionTraceCosts` (`:757-777`): out of range is a
    /// warn-and-return; otherwise `scoring` is instantiated when absent, the cost array is
    /// **replaced** (not resized) when its length disagrees with the layer count — so untouched
    /// entries come back as `0.0`, not as their old values — the value is clamped with
    /// `Math.max(value, 0.1)`, and [`Self::board_specific_trace_costs_applied`] is set to
    /// `Some(true)` (`:776`).
    pub fn set_preferred_direction_trace_costs(&mut self, layer: usize, value: f64) {
        let layer_count = self.get_layer_count();
        if layer >= layer_count {
            return;
        }
        let scoring = self.scoring.get_or_insert_with(ScoringSettings::default);
        if !matches!(&scoring.preferred_direction_trace_cost, Some(a) if a.len() == layer_count) {
            scoring.preferred_direction_trace_cost = Some(vec![0.0; layer_count]);
        }
        scoring
            .preferred_direction_trace_cost
            .as_mut()
            .expect("set above")[layer] = java_math_max(value, 0.1);
        self.board_specific_trace_costs_applied = Some(true);
    }

    /// `RouterSettings.getPreferredDirectionTraceCosts` (`:779-796`): `0.0` out of range, `1.0`
    /// when `scoring` or the array is absent or too short.
    pub fn get_preferred_direction_trace_costs(&self, layer: usize) -> f64 {
        if layer >= self.get_layer_count() {
            return 0.0;
        }
        self.scoring
            .as_ref()
            .and_then(|s| s.preferred_direction_trace_cost.as_ref())
            .and_then(|a| a.get(layer).copied())
            .unwrap_or(1.0)
    }

    /// `RouterSettings.setAgainstPreferredDirectionTraceCosts` (`:838-859`) — the same shape as
    /// [`Self::set_preferred_direction_trace_costs`], on the other array, with the same
    /// `Math.max(value, 0.1)` clamp (`:857`) and the same applied-flag side effect (`:858`).
    pub fn set_against_preferred_direction_trace_costs(&mut self, layer: usize, value: f64) {
        let layer_count = self.get_layer_count();
        if layer >= layer_count {
            return;
        }
        let scoring = self.scoring.get_or_insert_with(ScoringSettings::default);
        if !matches!(&scoring.undesired_direction_trace_cost, Some(a) if a.len() == layer_count) {
            scoring.undesired_direction_trace_cost = Some(vec![0.0; layer_count]);
        }
        scoring
            .undesired_direction_trace_cost
            .as_mut()
            .expect("set above")[layer] = java_math_max(value, 0.1);
        self.board_specific_trace_costs_applied = Some(true);
    }

    /// `RouterSettings.getAgainstPreferredDirectionTraceCosts` (`:798-816`).
    pub fn get_against_preferred_direction_trace_costs(&self, layer: usize) -> f64 {
        if layer >= self.get_layer_count() {
            return 0.0;
        }
        self.scoring
            .as_ref()
            .and_then(|s| s.undesired_direction_trace_cost.as_ref())
            .and_then(|a| a.get(layer).copied())
            .unwrap_or(1.0)
    }

    /// `RouterSettings.getHorizontalTraceCosts` (`:818-830`): the preferred-direction cost when
    /// the layer routes horizontally, the undesired-direction cost otherwise.
    ///
    /// Java bug: getHorizontalTraceCosts (RouterSettings.java:825-827) dereferences
    /// `scoring.preferredDirectionTraceCost` / `scoring.undesiredDirectionTraceCost` with no null
    /// or length guard, unlike `getPreferredDirectionTraceCosts` (:781-785) and
    /// `getAgainstPreferredDirectionTraceCosts` (:806-810) immediately above it, which both
    /// answer `1.0` in exactly that situation. A `RouterSettings` with a non-empty `layers` and a
    /// `scoring` whose cost arrays were never allocated therefore throws
    /// `NullPointerException` here — JVM-verified against the clone-HEAD jar (see
    /// `task-4-report.md`, probe row `E`). Reproduced as a panic, not softened to `1.0`; see
    /// `docs/java-quirks.md` #123.
    pub fn get_horizontal_trace_costs(&self, layer: usize) -> f64 {
        if layer >= self.get_layer_count() {
            return 0.0;
        }
        let scoring = self
            .scoring
            .as_ref()
            .expect("RouterSettings.java:825-827: scoring is dereferenced without a null check");
        let array = if self.get_preferred_direction_is_horizontal(layer) {
            scoring.preferred_direction_trace_cost.as_ref()
        } else {
            scoring.undesired_direction_trace_cost.as_ref()
        };
        array.expect(
            "RouterSettings.java:825-827: the trace cost array is dereferenced without a null check",
        )[layer]
    }

    /// `RouterSettings.getVerticalTraceCosts` (`:861-874`): the mirror of
    /// [`Self::get_horizontal_trace_costs`], reading the *other* array for the same layer.
    ///
    /// Java bug: getVerticalTraceCosts (RouterSettings.java:869-871) has the same unguarded
    /// dereference as its horizontal twin — see [`Self::get_horizontal_trace_costs`] and
    /// `docs/java-quirks.md` #123.
    pub fn get_vertical_trace_costs(&self, layer: usize) -> f64 {
        if layer >= self.get_layer_count() {
            return 0.0;
        }
        let scoring = self
            .scoring
            .as_ref()
            .expect("RouterSettings.java:869-871: scoring is dereferenced without a null check");
        let array = if self.get_preferred_direction_is_horizontal(layer) {
            scoring.undesired_direction_trace_cost.as_ref()
        } else {
            scoring.preferred_direction_trace_cost.as_ref()
        };
        array.expect(
            "RouterSettings.java:869-871: the trace cost array is dereferenced without a null check",
        )[layer]
    }

    /// `RouterSettings.getTraceCosts` (`:877-889`): one [`ExpansionCostFactor`] per entry of
    /// `scoring.preferredDirectionTraceCost`, empty when `scoring` or that array is absent.
    ///
    /// The guard at `:878-880` only checks `preferredDirectionTraceCost`, so a settings object
    /// with a populated preferred array and an absent *undesired* array reaches
    /// [`Self::get_horizontal_trace_costs`]' unguarded dereference and panics — the same Java bug
    /// one level up.
    pub fn get_trace_costs(&self) -> Vec<ExpansionCostFactor> {
        let Some(length) = self
            .scoring
            .as_ref()
            .and_then(|s| s.preferred_direction_trace_cost.as_ref())
            .map(Vec::len)
        else {
            return Vec::new();
        };
        (0..length)
            .map(|i| ExpansionCostFactor {
                horizontal: self.get_horizontal_trace_costs(i),
                vertical: self.get_vertical_trace_costs(i),
            })
            .collect()
    }

    // ---- clone, validate, thread normalisation -----------------------------------------------

    /// renamed: clone -> java_clone (`RouterSettings.java:487-524`).
    ///
    /// The literal transcription of Java's `clone()`, which is **not** the same operation as
    /// Rust's derived [`Clone`] and must be used wherever the Java call site does — notably
    /// `SettingsMerger.merge`'s choice of merge base (`SettingsMerger.java:133-193`, Task 8).
    /// Two differences, both JVM-verified (`task-4-report.md`, probe row `A`):
    ///
    /// 1. Java bug: clone (RouterSettings.java:487-524) never assigns `result.resultJsonPath`.
    ///    Every other scalar is re-copied explicitly; this one is missing from the list, so a
    ///    clone always loses its result-manifest path. Reproduced — the field is deliberately
    ///    absent from the body below. See `docs/java-quirks.md` #114.
    /// 2. A `null` `optimizer`/`scoring`/`fanout` comes back as a **fresh default object**, not
    ///    as `null` (`:518-520`), which `BendCostSettingsTest.nullScoringSafety:66-68` pins
    ///    directly. The derived `Clone` keeps them `None`.
    ///
    /// Quirk Q12 (no `docs/java-quirks.md` row of its own — the same `clone()` call is the
    /// counter-example named in passing by #119's text): `:490-492` calls
    /// `setLayerCount(getLayerCount())` on the fresh result before
    /// copying anything, which reallocates its `layers` and both of its `scoring` cost arrays and
    /// resets every per-layer field. That is harmless only by accident of ordering — `layerCount
    /// > 0` implies `this.layers != null`, so `:493-501` replaces `layers` wholesale and `:519`
    /// replaces `scoring` wholesale, overwriting everything `setLayerCount` just did. Reorder the
    /// method and it silently starts wiping the caller's costs. The call is kept here because it
    /// is what Java does, and `java_clone_replays_javas_sequence` pins the equivalence.
    pub fn java_clone(&self) -> Self {
        let mut result = Self::new();
        let layer_count = self.get_layer_count();
        if layer_count > 0 {
            result.set_layer_count(layer_count);
        }
        result.algorithm = self.algorithm.clone();
        result.job_timeout_string = self.job_timeout_string.clone();
        if self.layers.is_some() {
            result.layers = self.layers.clone();
        }
        result.max_passes = self.max_passes;
        result.max_items = self.max_items;
        result.save_intermediate_stages = self.save_intermediate_stages;
        result.copper_to_edge_clearance_um = self.copper_to_edge_clearance_um;
        result.hole_clearance_um = self.hole_clearance_um;
        result.neck_width_um = self.neck_width_um;
        result.strict_drc = self.strict_drc;
        result.ignore_net_classes = self.ignore_net_classes.clone();
        result.trace_pull_tight_accuracy = self.trace_pull_tight_accuracy;
        result.enabled = self.enabled;
        result.vias_allowed = self.vias_allowed;
        result.automatic_neckdown = self.automatic_neckdown;
        result.max_threads = self.max_threads;
        result.optimizer = Some(self.optimizer.clone().unwrap_or_default());
        result.scoring = Some(self.scoring.clone().unwrap_or_default());
        result.fanout = Some(self.fanout.clone().unwrap_or_default());
        result.board_specific_trace_costs_applied = self.board_specific_trace_costs_applied;
        // `result.result_json_path` is deliberately NOT assigned — see the doc comment above.
        result
    }

    /// `RouterSettings.setMaxThreads` (`:176-187`): normalises through `normalizeMaxThreads`
    /// (`:137-149`) and mirrors the answer into `optimizer.maxThreads` — but only when
    /// `optimizer` is already present (`:183`); unlike [`Self::set_run_optimizer`] this one does
    /// not instantiate it.
    ///
    /// `Runtime.getRuntime().availableProcessors()` is injected as `host` (plan ruling 6).
    pub fn set_max_threads(&mut self, value: Option<i32>, host: &HostEnvironment) {
        let normalized = normalize_max_threads(value, host);
        self.max_threads = Some(normalized);
        if let Some(optimizer) = self.optimizer.as_mut() {
            optimizer.max_threads = Some(normalized);
        }
    }

    /// `RouterSettings.validate` (`:932-965`): normalises the three fields that affect routing
    /// execution.
    ///
    /// - `maxPasses`: outside `[0, 9999]` becomes `9999`; exactly `0` means "no limit" and
    ///   becomes `Integer.MAX_VALUE`.
    /// - `maxThreads`: absent or negative becomes `defaultMaxThreads()`; greater than the
    ///   processor count is capped at it.
    /// - `tracePullTightAccuracy`: below `1` becomes `500`.
    ///
    /// Java bug: validate (RouterSettings.java:951) tests `maxThreads > availableProcessors`
    /// where `normalizeMaxThreads` (:144-146) special-cases `value == 0` to mean "every
    /// processor". `validate()` therefore leaves a `0` as `0` — a thread pool of size zero —
    /// while `setMaxThreads(0)` on the same object yields the full core count. Two normalisers
    /// in one class disagree about the one input a user is most likely to write for "no limit".
    /// JVM-verified (`task-4-report.md`, probe rows `B.maxThreads 0 -> 0` and
    /// `C.setMaxThreads 0 -> 4`); `docs/java-quirks.md` #124.
    ///
    /// Java bug: validate (RouterSettings.java:934) dereferences `this.maxPasses` unboxed, and
    /// `:958` does the same for `this.tracePullTightAccuracy`. Both are `Integer`, and both are
    /// `null` on any `RouterSettings` that did not come through `DefaultSettings` — so
    /// `SettingsMerger.merge`'s unconditional `validate()` call (`SettingsMerger.java:189`)
    /// throws `NullPointerException` for a merger whose lowest-priority source is not
    /// `DefaultSettings`. Reproduced as a documented panic rather than a `Result`: the crash is
    /// reachable in Java (nothing forbids such a merger), so a port that returned an error would
    /// diverge on exactly the input that matters. JVM-verified (probe rows `B.nullMaxPasses`,
    /// `B.nullTpta`); `docs/java-quirks.md` #125.
    ///
    /// # Panics
    ///
    /// If `max_passes` or `trace_pull_tight_accuracy` is `None` — see above.
    pub fn validate(&mut self, host: &HostEnvironment) {
        let max_passes = self
            .max_passes
            .expect("RouterSettings.java:934: maxPasses is dereferenced unboxed");
        if !(0..=9999).contains(&max_passes) {
            self.max_passes = Some(9999);
        } else if max_passes == 0 {
            self.max_passes = Some(i32::MAX);
        }

        let available_processors = host.available_processors() as i32;
        match self.max_threads {
            None => self.max_threads = Some(host.default_max_threads()),
            Some(value) if value < 0 => self.max_threads = Some(host.default_max_threads()),
            Some(value) if value > available_processors => {
                self.max_threads = Some(available_processors);
            }
            Some(_) => {}
        }

        let accuracy = self
            .trace_pull_tight_accuracy
            .expect("RouterSettings.java:958: tracePullTightAccuracy is dereferenced unboxed");
        if accuracy < 1 {
            self.trace_pull_tight_accuracy = Some(500);
        }
    }
}

/// `RouterSettings.normalizeMaxThreads` (`RouterSettings.java:137-149`), with
/// `Runtime.getRuntime().availableProcessors()` injected (plan ruling 6).
///
/// Absent or negative gives `defaultMaxThreads()`; **`0` gives the full processor count** — the
/// half of quirk #124 that [`RouterSettings::validate`] disagrees with; anything else is capped
/// at the processor count.
fn normalize_max_threads(value: Option<i32>, host: &HostEnvironment) -> i32 {
    match value {
        None => host.default_max_threads(),
        Some(value) if value < 0 => host.default_max_threads(),
        Some(0) => host.available_processors() as i32,
        Some(value) => value.min(host.available_processors() as i32),
    }
}

/// `Math.max(MIN_BEND_COST, Math.min(MAX_BEND_COST, value))`
/// (`RouterSettings.java:689`, `:698-699`). Rust's `f64::clamp` has Java's NaN behaviour here —
/// `Math.min`/`Math.max` propagate a NaN input and so does `clamp` — and the bounds are constant,
/// so it cannot hit `clamp`'s "min > max" panic.
fn clamp_bend_cost(value: f64) -> f64 {
    value.clamp(RouterSettings::MIN_BEND_COST, RouterSettings::MAX_BEND_COST)
}

/// `Math.max(double, double)`. Rust's `f64::max` returns *the other argument* when one is NaN;
/// Java's `Math.max` propagates the NaN. `field_path::java_parse_f64` accepts the literal `NaN`
/// (`Double.valueOf`'s grammar), so a settings source can put one into a trace cost — hence the
/// explicit reproduction rather than a bare `f64::max`.
fn java_math_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}

/// `AutorouteControl.ExpansionCostFactor` (`autoroute/maze/AutorouteControl.java:287`), the
/// per-layer horizontal/vertical trace-cost pair [`RouterSettings::get_trace_costs`] returns.
///
/// obligation: Plan 6 owns `autoroute/maze/AutorouteControl.java` and the maze search that
/// consumes these factors. The record is defined here, in `fr-settings`, because
/// `RouterSettings.getTraceCosts` is its only producer and `fr-settings` cannot depend on a
/// crate that does not exist yet; when Plan 6 ports `AutorouteControl` it should re-export this
/// type rather than declare a second one. Recorded in `docs/java-quirks.md`'s obligation
/// register. **Discharged in Plan 6 Task 1**: `fr-router`'s `lib.rs` re-exports it (plan-6
/// ruling 8) and `crates/fr-router/tests/skeleton.rs` pins the two paths to one `TypeId`.
///
/// (Correction to the task brief, which cites `AutorouteControl.java:118`: the record is declared
/// at `:287` at the clone's HEAD. `:118` is inside the `AutorouteControl(RoutingBoard, int,
/// RouterSettings)` constructor.)
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct ExpansionCostFactor {
    /// The cost factor for horizontal movement on the layer.
    pub horizontal: f64,
    /// The cost factor for vertical movement on the layer.
    pub vertical: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Quirk Q11's (`docs/java-quirks.md` #126) subtle half. `setLayerCount`'s reallocation
    /// branch (`:456-461`) is the *only*
    /// place `boardSpecificTraceCostsApplied` is cleared (`:457`), and it does not fire when the
    /// layer count is unchanged — while the per-layer and cost-array resets at `:466-477` run
    /// unconditionally. So a same-count `setLayerCount` wipes every cost the merge just parsed
    /// and still reports the costs as board-tuned. JVM-verified (`task-4-report.md`, probe rows
    /// `D.before` / `D.after(same)`).
    ///
    /// Java bug: setLayerCount (RouterSettings.java:455-478) resets the cost arrays and every
    /// per-layer field on every call, but only clears `boardSpecificTraceCostsApplied` when it
    /// reallocates `layers`. See `docs/java-quirks.md` #126.
    ///
    /// This lives here rather than in `tests/router_settings.rs` because
    /// `board_specific_trace_costs_applied` is `pub(crate)` — `private` in Java for the same
    /// reason — so only an in-crate test can observe it. The cost-array half is pinned from the
    /// outside by `tests/router_settings.rs::set_layer_count_rewipes_costs`.
    #[test]
    fn set_layer_count_rewipes_costs_but_keeps_the_applied_flag() {
        let mut settings = RouterSettings::new();
        settings.set_layer_count(2);
        settings.set_preferred_direction_trace_costs(0, 2.5);
        assert_eq!(settings.board_specific_trace_costs_applied, Some(true));

        settings.set_layer_count(2);

        assert_eq!(settings.get_preferred_direction_trace_costs(0), 1.0);
        assert_eq!(
            settings.board_specific_trace_costs_applied,
            Some(true),
            "the :456 reallocation branch did not fire, so :457 did not clear the flag"
        );
    }

    /// A *different* layer count does take the reallocation branch and does clear the flag —
    /// `Issue729TraceCostSettingsTest.setLayerCountResetsTraceCostAppliedFlag` (:106-114), minus
    /// its `applyBoardSpecificOptimizations` set-up (Task 5's). JVM probe row `D.after(diff)`.
    #[test]
    fn set_layer_count_resets_applied_flag() {
        let mut settings = RouterSettings::new();
        settings.set_layer_count(2);
        settings.set_preferred_direction_trace_costs(0, 2.5);
        assert_eq!(settings.board_specific_trace_costs_applied, Some(true));

        settings.set_layer_count(4);

        assert_eq!(settings.board_specific_trace_costs_applied, Some(false));
        assert_eq!(settings.get_layer_count(), 4);
    }

    /// An out-of-range trace-cost setter must not set the applied flag either — it returns before
    /// `:776`/`:858`. JVM probe row `F.setterOutOfRange applied=null`.
    #[test]
    fn out_of_range_trace_cost_setter_does_not_set_the_applied_flag() {
        let mut settings = RouterSettings::new();
        settings.set_preferred_direction_trace_costs(0, 9.0);
        assert_eq!(settings.board_specific_trace_costs_applied, None);
        assert_eq!(
            settings
                .scoring
                .as_ref()
                .unwrap()
                .preferred_direction_trace_cost,
            None
        );
    }

    /// `java_clone` carries the private applied flag across (`:521`) — probe row
    /// `A.applied(src)=true applied(clone)=true`.
    #[test]
    fn java_clone_carries_the_applied_flag() {
        let mut settings = RouterSettings::new();
        settings.set_layer_count(2);
        settings.set_preferred_direction_trace_costs(0, 2.5);
        assert_eq!(
            settings.java_clone().board_specific_trace_costs_applied,
            Some(true)
        );

        let mut untouched = RouterSettings::new();
        untouched.set_layer_count(2);
        assert_eq!(
            untouched.java_clone().board_specific_trace_costs_applied,
            Some(false)
        );
    }

    /// `Math.max` propagates NaN where Rust's `f64::max` does not.
    #[test]
    fn java_math_max_propagates_nan() {
        assert!(java_math_max(f64::NAN, 0.1).is_nan());
        assert!(java_math_max(0.1, f64::NAN).is_nan());
        assert_eq!(java_math_max(0.05, 0.1), 0.1);
        assert_eq!(java_math_max(5.0, 0.1), 5.0);
    }
}
