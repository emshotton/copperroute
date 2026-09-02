//! The four tool input schemas, and the `RouterSettings` schema `list_settings` answers — all of
//! them **hand-written** `serde_json::json!` literals.
//!
//! # Why not `schemars`
//!
//! Spec §13 says *"tool input schemas generated with `schemars` from the settings struct so CLI
//! and MCP cannot drift"*. Controller ruling AO/General **refuses the dependency**, and the
//! reason is not the crate: deriving `JsonSchema` on [`RouterSettings`] means deriving it on
//! `FanoutSettings`, `OptimizerSettings`, `ScoringSettings`, `LayerSettings` and the two
//! strategy enums — a workspace-wide derive sweep in the last plan of the port, on the one type
//! Plan 4 ruling 3 deliberately kept free of extra derives. The Global Constraint forbids adding
//! a dependency, and this is not the task to spend a waiver on.
//!
//! **The anti-drift device the spec asked for is bought back by two tests instead**, and they are
//! what discharges ruling AO's "revisit only if that proves error-prone":
//!
//! 1. `the_schemas_match_the_committed_golden` (`tests/mcp_stdio.rs`) — every schema is compared
//!    against `tests/data/mcp-schemas.json` byte for byte, so a schema cannot change without a
//!    reviewer seeing the diff;
//! 2. `every_settings_field_is_in_the_schema_and_vice_versa` — the schema's property names are
//!    compared **both ways** against `RouterSettings::FIELD_NAMES` and the four nested structs'
//!    own `FIELD_NAMES`, so a settings field added later cannot silently vanish from the schema
//!    and a schema property cannot outlive the field it describes.
//!
//! The second is strictly stronger than `schemars` for the failure everyone actually fears (a
//! new field nobody exposes) and strictly weaker for one nobody has hit (a field whose *type*
//! changes without its name changing) — recorded here so the trade is visible rather than
//! implied.
//!
//! # Defaults live in `DefaultSettings`, not here
//!
//! The schema carries a `description` per property and **no `default`**. `list_settings` answers
//! `{"schema": …, "defaults": …}`, where `defaults` is `DefaultSettings`' own table resolved at
//! call time (`settings/sources/DefaultSettings.java:83`) — including the two fields that depend
//! on the host's processor count (plan ruling 6). Copying those numbers into a literal here would
//! be a second source of truth that a `DefaultSettings` edit could not update, and it would make
//! the golden above machine-dependent.

use serde_json::{Value, json};

/// The `dsn_path` / `dsn_text` pair every board-reading tool takes, with the "exactly one"
/// constraint expressed as JSON Schema's `oneOf` over `required`.
///
/// Ruling AO: **flat arguments**. The jar's 24 generated tools wrap everything in
/// `{path, query, body}` (`api/mcp/OpenApiMcpToolRegistry.java`, measured in
/// `docs/plan-8-prep/evidence/job3-summary.md` §4); its own four hand-written tools are flat, and
/// so are these. Delta row 10.
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

/// `oneOf` over the two `required` shapes — the schema spelling of "exactly one of".
fn one_of_dsn_path_or_text() -> Value {
    json!([
        { "required": ["dsn_path"], "not": { "required": ["dsn_text"] } },
        { "required": ["dsn_text"], "not": { "required": ["dsn_path"] } }
    ])
}

/// `route_board`'s input — spec §13's
/// `{ dsn_path | dsn_text, ses_path?, rules_path?, output_path?, settings? }`.
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

/// `check_drc`'s input — spec §13's `{ dsn_path | dsn_text, ses_path?, rules_path? }`.
///
/// **No coordinate-unit option**, and that is a recorded decision rather than an omission: `-drc`
/// hard-codes `"mm"` (`Freerouting.java:335-336`, quirk #151) and exposing the other four arms of
/// `DesignRulesChecker.convertCoordinate` (`:512-525`) would make this tool strictly more capable
/// than the surface it reproduces. `commands::drc`'s step 10 carries the same note.
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
    json!({
        "type": "object",
        "properties": properties,
        "oneOf": one_of_dsn_path_or_text(),
        "additionalProperties": false
    })
}

/// `board_info`'s input — spec §13's `{ dsn_path | dsn_text }`.
#[must_use]
pub fn board_info_schema() -> Value {
    json!({
        "type": "object",
        "properties": board_input_properties(),
        "oneOf": one_of_dsn_path_or_text(),
        "additionalProperties": false
    })
}

