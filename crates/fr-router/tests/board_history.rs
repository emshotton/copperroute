//! Plan 7 Task 2: `autoroute/BoardHistory.java` — the pass loop's best-board memory
//! (controller ruling AF).
//!
//! # Where the numbers come from
//!
//! Every expectation below is **read off the HEAD jar**, never off this port. The probe is
//! `scripts/differential/java/probes/P7T2Probe.java`, committed with the exact `javac`/`java`
//! invocation in its header, and its whole stdout is committed as
//! `tests/data/p7t2-board-history.txt`. [`the_transcript_matches_the_jvm`] regenerates that file's
//! `boards` section and its first four phases from the port and compares them **line for line**, so a different eviction,
//! a different sort, a different `restoreCount`, one different score digit or one different
//! restored item fails it. The one line it drops is the `HEADER`, which names the jar by absolute
//! path and carries its size and mtime — the `p6t1` convention. The last phase, the
//! `BoardHistoryTest` replay, is outside that comparison for the reason [`expected_lines`] gives.
//!
//! Hashes are compared as an **equality pattern**, not as values: the transcript renders a hash
//! as a label `H0`, `H1`, … assigned in order of first appearance, because Java's is a hex MD5
//! over `serialize(true)` and the port's [`Board::structural_hash`] is a `u64` over the item
//! graph (controller ruling AH). Two boards agree iff both languages give them the same label.
//!
//! # This file is also the port of `src/test/java/app/freerouting/autoroute/BoardHistoryTest.java`
//!
//! Plan ruling 14. The six Java methods are ported by name, each with its Java line range in the
//! doc comment, and each is pinned by the probe's `=== phase javatest ===` section rather than by
//! this port's own opinion — the probe replays that suite against the jar on the suite's own two
//! fixtures (`empty_board.dsn` and `Issue159-setonix_2hp-pcb.dsn`).
//!
//! **Two of those six tests pass in Java for a reason their comments get wrong**, and the ported
//! versions say so: `restoreBestBoardFromMultiple` and `sizeCapEvictsWorstEntry` both assert that
//! the empty `board1` outranks the 199-item `board2` "because board2 has items, so it should have
//! a worse (lower) score". Measured on the jar, **both boards score `0.0`** — neither has any
//! connection, so `getNormalizedScore`'s `maximumScore <= 0f` guard (`BoardStatistics.java:626`,
//! Task 1's finding 1) returns `0f` for both. The assertions hold because of a *tie*: a stable
//! descending sort keeps `board1` first, and `add`'s `newScore <= worstScore` refuses an equal
//! board at capacity.

use std::collections::{BTreeMap, BTreeSet};

use fr_board::prelude::*;
use fr_board::structure::FixedState;
use fr_dsn::java_float_to_string;
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_geometry::{Point, Polyline};
use fr_router::pipeline::{BoardHistory, java_float_compare};
use fr_router::route_connection;
use fr_router::score::BoardStatistics;
use fr_settings::sources::DefaultSettings;
use fr_settings::{HostEnvironment, RouterSettings, ScoringSettings, SettingsSource};

// =================================================================================================
// The harness — `P7T2Probe.java`'s, which is `P6T1.java`'s
// =================================================================================================

const RPI_SPLITTER: &str = "fixtures/Issue143-rpi_splitter.dsn";
const EMPTY_BOARD: &str = "fixtures/empty_board.dsn";
const SETONIX: &str = "fixtures/Issue159-setonix_2hp-pcb.dsn";

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

/// `P6T1.main`'s settings, as `P7T2Probe.buildBoard` builds them.
fn build_settings(board: &Board) -> RouterSettings {
    let mut settings = DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    settings.set_layer_count(board.get_layer_count());
    settings.apply_board_specific_optimizations(board);
    settings
}

fn scoring_of(settings: &RouterSettings) -> ScoringSettings {
    settings
        .scoring
        .clone()
        .expect("DefaultSettings always writes a scoring block")
}

