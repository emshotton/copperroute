//! `util/ReflectionUtil.java`'s `setFieldValue` (:21-25), `setPropertyRecursive` (:27-82),
//! `getFieldByNameOrSerializedName` (:84-115), `snakeToLowerCamel` (:117-130) and `convertValue`
//! (:132-205): assigning a string value to a nested settings field addressed by a textual
//! property path.
//!
//! This is the half of `ReflectionUtil` the *string-keyed* settings sources use — Task 7's
//! `EnvironmentVariablesSource` (`FREEROUTING__ROUTER__OPTIMIZER__MAX_THREADS=8`) and
//! `CliSettings` (`--router.optimizer.max_threads=8`). [`crate::copy_fields`] is the other half,
//! `copyFields`, which merges two already-typed objects.
//!
//! ## Name resolution is a static table, not reflection
//!
//! Java reads `clazz.getDeclaredFields()` and each field's `@SerializedName`. Rust has no runtime
//! field table, so each struct carries an explicit `FIELDS: &[FieldSpec]` const listing its
//! fields **in Java declaration order** — the same order [`crate::copy_fields`] walks and each
//! struct's `FIELD_NAMES` pins. `tests/field_path.rs`'s
//! `field_tables_match_the_declaration_order_pins` asserts the two tables cannot drift apart.
//!
//! Two consequences of Java's `getDeclaredFields()` that the port cannot reproduce, both of them
//! errors on either side:
//! - Java's list includes `RouterSettings`' four `public static final` constants (`:15-18`), so
//!   `min_bend_cost` resolves to `MIN_BEND_COST` and then throws `IllegalAccessException` at
//!   `field.set`. Rust has no such struct field, so the port fails one step earlier with
//!   [`MergeError::NoSuchField`].
//! - Java's list is not filtered by modifier (unlike `copyFields`, which skips non-`public`
//!   fields at `:226-228`), and `setAccessible(true)` at `:36` opens the private ones. So
//!   `board_specific_trace_costs_applied` **is** settable through a property path even though
//!   `copyFields` never copies it — reproduced here, and pinned by `private_fields_are_settable`.
//!
//! Both are `docs/java-quirks.md` row 121.
//!
//! ## What is deliberately not reproduced
//!
//! not ported: getFieldByNameOrSerializedName's superclass fallback (`ReflectionUtil.java:111-113`)
//! — none of `RouterSettings`, `LayerSettings`, `ScoringSettings`, `OptimizerSettings` or
//! `FanoutSettings` extends anything but `Object`, so the recursion is unreachable for every type
//! this crate resolves paths against.
//!
//! `Double.parseDouble`'s **hexadecimal floating-point** grammar (`0x1p3` → `8.0`, verified
//! against the JVM) is not implemented: [`java_parse_f64`] returns [`MergeError::NumberFormat`]
//! for it. Rust's own `f64::from_str` has no hex-float form either, and no settings source would
//! plausibly carry one; the divergence is number-vs-error, never a wrong number. Recorded as a
//! divergence in `docs/java-quirks.md` and pinned by
//! `hexadecimal_float_literals_are_a_recorded_divergence`. Both divergences are quirks row 122.
//!
//! `Integer.parseInt`/`Long.parseLong` accept **any Unicode decimal digit** (`Character.digit`,
//! so `"٣"` parses as `3`); Rust's `i32::from_str` is ASCII-only. Same divergence class:
//! an error here where Java produced a number, for input no settings source would produce.

use crate::{
    BoardUpdateStrategy, FanoutSettings, ItemSelectionStrategy, JavaEnum, LayerSettings,
    MergeError, OptimizerSettings, RouterSettings, ScoringSettings,
};

// -------------------------------------------------------------------------------------------
// The field table
// -------------------------------------------------------------------------------------------

/// What `convertValue` (`ReflectionUtil.java:132-205`) must do with a value bound for this field,
/// or — for [`FieldKind::Nested`] and [`FieldKind::ObjectArray`] — that the field is navigated
/// through rather than converted into (`:46`, `:73`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    /// `Boolean` — [`java_parse_bool`] (`:145-154`).
    Bool,
    /// `Integer` — [`java_parse_i32`] (`:136-138`).
    I32,
    /// `Long` — [`java_parse_i64`] (`:139-141`).
    I64,
    /// `Float` — [`java_parse_f32`] (`:155-157`).
    F32,
    /// `Double` — [`java_parse_f64`] (`:142-144`).
    F64,
    /// `String` — the value verbatim (`:133-135`, `targetType.isInstance(value)`).
    Str,
    /// A Java `enum`, carrying its constant names in declaration order; matched
    /// case-insensitively after `.trim()` (`:158-164`).
    Enum(&'static [&'static str]),
    /// `String[]` — [`java_parse_string_vec`] (`:165-178`).
    StringVec,
    /// `double[]` — [`java_parse_f64_vec`] (`:179-190`).
    F64Vec,
    /// `int[]` — [`java_parse_i32_vec`] (`:191-202`). No field of the ported structs has this
    /// type; the arm exists because `convertValue` has it and a later plan may add such a field.
    I32Vec,
    /// A nested settings object: navigated into, instantiating it first when absent (`:73-81`).
    Nested,
    /// An array of settings objects (`RouterSettings.layers`): navigated into with the
    /// comma-splitting rule at `:46-72`.
    ObjectArray,
}

/// One field of a settings struct, as `getFieldByNameOrSerializedName`
/// (`ReflectionUtil.java:84-115`) sees it.
#[derive(Debug, Clone, Copy)]
pub struct FieldSpec {
    /// The `@SerializedName` value, or `""` for a field carrying no annotation — which stands in
    /// for Java's `if (annotation != null)` guard at `:89`, and must never match a path segment.
    pub serialized: &'static str,
    /// The `@SerializedName` `alternate` list (`:96-103`).
    pub alternates: &'static [&'static str],
    /// The Java field name, as `Field.getName()` returns it (`:105-109`).
    pub java_name: &'static str,
    /// The Rust field name — the key the assignment `match`es on, and the name pinned by each
    /// struct's `FIELD_NAMES`.
    pub rust_name: &'static str,
    /// How `convertValue` treats the field, or that it is navigated through.
    pub kind: FieldKind,
}

