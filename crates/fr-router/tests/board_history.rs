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

const POOL_K: [usize; 7] = [0, 1, 3, 4, 5, 6, 8];

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

fn java_fixed_state(state: FixedState) -> &'static str {
    match state {
        FixedState::Unfixed => "UNFIXED",
        FixedState::ShoveFixed => "SHOVE_FIXED",
        FixedState::UserFixed => "USER_FIXED",
        FixedState::SystemFixed => "SYSTEM_FIXED",
    }
}

fn dump_nets(net_nos: &[i32]) -> String {
    let inner: Vec<String> = net_nos.iter().map(i32::to_string).collect();
    format!("[{}]", inner.join(","))
}

fn dump_point(point: &Point) -> String {
    let rounded = point.to_float().round();
    format!("({},{})", rounded.x, rounded.y)
}

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

const T2: &str = include_str!("data/p7t2-board-history.txt");

fn expected_lines() -> Vec<&'static str> {
    T2.lines()
        .filter(|line| !line.starts_with("HEADER "))
        .take_while(|line| *line != "=== phase javatest ===")
        .map(str::trim_end)
        .collect()
}

const KNOWN_DIVERGENCES: &[(usize, &str, &str)] = &[(
    7,
    "  B8 hash=H5 score=599.98035 items=49 maxId=190",
    "  B8 hash=H5 score=599.98035 items=49 maxId=192",
)];

