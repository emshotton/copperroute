//! `EnvironmentVariablesSource` (`FREEROUTING__ROUTER__OPTIMIZER__MAX_THREADS=8`) and
use crate::{
    BoardUpdateStrategy, FanoutSettings, ItemSelectionStrategy, LayerSettings, MergeError,
    NamedEnum, OptimizerSettings, RouterSettings, ScoringSettings,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    Bool,
    I32,
    I64,
    F32,
    F64,
    Str,
    Enum(&'static [&'static str]),
    StringVec,
    F64Vec,
    I32Vec,
    Nested,
    ObjectArray,
}

#[derive(Debug, Clone, Copy)]
pub struct FieldSpec {
    pub serialized: &'static str,
    pub alternates: &'static [&'static str],
    pub legacy_name: &'static str,
    pub rust_name: &'static str,
    pub kind: FieldKind,
}

const BOARD_UPDATE_STRATEGY_NAMES: &[&str] = &["GREEDY", "GLOBAL_OPTIMAL", "HYBRID"];
const ITEM_SELECTION_STRATEGY_NAMES: &[&str] = &["SEQUENTIAL", "RANDOM", "PRIORITIZED"];

const fn spec(
    serialized: &'static str,
    legacy_name: &'static str,
    rust_name: &'static str,
    kind: FieldKind,
) -> FieldSpec {
    FieldSpec {
        serialized,
        alternates: &[],
        legacy_name,
        rust_name,
        kind,
    }
}

const fn spec_alt(
    serialized: &'static str,
    alternates: &'static [&'static str],
    legacy_name: &'static str,
    rust_name: &'static str,
    kind: FieldKind,
) -> FieldSpec {
    FieldSpec {
        serialized,
        alternates,
        legacy_name,
        rust_name,
        kind,
    }
}

impl RouterSettings {
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
        spec(
            "",
            "boardSpecificTraceCostsApplied",
            "board_specific_trace_costs_applied",
            FieldKind::Bool,
        ),
        spec(
            "opt_changed_area_ms",
            "opt_changed_area_ms",
            "opt_changed_area_ms",
            FieldKind::I32,
        ),
        spec(
            "smd_via_relaxation",
            "smd_via_relaxation",
            "smd_via_relaxation",
            FieldKind::Bool,
        ),
        spec(
            "failure_give_up_threshold",
            "failure_give_up_threshold",
            "failure_give_up_threshold",
            FieldKind::I32,
        ),
        spec(
            "connection_search_steps",
            "connection_search_steps",
            "connection_search_steps",
            FieldKind::I64,
        ),
    ];
}

impl LayerSettings {
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
        spec(
            "smd_via_cost_factor",
            "smdViaCostFactor",
            "smd_via_cost_factor",
            FieldKind::F64,
        ),
    ];
}

