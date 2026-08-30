//! Plan 6 Task 17: the port's per-connection routing against the HEAD jar's, on five boards.
//!
//! # What the references are
//!
//! `tests/reference/<stem>/router.jsonl` is one JSON line per connection, written **verbatim** by
//! `scripts/differential/java/P6T1.java` on the clone's HEAD build (plan-6's global constraints
//! make HEAD the parity jar) — no post-processing beyond dropping the driver's own `HEADER` line,
//! which names the jar by absolute path and lives in `router.meta.txt` instead. The rows, the
//! exact command per stem and the jar's identity are `tests/reference/router-fixtures.txt` and
//! each stem's `router.meta.txt`; `scripts/gen-router-reference.sh` regenerates them, and
//! `scripts/differential/run.sh p6t1` is the same driver diffed live against the same Rust code
//! this file runs.
//!
//! # The acceptance ladder (plan-6 ruling 1)
//!
//! Per connection, in order:
//!
//! * **(a)** the same `AutorouteAttemptState` *and* the same ripped-item id set — required for
//!   every connection of every stem;
//! * **(b)** the same inserted geometry: every new trace's layer, half width and polyline corner
//!   list, and every new via's centre, padstack and layer span, in insertion order, with the same
//!   item ids — required for `router-rpi-splitter`, reported for the rest;
//! * **(c)** spec §9's metric block: incompletes delta equal, via delta equal, `violations == 0`,
//!   cumulative trace length within ±10 % — required everywhere.
//!
//! **Measured result: every stem reaches (a), (b) and (c) on every connection.** The ladder's
//! demotion path — a connection that reaches (a)+(c) but not (b) becomes a README row rather than
//! a failure — is therefore unused, and [`geometry_is_required_where_it_was_reached`] is what stops
//! it being quietly re-entered: it asserts that (b) holds on *all five* stems, so a future change
//! that demotes one has to say so in the ladder rather than in a passing test.
//!
//! # `router-tutorial-board` routes nothing, and that is the assertion
//!
//! `examples/tutorial_board/tutorial_board.dsn`'s `(network …)` scope is 438 empty `@:no_net_N`
//! nets, so no item has a non-empty unconnected set and the connection list is empty. The stem is
//! kept because "the port agrees there is nothing to route here" is a real regression guard on the
//! DSN reader and on `Board::unconnected_set`, and because the plan's fixture table names the
//! board; `every_stem_has_the_connection_count_its_meta_records` is where the count is pinned.

use std::collections::{BTreeMap, BTreeSet};

use fr_board::prelude::*;
use fr_drc::DesignRulesChecker;
use fr_dsn::java_double_to_string;
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_geometry::Point;
use fr_router::route_connection;
use fr_settings::sources::DefaultSettings;
use fr_settings::{HostEnvironment, RouterSettings, SettingsSource};
use parity::{RouterConnectionDoc, RouterMetrics, RouterTraceDoc, RouterViaDoc};

// ---------------------------------------------------------------------------------------------
// The fixture table
// ---------------------------------------------------------------------------------------------

/// One row of `tests/reference/router-fixtures.txt`: `stem|dsn|max_items`.
struct Row {
    stem: String,
    dsn: String,
    max_items: usize,
}

/// Reads the generator's own fixture table, so the tests and the references cannot drift apart.
fn rows() -> Vec<Row> {
    let path = parity::workspace_root().join("tests/reference/router-fixtures.txt");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let mut fields = line.split('|');
            let mut next = || fields.next().unwrap_or_default().trim().to_string();
            let (stem, dsn, max_items) = (next(), next(), next());
            Row {
                stem,
                dsn,
                max_items: max_items
                    .parse()
                    .unwrap_or_else(|e| panic!("max_items {max_items:?}: {e}")),
            }
        })
        .collect()
}

fn row(stem: &str) -> Row {
    rows()
        .into_iter()
        .find(|r| r.stem == stem)
        .unwrap_or_else(|| panic!("no row for {stem} in router-fixtures.txt"))
}

fn reference_path(stem: &str) -> std::path::PathBuf {
    parity::reference(stem, "router.jsonl")
}

