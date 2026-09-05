//! Rust twin of `scripts/differential/java/P7T7.java` (Plan 7 Task 1): `fr_router::score` —
//! `BoardStatistics`' computing constructor, `is_pin_escaped` and the three score methods.
//!
//! Usage: `p7t7 <dsn> [routeK] [ripupPassNo]`. With `routeK > 0` the first `routeK` connections
//! are routed through `fr_router::route_connection` first — the same four choices `p6t1` makes —
//! so the trace / via / bend / weighted-length blocks are exercised on a board that is actually
//! routed rather than on one the DSN reader just built.
//!
//! **The four routing choices are `P6T1.java`'s and are duplicated here on purpose.** The Java
//! twin *reuses* `P6T1`'s own `loadBoard`/`pickConnections`/`route` (they are package-private
//! statics and `P7T7.java` therefore declares the same package); Rust binaries cannot share a
//! private module without a crate-level refactor of the differential harness, so the four choices
//! are transcribed here instead — and any drift shows up as a `p7t7` diff, because the Java side
//! is the un-duplicated one:
//!
//! 1. settings are `DefaultSettings` + `set_layer_count` + `apply_board_specific_optimizations`;
//! 2. `start_marking_changed_area` runs before every connection (quirk #177);
//! 3. `route_connection`'s four parameters are the `RoutingJob` constructor's;
//! 4. nothing above plan-6 ruling 2's seam runs.
//!
//! The output is one `key=value` line per DTO field, per **variant** — the four constructor
//! argument combinations with live callers, A/B/C/D as `P7T7.java`'s class comment lists them —
//! then `calculateScore` / `getMaximumScore` / `getNormalizedScore` under four `ScoringSettings`
//! presets for A, B and C, then `is_pin_escaped` for every SMD pin in ascending item id.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufWriter, Write};
use std::time::UNIX_EPOCH;

use fr_board::prelude::*;
use fr_board::structure::Unit;
use fr_dsn::parser::scope_parameter::DsnReadOptions;
use fr_dsn::{java_double_to_string, java_float_to_string, BoardReadResult};
use fr_router::route_connection;
use fr_router::score::{java_double_stream_sum, BoardStatistics};
use fr_settings::sources::DefaultSettings;
use fr_settings::{HostEnvironment, RouterSettings, ScoringSettings, SettingsSource};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: p7t7 <dsn> [routeK] [ripupPassNo]");
        std::process::exit(2);
    }
    let dsn = std::fs::canonicalize(&args[0])
        .unwrap_or_else(|e| panic!("cannot resolve {}: {e}", args[0]));
    let route_k: usize = args.get(1).map_or(0, |a| a.parse().expect("routeK"));
    let ripup_pass_no: i32 = args.get(2).map_or(1, |a| a.parse().expect("ripupPassNo"));

    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    print_header(&mut out, &dsn, route_k, ripup_pass_no);

    let mut board = load_board(&dsn);
    let settings = build_settings(&board);

    for connection in pick_connections(&board, route_k) {
        route_one(&mut board, &settings, &connection, ripup_pass_no);
    }

    emit(
        &mut out,
        "A",
        &BoardStatistics::new(&mut board),
        &settings,
        true,
    );
    emit(
        &mut out,
        "B",
        &BoardStatistics::with_options(&mut board, None, false),
        &settings,
        true,
    );
    emit(
        &mut out,
        "C",
        &BoardStatistics::compute(&mut board, Some(Unit::Mil), true, true),
        &settings,
        true,
    );
    emit(
        &mut out,
        "D",
        &BoardStatistics::compute(&mut board, None, true, false),
        &settings,
        false,
    );

    emit_synthetic(&mut out);
    emit_kahan(&mut out);

    // `board.get_smd_pins()` is descending item id (quirk #63); Java sorts its own list ascending,
    // so this reverses.
    let mut smd_pins = board.get_smd_pins();
    smd_pins.reverse();
    for pin in smd_pins {
        let escaped = BoardStatistics::is_pin_escaped(&mut board, pin);
        writeln!(out, "escaped {}={escaped}", pin.0).expect("write");
    }
    out.flush().expect("flush");
}