impl OptimizerSettings {
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
            "max_search_steps",
            "maxSearchSteps",
            "max_search_steps",
            FieldKind::I64,
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

pub(crate) fn split_dropping_trailing_empty(
    value: &str,
    is_separator: impl Fn(char) -> bool + Copy,
) -> Vec<&str> {
    if !value.contains(is_separator) {
        return vec![value];
    }
    let mut parts: Vec<&str> = value.split(is_separator).collect();
    while parts.last() == Some(&"") {
        parts.pop();
    }
    parts
}

fn snake_to_lower_camel(name: &str) -> String {
    if !name.contains('_') {
        return name.to_string();
    }
    let parts = split_dropping_trailing_empty(name, |c| c == '_');
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

fn equals_ignore_case(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

fn candidate_matches(candidate: &str, name: &str, camel_name: &str) -> bool {
    if equals_ignore_case(candidate, name) || equals_ignore_case(candidate, camel_name) {
        return true;
    }
    let camel_candidate = snake_to_lower_camel(candidate);
    equals_ignore_case(&camel_candidate, name) || equals_ignore_case(&camel_candidate, camel_name)
}

fn resolve_field(fields: &'static [FieldSpec], name: &str) -> Option<&'static FieldSpec> {
    let camel_name = snake_to_lower_camel(name);
    for field in fields {
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
        if equals_ignore_case(field.legacy_name, name)
            || equals_ignore_case(field.legacy_name, &camel_name)
            || equals_ignore_case(&snake_to_lower_camel(field.legacy_name), &camel_name)
        {
            return Some(field);
        }
    }
    None
}

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

pub fn parse_i32(value: &str, path: &str) -> Result<i32, MergeError> {
    value.parse::<i32>().map_err(|_| number_format(path, value))
}

pub fn parse_i64(value: &str, path: &str) -> Result<i64, MergeError> {
    value.parse::<i64>().map_err(|_| number_format(path, value))
}

pub fn parse_f64(value: &str, path: &str) -> Result<f64, MergeError> {
    value
        .trim()
        .parse::<f64>()
        .map_err(|_| number_format(path, value))
}

pub fn parse_f32(value: &str, path: &str) -> Result<f32, MergeError> {
    value
        .trim()
        .parse::<f32>()
        .map_err(|_| number_format(path, value))
}

#[must_use]
pub fn parse_bool(value: &str) -> bool {
    if value == "0" {
        return false;
    }
    if value == "1" {
        return true;
    }
    value.eq_ignore_ascii_case("true")
}

#[must_use]
pub fn enum_constant(constants: &[&'static str], value: &str) -> Option<&'static str> {
    let trimmed = (value).trim();
    constants
        .iter()
        .copied()
        .find(|constant| equals_ignore_case(constant, trimmed))
}

fn convert_enum<T: NamedEnum>(kind: FieldKind, value: &str, path: &str) -> Result<T, MergeError> {
    let FieldKind::Enum(constants) = kind else {
        return Err(type_mismatch(path, value));
    };
    enum_constant(constants, value)
        .and_then(T::from_name)
        .ok_or_else(|| MergeError::EnumName {
            path: path.to_string(),
            value: value.to_string(),
        })
}

#[must_use]
pub fn parse_string_vec(value: &str) -> Vec<String> {
    let raw = (value).trim();
    if raw.is_empty() {
        return Vec::new();
    }
    split_dropping_trailing_empty(raw, |c| c == ',')
        .into_iter()
        .map(|token| (token).trim().to_string())
        .collect()
}

pub fn parse_f64_vec(value: &str, path: &str) -> Result<Vec<f64>, MergeError> {
    let raw = (value).trim();
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    split_dropping_trailing_empty(raw, |c| c == ',')
        .into_iter()
        .map(|token| parse_f64((token).trim(), path))
        .collect()
}

pub fn parse_i32_vec(value: &str, path: &str) -> Result<Vec<i32>, MergeError> {
    let raw = (value).trim();
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    split_dropping_trailing_empty(raw, |c| c == ',')
        .into_iter()
        .map(|token| parse_i32((token).trim(), path))
        .collect()
}

pub fn set_field_value(
    target: &mut RouterSettings,
    property_path: &str,
    value: &str,
) -> Result<(), MergeError> {
    let segments = split_dropping_trailing_empty(property_path, |c| matches!(c, '.' | ':' | '-'));
    if segments.is_empty() {
        return Err(no_such_field(property_path));
    }
    set_router_property(target, &segments, 0, value, property_path)
}

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
        "layers" => {
            let tokens = split_dropping_trailing_empty(value, |c| c == ',');
            let layers = target
                .layers
                .get_or_insert_with(|| vec![LayerSettings::default(); tokens.len()]);
            let limit = layers.len().min(tokens.len());
            for (element, token) in layers.iter_mut().zip(tokens).take(limit) {
                set_layer_property(element, segments, index + 1, (token).trim(), path)?;
            }
            Ok(())
        }
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
        "enabled" => target.enabled = Some(parse_bool(value)),
        "algorithm" => target.algorithm = Some(value.to_string()),
        "copper_to_edge_clearance_um" => {
            target.copper_to_edge_clearance_um = Some(parse_f64(value, path)?);
        }
        "hole_clearance_um" => target.hole_clearance_um = Some(parse_f64(value, path)?),
        "neck_width_um" => target.neck_width_um = Some(parse_f64(value, path)?),
        "strict_drc" => target.strict_drc = Some(parse_bool(value)),
        "job_timeout_string" => target.job_timeout_string = Some(value.to_string()),
        "max_passes" => target.max_passes = Some(parse_i32(value, path)?),
        "max_items" => target.max_items = Some(parse_i32(value, path)?),
        "save_intermediate_stages" => target.save_intermediate_stages = Some(parse_bool(value)),
        "ignore_net_classes" => target.ignore_net_classes = Some(parse_string_vec(value)),
        "trace_pull_tight_accuracy" => {
            target.trace_pull_tight_accuracy = Some(parse_i32(value, path)?);
        }
        "vias_allowed" => target.vias_allowed = Some(parse_bool(value)),
        "automatic_neckdown" => target.automatic_neckdown = Some(parse_bool(value)),
        "max_threads" => target.max_threads = Some(parse_i32(value, path)?),
        "result_json_path" => target.result_json_path = Some(value.to_string()),
        "board_specific_trace_costs_applied" => {
            target.board_specific_trace_costs_applied = Some(parse_bool(value));
        }
        "opt_changed_area_ms" => target.opt_changed_area_ms = Some(parse_i32(value, path)?),
        "smd_via_relaxation" => target.smd_via_relaxation = Some(parse_bool(value)),
        "failure_give_up_threshold" => {
            target.failure_give_up_threshold = Some(parse_i32(value, path)?);
        }
        "connection_search_steps" => {
            target.connection_search_steps = Some(parse_i64(value, path)?);
        }
        _ => return Err(type_mismatch(path, value)),
    }
    Ok(())
}

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
        "routable" => target.routable = Some(parse_bool(value)),
        "preferred_direction_horizontal" => {
            target.preferred_direction_horizontal = Some(parse_bool(value));
        }
        "bend_cost" => target.bend_cost = Some(parse_f64(value, path)?),
        _ => return Err(type_mismatch(path, value)),
    }
    Ok(())
}

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
            target.preferred_direction_trace_cost = Some(parse_f64_vec(value, path)?);
        }
        "undesired_direction_trace_cost" => {
            target.undesired_direction_trace_cost = Some(parse_f64_vec(value, path)?);
        }
        "default_preferred_direction_trace_cost" => {
            target.default_preferred_direction_trace_cost = Some(parse_f64(value, path)?);
        }
        "default_undesired_direction_trace_cost" => {
            target.default_undesired_direction_trace_cost = Some(parse_f64(value, path)?);
        }
        "via_costs" => target.via_costs = Some(parse_i32(value, path)?),
        "plane_via_costs" => target.plane_via_costs = Some(parse_i32(value, path)?),
        "start_ripup_costs" => target.start_ripup_costs = Some(parse_i32(value, path)?),
        "unrouted_net_penalty" => target.unrouted_net_penalty = Some(parse_f32(value, path)?),
        "clearance_violation_penalty" => {
            target.clearance_violation_penalty = Some(parse_f32(value, path)?);
        }
        "bend_penalty" => target.bend_penalty = Some(parse_f32(value, path)?),
        "default_bend_cost" => target.default_bend_cost = Some(parse_f64(value, path)?),
        "smd_via_cost_factor" => {
            target.smd_via_cost_factor = Some(parse_f64(value, path)?);
        }
        _ => return Err(type_mismatch(path, value)),
    }
    Ok(())
}

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
        "enabled" => target.enabled = Some(parse_bool(value)),
        "algorithm" => target.algorithm = Some(value.to_string()),
        "max_passes" => target.max_passes = Some(parse_i32(value, path)?),
        "max_items" => target.max_items = Some(parse_i32(value, path)?),
        "max_threads" => target.max_threads = Some(parse_i32(value, path)?),
        "optimization_improvement_threshold" => {
            target.optimization_improvement_threshold = Some(parse_f32(value, path)?);
        }
        "max_consecutive_failures" => {
            target.max_consecutive_failures = Some(parse_i32(value, path)?);
        }
        "additional_ripup_cost_factor_at_start" => {
            target.additional_ripup_cost_factor_at_start = Some(parse_i32(value, path)?);
        }
        "trace_ripup_cost_factor" => {
            target.trace_ripup_cost_factor = Some(parse_f32(value, path)?);
        }
        "max_autoroute_passes" => target.max_autoroute_passes = Some(parse_i32(value, path)?),
        "max_search_steps" => target.max_search_steps = Some(parse_i64(value, path)?),
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
        "enabled" => target.enabled = Some(parse_bool(value)),
        "max_passes" => target.max_passes = Some(parse_i32(value, path)?),
        "max_items" => target.max_items = Some(parse_i32(value, path)?),
        "max_milliseconds_per_pin" => {
            target.max_milliseconds_per_pin = Some(parse_i64(value, path)?);
        }
        "ripup_allowed" => target.ripup_allowed = Some(parse_bool(value)),
        "min_escape_length_mm" => target.min_escape_length_mm = Some(parse_f64(value, path)?),
        "max_escape_length_mm" => target.max_escape_length_mm = Some(parse_f64(value, path)?),
        "start_via_diameter_mm" => target.start_via_diameter_mm = Some(parse_f64(value, path)?),
        "end_via_diameter_mm" => target.end_via_diameter_mm = Some(parse_f64(value, path)?),
        "pin_sorting_order" => target.pin_sorting_order = Some(value.to_string()),
        "fallback_to_board_vias" => target.fallback_to_board_vias = Some(parse_bool(value)),
        "timeout_string" => target.timeout_string = Some(value.to_string()),
        _ => return Err(type_mismatch(path, value)),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn split_dropping_trailing_empty_matches_java() {
        let comma = |c: char| c == ',';
        assert_eq!(split_dropping_trailing_empty("a,b", comma), ["a", "b"]);
        assert_eq!(split_dropping_trailing_empty("a,,b", comma), ["a", "", "b"]);
        assert_eq!(split_dropping_trailing_empty("a,b,", comma), ["a", "b"]);
        assert_eq!(split_dropping_trailing_empty("a,b,,", comma), ["a", "b"]);
        assert_eq!(split_dropping_trailing_empty(",a", comma), ["", "a"]);
        assert_eq!(split_dropping_trailing_empty("abc", comma), ["abc"]);
        assert_eq!(split_dropping_trailing_empty("", comma), [""]);
        assert!(split_dropping_trailing_empty(",", comma).is_empty());
        assert!(split_dropping_trailing_empty(",,", comma).is_empty());
    }

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