/// `P6T1.pickConnections`: `getItems()` order (descending id, quirk #63) × the item's own net
/// index order, keeping the pairs with a non-empty unconnected set.
fn pick_connections(board: &Board, max_items: usize) -> Vec<(ItemId, i32)> {
    let mut result = Vec::new();
    for item in board.get_items() {
        let item_id = item.id();
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

/// `P6T1.route` — one connection through steps 1-5, with the four choices `p6t1` documents.
fn route_one(board: &mut Board, settings: &RouterSettings, item_id: ItemId, net_no: i32) {
    if board.get_item(item_id).is_none() {
        return;
    }
    board.start_marking_changed_area();
    let mut ripped: BTreeSet<ItemId> = BTreeSet::new();
    let mut ripup_costs: BTreeMap<ItemId, i32> = BTreeMap::new();
    let mut engine = None;
    route_connection(
        board,
        &mut engine,
        item_id,
        net_no,
        settings,
        &settings.get_trace_costs(),
        &mut ripped,
        &mut ripup_costs,
        1,
        settings.get_start_ripup_costs(),
        !settings.is_fanout_enabled(),
        false,
        &|| false,
    );
}

/// `P7T2Probe.buildBoard` for a single `k`: a fresh load, then the first `k` connections routed.
fn build_board(rel_path: &str, k: usize) -> Board {
    let mut board = load_board(rel_path);
    let settings = build_settings(&board);
    if k == 0 {
        return board;
    }
    for (item_id, net_no) in pick_connections(&board, k) {
        route_one(&mut board, &settings, item_id, net_no);
    }
    board
}

/// `P7T2Probe.POOL_K` — the routed prefixes the transcript uses. `k = 2` and `k = 7` are
/// deliberately absent; see [`the_hash_ignores_a_failed_pass_that_java_can_still_tell_apart`].
const POOL_K: [usize; 7] = [0, 1, 3, 4, 5, 6, 8];

/// The probe's seven-board pool, indexed as [`POOL_K`].
///
/// The probe builds each `Bk` by an independent load and re-route, so that its pool does not
/// depend on `RoutingBoard.deepCopy`. This side routes **once** and takes a [`Clone`] after each
/// connection, which is the same board: `pickConnections(board, k)` on a freshly loaded board is
/// the `k`-element prefix of `pickConnections(board, 8)`, and the connections are routed in list
/// order, so "route `k` from scratch" and "stop after the `k`th" are the same states. `B1X` is
/// still built independently below, so the transcript keeps one genuine two-loads-agree check.
fn build_pool(rel_path: &str) -> Vec<Board> {
    let mut board = load_board(rel_path);
    let settings = build_settings(&board);
    let connections = pick_connections(&board, 8);
    let mut snapshots = vec![board.clone()];
    for (item_id, net_no) in connections {
        route_one(&mut board, &settings, item_id, net_no);
        snapshots.push(board.clone());
    }
    assert_eq!(snapshots.len(), 9, "nine states, k = 0..8");
    POOL_K.iter().map(|&k| snapshots[k].clone()).collect()
}

// =================================================================================================
// The transcript's rendering
// =================================================================================================

/// `P7T2Probe.HASH_LABELS` — `H0`, `H1`, … in order of first appearance (ruling AH's equality
/// pattern; the values themselves are not comparable across languages).
#[derive(Default)]
struct HashLabels(Vec<u64>);

impl HashLabels {
    fn label(&mut self, hash: u64) -> String {
        match self.0.iter().position(|&h| h == hash) {
            Some(i) => format!("H{i}"),
            None => {
                self.0.push(hash);
                format!("H{}", self.0.len() - 1)
            }
        }
    }
}

/// The transcript this side produces, one `String` per line.
struct Transcript {
    lines: Vec<String>,
    labels: HashLabels,
    scoring: ScoringSettings,
    call_no: usize,
}

impl Transcript {
    fn new(scoring: ScoringSettings) -> Transcript {
        Transcript {
            lines: Vec::new(),
            labels: HashLabels::default(),
            scoring,
            call_no: 0,
        }
    }

    fn push(&mut self, line: String) {
        self.lines.push(line);
    }

    fn score(&self, board: &mut Board) -> String {
        java_float_to_string(BoardStatistics::new(board).normalized_score(&self.scoring))
    }

    /// `P7T2Probe.describeBoard` — the score is computed **first** and the hash second, because
    /// Java's `getHash()` moves the first time anything asks a `Pin` for its centre (see the
    /// probe's header and quirk #200). This side is order-insensitive; the order is kept so the
    /// two programs read alike.
    fn describe_board(&mut self, name: &str, board: &mut Board) {
        let score = self.score(board);
        let label = self.labels.label(board.structural_hash());
        let line = format!(
            "  {name} hash={label} score={score} items={} maxId={}",
            board.get_items().count(),
            board.communication.id_gen.max_generated_id().0
        );
        self.push(line);
    }

    /// `P7T2Probe.call` — one call's line, then the whole list in list order.
    fn call(&mut self, history: &BoardHistory, op: &str, ret: &str) {
        self.call_no += 1;
        let call_no = self.call_no;
        self.push(format!("call={call_no} {op} ret={ret}"));
        self.push(format!("  size={}", history.size()));
        for (i, entry) in history.entries().iter().enumerate() {
            let label = self.labels.label(entry.hash);
            self.push(format!(
                "  entry={i} hash={label} score={} restoreCount={}",
                java_float_to_string(entry.score),
                entry.restore_count
            ));
        }
    }

    /// `P7T2Probe.restore` — the call line, the list, then the restored board's identity.
    fn restore(&mut self, history: &BoardHistory, op: &str, restored: Option<&mut Board>) {
        self.call(
            history,
            op,
            if restored.is_none() { "null" } else { "board" },
        );
        let Some(board) = restored else {
            self.push("  restored=null".to_string());
            return;
        };
        let score = self.score(board);
        let label = self.labels.label(board.structural_hash());
        let line = format!(
            "  restored hash={label} score={score} items={} maxId={}",
            board.get_items().count(),
            board.communication.id_gen.max_generated_id().0
        );
        self.push(line);
    }

    /// `P7T2Probe.dumpItems` — one line per item, in `getItems()` order (descending id,
    /// quirk #63).
    fn dump_items(&mut self, board: &Board) {
        self.push(format!(
            "  items maxId={}",
            board.communication.id_gen.max_generated_id().0
        ));
        let lines: Vec<String> = board
            .get_items()
            .map(|item| {
                let mut line = format!(
                    "  item id={} type={} nets={} cl={} fixed={}",
                    item.id().0,
                    java_type_name(item),
                    dump_nets(item.net_nos()),
                    item.clearance_class(),
                    java_fixed_state(item.get_fixed_state()),
                );
                match item {
                    Item::Trace(trace) => line.push_str(&format!(
                        " layer={} hw={} corners={}",
                        trace.get_layer(),
                        trace.get_half_width(),
                        dump_corners(trace.polyline())
                    )),
                    Item::Via(_) | Item::Pin(_) => {
                        let center = board
                            .drill_center(item.id())
                            .expect("a drill item has a centre");
                        line.push_str(&format!(" center={}", dump_point(&center)));
                    }
                    _ => {}
                }
                line
            })
            .collect();
        for line in lines {
            self.push(line);
        }
    }
}

/// Java's `item.getClass().getSimpleName()`.
fn java_type_name(item: &Item) -> &'static str {
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

/// Java's `FixedState` enum names (`board/model/structure/FixedState.java:5-8`).
fn java_fixed_state(state: FixedState) -> &'static str {
    match state {
        FixedState::Unfixed => "UNFIXED",
        FixedState::ShoveFixed => "SHOVE_FIXED",
        FixedState::UserFixed => "USER_FIXED",
        FixedState::SystemFixed => "SYSTEM_FIXED",
    }
}

/// `P7T2Probe.nets` — `java.util.Arrays.toString` with the spaces removed.
fn dump_nets(net_nos: &[i32]) -> String {
    let inner: Vec<String> = net_nos.iter().map(i32::to_string).collect();
    format!("[{}]", inner.join(","))
}

/// `P7T2Probe.pointOf` — `p.toFloat().round()`, rendered `(x,y)`.
fn dump_point(point: &Point) -> String {
    let rounded = point.to_float().round();
    format!("({},{})", rounded.x, rounded.y)
}

/// `P7T2Probe.corners` — an `IntPoint` corner prints exactly, any other prints its `cornerApprox`
/// through `Double.toString`.
fn dump_corners(polyline: &Polyline) -> String {
    let corners: Vec<String> = (0..polyline.corner_count())
        .map(
            |i| match polyline.corner(i).expect("i is below cornerCount") {
                Point::Int(p) => format!("({},{})", p.x, p.y),
                Point::Rational(_) => {
                    let f = polyline.corner_approx(i).expect("i is below cornerCount");
                    format!(
                        "~({},{})",
                        fr_dsn::java_double_to_string(f.x),
                        fr_dsn::java_double_to_string(f.y)
                    )
                }
            },
        )
        .collect();
    format!("[{}]", corners.join(","))
}

// =================================================================================================
// The transcript
// =================================================================================================

const T2: &str = include_str!("data/p7t2-board-history.txt");

/// The committed transcript's routed-board phases: everything from `=== boards ===` down to (but
/// not including) `=== phase javatest ===`, minus the `HEADER` line, which names the jar by
/// absolute path and carries its size and mtime (the `p6t1` convention).
///
/// The `javatest` phase is **deliberately outside** the byte-for-byte comparison. Its two boards
/// are `empty_board.dsn` and `Issue159-setonix_2hp-pcb.dsn`, neither of which has a single trace
/// or via, and [`Board::structural_hash`] hashes **only traces and vias** — so the port gives them
/// the same hash and the JVM does not. That is a recorded divergence with a named owner: see
/// [`the_hash_cannot_tell_two_trace_free_boards_apart`], and the six ported `BoardHistoryTest`
/// methods below, which assert against that phase's lines one value at a time and mark the three
/// that the divergence moves.
fn expected_lines() -> Vec<&'static str> {
    T2.lines()
        .filter(|line| !line.starts_with("HEADER "))
        .take_while(|line| *line != "=== phase javatest ===")
        .map(str::trim_end)
        .collect()
}

