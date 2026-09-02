//! `util/ReflectionUtil.java`'s `copyFields` (:215-344) and `getDefaultValue` (:350-372), as a
//! hand-written per-struct merge engine.
//!
//! Java walks `source.getClass().getDeclaredFields()` reflectively and applies eight rules to each
//! field. Rust has no runtime field table, so each struct gets an explicit, hand-written field
//! table — [`CopyFields::merge_fields_into`] — listing its fields **in Java declaration order**,
//! each one routed through the helper named after the rule that governs it. No macro generates
//! these tables: the order is load-bearing and must stay readable (plan ruling 4).
//!
//! A hand-written table can silently omit a field, so each struct has a
//! `*_table_covers_every_field` test in `tests/copy_fields.rs`: it populates **every** `pub` field
//! of the source with a distinct non-default value, copies into a `default()` target and asserts
//! the whole struct compares equal (plus the same under [`MergeMode::FillAbsent`]). Omit one
//! table entry and that equality fails. Each struct's `FIELD_NAMES` const and
//! `tests/struct_shape.rs` pin the *order* against Java's `getDeclaredFields()`; these tests pin
//! the table's *completeness* against the struct, and tie the two together by asserting the
//! change count against `FIELD_NAMES.len()` wherever the struct has no nested object to make the
//! arithmetic indirect.
//!
//! ## The eight rules (`ReflectionUtil.java` line numbers)
//!
//! 1. **:221-228** — skip `static` fields and non-`public` fields. Rust has no per-instance
//!    statics, so Java's four `public static final` constants on `RouterSettings` (:15-18) simply
//!    are not struct fields; the non-`public` half is live and drops
//!    `RouterSettings.boardSpecificTraceCostsApplied` (`private transient`, :111) and `pcs`
//!    (`private transient`, :114, not ported at all). `transient` is **not** skipped.
//! 2. **:234-238** — `shouldCopy = sourceValue != null`, i.e. `Option::is_some` here; a Java
//!    **primitive** field additionally has to differ from `getDefaultValue(field)`. Note that
//!    `getDefaultValue` answers the *type's* default (`false`, `0`, :362-363/:355-356), not the
//!    field's initialiser — so `DesignRulesCheckerSettings.includeWarnings`, initialised to
//!    `true`, still has default `false`. See [`primitive_bool_copy`].
//! 3. **:242-259** — primitives, wrapper types and `String`: copy. See [`scalar_copy`].
//! 4. **:260-266** — enums: `Enum.valueOf(type, sourceValue.toString())`, exact and
//!    case-sensitive (unlike `setFieldValue`'s case-insensitive match at :158-164 — two different
//!    rules in one file). See [`enum_copy`] / [`enum_copy_by_name`].
//! 5. **:269-290** — arrays of primitives or `String`: copy **only** when the target is `null`, or
//!    empty while the source is non-empty. First writer wins. See [`primitive_array_copy`].
//! 6. **:291-327** — arrays of objects: merge element-wise when `target.len() >= source.len()`
//!    (never shrinking the target), otherwise replace with a fresh array of `source.len()` freshly
//!    constructed, deep-copied elements. See [`object_array_merge`].
//! 7. **:328-336** — any other object: recurse, instantiating a `null` target field with its
//!    no-arg constructor first. See [`nested_copy`].
//! 8. **:338-340** — every exception is caught per field and logged, so one bad field never
//!    aborts the merge. This crate has no `FRLogger` (plan Global Constraints), so a failure
//!    becomes a [`MergeError`] pushed onto [`MergeReport::errors`] and the walk continues.
//!
//! ## Change counting
//!
//! `MergeReport::fields_changed` counts **by rule**, not by Java's boxed-reference `!=` test
//! (`:256`) — plan ruling 2. Java's `!=` makes two equal `Integer`s above the 127 cache count as
//! "changed" while two cached ones do not, which Rust has no boxing to reproduce and no caller
//! observes (`SettingsMerger.java:171` logs the count, `RulesReader.java:156` discards it). The
//! one countable rule that *is* reproducible — the object-array arm adding `source.len()`
//! unconditionally, ignoring the inner `copyFields` return values (`:311`, `:325`) — is
//! reproduced exactly. Verified against the JVM: `copyFields` reports 0 for a `false` primitive,
//! 7 for seven scalars and 2 for a 2-element `LayerSettings[]` onto a 6-element target regardless
//! of whether anything moved (see task-2-report.md).
//!
//! ## `fill_absent_from`
//!
//! [`MergeMode::FillAbsent`] is the same walk with the scalar roles inverted: a field is written
//! only when the target's is still `None`. It has no Java analogue — it is the derived operation
//! plan ruling 1 needs to express merge #2's own `0..60` source chain as one linear pass. Rule 5
//! is already "fill only what is absent" in Java, so it behaves identically in both modes.