// ------------------------------------------------------------------------------------------------
// Header, board, settings — `P6T1.java`'s choices, transcribed (see the module comment)
// ------------------------------------------------------------------------------------------------

fn print_header<W: Write>(out: &mut W, dsn: &std::path::Path, route_k: usize, pass: i32) {
    let jar =
        std::env::var("FREEROUTING_JAR").expect("environment variable FREEROUTING_JAR is not set");
    let jar = std::fs::canonicalize(jar).expect("jar exists");
    let meta = std::fs::metadata(&jar).expect("jar metadata");
    let mtime = meta
        .modified()
        .expect("mtime")
        .duration_since(UNIX_EPOCH)
        .expect("after epoch")
        .as_millis();
    writeln!(
        out,
        "HEADER jar={} bytes={} mtime={mtime} fixture={} routeK={route_k} ripupPassNo={pass}",
        jar.display(),
        meta.len(),
        dsn.file_name().expect("a file name").to_string_lossy(),
    )
    .expect("write");
    if let Ok(exe) = std::env::current_exe() {
        eprintln!("rust-binary {}", exe.display());
    }
}

/// `P6T1.loadBoard`, without the `.rules` slot this driver does not take.
fn load_board(dsn: &std::path::Path) -> Board {
    let file = std::fs::File::open(dsn).unwrap_or_else(|e| panic!("cannot open {dsn:?}: {e}"));
    let design_name = dsn
        .file_name()
        .expect("a file name")
        .to_string_lossy()
        .into_owned();
    let result = fr_dsn::read_board(file, None, Some(&design_name), &DsnReadOptions::default());
    match result {
        BoardReadResult::Success { board, .. } | BoardReadResult::OutlineMissing { board, .. } => {
            *board.unwrap_or_else(|| panic!("{design_name} produced no board"))
        }
        other => panic!("{design_name} did not read: {other:?}"),
    }
}

/// `P6T1.main`'s three settings lines.
fn build_settings(board: &Board) -> RouterSettings {
    let host = HostEnvironment::detect();
    let mut settings = DefaultSettings::new(&host)
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    settings.set_layer_count(board.get_layer_count());
    settings.apply_board_specific_optimizations(board);
    settings
}

/// `P6T1.Connection`.
struct Connection {
    item_id: ItemId,
    net_no: i32,
}

/// `P6T1.pickConnections`, with this driver's own `route_k == 0` guard (Java's tests
/// `result.size() >= maxItems` **after** appending, so a `maxItems` of 0 would still return one).
fn pick_connections(board: &Board, max_items: usize) -> Vec<Connection> {
    let mut result = Vec::new();
    if max_items == 0 {
        return result;
    }
    for item_id in board.items_in_board_order() {
        let Some(item) = board.get_item(item_id) else {
            continue;
        };
        if item.as_connectable().is_none() {
            continue;
        }
        let net_nos: Vec<i32> = item.net_nos().to_vec();
        for net_no in net_nos {
            if board.unconnected_set(item_id, net_no).is_empty() {
                continue;
            }
            result.push(Connection { item_id, net_no });
            if result.len() >= max_items {
                return result;
            }
        }
    }
    result
}

/// `P6T1.routeOne`, minus the rendering: this driver compares the *statistics* of the routed
/// board, and `p6t1` already compares the per-connection outcome byte for byte.
fn route_one(
    board: &mut Board,
    settings: &RouterSettings,
    connection: &Connection,
    ripup_pass_no: i32,
) {
    if board.get_item(connection.item_id).is_none() {
        return;
    }
    board.start_marking_changed_area();
    let mut ripped: BTreeSet<ItemId> = BTreeSet::new();
    let mut ripup_costs: BTreeMap<ItemId, i32> = BTreeMap::new();
    let trace_costs = settings.get_trace_costs();
    let mut engine = None;
    route_connection(
        board,
        &mut engine,
        connection.item_id,
        connection.net_no,
        settings,
        &trace_costs,
        &mut ripped,
        &mut ripup_costs,
        ripup_pass_no,
        settings.get_start_ripup_costs(),
        !settings.is_fanout_enabled(),
        false,
        &|| false,
    );
}