fn assert_lines_match(actual: &[String]) {
    let expected = expected_lines();
    // A truncated or re-cut transcript must fail loudly rather than silently comparing nothing:
    // `take_while` would answer an empty `Vec` if the `boards` banner ever moved.
    assert_eq!(expected.first().copied(), Some("=== boards ==="));
    for banner in [
        "=== phase cap3 ===",
        "=== phase cap30 ===",
        "=== phase tie ===",
        "=== phase floatcompare ===",
    ] {
        assert!(
            expected.contains(&banner),
            "the compared section of the transcript is missing `{banner}`"
        );
    }

    let mut diffs = Vec::new();
    for i in 0..expected.len().max(actual.len()) {
        let want = expected.get(i).copied().unwrap_or("<missing>");
        let got = actual
            .get(i)
            .map(|row| row.trim_end())
            .unwrap_or("<missing>");
        if want != got {
            diffs.push(format!("line {i}\n  jvm:  {want}\n  rust: {got}"));
        }
    }
    assert!(
        diffs.is_empty(),
        "{} of {} transcript lines differ\n{}",
        diffs.len(),
        expected.len().max(actual.len()),
        diffs
            .iter()
            .take(12)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Regenerates `P7T2Probe`'s `=== boards ===` section and its `cap3`, `cap30`, `tie` and
/// `floatcompare` phases, and compares them with the committed transcript **line for line**: 291
/// lines, **47** `BoardHistory` calls over eight boards (32 + 11 + 4), every entry of the list
/// after every call, plus `Float.compare`'s fourteen rows.
///
/// It runs under a plain `cargo test --workspace`: `Issue143-rpi_splitter.dsn` is the smallest
/// board in the corpus that actually routes, and its eight connections plus the two extra loads
/// are well under a second even unoptimised — measured, not assumed. Ruling AM puts this stem in
/// CI for exactly that reason.
#[test]
fn the_transcript_matches_the_jvm() {
    let mut pool = build_pool(RPI_SPLITTER);
    let mut b1x = build_board(RPI_SPLITTER, 1);
    let scoring = scoring_of(&build_settings(&pool[0]));
    let mut t = Transcript::new(scoring.clone());

    // --- the pool ---------------------------------------------------------------------------
    t.push("=== boards ===".to_string());
    for (i, board) in pool.iter_mut().enumerate() {
        t.describe_board(&format!("B{}", POOL_K[i]), board);
    }
    t.describe_board("B1X", &mut b1x);

    // --- phase 1: the cap-3 constructor -------------------------------------------------------
    t.push("=== phase cap3 ===".to_string());
    t.call_no = 0;
    let mut h = BoardHistory::with_capacity(&scoring, 3);

    let size = h.size().to_string();
    t.call(&h, "size", &size);
    let max = java_float_to_string(h.max_score());
    t.call(&h, "getMaxScore", &max);
    let contains = h.contains(&pool[0]).to_string();
    t.call(&h, "contains(B0)", &contains);
    let rank = h.rank(&pool[0]).to_string();
    t.call(&h, "getRank(B0)", &rank);
    let mut restored = h.restore_best_board();
    t.restore(&h, "restoreBestBoard", restored.as_mut());

    h.add(&mut pool[0]);
    t.call(&h, "add(B0)", "-");
    h.add(&mut pool[0]);
    t.call(&h, "add(B0) again", "-");
    h.add(&mut b1x);
    t.call(&h, "add(B1X-as-B1-twin)", "-");
    h.add(&mut pool[2]);
    t.call(&h, "add(B3)", "-");
    let contains = h.contains(&pool[1]).to_string();
    t.call(&h, "contains(B1)", &contains);
    let contains = h.contains(&pool[3]).to_string();
    t.call(&h, "contains(B4)", &contains);

    let size = h.size().to_string();
    t.call(&h, "size", &size);
    h.add(&mut pool[3]);
    t.call(&h, "add(B4) at capacity", "-");
    h.add(&mut pool[0]);
    t.call(&h, "add(B0) at capacity", "-");
    let contains = h.contains(&pool[0]).to_string();
    t.call(&h, "contains(B0)", &contains);
    let max = java_float_to_string(h.max_score());
    t.call(&h, "getMaxScore", &max);
    for i in [1usize, 2, 3] {
        let rank = h.rank(&pool[i]).to_string();
        t.call(&h, &format!("getRank(B{})", POOL_K[i]), &rank);
    }

    let mut r1 = h.restore_board(0);
    t.restore(&h, "restoreBoard(0)", r1.as_mut());
    let rank = r1
        .as_ref()
        .map(|b| h.rank(b))
        .expect("the history is not empty")
        .to_string();
    t.call(&h, "getRank(restored#1)", &rank);
    let mut restored = h.restore_board(1);
    t.restore(&h, "restoreBoard(1)", restored.as_mut());
    let mut restored = h.restore_board(1);
    t.restore(&h, "restoreBoard(1) again", restored.as_mut());
    let mut restored = h.restore_board(1);
    t.restore(&h, "restoreBoard(1) a third time", restored.as_mut());
    let mut restored = h.restore_board(-7);
    t.restore(&h, "restoreBoard(-7) is unlimited", restored.as_mut());
    let mut restored = h.restore_best_board();
    t.restore(&h, "restoreBestBoard", restored.as_mut());

    h.remove(&pool[1]);
    t.call(&h, "remove(B1)", "-");
    h.remove(&pool[1]);
    t.call(&h, "remove(B1) again", "-");
    let rank = h.rank(&pool[1]).to_string();
    t.call(&h, "getRank(B1)", &rank);
    h.clear();
    t.call(&h, "clear", "-");
    let max = java_float_to_string(h.max_score());
    t.call(&h, "getMaxScore", &max);
    let mut restored = h.restore_best_board();
    t.restore(&h, "restoreBestBoard", restored.as_mut());

    // --- phase 2: the default cap --------------------------------------------------------------
    t.push("=== phase cap30 ===".to_string());
    t.call_no = 0;
    let mut big = BoardHistory::new(&scoring);
    for i in 0..pool.len() {
        big.add(&mut pool[i]);
        t.call(&big, &format!("add(B{})", POOL_K[i]), "-");
    }
    let max = java_float_to_string(big.max_score());
    t.call(&big, "getMaxScore", &max);
    let mut best = big.restore_best_board();
    t.restore(&big, "restoreBestBoard", best.as_mut());
    let best = best.expect("the history is not empty");
    let rank = big.rank(&best).to_string();
    t.call(&big, "getRank(best)", &rank);
    t.dump_items(&best);
    big.clear();
    t.call(&big, "clear", "-");

    // --- phase 3: the eviction gate's `<=` -----------------------------------------------------
    //
    // `B1F` is `B1` with its highest-id trace marked `USER_FIXED`: a different `structural_hash`
    // (the fixed state is one of the seven fields it covers) and an identical score (none of
    // `calculateScore`'s six inputs is fixed-state dependent). Nothing in the pool ties, so this
    // is the only case that separates `add`'s `newScore <= worstScore` (`:73`) from a `<`.
    t.push("=== phase tie ===".to_string());
    t.call_no = 0;
    let mut b1f = build_board(RPI_SPLITTER, 1);
    let first_trace = b1f
        .get_items()
        .find(|item| matches!(item, Item::Trace(_)))
        .expect("the routed board has a trace")
        .id();
    b1f.get_item_mut(first_trace)
        .expect("the id came from the board")
        .set_fixed_state(FixedState::UserFixed);
    t.describe_board("B1F", &mut b1f);

    let mut tie = BoardHistory::with_capacity(&scoring, 1);
    tie.add(&mut pool[1]);
    t.call(&tie, "add(B1) at cap 1", "-");
    tie.add(&mut b1f);
    t.call(&tie, "add(B1F), an equal score and a new hash", "-");
    let contains = tie.contains(&b1f).to_string();
    t.call(&tie, "contains(B1F)", &contains);
    let rank = tie.rank(&b1f).to_string();
    t.call(&tie, "getRank(B1F)", &rank);

    // --- phase 4: `Float.compare` ---------------------------------------------------------------
    t.push("=== phase floatcompare ===".to_string());
    for [a, b] in float_compare_pairs() {
        t.push(format!(
            "compare bits({:08x},{:08x}) = {}",
            a.to_bits(),
            b.to_bits(),
            match java_float_compare(a, b) {
                std::cmp::Ordering::Less => -1,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => 1,
            }
        ));
    }

    assert_lines_match(&t.lines);
}

/// `P7T2Probe`'s `Float.compare` table: the ordinary cases, both signed zeros, a canonical NaN in
/// every position and — the discriminator — a **negative** NaN, which `Float.compare` canonicalises
/// to `0x7fc00000` and therefore sorts *above* `+Infinity`, where [`f32::total_cmp`] would sort it
/// below `-Infinity`.
fn float_compare_pairs() -> Vec<[f32; 2]> {
    let neg_nan = f32::from_bits(0xffc0_0000);
    vec![
        [1.0, 2.0],
        [2.0, 1.0],
        [1.0, 1.0],
        [0.0, -0.0],
        [-0.0, 0.0],
        [-0.0, -0.0],
        [f32::NAN, 1.0],
        [1.0, f32::NAN],
        [f32::NAN, f32::NAN],
        [f32::NAN, f32::INFINITY],
        [neg_nan, 1.0],
        [1.0, neg_nan],
        [neg_nan, f32::NEG_INFINITY],
        [f32::NEG_INFINITY, f32::INFINITY],
    ]
}

/// The comparator `restoreBoard:143` sorts with is `Float.compare`, and it is neither
/// [`f32::partial_cmp`] (which answers `None` for every NaN pair and `Equal` for `-0.0` against
/// `+0.0`) nor [`f32::total_cmp`] (which disagrees on a **negative** NaN). The JVM's answers are
/// the transcript's `=== phase floatcompare ===` rows.
#[test]
fn java_float_compare_is_the_jdks() {
    use std::cmp::Ordering::{Equal, Greater, Less};
    let neg_nan = f32::from_bits(0xffc0_0000);

    assert_eq!(java_float_compare(1.0, 2.0), Less);
    assert_eq!(java_float_compare(1.0, 1.0), Equal);
    // `-0.0 < +0.0`, where `partial_cmp` says `Equal`.
    assert_eq!(java_float_compare(0.0, -0.0), Greater);
    assert_eq!(java_float_compare(-0.0, 0.0), Less);
    assert_eq!(0.0f32.partial_cmp(&-0.0f32), Some(Equal));
    // A NaN sorts above `+Infinity`, where `partial_cmp` says `None`.
    assert_eq!(java_float_compare(f32::NAN, f32::INFINITY), Greater);
    assert_eq!(java_float_compare(f32::NAN, f32::NAN), Equal);
    assert_eq!(f32::NAN.partial_cmp(&f32::INFINITY), None);
    // And a *negative* NaN does too, where `total_cmp` puts it below `-Infinity`.
    assert_eq!(java_float_compare(neg_nan, f32::NEG_INFINITY), Greater);
    assert_eq!(neg_nan.total_cmp(&f32::NEG_INFINITY), Less);
}

// =================================================================================================
// The named behavioural pins (the brief's list)
// =================================================================================================

/// Two boards for the cheap tests: the unrouted `rpi_splitter` and the same board with its first
/// connection routed. The transcript's `B0` and `B1`.
fn two_boards() -> (Board, Board, ScoringSettings) {
    let b0 = build_board(RPI_SPLITTER, 0);
    let b1 = build_board(RPI_SPLITTER, 1);
    let scoring = scoring_of(&build_settings(&b0));
    (b0, b1, scoring)
}

/// `add` (BoardHistory.java:48-80) has **no score gate below capacity**: `:53`'s `if` guards the
/// whole eviction block, so any board with a new hash enters however badly it scores.
///
/// The transcript's `cap3` calls 6-9: `B0` scores `0.0` and still enters a history that already
/// holds boards scoring `199.99464`.
#[test]
fn under_capacity_any_distinct_board_enters() {
    let (mut b0, mut b1, scoring) = two_boards();
    let mut h = BoardHistory::with_capacity(&scoring, 3);

    h.add(&mut b1);
    assert_eq!(h.size(), 1);
    // `B0` is strictly worse than `B1` — the jar says 0.0 against 199.99464 — and enters anyway.
    h.add(&mut b0);
    assert_eq!(h.size(), 2, "under capacity there is no score gate");
    assert!(h.contains(&b0));
    assert_eq!(java_float_to_string(h.entries()[0].score), "199.99464");
    assert_eq!(java_float_to_string(h.entries()[1].score), "0.0");
}

/// At capacity `add` refuses a board that is not **strictly** better than the worst entry
/// (BoardHistory.java:70-75, `newScore <= worstScore`), and the refusal is complete: the size is
/// unchanged **and** the board is absent by `contains`.
///
/// The transcript's `cap3` call 14 — `add(B0)` at capacity leaves the list exactly as call 13 left
/// it. Renamed from the brief's `…_without_serialising`: ruling 8 replaced the serialisation with
/// a clone, so the original name asserted a mechanism this port does not have.
#[test]
fn at_capacity_a_worse_board_is_rejected() {
    let (mut b0, mut b1, scoring) = two_boards();
    let mut h = BoardHistory::with_capacity(&scoring, 1);

    h.add(&mut b1);
    assert_eq!(h.size(), 1);
    let before: Vec<u64> = h.entries().iter().map(|e| e.hash).collect();

    h.add(&mut b0);
    assert_eq!(h.size(), 1, "the size must stay at the cap");
    assert!(!h.contains(&b0), "the rejected board must be absent");
    assert_eq!(
        before,
        h.entries().iter().map(|e| e.hash).collect::<Vec<_>>(),
        "nothing was evicted either"
    );
}

/// The `<=` of `add`'s eviction gate (BoardHistory.java:73), which a `<` would pass every other
/// test in this file: a board that **ties** the worst entry is refused, not swapped in.
///
/// The tie has to be built, because nothing in the corpus pool ties: `B1F` is `B1` with its
/// highest-id trace marked `USER_FIXED`, which moves [`Board::structural_hash`] (the fixed state
/// is one of the seven fields it covers) and moves none of `calculateScore`'s six inputs
/// (incompletes, violations, bends, trace length, vias, `maximumCount`). The transcript's
/// `=== phase tie ===` is the JVM answering the same four calls.
#[test]
fn at_capacity_an_equal_scoring_board_is_rejected_too() {
    let (_, mut b1, scoring) = two_boards();
    let mut b1f = build_board(RPI_SPLITTER, 1);
    let first_trace = b1f
        .get_items()
        .find(|item| matches!(item, Item::Trace(_)))
        .expect("the routed board has a trace")
        .id();
    b1f.get_item_mut(first_trace)
        .expect("the id came from the board")
        .set_fixed_state(FixedState::UserFixed);

    assert_ne!(
        b1.structural_hash(),
        b1f.structural_hash(),
        "a different board"
    );
    assert_eq!(
        java_float_to_string(BoardStatistics::new(&mut b1).normalized_score(&scoring)),
        java_float_to_string(BoardStatistics::new(&mut b1f).normalized_score(&scoring)),
        "with an identical score — the JVM says 199.99464 for both"
    );

    let mut h = BoardHistory::with_capacity(&scoring, 1);
    h.add(&mut b1);
    h.add(&mut b1f);
    assert_eq!(h.size(), 1, "the tie does not evict");
    assert!(!h.contains(&b1f), "and the newcomer is absent");
    assert_eq!(h.rank(&b1f), -1);
}

/// `add`'s first act is `contains(board)` (BoardHistory.java:49-51), so a board whose hash is
/// already present never reaches the gate — and hash equality, not object identity, is what
/// decides it.
///
/// The transcript's `cap3` calls 7 and 8: `add(B0)` a second time changes nothing, and `B2X` — an
/// independently loaded and re-routed twin of `B2` — collides with the entry `B2` would have made.
#[test]
fn an_identical_board_is_rejected_by_hash() {
    let (_, mut b1, scoring) = two_boards();
    let mut twin = build_board(RPI_SPLITTER, 1);
    assert_eq!(
        b1.structural_hash(),
        twin.structural_hash(),
        "two independent builds of the same board hash alike"
    );

    let mut h = BoardHistory::new(&scoring);
    h.add(&mut b1);
    h.add(&mut b1);
    assert_eq!(h.size(), 1, "the same board twice is one entry");
    h.add(&mut twin);
    assert_eq!(h.size(), 1, "a distinct object with the same hash is too");
}

/// **Java bug (quirk #197):** `getMaxScore`'s accumulator starts at `0` (BoardHistory.java:118),
/// not at `-inf`, so an empty history answers `0.0` — and `AutorouteBatchLoop.java:306`'s strict
/// `bh.getMaxScore() > boardScoreAfter` therefore never fires on one.
#[test]
fn max_score_of_an_empty_history_is_zero_not_negative_infinity() {
    let (_, _, scoring) = two_boards();
    let h = BoardHistory::new(&scoring);

    assert_eq!(h.size(), 0);
    assert_eq!(java_float_to_string(h.max_score()), "0.0");
    assert!(h.max_score().is_finite(), "not -inf, and not NaN either");

    // The counterfactual, so the seed is shown to be load-bearing. `AutorouteBatchLoop.java:306`
    // is `bh.getMaxScore() > boardScoreAfter`: with Java's `0` seed an empty history cannot fire
    // it against a board scoring `0` or better, and with a `-inf` seed it could not fire it
    // against anything — the two disagree on every board that scores **below** zero, which is
    // every board with more clearance violations than `maximumScore` can absorb.
    let empty_history_vs_a_negative_board = h.max_score() > -1.0f32;
    assert!(
        empty_history_vs_a_negative_board,
        "the 0 seed fires the restore gate where -inf would not"
    );
}

/// **Java bug (quirk #198):** `restoreBoard` **sorts the list in place** (BoardHistory.java:143)
/// and increments the chosen entry's `restoreCount` (`:147`), both under a *read* lock.
///
/// The transcript's `cap30` calls 10 and 11: seven entries in insertion order become seven entries
/// in descending score order, and the winner's `restoreCount` goes 0 → 1.
#[test]
fn restore_board_increments_the_restore_count_and_reorders_the_list() {
    let (mut b0, mut b1, scoring) = two_boards();
    let mut h = BoardHistory::new(&scoring);

    // Insertion order is worst-first, which is not score order.
    h.add(&mut b0);
    h.add(&mut b1);
    assert_eq!(java_float_to_string(h.entries()[0].score), "0.0");
    assert_eq!(java_float_to_string(h.entries()[1].score), "199.99464");
    assert_eq!(h.entries()[0].restore_count, 0);

    let restored = h.restore_board(0).expect("a two-entry history restores");

    // Descending by score, in place — the list is state, not a view.
    assert_eq!(java_float_to_string(h.entries()[0].score), "199.99464");
    assert_eq!(java_float_to_string(h.entries()[1].score), "0.0");
    assert_eq!(h.entries()[0].restore_count, 1, "the winner's count rose");
    assert_eq!(h.entries()[1].restore_count, 0, "and only the winner's");
    assert_eq!(restored.structural_hash(), b1.structural_hash());
}

/// The pin for quirk #198's consequence: `getRank` reads the **current list order**, so the same
/// board ranks differently before and after a `restoreBoard` — and
/// `AutorouteBatchLoop.java:315-320` breaks the whole pass loop on `getRank(...) >
/// BOARD_RANK_LIMIT`.
#[test]
fn get_rank_depends_on_the_last_restore_sort() {
    let (mut b0, mut b1, scoring) = two_boards();
    let mut h = BoardHistory::new(&scoring);

    h.add(&mut b0);
    h.add(&mut b1);
    assert_eq!(h.rank(&b0), 1, "insertion order until the first restore");
    assert_eq!(h.rank(&b1), 2);

    h.restore_board(0);

    assert_eq!(h.rank(&b1), 1, "score order after it");
    assert_eq!(h.rank(&b0), 2);
    assert_eq!(
        h.rank(&build_board(RPI_SPLITTER, 3)),
        -1,
        "and -1 for a miss"
    );
}

/// `restoreBoard`'s budget clause (BoardHistory.java:135-137) is `maxAllowedRestoreCount <= 0`,
/// not `== 0`: both `0` — which is what `restoreBestBoard` (`:158-160`) passes — and any negative
/// number mean `Integer.MAX_VALUE`, i.e. no budget at all.
#[test]
fn restore_board_zero_means_unlimited() {
    let (_, mut b1, scoring) = two_boards();
    let mut h = BoardHistory::new(&scoring);
    h.add(&mut b1);

    for expected_count in 1..=4 {
        let budget = match expected_count {
            1 => 0,
            2 => -1,
            3 => i32::MIN,
            _ => 0,
        };
        assert!(
            h.restore_board(budget).is_some(),
            "budget {budget} is unlimited"
        );
        assert_eq!(h.entries()[0].restore_count, expected_count);
    }

    // A *positive* budget really does bound it: the entry is now at 4 restores.
    assert!(h.restore_board(3).is_none(), "4 > 3, so no entry qualifies");
    assert_eq!(h.entries()[0].restore_count, 4, "and no count moved");
    assert!(h.restore_board(4).is_some(), "4 <= 4 still qualifies");
}

/// A **roster assertion**, and deliberately so — it asserts the roster, not behaviour, which is
/// its whole job (the Plan 5 precedent).
///
/// `autoroute/BoardHistoryEntry.java`'s public, `Comparable` top-level class is **shadowed** by
/// `BoardHistory`'s own `private static class BoardHistoryEntry` (BoardHistory.java:188) and is
/// reachable from nothing (quirk #199). `scripts/audit-port.sh` accepts the class only because
/// `crates/fr-router/src/lib.rs` carries the marker; this test is what stops the marker being
/// deleted by someone who sees the ported [`fr_router::BoardHistoryEntry`] — which is the
/// **nested** class — and concludes the roster line is stale.
#[test]
fn the_top_level_board_history_entry_class_is_unreachable() {
    let lib_rs = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let text = std::fs::read_to_string(&lib_rs)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", lib_rs.display()));
    assert!(
        text.contains("// not ported: `BoardHistoryEntry.compareTo`"),
        "the roster line for the shadowed top-level BoardHistoryEntry is missing from {}",
        lib_rs.display()
    );
    assert!(
        text.contains("quirk #199"),
        "the roster line must cite the quirk that explains why the class is dead"
    );
}

// =================================================================================================
// The port of `src/test/java/app/freerouting/autoroute/BoardHistoryTest.java` (plan ruling 14)
// =================================================================================================
//
// Method for method, in declaration order, on the suite's own two fixtures. Every expectation is
// the JVM's, read off `tests/data/p7t2-board-history.txt`'s `=== phase javatest ===` section,
// which is `P7T2Probe.javaTest` replaying this suite against the HEAD jar.
//
// **Three of the six carry an `XDIFF:` assertion**, and all three are the same root cause:
// `empty_board.dsn` and `Issue159-setonix_2hp-pcb.dsn` have **no traces and no vias**, and
// `Board::structural_hash` (`crates/fr-board/src/board/snapshot.rs:210-240`) hashes only traces
// and vias — so the port gives every trace-free board the same hash, where Java's MD5 over
// `serialize(true)` sees the whole item graph and gives a 1-item board and a 199-item board
// different ones. **Task 3 (controller ruling AH) owns the widening**, and these three assertions
// are what it flips; this task's brief is explicit that it consumes `structural_hash` as it stands
// and reports rather than fixes. See [`the_hash_cannot_tell_two_trace_free_boards_apart`].

/// `BoardHistoryTest.setUp` (BoardHistoryTest.java:27-50): the empty board and the 199-item one,
/// reloaded before every method — `new BoardStatistics(board)` mutates the board it measures, so
/// a shared pair would make the sixth method depend on the first five.
fn set_up() -> (Board, Board, ScoringSettings) {
    let board1 = load_board(EMPTY_BOARD);
    let board2 = load_board(SETONIX);
    let scoring = scoring_of(&build_settings(&board1));
    (board1, board2, scoring)
}

/// `BoardHistoryTest.addAndRestoreBoard` (BoardHistoryTest.java:53-66).
///
/// Transcript calls 1-2: `size=1`, `entry=0 … restoreCount=0` then `restoreCount=1`,
/// `restored hash=H6 score=0.0 items=1 maxId=1`, `notSameInstance=true`, `sameHashAsBoard1=true`.
#[test]
fn add_and_restore_board() {
    let (mut board1, _board2, scoring) = set_up();
    let mut history = BoardHistory::new(&scoring);
    history.add(&mut board1);
    assert_eq!(history.size(), 1);
    assert_eq!(java_float_to_string(history.entries()[0].score), "0.0");

    let restored = history.restore_best_board().expect("assertNotNull(:60)");

    // `assertNotSame(board1, restoredBoard, "Restored board should be a new instance")` (:61) —
    // the port's `restore_board` answers an owned `Board`, so there is no aliasing to test.
    assert_eq!(history.entries()[0].restore_count, 1);
    // `assertEquals(board1.getHash(), restoredBoard.getHash(), …)` (:62-65).
    assert_eq!(restored.structural_hash(), board1.structural_hash());
    assert_eq!(restored.get_items().count(), 1);
    assert_eq!(restored.communication.id_gen.max_generated_id().0, 1);
}

/// `BoardHistoryTest.restoreBestBoardFromMultiple` (BoardHistoryTest.java:68-81).
///
/// Transcript calls 3-5. **The Java comment's premise is false**: it says "board2 has items, so it
/// should have a worse (lower) score than the empty board1", and the jar answers `score=0.0` for
/// **both** — neither board has a connection, so `getNormalizedScore`'s `maximumScore <= 0f` guard
/// (`BoardStatistics.java:626`) returns `0f` for each. The assertion holds because the descending
/// sort is **stable**, so the tie keeps `board1` first.
#[test]
fn restore_best_board_from_multiple() {
    let (mut board1, mut board2, scoring) = set_up();
    let mut history = BoardHistory::new(&scoring);
    history.add(&mut board1);
    history.add(&mut board2);

    // XDIFF: the JVM's transcript says `size=2` at call 4 (two entries, `H6` and `H7`). The port
    // says 1, because `add`'s `contains` gate sees one hash for both trace-free boards. Task 3.
    assert_eq!(
        history.size(),
        1,
        "XDIFF vs the JVM's 2 — see the section header"
    );

    let best = history.restore_best_board().expect("assertNotNull(:76)");
    // `assertEquals(board1.getHash(), bestBoard.getHash(), …)` (:78-80) — which holds either way.
    assert_eq!(best.structural_hash(), board1.structural_hash());
    assert_eq!(best.get_items().count(), 1);
}

/// `BoardHistoryTest.contains` (BoardHistoryTest.java:83-90).
///
/// Transcript calls 6-8: `contains(board1) ret=true`, `contains(board2) ret=false`.
#[test]
fn contains() {
    let (mut board1, board2, scoring) = set_up();
    let mut history = BoardHistory::new(&scoring);
    history.add(&mut board1);

    // `assertTrue(history.contains(board1), …)` (:87).
    assert!(history.contains(&board1));
    // `assertFalse(history.contains(board2), …)` (:88) — XDIFF: the JVM answers `false`, the port
    // answers `true`, because the two boards share a hash. Task 3.
    assert!(
        history.contains(&board2),
        "XDIFF vs the JVM's false — see the section header"
    );
}

/// `BoardHistoryTest.clear` (BoardHistoryTest.java:92-101).
///
/// Transcript calls 9-10: `size=2` after the two adds, `size=0` after `clear`.
#[test]
fn clear() {
    let (mut board1, mut board2, scoring) = set_up();
    let mut history = BoardHistory::new(&scoring);
    history.add(&mut board1);
    history.add(&mut board2);
    // `assertEquals(2, history.size())` (:97) — XDIFF: the port says 1. Task 3.
    assert_eq!(
        history.size(),
        1,
        "XDIFF vs the JVM's 2 — see the section header"
    );

    history.clear();
    // `assertEquals(0, history.size(), "History should be empty after clear()")` (:100).
    assert_eq!(history.size(), 0);
}

/// `BoardHistoryTest.sizeCapNeverExceedsMaxHistorySize` (BoardHistoryTest.java:103-115).
///
/// Transcript call 11: `size=2`, `withinCap=true`. The port's size is 1 (the same XDIFF), and the
/// assertion this method actually makes — `size() <= MAX_HISTORY_SIZE` — holds either way, so it
/// is the one ported test the divergence cannot reach.
#[test]
fn size_cap_never_exceeds_max_history_size() {
    let (mut board1, mut board2, scoring) = set_up();
    let mut history = BoardHistory::with_capacity(&scoring, BoardHistory::MAX_HISTORY_SIZE);
    history.add(&mut board1);
    history.add(&mut board2);
    history.add(&mut board1);

    // `assertTrue(history.size() <= BoardHistory.MAX_HISTORY_SIZE, …)` (:111-113).
    assert!(history.size() <= BoardHistory::MAX_HISTORY_SIZE);
    assert_eq!(BoardHistory::MAX_HISTORY_SIZE, 30, "BoardHistory.java:29");
}

/// `BoardHistoryTest.sizeCapEvictsWorstEntry` (BoardHistoryTest.java:117-141) — the package-private
/// cap-1 constructor (`BoardHistory.java:42-45`).
///
/// Transcript calls 12-14: `size=1` after each add, and `restored hash=H6 … items=1`. Both
/// languages keep `board1`, by different routes: the JVM reaches `add`'s capacity gate and refuses
/// `board2` on `newScore <= worstScore` (`0.0 <= 0.0`, `:73`), while the port refuses it one line
/// earlier, at the `contains` gate. The observable outcome — size, survivor — is the same.
#[test]
fn size_cap_evicts_worst_entry() {
    let (mut board1, mut board2, scoring) = set_up();
    let mut history = BoardHistory::with_capacity(&scoring, 1);

    history.add(&mut board1);
    // `assertEquals(1, history.size(), "History should have 1 entry after first add")` (:126).
    assert_eq!(history.size(), 1);

    history.add(&mut board2);
    // `assertEquals(1, history.size(), "History size must stay at the cap")` (:133).
    assert_eq!(history.size(), 1);

    let best = history.restore_best_board().expect("assertNotNull(:137)");
    // `assertEquals(board1.getHash(), best.getHash(), …)` (:138-141).
    assert_eq!(best.structural_hash(), board1.structural_hash());
    assert_eq!(
        best.get_items().count(),
        1,
        "board1, not the 199-item board2"
    );
}

/// `restoreBoard`'s `BasicBoard.deserialize(entry.board)` (BoardHistory.java:148) is a Java
/// **serialization round trip**, so the restored board is the snapshot with every `transient` field
/// reset by `readObject` (BasicBoard.java:1388-1400) — and only those. [`Board::deep_copy`] is that
/// round trip (controller ruling AF), and this test is the field-by-field evidence.
///
/// **Preserved**, because Java's writes them: the item map (ids, geometry, nets, clearance
/// classes, fixed states), the components, the rules and the library, the bounding box — and the
/// **item-id counter**, because `board.communication` is a `public final Communication`
/// (BasicBoard.java:88) and `ItemIdGenerator` is `Serializable` with a non-`transient`
/// `lastGeneratedId` (ItemIdGenerator.java:21-25). A restored board therefore re-issues the ids the
/// discarded board burned, in both languages.
///
/// **Dropped**, because Java's are `transient`: `changedArea` (RoutingBoard.java:67),
/// `shoveFailingObstacle` (`:72`), `shoveFailingLayer` (`:73` — back to `0`, **not** the `-1` its
/// declaration-site initializer gives a fresh board, because deserialization runs no constructor;
/// the reproduced Java bug is on `Board::deep_copy`), `normalizeSuppressedNetNos`
/// (BasicBoard.java:96), `revision` (`:97`), the search-tree manager (`:94`, rebuilt by
/// reinsertion — the port clones it instead, which `board/snapshot.rs`'s module doc argues is
/// equivalent and strictly more faithful), `autorouteEngine` (RoutingBoard.java:70, outside
/// `Board` in this port by plan-6 ruling 3) and every item's `autorouteInfo` (Item.java:67).
///
/// That last one is why `deep_copy` is the right method even though its two extra steps —
/// `clearAllItemTemporaryAutorouteData` and `finishAutoroute`, which belong to
/// `RoutingBoardUndoFacade.deepCopy` rather than to a plain `deserialize` — are **not** in
/// `restoreBoard`'s Java: both are no-ops in JAVA (a deserialized board's `autorouteInfo` is
/// already null — it is `transient`). In the PORT, `deep_copy` is a structural clone, so
/// `clear_autoroute_scratch` is load-bearing (it produces the null-scratch state Java gets for
/// free); `finish_autoroute` remains empty.
#[test]
fn a_restored_board_is_javas_deserialize_round_trip() {
    let (_, mut b1, scoring) = two_boards();

    // Dirty every transient the round trip has to clear.
    b1.start_marking_changed_area();
    let first_id = b1.get_items().next().expect("a board with items").id();
    b1.shove_failing_obstacle = Some(first_id);
    b1.shove_failing_layer = 3;
    b1.normalize_suppressed_net_nos.insert(7);
    assert!(b1.changed_area.is_some());
    assert!(b1.revision() > 0, "routing bumped the revision");

    let ids_before: Vec<u32> = b1.get_items().map(|item| item.id().0).collect();
    let max_id_before = b1.communication.id_gen.max_generated_id();

    let mut history = BoardHistory::new(&scoring);
    history.add(&mut b1);
    let restored = history.restore_best_board().expect("one entry");

    // Preserved.
    assert_eq!(
        restored
            .get_items()
            .map(|item| item.id().0)
            .collect::<Vec<_>>(),
        ids_before
    );
    assert_eq!(
        restored.communication.id_gen.max_generated_id(),
        max_id_before,
        "a restored board re-issues the ids the discarded one burned"
    );
    assert_eq!(restored.get_layer_count(), b1.get_layer_count());

    // Dropped.
    assert!(restored.changed_area.is_none(), "RoutingBoard.java:67");
    assert!(restored.shove_failing_obstacle.is_none(), "`:72`");
    assert_eq!(
        restored.shove_failing_layer, 0,
        "`:73` — Java's bug, not -1"
    );
    assert!(restored.normalize_suppressed_net_nos.is_empty());
    assert_eq!(restored.revision(), 0, "BasicBoard.java:97");

    // Each restore is a fresh board, as each `deserialize` is: the second one has the transients
    // reset again and is not aliased to the first.
    let second = history.restore_best_board().expect("still one entry");
    assert!(second.changed_area.is_none());
    assert_eq!(second.structural_hash(), restored.structural_hash());
}

// =================================================================================================
// The two recorded hash divergences (controller ruling AH — Task 3 owns the fix)
// =================================================================================================

/// **XDIFF, recorded not fixed** (this task's brief: "consume `structural_hash` as it exists; if
/// you find a corpus case where hash inequality vs Java's changes a decision, report it — do not
/// fix it here").
///
/// [`Board::structural_hash`] hashes **only** `Item::Trace` and `Item::Via`
/// (`crates/fr-board/src/board/snapshot.rs:210-240`), so **every board with no trace and no via
/// hashes alike** — a 1-item empty board, a 199-item unrouted board and a 33-item unrouted board
/// all answer the same `u64`. Java's `getHash()` is an MD5 over `serialize(true)`, which writes
/// `board.itemList` — the whole item graph — and tells all three apart
/// (`p7t2-board-history.txt`: `board1 hash=H6 items=1`, `board2 hash=H7 items=199`).
///
/// It changes three `BoardHistory` decisions: `contains` answers `true` where Java answers
/// `false`, `add` refuses a board Java accepts, and `getRank` finds the wrong entry. In the real
/// pass loop the exposure is narrow — `AutorouteBatchLoop` adds one board lineage, and every pass
/// after the first has traces — but it is not nil: pass 1 of a run whose fanout inserted nothing
/// adds a trace-free board. **Task 3 (ruling AH) widens the hash to the `serialize(true)` field
/// set, and this test is one of the two it flips.**
#[test]
fn the_hash_cannot_tell_two_trace_free_boards_apart() {
    let empty = load_board(EMPTY_BOARD);
    let setonix = load_board(SETONIX);
    let rpi = build_board(RPI_SPLITTER, 0);

    assert_eq!(empty.get_items().count(), 1);
    assert_eq!(setonix.get_items().count(), 199);
    assert_eq!(rpi.get_items().count(), 33);
    for board in [&empty, &setonix, &rpi] {
        assert_eq!(board.get_traces().len(), 0);
        assert_eq!(board.get_vias().len(), 0);
    }

    assert_eq!(
        empty.structural_hash(),
        setonix.structural_hash(),
        "XDIFF: the JVM gives these H6 and H7"
    );
    assert_eq!(
        empty.structural_hash(),
        rpi.structural_hash(),
        "XDIFF: and the rpi splitter a third value again"
    );
    // `diff_traces` is ruling AH's named tie-break, and it agrees with Java that the boards are
    // not the same — but only about *traces*, which is zero for all three here.
    assert_eq!(empty.diff_traces(&setonix), 0);
}

/// **XDIFF, recorded not fixed** — the mirror image of the one above, and the reason
/// [`POOL_K`] skips `k = 2`.
///
/// Connections 2 and 3 of `Issue143-rpi_splitter.dsn` **fail** (`tests/reference/
/// router-rpi-splitter/router.jsonl`: `k=2 FAILED`, `k=3 FAILED`). A failed attempt inserts
/// nothing, so the board after connection 2 has **the same 38 items with the same 38 ids and the
/// same geometry** as the board after connection 1 — the port hashes them equal, correctly. Java
/// does not: the attempt burned item ids 52-76 and, on the way, asked pins for their centre, which
/// fills the **non-transient** `DrillItem.center` cache (`DrillItem.java:28`, lazily via
/// `Pin.getCenter`, `Pin.java:92-140`) that `serialize(true)` writes. Verified in the JVM by
/// serializing both boards and diffing the bytes: identical item lists, `8501` bytes against
/// `8566`, first difference at offset `5487`, where a `null` centre becomes a `Point`.
///
/// So Java's "board hash" moves when nothing about the board's routing state does. Quirk #200.
/// The port is the one that is right here, and Task 3's widening must **not** learn to reproduce
/// this: a hash over the item graph gives the port's answer, not the JVM's.
#[test]
fn the_hash_ignores_a_failed_pass_that_java_can_still_tell_apart() {
    let b1 = build_board(RPI_SPLITTER, 1);
    let b2 = build_board(RPI_SPLITTER, 2);

    assert_eq!(b1.get_items().count(), 38);
    assert_eq!(b2.get_items().count(), 38);
    let ids1: Vec<u32> = b1.get_items().map(|i| i.id().0).collect();
    let ids2: Vec<u32> = b2.get_items().map(|i| i.id().0).collect();
    assert_eq!(ids1, ids2, "connection 2 failed and inserted nothing");
    assert_eq!(b1.communication.id_gen.max_generated_id().0, 51);
    assert_eq!(
        b2.communication.id_gen.max_generated_id().0,
        76,
        "but it did burn ids"
    );

    assert_eq!(
        b1.structural_hash(),
        b2.structural_hash(),
        "XDIFF: the JVM gives these two different MD5s"
    );
}