fn read_reference(stem: &str) -> Vec<RouterConnectionDoc> {
    let path = reference_path(stem);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    parity::parse_router_jsonl(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

// ---------------------------------------------------------------------------------------------
// The port side — the exact four choices `P6T1.java` documents
// ---------------------------------------------------------------------------------------------

fn load_board(rel_path: &str) -> Board {
    let path = parity::java_dir().join(rel_path);
    let file = std::fs::File::open(&path)
        .unwrap_or_else(|e| panic!("cannot open {}: {e}", path.display()));
    let design_name = path
        .file_name()
        .expect("a file name")
        .to_string_lossy()
        .into_owned();
    match fr_dsn::read_board(file, None, Some(&design_name), &DsnReadOptions::default()) {
        BoardReadResult::Success { board, .. } | BoardReadResult::OutlineMissing { board, .. } => {
            *board.unwrap_or_else(|| panic!("{design_name} produced no board"))
        }
        other => panic!("{design_name} did not read: {other:?}"),
    }
}

/// `P6T1.main`'s settings: the headless ladder's priority-0 source, sized and tuned for the board.
/// Not a bare `RouterSettings::new()` — `DefaultSettings.java:103` sets `automaticNeckdown = true`.
fn build_settings(board: &Board) -> RouterSettings {
    let mut settings = DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    settings.set_layer_count(board.get_layer_count());
    settings.apply_board_specific_optimizations(board);
    settings
}

/// `P6T1.pickConnections`: `getItems()` order (descending id, quirk #63) × the item's own net
/// index order, keeping the pairs with a non-empty unconnected set, computed once.
fn pick_connections(board: &Board, max_items: usize) -> Vec<(ItemId, i32)> {
    let mut result = Vec::new();
    for item_id in board.items_in_board_order() {
        let Some(item) = board.get_item(item_id) else {
            continue;
        };
        if item.as_connectable().is_none() {
            continue;
        }
        for net_no in item.net_nos().to_vec() {
            if board.unconnected_set(item_id, net_no).is_empty() {
                continue;
            }
            result.push((item_id, net_no));
            if result.len() >= max_items {
                return result;
            }
        }
    }
    result
}

/// Routes the whole stem and answers the port's own `router.jsonl`, as typed documents.
fn route_stem(row: &Row) -> Vec<RouterConnectionDoc> {
    let mut board = load_board(&row.dsn);
    let settings = build_settings(&board);
    let trace_costs = settings.get_trace_costs();
    let mut out = Vec::new();

    for (k, (item_id, net_no)) in pick_connections(&board, row.max_items)
        .into_iter()
        .enumerate()
    {
        let k = k + 1;
        if board.get_item(item_id).is_none() {
            out.push(RouterConnectionDoc {
                k,
                item: i64::from(item_id.0),
                net: net_no,
                state: "GONE".to_string(),
                details: String::new(),
                ripped: Vec::new(),
                ripup_costs: Vec::new(),
                max_id_before: 0,
                max_id_after: 0,
                traces: Vec::new(),
                vias: Vec::new(),
                other_inserted: 0,
                metrics: None,
            });
            continue;
        }

        // `AutoroutePassRunner.java:224` — quirk #177 makes the presence of `changed_area`
        // observable inside `TraceShover::insert`, so leaving this out would route another board.
        board.start_marking_changed_area();
        let mut ripped: BTreeSet<ItemId> = BTreeSet::new();
        let mut ripup_costs: BTreeMap<ItemId, i32> = BTreeMap::new();
        let max_id_before = board.communication.id_gen.max_generated_id();
        let mut engine = None;
        let result = route_connection(
            &mut board,
            &mut engine,
            item_id,
            net_no,
            &settings,
            &trace_costs,
            &mut ripped,
            &mut ripup_costs,
            1,
            settings.get_start_ripup_costs(),
            !settings.is_fanout_enabled(),
            false,
            &|| false,
        );
        let max_id_after = board.communication.id_gen.max_generated_id();
        let (traces, vias, other_inserted) = inserted_geometry(&board, max_id_before);
        let metrics = metrics(&mut board, net_no);

        out.push(RouterConnectionDoc {
            k,
            item: i64::from(item_id.0),
            net: net_no,
            state: result.state.name().to_string(),
            details: result.details.clone().unwrap_or_default(),
            // Rendered in Java's descending `TreeSet<Item>` order (quirk #44).
            ripped: ripped.iter().rev().map(|id| i64::from(id.0)).collect(),
            ripup_costs: ripup_costs
                .iter()
                .map(|(id, cost)| (i64::from(id.0), *cost))
                .collect(),
            max_id_before: i64::from(max_id_before.0),
            max_id_after: i64::from(max_id_after.0),
            traces,
            vias,
            other_inserted,
            metrics: Some(metrics),
        });
    }
    out
}

/// `P6T1.appendInsertedGeometry`.
fn inserted_geometry(
    board: &Board,
    max_id_before: ItemId,
) -> (Vec<RouterTraceDoc>, Vec<RouterViaDoc>, usize) {
    let inserted: Vec<ItemId> = board
        .items_in_board_order()
        .into_iter()
        .filter(|id| id.0 > max_id_before.0)
        .rev()
        .collect();
    let ctx = board.ctx();
    let mut traces = Vec::new();
    let mut vias = Vec::new();
    let mut other = 0;
    for id in inserted {
        match board.get_item(id) {
            Some(Item::Trace(trace)) => traces.push(RouterTraceDoc {
                id: i64::from(id.0),
                layer: trace.get_layer(),
                half_width: trace.get_half_width(),
                corners: (0..trace.polyline().corner_count())
                    .map(|i| corner(trace.polyline(), i))
                    .collect(),
            }),
            Some(Item::Via(via)) => vias.push(RouterViaDoc {
                id: i64::from(id.0),
                center: point(&via.get_center()),
                padstack: via
                    .get_padstack(&ctx)
                    .expect("a via always resolves its padstack")
                    .name
                    .clone(),
                first_layer: via.first_layer(&ctx),
                last_layer: via.last_layer(&ctx),
            }),
            _ => other += 1,
        }
    }
    (traces, vias, other)
}

/// `P6T1.appendMetrics`.
fn metrics(board: &mut Board, net_no: i32) -> RouterMetrics {
    let vias = board.net_via_count(net_no) as i64;
    let trace_length = java_double_to_string(board.cumulative_trace_length());
    let mut drc = DesignRulesChecker::new(board);
    RouterMetrics {
        incompletes: drc.get_incomplete_count() as i64,
        vias,
        trace_length,
        violations: drc.get_all_clearance_violations().len() as i64,
    }
}

/// `P6T1.pt`.
fn point(p: &Point) -> String {
    match p {
        Point::Int(ip) => format!("({},{})", ip.x, ip.y),
        Point::Rational(_) => {
            let f = p.to_float();
            format!(
                "~({},{})",
                java_double_to_string(f.x),
                java_double_to_string(f.y)
            )
        }
    }
}

/// `P6T1.corners`, one corner.
fn corner(p: &fr_geometry::Polyline, i: usize) -> String {
    match p.corner(i) {
        Some(Point::Int(ip)) => format!("({},{})", ip.x, ip.y),
        _ => {
            let f = p.corner_approx(i).expect("a corner of a valid polyline");
            format!(
                "~({},{})",
                java_double_to_string(f.x),
                java_double_to_string(f.y)
            )
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The ladder
// ---------------------------------------------------------------------------------------------

/// How far up ruling 1's ladder one stem got. `check` requires (a) and (c) and returns this so a
/// caller can require or merely report (b).
struct Ladder {
    /// Connections whose (b) rung failed, with the first differing value.
    geometry_diffs: Vec<String>,
    connections: usize,
}

fn check(stem: &str) -> Option<Ladder> {
    if !parity::require_java_dir() {
        return None;
    }
    if !parity::require_reference(&reference_path(stem)) {
        return None;
    }
    let row = row(stem);
    let expected = read_reference(stem);
    let actual = route_stem(&row);

    assert_eq!(
        actual.len(),
        expected.len(),
        "{stem}: the port found {} connections, the reference has {}",
        actual.len(),
        expected.len()
    );

    let mut geometry_diffs = Vec::new();
    for (i, (got, want)) in actual.iter().zip(expected.iter()).enumerate() {
        let at = format!("{stem} k={}", i + 1);
        // The connection's identity has to agree before any rung means anything.
        assert_eq!(
            (got.k, got.item, got.net),
            (want.k, want.item, want.net),
            "{at}: the connection lists diverged — the port picked item {}/net {}, Java item {}/net {}",
            got.item,
            got.net,
            want.item,
            want.net
        );
        // (a).
        assert_eq!(
            got.state, want.state,
            "{at}: state {} != Java's {} (details {:?} vs {:?})",
            got.state, want.state, got.details, want.details
        );
        assert_eq!(
            got.ripped, want.ripped,
            "{at}: ripped set {:?} != Java's {:?}",
            got.ripped, want.ripped
        );
        // (b), collected rather than asserted — the caller decides.
        if let Some(diff) = first_geometry_difference(got, want) {
            geometry_diffs.push(format!("{at}: {diff}"));
        }
        // (c).
        let (Some(mine), Some(theirs)) = (got.metrics.as_ref(), want.metrics.as_ref()) else {
            assert_eq!(
                got.metrics.is_some(),
                want.metrics.is_some(),
                "{at}: one side reported metrics and the other did not"
            );
            continue;
        };
        let previous = i.checked_sub(1).and_then(|j| actual[j].metrics.as_ref());
        let expected_previous = i.checked_sub(1).and_then(|j| expected[j].metrics.as_ref());
        if let Err(why) = mine.check_spec9(previous, theirs, expected_previous) {
            panic!("{at}: {why}");
        }
    }
    Some(Ladder {
        geometry_diffs,
        connections: actual.len(),
    })
}

/// The first value of rung (b) that differs, as one line — never a whole-document dump.
fn first_geometry_difference(
    got: &RouterConnectionDoc,
    want: &RouterConnectionDoc,
) -> Option<String> {
    if got.details != want.details {
        return Some(format!(
            "details {:?} != Java's {:?}",
            got.details, want.details
        ));
    }
    if got.ripup_costs != want.ripup_costs {
        return Some(format!(
            "ripup costs {:?} != Java's {:?}",
            got.ripup_costs, want.ripup_costs
        ));
    }
    if (got.max_id_before, got.max_id_after) != (want.max_id_before, want.max_id_after) {
        return Some(format!(
            "id range {}..{} != Java's {}..{}",
            got.max_id_before, got.max_id_after, want.max_id_before, want.max_id_after
        ));
    }
    if got.traces.len() != want.traces.len() {
        return Some(format!(
            "{} inserted traces, Java {}",
            got.traces.len(),
            want.traces.len()
        ));
    }
    for (a, b) in got.traces.iter().zip(want.traces.iter()) {
        if a != b {
            if (a.id, a.layer, a.half_width) != (b.id, b.layer, b.half_width) {
                return Some(format!(
                    "trace id/layer/halfWidth ({},{},{}) != Java's ({},{},{})",
                    a.id, a.layer, a.half_width, b.id, b.layer, b.half_width
                ));
            }
            let first = a
                .corners
                .iter()
                .zip(b.corners.iter())
                .position(|(x, y)| x != y);
            return Some(match first {
                Some(i) => format!(
                    "trace {} corner {i} {} != Java's {}",
                    a.id, a.corners[i], b.corners[i]
                ),
                None => format!(
                    "trace {} has {} corners, Java's has {}",
                    a.id,
                    a.corners.len(),
                    b.corners.len()
                ),
            });
        }
    }
    if got.vias.len() != want.vias.len() {
        return Some(format!(
            "{} inserted vias, Java {}",
            got.vias.len(),
            want.vias.len()
        ));
    }
    for (a, b) in got.vias.iter().zip(want.vias.iter()) {
        if a != b {
            return Some(format!("via {a:?} != Java's {b:?}"));
        }
    }
    if got.other_inserted != want.other_inserted {
        return Some(format!(
            "{} other inserted items, Java {}",
            got.other_inserted, want.other_inserted
        ));
    }
    None
}

/// (a)+(b)+(c) on one stem, with (b) required.
fn check_all_rungs(stem: &str) -> Option<Ladder> {
    let ladder = check(stem)?;
    assert!(
        ladder.geometry_diffs.is_empty(),
        "{stem}: ruling 1(b) failed on {} connection(s):\n{}",
        ladder.geometry_diffs.len(),
        ladder.geometry_diffs.join("\n")
    );
    Some(ladder)
}

// ---------------------------------------------------------------------------------------------
// One test per stem
// ---------------------------------------------------------------------------------------------

/// The MATCH-first fixture (plan-6 ruling 11): the smallest board that actually routes. Its eight
/// connections cover `ROUTED` with vias, `NO_UNCONNECTED_NETS`, `AutorouteEngine.java:207-213`'s
/// "no connection was found" `FAILED` and `:271-277`'s "could not be inserted" `FAILED`.
#[test]
fn router_rpi_splitter() {
    let Some(ladder) = check_all_rungs("router-rpi-splitter") else {
        return;
    };
    assert_eq!(ladder.connections, 8);
}

/// Spec §14.3's smoke board, routed **whole** rather than the plan's first two connections: 294
/// connections, 242 of them `ROUTED`, and the only place in the corpus where a connection rips —
/// up to three items at once, which is what discharges the ripped-set and `removeItems` ordering
/// obligations `engine.rs` left for this task.
///
/// `#[cfg_attr(debug_assertions, ignore)]` per Plan 3's convention: the board is minutes of work
/// in a debug build and seconds in a release one.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn router_dac2020_bm01() {
    let Some(ladder) = check_all_rungs("router-dac2020-bm01") else {
        return;
    };
    assert_eq!(ladder.connections, 294);
}

/// `J2ReferenceRoutingTest.java:29`'s board, routed whole.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn router_j2_reference() {
    let Some(ladder) = check_all_rungs("router-j2-reference") else {
        return;
    };
    assert_eq!(ladder.connections, 45);
}

/// The CLI end-to-end board (spec §14.4). It routes **nothing** — see the module comment — and the
/// assertion is that the port agrees.
#[test]
fn router_tutorial_board() {
    let Some(ladder) = check_all_rungs("router-tutorial-board") else {
        return;
    };
    assert_eq!(
        ladder.connections, 0,
        "tutorial_board.dsn's 438 nets are all empty `@:no_net_N`, so nothing is routable"
    );
}

/// A board with a `(plane …)` net and a copper pour, i.e. `ConductionArea` items on the search
/// tree that every room completion has to walk past.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn router_ecc83_input() {
    let Some(ladder) = check_all_rungs("router-ecc83-input") else {
        return;
    };
    assert_eq!(ladder.connections, 22);
}

// ---------------------------------------------------------------------------------------------
// Properties of the reference set itself
// ---------------------------------------------------------------------------------------------

/// Every `router.meta.txt` must name the HEAD jar and its `2.3.1-SNAPSHOT` version: plan-6's
/// global constraints make HEAD the parity jar, and a reference regenerated against the pinned
/// 2.3.0 release would be a *different algorithm* (`autoroute/**` was refactored between them),
/// not merely an older one.
#[test]
fn references_are_from_the_head_jar() {
    for row in rows() {
        let meta_path = parity::reference(&row.stem, "router.meta.txt");
        if !parity::require_reference(&meta_path) {
            continue;
        }
        let meta = std::fs::read_to_string(&meta_path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", meta_path.display()));
        assert!(
            meta.contains("freerouting-current-executable.jar"),
            "{} does not name the HEAD jar:\n{meta}",
            meta_path.display()
        );
        assert!(
            meta.contains("2.3.1-SNAPSHOT"),
            "{} does not name a 2.3.1-SNAPSHOT jar:\n{meta}",
            meta_path.display()
        );
        assert!(
            meta.contains("-XX:hashCode=2"),
            "{} was not generated under the constant-hash mode:\n{meta}",
            meta_path.display()
        );
    }
}

/// The generator writes the connection count into `router.meta.txt`; this is what stops a
/// truncated or half-written `router.jsonl` from being committed beside a meta that describes a
/// longer run — and it is where `router-tutorial-board`'s zero is pinned.
#[test]
fn every_stem_has_the_connection_count_its_meta_records() {
    for row in rows() {
        let meta_path = parity::reference(&row.stem, "router.meta.txt");
        if !parity::require_reference(&meta_path)
            || !parity::require_reference(&reference_path(&row.stem))
        {
            continue;
        }
        let meta = std::fs::read_to_string(&meta_path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", meta_path.display()));
        let recorded: usize = meta
            .lines()
            .find_map(|line| line.strip_prefix("connections"))
            .map(|rest| rest.trim())
            .unwrap_or_else(|| panic!("{} has no `connections` line", meta_path.display()))
            .parse()
            .expect("a count");
        assert_eq!(
            read_reference(&row.stem).len(),
            recorded,
            "{}: router.jsonl and router.meta.txt disagree on the connection count",
            row.stem
        );
    }
}

/// Ruling 1's (b) rung is *required* on `router-rpi-splitter` and *reported* elsewhere. It is in
/// fact reached everywhere, so this test says so: if a future change demotes a stem to (a)+(c),
/// this is the test that fails and the README's ladder table is what has to be updated.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn geometry_is_required_where_it_was_reached() {
    for row in rows() {
        let Some(ladder) = check(&row.stem) else {
            continue;
        };
        assert!(
            ladder.geometry_diffs.is_empty(),
            "{}: ruling 1(b) was reached when the references were generated and is not now:\n{}",
            row.stem,
            ladder.geometry_diffs.join("\n")
        );
    }
}