// ------------------------------------------------------------------------------------------------
// Rendering — `P7T7.emit`
// ------------------------------------------------------------------------------------------------

fn emit<W: Write>(
    out: &mut W,
    tag: &str,
    s: &BoardStatistics,
    settings: &RouterSettings,
    with_score: bool,
) {
    let b = s.board.bounding_box.expect("the ctor always sets it");
    let size = s.board.size.expect("the ctor always sets it");
    p(out, tag, "host", &s.host);
    p(out, tag, "unit", &s.unit);
    p(out, tag, "board.boundingBox.x", &f(Some(b.x)));
    p(out, tag, "board.boundingBox.y", &f(Some(b.y)));
    p(out, tag, "board.boundingBox.width", &f(Some(b.width)));
    p(out, tag, "board.boundingBox.height", &f(Some(b.height)));
    p(out, tag, "board.size.x", &f(Some(size.x)));
    p(out, tag, "board.size.y", &f(Some(size.y)));
    p(out, tag, "board.size.width", &f(Some(size.width)));
    p(out, tag, "board.size.height", &f(Some(size.height)));
    p(out, tag, "layers.totalCount", &i(s.layers.total_count));
    p(out, tag, "layers.signalCount", &i(s.layers.signal_count));
    p(out, tag, "items.totalCount", &i(s.items.total_count));
    p(out, tag, "items.traceCount", &i(s.items.trace_count));
    p(out, tag, "items.viaCount", &i(s.items.via_count));
    p(
        out,
        tag,
        "items.conductionAreaCount",
        &i(s.items.conduction_area_count),
    );
    p(
        out,
        tag,
        "items.drillItemCount",
        &i(s.items.drill_item_count),
    );
    p(out, tag, "items.pinCount", &i(s.items.pin_count));
    p(
        out,
        tag,
        "items.componentOutlineCount",
        &i(s.items.component_outline_count),
    );
    p(out, tag, "items.otherCount", &i(s.items.other_count));
    p(
        out,
        tag,
        "components.totalCount",
        &i(s.components.total_count),
    );
    p(out, tag, "pads.totalCount", &i(s.pads.total_count));
    p(out, tag, "nets.totalCount", &i(s.nets.total_count));
    p(out, tag, "nets.classCount", &i(s.nets.class_count));
    p(
        out,
        tag,
        "connections.maximumCount",
        &i(s.connections.maximum_count),
    );
    p(
        out,
        tag,
        "connections.incompleteCount",
        &i(s.connections.incomplete_count),
    );
    p(out, tag, "traces.totalCount", &i(s.traces.total_count));
    p(
        out,
        tag,
        "traces.totalSegmentCount",
        &i(s.traces.total_segment_count),
    );
    p(out, tag, "traces.totalLength", &f(s.traces.total_length));
    p(
        out,
        tag,
        "traces.totalLengthMm",
        &f(s.traces.total_length_mm),
    );
    p(
        out,
        tag,
        "traces.totalWeightedLength",
        &f(s.traces.total_weighted_length),
    );
    p(
        out,
        tag,
        "traces.averageLength",
        &f(s.traces.average_length),
    );
    p(
        out,
        tag,
        "traces.totalVerticalLength",
        &f(s.traces.total_vertical_length),
    );
    p(
        out,
        tag,
        "traces.totalHorizontalLength",
        &f(s.traces.total_horizontal_length),
    );
    p(
        out,
        tag,
        "traces.totalAngledLength",
        &f(s.traces.total_angled_length),
    );
    p(out, tag, "bends.totalCount", &i(s.bends.total_count));
    p(
        out,
        tag,
        "bends.ninetyDegreeCount",
        &i(s.bends.ninety_degree_count),
    );
    p(
        out,
        tag,
        "bends.fortyFiveDegreeCount",
        &i(s.bends.forty_five_degree_count),
    );
    p(
        out,
        tag,
        "bends.otherAngleCount",
        &i(s.bends.other_angle_count),
    );
    p(out, tag, "vias.totalCount", &i(s.vias.total_count));
    p(
        out,
        tag,
        "vias.throughHoleCount",
        &i(s.vias.through_hole_count),
    );
    p(out, tag, "vias.blindCount", &i(s.vias.blind_count));
    p(out, tag, "vias.buriedCount", &i(s.vias.buried_count));
    p(
        out,
        tag,
        "clearanceViolations.totalCount",
        &i(s.clearance_violations.total_count),
    );
    p(
        out,
        tag,
        "clearanceViolations.minViolationUm",
        &d(s.clearance_violations.min_violation_um),
    );
    p(
        out,
        tag,
        "clearanceViolations.maxViolationUm",
        &d(s.clearance_violations.max_violation_um),
    );
    p(
        out,
        tag,
        "clearanceViolations.avgViolationUm",
        &d(s.clearance_violations.avg_violation_um),
    );
    p(
        out,
        tag,
        "fanout.totalSmdPins",
        &s.fanout.total_smd_pins.to_string(),
    );
    p(
        out,
        tag,
        "fanout.pinsToEscape",
        &s.fanout.pins_to_escape.to_string(),
    );
    p(
        out,
        tag,
        "fanout.escapedCount",
        &s.fanout.escaped_count.to_string(),
    );

    if !with_score {
        return;
    }
    for index in 0..4 {
        let sc = preset(settings, index);
        p(
            out,
            tag,
            &format!("score{index}.calculateScore"),
            &f(Some(s.calculate_score(&sc))),
        );
        p(
            out,
            tag,
            &format!("score{index}.getMaximumScore"),
            &f(Some(s.maximum_score(&sc))),
        );
        p(
            out,
            tag,
            &format!("score{index}.getNormalizedScore"),
            &f(Some(s.normalized_score(&sc))),
        );
    }
}

