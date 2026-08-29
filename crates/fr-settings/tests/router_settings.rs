//! `RouterSettings`'s null-coalescing accessors, setter clamps, `setLayerCount`, `clone` and
//! `validate` (`settings/RouterSettings.java:120-259, 440-560, 590-900, 925-965`).
//!
//! Ported from the Java tests `settings/BendCostSettingsTest.java`,
//! `settings/NeckWidthSettingsTest.java` and `settings/Issue729TraceCostSettingsTest.java`, plus
//! the `validate`/`normalizeMaxThreads` matrices that no Java test covers. Every numeric
//! expectation below was read back out of the real JVM — see `task-4-report.md` for the driver
//! (`VProbe.java`) and its transcript.
//!
//! The two tests that must observe `board_specific_trace_costs_applied` (a `pub(crate)` field,
//! `private` in Java for the same reason) live as unit tests in `src/router_settings.rs`:
//! `set_layer_count_rewipes_costs_but_keeps_the_applied_flag` and
//! `set_layer_count_resets_applied_flag`. The cost-array half of the first one is also pinned
//! here, from the outside, by `set_layer_count_rewipes_costs`.

use fr_settings::prelude::*;

/// The `HostEnvironment` every threading expectation below is stated against
/// (`java -XX:ActiveProcessorCount=4`, which is how the JVM probe was run).
fn host() -> HostEnvironment {
    HostEnvironment::with_processors(4)
}

/// A settings object with `max_passes` and `trace_pull_tight_accuracy` populated, so
/// `validate()`'s two unboxed dereferences (`RouterSettings.java:934, :958`) do not panic.
fn validatable() -> RouterSettings {
    let mut s = RouterSettings::new();
    s.max_passes = Some(50);
    s.trace_pull_tight_accuracy = Some(500);
    s.max_threads = Some(2);
    s
}

// --- BendCostSettingsTest.java --------------------------------------------------------------

/// `BendCostSettingsTest.defaultBendCost` (:12-18) asserts `DefaultSettings().getSettings()`
/// gives `scoring.defaultBendCost == 0.0`. `DefaultSettings` is Task 6's; what this task can pin
/// is the *un*seeded shape the brief spells out: the no-arg constructor allocates `scoring` but
/// leaves `defaultBendCost` absent, and `getBendCost` then answers `0.0`.
#[test]
fn default_bend_cost() {
    let mut settings = RouterSettings::new();
    assert!(settings.scoring.is_some(), "new() allocates scoring");
    assert_eq!(settings.scoring.as_ref().unwrap().default_bend_cost, None);

    settings.set_layer_count(2);
    assert_eq!(settings.get_bend_cost(0), 0.0);
    assert_eq!(settings.get_bend_cost(1), 0.0);
}

/// `BendCostSettingsTest.setGetBendCost` (:20-42). Java's last row uses `15.0`; the brief uses
/// `99.0`. Both clamp to `MAX_BEND_COST`, so both are asserted.
#[test]
fn set_get_bend_cost() {
    let mut settings = RouterSettings::new();
    settings.set_layer_count(2);

    assert_eq!(settings.get_bend_cost(0), 0.0);
    assert_eq!(settings.get_bend_cost(1), 0.0);

    settings.set_bend_cost(0, 2.5);
    settings.set_bend_cost(1, 5.0);
    assert_eq!(settings.get_bend_cost(0), 2.5);
    assert_eq!(settings.get_bend_cost(1), 5.0);

    settings.set_bend_cost(0, -1.0);
    assert_eq!(settings.get_bend_cost(0), RouterSettings::MIN_BEND_COST);

    settings.set_bend_cost(1, 15.0);
    assert_eq!(settings.get_bend_cost(1), RouterSettings::MAX_BEND_COST);
    settings.set_bend_cost(1, 99.0);
    assert_eq!(settings.get_bend_cost(1), 9.9);
}

