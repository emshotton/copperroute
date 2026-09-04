//! Plan 7 Task 6: `board.optimize.ViaOptimizer`'s `optViaLocation` (ViaOptimizer.java:33-158),
//! `optPlaneOrFanoutVia` (:161-296) and `isWithinTolerance` (:719-732) — the via half of the
//! pull-tight sweep `TraceTightener.optChangedArea:160-165` runs on every routed connection.
//!
//! # Where the numbers come from
//!
//! `scripts/differential/java/P7T4.java` runs the two methods over every via of a board the
//! `P6T1` machinery actually routed, printing per via the contact ids (descending), the dispatch
//! class `:39-106` computes, the returned boolean and the centre before and after, then the whole
//! board. Its stdout for three fixtures × modes 0, 1 and 3, plus a subset of mode 2's scripted
//! `isWithinTolerance` stream, is committed as `tests/data/p7t4-via-optimizer.txt` (the `HEADER`
//! line, which names the jar by absolute path, is stripped) and diffed live by
//! `scripts/differential/run.sh p7t4`.
//!
//! # What is pinned and what is not, and why
//!
//! **Plan 7 Task 7 closed the last gap.** Task 6 landed the two entry points with the three
//! `repositionVia` overloads stubbed — overload C as an inert `None`, overload A as a loud
//! `unimplemented!` (controller ruling B1) — and this file recorded the measured divergence. Task 7
//! ported all three, and **every section of the transcript now matches the JVM row for row**:
//! [`the_matching_runs_match_the_jvm_row_for_row`] walks all nine board sections, and
//! [`the_only_divergence_is_repositionvia`] — the test that used to name eight divergent via ids by
//! hand — now asserts those same eight rows are **identical**. The `TASK7_GUARD` rows and
//! `ViaOptimizer::reaches_task_seven_guard` are gone from both halves of the driver.
//!
//! The three overloads themselves are pinned by `via_optimizer_reposition.rs`, against `p7t4`
//! modes 3, 4 and 5 — one per overload, each driven directly.
//!
//! The rest of the file isolates single branches on hand-built input, because a real board
//! exercises them all at once and could not say which one fired.

use fr_board::items::Item;
use fr_board::prelude::*;
use fr_dsn::java_double_to_string;
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_geometry::{IntBox, IntPoint, Line, Point, Polyline};
use fr_router::board_ext::ViaOptimizer;
use fr_router::route_connection;
use fr_settings::sources::DefaultSettings;
use fr_settings::{ExpansionCostFactor, HostEnvironment, RouterSettings, SettingsSource};
use std::collections::{BTreeMap, BTreeSet};

fn never() -> bool {
    false
}

// =================================================================================================
// `isWithinTolerance` (:719-732)
// =================================================================================================

/// The `bnd` family of `P7T4` mode 2: for each `tolerance`, three points whose Manhattan distance
/// from `p1` is `tolerance - 1`, `tolerance` and `tolerance + 1`. Java's operator is `<=`, so the
/// middle one is **inside**.
#[test]
fn is_within_tolerance_matches_java_at_the_boundary() {
    let mut checked = 0;
    for row in transcript_section("tol") {
        let Some(rest) = row.strip_prefix("bnd ") else {
            continue;
        };
        let (p1, p2, tolerance, expected) = parse_tolerance_row(rest);
        assert_eq!(
            ViaOptimizer::is_within_tolerance(Some(&p1), &p2, tolerance),
            expected,
            "isWithinTolerance{p1:?} {p2:?} {tolerance}"
        );
        checked += 1;
    }
    assert_eq!(checked, 768, "the boundary family is 256 triples × 3");
}

/// The scripted stream's `tol` family, which reaches negative tolerances (`% 121 - 20`) and
/// distances far outside them.
#[test]
fn is_within_tolerance_matches_java_over_the_scripted_stream() {
    let mut checked = 0;
    for row in transcript_section("tol") {
        let Some(rest) = row.strip_prefix("tol ") else {
            continue;
        };
        let (p1, p2, tolerance, expected) = parse_tolerance_row(rest);
        assert_eq!(
            ViaOptimizer::is_within_tolerance(Some(&p1), &p2, tolerance),
            expected,
            "isWithinTolerance{p1:?} {p2:?} {tolerance}"
        );
        checked += 1;
    }
    assert!(checked >= 100, "the scripted subset is not empty");
}

/// `:720-722` — Java's `p1 == null || p2 == null` guard. The port keeps it as an `Option` on the
/// first argument because every call site feeds `PolylineTrace.firstCorner()` / `lastCorner()`,
/// whose port counterparts answer `Option<Point>`; a trace with no corners is the port's `null`.
#[test]
fn is_within_tolerance_answers_false_for_a_missing_corner() {
    assert!(!ViaOptimizer::is_within_tolerance(
        None,
        &Point::new(0, 0),
        1_000_000
    ));
}

fn parse_tolerance_row(rest: &str) -> (Point, Point, i32, bool) {
    // `i=0 p1=(-1,2) p2=(3,4) t=5 -> true`, or `i=0 d=-1 p1=…` for the `bnd` family.
    let mut p1 = None;
    let mut p2 = None;
    let mut tolerance = None;
    let mut expected = None;
    let mut fields = rest.split_whitespace().peekable();
    while let Some(field) = fields.next() {
        if let Some(v) = field.strip_prefix("p1=") {
            p1 = Some(parse_point(v));
        } else if let Some(v) = field.strip_prefix("p2=") {
            p2 = Some(parse_point(v));
        } else if let Some(v) = field.strip_prefix("t=") {
            tolerance = Some(v.parse::<i32>().expect("an int tolerance"));
        } else if field == "->" {
            expected = Some(fields.next().expect("an answer") == "true");
        }
    }
    (
        p1.expect("p1"),
        p2.expect("p2"),
        tolerance.expect("t"),
        expected.expect("->"),
    )
}