const BOARD_UPDATE_STRATEGY_NAMES: &[&str] = &["GREEDY", "GLOBAL_OPTIMAL", "HYBRID"];
const ITEM_SELECTION_STRATEGY_NAMES: &[&str] = &["SEQUENTIAL", "RANDOM", "PRIORITIZED"];

/// Shorthand for a field with no `alternate` list.
const fn spec(
    serialized: &'static str,
    java_name: &'static str,
    rust_name: &'static str,
    kind: FieldKind,
) -> FieldSpec {
    FieldSpec {
        serialized,
        alternates: &[],
        java_name,
        rust_name,
        kind,
    }
}

/// Shorthand for a field with a one-entry `alternate` list.
const fn spec_alt(
    serialized: &'static str,
    alternates: &'static [&'static str],
    java_name: &'static str,
    rust_name: &'static str,
    kind: FieldKind,
) -> FieldSpec {
    FieldSpec {
        serialized,
        alternates,
        java_name,
        rust_name,
        kind,
    }
}

impl RouterSettings {
    /// `RouterSettings.getDeclaredFields()` (`RouterSettings.java:20-111`) minus the four
    /// `public static final` constants and the dropped `pcs` — see the module doc comment.
    pub const FIELDS: &'static [FieldSpec] = &[
        spec("enabled", "enabled", "enabled", FieldKind::Bool),
        spec("algorithm", "algorithm", "algorithm", FieldKind::Str),
        spec("fanout", "fanout", "fanout", FieldKind::Nested),
        spec(
            "copper_to_edge_clearance_um",
            "copperToEdgeClearanceUm",
            "copper_to_edge_clearance_um",
            FieldKind::F64,
        ),
        spec(
            "hole_clearance_um",
            "holeClearanceUm",
            "hole_clearance_um",
            FieldKind::F64,
        ),
        spec(
            "neck_width_um",
            "neckWidthUm",
            "neck_width_um",
            FieldKind::F64,
        ),
        spec("strict_drc", "strictDrc", "strict_drc", FieldKind::Bool),
        spec(
            "job_timeout",
            "jobTimeoutString",
            "job_timeout_string",
            FieldKind::Str,
        ),
        spec("max_passes", "maxPasses", "max_passes", FieldKind::I32),
        spec("max_items", "maxItems", "max_items", FieldKind::I32),
        spec("layers", "layers", "layers", FieldKind::ObjectArray),
        spec(
            "save_intermediate_stages",
            "saveIntermediateStages",
            "save_intermediate_stages",
            FieldKind::Bool,
        ),
        spec(
            "ignore_net_classes",
            "ignoreNetClasses",
            "ignore_net_classes",
            FieldKind::StringVec,
        ),
        spec_alt(
            "trace_pull_tight_accuracy",
            &["tracePullTightAccuracy"],
            "tracePullTightAccuracy",
            "trace_pull_tight_accuracy",
            FieldKind::I32,
        ),
        spec(
            "allowed_via_types",
            "viasAllowed",
            "vias_allowed",
            FieldKind::Bool,
        ),
        spec_alt(
            "automatic_neckdown",
            &["automaticNeckdown"],
            "automaticNeckdown",
            "automatic_neckdown",
            FieldKind::Bool,
        ),
        spec("optimizer", "optimizer", "optimizer", FieldKind::Nested),
        spec("scoring", "scoring", "scoring", FieldKind::Nested),
        spec("max_threads", "maxThreads", "max_threads", FieldKind::I32),
        spec(
            "result_json",
            "resultJsonPath",
            "result_json_path",
            FieldKind::Str,
        ),
        // `private transient`, and with no `@SerializedName` (RouterSettings.java:110) — so its
        // `serialized` is the empty sentinel. Java's lookup does not filter by modifier, so this
        // one is reachable from a property path even though `copyFields` skips it.
        spec(
            "",
            "boardSpecificTraceCostsApplied",
            "board_specific_trace_costs_applied",
            FieldKind::Bool,
        ),
    ];
}

impl LayerSettings {
    /// `LayerSettings.getDeclaredFields()` (`LayerSettings.java:9-21`).
    pub const FIELDS: &'static [FieldSpec] = &[
        spec("routable", "routable", "routable", FieldKind::Bool),
        spec(
            "preferred_direction_horizontal",
            "preferredDirectionHorizontal",
            "preferred_direction_horizontal",
            FieldKind::Bool,
        ),
        spec("bend_cost", "bendCost", "bend_cost", FieldKind::F64),
    ];
}

impl ScoringSettings {
    /// `ScoringSettings.getDeclaredFields()` (`ScoringSettings.java:29-81`).
    pub const FIELDS: &'static [FieldSpec] = &[
        spec(
            "preferred_direction_trace_cost",
            "preferredDirectionTraceCost",
            "preferred_direction_trace_cost",
            FieldKind::F64Vec,
        ),
        spec(
            "undesired_direction_trace_cost",
            "undesiredDirectionTraceCost",
            "undesired_direction_trace_cost",
            FieldKind::F64Vec,
        ),
        spec(
            "default_preferred_direction_trace_cost",
            "defaultPreferredDirectionTraceCost",
            "default_preferred_direction_trace_cost",
            FieldKind::F64,
        ),
        spec(
            "default_undesired_direction_trace_cost",
            "defaultUndesiredDirectionTraceCost",
            "default_undesired_direction_trace_cost",
            FieldKind::F64,
        ),
        spec_alt(
            "via_costs",
            &["viaCosts"],
            "viaCosts",
            "via_costs",
            FieldKind::I32,
        ),
        spec(
            "plane_via_costs",
            "planeViaCosts",
            "plane_via_costs",
            FieldKind::I32,
        ),
        spec_alt(
            "start_ripup_costs",
            &["startRipupCosts"],
            "startRipupCosts",
            "start_ripup_costs",
            FieldKind::I32,
        ),
        spec(
            "unrouted_net_penalty",
            "unroutedNetPenalty",
            "unrouted_net_penalty",
            FieldKind::F32,
        ),
        spec(
            "clearance_violation_penalty",
            "clearanceViolationPenalty",
            "clearance_violation_penalty",
            FieldKind::F32,
        ),
        spec(
            "bend_penalty",
            "bendPenalty",
            "bend_penalty",
            FieldKind::F32,
        ),
        spec(
            "default_bend_cost",
            "defaultBendCost",
            "default_bend_cost",
            FieldKind::F64,
        ),
    ];
}