/// `getBendCost` clamps `scoring.defaultBendCost` on the way *out* too
/// (`RouterSettings.java:697-700`) — JVM probe row `F.defaultBendCost 15.0 -> 9.9`, `-3.0 -> 0.0`.
#[test]
fn bend_cost_falls_back_to_a_clamped_default_bend_cost() {
    let mut settings = RouterSettings::new();
    settings.set_layer_count(2);

    settings.scoring.as_mut().unwrap().default_bend_cost = Some(3.0);
    assert_eq!(settings.get_bend_cost(0), 3.0);

    settings.scoring.as_mut().unwrap().default_bend_cost = Some(15.0);
    assert_eq!(settings.get_bend_cost(0), 9.9);

    settings.scoring.as_mut().unwrap().default_bend_cost = Some(-3.0);
    assert_eq!(settings.get_bend_cost(0), 0.0);

    // An explicit per-layer value wins over the default.
    settings.set_bend_cost(1, 2.0);
    assert_eq!(settings.get_bend_cost(1), 2.0);

    // Out of range answers 0.0 regardless (`:679-682`).
    assert_eq!(settings.get_bend_cost(9), 0.0);
    // ... and the setter is a no-op out of range (`:675-678`).
    settings.set_bend_cost(9, 4.0);
    assert_eq!(settings.get_layer_count(), 2);
}

/// `BendCostSettingsTest.nullScoringSafety` (:61-83), minus its
/// `applyBoardSpecificOptimizations` tail (Task 5's).
#[test]
fn null_scoring_safety() {
    let mut settings = RouterSettings::new();
    settings.set_layer_count(2);
    settings.scoring = None;

    // clone() with a null scoring still produces a non-null one (`:519`).
    assert!(settings.java_clone().scoring.is_some());

    assert_eq!(settings.get_start_ripup_costs(), 1);
    settings.set_start_ripup_costs(5);
    assert_eq!(settings.get_start_ripup_costs(), 5);

    assert_eq!(settings.get_via_costs(), 1);
    settings.set_via_costs(3);
    assert_eq!(settings.get_via_costs(), 3);

    assert_eq!(settings.get_preferred_direction_trace_costs(0), 1.0);
    settings.scoring = None; // reset to null, as the Java test does
    settings.set_preferred_direction_trace_costs(0, 2.0);
    assert_eq!(settings.get_preferred_direction_trace_costs(0), 2.0);
}

// --- NeckWidthSettingsTest.java ---------------------------------------------------------------

/// `NeckWidthSettingsTest.defaultIsOff` (:18-21) and `cloneCarriesTheField` (:23-28), plus the
/// negative row `getNeckWidthUm`'s `> 0` guard implies (`RouterSettings.java:527-529`).
#[test]
fn neck_width() {
    assert_eq!(RouterSettings::new().get_neck_width_um(), 0.0);

    let mut settings = RouterSettings::new();
    settings.neck_width_um = Some(130.0);
    assert_eq!(settings.java_clone().get_neck_width_um(), 130.0);

    settings.neck_width_um = Some(-5.0);
    assert_eq!(settings.get_neck_width_um(), 0.0);

    settings.neck_width_um = Some(0.0);
    assert_eq!(settings.get_neck_width_um(), 0.0);
}

// --- the plain null-coalescing accessors -----------------------------------------------------