fn parse_point(text: &str) -> Point {
    let inner = text
        .trim_start_matches('(')
        .trim_end_matches(')')
        .split_once(',')
        .expect("(x,y)");
    Point::Int(IntPoint::new(
        inner.0.parse().expect("x"),
        inner.1.parse().expect("y"),
    ))
}

// =================================================================================================
// The `p7t4` transcript — the two methods over three real routed boards
// =================================================================================================

const TRANSCRIPT: &str = include_str!("data/p7t4-via-optimizer.txt");

fn transcript_section(name: &str) -> Vec<&'static str> {
    let header = format!("######## {name}");
    let mut rows = Vec::new();
    let mut inside = false;
    for line in TRANSCRIPT.lines() {
        if line.starts_with("######## ") {
            inside = line == header;
            continue;
        }
        if inside {
            rows.push(line.trim_end());
        }
    }
    assert!(!rows.is_empty(), "transcript section {name} is empty");
    rows
}

/// The fixture behind each transcript tag.
fn fixture_of(tag: &str) -> &'static str {
    match tag {
        "rpi" => "Issue143-rpi_splitter.dsn",
        "j2" => "Issue026-J2_reference.dsn",
        "ecc83" => "Issue649-kicad_ecc83-pp_input_board_v1.dsn",
        other => panic!("unknown transcript tag {other}"),
    }
}

/// `P7T4`'s run with `accuracy = 500` and `routeK = 12`, replayed.
fn p7t4_rows(tag: &str, mode: i32) -> Vec<String> {
    let mut out = Vec::new();
    let design_name = fixture_of(tag);
    let path = parity::fixture(design_name);
    let file = std::fs::File::open(&path)
        .unwrap_or_else(|e| panic!("cannot open {}: {e}", path.display()));
    let mut board =
        match fr_dsn::read_board(file, None, Some(design_name), &DsnReadOptions::default()) {
            BoardReadResult::Success { board, .. }
            | BoardReadResult::OutlineMissing { board, .. } => {
                *board.expect("the fixture produces a board")
            }
            other => panic!("{design_name} did not read: {other:?}"),
        };
    let settings = build_settings(&board);

    for (k, (item_id, net_no)) in pick_connections(&board, 12).into_iter().enumerate() {
        let k = k + 1;
        if board.get_item(item_id).is_none() {
            out.push(format!("route k={k} item={} state=GONE", item_id.0));
            continue;
        }
        board.start_marking_changed_area();
        let mut ripped: BTreeSet<ItemId> = BTreeSet::new();
        let mut ripup_costs: BTreeMap<ItemId, i32> = BTreeMap::new();
        let trace_costs = settings.get_trace_costs();
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
            &never,
        );
        out.push(format!(
            "route k={k} item={} net={net_no} state={} ripped={}",
            item_id.0,
            result.state.name(),
            ripped.len()
        ));
    }

    let trace_costs: Option<Vec<ExpansionCostFactor>> = if mode == 6 {
        None
    } else {
        Some(vec![
            ExpansionCostFactor {
                horizontal: 1.0,
                vertical: 1.0,
            };
            board.get_layer_count()
        ])
    };
    out.push(format!(
        "sweep regime={} traceCosts={}",
        regime_name(board.rules.trace_angle_restriction),
        match &trace_costs {
            Some(costs) => costs.len().to_string(),
            None => "null".to_string(),
        }
    ));

    let via_ids: Vec<ItemId> = board
        .get_items()
        .filter(|item| matches!(item, Item::Via(_)))
        .map(Item::id)
        .collect();
    out.push(format!("vias n={}", via_ids.len()));

    for via_id in via_ids {
        if !matches!(board.get_item(via_id), Some(Item::Via(_))) {
            out.push(format!("via id={} state=GONE", via_id.0));
            continue;
        }
        let center = board.drill_center(via_id).expect("a via has a centre");
        let min_width = match board.get_item(via_id) {
            Some(Item::Via(v)) => v.min_width(&board.ctx()),
            _ => unreachable!("just matched"),
        };
        let contacts = contact_ids(&board, via_id);
        let class = classify(&board, via_id);
        let result = if mode == 1 {
            ViaOptimizer::opt_plane_or_fanout_via(&mut board, via_id, 500, 10)
        } else {
            ViaOptimizer::opt_via_location(&mut board, via_id, trace_costs.as_deref(), 500, 10)
        }
        .expect("ViaOptimizer cannot fail in Java");
        let after = match board.drill_center(via_id) {
            Some(point) if matches!(board.get_item(via_id), Some(Item::Via(_))) => {
                dump_point(&point)
            }
            _ => "gone".to_string(),
        };
        out.push(format!(
            "via id={} center={} minWidth={} contacts={contacts} class={class} result={result} after={after}",
            via_id.0,
            dump_point(&center),
            java_double_to_string(min_width),
        ));
    }

    out.extend(dump_board(&board));
    out
}