use crate::{
    BoardUpdateStrategy, DebugSettings, DesignRulesCheckerSettings, FanoutSettings,
    ItemSelectionStrategy, LayerSettings, MergeError, MergeReport, OptimizerSettings,
    RouterSettings, ScoringSettings,
};

/// Which side wins when both the source and the target have a value for a scalar field.
///
/// [`MergeMode::Overwrite`] is `ReflectionUtil.copyFields`; [`MergeMode::FillAbsent`] is plan
/// ruling 1's inverse (added in Plan 4, no Java analogue).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeMode {
    /// The source wins wherever it has a value — `copyFields`' own rule.
    Overwrite,
    /// The target wins wherever it already has a value; only `None` fields are filled.
    FillAbsent,
}

/// `ReflectionUtil.copyFields(source, target)` (`ReflectionUtil.java:215-344`) for one struct.
///
/// Implementors write out their field table by hand, in Java declaration order.
pub trait CopyFields {
    /// The field table. Implement this; call [`Self::copy_fields_into`] instead.
    fn merge_fields_into(&self, target: &mut Self, mode: MergeMode, report: &mut MergeReport);

    /// `ReflectionUtil.copyFields(self, target)` (`ReflectionUtil.java:215-344`), in Java
    /// declaration order. Returns nothing; the count and any per-field failure land in `report`.
    fn copy_fields_into(&self, target: &mut Self, report: &mut MergeReport) {
        self.merge_fields_into(target, MergeMode::Overwrite, report);
    }
}

/// A ported Java `enum`, addressable by its exact constant name — what rule 4's
/// `Enum.valueOf(enumType, sourceValue.toString())` (`ReflectionUtil.java:260-266`) needs.
pub trait JavaEnum: Copy + Sized {
    /// The exact Java constant name (`Enum.toString()`).
    fn java_name(self) -> &'static str;

    /// `Enum.valueOf(Self, name)` — exact, **case-sensitive**; `None` where Java throws
    /// `IllegalArgumentException`.
    fn from_java_name(name: &str) -> Option<Self>;
}

impl JavaEnum for BoardUpdateStrategy {
    fn java_name(self) -> &'static str {
        BoardUpdateStrategy::java_name(self)
    }

    fn from_java_name(name: &str) -> Option<Self> {
        match name {
            "GREEDY" => Some(Self::Greedy),
            "GLOBAL_OPTIMAL" => Some(Self::GlobalOptimal),
            "HYBRID" => Some(Self::Hybrid),
            _ => None,
        }
    }
}

