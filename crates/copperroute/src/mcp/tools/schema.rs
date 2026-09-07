use serde_json::{Value, json};

fn board_input_properties() -> Value {
    json!({
        "dsn_path": {
            "type": "string",
            "description": "Path to a Specctra DSN file, or to a KiCad board JSON file. Exactly one of dsn_path and dsn_text is required."
        },
        "dsn_text": {
            "type": "string",
            "description": "The Specctra DSN document itself, as text. Exactly one of dsn_path and dsn_text is required."
        }
    })
}

fn one_of_dsn_path_or_text() -> Value {
    json!([
        { "required": ["dsn_path"], "not": { "required": ["dsn_text"] } },
        { "required": ["dsn_text"], "not": { "required": ["dsn_path"] } }
    ])
}

#[must_use]
pub fn route_board_schema() -> Value {
    let mut properties = board_input_properties();
    let object = properties.as_object_mut().expect("a JSON object");
    object.insert("ses_path".into(), json!({
        "type": "string",
        "description": "An existing session to import onto the board before routing (a .ses, or a KiCad session .json), so the run continues from a previous result. The CLI spelling is --ses."
    }));
    object.insert("rules_path".into(), json!({
        "type": "string",
        "description": "A Specctra .rules file, applied at priority 40 and then re-read against the loaded board. The CLI spelling is --rules."
    }));
    object.insert("output_path".into(), json!({
        "type": "string",
        "description": "Where to write the session file. When given, the result carries ses_path and no ses_text, which keeps a large SES out of the model's context; when omitted, the result carries ses_text and data (Base64) instead."
    }));
    object.insert("settings".into(), json!({
        "type": "object",
        "description": "A sparse RouterSettings override, applied at priority 70 above every other settings source. Call list_settings for the field names, their descriptions and the defaults in force. Only the fields named here are overridden.",
        "additionalProperties": true
    }));
    json!({
        "type": "object",
        "properties": properties,
        "oneOf": one_of_dsn_path_or_text(),
        "additionalProperties": false
    })
}

#[must_use]
pub fn check_drc_schema() -> Value {
    let mut properties = board_input_properties();
    let object = properties.as_object_mut().expect("a JSON object");
    object.insert("ses_path".into(), json!({
        "type": "string",
        "description": "A session to import onto the board before checking (a .ses, or a KiCad session .json), so the report describes the routed board. The CLI spelling is --ses."
    }));
    object.insert("rules_path".into(), json!({
        "type": "string",
        "description": "A Specctra .rules file, applied to the board before checking. The CLI spelling is --rules."
    }));
    object.insert("kicad_project_path".into(), json!({
        "type": "string",
        "description": "A KiCad .kicad_pro project whose board design rules (hole-to-hole, edge clearance, via minimums, netclass clearances, rule severities) are applied before checking. The CLI spelling is --kicad-project."
    }));
    json!({
        "type": "object",
        "properties": properties,
        "oneOf": one_of_dsn_path_or_text(),
        "additionalProperties": false
    })
}

#[must_use]
pub fn board_info_schema() -> Value {
    json!({
        "type": "object",
        "properties": board_input_properties(),
        "oneOf": one_of_dsn_path_or_text(),
        "additionalProperties": false
    })
}

#[must_use]
pub fn list_settings_schema() -> Value {
    json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false
    })
}