/// Every accessor's answer on a freshly constructed `RouterSettings` — JVM probe row `F`.
#[test]
fn accessor_defaults_on_a_fresh_settings_object() {
    let settings = RouterSettings::new();
    assert!(settings.get_run_router()); // enabled == null -> true (`:550-552`)
    assert!(!settings.get_run_optimizer()); // optimizer.enabled == null -> false (`:559-568`)
    assert!(settings.get_vias_allowed()); // `:594-597`
    assert_eq!(settings.get_via_costs(), 1); // `:599-602`
    assert_eq!(settings.get_plane_via_costs(), 1); // `:612-615`
    assert_eq!(settings.get_start_ripup_costs(), 1); // `:536-539`
    assert_eq!(settings.get_neck_width_um(), 0.0); // `:526-529`
    assert!(!settings.is_strict_drc()); // `:531-534`
    assert!(!settings.get_automatic_neckdown()); // `:891-894`
    assert_eq!(settings.get_layer_count(), 0); // `:442-448`

    // With no layers at all, every per-layer accessor is out of range.
    assert!(!settings.get_layer_active(0));
    assert_eq!(settings.get_bend_cost(0), 0.0);
    assert!(!settings.get_preferred_direction_is_horizontal(0));
    assert_eq!(settings.get_preferred_direction_trace_costs(0), 0.0);
    assert_eq!(settings.get_against_preferred_direction_trace_costs(0), 0.0);
    assert_eq!(settings.get_horizontal_trace_costs(0), 0.0);
    assert_eq!(settings.get_vertical_trace_costs(0), 0.0);
    assert!(settings.get_trace_costs().is_empty());
}

/// `getRunOptimizer` with an absent `optimizer` is `false`, and `setRunOptimizer` instantiates
/// one (`RouterSettings.java:559-576`) — JVM probe rows `F.setRunOptimizer`/`F.getRunOptimizer`.
#[test]
fn run_router_and_run_optimizer() {
    let mut settings = RouterSettings::new();
    settings.optimizer = None;
    assert!(!settings.get_run_optimizer());
    settings.set_run_optimizer(true);
    assert!(settings.get_run_optimizer());
    assert!(settings.optimizer.is_some());

    settings.set_run_router(false);
    assert!(!settings.get_run_router());
    assert_eq!(settings.enabled, Some(false));

    settings.set_enabled(None);
    assert!(settings.get_run_router());
}

/// The three `Math.max(value, 1)` clamps (`:609`, `:622`, `:546`) — JVM probe row `F.clamps`.
#[test]
fn scalar_setter_clamps() {
    let mut settings = RouterSettings::new();
    settings.set_via_costs(-4);
    settings.set_plane_via_costs(0);
    settings.set_start_ripup_costs(-9);
    assert_eq!(settings.get_via_costs(), 1);
    assert_eq!(settings.get_plane_via_costs(), 1);
    assert_eq!(settings.get_start_ripup_costs(), 1);

    settings.set_via_costs(7);
    settings.set_plane_via_costs(8);
    settings.set_start_ripup_costs(9);
    assert_eq!(settings.get_via_costs(), 7);
    assert_eq!(settings.get_plane_via_costs(), 8);
    assert_eq!(settings.get_start_ripup_costs(), 9);
}

/// `get/setLayerActive` (`:631-667`) and `get/setPreferredDirectionIsHorizontal` (`:711-749`).
/// The odd/even fallback is JVM probe row `F.prefHoriz 0/1/2 = false/true/false`.
#[test]
fn layer_active_and_preferred_direction() {
    let mut settings = RouterSettings::new();
    settings.set_layer_count(3);

    assert!(settings.get_layer_active(0));
    settings.set_layer_active(1, false);
    assert!(!settings.get_layer_active(1));
    // Out of range: `false` from the getter, a no-op in the setter.
    assert!(!settings.get_layer_active(3));
    settings.set_layer_active(3, false);
    assert_eq!(settings.get_layer_count(), 3);

    assert!(!settings.get_preferred_direction_is_horizontal(0));
    assert!(settings.get_preferred_direction_is_horizontal(1));
    assert!(!settings.get_preferred_direction_is_horizontal(2));
    settings.set_preferred_direction_is_horizontal(0, true);
    assert!(settings.get_preferred_direction_is_horizontal(0));
    assert!(!settings.get_preferred_direction_is_horizontal(3));
}