// =================================================================================================
// RETIREMENT RECORD — the `Issue026-J2_reference` arm of the p7t4 family (ruling CC)
// =================================================================================================
//
// **What retired.** The three `j2` sections of `p7t4` — modes 0, 1 and 6 — and with them
// `the_normal_contacts_are_visited_descending`, which was `j2`-only, and the `j2` arm of
// `the_matching_runs_match_the_jvm_row_for_row`, `the_only_divergence_is_repositionvia` and
// `the_overload_dispatch_matches_javas_contact_counts`. `via_optimizer_reposition.rs` carries the
// same record for the `j2` arm of the three overload tests and the two `j2`-only tests there.
// Nothing is silently deleted; this block is the record ruling CC requires.
//
// **The last matching run.** `cf4c6f3`, the commit immediately before Task 8's merge `57055c1`.
// Re-run at the accept wave in a worktree at that commit: `cargo nextest run -p fr-router
// --test via_optimizer --test via_optimizer_reposition` — **18 passed, 0 failed**.
//
// **The diverging cause.** `tests/data/p7t4-via-optimizer.txt` is a **jar** transcript over a board
// the **port** routes: `routed_j2()` calls `route_connection` twelve times before the first via is
// examined. Task 8's fixes change what those twelve calls produce. Measured at the wave, section by
// section, against the whole transcript:
//
// | fixture | rows | rows differing | differing only in board item ids |
// |---|---|---|---|
// | `ecc83` modes 0/1/6 | 379 each | **0** | — |
// | `rpi` modes 0/1/6 | 68 each | 7 each | **all 7** |
// | `j2` modes 0/1/6 | 124/123/124 | 22/16/22 | 18/13/18 |
//
// So `ecc83` is byte-identical and `rpi`'s divergence is identity only. `j2`'s is not: one net-10
// trace carries corners `(1178685,-829062) (1228467,-829062)` in the port's board where the jar's
// carries `(1178686,-829063) (1228466,-829063)`, one integer unit apart on two corners, and its
// `lines=` list differs with it. **No renaming of ids and no offset can reconcile that**, which is
// exactly ruling CC's argument for this family. The remaining `j2` rows differ in contact ids that
// a renaming does cover, and they retire with the section they are in rather than being kept
// alone.
//
// **The diverging cause rows.** #163, #171 and #165b (the door-set fixes) and #156/#167/#158 (one
// shared id counter), all `fixed: T8`.
//
// **The authorizing rows.** Ruling CC(b) — the plan9-t7t8 bench adjudicated Task 8's BP12
// escalation as variance and opened this accept wave; Task 8 §6a, which raised the family as a
// design question rather than a re-cut; Task 8 §6's note that inventing a port lane for the
// unit-level jar transcripts is a controller ruling and not a task decision.
//
// **What survives, and what is really lost.** Six of the nine board sections are still compared
// with the jar — `ecc83`'s three byte for byte, `rpi`'s three under one declared renaming of board
// item ids, described at `assert_rows_match_up_to_one_renaming`. The dispatch replica still checks
// every `ecc83` and `rpi` via's class and contact walk against the jar's own classification. What
// is lost is `j2`'s ten `TWO_TRACES` vias: the descending contact walk of `:46` now has no
// two-contact producer compared with the jar, and overload C's cost gates have none either. That
// is the cost of this retirement, and it is reported to the controller as such rather than left
// implicit in a row count.

/// **Board item ids are no longer comparable on two of the three fixtures; everything else is.**
/// `accepted at plan9-t7t8 (ruling CC)` — see the retirement record above for the measurement.
///
/// `ecc83`'s sections are byte-identical to the jar. `rpi`'s differ only in board item ids, so its
/// rows are compared with the ids **blanked** — exact on every state, coordinate, net, clearance,
/// layer, half width, corner list and result — and the ids themselves are then required to be
/// **one consistent renaming**: the jar's id `a` must map to the same port id everywhere in the
/// section, and no two jar ids may map to one port id.
///
/// That is stronger than a pasted table and much stronger than ignoring the ids:
///
/// * any byte outside an id token moving fails, in either direction;
/// * a row appearing, vanishing or changing order fails, because the blanked comparison is a
///   whole-sequence equality;
/// * the same jar id resolving to two different port ids fails, and two jar ids collapsing onto
///   one port id fails — so a via genuinely swapping places with another is caught even though its
///   number is not pinned.
const ID_KEYS: [&str; 4] = ["via id=", "item id=", "maxId=", "contacts=["];

/// Splits `line` into its id values, in order, and the line with each of them replaced by `#`.
///
/// An id is a digit run introduced by one of [`ID_KEYS`]; inside a `contacts=[a:Type,b:Type]` list
/// the scan continues over the commas, so both contacts are ids. Coordinates, cost factors, half
/// widths, clearance classes and layer numbers are never introduced by a key and are left alone.
fn split_ids(line: &str) -> (Vec<i64>, String) {
    let mut ids = Vec::new();
    let mut blanked = String::with_capacity(line.len());
    let mut rest = line;
    loop {
        let Some((at, key_len)) = ID_KEYS
            .iter()
            .filter_map(|key| rest.find(key).map(|at| (at, key.len())))
            .min_by_key(|(at, _)| *at)
        else {
            break;
        };
        let head = at + key_len;
        blanked.push_str(&rest[..head]);
        rest = &rest[head..];
        loop {
            let digits = rest
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(rest.len());
            let Ok(id) = rest[..digits].parse::<i64>() else {
                break;
            };
            ids.push(id);
            blanked.push('#');
            rest = &rest[digits..];
            let Some(comma) = rest.find(',') else { break };
            if rest[..comma].contains(']') || rest[..comma].contains(' ') {
                break;
            }
            blanked.push_str(&rest[..=comma]);
            rest = &rest[comma + 1..];
        }
    }
    blanked.push_str(rest);
    (ids, blanked)
}