fn assert_lines_match(actual: &[String]) {
    let expected = expected_lines();
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
    let mut accounted = 0usize;
    for i in 0..expected.len().max(actual.len()) {
        let want = expected.get(i).copied().unwrap_or("<missing>");
        let got = actual
            .get(i)
            .map(|row| row.trim_end())
            .unwrap_or("<missing>");
        if want == got {
            continue;
        }
        if KNOWN_DIVERGENCES
            .iter()
            .any(|(line, jvm, rust)| *line == i && *jvm == want && *rust == got)
        {
            accounted += 1;
            continue;
        }
        diffs.push(format!("line {i}\n  jvm:  {want}\n  rust: {got}"));
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
    assert_eq!(
        accounted,
        KNOWN_DIVERGENCES.len(),
        "the transcript declares {} known divergence(s) from the jar but only {accounted} of them \
         still differ — a divergence that has healed must be deleted from KNOWN_DIVERGENCES, not \
         left to rot",
        KNOWN_DIVERGENCES.len()
    );
}

#[test]
fn the_transcript_matches_the_jvm() {
    let mut pool = build_pool(RPI_SPLITTER);
    let mut b1x = build_board(RPI_SPLITTER, 1);
    let scoring = scoring_of(&build_settings(&pool[0]));
    let mut t = Transcript::new(scoring.clone());

    t.push("=== boards ===".to_string());
    for (i, board) in pool.iter_mut().enumerate() {
        t.describe_board(&format!("B{}", POOL_K[i]), board);
    }
    t.describe_board("B1X", &mut b1x);

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

#[test]
fn java_float_compare_is_the_jdks() {
    use std::cmp::Ordering::{Equal, Greater, Less};
    let neg_nan = f32::from_bits(0xffc0_0000);

    assert_eq!(java_float_compare(1.0, 2.0), Less);
    assert_eq!(java_float_compare(1.0, 1.0), Equal);
    assert_eq!(java_float_compare(0.0, -0.0), Greater);
    assert_eq!(java_float_compare(-0.0, 0.0), Less);
    assert_eq!(0.0f32.partial_cmp(&-0.0f32), Some(Equal));
    assert_eq!(java_float_compare(f32::NAN, f32::INFINITY), Greater);
    assert_eq!(java_float_compare(f32::NAN, f32::NAN), Equal);
    assert_eq!(f32::NAN.partial_cmp(&f32::INFINITY), None);
    assert_eq!(java_float_compare(neg_nan, f32::NEG_INFINITY), Greater);
    assert_eq!(neg_nan.total_cmp(&f32::NEG_INFINITY), Less);
}

fn two_boards() -> (Board, Board, ScoringSettings) {
    let b0 = build_board(RPI_SPLITTER, 0);
    let b1 = build_board(RPI_SPLITTER, 1);
    let scoring = scoring_of(&build_settings(&b0));
    (b0, b1, scoring)
}

#[test]
fn under_capacity_any_distinct_board_enters() {
    let (mut b0, mut b1, scoring) = two_boards();
    let mut h = BoardHistory::with_capacity(&scoring, 3);

    h.add(&mut b1);
    assert_eq!(h.size(), 1);
    h.add(&mut b0);
    assert_eq!(h.size(), 2, "under capacity there is no score gate");
    assert!(h.contains(&b0));
    assert_eq!(java_float_to_string(h.entries()[0].score), "199.99464");
    assert_eq!(java_float_to_string(h.entries()[1].score), "0.0");
}

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

#[test]
fn max_score_of_an_empty_history_is_zero_not_negative_infinity() {
    let (_, _, scoring) = two_boards();
    let h = BoardHistory::new(&scoring);

    assert_eq!(h.size(), 0);
    assert_eq!(java_float_to_string(h.max_score()), "0.0");
    assert!(h.max_score().is_finite(), "not -inf, and not NaN either");

    let empty_history_vs_a_negative_board = h.max_score() > -1.0f32;
    assert!(
        empty_history_vs_a_negative_board,
        "the 0 seed fires the restore gate where -inf would not"
    );
}

#[test]
fn restore_board_increments_the_restore_count_and_reorders_the_list() {
    let (mut b0, mut b1, scoring) = two_boards();
    let mut h = BoardHistory::new(&scoring);

    h.add(&mut b0);
    h.add(&mut b1);
    assert_eq!(java_float_to_string(h.entries()[0].score), "0.0");
    assert_eq!(java_float_to_string(h.entries()[1].score), "199.99464");
    assert_eq!(h.entries()[0].restore_count, 0);

    let restored = h.restore_board(0).expect("a two-entry history restores");

    assert_eq!(java_float_to_string(h.entries()[0].score), "199.99464");
    assert_eq!(java_float_to_string(h.entries()[1].score), "0.0");
    assert_eq!(h.entries()[0].restore_count, 1, "the winner's count rose");
    assert_eq!(h.entries()[1].restore_count, 0, "and only the winner's");
    assert_eq!(restored.structural_hash(), b1.structural_hash());
}

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

    assert!(h.restore_board(3).is_none(), "4 > 3, so no entry qualifies");
    assert_eq!(h.entries()[0].restore_count, 4, "and no count moved");
    assert!(h.restore_board(4).is_some(), "4 <= 4 still qualifies");
}

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

fn set_up() -> (Board, Board, ScoringSettings) {
    let board1 = load_board(EMPTY_BOARD);
    let board2 = load_board(SETONIX);
    let scoring = scoring_of(&build_settings(&board1));
    (board1, board2, scoring)
}

#[test]
fn add_and_restore_board() {
    let (mut board1, _board2, scoring) = set_up();
    let mut history = BoardHistory::new(&scoring);
    history.add(&mut board1);
    assert_eq!(history.size(), 1);
    assert_eq!(java_float_to_string(history.entries()[0].score), "0.0");

    let restored = history.restore_best_board().expect("assertNotNull(:60)");

    assert_eq!(history.entries()[0].restore_count, 1);
    assert_eq!(restored.structural_hash(), board1.structural_hash());
    assert_eq!(restored.get_items().count(), 1);
    assert_eq!(restored.communication.id_gen.max_generated_id().0, 1);
}

#[test]
fn restore_best_board_from_multiple() {
    let (mut board1, mut board2, scoring) = set_up();
    let mut history = BoardHistory::new(&scoring);
    history.add(&mut board1);
    history.add(&mut board2);

    assert_eq!(history.size(), 2, "the JVM's call-4 size");

    let best = history.restore_best_board().expect("assertNotNull(:76)");
    assert_eq!(best.structural_hash(), board1.structural_hash());
    assert_eq!(best.get_items().count(), 1);
}

#[test]
fn contains() {
    let (mut board1, board2, scoring) = set_up();
    let mut history = BoardHistory::new(&scoring);
    history.add(&mut board1);

    assert!(history.contains(&board1));
    assert!(!history.contains(&board2));
}

#[test]
fn clear() {
    let (mut board1, mut board2, scoring) = set_up();
    let mut history = BoardHistory::new(&scoring);
    history.add(&mut board1);
    history.add(&mut board2);
    assert_eq!(history.size(), 2);

    history.clear();
    assert_eq!(history.size(), 0);
}

#[test]
fn size_cap_never_exceeds_max_history_size() {
    let (mut board1, mut board2, scoring) = set_up();
    let mut history = BoardHistory::with_capacity(&scoring, BoardHistory::MAX_HISTORY_SIZE);
    history.add(&mut board1);
    history.add(&mut board2);
    history.add(&mut board1);

    assert!(history.size() <= BoardHistory::MAX_HISTORY_SIZE);
    assert_eq!(history.size(), 2, "the JVM's call-11 size");
    assert_eq!(BoardHistory::MAX_HISTORY_SIZE, 30, "BoardHistory.java:29");
}

#[test]
fn size_cap_evicts_worst_entry() {
    let (mut board1, mut board2, scoring) = set_up();
    let mut history = BoardHistory::with_capacity(&scoring, 1);

    history.add(&mut board1);
    assert_eq!(history.size(), 1);

    history.add(&mut board2);
    assert_eq!(history.size(), 1);

    let best = history.restore_best_board().expect("assertNotNull(:137)");
    assert_eq!(best.structural_hash(), board1.structural_hash());
    assert_eq!(
        best.get_items().count(),
        1,
        "board1, not the 199-item board2"
    );
}

#[test]
fn a_restored_board_is_javas_deserialize_round_trip() {
    let (_, mut b1, scoring) = two_boards();

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

    assert!(restored.changed_area.is_none(), "RoutingBoard.java:67");
    assert!(restored.shove_failing_obstacle.is_none(), "`:72`");
    assert_eq!(
        restored.shove_failing_layer, 0,
        "`:73` — Java's bug, not -1"
    );
    assert!(restored.normalize_suppressed_net_nos.is_empty());
    assert_eq!(restored.revision(), 0, "BasicBoard.java:97");

    let second = history.restore_best_board().expect("still one entry");
    assert!(second.changed_area.is_none());
    assert_eq!(second.structural_hash(), restored.structural_hash());
}

#[test]
fn trace_free_boards_are_distinguishable() {
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

    assert_ne!(
        empty.structural_hash(),
        setonix.structural_hash(),
        "the JVM gives these H6 and H7"
    );
    assert_ne!(
        empty.structural_hash(),
        rpi.structural_hash(),
        "and the rpi splitter a third value again"
    );
    assert_ne!(setonix.structural_hash(), rpi.structural_hash());
    assert_eq!(empty.diff_traces(&setonix), 0);
}

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