/// `get/setPreferredDirectionTraceCosts` (`:757-790`) and their against-preferred siblings
/// (`:795-813`, `:833-855`): the `Math.max(value, 0.1)` clamp, the reallocation when the array's
/// length disagrees with the layer count, and the `boardSpecificTraceCostsApplied = true`
/// side effect. JVM probe rows `F.prefTraceClamp`, `F.undesiredClamp`, `F.realloc`.
#[test]
fn trace_cost_setters_clamp_and_reallocate() {
    let mut settings = RouterSettings::new();
    settings.set_layer_count(2);

    settings.set_preferred_direction_trace_costs(0, 0.05);
    assert_eq!(settings.get_preferred_direction_trace_costs(0), 0.1);
    settings.set_against_preferred_direction_trace_costs(0, -1.0);
    assert_eq!(settings.get_against_preferred_direction_trace_costs(0), 0.1);

    // A wrong-length array is thrown away and replaced with a zero-filled one of the right
    // length, so the untouched entry reads back as 0.0, not as its old value (`:769`).
    settings
        .scoring
        .as_mut()
        .unwrap()
        .preferred_direction_trace_cost = Some(vec![7.0]);
    settings.set_preferred_direction_trace_costs(1, 2.0);
    assert_eq!(
        settings
            .scoring
            .as_ref()
            .unwrap()
            .preferred_direction_trace_cost,
        Some(vec![0.0, 2.0])
    );

    // Out of range: getter 0.0, setter a no-op.
    assert_eq!(settings.get_preferred_direction_trace_costs(9), 0.0);
    assert_eq!(settings.get_against_preferred_direction_trace_costs(9), 0.0);
    let before = settings.clone();
    settings.set_preferred_direction_trace_costs(9, 5.0);
    settings.set_against_preferred_direction_trace_costs(9, 5.0);
    assert_eq!(settings, before);
}

/// A `scoring` whose arrays are shorter than the layer count answers `1.0`
/// (`:781-785`, `:806-810`).
#[test]
fn trace_cost_getters_fall_back_to_one() {
    let mut settings = RouterSettings::new();
    settings.set_layer_count(2);
    settings
        .scoring
        .as_mut()
        .unwrap()
        .preferred_direction_trace_cost = Some(vec![3.0]);
    settings
        .scoring
        .as_mut()
        .unwrap()
        .undesired_direction_trace_cost = None;

    assert_eq!(settings.get_preferred_direction_trace_costs(0), 3.0);
    assert_eq!(settings.get_preferred_direction_trace_costs(1), 1.0);
    assert_eq!(settings.get_against_preferred_direction_trace_costs(0), 1.0);
}

/// `getHorizontalTraceCosts`/`getVerticalTraceCosts` swap the two arrays by preferred direction
/// (`:818-830`, `:860-874`) and `getTraceCosts` pairs them (`:877-889`). JVM probe rows
/// `E.layer0`, `E.layer1`, `E.getTraceCosts`.
#[test]
fn horizontal_vertical_and_trace_costs() {
    let mut settings = RouterSettings::new();
    settings.set_layer_count(2);
    settings.set_preferred_direction_trace_costs(0, 2.0);
    settings.set_against_preferred_direction_trace_costs(0, 3.0);
    settings.set_preferred_direction_trace_costs(1, 4.0);
    settings.set_against_preferred_direction_trace_costs(1, 5.0);

    // layer 0: preferred direction is vertical, so horizontal reads the *undesired* array.
    assert!(!settings.get_preferred_direction_is_horizontal(0));
    assert_eq!(settings.get_horizontal_trace_costs(0), 3.0);
    assert_eq!(settings.get_vertical_trace_costs(0), 2.0);

    // layer 1: preferred direction is horizontal, so the pairing flips.
    assert!(settings.get_preferred_direction_is_horizontal(1));
    assert_eq!(settings.get_horizontal_trace_costs(1), 4.0);
    assert_eq!(settings.get_vertical_trace_costs(1), 5.0);

    assert_eq!(
        settings.get_trace_costs(),
        vec![
            ExpansionCostFactor {
                horizontal: 3.0,
                vertical: 2.0
            },
            ExpansionCostFactor {
                horizontal: 4.0,
                vertical: 5.0
            },
        ]
    );
}

