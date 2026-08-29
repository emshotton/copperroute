//! `util/ReflectionUtil.java`'s `setFieldValue` (:21-82), `getFieldByNameOrSerializedName`
//! (:84-115), `snakeToLowerCamel` (:117-130) and `convertValue` (:132-205).
//!
//! The first four tests are the four cases of `util/ReflectionUtilArrayTest.java` (75 lines),
//! ported verbatim. The rest pin the quirks and the conversion tolerances; every expected value
//! was read off a JVM probe (`FProbe`/`DProbe`, JDK 25, `freerouting-current-executable.jar`
//! built 2026-08-27 — the transcript is in task-3-report.md), not inferred from the Java source.

use fr_settings::field_path::{FieldKind, set_field_value};
use fr_settings::{
    BoardUpdateStrategy, FanoutSettings, ItemSelectionStrategy, LayerSettings, MergeError,
    OptimizerSettings, RouterSettings, ScoringSettings,
};

// ---------------------------------------------------------------------------------------------
// ReflectionUtilArrayTest.java, ported verbatim
// ---------------------------------------------------------------------------------------------

/// `ReflectionUtilArrayTest.setSimpleProperty` (:13-21).
#[test]
fn set_simple_property() {
    let mut settings = RouterSettings::new();
    set_field_value(&mut settings, "enabled", "false").expect("resolves");
    assert_eq!(settings.enabled, Some(false));

    set_field_value(&mut settings, "enabled", "true").expect("resolves");
    assert_eq!(settings.enabled, Some(true));
}

/// `ReflectionUtilArrayTest.setNestedArrayPropertiesWhenNull` (:23-33).
#[test]
fn set_nested_array_properties_when_null() {
    let mut settings = RouterSettings::new();
    // settings.layers is initially None
    set_field_value(&mut settings, "layers.routable", "false,true").expect("resolves");

    let layers = settings.layers.as_ref().expect("allocated");
    assert_eq!(layers.len(), 2);
    assert_eq!(layers[0].routable, Some(false));
    assert_eq!(layers[1].routable, Some(true));
}

/// `ReflectionUtilArrayTest.setNestedArrayPropertiesWhenInitialized` (:35-47).
#[test]
fn set_nested_array_properties_when_initialized() {
    let mut settings = RouterSettings::new();
    settings.set_layer_count(2);

    set_field_value(&mut settings, "layers.routable", "false,true").expect("resolves");
    assert_eq!(settings.layers.as_ref().unwrap()[0].routable, Some(false));
    assert_eq!(settings.layers.as_ref().unwrap()[1].routable, Some(true));

    set_field_value(
        &mut settings,
        "layers.preferred_direction_horizontal",
        "true,false",
    )
    .expect("resolves");
    let layers = settings.layers.as_ref().expect("allocated");
    assert_eq!(layers[0].preferred_direction_horizontal, Some(true));
    assert_eq!(layers[1].preferred_direction_horizontal, Some(false));
}

/// `ReflectionUtilArrayTest.caseInsensitiveAndSerializedNameMatching` (:49-73).
#[test]
fn case_insensitive_and_serialized_name_matching() {
    let mut settings = RouterSettings::new();
    settings.set_layer_count(2);

    // Matches via SerializedName value (preferred_direction_horizontal)
    set_field_value(
        &mut settings,
        "layers.preferred_direction_horizontal",
        "true,false",
    )
    .expect("resolves");
    assert_eq!(pdh(&settings), [Some(true), Some(false)], "serialized name");

    // Matches via Java field name in camelCase (preferredDirectionHorizontal)
    set_field_value(
        &mut settings,
        "layers.preferredDirectionHorizontal",
        "false,true",
    )
    .expect("resolves");
    assert_eq!(pdh(&settings), [Some(false), Some(true)], "java field name");

    // Matches via uppercase SCREAMING_SNAKE_CASE
    set_field_value(
        &mut settings,
        "LAYERS.PREFERRED_DIRECTION_HORIZONTAL",
        "true,false",
    )
    .expect("resolves");
    assert_eq!(pdh(&settings), [Some(true), Some(false)], "screaming snake");

    // Matches routable via SerializedName / field name
    set_field_value(&mut settings, "layers.routable", "true,false").expect("resolves");
    let layers = settings.layers.as_ref().expect("allocated");
    assert_eq!(layers[0].routable, Some(true));
    assert_eq!(layers[1].routable, Some(false));
}