impl OptimizerSettings {
    /// `OptimizerSettings.getDeclaredFields()` (`OptimizerSettings.java:14-95`).
    pub const FIELDS: &'static [FieldSpec] = &[
        spec("enabled", "enabled", "enabled", FieldKind::Bool),
        spec("algorithm", "algorithm", "algorithm", FieldKind::Str),
        spec("max_passes", "maxPasses", "max_passes", FieldKind::I32),
        spec("max_items", "maxItems", "max_items", FieldKind::I32),
        spec("max_threads", "maxThreads", "max_threads", FieldKind::I32),
        spec(
            "improvement_threshold",
            "optimizationImprovementThreshold",
            "optimization_improvement_threshold",
            FieldKind::F32,
        ),
        spec(
            "max_consecutive_failures",
            "maxConsecutiveFailures",
            "max_consecutive_failures",
            FieldKind::I32,
        ),
        spec(
            "additional_ripup_cost_factor_at_start",
            "additionalRipupCostFactorAtStart",
            "additional_ripup_cost_factor_at_start",
            FieldKind::I32,
        ),
        spec(
            "trace_ripup_cost_factor",
            "traceRipupCostFactor",
            "trace_ripup_cost_factor",
            FieldKind::F32,
        ),
        spec(
            "max_autoroute_passes",
            "maxAutoroutePasses",
            "max_autoroute_passes",
            FieldKind::I32,
        ),
        spec(
            "board_update_strategy",
            "boardUpdateStrategy",
            "board_update_strategy",
            FieldKind::Enum(BOARD_UPDATE_STRATEGY_NAMES),
        ),
        spec(
            "hybrid_ratio",
            "hybridRatio",
            "hybrid_ratio",
            FieldKind::Str,
        ),
        spec(
            "item_selection_strategy",
            "itemSelectionStrategy",
            "item_selection_strategy",
            FieldKind::Enum(ITEM_SELECTION_STRATEGY_NAMES),
        ),
        spec("timeout", "timeoutString", "timeout_string", FieldKind::Str),
    ];
}

impl FanoutSettings {
    /// `FanoutSettings.getDeclaredFields()` (`FanoutSettings.java:22-106`).
    pub const FIELDS: &'static [FieldSpec] = &[
        spec("enabled", "enabled", "enabled", FieldKind::Bool),
        spec("max_passes", "maxPasses", "max_passes", FieldKind::I32),
        spec("max_items", "maxItems", "max_items", FieldKind::I32),
        spec(
            "max_milliseconds_per_pin",
            "maxMillisecondsPerPin",
            "max_milliseconds_per_pin",
            FieldKind::I64,
        ),
        spec_alt(
            "ripup_allowed",
            &["ripupAllowed"],
            "ripupAllowed",
            "ripup_allowed",
            FieldKind::Bool,
        ),
        spec(
            "min_escape_length_mm",
            "minEscapeLengthMm",
            "min_escape_length_mm",
            FieldKind::F64,
        ),
        spec(
            "max_escape_length_mm",
            "maxEscapeLengthMm",
            "max_escape_length_mm",
            FieldKind::F64,
        ),
        spec(
            "start_via_diameter_mm",
            "startViaDiameterMm",
            "start_via_diameter_mm",
            FieldKind::F64,
        ),
        spec(
            "end_via_diameter_mm",
            "endViaDiameterMm",
            "end_via_diameter_mm",
            FieldKind::F64,
        ),
        spec(
            "pin_sorting_order",
            "pinSortingOrder",
            "pin_sorting_order",
            FieldKind::Str,
        ),
        spec(
            "fallback_to_board_vias",
            "fallbackToBoardVias",
            "fallback_to_board_vias",
            FieldKind::Bool,
        ),
        spec("timeout", "timeoutString", "timeout_string", FieldKind::Str),
    ];
}

// -------------------------------------------------------------------------------------------
// Java string primitives
// -------------------------------------------------------------------------------------------

/// `String.trim()`: strips code units `<= ' '` from both ends — *not* Rust's `str::trim`, which
/// also strips Unicode whitespace such as `U+00A0` (verified: `Double.parseDouble(" 7")`
/// throws, while `Double.parseDouble("\t7\n")` gives `7.0`).
pub(crate) fn java_trim(value: &str) -> &str {
    value.trim_matches(|c: char| c <= '\u{20}')
}

/// `String.split(regex)` with the default limit: pieces in order, **trailing empty pieces
/// dropped**; and when the separator does not occur at all, the whole string as a single piece
/// (so `"".split(",")` is `[""]` while `",".split(",")` is `[]`).
pub(crate) fn java_split(value: &str, is_separator: impl Fn(char) -> bool + Copy) -> Vec<&str> {
    if !value.contains(is_separator) {
        return vec![value];
    }
    let mut parts: Vec<&str> = value.split(is_separator).collect();
    while parts.last() == Some(&"") {
        parts.pop();
    }
    parts
}

/// `ReflectionUtil.snakeToLowerCamel` (`:117-130`): unchanged when there is no `_`, otherwise
/// part 0 lower-cased and every later non-empty part title-cased (`:124` skips empty parts, so
/// `a__b` and `a_b` agree).
///
/// totalized: Java indexes `parts[0]` after a split that drops trailing empties, so a name of
/// nothing but underscores (`"_"`) throws `ArrayIndexOutOfBoundsException` (verified, JVM probe
/// H9). Here it is the empty string, which matches no field and so surfaces as
/// [`MergeError::NoSuchField`] — an error either way, and no reachable caller can tell them
/// apart.
fn snake_to_lower_camel(name: &str) -> String {
    if !name.contains('_') {
        return name.to_string();
    }
    let parts = java_split(name, |c| c == '_');
    let Some((first, rest)) = parts.split_first() else {
        return String::new();
    };
    let mut out = first.to_lowercase();
    for part in rest {
        let mut chars = part.chars();
        let Some(initial) = chars.next() else {
            continue;
        };
        out.extend(initial.to_uppercase());
        out.push_str(&chars.as_str().to_lowercase());
    }
    out
}