/// The comparison described on [`ID_KEYS`]: every row equal once ids are blanked, and the ids
/// themselves one consistent renaming.
fn assert_rows_match_up_to_one_renaming(label: &str, ours: &[String], theirs: &[&str]) {
    let (our_ids, our_blanked): (Vec<Vec<i64>>, Vec<String>) =
        ours.iter().map(|row| split_ids(row)).unzip();
    let (their_ids, their_blanked): (Vec<Vec<i64>>, Vec<String>) =
        theirs.iter().map(|row| split_ids(row)).unzip();
    assert_eq!(
        our_blanked, their_blanked,
        "{label}: a difference outside the board item ids"
    );

    let mut forward: BTreeMap<i64, i64> = BTreeMap::new();
    let mut backward: BTreeMap<i64, i64> = BTreeMap::new();
    for (row, (theirs_row, ours_row)) in their_ids.iter().zip(&our_ids).enumerate() {
        assert_eq!(theirs_row.len(), ours_row.len(), "{label} row {row}");
        for (&jar, &port) in theirs_row.iter().zip(ours_row) {
            if let Some(&seen) = forward.get(&jar) {
                assert_eq!(
                    seen, port,
                    "{label} row {row}: the jar's id {jar} is the port's {seen} elsewhere and \
                     {port} here — not one renaming"
                );
            }
            if let Some(&seen) = backward.get(&port) {
                assert_eq!(
                    seen, jar,
                    "{label} row {row}: the port's id {port} stands for the jar's {seen} \
                     elsewhere and {jar} here — not one renaming"
                );
            }
            forward.insert(jar, port);
            backward.insert(port, jar);
        }
    }
    assert!(
        !forward.is_empty(),
        "{label}: no id was compared, so the renaming check did nothing"
    );
}

/// `split_ids` is load-bearing enough to be tested rather than trusted: a bug in it would let a
/// real difference through as "an id".
#[test]
fn the_id_split_finds_ids_and_nothing_else() {
    assert_eq!(split_ids("maxId=213"), (vec![213], "maxId=#".to_string()));
    assert_eq!(
        split_ids("via id=264 center=(1328495,-867516) contacts=[266:PolylineTrace,263:PolylineTrace] class=TWO_TRACES"),
        (
            vec![264, 266, 263],
            "via id=# center=(1328495,-867516) contacts=[#:PolylineTrace,#:PolylineTrace] class=TWO_TRACES".to_string()
        )
    );
    // Coordinates, nets, clearances, half widths and corner lists are NOT ids.
    let untouched = "route k=1 item=23 net=3 state=ROUTED ripped=0";
    assert_eq!(
        split_ids(untouched),
        (Vec::new(), untouched.to_string()),
        "`item=` is not `item id=`"
    );
}

/// **Every** board section of the transcript the two programs can still be compared on: `ecc83`'s
/// three modes byte for byte, `rpi`'s three under one renaming of board item ids. `j2`'s three
/// retired at the plan9-t7t8 accept wave — the record above says why and what was measured.
#[test]
fn the_matching_runs_match_the_jvm_row_for_row() {
    for (tag, mode) in [("ecc83", 0), ("ecc83", 1), ("ecc83", 6)] {
        assert_eq!(
            p7t4_rows(tag, mode),
            transcript_section(&format!("{tag} mode {mode}")),
            "p7t4 {tag} mode {mode}: still byte for byte"
        );
    }
    for (tag, mode) in [("rpi", 0), ("rpi", 1), ("rpi", 6)] {
        assert_rows_match_up_to_one_renaming(
            &format!("p7t4 {tag} mode {mode}"),
            &p7t4_rows(tag, mode),
            &transcript_section(&format!("{tag} mode {mode}")),
        );
    }
}

/// The eight via ids whose rows the port could **not** reproduce before Plan 7 Task 7 landed the
/// three `repositionVia` overloads, per `(tag, mode)`. Task 6 wrote these lists by hand as its
/// honest form of "0 diffs"; Task 7 turned them from a divergence list into an **equality pin** —
/// [`the_only_divergence_is_repositionvia`] now asserts each of these rows is identical to the
/// JVM's, character for character.
///
/// * `rpi`'s 187 and 84 are the two `PLANE_OR_FANOUT_ONE_CONTACT` vias, which Java moves through
///   **overload A** (`optPlaneOrFanoutVia:216-217`). Task 6 printed a `TASK7_GUARD` row for them on
///   *both* sides rather than record a port-only move; those rows are gone and both sides now print
///   the move.
/// * `j2`'s six are `TWO_TRACES` vias, of which Java moves four through **overload C**
///   (`:434-713`); the moves then change the contact ids of the two it does not.
///
/// **`j2`'s six retired at the plan9-t7t8 accept wave (ruling CC)** with the rest of that
/// fixture's arm; they were `264, 231, 200, 189, 130, 124` and the record above carries the cause.
/// `rpi`'s two are the JAR's ids and are kept as the jar's — the port calls the first of them
/// `189` — because the renaming is what the comparison resolves.
fn formerly_divergent_vias(tag: &str, mode: i32) -> &'static [u32] {
    match (tag, mode) {
        ("rpi", 0) | ("rpi", 6) => &[187, 84],
        _ => &[],
    }
}

