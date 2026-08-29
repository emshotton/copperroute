//! `settings/RouterSettings.java` (966 lines): mutable router configuration assembled from the
//! configured settings sources.
//!
//! Task 1 ports only the data model — the field table below, `new()` (Java's no-arg
//! constructor), and `FIELD_NAMES`. `RouterSettings.java`'s ~50 other public methods
//! (`applyNewValuesFrom`, `validate`, `applyBoardSpecificOptimizations`, the per-layer
//! getters/setters, `clone`, ...) are the merge engine's job (Tasks 2/3) and are intentionally
//! `MISSING` from this task's `scripts/audit-port.sh` run — see task-1-report.md.
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
//! **Correction to the task brief's illustrative code snippet:** the brief's snippet attaches
//! `skip_serializing` to `max_items`/`save_intermediate_stages`/`ignore_net_classes` (implying
//! they are readable, not writable) and no skip at all to `layers` (implying it round-trips both
//! ways) — the reverse of what the JVM probe above shows. Java wins per the plan's Global
//! Constraints; this file follows the verified behaviour and the correction is called out in
//! task-1-report.md.

use serde::{Deserialize, Serialize};

use crate::{FanoutSettings, LayerSettings, OptimizerSettings, ScoringSettings};

/// Mutable router configuration assembled from the configured settings sources
/// (`RouterSettings.java:13-111`).
///
/// Every field is `Option<T>` (plan ruling 4): nullability *is* the merge protocol
/// (`SettingsMerger.java:22-31` — a source's `null` means "no opinion", not "off"). Field
/// declaration order matches Java's `getDeclaredFields()` order exactly, because the merge
/// engine's `copy_fields` (Task 2) iterates it in that order; see [`Self::FIELD_NAMES`].
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct RouterSettings {
    /// `RouterSettings.java:20-21`.
    #[serde(rename = "enabled")]
    pub enabled: Option<bool>,

    /// `RouterSettings.java:23-24`.
    #[serde(rename = "algorithm")]
    pub algorithm: Option<String>,

    /// Configuration for the SMD-pin fanout pre-pass. `RouterSettings.java:26-28`.
    #[serde(rename = "fanout")]
    pub fanout: Option<FanoutSettings>,

    /// `RouterSettings.java:30-31`.
    #[serde(rename = "copper_to_edge_clearance_um")]
    pub copper_to_edge_clearance_um: Option<f64>,

    /// `RouterSettings.java:33-34`.
    #[serde(rename = "hole_clearance_um")]
    pub hole_clearance_um: Option<f64>,

    /// Opt-in width necking: when a connection fails at its net-class trace width, retry it once
    /// with all trace half-widths clamped to this width (in micrometers). 0/absent = off.
    /// `RouterSettings.java:36-43`.
    #[serde(rename = "neck_width_um")]
    pub neck_width_um: Option<f64>,

    /// When true, a routed connection whose newly inserted traces/vias carry clearance
    /// violations is ripped up again and counted as not routed for that pass.
    /// `RouterSettings.java:45-52`.
    #[serde(rename = "strict_drc")]
    pub strict_drc: Option<bool>,

    /// `RouterSettings.java:54-55`.
    #[serde(rename = "job_timeout")]
    pub job_timeout_string: Option<String>,

    /// `RouterSettings.java:57-58`.
    #[serde(rename = "max_passes")]
    pub max_passes: Option<i32>,

    /// `transient` — never round-tripped through JSON. See the module doc comment.
    /// `RouterSettings.java:60-61`.
    #[serde(skip)]
    pub max_items: Option<i32>,

    /// `transient`, but specially re-read on deserialize (not on serialize). See the module doc
    /// comment. `RouterSettings.java:63-64`.
    #[serde(rename = "layers", skip_serializing)]
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
    #[serde(rename = "trace_pull_tight_accuracy", alias = "tracePullTightAccuracy")]
    pub trace_pull_tight_accuracy: Option<i32>,

    /// `Boolean` field under a plural-sounding JSON key: Java's own `@SerializedName` renames it
    /// to `allowed_via_types` (`RouterSettings.java:78-79`) even though the Java field name and
    /// type both read as a single flag, not a collection. Not a bug — just the serialised name —
    /// so no `// Java bug:` marker; this doc note is the record the controller decision asked
    /// for.
    #[serde(rename = "allowed_via_types")]
    pub vias_allowed: Option<bool>,

    /// If true, the trace width at static pins smaller than the trace width is lowered
    /// automatically to the pin width, if necessary. `RouterSettings.java:81-88`.
    #[serde(rename = "automatic_neckdown", alias = "automaticNeckdown")]
    pub automatic_neckdown: Option<bool>,

    /// `RouterSettings.java:90-91`.
    #[serde(rename = "optimizer")]
    pub optimizer: Option<OptimizerSettings>,

    /// `RouterSettings.java:93-94`.
    #[serde(rename = "scoring")]
    pub scoring: Option<ScoringSettings>,

    /// `RouterSettings.java:96-97`.
    #[serde(rename = "max_threads")]
    pub max_threads: Option<i32>,

    /// Optional path for a machine-readable routing result manifest (JSON). Used by benchmark and
    /// autopilot harnesses; ignored when `None` or blank. `RouterSettings.java:99-104`.
    #[serde(rename = "result_json")]
    pub result_json_path: Option<String>,

    /// When `Some(true)`, per-layer trace costs were initialized from board geometry (or set
    /// explicitly by the user) and must not be overwritten by subsequent calls to
    /// `apply_board_specific_optimizations` (Task 2/3). `private transient` in Java
    /// (`RouterSettings.java:106-111`) — private, so `copy_fields` must skip it regardless of
    /// order (plan ruling 4: non-`public` fields are never copied); kept non-`pub` here for the
    /// same reason. `transient`, so also never round-tripped through JSON.
    #[serde(skip)]
    pub(crate) board_specific_trace_costs_applied: Option<bool>,
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
    /// comments). Pins the order the merge engine's `copy_fields` (Task 2) must iterate in.
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
}

// `RouterSettings.clone()` (RouterSettings.java:486-522) is intentionally left MISSING for this
// task: unlike the four nested structs' clone() methods, it is not a plain deep copy — it
// recomputes the layer count via getLayerCount()/setLayerCount(), and delegates to each nested
// object's own clone(). Reproducing it correctly (Task 4's job, not this task's plain-data-model
// scope) must also reproduce:
//
// Java bug: RouterSettings.clone (RouterSettings.java:487-524) never assigns
// `result.resultJsonPath` — every other scalar field is explicitly re-copied in the method body,
// but this one is silently missing from that list, so a cloned RouterSettings always loses its
// result-manifest path. Confirmed by reading the method body end to end (not yet a Rust behaviour
// to test against, since `clone()` doesn't exist here yet); see docs/java-quirks.md #114. Rust
// location: ported in Task 4 (`clone`).