/// Quirk Q10: `getHorizontalTraceCosts` reads `scoring.preferredDirectionTraceCost[layer]` with
/// no null guard (`:825-827`), unlike `getPreferredDirectionTraceCosts` two methods above, so a
/// `scoring` with null arrays and a non-empty `layers` throws. JVM-confirmed:
/// `E.getHorizontalTraceCosts(0) -> java.lang.NullPointerException`.
#[test]
#[should_panic(expected = "RouterSettings.java:825-827")]
fn horizontal_trace_costs_panics_without_the_array() {
    let mut settings = RouterSettings::new();
    settings.layers = Some(vec![LayerSettings::default(); 2]);
    settings.scoring = Some(ScoringSettings::default());
    let _ = settings.get_horizontal_trace_costs(0);
}

/// The same for `getVerticalTraceCosts` (`:869-871`).
#[test]
#[should_panic(expected = "RouterSettings.java:869-871")]
fn vertical_trace_costs_panics_without_the_array() {
    let mut settings = RouterSettings::new();
    settings.layers = Some(vec![LayerSettings::default(); 2]);
    settings.scoring = Some(ScoringSettings::default());
    let _ = settings.get_vertical_trace_costs(0);
}

/// The out-of-range guard runs *before* the unguarded dereference, so out-of-range indices are
/// still safe even with null arrays (`:820-823`). JVM probe rows
/// `E.getHorizontalTraceCosts(-1)`/`(9) = 0.0`.
#[test]
fn horizontal_trace_costs_out_of_range_is_checked_first() {
    let mut settings = RouterSettings::new();
    settings.layers = Some(vec![LayerSettings::default(); 2]);
    settings.scoring = Some(ScoringSettings::default());
    assert_eq!(settings.get_horizontal_trace_costs(9), 0.0);
    assert_eq!(settings.get_vertical_trace_costs(9), 0.0);
    // getTraceCosts' own guard fires before it can reach the unguarded read (`:878-880`).
    assert!(settings.get_trace_costs().is_empty());
}

// --- setLayerCount ----------------------------------------------------------------------------

/// Quirk Q11's observable half: calling `setLayerCount` with the layer count it already has
/// still wipes the cost arrays and the per-layer fields (`RouterSettings.java:466-477`). The
/// `boardSpecificTraceCostsApplied` half is the unit test in `src/router_settings.rs`. JVM probe
/// rows `D.before`/`D.after(same)`.
#[test]
fn set_layer_count_rewipes_costs() {
    let mut settings = RouterSettings::new();
    settings.set_layer_count(2);
    settings.set_preferred_direction_trace_costs(0, 2.5);
    settings.set_against_preferred_direction_trace_costs(1, 3.5);
    settings.set_bend_cost(0, 4.0);
    settings.set_layer_active(1, false);
    assert_eq!(settings.get_preferred_direction_trace_costs(0), 2.5);

    settings.set_layer_count(2);

    assert_eq!(settings.get_preferred_direction_trace_costs(0), 1.0);
    assert_eq!(settings.get_against_preferred_direction_trace_costs(1), 1.0);
    assert_eq!(settings.get_bend_cost(0), 0.0);
    assert!(settings.get_layer_active(1));
}

// --- clone ------------------------------------------------------------------------------------