fn pdh(settings: &RouterSettings) -> Vec<Option<bool>> {
    settings
        .layers
        .as_ref()
        .expect("allocated")
        .iter()
        .map(|l| l.preferred_direction_horizontal)
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Q14 — `-` is a path separator
// ---------------------------------------------------------------------------------------------

/// `ReflectionUtil.java:23` splits on `[.:\-]`, so a `--router.x-y=` argument silently becomes a
/// two-segment path. JVM probe A: `setFieldValue(s, "optimizer-max_passes", "7")` → `ok`,
/// `optimizer.maxPasses = 7`.
#[test]
fn hyphen_and_colon_are_path_separators() {
    let mut settings = RouterSettings::new();
    set_field_value(&mut settings, "optimizer-max_passes", "7").expect("resolves");
    assert_eq!(settings.optimizer.as_ref().unwrap().max_passes, Some(7));

    // JVM probe H12.
    set_field_value(&mut settings, "optimizer:max_passes", "4").expect("resolves");
    assert_eq!(settings.optimizer.as_ref().unwrap().max_passes, Some(4));
}

/// Only the *path* is split; the value is passed through untouched even when it contains a
/// separator character. JVM probes F5 (`optimizer.hybrid_ratio = "1:1"` → `1:1`) and H13
/// (`algorithm = "freerouting-router"` → `freerouting-router`).
#[test]
fn separator_characters_in_the_value_survive() {
    let mut settings = RouterSettings::new();
    set_field_value(&mut settings, "optimizer.hybrid_ratio", "1:1").expect("resolves");
    assert_eq!(
        settings.optimizer.as_ref().unwrap().hybrid_ratio.as_deref(),
        Some("1:1")
    );

    set_field_value(&mut settings, "algorithm", "freerouting-router").expect("resolves");
    assert_eq!(settings.algorithm.as_deref(), Some("freerouting-router"));
}

// ---------------------------------------------------------------------------------------------
// Q15 — array navigation
// ---------------------------------------------------------------------------------------------

/// `ReflectionUtil.java:61-72` writes `min(arrayLength, tokenCount)` elements: extra tokens are
/// dropped without a word. JVM probe C1: `layers.routable = "a,b,c"` on a 2-element array → `ok`,
/// `len=2`, `routable=false,false` (the two written tokens both parse as `false` — Q16).
#[test]
fn extra_array_tokens_are_dropped() {
    let mut settings = RouterSettings::new();
    settings.set_layer_count(2);
    set_field_value(&mut settings, "layers.routable", "a,b,c").expect("resolves");

    let layers = settings.layers.as_ref().expect("allocated");
    assert_eq!(layers.len(), 2);
    assert_eq!(layers[0].routable, Some(false));
    assert_eq!(layers[1].routable, Some(false));
}

/// `ReflectionUtil.java:56-59` allocates the array at the *token count*, not at the board's real
/// layer count. JVM probe C2: `layers.routable = "a,b,c"` on a null array → `len=3`.
#[test]
fn null_array_is_allocated_at_the_token_count() {
    let mut settings = RouterSettings::new();
    set_field_value(&mut settings, "layers.routable", "a,b,c").expect("resolves");
    assert_eq!(settings.layers.as_ref().expect("allocated").len(), 3);
}

/// A target longer than the token list keeps its extra elements untouched (`limit` is the
/// minimum). JVM probe C5: 3 layers, `"true,true"` → `[true,true,true]` — element 2 keeps the
/// `routable = true` `setLayerCount` gave it.
#[test]
fn extra_array_elements_are_left_untouched() {
    let mut settings = RouterSettings::new();
    settings.set_layer_count(3);
    set_field_value(&mut settings, "layers.bend_cost", "1.5,2.5").expect("resolves");

    let layers = settings.layers.as_ref().expect("allocated");
    assert_eq!(layers.len(), 3);
    assert_eq!(layers[0].bend_cost, Some(1.5));
    assert_eq!(layers[1].bend_cost, Some(2.5));
    assert_eq!(layers[2].bend_cost, None);
}

/// Java's `String.split(",")` drops trailing empty tokens, so a trailing comma does not create a
/// third element. JVM probe C4: `"false,true,"` on a null array → `len=2`.
#[test]
fn a_trailing_comma_does_not_add_an_array_element() {
    let mut settings = RouterSettings::new();
    set_field_value(&mut settings, "layers.routable", "false,true,").expect("resolves");
    assert_eq!(settings.layers.as_ref().expect("allocated").len(), 2);
}

// ---------------------------------------------------------------------------------------------
// Q16 — Boolean.parseBoolean
// ---------------------------------------------------------------------------------------------

/// `ReflectionUtil.java:145-154`: `"0"` → `false`, `"1"` → `true`, otherwise
/// `Boolean.parseBoolean` — case-insensitive `"true"`, and **anything else is silently `false`**,
/// with no error. JVM probe B: `yes` → `false` (ok), `TRUE` → `true`, `1` → `true`, `0` →
/// `false`, `" true "` (with spaces) → `false`.
#[test]
fn boolean_conversion_silently_falls_back_to_false() {
    let mut settings = RouterSettings::new();

    set_field_value(&mut settings, "enabled", "yes").expect("no error — that is the quirk");
    assert_eq!(settings.enabled, Some(false));

    set_field_value(&mut settings, "enabled", "TRUE").expect("resolves");
    assert_eq!(settings.enabled, Some(true));

    set_field_value(&mut settings, "enabled", "0").expect("resolves");
    assert_eq!(settings.enabled, Some(false));

    set_field_value(&mut settings, "enabled", "1").expect("resolves");
    assert_eq!(settings.enabled, Some(true));

    // Boolean.parseBoolean does not trim.
    set_field_value(&mut settings, "enabled", " true ").expect("resolves");
    assert_eq!(settings.enabled, Some(false));
}

/// The array-navigation branch trims each token before handing it to `convertValue`
/// (`ReflectionUtil.java:71`, `valTokens[i].trim()`), but a scalar leaf's value reaches
/// `Boolean.parseBoolean` untrimmed — so the same `" true "` means `true` through an array and
/// `false` at the top level. JVM-verified: `layers.routable = " true , true "` gives
/// `true,true` while `enabled = " true "` gives `false`.
#[test]
fn array_tokens_are_trimmed_but_scalar_leaves_are_not() {
    let mut settings = RouterSettings::new();
    settings.set_layer_count(2);
    set_field_value(&mut settings, "layers.routable", " true , true ").expect("resolves");
    let layers = settings.layers.as_ref().expect("allocated");
    assert_eq!(layers[0].routable, Some(true));
    assert_eq!(layers[1].routable, Some(true));

    set_field_value(&mut settings, "enabled", " true ").expect("resolves");
    assert_eq!(settings.enabled, Some(false));
}

// ---------------------------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------------------------

/// `ReflectionUtil.java:158-164` matches the **Java constant name**, case-insensitively, after
/// `.trim()`. A name that matches nothing falls through the whole `convertValue` chain and Java
/// then `field.set`s a raw `String` into an enum field — `IllegalArgumentException`, which the
/// port returns as [`MergeError::EnumName`]. JVM probe F: `global_optimal` → `GLOBAL_OPTIMAL`,
/// `GLOBAL_OPTIMAL` → `GLOBAL_OPTIMAL`, `" hybrid "` → `HYBRID`, `globalOptimal` →
/// `IllegalArgumentException`.
#[test]
fn enum_matching_is_case_insensitive_on_the_java_constant_name() {
    let mut settings = RouterSettings::new();

    set_field_value(
        &mut settings,
        "optimizer.board_update_strategy",
        "global_optimal",
    )
    .expect("resolves");
    assert_eq!(
        settings.optimizer.as_ref().unwrap().board_update_strategy,
        Some(BoardUpdateStrategy::GlobalOptimal)
    );

    settings.optimizer.as_mut().unwrap().board_update_strategy = None;
    set_field_value(
        &mut settings,
        "optimizer.board_update_strategy",
        "GLOBAL_OPTIMAL",
    )
    .expect("resolves");
    assert_eq!(
        settings.optimizer.as_ref().unwrap().board_update_strategy,
        Some(BoardUpdateStrategy::GlobalOptimal)
    );

    set_field_value(&mut settings, "optimizer.board_update_strategy", " hybrid ")
        .expect("resolves");
    assert_eq!(
        settings.optimizer.as_ref().unwrap().board_update_strategy,
        Some(BoardUpdateStrategy::Hybrid)
    );

    // camelCase is not a Java constant name — no match anywhere in the chain.
    let err = set_field_value(
        &mut settings,
        "optimizer.board_update_strategy",
        "globalOptimal",
    )
    .expect_err("no constant matches");
    assert!(matches!(err, MergeError::EnumName { .. }), "got {err:?}");
    assert_eq!(
        settings.optimizer.as_ref().unwrap().board_update_strategy,
        Some(BoardUpdateStrategy::Hybrid),
        "the failed write leaves the old value in place"
    );

    set_field_value(
        &mut settings,
        "optimizer.item_selection_strategy",
        "prioritized",
    )
    .expect("resolves");
    assert_eq!(
        settings.optimizer.as_ref().unwrap().item_selection_strategy,
        Some(ItemSelectionStrategy::Prioritized)
    );
}

// ---------------------------------------------------------------------------------------------
// Numbers — two different whitespace tolerances in one file
// ---------------------------------------------------------------------------------------------

/// `Integer.parseInt` rejects surrounding whitespace (`ReflectionUtil.java:136-138`) while
/// `Double.parseDouble` trims it (`:142-143`). JVM probe E1 (`max_passes = " 7 "` →
/// `NumberFormatException`) and E4 (`copper_to_edge_clearance_um = " 7 "` → `7.0`).
#[test]
fn int_rejects_whitespace_but_double_trims_it() {
    let mut settings = RouterSettings::new();

    let err = set_field_value(&mut settings, "max_passes", " 7 ").expect_err("parseInt is strict");
    assert!(
        matches!(err, MergeError::NumberFormat { .. }),
        "got {err:?}"
    );
    assert_eq!(settings.max_passes, None);

    set_field_value(&mut settings, "copper_to_edge_clearance_um", " 7 ")
        .expect("parseDouble trims");
    assert_eq!(settings.copper_to_edge_clearance_um, Some(7.0));
}

/// `Integer.parseInt` accepts a leading `+` and rejects underscores and overflow. JVM probe E2
/// (`+7` → 7), E3 (`7_0` → `NumberFormatException`), E16 (`99999999999` →
/// `NumberFormatException`).
#[test]
fn int_conversion_follows_parse_int() {
    let mut settings = RouterSettings::new();

    set_field_value(&mut settings, "max_passes", "+7").expect("resolves");
    assert_eq!(settings.max_passes, Some(7));

    assert!(matches!(
        set_field_value(&mut settings, "max_passes", "7_0"),
        Err(MergeError::NumberFormat { .. })
    ));
    assert!(matches!(
        set_field_value(&mut settings, "max_passes", "99999999999"),
        Err(MergeError::NumberFormat { .. })
    ));
    assert_eq!(
        settings.max_passes,
        Some(7),
        "both failures leave 7 in place"
    );
}

/// `Double.parseDouble` accepts scientific notation, a trailing `d`/`f` suffix, exactly-spelled
/// `Infinity`/`NaN`, and a bare leading `.`/trailing `.`; it rejects the lowercase `inf` spelling
/// Rust's own `f64::from_str` accepts, and the empty string. JVM probe E5-E13 and DProbe.
#[test]
fn double_conversion_follows_parse_double() {
    let mut settings = RouterSettings::new();

    for (input, expected) in [
        ("1e5", 100_000.0),
        ("5d", 5.0),
        ("5F", 5.0),
        (".5", 0.5),
        ("5.", 5.0),
    ] {
        set_field_value(&mut settings, "hole_clearance_um", input).expect(input);
        assert_eq!(
            settings.hole_clearance_um,
            Some(expected),
            "input {input:?}"
        );
    }

    set_field_value(&mut settings, "hole_clearance_um", "Infinity").expect("resolves");
    assert_eq!(settings.hole_clearance_um, Some(f64::INFINITY));
    set_field_value(&mut settings, "hole_clearance_um", "-Infinity").expect("resolves");
    assert_eq!(settings.hole_clearance_um, Some(f64::NEG_INFINITY));
    set_field_value(&mut settings, "hole_clearance_um", "NaN").expect("resolves");
    assert!(settings.hole_clearance_um.expect("set").is_nan());

    for bad in [
        "inf",
        "infinity",
        "nan",
        "",
        " ",
        "1e",
        ".",
        "5.5.5",
        "--5",
        "1_0",
        "Infinityd",
    ] {
        assert!(
            matches!(
                set_field_value(&mut settings, "hole_clearance_um", bad),
                Err(MergeError::NumberFormat { .. })
            ),
            "{bad:?} should not parse"
        );
    }
}

/// `Long.parseLong` for `FanoutSettings.maxMillisecondsPerPin` and `Float.parseFloat` for the
/// three `Float` penalties. JVM probe E14 (`9000000000` → `9000000000`) and E15 (`1e40` →
/// `Infinity`, because `Float.parseFloat` overflows to infinity rather than throwing).
#[test]
fn long_and_float_fields_use_their_own_parsers() {
    let mut settings = RouterSettings::new();

    set_field_value(
        &mut settings,
        "fanout.max_milliseconds_per_pin",
        "9000000000",
    )
    .expect("resolves");
    assert_eq!(
        settings.fanout.as_ref().unwrap().max_milliseconds_per_pin,
        Some(9_000_000_000)
    );

    set_field_value(&mut settings, "scoring.unrouted_net_penalty", "1e40").expect("resolves");
    assert_eq!(
        settings.scoring.as_ref().unwrap().unrouted_net_penalty,
        Some(f32::INFINITY)
    );
}

// ---------------------------------------------------------------------------------------------
// List-valued leaves
// ---------------------------------------------------------------------------------------------

/// `double[]` (`ReflectionUtil.java:179-190`): trim the whole value, split on `,`, trim each
/// token. JVM probe G1: `scoring.preferred_direction_trace_cost = "1.5, 2.0"` → `[1.5, 2.0]`.
#[test]
fn double_array_leaf_splits_and_trims() {
    let mut settings = RouterSettings::new();
    set_field_value(
        &mut settings,
        "scoring.preferred_direction_trace_cost",
        "1.5, 2.0",
    )
    .expect("resolves");
    assert_eq!(
        settings
            .scoring
            .as_ref()
            .unwrap()
            .preferred_direction_trace_cost
            .as_deref(),
        Some([1.5, 2.0].as_slice())
    );

    // A token that is not a number is a NumberFormatException, not a silent skip (probe G5).
    assert!(matches!(
        set_field_value(&mut settings, "scoring.preferred_direction_trace_cost", "x"),
        Err(MergeError::NumberFormat { .. })
    ));
}

/// `String[]` (`ReflectionUtil.java:165-178`): an empty value gives a zero-length array; Java's
/// `split(",")` drops trailing empty tokens but keeps interior ones. JVM probes G2
/// (`" a , b ,"` → `[a, b]`), G3 (`""` → `[]`), G4 (`"a,,b"` → `[a, , b]`).
#[test]
fn string_array_leaf_follows_java_split() {
    let mut settings = RouterSettings::new();

    set_field_value(&mut settings, "ignore_net_classes", " a , b ,").expect("resolves");
    assert_eq!(
        settings.ignore_net_classes.as_deref(),
        Some(["a".to_string(), "b".to_string()].as_slice())
    );

    set_field_value(&mut settings, "ignore_net_classes", "").expect("resolves");
    assert_eq!(settings.ignore_net_classes.as_deref(), Some([].as_slice()));

    set_field_value(&mut settings, "ignore_net_classes", "a,,b").expect("resolves");
    assert_eq!(
        settings.ignore_net_classes.as_deref(),
        Some(["a".to_string(), String::new(), "b".to_string()].as_slice())
    );
}

// ---------------------------------------------------------------------------------------------
// Name resolution
// ---------------------------------------------------------------------------------------------

/// Every spelling `getFieldByNameOrSerializedName` accepts, on fields whose `@SerializedName`
/// value, `alternate` and Java field name all differ. JVM probes D4-D13 and H15-H20.
#[test]
fn serialized_alternate_and_java_names_all_resolve() {
    let mut settings = RouterSettings::new();

    // @SerializedName value, its alternate, and the SCREAMING form of the Java name.
    set_field_value(&mut settings, "trace_pull_tight_accuracy", "8").expect("serialized");
    assert_eq!(settings.trace_pull_tight_accuracy, Some(8));
    set_field_value(&mut settings, "tracePullTightAccuracy", "9").expect("alternate");
    assert_eq!(settings.trace_pull_tight_accuracy, Some(9));
    set_field_value(&mut settings, "TRACEPULLTIGHTACCURACY", "7").expect("java name, any case");
    assert_eq!(settings.trace_pull_tight_accuracy, Some(7));

    // The serialized name and the Java field name are unrelated words.
    set_field_value(&mut settings, "allowed_via_types", "true").expect("serialized");
    assert_eq!(settings.vias_allowed, Some(true));
    set_field_value(&mut settings, "vias_allowed", "false").expect("snake of the java name");
    assert_eq!(settings.vias_allowed, Some(false));
    set_field_value(&mut settings, "viasAllowed", "true").expect("java name");
    assert_eq!(settings.vias_allowed, Some(true));

    // `job_timeout` (serialized) vs `jobTimeoutString` (Java) — and the snake form of the latter.
    set_field_value(&mut settings, "job_timeout", "5m").expect("serialized");
    assert_eq!(settings.job_timeout_string.as_deref(), Some("5m"));
    set_field_value(&mut settings, "jobTimeoutString", "6m").expect("java name");
    assert_eq!(settings.job_timeout_string.as_deref(), Some("6m"));
    set_field_value(&mut settings, "job_timeout_string", "7m").expect("snake of the java name");
    assert_eq!(settings.job_timeout_string.as_deref(), Some("7m"));

    set_field_value(&mut settings, "result_json", "/tmp/x").expect("serialized");
    assert_eq!(settings.result_json_path.as_deref(), Some("/tmp/x"));
    set_field_value(&mut settings, "resultJsonPath", "/tmp/y").expect("java name");
    assert_eq!(settings.result_json_path.as_deref(), Some("/tmp/y"));

    // Nested structs resolve the same way.
    set_field_value(&mut settings, "scoring.viaCosts", "3").expect("alternate");
    assert_eq!(settings.scoring.as_ref().unwrap().via_costs, Some(3));
    set_field_value(&mut settings, "scoring.via_costs", "4").expect("serialized");
    assert_eq!(settings.scoring.as_ref().unwrap().via_costs, Some(4));
    set_field_value(&mut settings, "fanout.ripupAllowed", "true").expect("alternate");
    assert_eq!(settings.fanout.as_ref().unwrap().ripup_allowed, Some(true));
    set_field_value(&mut settings, "optimizer.improvement_threshold", "0.5").expect("serialized");
    assert_eq!(
        settings
            .optimizer
            .as_ref()
            .unwrap()
            .optimization_improvement_threshold,
        Some(0.5)
    );
    set_field_value(
        &mut settings,
        "optimizer.optimization_improvement_threshold",
        "0.25",
    )
    .expect("snake of the java name");
    assert_eq!(
        settings
            .optimizer
            .as_ref()
            .unwrap()
            .optimization_improvement_threshold,
        Some(0.25)
    );
    set_field_value(&mut settings, "optimizer.timeout", "9m").expect("serialized");
    assert_eq!(
        settings
            .optimizer
            .as_ref()
            .unwrap()
            .timeout_string
            .as_deref(),
        Some("9m")
    );
}

/// `ReflectionUtil.java:114`: no field of any spelling → `NoSuchFieldException`. JVM probe H1.
#[test]
fn an_unknown_name_is_no_such_field() {
    let mut settings = RouterSettings::new();
    let err = set_field_value(&mut settings, "nope", "1").expect_err("no such field");
    assert!(matches!(err, MergeError::NoSuchField { .. }), "got {err:?}");
}

/// An intermediate `None` struct field is instantiated before recursing
/// (`ReflectionUtil.java:76-79`). JVM probe H21.
#[test]
fn a_null_nested_object_is_instantiated() {
    let mut settings = RouterSettings::default(); // every field None, including `fanout`
    assert_eq!(settings.fanout, None);
    set_field_value(&mut settings, "fanout.max_passes", "3").expect("resolves");
    assert_eq!(settings.fanout.as_ref().unwrap().max_passes, Some(3));
}

/// Java's `split` drops trailing empty segments, so a trailing separator is invisible; an
/// interior empty segment is not, and matches no field. JVM probes H10 (`enabled.` → ok) and H11
/// (`optimizer..max_passes` → `NoSuchFieldException` with an empty name).
#[test]
fn empty_path_segments_follow_java_split() {
    let mut settings = RouterSettings::new();

    set_field_value(&mut settings, "enabled.", "false").expect("trailing separator is dropped");
    assert_eq!(settings.enabled, Some(false));

    assert!(matches!(
        set_field_value(&mut settings, "optimizer..max_passes", "3"),
        Err(MergeError::NoSuchField { .. })
    ));
}

// ---------------------------------------------------------------------------------------------
// Divergences and totalizations (Java throws where the port returns an error)
// ---------------------------------------------------------------------------------------------

/// Java's `getDeclaredFields()` includes `static final` constants, so `min_bend_cost` resolves to
/// `RouterSettings.MIN_BEND_COST` and then fails at `field.set` with an `IllegalAccessException`
/// (JVM probes H2/H3). The port has no such struct field, so it fails one step earlier with
/// [`MergeError::NoSuchField`] — an error either way, with a different name.
#[test]
fn java_static_constants_are_not_settable_fields() {
    let mut settings = RouterSettings::new();
    assert!(matches!(
        set_field_value(&mut settings, "min_bend_cost", "1"),
        Err(MergeError::NoSuchField { .. })
    ));
    assert!(matches!(
        set_field_value(&mut settings, "ALGORITHM_CURRENT", "x"),
        Err(MergeError::NoSuchField { .. })
    ));
}

/// Assigning a scalar string to a struct- or array-typed field falls through `convertValue` and
/// blows up at `field.set` (`IllegalArgumentException`; JVM probes H5/H6). Navigating *through* a
/// scalar field blows up trying to instantiate it (`NoSuchMethodException`; probe H7). All three
/// become [`MergeError::TypeMismatch`].
#[test]
fn type_mismatches_are_errors_not_panics() {
    let mut settings = RouterSettings::new();
    for path in ["fanout", "layers", "enabled.foo", "max_passes.value"] {
        let err = set_field_value(&mut settings, path, "x").expect_err(path);
        assert!(
            matches!(err, MergeError::TypeMismatch { .. }),
            "{path}: {err:?}"
        );
    }
}

/// `Double.parseDouble` accepts the hexadecimal floating-point grammar (`0x1p3` → `8.0`, DProbe).
/// The port does not implement it and returns [`MergeError::NumberFormat`] instead — a recorded
/// divergence, pinned here so it cannot change silently.
#[test]
fn hexadecimal_float_literals_are_a_recorded_divergence() {
    let mut settings = RouterSettings::new();
    assert!(matches!(
        set_field_value(&mut settings, "hole_clearance_um", "0x1p3"),
        Err(MergeError::NumberFormat { .. })
    ));
}

/// A path that is nothing but separators splits to a zero-length array and Java indexes it —
/// `ArrayIndexOutOfBoundsException` (JVM probes H8/H9). The port returns
/// [`MergeError::NoSuchField`] rather than panicking.
#[test]
fn an_all_separator_path_is_an_error_not_a_panic() {
    let mut settings = RouterSettings::new();
    for path in [".", "-", ":", "..", "_"] {
        assert!(
            matches!(
                set_field_value(&mut settings, path, "1"),
                Err(MergeError::NoSuchField { .. })
            ),
            "{path:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// The field tables cannot drift from Task 1's declaration-order pins
// ---------------------------------------------------------------------------------------------

/// Every struct's `FIELDS` table lists exactly the fields of its `FIELD_NAMES` pin, in the same
/// (Java declaration) order — the property lookup and the merge engine walk the same table.
#[test]
fn field_tables_match_the_declaration_order_pins() {
    fn rust_names(fields: &[fr_settings::field_path::FieldSpec]) -> Vec<&'static str> {
        fields.iter().map(|f| f.rust_name).collect()
    }

    assert_eq!(
        rust_names(RouterSettings::FIELDS),
        RouterSettings::FIELD_NAMES
    );
    assert_eq!(
        rust_names(LayerSettings::FIELDS),
        LayerSettings::FIELD_NAMES
    );
    assert_eq!(
        rust_names(ScoringSettings::FIELDS),
        ScoringSettings::FIELD_NAMES
    );
    assert_eq!(
        rust_names(OptimizerSettings::FIELDS),
        OptimizerSettings::FIELD_NAMES
    );
    assert_eq!(
        rust_names(FanoutSettings::FIELDS),
        FanoutSettings::FIELD_NAMES
    );
}

/// The four fields that navigate rather than convert are exactly Java's three nested objects plus
/// the one object array — nothing else may claim [`FieldKind::Nested`]/[`FieldKind::ObjectArray`],
/// because that is what decides `setPropertyRecursive`'s array branch (`ReflectionUtil.java:46`).
#[test]
fn only_the_four_navigable_router_fields_are_navigable() {
    let navigable: Vec<&str> = RouterSettings::FIELDS
        .iter()
        .filter(|f| matches!(f.kind, FieldKind::Nested | FieldKind::ObjectArray))
        .map(|f| f.rust_name)
        .collect();
    assert_eq!(navigable, ["fanout", "layers", "optimizer", "scoring"]);
}
