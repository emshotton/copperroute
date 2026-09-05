//! Only `router-dac2020-bm01` is `#[cfg_attr(debug_assertions, ignore)]` (294 connections
use std::collections::{BTreeMap, BTreeSet};

use fr_board::prelude::*;
use fr_drc::DesignRulesChecker;
use fr_dsn::java_double_to_string;
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_geometry::Point;
use fr_router::autoroute::maze::ViaPricing;
use fr_router::pipeline::RouterBudget;
use fr_router::{route_connection, route_connection_full};
use fr_settings::sources::DefaultSettings;
use fr_settings::{HostEnvironment, RouterSettings, SettingsSource};
use parity::{RouterConnectionDoc, RouterMetrics, RouterTraceDoc, RouterViaDoc};

struct Row {
    stem: String,
    dsn: String,
    max_items: usize,
    ripup_pass_no: i32,
}

fn rows() -> Vec<Row> {
    let path = parity::workspace_root().join("tests/reference/router-fixtures.txt");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| {
            let mut fields = line.split('|');
            let mut next = || fields.next().unwrap_or_default().trim().to_string();
            let (stem, dsn, max_items, ripup_pass_no) = (next(), next(), next(), next());
            if max_items == "-" {
                return None;
            }
            Some(Row {
                stem,
                dsn,
                max_items: max_items
                    .parse()
                    .unwrap_or_else(|e| panic!("max_items {max_items:?}: {e}")),
                ripup_pass_no: if ripup_pass_no.is_empty() || ripup_pass_no == "-" {
                    1
                } else {
                    ripup_pass_no
                        .parse()
                        .unwrap_or_else(|e| panic!("ripup_pass_no {ripup_pass_no:?}: {e}"))
                },
            })
        })
        .collect()
}

fn row(stem: &str) -> Row {
    rows()
        .into_iter()
        .find(|r| r.stem == stem)
        .unwrap_or_else(|| panic!("no row for {stem} in router-fixtures.txt"))
}

/// The stems whose per-stem tests carry `#[cfg_attr(debug_assertions, ignore)]`, and which
const DEBUG_IGNORED_STEMS: [&str; 2] = ["router-dac2020-bm01", "router-dac2020-bm01-pass2"];

fn reference_path(stem: &str) -> std::path::PathBuf {
    parity::reference(stem, "router.jsonl")
}