#[must_use]
pub fn router_settings_schema() -> Value {
    json!({
        "type": "object",
        "title": "RouterSettings",
        "description": "The router's settings, as `settings/RouterSettings.java` declares them and as Gson spells them on the wire, plus `opt_changed_area_ms`, which the port adds because the Java constant behind it is inlined by javac and cannot be reached. Every field is optional: a value given here overrides the resolved default, and a field left out keeps it. Unknown keys are ignored rather than refused, which is Gson's behaviour and therefore this reader's.",
        "additionalProperties": false,
        "properties": {
            "enabled": { "type": "boolean", "description": "Run the auto-routing stage at all." },
            "algorithm": { "type": "string", "description": "Which routing algorithm to use. One name is implemented; anything else is normalised back to it." },
            "fanout": fanout_schema(),
            "copper_to_edge_clearance_um": { "type": "number", "description": "Clearance between copper and the board outline, in micrometres. Applied to the board at load time." },
            "hole_clearance_um": { "type": "number", "description": "Clearance between copper and drilled holes, in micrometres. Applied to the board at load time." },
            "neck_width_um": { "type": "number", "description": "Trace width, in micrometres, used where a full-width trace will not fit (see automatic_neckdown)." },
            "strict_drc": { "type": "boolean", "description": "Refuse a route that would leave a clearance violation, rather than accepting it and scoring the penalty." },
            "job_timeout": { "type": "string", "description": "Wall-clock budget for the whole job, as a timespan (for example 01:30:00 or 90s). Empty means no budget. A job that overruns it answers timed_out: true." },
            "max_passes": { "type": "integer", "description": "How many auto-routing passes to run. 0 means unlimited." },
            "layers": {
                "type": "array",
                "description": "Per-layer overrides, one entry per board layer in stack order. An array whose length disagrees with the board's layer count is discarded whole and re-derived from the board.",
                "items": layer_schema()
            },
            "trace_pull_tight_accuracy": { "type": "integer", "description": "How hard the optimizer pulls a trace tight; larger is more accurate and slower." },
            "allowed_via_types": { "type": "boolean", "description": "Let the router place vias. (The Java field is `viasAllowed`; `allowed_via_types` is the name Gson serialises it under.)" },
            "automatic_neckdown": { "type": "boolean", "description": "Narrow a trace to neck_width_um where the full width will not fit." },
            "optimizer": optimizer_schema(),
            "scoring": scoring_schema(),
            "max_threads": { "type": "integer", "description": "Worker threads for the stages that are parallel. Defaults to a share of the host's processor count." },
            "result_json": { "type": "string", "description": "Where a run writes its result manifest. Read by the CLI; this tool answers its result directly and does not write one." },
            "opt_changed_area_ms": { "type": "integer", "description": "Wall-clock budget, in milliseconds, for the pull-tight pass over a changed area. 0 or absent (the default) means no budget and the pull-tight always runs to completion, which is deterministic. The Java program hard-codes 1000 here and cannot switch it off, which makes its own output depend on how fast the machine is; set 1000 to reproduce that." },
            "smd_via_relaxation": { "type": "boolean", "description": "Discount via cost on pure-SMD nets (default true). DSN inputs also retain the legacy SMD attachment search relaxation. KiCad JSON inputs require explicit viaInPadAllowed permission; this setting never overrides it." },
            "failure_give_up_threshold": { "type": "integer", "description": "Skip an item on a later auto-routing pass once it has failed this many times. Absent (the default) retries every item on every pass." },
            "connection_search_steps": { "type": "integer", "description": "Cap each connection's search at this many maze-search steps, independent of machine speed. Absent is the default of 250000; 0 lifts the cap." }
        }
    })
}

/// `excludeFieldsWithModifiers` override). The port's `#[serde(skip)]` reproduces that exactly,
pub const TRANSIENT_ROUTER_SETTINGS_FIELDS: &[&str] = &[
    "max_items",
    "save_intermediate_stages",
    "ignore_net_classes",
    "board_specific_trace_costs_applied",
];

pub const TRANSIENT_OPTIMIZER_FIELDS: &[&str] = &[
    "board_update_strategy",
    "hybrid_ratio",
    "item_selection_strategy",
];

pub const TRANSIENT_SCORING_FIELDS: &[&str] = &[
    "preferred_direction_trace_cost",
    "undesired_direction_trace_cost",
];

pub const TRANSIENT_FANOUT_FIELDS: &[&str] = &[];

pub const TRANSIENT_LAYER_FIELDS: &[&str] = &[];

fn fanout_schema() -> Value {
    json!({
        "type": "object",
        "description": "The fanout pre-pass, which escapes SMD pins onto a routable layer before auto-routing starts.",
        "additionalProperties": false,
        "properties": {
            "enabled": { "type": "boolean", "description": "Run the fanout pre-pass. It is skipped anyway on a board with no SMD pins." },
            "max_passes": { "type": "integer", "description": "How many fanout passes to run." },
            "max_items": { "type": "integer", "description": "How many pins one fanout pass may process." },
            "max_milliseconds_per_pin": { "type": "integer", "description": "Wall-clock budget per pin. A machine-speed dependency: the same board can fan out differently on a slower host." },
            "ripup_allowed": { "type": "boolean", "description": "Let the fanout pass rip up existing traces." },
            "min_escape_length_mm": { "type": "number", "description": "Shortest escape trace the pass will produce, in millimetres." },
            "max_escape_length_mm": { "type": "number", "description": "Longest escape trace the pass will produce, in millimetres." },
            "start_via_diameter_mm": { "type": "number", "description": "Diameter of the via at the pin end of an escape, in millimetres." },
            "end_via_diameter_mm": { "type": "number", "description": "Diameter of the via at the far end of an escape, in millimetres." },
            "pin_sorting_order": { "type": "string", "description": "The order pins are fanned out in." },
            "fallback_to_board_vias": { "type": "boolean", "description": "Use the board's own via rule when the configured diameters do not fit." },
            "timeout": { "type": "string", "description": "Wall-clock budget for the fanout stage, as a timespan. A stage timeout is not a job timeout: the job still finishes COMPLETED." }
        }
    })
}