/// `String.equalsIgnoreCase`. Every candidate is an ASCII Java identifier or `@SerializedName`
/// value, so ASCII case folding decides every comparison that can succeed.
fn equals_ignore_case(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

// -------------------------------------------------------------------------------------------
// getFieldByNameOrSerializedName
// -------------------------------------------------------------------------------------------

/// One `@SerializedName` value or alternate against the four spellings Java tries
/// (`ReflectionUtil.java:90-93` / `:97-100`).
fn candidate_matches(candidate: &str, name: &str, camel_name: &str) -> bool {
    if equals_ignore_case(candidate, name) || equals_ignore_case(candidate, camel_name) {
        return true;
    }
    let camel_candidate = snake_to_lower_camel(candidate);
    equals_ignore_case(&camel_candidate, name) || equals_ignore_case(&camel_candidate, camel_name)
}

/// `ReflectionUtil.getFieldByNameOrSerializedName` (`:84-115`), minus the superclass fallback
/// (see the module doc comment). Fields are tried in declaration order and the first match wins,
/// so a later field's Java name never beats an earlier field's `@SerializedName`.
fn resolve_field(fields: &'static [FieldSpec], name: &str) -> Option<&'static FieldSpec> {
    let camel_name = snake_to_lower_camel(name);
    for field in fields {
        // Java's `if (annotation != null)` guard (:89): a field with no @SerializedName skips
        // this whole block, which the empty-string sentinel stands in for.
        if !field.serialized.is_empty() {
            if candidate_matches(field.serialized, name, &camel_name) {
                return Some(field);
            }
            if field
                .alternates
                .iter()
                .any(|alt| candidate_matches(alt, name, &camel_name))
            {
                return Some(field);
            }
        }
        // :105-109. Java's third comparison (:107) checks `snakeToLowerCamel(fieldName)` against
        // `camelName` only, not against `name` — transcribed as written; it makes no difference
        // because a Java field name never contains `_`, so `snakeToLowerCamel` returns it
        // unchanged and :106 already covers that comparison.
        if equals_ignore_case(field.java_name, name)
            || equals_ignore_case(field.java_name, &camel_name)
            || equals_ignore_case(&snake_to_lower_camel(field.java_name), &camel_name)
        {
            return Some(field);
        }
    }
    None
}

// -------------------------------------------------------------------------------------------
// convertValue
// -------------------------------------------------------------------------------------------

fn number_format(path: &str, value: &str) -> MergeError {
    MergeError::NumberFormat {
        path: path.to_string(),
        value: value.to_string(),
    }
}

fn no_such_field(path: &str) -> MergeError {
    MergeError::NoSuchField {
        path: path.to_string(),
    }
}

fn type_mismatch(path: &str, value: &str) -> MergeError {
    MergeError::TypeMismatch {
        path: path.to_string(),
        value: value.to_string(),
    }
}

/// `Integer.parseInt(value)` (`ReflectionUtil.java:136-138`): an optional `+`/`-` and ASCII
/// digits, with no whitespace tolerance, no digit separators and no silent overflow — exactly
/// Rust's `i32::from_str`. (JVM-verified: `" 7 "`, `"7_0"` and `"99999999999"` all throw, `"+7"`
/// and `"0007"` give `7`.)
pub fn java_parse_i32(value: &str, path: &str) -> Result<i32, MergeError> {
    value.parse::<i32>().map_err(|_| number_format(path, value))
}

/// `Long.parseLong(value)` (`ReflectionUtil.java:139-141`) — see [`java_parse_i32`].
pub fn java_parse_i64(value: &str, path: &str) -> Result<i64, MergeError> {
    value.parse::<i64>().map_err(|_| number_format(path, value))
}

/// What `Double.parseDouble` recognises after `String.trim()`.
enum FloatLexeme<'a> {
    /// Exactly `NaN` after an optional sign.
    Nan,
    /// Exactly `Infinity` after an optional sign; the flag is that sign.
    Infinity(bool),
    /// A decimal literal with any `d`/`D`/`f`/`F` suffix already removed — a slice Rust's own
    /// `from_str` parses identically (both are correctly rounded).
    Decimal(&'a str),
}

/// `Double.valueOf`'s grammar minus the hexadecimal form (see the module doc comment): optional
/// sign, then `NaN`/`Infinity` spelled exactly, or digits with an optional `.`, an optional
/// `[eE][+-]?digits` exponent and an optional `[fFdD]` suffix — nothing else, nothing after.
fn java_float_lexeme(value: &str) -> Option<FloatLexeme<'_>> {
    let bytes = value.as_bytes();
    let mut i = 0;
    let mut negative = false;
    match bytes.first() {
        Some(b'-') => {
            negative = true;
            i = 1;
        }
        Some(b'+') => i = 1,
        Some(_) => {}
        None => return None,
    }
    match &value[i..] {
        "NaN" => return Some(FloatLexeme::Nan),
        "Infinity" => return Some(FloatLexeme::Infinity(negative)),
        _ => {}
    }

    let mut j = i;
    let mut digits = 0usize;
    while j < bytes.len() && bytes[j].is_ascii_digit() {
        j += 1;
        digits += 1;
    }
    if j < bytes.len() && bytes[j] == b'.' {
        j += 1;
        while j < bytes.len() && bytes[j].is_ascii_digit() {
            j += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return None;
    }
    if j < bytes.len() && (bytes[j] == b'e' || bytes[j] == b'E') {
        let mut k = j + 1;
        if k < bytes.len() && (bytes[k] == b'+' || bytes[k] == b'-') {
            k += 1;
        }
        let exponent_start = k;
        while k < bytes.len() && bytes[k].is_ascii_digit() {
            k += 1;
        }
        if k == exponent_start {
            return None; // `1e` — Java rejects a bare exponent marker.
        }
        j = k;
    }
    let end = j;
    if j < bytes.len() && matches!(bytes[j], b'f' | b'F' | b'd' | b'D') {
        j += 1;
    }
    if j != bytes.len() {
        return None;
    }
    Some(FloatLexeme::Decimal(&value[..end]))
}

/// `Double.parseDouble(value)` (`ReflectionUtil.java:142-144`). Unlike `Integer.parseInt`, this
/// one trims (`FloatingDecimal.readJavaFormatString` calls `String.trim()` first), accepts a
/// `d`/`f` type suffix, and spells the two special values `Infinity` and `NaN` **case
/// sensitively** — `"inf"`, which Rust's `f64::from_str` would accept, is a
/// `NumberFormatException`.
pub fn java_parse_f64(value: &str, path: &str) -> Result<f64, MergeError> {
    match java_float_lexeme(java_trim(value)) {
        Some(FloatLexeme::Nan) => Ok(f64::NAN),
        Some(FloatLexeme::Infinity(negative)) => Ok(if negative {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        }),
        Some(FloatLexeme::Decimal(decimal)) => decimal
            .parse::<f64>()
            .map_err(|_| number_format(path, value)),
        None => Err(number_format(path, value)),
    }
}

/// `Float.parseFloat(value)` (`ReflectionUtil.java:155-157`) — the same grammar as
/// [`java_parse_f64`], rounded to `f32`, so an out-of-range magnitude becomes an infinity rather
/// than an error (`"1e40"` → `Infinity`, JVM-verified).
pub fn java_parse_f32(value: &str, path: &str) -> Result<f32, MergeError> {
    match java_float_lexeme(java_trim(value)) {
        Some(FloatLexeme::Nan) => Ok(f32::NAN),
        Some(FloatLexeme::Infinity(negative)) => Ok(if negative {
            f32::NEG_INFINITY
        } else {
            f32::INFINITY
        }),
        Some(FloatLexeme::Decimal(decimal)) => decimal
            .parse::<f32>()
            .map_err(|_| number_format(path, value)),
        None => Err(number_format(path, value)),
    }
}

/// `convertValue`'s boolean arm (`ReflectionUtil.java:145-154`): `"0"` → `false`, `"1"` → `true`,
/// otherwise `Boolean.parseBoolean`.
///
/// Java bug: convertValue (ReflectionUtil.java:145-154) reports no error for a value that is
/// neither a boolean nor `0`/`1` — `Boolean.parseBoolean` is `"true".equalsIgnoreCase(s)`, so
/// `--router.enabled=yes` silently sets `false`, and `" true "` (with spaces, which
/// `parseBoolean` does not trim) does too. JVM-verified; see `docs/java-quirks.md` row 120.
#[must_use]
pub fn java_parse_bool(value: &str) -> bool {
    if value == "0" {
        return false;
    }
    if value == "1" {
        return true;
    }
    value.eq_ignore_ascii_case("true")
}

/// `convertValue`'s enum arm (`ReflectionUtil.java:158-164`): the constant whose name equals
/// `value.trim()` ignoring case, or `None` — which in Java means falling through the rest of the
/// chain to a `field.set` of a raw `String` into an enum field, i.e. `IllegalArgumentException`.
#[must_use]
pub fn java_enum_constant(constants: &[&'static str], value: &str) -> Option<&'static str> {
    let trimmed = java_trim(value);
    constants
        .iter()
        .copied()
        .find(|constant| equals_ignore_case(constant, trimmed))
}

fn convert_enum<T: JavaEnum>(kind: FieldKind, value: &str, path: &str) -> Result<T, MergeError> {
    let FieldKind::Enum(constants) = kind else {
        return Err(type_mismatch(path, value));
    };
    java_enum_constant(constants, value)
        .and_then(T::from_java_name)
        .ok_or_else(|| MergeError::EnumName {
            path: path.to_string(),
            value: value.to_string(),
        })
}

/// `convertValue`'s `String[]` arm (`ReflectionUtil.java:165-178`): trim the whole value, an
/// empty result gives a zero-length array, otherwise split on `,` and trim every token. Java's
/// `split` drops trailing empty tokens but keeps interior ones, so `" a , b ,"` is `["a", "b"]`
/// while `"a,,b"` is `["a", "", "b"]`.
#[must_use]
pub fn java_parse_string_vec(value: &str) -> Vec<String> {
    let raw = java_trim(value);
    if raw.is_empty() {
        return Vec::new();
    }
    java_split(raw, |c| c == ',')
        .into_iter()
        .map(|token| java_trim(token).to_string())
        .collect()
}

/// `convertValue`'s `double[]` arm (`ReflectionUtil.java:179-190`) — [`java_parse_string_vec`]'s
/// tokens through [`java_parse_f64`]; one bad token fails the whole assignment.
pub fn java_parse_f64_vec(value: &str, path: &str) -> Result<Vec<f64>, MergeError> {
    let raw = java_trim(value);
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    java_split(raw, |c| c == ',')
        .into_iter()
        .map(|token| java_parse_f64(java_trim(token), path))
        .collect()
}

/// `convertValue`'s `int[]` arm (`ReflectionUtil.java:191-202`) — see [`java_parse_f64_vec`].
pub fn java_parse_i32_vec(value: &str, path: &str) -> Result<Vec<i32>, MergeError> {
    let raw = java_trim(value);
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    java_split(raw, |c| c == ',')
        .into_iter()
        .map(|token| java_parse_i32(java_trim(token), path))
        .collect()
}

// -------------------------------------------------------------------------------------------
// setFieldValue / setPropertyRecursive
// -------------------------------------------------------------------------------------------

/// `ReflectionUtil.setFieldValue(obj, propertyName, newValue)` (`ReflectionUtil.java:21-25`):
/// assigns `value` to the field of `target` addressed by `property_path`.
///
/// Java bug: setFieldValue (ReflectionUtil.java:23) splits the path on `[.:\-]`, so `-` is a path
/// separator interchangeable with `.` and `:` — a `--router.trace-cost=…` argument silently
/// becomes the two-segment path `router` / `trace` / `cost` rather than one field name with a
/// hyphen in it. JVM-verified (`optimizer-max_passes` sets `optimizer.maxPasses`); see
/// `docs/java-quirks.md` row 118. Only the *path* is split: the value is passed through
/// untouched, so a value such as `1:1` or `freerouting-router` survives verbatim.
///
/// # Errors
///
/// Returns [`MergeError::NoSuchField`] when a path segment names no field,
/// [`MergeError::NumberFormat`] / [`MergeError::EnumName`] where Java throws
/// `NumberFormatException` / `IllegalArgumentException` out of `convertValue`, and
/// [`MergeError::TypeMismatch`] where Java throws from `field.set` or from instantiating a
/// non-instantiable intermediate type. Task 7's sources log-and-continue on each, exactly as
/// `EnvironmentVariablesSource.java:81-89` and `CliSettings.java:97-99` do.
pub fn set_field_value(
    target: &mut RouterSettings,
    property_path: &str,
    value: &str,
) -> Result<(), MergeError> {
    let segments = java_split(property_path, |c| matches!(c, '.' | ':' | '-'));
    if segments.is_empty() {
        // totalized: Java indexes `propertyPath[0]` unconditionally (:34), so a path of nothing
        // but separators throws ArrayIndexOutOfBoundsException (JVM probe H8). An error either
        // way; no caller distinguishes them.
        return Err(no_such_field(property_path));
    }
    set_router_property(target, &segments, 0, value, property_path)
}

/// `setPropertyRecursive` (`ReflectionUtil.java:27-82`) at a [`RouterSettings`].
fn set_router_property(
    target: &mut RouterSettings,
    segments: &[&str],
    index: usize,
    value: &str,
    path: &str,
) -> Result<(), MergeError> {
    let field = resolve_field(RouterSettings::FIELDS, segments[index])
        .ok_or_else(|| no_such_field(path))?;

    if index + 1 == segments.len() {
        return set_router_leaf(target, field, value, path);
    }

    match field.rust_name {
        // :46-72 — the one array field, and the only place a value is comma-split during
        // navigation rather than at the leaf.
        "layers" => {
            let tokens = java_split(value, |c| c == ',');
            // Java bug: setPropertyRecursive (ReflectionUtil.java:56-58) sizes a freshly
            // allocated array by the *token count* of the value, not by the board's layer count,
            // so `--router.layers.routable=true,true,true` on a 6-layer board leaves a 3-element
            // array behind. JVM-verified; see `docs/java-quirks.md` row 119.
            let layers = target
                .layers
                .get_or_insert_with(|| vec![LayerSettings::default(); tokens.len()]);
            // Java bug: setPropertyRecursive (ReflectionUtil.java:62) writes only
            // `min(arrayLength, tokenCount)` elements, so surplus tokens are dropped and surplus
            // elements left untouched, both without a word to the caller. Quirks row 119.
            let limit = layers.len().min(tokens.len());
            for (element, token) in layers.iter_mut().zip(tokens).take(limit) {
                set_layer_property(element, segments, index + 1, java_trim(token), path)?;
            }
            Ok(())
        }
        // :73-81 — normal object navigation, instantiating an absent nested object first.
        "fanout" => set_fanout_property(
            target.fanout.get_or_insert_with(FanoutSettings::default),
            segments,
            index + 1,
            value,
            path,
        ),
        "optimizer" => set_optimizer_property(
            target
                .optimizer
                .get_or_insert_with(OptimizerSettings::default),
            segments,
            index + 1,
            value,
            path,
        ),
        "scoring" => set_scoring_property(
            target.scoring.get_or_insert_with(ScoringSettings::default),
            segments,
            index + 1,
            value,
            path,
        ),
        // Everything else. Java fails here too, but by two different routes depending on the
        // field's type, and one of them writes before it fails:
        //
        // - A *scalar* field (`Boolean`, `Integer`, `Double`, `String`) takes the `else` branch
        //   at `:73-81`, finds the field null, and asks for its no-arg constructor — which none
        //   of those four has, so `getDeclaredConstructor()` throws `NoSuchMethodException`
        //   without touching the object (JVM probe H7: `max_passes.foo` leaves `maxPasses` null).
        // - A *primitive- or String-array* field — `ignore_net_classes` (`String[]`) is the only
        //   one reachable from here, and `scoring.preferred_direction_trace_cost` /
        //   `undesired_direction_trace_cost` (`double[]`) are the same case one level down — is
        //   an array as far as `:46` is concerned, so Java takes the **array** branch: it splits
        //   the value on `,`, allocates the array, instantiates each `null` element (`new
        //   String()`, i.e. `""`), and only then recurses and fails to resolve the next segment.
        //
        // totalized: the array branch's partial write is not reproduced. JVM-verified (RProbe
        // A1/A2, `crates/fr-settings/tests/data/RProbe.java`): `ignore_net_classes.foo=a,b`
        // throws `NoSuchFieldException: foo` having already left `ignoreNetClasses = ["", null]`,
        // and `scoring.preferred_direction_trace_cost.foo=1,2` leaves `[0.0, 0.0]`. The port
        // answers `MergeError::TypeMismatch` and writes nothing. Both callers swallow the failure
        // identically (`EnvironmentVariablesSource.java:81-89`, `CliSettings.java:97-99`), so the
        // only observable difference is Java's half-built array left on the settings object — a
        // strictly worse state that no caller asked for and none reads back deliberately.
        _ => Err(type_mismatch(path, value)),
    }
}

fn set_router_leaf(
    target: &mut RouterSettings,
    field: &FieldSpec,
    value: &str,
    path: &str,
) -> Result<(), MergeError> {
    match field.rust_name {
        "enabled" => target.enabled = Some(java_parse_bool(value)),
        "algorithm" => target.algorithm = Some(value.to_string()),
        "copper_to_edge_clearance_um" => {
            target.copper_to_edge_clearance_um = Some(java_parse_f64(value, path)?);
        }
        "hole_clearance_um" => target.hole_clearance_um = Some(java_parse_f64(value, path)?),
        "neck_width_um" => target.neck_width_um = Some(java_parse_f64(value, path)?),
        "strict_drc" => target.strict_drc = Some(java_parse_bool(value)),
        "job_timeout_string" => target.job_timeout_string = Some(value.to_string()),
        "max_passes" => target.max_passes = Some(java_parse_i32(value, path)?),
        "max_items" => target.max_items = Some(java_parse_i32(value, path)?),
        "save_intermediate_stages" => {
            target.save_intermediate_stages = Some(java_parse_bool(value))
        }
        "ignore_net_classes" => target.ignore_net_classes = Some(java_parse_string_vec(value)),
        "trace_pull_tight_accuracy" => {
            target.trace_pull_tight_accuracy = Some(java_parse_i32(value, path)?);
        }
        "vias_allowed" => target.vias_allowed = Some(java_parse_bool(value)),
        "automatic_neckdown" => target.automatic_neckdown = Some(java_parse_bool(value)),
        "max_threads" => target.max_threads = Some(java_parse_i32(value, path)?),
        "result_json_path" => target.result_json_path = Some(value.to_string()),
        "board_specific_trace_costs_applied" => {
            target.board_specific_trace_costs_applied = Some(java_parse_bool(value));
        }
        // `fanout`, `layers`, `optimizer`, `scoring`: `convertValue` matches none of its arms for
        // a struct or object-array target type and returns the raw `String` (:203-204), which
        // `field.set` (:41) then rejects with `IllegalArgumentException` (JVM probes H5/H6).
        _ => return Err(type_mismatch(path, value)),
    }
    Ok(())
}

/// `setPropertyRecursive` at a [`LayerSettings`] — always a leaf, since the struct has no nested
/// fields.
fn set_layer_property(
    target: &mut LayerSettings,
    segments: &[&str],
    index: usize,
    value: &str,
    path: &str,
) -> Result<(), MergeError> {
    let field =
        resolve_field(LayerSettings::FIELDS, segments[index]).ok_or_else(|| no_such_field(path))?;
    if index + 1 != segments.len() {
        return Err(type_mismatch(path, value));
    }
    match field.rust_name {
        "routable" => target.routable = Some(java_parse_bool(value)),
        "preferred_direction_horizontal" => {
            target.preferred_direction_horizontal = Some(java_parse_bool(value));
        }
        "bend_cost" => target.bend_cost = Some(java_parse_f64(value, path)?),
        _ => return Err(type_mismatch(path, value)),
    }
    Ok(())
}

/// `setPropertyRecursive` at a [`ScoringSettings`].
fn set_scoring_property(
    target: &mut ScoringSettings,
    segments: &[&str],
    index: usize,
    value: &str,
    path: &str,
) -> Result<(), MergeError> {
    let field = resolve_field(ScoringSettings::FIELDS, segments[index])
        .ok_or_else(|| no_such_field(path))?;
    if index + 1 != segments.len() {
        return Err(type_mismatch(path, value));
    }
    match field.rust_name {
        "preferred_direction_trace_cost" => {
            target.preferred_direction_trace_cost = Some(java_parse_f64_vec(value, path)?);
        }
        "undesired_direction_trace_cost" => {
            target.undesired_direction_trace_cost = Some(java_parse_f64_vec(value, path)?);
        }
        "default_preferred_direction_trace_cost" => {
            target.default_preferred_direction_trace_cost = Some(java_parse_f64(value, path)?);
        }
        "default_undesired_direction_trace_cost" => {
            target.default_undesired_direction_trace_cost = Some(java_parse_f64(value, path)?);
        }
        "via_costs" => target.via_costs = Some(java_parse_i32(value, path)?),
        "plane_via_costs" => target.plane_via_costs = Some(java_parse_i32(value, path)?),
        "start_ripup_costs" => target.start_ripup_costs = Some(java_parse_i32(value, path)?),
        "unrouted_net_penalty" => target.unrouted_net_penalty = Some(java_parse_f32(value, path)?),
        "clearance_violation_penalty" => {
            target.clearance_violation_penalty = Some(java_parse_f32(value, path)?);
        }
        "bend_penalty" => target.bend_penalty = Some(java_parse_f32(value, path)?),
        "default_bend_cost" => target.default_bend_cost = Some(java_parse_f64(value, path)?),
        _ => return Err(type_mismatch(path, value)),
    }
    Ok(())
}

/// `setPropertyRecursive` at an [`OptimizerSettings`].
fn set_optimizer_property(
    target: &mut OptimizerSettings,
    segments: &[&str],
    index: usize,
    value: &str,
    path: &str,
) -> Result<(), MergeError> {
    let field = resolve_field(OptimizerSettings::FIELDS, segments[index])
        .ok_or_else(|| no_such_field(path))?;
    if index + 1 != segments.len() {
        return Err(type_mismatch(path, value));
    }
    match field.rust_name {
        "enabled" => target.enabled = Some(java_parse_bool(value)),
        "algorithm" => target.algorithm = Some(value.to_string()),
        "max_passes" => target.max_passes = Some(java_parse_i32(value, path)?),
        "max_items" => target.max_items = Some(java_parse_i32(value, path)?),
        "max_threads" => target.max_threads = Some(java_parse_i32(value, path)?),
        "optimization_improvement_threshold" => {
            target.optimization_improvement_threshold = Some(java_parse_f32(value, path)?);
        }
        "max_consecutive_failures" => {
            target.max_consecutive_failures = Some(java_parse_i32(value, path)?);
        }
        "additional_ripup_cost_factor_at_start" => {
            target.additional_ripup_cost_factor_at_start = Some(java_parse_i32(value, path)?);
        }
        "trace_ripup_cost_factor" => {
            target.trace_ripup_cost_factor = Some(java_parse_f32(value, path)?);
        }
        "max_autoroute_passes" => target.max_autoroute_passes = Some(java_parse_i32(value, path)?),
        "board_update_strategy" => {
            target.board_update_strategy = Some(convert_enum::<BoardUpdateStrategy>(
                field.kind, value, path,
            )?);
        }
        "hybrid_ratio" => target.hybrid_ratio = Some(value.to_string()),
        "item_selection_strategy" => {
            target.item_selection_strategy = Some(convert_enum::<ItemSelectionStrategy>(
                field.kind, value, path,
            )?);
        }
        "timeout_string" => target.timeout_string = Some(value.to_string()),
        _ => return Err(type_mismatch(path, value)),
    }
    Ok(())
}

/// `setPropertyRecursive` at a [`FanoutSettings`].
fn set_fanout_property(
    target: &mut FanoutSettings,
    segments: &[&str],
    index: usize,
    value: &str,
    path: &str,
) -> Result<(), MergeError> {
    let field = resolve_field(FanoutSettings::FIELDS, segments[index])
        .ok_or_else(|| no_such_field(path))?;
    if index + 1 != segments.len() {
        return Err(type_mismatch(path, value));
    }
    match field.rust_name {
        "enabled" => target.enabled = Some(java_parse_bool(value)),
        "max_passes" => target.max_passes = Some(java_parse_i32(value, path)?),
        "max_items" => target.max_items = Some(java_parse_i32(value, path)?),
        "max_milliseconds_per_pin" => {
            target.max_milliseconds_per_pin = Some(java_parse_i64(value, path)?);
        }
        "ripup_allowed" => target.ripup_allowed = Some(java_parse_bool(value)),
        "min_escape_length_mm" => target.min_escape_length_mm = Some(java_parse_f64(value, path)?),
        "max_escape_length_mm" => target.max_escape_length_mm = Some(java_parse_f64(value, path)?),
        "start_via_diameter_mm" => {
            target.start_via_diameter_mm = Some(java_parse_f64(value, path)?)
        }
        "end_via_diameter_mm" => target.end_via_diameter_mm = Some(java_parse_f64(value, path)?),
        "pin_sorting_order" => target.pin_sorting_order = Some(value.to_string()),
        "fallback_to_board_vias" => target.fallback_to_board_vias = Some(java_parse_bool(value)),
        "timeout_string" => target.timeout_string = Some(value.to_string()),
        _ => return Err(type_mismatch(path, value)),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `snakeToLowerCamel` (`ReflectionUtil.java:117-130`), including the empty-part skip at
    /// `:124` and the totalization of Java's `parts[0]` index-out-of-bounds.
    #[test]
    fn snake_to_lower_camel_matches_java() {
        assert_eq!(snake_to_lower_camel("maxPasses"), "maxPasses");
        assert_eq!(snake_to_lower_camel("MAXPASSES"), "MAXPASSES");
        assert_eq!(snake_to_lower_camel("max_passes"), "maxPasses");
        assert_eq!(snake_to_lower_camel("MAX_PASSES"), "maxPasses");
        assert_eq!(snake_to_lower_camel("max__passes"), "maxPasses");
        assert_eq!(
            snake_to_lower_camel("preferred_DIRECTION_horizontal"),
            "preferredDirectionHorizontal"
        );
        assert_eq!(snake_to_lower_camel("_max_passes"), "MaxPasses");
        assert_eq!(snake_to_lower_camel("max_passes_"), "maxPasses");
        assert_eq!(snake_to_lower_camel("_"), "");
    }

    /// `String.split` with the default limit: trailing empties dropped, interior ones kept, and
    /// a separator-free string returned whole (so `""` is one empty piece, `","` is none).
    #[test]
    fn java_split_matches_java() {
        let comma = |c: char| c == ',';
        assert_eq!(java_split("a,b", comma), ["a", "b"]);
        assert_eq!(java_split("a,,b", comma), ["a", "", "b"]);
        assert_eq!(java_split("a,b,", comma), ["a", "b"]);
        assert_eq!(java_split("a,b,,", comma), ["a", "b"]);
        assert_eq!(java_split(",a", comma), ["", "a"]);
        assert_eq!(java_split("abc", comma), ["abc"]);
        assert_eq!(java_split("", comma), [""]);
        assert!(java_split(",", comma).is_empty());
        assert!(java_split(",,", comma).is_empty());
    }

    /// `String.trim()` strips code units `<= ' '` only — not `U+00A0`, which Rust's `str::trim`
    /// would strip (JVM-verified: `parseDouble(" 7")` throws).
    #[test]
    fn java_trim_is_not_rust_trim() {
        assert_eq!(java_trim("\t 7 \n"), "7");
        assert_eq!(java_trim("\u{a0}7"), "\u{a0}7");
        assert!(java_parse_f64("\u{a0}7", "x").is_err());
    }

    /// Unlike `copyFields`, which skips non-`public` fields (`ReflectionUtil.java:226-228`),
    /// `getFieldByNameOrSerializedName` does not filter by modifier and `setAccessible(true)`
    /// (`:36`) opens the private ones — so this `private transient` field *is* reachable from a
    /// property path (JVM probe H4). It is `pub(crate)` in Rust for the same reason it is
    /// `private` in Java, so only an in-crate test can observe the write.
    #[test]
    fn private_fields_are_settable() {
        let mut settings = RouterSettings::new();
        assert_eq!(settings.board_specific_trace_costs_applied, None);
        set_field_value(&mut settings, "board_specific_trace_costs_applied", "true")
            .expect("private is not a filter here");
        assert_eq!(settings.board_specific_trace_costs_applied, Some(true));

        set_field_value(&mut settings, "boardSpecificTraceCostsApplied", "false")
            .expect("java name resolves too");
        assert_eq!(settings.board_specific_trace_costs_applied, Some(false));
    }

    /// The empty `serialized` sentinel stands in for Java's `if (annotation != null)` guard: it
    /// must never match, least of all an empty path segment.
    #[test]
    fn the_no_annotation_sentinel_never_matches() {
        assert!(resolve_field(RouterSettings::FIELDS, "").is_none());
        let mut settings = RouterSettings::new();
        assert!(matches!(
            set_field_value(&mut settings, "optimizer..max_passes", "1"),
            Err(MergeError::NoSuchField { .. })
        ));
    }
}