fn read_reference(stem: &str) -> Vec<RouterConnectionDoc> {
    let path = reference_path(stem);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    parity::parse_router_jsonl(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

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

fn build_settings(board: &Board) -> RouterSettings {
    let mut settings = DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    settings.set_layer_count(board.get_layer_count());
    settings.apply_board_specific_optimizations(board);
    settings
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Steps {
    OneToFive,
    OneToEight,
}

fn route_stem(row: &Row) -> Vec<RouterConnectionDoc> {
    route_stem_with(row, Steps::OneToFive)
}

fn route_stem_with(row: &Row, steps: Steps) -> Vec<RouterConnectionDoc> {
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

        board.start_marking_changed_area();
        let mut ripped: BTreeSet<ItemId> = BTreeSet::new();
        let mut ripup_costs: BTreeMap<ItemId, i32> = BTreeMap::new();
        let max_id_before = board.communication.id_gen.max_generated_id();
        let mut engine = None;
        let result = match steps {
            Steps::OneToFive => route_connection(
                &mut board,
                &mut engine,
                item_id,
                net_no,
                &settings,
                &trace_costs,
                &mut ripped,
                &mut ripup_costs,
                row.ripup_pass_no,
                settings.get_start_ripup_costs(),
                !settings.is_fanout_enabled(),
                false,
                &|| false,
            ),
            Steps::OneToEight => route_connection_full(
                &mut board,
                &mut engine,
                item_id,
                net_no,
                &settings,
                &trace_costs,
                ViaPricing::ByPadstackRadius,
                &mut ripped,
                &mut ripup_costs,
                row.ripup_pass_no,
                settings.get_start_ripup_costs(),
                !settings.is_fanout_enabled(),
                settings.trace_pull_tight_accuracy.unwrap_or(500),
                RouterBudget::disabled(),
                &|| false,
            ),
        };
        let max_id_after = board.communication.id_gen.max_generated_id();
        let (traces, vias, other_inserted) = inserted_geometry(&board, max_id_before);
        let metrics = metrics(&mut board, net_no);

        out.push(RouterConnectionDoc {
            k,
            item: i64::from(item_id.0),
            net: net_no,
            state: result.state.name().to_string(),
            details: result.details.clone().unwrap_or_default(),
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

fn metrics(board: &mut Board, net_no: i32) -> RouterMetrics {
    let vias = board.net_via_count(net_no) as i64;
    let trace_length = java_double_to_string(board.cumulative_trace_length());
    let (incompletes, violations) = {
        let mut drc = DesignRulesChecker::new(board);
        (drc.get_incomplete_count(), drc.get_all_violations())
    };
    let violations = violations
        .iter()
        .filter(|violation| violation.involves_routing(board))
        .count() as i64;
    RouterMetrics {
        incompletes: incompletes as i64,
        vias,
        trace_length,
        violations,
    }
}

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

struct Ladder {
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
    if parity::regolden_label().is_some() {
        parity::write_router_jsonl(&reference_path(stem), &actual);
        return None;
    }

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
        assert_eq!(
            (got.k, got.item, got.net),
            (want.k, want.item, want.net),
            "{at}: the connection lists diverged — the port picked item {}/net {}, Java item {}/net {}",
            got.item,
            got.net,
            want.item,
            want.net
        );
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
        if let Some(diff) = first_geometry_difference(got, want) {
            geometry_diffs.push(format!("{at}: {diff}"));
        }
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

#[test]
fn router_rpi_splitter() {
    let Some(ladder) = check_all_rungs("router-rpi-splitter") else {
        return;
    };
    assert_eq!(ladder.connections, 8);
}

/// `#[cfg_attr(debug_assertions, ignore)]` per Plan 3's convention: the board is minutes of work
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn router_dac2020_bm01() {
    let Some(ladder) = check_all_rungs("router-dac2020-bm01") else {
        return;
    };
    assert_eq!(ladder.connections, 294);
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn router_dac2020_bm01_pass2() {
    let Some(ladder) = check_all_rungs("router-dac2020-bm01-pass2") else {
        return;
    };
    assert_eq!(ladder.connections, 294);
}

/// **Deliberately not `#[cfg_attr(debug_assertions, ignore)]`.** This is the regression test for
#[test]
fn router_j2_reference() {
    let Some(ladder) = check_all_rungs("router-j2-reference") else {
        return;
    };
    assert_eq!(ladder.connections, 45);
}

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

#[test]
fn router_ecc83_input() {
    let Some(ladder) = check_all_rungs("router-ecc83-input") else {
        return;
    };
    assert_eq!(ladder.connections, 22);
}

#[test]
fn references_are_from_the_head_jar() {
    for row in rows() {
        let meta_path = parity::reference(&row.stem, "router.meta.txt");
        if !parity::require_reference(&meta_path) {
            continue;
        }
        let meta = std::fs::read_to_string(&meta_path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", meta_path.display()));
        if parity::declared_lane(&meta).starts_with("port") {
            parity::assert_port_lane_provenance(&meta, &row.stem);
            continue;
        }
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

#[test]
fn geometry_is_required_where_it_was_reached() {
    for row in rows() {
        if cfg!(debug_assertions) && DEBUG_IGNORED_STEMS.contains(&row.stem.as_str()) {
            continue;
        }
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

fn steps18_reference_path(stem: &str) -> std::path::PathBuf {
    parity::reference(stem, "router-steps18.jsonl")
}

fn read_steps18_reference(stem: &str) -> Vec<RouterConnectionDoc> {
    let path = steps18_reference_path(stem);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    parity::parse_router_jsonl(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn steps18_pair(stem: &str) -> Option<(Vec<RouterConnectionDoc>, Vec<RouterConnectionDoc>)> {
    if !parity::require_java_dir() || !parity::require_reference(&steps18_reference_path(stem)) {
        return None;
    }
    let expected = read_steps18_reference(stem);
    let actual = route_stem_with(&row(stem), Steps::OneToEight);
    if parity::regolden_label().is_some() {
        parity::write_router_jsonl(&steps18_reference_path(stem), &actual);
        return None;
    }
    assert_eq!(
        actual.len(),
        expected.len(),
        "{stem}: the port found {} connections, the reference has {}",
        actual.len(),
        expected.len()
    );
    Some((actual, expected))
}

fn assert_steps18_matches(stem: &str) {
    let Some((actual, expected)) = steps18_pair(stem) else {
        return;
    };
    for (i, (got, want)) in actual.iter().zip(expected.iter()).enumerate() {
        assert_eq!(
            got,
            want,
            "{stem} k={} diverged under --steps=1-8:\n  port: {got:?}\n  java: {want:?}",
            i + 1
        );
    }
}

#[test]
fn steps_one_to_eight_matches_the_jar() {
    for stem in [
        "router-rpi-splitter",
        "router-j2-reference",
        "router-ecc83-input",
        "router-tutorial-board",
    ] {
        assert_steps18_matches(stem);
    }
}

#[cfg_attr(debug_assertions, ignore)]
#[test]
fn steps_one_to_eight_on_dac2020_at_pass_two_matches_the_jar() {
    assert_steps18_matches("router-dac2020-bm01-pass2");
}

#[cfg_attr(debug_assertions, ignore)]
#[test]
fn steps_one_to_eight_on_dac2020_matches_the_jar() {
    let Some((actual, expected)) = steps18_pair("router-dac2020-bm01") else {
        return;
    };
    assert_eq!(294, expected.len());
    for (i, (got, want)) in actual.iter().zip(expected.iter()).enumerate() {
        let k = i + 1;
        assert_eq!(
            got,
            want,
            "router-dac2020-bm01 k={k} diverged under --steps=1-8{}:\n  port: {got:?}\n  java: \
             {want:?}",
            if k == 175 {
                " — this is quirk #210's connection: check that \
                 fr_router::board_ext::tightener::scan_contacts still walks its contacts .rev()ed"
            } else {
                ""
            }
        );
    }
}