fn optimizer_schema() -> Value {
    json!({
        "type": "object",
        "description": "The optimizer stage, which runs after auto-routing to shorten traces and remove vias.",
        "additionalProperties": false,
        "properties": {
            "enabled": { "type": "boolean", "description": "Run the optimizer stage." },
            "algorithm": { "type": "string", "description": "Which optimizer algorithm to use." },
            "max_passes": { "type": "integer", "description": "How many optimizer passes to run." },
            "max_items": { "type": "integer", "description": "How many items one optimizer pass may process." },
            "max_threads": { "type": "integer", "description": "Worker threads for the optimizer." },
            "improvement_threshold": { "type": "number", "description": "Stop once a pass improves the score by less than this fraction. (The Java field is `optimizationImprovementThreshold`.)" },
            "max_consecutive_failures": { "type": "integer", "description": "Give up after this many consecutive passes that improve nothing." },
            "additional_ripup_cost_factor_at_start": { "type": "integer", "description": "Extra rip-up cost applied at the start of the stage, decaying as it runs." },
            "trace_ripup_cost_factor": { "type": "number", "description": "How expensive ripping up an existing trace is relative to routing a new one." },
            "max_autoroute_passes": { "type": "integer", "description": "How many auto-routing passes the optimizer may spend re-routing what it rips up." },
            "max_search_steps": { "type": "integer", "description": "A deterministic cap on the total search steps the optimizer may spend; 0 means unlimited. Bounds runaway routing on incomplete boards without a wall-clock." },
            "timeout": { "type": "string", "description": "Wall-clock budget for the optimizer stage, as a timespan. A stage timeout is not a job timeout: the job still finishes COMPLETED." }
        }
    })
}

fn scoring_schema() -> Value {
    json!({
        "type": "object",
        "description": "The routing cost table. The two per-layer arrays (preferred_direction_trace_cost, undesired_direction_trace_cost) are transient and cannot be set here: they are derived from the board aspect ratio and the two default_* costs below.",
        "additionalProperties": false,
        "properties": {
            "default_preferred_direction_trace_cost": { "type": "number", "description": "The value the per-layer preferred-direction costs are tuned from." },
            "default_undesired_direction_trace_cost": { "type": "number", "description": "The value the per-layer undesired-direction costs are tuned from." },
            "via_costs": { "type": "integer", "description": "Cost of placing one via." },
            "plane_via_costs": { "type": "integer", "description": "Cost of a via that lands on a plane (a poured net), usually lower than via_costs." },
            "start_ripup_costs": { "type": "integer", "description": "Cost of ripping up an existing trace at the start of a pass." },
            "unrouted_net_penalty": { "type": "number", "description": "Score penalty per connection still in the ratsnest." },
            "clearance_violation_penalty": { "type": "number", "description": "Score penalty per clearance violation left on the board." },
            "bend_penalty": { "type": "number", "description": "Score penalty per bend in a finished trace." },
            "default_bend_cost": { "type": "number", "description": "Millimetres of trace a bend is worth to the maze search, where a layer names no bend_cost of its own." },
            "smd_via_cost_factor": { "type": "number", "description": "Multiplier on the via cost for nets whose pins are all surface-mount; 1.0 charges such nets the full via cost." }
        }
    })
}

fn layer_schema() -> Value {
    json!({
        "type": "object",
        "description": "One board layer's overrides.",
        "additionalProperties": false,
        "properties": {
            "routable": { "type": "boolean", "description": "Whether the auto-router may use this layer." },
            "preferred_direction_horizontal": { "type": "boolean", "description": "Whether this layer's preferred trace direction is horizontal." },
            "bend_cost": { "type": "number", "description": "This layer's bend cost in millimetres of trace, overriding scoring.default_bend_cost. 0.0 is no penalty; 100.0 is the ceiling." }
        }
    })
}