/// **The Task 7 sentinel, flipped.** Task 6 left this test naming eight via ids the port could not
/// reproduce and asserting the divergence went no further; it is now an equality pin — for each of
/// the four `(fixture, mode)` pairs that used to diverge, the port's whole transcript equals the
/// JVM's, and each of the eight named rows is checked again by id so a regression names the via
/// rather than dumping the run.
#[test]
fn the_only_divergence_is_repositionvia() {
    for (tag, mode) in [("rpi", 0), ("rpi", 6), ("ecc83", 0)] {
        let ours = p7t4_rows(tag, mode);
        let theirs = transcript_section(&format!("{tag} mode {mode}"));
        assert_rows_match_up_to_one_renaming(&format!("p7t4 {tag} mode {mode}"), &ours, &theirs);

        let our_vias: Vec<&String> = ours.iter().filter(|r| r.starts_with("via ")).collect();
        let their_vias: Vec<&&str> = theirs.iter().filter(|r| r.starts_with("via ")).collect();
        assert_eq!(
            our_vias.len(),
            their_vias.len(),
            "{tag} mode {mode}: via row count"
        );
        let mut checked = Vec::new();
        for (ours_row, theirs_row) in our_vias.iter().zip(&their_vias) {
            let id = via_id_of(theirs_row);
            if !formerly_divergent_vias(tag, mode).contains(&id) {
                continue;
            }
            checked.push(id);
            // The columns Task 6 could only compare on the first divergent row: `class=`,
            // `contacts=` and the centre *before* the call all agreed even then. `result=` and
            // `after=` are the two Task 7 closed. The row equality below is the id-blind one,
            // because the ids themselves are resolved by the renaming check above.
            assert_eq!(
                split_ids(ours_row).1,
                split_ids(theirs_row).1,
                "{tag} mode {mode}: via {id} still diverges"
            );
        }
        assert_eq!(
            checked,
            formerly_divergent_vias(tag, mode),
            "{tag} mode {mode}: every formerly divergent via is still in the run"
        );
    }
}

fn via_id_of(row: &str) -> u32 {
    column(row, "id=")
        .parse()
        .unwrap_or_else(|_| panic!("an id in {row}"))
}

fn column<'a>(row: &'a str, key: &str) -> &'a str {
    let start = row.find(key).unwrap_or_else(|| panic!("{key} in {row}")) + key.len();
    let rest = &row[start..];
    match rest.find(' ') {
        Some(end) => &rest[..end],
        None => rest,
    }
}

// =================================================================================================
// Single branches, on hand-built input
// =================================================================================================

const BOUNDING_BOX: IntBox = IntBox {
    ll: IntPoint {
        x: -100_000,
        y: -100_000,
    },
    ur: IntPoint {
        x: 100_000,
        y: 100_000,
    },
};

fn layers() -> LayerStructure {
    LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)])
}

fn empty_board() -> Board {
    let clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 100);
    let mut rules = BoardRules::new(layers(), clearance_matrix);
    rules.trace_angle_restriction = AngleRestriction::None;
    let mut board = Board::new(
        Vec::new(),
        0,
        BOUNDING_BOX,
        rules,
        BoardLibrary::new(Padstacks::new(layers()), Packages::new()),
        Components::new(),
        Communication::default(),
    );
    let default_class = board.rules.get_default_net_class();
    board.rules.nets.add("N1", 1, false, default_class);
    board
}

/// `:39-41` — a shove-fixed via is refused before anything else happens, and `:42-45` — a
/// recursion budget of zero is refused before the contacts are even read. Both are pure guards, so
/// a board with no via at all is enough to prove the id-shaped ones; the interesting half is that
/// **neither leaves a trace on the board**.
#[test]
fn the_two_guards_refuse_before_touching_the_board() {
    let mut board = empty_board();
    let before = board.structural_hash();
    assert!(
        !ViaOptimizer::opt_via_location(&mut board, ItemId(9999), None, 500, 10)
            .expect("cannot fail"),
        "an id naming nothing is Java's impossible `Via` argument"
    );
    assert!(
        !ViaOptimizer::opt_via_location(&mut board, ItemId(9999), None, 500, 0)
            .expect("cannot fail"),
        ":42-45, maxRecursionDepth <= 0"
    );
    assert!(
        !ViaOptimizer::opt_plane_or_fanout_via(&mut board, ItemId(9999), 500, 0)
            .expect("cannot fail"),
        ":163-166, maxRecursionDepth <= 0"
    );
    assert_eq!(before, board.structural_hash(), "the board was not touched");
}

/// The refusal path is the common one on a real board — `:132-134` for `optViaLocation` and
/// `:261-263` for `optPlaneOrFanoutVia` — and it must leave the board exactly as it was.
///
/// Before Task 7 the port refused **every** via of `Issue026-J2_reference` (overload C's stub
/// declined) and this test could simply assert "nothing moved". Now four of the six move, so the
/// statement is the sharper one: `Ok(false)` implies a byte-identical board, and `Ok(true)` implies
/// a changed one. A refusal that quietly mutated, or a move that quietly did not, fails here.
#[test]
fn a_refused_move_leaves_the_board_byte_identical() {
    let mut board = routed_j2();
    let via_ids: Vec<ItemId> = board
        .get_items()
        .filter(|item| matches!(item, Item::Via(_)))
        .map(Item::id)
        .collect();
    assert!(!via_ids.is_empty(), "the routed prefix places vias");
    let mut refused = 0;
    let mut moved_count = 0;
    for via_id in via_ids {
        let before = board.structural_hash();
        let moved =
            ViaOptimizer::opt_via_location(&mut board, via_id, None, 500, 10).expect("cannot fail");
        let after = board.structural_hash();
        if moved {
            moved_count += 1;
            assert_ne!(
                before, after,
                "via {} answered true and did not move",
                via_id.0
            );
        } else {
            refused += 1;
            assert_eq!(
                before, after,
                "a refused move changed the board at via {}",
                via_id.0
            );
        }
    }
    assert!(refused > 0, "the fixture still exercises the refusal path");
    assert!(
        moved_count > 0,
        "and the move path, which is what Task 7 opened"
    );
}