/// `RouterSettings.clone` (`:487-524`) replayed field for field, asserted against
/// [`RouterSettings::java_clone`]. Quirk Q12: Java calls `setLayerCount(layerCount)` first
/// (`:490-492`), which wipes the layers and both cost arrays of the *fresh* result — and is then
/// entirely overwritten by `:493-501` and `:519`. It is correct only by accident of ordering.
#[test]
fn java_clone_replays_javas_sequence() {
    let source = populated();

    // The literal Java sequence, written out.
    let mut replay = RouterSettings::new();
    let layer_count = source.get_layer_count();
    if layer_count > 0 {
        replay.set_layer_count(layer_count); // :490-492 — entirely overwritten below
    }
    replay.algorithm = source.algorithm.clone();
    replay.job_timeout_string = source.job_timeout_string.clone();
    if source.layers.is_some() {
        replay.layers = source.layers.clone();
    }
    replay.max_passes = source.max_passes;
    replay.max_items = source.max_items;
    replay.save_intermediate_stages = source.save_intermediate_stages;
    replay.copper_to_edge_clearance_um = source.copper_to_edge_clearance_um;
    replay.hole_clearance_um = source.hole_clearance_um;
    replay.neck_width_um = source.neck_width_um;
    replay.strict_drc = source.strict_drc;
    replay.ignore_net_classes = source.ignore_net_classes.clone();
    replay.trace_pull_tight_accuracy = source.trace_pull_tight_accuracy;
    replay.enabled = source.enabled;
    replay.vias_allowed = source.vias_allowed;
    replay.automatic_neckdown = source.automatic_neckdown;
    replay.max_threads = source.max_threads;
    replay.optimizer = Some(source.optimizer.clone().unwrap_or_default());
    replay.scoring = Some(source.scoring.clone().unwrap_or_default());
    replay.fanout = Some(source.fanout.clone().unwrap_or_default());
    // :521 restores the applied flag; :487-524 never assigns result_json_path (quirk #114).

    let cloned = source.java_clone();
    assert_eq!(cloned.result_json_path, None);
    assert_eq!(replay.result_json_path, None);
    assert_eq!(cloned, replay);
}

/// Quirk #114 stated as its own row, and the two ways `java_clone` differs from Rust's derived
/// `Clone`: the dropped `result_json_path` and the null nested objects that come back non-null.
/// JVM probe rows `A.resultJsonPath(clone) = null` and
/// `A.nullScoringClone.scoringNotNull = true`.
#[test]
fn java_clone_differs_from_the_derived_clone() {
    let source = populated();
    assert_eq!(source.result_json_path.as_deref(), Some("/tmp/result.json"));
    assert_eq!(source.java_clone().result_json_path, None);
    assert_eq!(
        source.clone().result_json_path.as_deref(),
        Some("/tmp/result.json"),
        "the derived Clone is a plain deep copy and does NOT reproduce the Java bug"
    );

    let mut nulled = RouterSettings::new();
    nulled.scoring = None;
    nulled.optimizer = None;
    nulled.fanout = None;
    let cloned = nulled.java_clone();
    assert_eq!(cloned.scoring, Some(ScoringSettings::default()));
    assert_eq!(cloned.optimizer, Some(OptimizerSettings::default()));
    assert_eq!(cloned.fanout, Some(FanoutSettings::default()));
    assert_eq!(nulled.clone().scoring, None);
}

/// Everything `clone()` *does* carry across, from JVM probe row `A`.
#[test]
fn java_clone_carries_every_other_field() {
    let source = populated();
    let cloned = source.java_clone();

    assert_eq!(cloned.algorithm.as_deref(), Some("alg"));
    assert_eq!(cloned.job_timeout_string.as_deref(), Some("1:00:00"));
    assert_eq!(cloned.max_passes, Some(42));
    assert_eq!(cloned.max_items, Some(7));
    assert_eq!(cloned.save_intermediate_stages, Some(true));
    assert_eq!(cloned.copper_to_edge_clearance_um, Some(11.0));
    assert_eq!(cloned.hole_clearance_um, Some(12.0));
    assert_eq!(cloned.neck_width_um, Some(130.0));
    assert_eq!(cloned.strict_drc, Some(true));
    assert_eq!(
        cloned.ignore_net_classes,
        Some(vec!["GND".to_string()]),
        "String[] is deep-copied at :506"
    );
    assert_eq!(cloned.trace_pull_tight_accuracy, Some(33));
    assert_eq!(cloned.enabled, Some(false));
    assert_eq!(cloned.vias_allowed, Some(false));
    assert_eq!(cloned.automatic_neckdown, Some(true));
    assert_eq!(cloned.max_threads, Some(3));
    assert_eq!(cloned.get_layer_count(), 2);
    assert_eq!(cloned.get_bend_cost(0), 2.5);
    assert_eq!(cloned.get_preferred_direction_trace_costs(0), 2.5);
    assert_eq!(
        cloned.scoring.as_ref().unwrap().default_bend_cost,
        Some(1.25)
    );
    assert_eq!(cloned.optimizer.as_ref().unwrap().max_passes, Some(55));
    assert_eq!(cloned.fanout.as_ref().unwrap().max_passes, Some(66));
}