/// `list_settings`' input — spec §13's `{}`. It takes nothing at all.
#[must_use]
pub fn list_settings_schema() -> Value {
    json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false
    })
}

/// The [`fr_settings::RouterSettings`] schema `list_settings` answers, and the thing
/// `route_board`'s `settings` argument must satisfy.
///
/// Every property name here is checked against the struct's own `FIELD_NAMES` in both directions
/// — see this module's docs.
#[must_use]
pub fn router_settings_schema() -> Value {
    json!({
        "type": "object",
        "title": "RouterSettings",
        "description": "The router's settings, as `settings/RouterSettings.java` declares them and as Gson spells them on the wire. Every field is optional: a value given here overrides the resolved default, and a field left out keeps it. Unknown keys are ignored rather than refused, which is Gson's behaviour and therefore this reader's.",
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
            "result_json": { "type": "string", "description": "Where a run writes its result manifest. Read by the CLI; this tool answers its result directly and does not write one." }
        }
    })
}

/// The `RouterSettings` fields this schema deliberately does **not** carry, because the reader
/// cannot see them: each is `transient` in Java, and Gson's default exclusion strategy drops a
/// `transient` field from **both** directions (`util/gson/GsonProvider.java` registers no
/// `excludeFieldsWithModifiers` override). The port's `#[serde(skip)]` reproduces that exactly,
/// so a caller who sent one of these would have it silently ignored — and a schema that
/// advertised it would be promising something the reader cannot deliver.
///
/// `layers` is the one `transient` field that *is* here: `RouterSettingsTypeAdapterFactory.read`
/// re-reads it from the raw tree (`:59-64`), which is why the port's attribute is
/// `skip_serializing` rather than `skip` — readable, not writable.
///
/// The list is a **constant, not a comment**, because
/// `every_settings_field_is_in_the_schema_and_vice_versa` counts against it: adding a field to
/// `RouterSettings` without adding it to the schema *or* to this list fails that test.
pub const TRANSIENT_ROUTER_SETTINGS_FIELDS: &[&str] = &[
    "max_items",
    "save_intermediate_stages",
    "ignore_net_classes",
    "board_specific_trace_costs_applied",
];

/// [`TRANSIENT_ROUTER_SETTINGS_FIELDS`]'s counterpart for `OptimizerSettings` — three `transient`
/// fields with no `TypeAdapterFactory` to re-read them, so Gson drops all three both ways.
pub const TRANSIENT_OPTIMIZER_FIELDS: &[&str] = &[
    "board_update_strategy",
    "hybrid_ratio",
    "item_selection_strategy",
];

/// …and for `ScoringSettings`: the two per-layer cost arrays are `transient`, so a caller cannot
/// set them here. They are derived from the board's aspect ratio and the two `default_*` costs,
/// which **are** settable.
pub const TRANSIENT_SCORING_FIELDS: &[&str] = &[
    "preferred_direction_trace_cost",
    "undesired_direction_trace_cost",
];

/// `FanoutSettings` and `LayerSettings` have no `transient` field at all — the empty lists are
/// here so the test below can treat all five structs uniformly.
pub const TRANSIENT_FANOUT_FIELDS: &[&str] = &[];

/// See [`TRANSIENT_FANOUT_FIELDS`].
pub const TRANSIENT_LAYER_FIELDS: &[&str] = &[];

/// `settings/FanoutSettings.java` — the SMD-pin fanout pre-pass.
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

/// `settings/OptimizerSettings.java` — the post-routing optimizer stage.
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
            "timeout": { "type": "string", "description": "Wall-clock budget for the optimizer stage, as a timespan. A stage timeout is not a job timeout: the job still finishes COMPLETED." }
        }
    })
}

/// `settings/ScoringSettings.java` — the maze-expansion cost table and the score's penalties.
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
            "default_bend_cost": { "type": "number", "description": "Cost added each time the maze search changes direction, where a layer names no bend_cost of its own." }
        }
    })
}

/// `settings/LayerSettings.java` — one entry of `layers`.
fn layer_schema() -> Value {
    json!({
        "type": "object",
        "description": "One board layer's overrides.",
        "additionalProperties": false,
        "properties": {
            "routable": { "type": "boolean", "description": "Whether the auto-router may use this layer." },
            "preferred_direction_horizontal": { "type": "boolean", "description": "Whether this layer's preferred trace direction is horizontal." },
            "bend_cost": { "type": "number", "description": "This layer's bend cost, overriding scoring.default_bend_cost. 0.0 is no penalty; 9.9 strongly avoids bends." }
        }
    })
}