/// `:46-78`, the dispatch, and `:46`'s descending contact walk — pinned against the JVM's own
/// classification for every via of the **two** fixtures the two programs still route the same way.
/// `P7T4.classify` is the same twenty lines on the Java side, computed *before* the call, so this
/// compares two independent transcriptions of `:39-106` rather than the port against itself.
///
/// Task 6 could only compare the first via of each run: from the second on, a via Java had moved and
/// the port had not made the two sides classify different boards. Task 7 removed that limit.
///
/// **`j2` retired at the plan9-t7t8 accept wave (ruling CC)** — the record at the top of this file
/// carries the cause. The `TWO_TRACES` count below falls from **ten to four**: six of the ten
/// two-contact vias the walk classified were `j2`'s. `rpi`'s four remain, so the `:118` arm is
/// still compared with the jar here; what has no jar comparison left is overload C's cost gates,
/// which only `j2`'s mode-5 section supplied. The count is asserted rather than dropped, so the
/// loss is a number a reader can see.
#[test]
fn the_overload_dispatch_matches_javas_contact_counts() {
    let mut seen = BTreeMap::new();
    for (tag, mode) in [("rpi", 0), ("ecc83", 0)] {
        let ours = p7t4_rows(tag, mode);
        let theirs = transcript_section(&format!("{tag} mode {mode}"));
        let ours_vias = ours.iter().filter(|r| r.starts_with("via "));
        let theirs_vias = theirs.iter().filter(|r| r.starts_with("via "));
        let mut rows = 0;
        for (ours_row, theirs_row) in ours_vias.zip(theirs_vias) {
            // The ids themselves are resolved by `assert_rows_match_up_to_one_renaming`, which the
            // row-for-row test runs over these same sections; what the walk order needs here is
            // that the two sides pair the same vias, and the blanked rows say that.
            assert_eq!(
                split_ids(ours_row).1,
                split_ids(theirs_row).1,
                "{tag}: via order"
            );
            assert_eq!(
                column(ours_row, "class="),
                column(theirs_row, "class="),
                "{tag}: via {}'s dispatch class",
                column(ours_row, "id=")
            );
            // The contact LIST is compared by the blanked whole-row equality above, which covers
            // its arity and its element types; the ids inside it are resolved by the renaming
            // check `the_matching_runs_match_the_jvm_row_for_row` runs over these same sections.
            // What is left to say here is the property `:46` exists for and a renaming cannot
            // carry: the walk is DESCENDING.
            let ours_contacts: Vec<i64> = split_ids(ours_row).0;
            let contacts = &ours_contacts[1..];
            assert!(
                contacts.windows(2).all(|w| w[0] > w[1]),
                "{tag}: via {}'s contacts are not descending: {contacts:?}",
                column(theirs_row, "id=")
            );
            *seen
                .entry(column(theirs_row, "class=").to_string())
                .or_insert(0) += 1;
            rows += 1;
        }
        assert_eq!(
            rows,
            theirs.iter().filter(|r| r.starts_with("via ")).count(),
            "{tag}: every via row was compared"
        );
    }
    assert_eq!(
        seen.get("PLANE_OR_FANOUT_ONE_CONTACT").copied(),
        Some(2),
        "`rpi` reaches the one-contact arm at :47 twice"
    );
    assert_eq!(
        seen.get("TWO_TRACES").copied(),
        Some(4),
        "`rpi` reaches the two-trace arm at :118 four times. It was TEN across `rpi` and `j2` \
         until the plan9-t7t8 accept wave retired the `j2` arm — see the retirement record. The \
         number is asserted rather than the retirement quietly absorbed."
    );
}