/// `P7T7.Synth`.
struct Synth {
    tag: &'static str,
    maximum_count: i32,
    incomplete_count: i32,
    violation_count: i32,
    bend_count: i32,
    total_length_mm: f32,
    via_count: i32,
    unrouted_net_penalty: f32,
    clearance_violation_penalty: f32,
    bend_penalty: f32,
    trace_cost: f64,
    via_costs: i32,
}

/// `P7T7.SYNTHETIC` — the four cases the corpus cannot produce. See the Java driver's Javadoc for
/// what each one pins.
const SYNTHETIC: [Synth; 6] = [
    Synth {
        tag: "S0",
        maximum_count: 16_777_217,
        incomplete_count: 1,
        violation_count: 3,
        bend_count: 7,
        total_length_mm: 1.1,
        via_count: 5,
        unrouted_net_penalty: 1.0,
        clearance_violation_penalty: 0.1,
        bend_penalty: 0.1,
        trace_cost: 0.1,
        via_costs: 3,
    },
    Synth {
        tag: "S1",
        maximum_count: 3,
        incomplete_count: 3,
        violation_count: 11,
        bend_count: 129,
        total_length_mm: 12345.678,
        via_count: 17,
        unrouted_net_penalty: 1.0E7,
        clearance_violation_penalty: 1.5,
        bend_penalty: 0.75,
        trace_cost: 3.3,
        via_costs: 42,
    },
    Synth {
        tag: "S2",
        maximum_count: 0,
        incomplete_count: 4,
        violation_count: 0,
        bend_count: 0,
        total_length_mm: 2.5,
        via_count: 1,
        unrouted_net_penalty: 5.0E6,
        clearance_violation_penalty: 1.0,
        bend_penalty: 1.0,
        trace_cost: 1.0,
        via_costs: 50,
    },
    Synth {
        tag: "S3",
        maximum_count: 7,
        incomplete_count: 0,
        violation_count: 0,
        bend_count: 0,
        total_length_mm: 0.0,
        via_count: 3000,
        unrouted_net_penalty: 1.0,
        clearance_violation_penalty: 1.0,
        bend_penalty: 1.0,
        trace_cost: 1.0,
        via_costs: 1_000_000,
    },
    Synth {
        tag: "S4",
        maximum_count: 1,
        incomplete_count: 1,
        violation_count: 0,
        bend_count: 0,
        total_length_mm: 1.0E-40,
        via_count: 0,
        unrouted_net_penalty: 3.4E38,
        clearance_violation_penalty: 1.0,
        bend_penalty: 1.0,
        trace_cost: 1.0,
        via_costs: 1,
    },
    Synth {
        tag: "S5",
        maximum_count: 2,
        incomplete_count: 2,
        violation_count: 0,
        bend_count: 0,
        total_length_mm: 0.0,
        via_count: 0,
        unrouted_net_penalty: 3.4E38,
        clearance_violation_penalty: 1.0,
        bend_penalty: 1.0,
        trace_cost: 1.0,
        via_costs: 1,
    },
];