/// The fixture the JVM probe's row `A` was built from, field for field.
fn populated() -> RouterSettings {
    let mut s = RouterSettings::new();
    s.set_layer_count(2);
    s.algorithm = Some("alg".to_string());
    s.job_timeout_string = Some("1:00:00".to_string());
    s.max_passes = Some(42);
    s.max_items = Some(7);
    s.save_intermediate_stages = Some(true);
    s.copper_to_edge_clearance_um = Some(11.0);
    s.hole_clearance_um = Some(12.0);
    s.neck_width_um = Some(130.0);
    s.strict_drc = Some(true);
    s.ignore_net_classes = Some(vec!["GND".to_string()]);
    s.trace_pull_tight_accuracy = Some(33);
    s.enabled = Some(false);
    s.vias_allowed = Some(false);
    s.automatic_neckdown = Some(true);
    s.max_threads = Some(3);
    s.result_json_path = Some("/tmp/result.json".to_string());
    s.set_bend_cost(0, 2.5);
    // Written straight into `scoring` rather than through
    // `set_preferred_direction_trace_costs`, which would also set the `pub(crate)`
    // `board_specific_trace_costs_applied` flag — a field `java_clone_replays_javas_sequence`'s
    // replay cannot write from outside the crate. `:521`'s copy of that flag is pinned by
    // `src/router_settings.rs::java_clone_carries_the_applied_flag` instead.
    s.scoring
        .as_mut()
        .unwrap()
        .preferred_direction_trace_cost
        .as_mut()
        .unwrap()[0] = 2.5;
    s.scoring.as_mut().unwrap().default_bend_cost = Some(1.25);
    s.optimizer.as_mut().unwrap().max_passes = Some(55);
    s.fanout.as_mut().unwrap().max_passes = Some(66);
    s
}

// --- validate / setMaxThreads -----------------------------------------------------------------

/// `RouterSettings.validate` (`:925-965`), row by row, on a 4-processor host. Every expectation
/// is JVM probe block `B`.
#[test]
fn validate_matrix() {
    for (input, expected) in [
        (-1, 9999),
        (10_000, 9999),
        (0, i32::MAX),
        (50, 50),
        (9999, 9999),
    ] {
        let mut s = validatable();
        s.max_passes = Some(input);
        s.validate(&host());
        assert_eq!(s.max_passes, Some(expected), "max_passes {input}");
    }

    for (input, expected) in [
        (None, 3),
        (Some(-1), 3),
        (Some(0), 0),
        (Some(9), 4),
        (Some(2), 2),
        (Some(4), 4),
    ] {
        let mut s = validatable();
        s.max_threads = input;
        s.validate(&host());
        assert_eq!(s.max_threads, Some(expected), "max_threads {input:?}");
    }

    for (input, expected) in [(0, 500), (1, 1), (500, 500), (-7, 500)] {
        let mut s = validatable();
        s.trace_pull_tight_accuracy = Some(input);
        s.validate(&host());
        assert_eq!(
            s.trace_pull_tight_accuracy,
            Some(expected),
            "trace_pull_tight_accuracy {input}"
        );
    }
}