/// `:47` and `:76-78` — a via with exactly one contact never reads `firstTrace`/`secondTrace` and
/// goes straight to the plane/fanout arm, where `:216-217` is `repositionVia` overload A.
///
/// **The Task 7 flip.** Task 6 named this `a_plane_via_reaches_task_sevens_guard` and asserted the
/// two one-contact vias *panicked* out of both entry points with a message naming overload A
/// (controller ruling B1: a `None` there would have fallen into the `:218-260` projection branch,
/// which inserts, and put via 84 at `(1016000,2968339)` — a board Java never produces). Overload A
/// is ported now, so the assertion is the real one: **the centres Java chooses.**
///
/// * via 187 moves to `(932812,1011224)` — overload A's answer, taken directly;
/// * via 84 moves to `(1016000,3119161)`. Overload A answers `(1016000,3007058)` there, which
///   *is* the check corner, so `:292-294`'s `newViaLocation.equals(checkCorner)` fires and
///   `optPlaneOrFanoutVia` recurses once more (Task 6 report N3). The two numbers are one call
///   apart, and this test pins the outer one;
/// * the four two-trace vias answer `Ok(false)` from `opt_plane_or_fanout_via` at `:188-190` (a
///   second trace contact is already recorded) and leave the board untouched.
///
/// Both entry points are driven, because it is `:76-78`'s dispatch that sends a one-contact via
/// into the plane arm at all: `opt_via_location` and `opt_plane_or_fanout_via` must agree.
#[test]
fn a_plane_via_moves_through_overload_a() {
    let board = routed_rpi();
    let via_ids: Vec<ItemId> = board
        .get_items()
        .filter(|item| matches!(item, Item::Via(_)))
        .map(Item::id)
        .collect();
    let mut one_contact = Vec::new();
    let mut two_trace = Vec::new();
    for via_id in &via_ids {
        if board.normal_contacts(*via_id).len() == 1 {
            one_contact.push(*via_id);
        } else {
            two_trace.push(*via_id);
        }
    }
    // PORT-REGRESSION PIN, `accepted at plan9-t7t8 (ruling CC)`: the jar's two one-contact vias
    // are `187` and `84`; the port's are `189` and `84`, because Task 8's door-set fixes mint two
    // more board items over this fixture's routing prologue. Via `84` is below the point where the
    // two boards part and keeps its number. **Both centres below are the jar's, to the digit**,
    // and so is the `:292-294` recursion via 84 is here for.
    assert_eq!(
        one_contact.iter().map(|id| id.0).collect::<Vec<_>>(),
        vec![189, 84],
        "the routed `rpi` prefix leaves exactly these two one-contact vias"
    );
    assert_eq!(two_trace.len(), 4);

    // `p7t4` rpi modes 0 and 1, via rows 187 (the port's 189) and 84.
    for (via_id, expected) in [
        (ItemId(189), IntPoint::new(932_812, 1_011_224)),
        (ItemId(84), IntPoint::new(1_016_000, 3_119_161)),
    ] {
        for label in ["opt_via_location", "opt_plane_or_fanout_via"] {
            let mut scratch = board.clone();
            let before = scratch.structural_hash();
            let moved = if label == "opt_via_location" {
                ViaOptimizer::opt_via_location(&mut scratch, via_id, None, 500, 10)
            } else {
                ViaOptimizer::opt_plane_or_fanout_via(&mut scratch, via_id, 500, 10)
            }
            .expect("cannot fail");
            assert!(moved, "{label} on via {} must move it", via_id.0);
            assert_ne!(before, scratch.structural_hash(), "{label} must mutate");
            assert_eq!(
                scratch.drill_center(via_id).expect("still a via"),
                Point::Int(expected),
                "{label} on via {}",
                via_id.0
            );
        }
    }

    for via_id in two_trace {
        let mut scratch = board.clone();
        let before = scratch.structural_hash();
        assert!(
            !ViaOptimizer::opt_plane_or_fanout_via(&mut scratch, via_id, 500, 10)
                .expect("cannot fail"),
            ":188-190 — a second trace contact is already recorded"
        );
        assert_eq!(
            before,
            scratch.structural_hash(),
            "and the board is untouched"
        );
    }
}

// **RETIRED at the plan9-t7t8 accept wave (ruling CC).**
// `the_normal_contacts_are_visited_descending` stood here. It pinned `:46` —
// `via.getNormalContacts()` is a `TreeSet<Item>` and `Item.compareTo` is `other.id - id`
// (Item.java:95-101), so the walk is **descending** id, and the two contacts of a two-contact via
// become `firstTrace` and `secondTrace` in that order for `repositionVia`'s argument list at
// `:118-131`. Reversing the pair would silently swap the two halves of the cost calculation.
//
// It replayed `j2 mode 0`'s six via rows, each against the board the previous call left behind, so
// it was wholly `j2`-keyed and retires with that fixture's arm. The record at the top of this file
// carries the cause and the last matching run.
//
// **The claim it made is not left unpinned.** `the_overload_dispatch_matches_javas_contact_counts`
// asserts the descending order directly, on every via of `rpi` and `ecc83`, against the jar's own
// `contacts=` column — which is the same statement without the `j2` board behind it. What is gone
// is the replay against a MOVING board, i.e. that the order still holds after a previous via has
// been repositioned; no surviving fixture supplies that.

// =================================================================================================
// `P6T1.java`'s choices, as `opt_changed_area.rs` transcribes them
// =================================================================================================

fn routed_rpi() -> Board {
    routed("Issue143-rpi_splitter.dsn")
}

fn routed_j2() -> Board {
    routed("Issue026-J2_reference.dsn")
}

fn routed(design_name: &str) -> Board {
    let path = parity::fixture(design_name);
    let file = std::fs::File::open(&path)
        .unwrap_or_else(|e| panic!("cannot open {}: {e}", path.display()));
    let mut board =
        match fr_dsn::read_board(file, None, Some(design_name), &DsnReadOptions::default()) {
            BoardReadResult::Success { board, .. }
            | BoardReadResult::OutlineMissing { board, .. } => {
                *board.expect("the fixture produces a board")
            }
            other => panic!("{design_name} did not read: {other:?}"),
        };
    let settings = build_settings(&board);
    for (item_id, net_no) in pick_connections(&board, 12) {
        if board.get_item(item_id).is_none() {
            continue;
        }
        board.start_marking_changed_area();
        let mut ripped: BTreeSet<ItemId> = BTreeSet::new();
        let mut ripup_costs: BTreeMap<ItemId, i32> = BTreeMap::new();
        let trace_costs = settings.get_trace_costs();
        let mut engine = None;
        route_connection(
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
            &never,
        );
    }
    board
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

// =================================================================================================
// Dumps — `P7T4.java`'s
// =================================================================================================

fn regime_name(angle: AngleRestriction) -> &'static str {
    match angle {
        AngleRestriction::NinetyDegree => "NINETY_DEGREE",
        AngleRestriction::FortyFiveDegree => "FORTYFIVE_DEGREE",
        AngleRestriction::None => "NONE",
    }
}