impl JavaEnum for ItemSelectionStrategy {
    fn java_name(self) -> &'static str {
        ItemSelectionStrategy::java_name(self)
    }

    fn from_java_name(name: &str) -> Option<Self> {
        match name {
            "SEQUENTIAL" => Some(Self::Sequential),
            "RANDOM" => Some(Self::Random),
            "PRIORITIZED" => Some(Self::Prioritized),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The per-rule helpers
// ---------------------------------------------------------------------------------------------

/// Rule 3 — primitives, wrapper types and `String` (`ReflectionUtil.java:242-259`).
///
/// Copies whenever the source is `Some` (or, under [`MergeMode::FillAbsent`], whenever the source
/// is `Some` and the target is `None`) and counts one. The `PartialEq` bound marks where Java's
/// `targetValue != sourceValue` identity test (`:256`) sits; it is deliberately not consulted,
/// because that test is boxed-reference identity, which Rust cannot reproduce and no caller
/// observes (plan ruling 2). The write is unobservable either way — Java only skips it when the
/// two references are the same object, hence already equal — so only the count diverges.
pub fn scalar_copy<T: PartialEq + Clone>(
    src: &Option<T>,
    dst: &mut Option<T>,
    mode: MergeMode,
    report: &mut MergeReport,
) {
    let Some(value) = src else {
        return; // rule 2: a null source field copies nothing.
    };
    if mode == MergeMode::FillAbsent && dst.is_some() {
        return;
    }
    *dst = Some(value.clone());
    report.fields_changed += 1;
}

/// Rule 4 — an enum field, resolved by its Java constant name
/// (`ReflectionUtil.java:260-266`).
pub fn enum_copy<T: JavaEnum>(
    path: &str,
    src: &Option<T>,
    dst: &mut Option<T>,
    mode: MergeMode,
    report: &mut MergeReport,
) {
    let Some(value) = src else {
        return;
    };
    if mode == MergeMode::FillAbsent && dst.is_some() {
        return;
    }
    enum_copy_by_name(path, value.java_name(), dst, report);
}

/// Rule 4's kernel: `Enum.valueOf(T, name)` then assign (`ReflectionUtil.java:262-265`).
///
// `ReflectionUtil.setFieldValue` (ReflectionUtil.java:21-205) is the *other*, string-keyed half
// of the same Java class: it resolves a dotted property name against a target object and coerces
// the value, with enum matching that is case-**in**sensitive (:158-164), unlike rule 4's exact
// match here. Ported in Task 3 as [`crate::field_path::set_field_value`]; the obligation marker
// that stood here is discharged.
///
/// Exact and case-sensitive. Where Java throws `IllegalArgumentException` — which
/// `copyFields`' `catch (Exception e)` (:338-340) then swallows — this pushes a
/// [`MergeError::EnumName`] and leaves the target alone. Reaching it from a typed
/// [`CopyFields`] impl is impossible (the source already holds a valid constant); it is the entry
/// point the string-keyed settings sources use.
pub fn enum_copy_by_name<T: JavaEnum>(
    path: &str,
    name: &str,
    dst: &mut Option<T>,
    report: &mut MergeReport,
) {
    match T::from_java_name(name) {
        Some(parsed) => {
            *dst = Some(parsed);
            report.fields_changed += 1;
        }
        None => report.errors.push(MergeError::EnumName {
            path: path.to_string(),
            value: name.to_string(),
        }),
    }
}

/// Rule 5 — an array of primitives or `String` (`ReflectionUtil.java:269-290`): first writer
/// wins.
///
/// Copies only when the target is `None`, or empty while the source is non-empty. Java's rule is
/// already "fill only what is absent", so [`MergeMode`] does not apply here.
///
/// **This is the arm quirk #128 turns into a trap.** `scoring.preferredDirectionTraceCost` and
/// `scoring.undesiredDirectionTraceCost` are `double[]`, so they reach this rule — and
/// [`crate::sources::DsnFileSettings`] fills both with all-`1.0` arrays at priority 20 for
/// essentially every board, as a side effect of seeding the layer count
/// (`DsnFileSettings.java:46-48` → `RouterSettings.setLayerCount` `:466-472`). From that point
/// on "first writer wins" means *nothing above priority 20 can set a per-layer trace cost*: the
/// `.rules` tier at 40, the environment at 55 and `--router.*` at 60 all lose theirs here,
/// silently. `ignoreNetClasses` is the third field on this rule and has the same property.
/// `layers` does **not** — it is an object array and goes to [`object_array_merge`], which is
/// why per-layer *directions* still get through.
pub fn primitive_array_copy<T: Clone>(
    src: &Option<Vec<T>>,
    dst: &mut Option<Vec<T>>,
    report: &mut MergeReport,
) {
    let Some(source) = src else {
        return; // rule 2.
    };
    let should_copy = match dst {
        None => true,                                            // `targetValue == null` (:285)
        Some(target) => target.is_empty() && !source.is_empty(), // `targetArrayLength == 0 && sourceArrayLength > 0`
    };
    if should_copy {
        *dst = Some(source.clone());
        report.fields_changed += 1;
    }
}

/// Rule 5 for a non-nullable `Vec` field — Java's `String[] operationFilters`
/// (`DebugSettings.java:24-34`), whose field initialiser means it is never observed `null`.
pub fn primitive_array_copy_plain<T: Clone>(src: &[T], dst: &mut Vec<T>, report: &mut MergeReport) {
    if dst.is_empty() && !src.is_empty() {
        *dst = src.to_vec();
        report.fields_changed += 1;
    }
}

/// Rule 6 — an array of objects (`ReflectionUtil.java:291-327`), never shrunk.
///
/// `target.len() >= source.len()` merges element-wise in place, leaving the target's surplus
/// elements untouched; otherwise the target is replaced by a fresh `Vec` of `source.len()`
/// default-constructed elements, each deep-copied from its source. `fields_changed` grows by
/// `source.len()` in **both** arms (`:311`, `:325`) whether or not anything moved, and the inner
/// `copy_fields_into` counts are discarded exactly as Java discards the inner return values —
/// their *errors* are not, since Java still logs those.
///
/// The `Clone` bound is not used: Java builds each new element with the component type's no-arg
/// constructor and `copyFields` into it (`:318-321`), not by cloning, and this reproduces that.
///
/// Java's per-element `sourceArray[i] != null` guard (`:305`, `:317`) has no counterpart: a
/// `Vec<T>` has no null elements, so in the replacement arm Java can leave a hole where this
/// leaves a default-constructed element. No settings source produces a `LayerSettings[]` with a
/// null element (`setLayerCount` fills every slot, `RouterSettings.java:455-477`).
pub fn object_array_merge<T: CopyFields + Default + Clone>(
    src: &Option<Vec<T>>,
    dst: &mut Option<Vec<T>>,
    mode: MergeMode,
    report: &mut MergeReport,
) {
    let Some(source) = src else {
        return; // rule 2.
    };
    let mut inner = MergeReport::default();
    // Java's `targetLength` is 0 for a null target (`:299-301`), and the arm is chosen on that
    // number alone — not on nullness — so an *empty* source array takes the merge arm even when
    // the target is null.
    let target_len = dst.as_ref().map_or(0, Vec::len);
    if target_len >= source.len() {
        // Merge arm (`:302-312`). A `None` target reaches this only when the source is empty too
        // (`0 >= 0`), and there Java's `targetObjArray` is the null it just read: the loop body
        // never runs and `field.set` is never called, so the target field stays **null** rather
        // than becoming an empty array. JVM-verified (task-2-report.md, probe K).
        if let Some(target) = dst.as_mut() {
            for (element, slot) in source.iter().zip(target.iter_mut()) {
                element.merge_fields_into(slot, mode, &mut inner);
            }
        }
    } else if mode == MergeMode::FillAbsent && dst.is_some() {
        // No Java analogue (rule 6 replaces a too-short target outright). "Fill what is absent"
        // cannot mean "discard what is present", so the target grows to the source's length with
        // default elements and every slot is then filled element-wise.
        let target = dst.as_mut().expect("is_some checked above");
        target.resize_with(source.len(), T::default);
        for (element, slot) in source.iter().zip(target.iter_mut()) {
            element.merge_fields_into(slot, mode, &mut inner);
        }
    } else {
        // Replacement arm (`:313-326`).
        let mut fresh = Vec::with_capacity(source.len());
        for element in source {
            let mut slot = T::default();
            element.merge_fields_into(&mut slot, mode, &mut inner);
            fresh.push(slot);
        }
        *dst = Some(fresh);
    }
    report.errors.append(&mut inner.errors);
    report.fields_changed += source.len();
}

/// Rule 7 — any other object (`ReflectionUtil.java:328-336`): recurse, instantiating a `None`
/// target with its no-arg constructor first. Unlike rule 6, the recursive count *is* added
/// (`:335`), and the field itself is not counted.
pub fn nested_copy<T: CopyFields + Default>(
    src: &Option<T>,
    dst: &mut Option<T>,
    mode: MergeMode,
    report: &mut MergeReport,
) {
    let Some(source) = src else {
        return; // rule 2.
    };
    let target = dst.get_or_insert_with(T::default);
    source.merge_fields_into(target, mode, report);
}

/// Rule 2's primitive-default suppression for a Java `boolean` field
/// (`ReflectionUtil.java:235-238` with `getDefaultValue` :362-363).
///
/// `getDefaultValue` answers `false` for `boolean.class`, so a `false` source value is
/// indistinguishable from "unset" and is never merged — regardless of the field's initialiser
/// (`DesignRulesCheckerSettings.includeWarnings` is initialised to `true`, and `false` still does
/// not copy). JVM-verified: `copyFields` of an `includeWarnings = false` source onto a `true`
/// target leaves `true` and reports 0 changes (task-2-report.md, probe D).
///
// Java bug: copyFields — a source that explicitly wants `includeWarnings = false` cannot express
// it through a merge, because the merge engine cannot tell "false" from "not set" for a primitive.
pub fn primitive_bool_copy(src: bool, dst: &mut bool, report: &mut MergeReport) {
    if !src {
        return;
    }
    *dst = true;
    report.fields_changed += 1;
}

/// Rule 2's primitive-default suppression for a Java `int` field
/// (`ReflectionUtil.java:235-238` with `getDefaultValue` :355-356): `0` never copies.
///
// Java bug: copyFields — same shape as `primitive_bool_copy`: `0` is indistinguishable from unset.
pub fn primitive_i32_copy(src: i32, dst: &mut i32, report: &mut MergeReport) {
    if src == 0 {
        return;
    }
    *dst = src;
    report.fields_changed += 1;
}

// ---------------------------------------------------------------------------------------------
// The field tables, one per struct, in Java declaration order
// ---------------------------------------------------------------------------------------------

impl CopyFields for LayerSettings {
    /// `LayerSettings.java:9-21` — three wrapper scalars.
    fn merge_fields_into(&self, target: &mut Self, mode: MergeMode, report: &mut MergeReport) {
        scalar_copy(&self.routable, &mut target.routable, mode, report);
        scalar_copy(
            &self.preferred_direction_horizontal,
            &mut target.preferred_direction_horizontal,
            mode,
            report,
        );
        scalar_copy(&self.bend_cost, &mut target.bend_cost, mode, report);
    }
}

impl CopyFields for ScoringSettings {
    /// `ScoringSettings.java:30-81` — two `transient double[]` (rule 5) then nine wrapper scalars.
    fn merge_fields_into(&self, target: &mut Self, mode: MergeMode, report: &mut MergeReport) {
        primitive_array_copy(
            &self.preferred_direction_trace_cost,
            &mut target.preferred_direction_trace_cost,
            report,
        );
        primitive_array_copy(
            &self.undesired_direction_trace_cost,
            &mut target.undesired_direction_trace_cost,
            report,
        );
        scalar_copy(
            &self.default_preferred_direction_trace_cost,
            &mut target.default_preferred_direction_trace_cost,
            mode,
            report,
        );
        scalar_copy(
            &self.default_undesired_direction_trace_cost,
            &mut target.default_undesired_direction_trace_cost,
            mode,
            report,
        );
        scalar_copy(&self.via_costs, &mut target.via_costs, mode, report);
        scalar_copy(
            &self.plane_via_costs,
            &mut target.plane_via_costs,
            mode,
            report,
        );
        scalar_copy(
            &self.start_ripup_costs,
            &mut target.start_ripup_costs,
            mode,
            report,
        );
        scalar_copy(
            &self.unrouted_net_penalty,
            &mut target.unrouted_net_penalty,
            mode,
            report,
        );
        scalar_copy(
            &self.clearance_violation_penalty,
            &mut target.clearance_violation_penalty,
            mode,
            report,
        );
        scalar_copy(&self.bend_penalty, &mut target.bend_penalty, mode, report);
        scalar_copy(
            &self.default_bend_cost,
            &mut target.default_bend_cost,
            mode,
            report,
        );
    }
}

impl CopyFields for OptimizerSettings {
    /// `OptimizerSettings.java:15-95` — ten wrapper scalars, two `transient` enums (rule 4) and
    /// two more scalars, interleaved exactly as Java declares them.
    fn merge_fields_into(&self, target: &mut Self, mode: MergeMode, report: &mut MergeReport) {
        scalar_copy(&self.enabled, &mut target.enabled, mode, report);
        scalar_copy(&self.algorithm, &mut target.algorithm, mode, report);
        scalar_copy(&self.max_passes, &mut target.max_passes, mode, report);
        scalar_copy(&self.max_items, &mut target.max_items, mode, report);
        scalar_copy(&self.max_threads, &mut target.max_threads, mode, report);
        scalar_copy(
            &self.optimization_improvement_threshold,
            &mut target.optimization_improvement_threshold,
            mode,
            report,
        );
        scalar_copy(
            &self.max_consecutive_failures,
            &mut target.max_consecutive_failures,
            mode,
            report,
        );
        scalar_copy(
            &self.additional_ripup_cost_factor_at_start,
            &mut target.additional_ripup_cost_factor_at_start,
            mode,
            report,
        );
        scalar_copy(
            &self.trace_ripup_cost_factor,
            &mut target.trace_ripup_cost_factor,
            mode,
            report,
        );
        scalar_copy(
            &self.max_autoroute_passes,
            &mut target.max_autoroute_passes,
            mode,
            report,
        );
        enum_copy(
            "board_update_strategy",
            &self.board_update_strategy,
            &mut target.board_update_strategy,
            mode,
            report,
        );
        scalar_copy(&self.hybrid_ratio, &mut target.hybrid_ratio, mode, report);
        enum_copy(
            "item_selection_strategy",
            &self.item_selection_strategy,
            &mut target.item_selection_strategy,
            mode,
            report,
        );
        scalar_copy(
            &self.timeout_string,
            &mut target.timeout_string,
            mode,
            report,
        );
    }
}

impl CopyFields for FanoutSettings {
    /// `FanoutSettings.java:23-106` — twelve wrapper scalars.
    fn merge_fields_into(&self, target: &mut Self, mode: MergeMode, report: &mut MergeReport) {
        scalar_copy(&self.enabled, &mut target.enabled, mode, report);
        scalar_copy(&self.max_passes, &mut target.max_passes, mode, report);
        scalar_copy(&self.max_items, &mut target.max_items, mode, report);
        scalar_copy(
            &self.max_milliseconds_per_pin,
            &mut target.max_milliseconds_per_pin,
            mode,
            report,
        );
        scalar_copy(&self.ripup_allowed, &mut target.ripup_allowed, mode, report);
        scalar_copy(
            &self.min_escape_length_mm,
            &mut target.min_escape_length_mm,
            mode,
            report,
        );
        scalar_copy(
            &self.max_escape_length_mm,
            &mut target.max_escape_length_mm,
            mode,
            report,
        );
        scalar_copy(
            &self.start_via_diameter_mm,
            &mut target.start_via_diameter_mm,
            mode,
            report,
        );
        scalar_copy(
            &self.end_via_diameter_mm,
            &mut target.end_via_diameter_mm,
            mode,
            report,
        );
        scalar_copy(
            &self.pin_sorting_order,
            &mut target.pin_sorting_order,
            mode,
            report,
        );
        scalar_copy(
            &self.fallback_to_board_vias,
            &mut target.fallback_to_board_vias,
            mode,
            report,
        );
        scalar_copy(
            &self.timeout_string,
            &mut target.timeout_string,
            mode,
            report,
        );
    }
}

impl CopyFields for DesignRulesCheckerSettings {
    /// `DesignRulesCheckerSettings.java:11-19` — three Java `boolean` **primitives**, so all three
    /// go through rule 2's default suppression.
    fn merge_fields_into(&self, target: &mut Self, _mode: MergeMode, report: &mut MergeReport) {
        // `MergeMode` is ignored here, and on `DebugSettings` below, because every field is a
        // non-nullable Java primitive: there is no `None` for `FillAbsent` to detect, so
        // "fill only what is absent" and "the source wins" are the same operation. Note this is a
        // *representational* limit, not a decision — rule 2 already makes `false`/`0` unmergeable
        // (quirks row 115), so "absent" and "default" are the same state either way.
        //
        // Task 2's `obligation:` for this decision is **discharged by Task 7**, which is the only
        // task that touches `GlobalSettings`. The `copyFields(loadedSettings, defaultSettings)`
        // call it worried about is `GlobalSettings.load` (`GlobalSettings.java:351`), inside the
        // `freerouting.json` read — out of scope by spec §2 (no persistent config file). *(This
        // sentence continued ~~"which is also why `JsonFileSettings` is unported and priority 10
        // is only reserved"~~ until Plan 8 Task 5 ported that source under scan ruling R7.
        // `GlobalSettings.load` is still out of scope: it reads the *whole* settings object and
        // writes it back, where `JsonFileSettings` reads only `root["router"]` and writes
        // nothing.)* Task 7's own
        // scope — `EnvironmentVariablesSource`, `CliSettings` and the dead `LegacyBridge` — merges
        // neither struct: the env and CLI sources go through `set_field_value` on a
        // `RouterSettings`, and `apply_command_line_arguments` only *records* `-drc`'s
        // `drcSettings.enabled` on `LegacyBridge`. So no `MergeMode` argument is observable here,
        // and no boxed fields are needed. Should a later plan port `GlobalSettings.load`, the note
        // above still applies: `fill_absent` semantics would need boxed fields first.
        primitive_bool_copy(self.enabled, &mut target.enabled, report);
        primitive_bool_copy(self.include_warnings, &mut target.include_warnings, report);
        primitive_bool_copy(self.include_errors, &mut target.include_errors, report);
    }
}

impl CopyFields for DebugSettings {
    /// `DebugSettings.java:11-34` — two `boolean` and one `int` primitive (rule 2), a
    /// `Set<String>` (rule 7) and a `String[]` (rule 5).
    fn merge_fields_into(&self, target: &mut Self, _mode: MergeMode, report: &mut MergeReport) {
        primitive_bool_copy(
            self.enable_detailed_logging,
            &mut target.enable_detailed_logging,
            report,
        );
        primitive_bool_copy(
            self.single_step_execution,
            &mut target.single_step_execution,
            report,
        );
        primitive_i32_copy(
            self.trace_insertion_delay,
            &mut target.trace_insertion_delay,
            report,
        );
        // `filter_by_net` is a `Set<String>` in Java (`DebugSettings.java:20-21`): not a
        // primitive, not an array and not an enum, so `copyFields` takes rule 7 and recurses into
        // `HashSet`'s own declared fields — every one of which is `private` or `static`, so rule 1
        // skips them all and nothing is copied. JVM-verified as a no-op (task-2-report.md, probe
        // E). Reproduced here as the deliberate no-op it is, rather than as a set union.
        //
        // Java bug: copyFields — a merged `DebugSettings` silently drops the source's net filter.
        let _ = &self.filter_by_net;
        primitive_array_copy_plain(
            &self.operation_filters,
            &mut target.operation_filters,
            report,
        );
    }
}

impl CopyFields for RouterSettings {
    /// `RouterSettings.java:21-111`, in declaration order. The trailing
    /// `boardSpecificTraceCostsApplied` is `private` and is skipped by rule 1; `pcs` is not
    /// ported at all.
    fn merge_fields_into(&self, target: &mut Self, mode: MergeMode, report: &mut MergeReport) {
        scalar_copy(&self.enabled, &mut target.enabled, mode, report);
        scalar_copy(&self.algorithm, &mut target.algorithm, mode, report);
        nested_copy(&self.fanout, &mut target.fanout, mode, report);
        scalar_copy(
            &self.copper_to_edge_clearance_um,
            &mut target.copper_to_edge_clearance_um,
            mode,
            report,
        );
        scalar_copy(
            &self.hole_clearance_um,
            &mut target.hole_clearance_um,
            mode,
            report,
        );
        scalar_copy(&self.neck_width_um, &mut target.neck_width_um, mode, report);
        scalar_copy(&self.strict_drc, &mut target.strict_drc, mode, report);
        scalar_copy(
            &self.job_timeout_string,
            &mut target.job_timeout_string,
            mode,
            report,
        );
        scalar_copy(&self.max_passes, &mut target.max_passes, mode, report);
        scalar_copy(&self.max_items, &mut target.max_items, mode, report);
        object_array_merge(&self.layers, &mut target.layers, mode, report);
        scalar_copy(
            &self.save_intermediate_stages,
            &mut target.save_intermediate_stages,
            mode,
            report,
        );
        primitive_array_copy(
            &self.ignore_net_classes,
            &mut target.ignore_net_classes,
            report,
        );
        scalar_copy(
            &self.trace_pull_tight_accuracy,
            &mut target.trace_pull_tight_accuracy,
            mode,
            report,
        );
        scalar_copy(&self.vias_allowed, &mut target.vias_allowed, mode, report);
        scalar_copy(
            &self.automatic_neckdown,
            &mut target.automatic_neckdown,
            mode,
            report,
        );
        nested_copy(&self.optimizer, &mut target.optimizer, mode, report);
        nested_copy(&self.scoring, &mut target.scoring, mode, report);
        scalar_copy(&self.max_threads, &mut target.max_threads, mode, report);
        scalar_copy(
            &self.result_json_path,
            &mut target.result_json_path,
            mode,
            report,
        );
        // rule 1 (`ReflectionUtil.java:226-228`): `board_specific_trace_costs_applied` is
        // `private transient` in Java (`RouterSettings.java:111`) and is never copied.
    }
}

impl RouterSettings {
    /// `RouterSettings.applyNewValuesFrom` (`RouterSettings.java:907-929`): applies every
    /// explicitly populated value from `source`.
    ///
    /// Java returns the change count; here it is [`MergeReport::fields_changed`], alongside the
    /// per-field errors Java would have logged and dropped.
    ///
    // not ported: applyNewValuesFrom's pcs.firePropertyChange calls (RouterSettings.java:917-926) —
    // GUI notification, and `pcs` itself is not ported (plan Global Constraints).
    // not ported: applyNewValuesFrom's `settings == null` guard (RouterSettings.java:908-911) —
    // a `&RouterSettings` cannot be null.
    pub fn apply_new_values_from(&mut self, source: &RouterSettings) -> MergeReport {
        let mut report = MergeReport::default();
        source.copy_fields_into(self, &mut report);
        report
    }

    /// Plan ruling 1's inverse of [`Self::apply_new_values_from`]: fill only what is still absent.
    /// Not a Java method — the derived operation that makes merge #2's own `0..60` source chain
    /// expressible in one linear pass.
    ///
    /// Only `None` scalars and null-or-empty arrays are filled; rule 5 (primitive/`String`
    /// arrays) is already first-writer-wins in Java, so it behaves identically here.
    ///
    // added in Plan 4: fillAbsentFrom — no Java analogue; see
    // docs/superpowers/plans/2026-08-28-plan-4-settings.md ruling 1.
    pub fn fill_absent_from(&mut self, source: &RouterSettings) -> MergeReport {
        let mut report = MergeReport::default();
        source.merge_fields_into(self, MergeMode::FillAbsent, &mut report);
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rule 1's non-`public` half (`ReflectionUtil.java:226-228`): `private transient Boolean
    /// boardSpecificTraceCostsApplied` (`RouterSettings.java:111`) never crosses a `copyFields`
    /// boundary. JVM-verified: after `copyFields` from a source whose flag is `Boolean.TRUE`, the
    /// target's is still `null` (task-2-report.md, probe F).
    ///
    /// Lives here rather than in `tests/copy_fields.rs` because the Rust field is `pub(crate)`
    /// for exactly the reason the Java one is `private` — only an in-crate test can set it.
    #[test]
    fn board_specific_flag_is_never_copied() {
        let mut source = RouterSettings::new();
        source.board_specific_trace_costs_applied = Some(true);
        let mut target = RouterSettings::new();
        assert_eq!(target.board_specific_trace_costs_applied, None);

        target.apply_new_values_from(&source);

        assert_eq!(target.board_specific_trace_costs_applied, None);

        // ... and not under the inverted mode either.
        target.fill_absent_from(&source);
        assert_eq!(target.board_specific_trace_costs_applied, None);
    }
}