/// `P7T7.emitSynthetic`.
fn emit_synthetic<W: Write>(out: &mut W) {
    for synth in &SYNTHETIC {
        let mut s = BoardStatistics::default();
        s.connections.maximum_count = Some(synth.maximum_count);
        s.connections.incomplete_count = Some(synth.incomplete_count);
        s.clearance_violations.total_count = Some(synth.violation_count);
        s.bends.total_count = Some(synth.bend_count);
        s.traces.total_length_mm = Some(synth.total_length_mm);
        s.vias.total_count = Some(synth.via_count);

        let sc = ScoringSettings {
            unrouted_net_penalty: Some(synth.unrouted_net_penalty),
            clearance_violation_penalty: Some(synth.clearance_violation_penalty),
            bend_penalty: Some(synth.bend_penalty),
            default_preferred_direction_trace_cost: Some(synth.trace_cost),
            via_costs: Some(synth.via_costs),
            ..ScoringSettings::default()
        };

        p(
            out,
            synth.tag,
            "calculateScore",
            &f(Some(s.calculate_score(&sc))),
        );
        p(
            out,
            synth.tag,
            "getMaximumScore",
            &f(Some(s.maximum_score(&sc))),
        );
        p(
            out,
            synth.tag,
            "getNormalizedScore",
            &f(Some(s.normalized_score(&sc))),
        );
    }
}

/// `P7T7.KAHAN` — the five vectors that pin `DoubleStream.sum()`'s **subtracting** tail
/// (`Collectors.computeFinalSum`: `summands[0] - summands[1]`, the compensation slot being
/// negated). The corpus cannot pin it, because `BoardStatistics.java:189` narrows the sum to
/// `f32` one line later; these do, at `double` width. `K3` drives the
/// `isNaN(tmp) && isInfinite(simpleSum)` arm and `K4` is the empty stream.
const KAHAN: [&[f64]; 5] = [
    &[44646902.244757555, 15114766.05020856, 134419886.7378119],
    &[
        153162863.29820704,
        22943764.53161407,
        51720560.6157495,
        294156676.54173774,
    ],
    &[29004725.81742059, 21933330.804348517, 86551149.06402807],
    &[f64::MAX, f64::MAX, -f64::MAX],
    &[],
];

/// `P7T7.emitKahan`.
fn emit_kahan<W: Write>(out: &mut W) {
    for (k, values) in KAHAN.iter().enumerate() {
        p(
            out,
            &format!("K{k}"),
            "sum",
            &java_double_to_string(java_double_stream_sum(values.iter().copied())),
        );
    }
}

/// `P7T7.preset`.
fn preset(settings: &RouterSettings, index: usize) -> ScoringSettings {
    let mut sc = settings
        .scoring
        .clone()
        .expect("DefaultSettings always writes a scoring block");
    match index {
        1 => sc.unrouted_net_penalty = Some(17.3),
        2 => sc.via_costs = Some(77),
        3 => sc.bend_penalty = Some(3.25),
        _ => {}
    }
    sc
}

fn p<W: Write>(out: &mut W, tag: &str, key: &str, value: &str) {
    writeln!(out, "{tag} {key}={value}").expect("write");
}

/// `Integer.toString`, with Java's `null` token for an absent boxed field.
fn i(value: Option<i32>) -> String {
    value.map_or_else(|| "null".to_string(), |v| v.to_string())
}

/// `Float.toString`.
fn f(value: Option<f32>) -> String {
    value.map_or_else(|| "null".to_string(), java_float_to_string)
}

/// `Double.toString`.
fn d(value: Option<f64>) -> String {
    value.map_or_else(|| "null".to_string(), java_double_to_string)
}