fn type_name(item: &Item) -> &'static str {
    match item {
        Item::Trace(_) => "PolylineTrace",
        Item::Via(_) => "Via",
        Item::Pin(_) => "Pin",
        Item::ObstacleArea(_) => "ObstacleArea",
        Item::ConductionArea(_) => "ConductionArea",
        Item::ViaObstacleArea(_) => "ViaObstacleArea",
        Item::ComponentObstacleArea(_) => "ComponentObstacleArea",
        Item::ComponentOutline(_) => "ComponentOutline",
        Item::BoardOutline(_) => "BoardOutline",
    }
}

fn contact_ids(board: &Board, via: ItemId) -> String {
    let inner: Vec<String> = board
        .normal_contacts(via)
        .into_iter()
        .rev()
        .map(|id| {
            format!(
                "{}:{}",
                id.0,
                type_name(board.get_item(id).expect("a contact"))
            )
        })
        .collect();
    format!("[{}]", inner.join(","))
}

/// `P7T4.classify` — the read-only replica of `optViaLocation:39-106`.
fn classify(board: &Board, via: ItemId) -> &'static str {
    let item = board.get_item(via).expect("a via");
    if item.is_shove_fixed(&board.rules) {
        return "SHOVE_FIXED";
    }
    let contacts: Vec<ItemId> = board.normal_contacts(via).into_iter().rev().collect();
    if contacts.len() == 1 {
        return "PLANE_OR_FANOUT_ONE_CONTACT";
    }
    if contacts.len() != 2 {
        return "WRONG_CONTACT_COUNT";
    }
    let mut traces: Vec<ItemId> = Vec::new();
    let mut is_plane_or_fanout_via = false;
    for contact in contacts {
        let Some(contact_item) = board.get_item(contact) else {
            return "UNUSABLE_CONTACT";
        };
        if contact_item.is_shove_fixed(&board.rules) || !contact_item.is_trace() {
            if matches!(contact_item, Item::ConductionArea(_)) {
                is_plane_or_fanout_via = true;
            } else {
                return "UNUSABLE_CONTACT";
            }
        } else {
            traces.push(contact);
        }
    }
    if is_plane_or_fanout_via {
        return "PLANE_OR_FANOUT_CONDUCTION";
    }
    let via_center = board.drill_center(via).expect("a via has a centre");
    let min_width = match board.get_item(via) {
        Some(Item::Via(v)) => v.min_width(&board.ctx()),
        _ => unreachable!("a via"),
    };
    let tolerance = (min_width / 2.0) as i32 + 1;
    for trace in traces {
        let Some(Item::Trace(t)) = board.get_item(trace) else {
            return "UNUSABLE_CONTACT";
        };
        let (first, last) = (t.first_corner(), t.last_corner());
        if !ViaOptimizer::is_within_tolerance(first.as_ref(), &via_center, tolerance)
            && !ViaOptimizer::is_within_tolerance(last.as_ref(), &via_center, tolerance)
        {
            return "NOT_AT_ENDPOINT";
        }
    }
    "TWO_TRACES"
}

fn dump_line(line: &Line) -> String {
    format!("({},{})->({},{})", line.a.x, line.a.y, line.b.x, line.b.y)
}

fn dump_corner(polyline: &Polyline, no: usize) -> String {
    match polyline.corner(no).expect("no is below cornerCount") {
        Point::Int(p) => format!("({},{})", p.x, p.y),
        Point::Rational(_) => {
            let f = polyline.corner_approx(no).expect("no is below cornerCount");
            format!(
                "~({},{})",
                java_double_to_string(f.x),
                java_double_to_string(f.y)
            )
        }
    }
}

fn dump_polyline(polyline: &Polyline) -> String {
    let lines: Vec<String> = polyline.lines().iter().map(dump_line).collect();
    let corners: Vec<String> = (0..polyline.corner_count())
        .map(|i| dump_corner(polyline, i))
        .collect();
    format!(
        "n={} lines=[{}] corners=[{}]",
        polyline.lines().len(),
        lines.join(","),
        corners.join(",")
    )
}

fn dump_point(point: &Point) -> String {
    let rounded = point.to_float().round();
    format!("({},{})", rounded.x, rounded.y)
}

fn dump_nets(net_nos: &[i32]) -> String {
    let inner: Vec<String> = net_nos.iter().map(i32::to_string).collect();
    format!("[{}]", inner.join(","))
}

fn dump_fixed_state(state: FixedState) -> &'static str {
    match state {
        FixedState::Unfixed => "UNFIXED",
        FixedState::ShoveFixed => "SHOVE_FIXED",
        FixedState::UserFixed => "USER_FIXED",
        FixedState::SystemFixed => "SYSTEM_FIXED",
    }
}

fn dump_board(board: &Board) -> Vec<String> {
    let mut rows = vec![format!(
        "maxId={}",
        board.communication.id_gen.max_generated_id()
    )];
    for item in board.get_items() {
        let mut line = format!(
            "item id={} type={} nets={} cl={} fix={}",
            item.id().0,
            type_name(item),
            dump_nets(item.net_nos()),
            item.clearance_class(),
            dump_fixed_state(item.get_fixed_state())
        );
        match item {
            Item::Trace(trace) => {
                line.push_str(&format!(
                    " layer={} hw={} {}",
                    trace.get_layer(),
                    trace.get_half_width(),
                    dump_polyline(trace.polyline())
                ));
            }
            Item::Via(_) | Item::Pin(_) => {
                let center = board
                    .drill_center(item.id())
                    .expect("a drill item has a centre");
                line.push_str(&format!(" center={}", dump_point(&center)));
            }
            _ => {}
        }
        rows.push(line);
    }
    rows
}