/// `setMaxThreads` -> `normalizeMaxThreads` (`:176-186`, `:137-149`), and the mirror into
/// `optimizer.maxThreads` (`:183-185`). JVM probe block `C`.
#[test]
fn set_max_threads_normalizes_and_mirrors() {
    for (input, expected) in [
        (None, 3),
        (Some(-1), 3),
        (Some(0), 4),
        (Some(9), 4),
        (Some(2), 2),
    ] {
        let mut s = RouterSettings::new();
        s.set_max_threads(input, &host());
        assert_eq!(s.max_threads, Some(expected), "set_max_threads {input:?}");
        assert_eq!(
            s.optimizer.as_ref().unwrap().max_threads,
            Some(expected),
            "optimizer mirror for {input:?}"
        );
    }

    // An absent optimizer is left absent — Java guards with `if (this.optimizer != null)`
    // (`:183`) rather than instantiating. JVM probe row `C.nullOptimizer`.
    let mut s = RouterSettings::new();
    s.optimizer = None;
    s.set_max_threads(Some(0), &host());
    assert_eq!(s.max_threads, Some(4));
    assert!(s.optimizer.is_none());
}

/// Quirk Q4, stated as one test: `validate()` tests `maxThreads > availableProcessors`
/// (`:951` — a strict `>`), so a `0` survives it untouched, while `normalizeMaxThreads` maps `0`
/// to the full processor count (`:144-146`). Two code paths in one class disagree about the same
/// input. JVM rows `B.maxThreads 0 -> 0` and `C.setMaxThreads 0 -> 4`.
#[test]
fn validate_and_normalize_disagree_about_zero_max_threads() {
    let mut validated = validatable();
    validated.max_threads = Some(0);
    validated.validate(&host());

    let mut normalized = RouterSettings::new();
    normalized.set_max_threads(Some(0), &host());

    assert_eq!(validated.max_threads, Some(0));
    assert_eq!(normalized.max_threads, Some(4));
    assert_ne!(validated.max_threads, normalized.max_threads);
}

/// Quirk Q5: `validate()` dereferences `this.maxPasses` unboxed (`:934`), so a settings object
/// that never went through `DefaultSettings` throws. JVM row
/// `B.nullMaxPasses -> java.lang.NullPointerException`.
#[test]
#[should_panic(expected = "RouterSettings.java:934")]
fn validate_panics_without_max_passes() {
    let mut s = RouterSettings::new();
    s.trace_pull_tight_accuracy = Some(500);
    s.validate(&host());
}

/// The same for `tracePullTightAccuracy` (`:958`). JVM row `B.nullTpta`.
#[test]
#[should_panic(expected = "RouterSettings.java:958")]
fn validate_panics_without_trace_pull_tight_accuracy() {
    let mut s = RouterSettings::new();
    s.max_passes = Some(50);
    s.validate(&host());
}

// --- the remaining plain setters ---------------------------------------------------------------

/// `setMaxPasses` (`:167-173`), `setJobTimeoutString` (`:191-197`), `setEnabled` (`:200-206`),
/// `setViasAllowed` (`:209-215`, `:218-220`), `setAutomaticNeckdown` (`:897-899`) — no clamping,
/// no normalisation, and (in the port) no `PropertyChangeSupport`.
#[test]
fn plain_setters() {
    let mut s = RouterSettings::new();
    s.set_max_passes(Some(7));
    assert_eq!(s.max_passes, Some(7));
    s.set_max_passes(None);
    assert_eq!(s.max_passes, None);

    s.set_job_timeout_string(Some("0:30:00".to_string()));
    assert_eq!(s.job_timeout_string.as_deref(), Some("0:30:00"));

    s.set_enabled(Some(false));
    assert_eq!(s.enabled, Some(false));

    s.set_vias_allowed(Some(false));
    assert_eq!(s.vias_allowed, Some(false));
    assert!(!s.get_vias_allowed());
    s.set_vias_allowed(None);
    assert!(s.get_vias_allowed());

    s.set_automatic_neckdown(true);
    assert!(s.get_automatic_neckdown());
    s.strict_drc = Some(true);
    assert!(s.is_strict_drc());
}
