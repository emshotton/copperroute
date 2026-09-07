//! Rust twin of `scripts/differential/java/P7T10.java` (Plan 7 Task 3, controller ruling AH): the
//! three **decisions** Java makes by comparing two `BasicBoard.getHash()` values, against the
//! port's `Board::structural_hash`.
//!
//! Usage: `p7t10 <dsn> <steps> [routeK] [mode]`. `mode` is `warm` (the default) or `raw`;
//! `routeK` defaults to 0.
//!
//! Three lines per mutation step — **decisions, never hash values**, which are not comparable
//! across the two languages by construction (ruling AH, `docs/java-quirks.md` #78):
//!
//! ```text
//! FANOUTSTOP <bool>   # BatchFanout.java:152-156's currentBoardHash.equals(lastBoardHash)
//! CONTAINS   <bool>   # BoardHistory.contains (BoardHistory.java:88-101), a 5-entry history
//! RANK       <int>    # BoardHistory.getRank  (BoardHistory.java:173-186), the same history
//! ```
//!
//! **`mode` is a Java-side knob only.** It selects whether `P7T10.java` puts the jar's
//! non-`transient` *by-product* fields into a canonical state before each hash — filling
//! `DrillItem`'s four lazy caches and resetting `Item.smallestClearance`, quirk #200 — or leaves
//! them as they fall. This port has no such instability to reproduce (ruling AH's answer, argued
//! in `crates/copper-board/src/board/snapshot.rs`'s audit table), so **both modes take exactly the
//! same path here** and differ only in the header line. A `raw` diff is therefore a measurement
//! of quirk #200's exposure in the jar; a `warm` diff is a port bug.
//!
//! **The four routing choices are `P6T1.java`'s and are duplicated here on purpose**, exactly as
//! `p7t7.rs` duplicates them (Rust binaries cannot share a private module without a crate-level
//! refactor of the harness); any drift shows up as a diff, because the Java side is the
//! un-duplicated one.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufWriter, Write};
use std::time::UNIX_EPOCH;

use copper_board::prelude::*;
use copper_dsn::parser::scope_parameter::DsnReadOptions;
use copper_dsn::BoardReadResult;
use copper_geometry::{Area, IntBox, IntVector, Point, Polyline, Shape, TileShape, Vector};
use copper_router::pipeline::BoardHistory;
use copper_router::route_connection;
use copper_settings::sources::DefaultSettings;
use copper_settings::{HostEnvironment, RouterSettings, SettingsSource};

/// `P7T10.HISTORY_CAP`.
const HISTORY_CAP: usize = 5;
/// `P7T10.HALF_WIDTH`.
const HALF_WIDTH: i32 = 30;
/// `P7T10.CLEARANCE_CLASS`.
const CLEARANCE_CLASS: usize = 1;

// ------------------------------------------------------------------------------------------------
// The shared pseudo-random stream — `P7T10`'s xorshift64, drawn call for call
// ------------------------------------------------------------------------------------------------

struct Rng {
    state: u64,
}

impl Rng {
    fn next(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }

    /// `P7T10.rnd`: `(int) Long.remainderUnsigned(next(), bound)`.
    fn rnd(&mut self, bound: i32) -> i32 {
        (self.next() % (bound as u64)) as i32
    }
}

/// `P7T10.seedFor`.
fn seed_for(stem: &str, steps: i32, route_k: i32) -> u64 {
    let mut h: u64 = 0x9E37_79B9_7F4A_7C15;
    // Java's `String.charAt` is a UTF-16 code unit; every corpus stem is ASCII, but the port
    // spells it as UTF-16 anyway so a non-ASCII stem cannot make the two streams differ.
    for unit in stem.encode_utf16() {
        h = h.wrapping_mul(1_000_003).wrapping_add(u64::from(unit));
    }
    h = h.wrapping_mul(1_000_003).wrapping_add(steps as u64);
    h = h.wrapping_mul(1_000_003).wrapping_add(route_k as u64);
    if h == 0 { 0x9E37_79B9_7F4A_7C15 } else { h }
}

// ------------------------------------------------------------------------------------------------

struct Driver {
    board: Board,
    via_padstack: PadstackId,
    area: IntBox,
    snapshots: Vec<Board>,
    rng: Rng,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: p7t10 <dsn> <steps> [routeK] [mode]");
        std::process::exit(2);
    }
    let dsn = std::fs::canonicalize(&args[0])
        .unwrap_or_else(|e| panic!("cannot resolve {}: {e}", args[0]));
    let steps: i32 = args[1].parse().expect("steps");
    let route_k: i32 = args.get(2).map_or(0, |a| a.parse().expect("routeK"));
    let mode = args.get(3).map_or("warm", |m| m.as_str());
    if mode != "warm" && mode != "raw" {
        eprintln!("mode must be `warm` or `raw`");
        std::process::exit(2);
    }

    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    print_header(&mut out, &dsn, steps, route_k, mode);

    let mut board = load_board(&dsn);
    let settings = build_settings(&board);
    for connection in pick_connections(&board, route_k) {
        route_one(&mut board, &settings, &connection);
    }

    // `P7T10`'s own via padstack, appended to the board's library so the script does not depend
    // on which padstacks a fixture happens to carry. The library is outside `serialize(true)`'s
    // reach (`Item.board` is transient), so adding it moves no hash.
    let layer_count = board.get_layer_count();
    let via_shapes: Vec<Option<Shape>> = (0..layer_count)
        .map(|_| {
            Some(Shape::Tile(TileShape::Box(IntBox::from_coords(
                -70, -70, 70, 70,
            ))))
        })
        .collect();
    let via_padstack = board
        .library
        .padstacks
        .add("p7t10-via", via_shapes, true, false);

    let bbox = board.bounding_box;
    let dx = (bbox.ur.x - bbox.ll.x) / 4;
    let dy = (bbox.ur.y - bbox.ll.y) / 4;
    let area = IntBox::from_coords(bbox.ll.x + dx, bbox.ll.y + dy, bbox.ur.x - dx, bbox.ur.y - dy);

    // `P7T10.normalizeByProducts` has no twin. It does two things on the Java side — it *fills*
    // `DrillItem`'s four lazy caches and it *resets* `Item.smallestClearance`, an accumulator that
    // never resets by itself and is the load-bearing half (the method is named for the pair; see
    // `P7T10.java`). Neither has anything to do here: the port's hash reads none of those fields
    // (the audit table's skipped rows in `crates/copper-board/src/board/snapshot.rs`), so there is no
    // by-product to canonicalise.

    let stem = dsn
        .file_name()
        .expect("a file name")
        .to_string_lossy()
        .into_owned();
    let mut driver = Driver {
        board,
        via_padstack,
        area,
        snapshots: Vec::new(),
        rng: Rng {
            state: seed_for(&stem, steps, route_k),
        },
    };

    let scoring = settings
        .scoring
        .clone()
        .expect("DefaultSettings always writes a scoring block");
    let mut history = BoardHistory::with_capacity(&scoring, HISTORY_CAP);

    // A debugging aid, off in every committed run: with `P7T10_STATE=1` both sides also print a
    // per-step board fingerprint, so a decision diff can be told apart from a board diff.
    let with_state = std::env::var("P7T10_STATE").is_ok_and(|v| v == "1");

    let mut last_hash: Option<u64> = None;
    for step in 1..=steps {
        driver.mutate();

        if with_state {
            writeln!(
                out,
                "STATE items={} traces={} vias={}",
                driver.board.get_items().count(),
                driver.board.get_traces().len(),
                driver.board.get_vias().len()
            )
            .expect("write");
        }

        // BatchFanout.java:152-156.
        let current = driver.board.structural_hash();
        writeln!(out, "FANOUTSTOP {}", Some(current) == last_hash).expect("write");
        last_hash = Some(current);

        writeln!(out, "CONTAINS {}", history.contains(&driver.board)).expect("write");
        writeln!(out, "RANK {}", history.rank(&driver.board)).expect("write");

        if step <= HISTORY_CAP as i32 {
            history.add(&mut driver.board);
            let copy = driver.board.deep_copy();
            driver.snapshots.push(copy);
        }
    }
    out.flush().expect("flush");
}

// ------------------------------------------------------------------------------------------------
// Header, board, settings — `P6T1.java`'s choices, transcribed (see the module comment)
// ------------------------------------------------------------------------------------------------

fn print_header<W: Write>(out: &mut W, dsn: &std::path::Path, steps: i32, route_k: i32, mode: &str) {
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
        "HEADER jar={} bytes={} mtime={mtime} fixture={} steps={steps} routeK={route_k} mode={mode}",
        jar.display(),
        meta.len(),
        dsn.file_name().expect("a file name").to_string_lossy(),
    )
    .expect("write");
}

/// `P6T1.loadBoard`, without the `.rules` slot this driver does not take.
fn load_board(dsn: &std::path::Path) -> Board {
    let file = std::fs::File::open(dsn).unwrap_or_else(|e| panic!("cannot open {dsn:?}: {e}"));
    let design_name = dsn
        .file_name()
        .expect("a file name")
        .to_string_lossy()
        .into_owned();
    let result = copper_dsn::read_board(file, None, Some(&design_name), &DsnReadOptions::default());
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

/// `P6T1.pickConnections`, with `p7t7.rs`'s own `route_k == 0` guard.
fn pick_connections(board: &Board, max_items: i32) -> Vec<Connection> {
    let mut result = Vec::new();
    if max_items <= 0 {
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
            if result.len() >= max_items as usize {
                return result;
            }
        }
    }
    result
}

/// `P7T7`'s routing loop, i.e. `P6T1.route` with `ripupPassNo = 1`.
fn route_one(board: &mut Board, settings: &RouterSettings, connection: &Connection) {
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
        1,
        settings.get_start_ripup_costs(),
        !settings.is_fanout_enabled(),
        false,
        &|| false,
    );
}

// ------------------------------------------------------------------------------------------------
// The mutation script — `P7T10.mutate`, draw for draw
// ------------------------------------------------------------------------------------------------

impl Driver {
    fn mutate(&mut self) {
        let roll = self.rng.rnd(100);
        if roll < 22 {
            self.insert_trace();
        } else if roll < 40 {
            self.remove_trace();
        } else if roll < 56 {
            self.insert_via();
        } else if roll < 68 {
            self.move_via();
        } else if roll < 78 {
            self.noop();
        } else if roll < 88 {
            self.insert_obstacle();
        } else if roll < 96 {
            self.restore_snapshot();
        } else {
            self.refix_item();
        }
    }

    /// `P7T10.noop` — two draws, so a no-op still advances the stream.
    fn noop(&mut self) {
        self.rng.rnd(2);
        self.rng.rnd(2);
    }

    fn rand_x(&mut self) -> i32 {
        self.area.ll.x + self.rng.rnd((self.area.ur.x - self.area.ll.x).max(1))
    }

    fn rand_y(&mut self) -> i32 {
        self.area.ll.y + self.rng.rnd((self.area.ur.y - self.area.ll.y).max(1))
    }

    fn rand_layer(&mut self) -> usize {
        self.rng.rnd(self.board.get_layer_count() as i32) as usize
    }

    fn rand_net(&mut self) -> i32 {
        1 + self.rng.rnd(2)
    }

    fn insert_trace(&mut self) {
        let layer = self.rand_layer();
        let net_no = self.rand_net();
        let x1 = self.rand_x();
        let y1 = self.rand_y();
        let x2 = self.rand_x();
        let y2 = self.rand_y();
        if x1 == x2 && y1 == y2 {
            return;
        }
        self.board.insert_trace_without_cleaning(
            Polyline::from_points(&[Point::new(x1, y1), Point::new(x2, y2)]),
            layer,
            HALF_WIDTH,
            vec![net_no],
            CLEARANCE_CLASS,
            FixedState::Unfixed,
        );
    }

    fn remove_trace(&mut self) {
        // `get_traces()` is descending item id (quirk #63), as Java's `getTraces()` is.
        let traces = self.board.get_traces();
        if traces.is_empty() {
            self.rng.rnd(2);
            return;
        }
        let index = self.rng.rnd(traces.len() as i32) as usize;
        self.board.remove_item(traces[index]);
    }

    fn insert_via(&mut self) {
        let x = self.rand_x();
        let y = self.rand_y();
        let net_no = self.rand_net();
        let attach = self.rng.rnd(2) == 0;
        // `insert_via` splits every trace it lands on (BasicBoard.java:287-293) and a split can
        // answer `Err` where Java throws quirk #22's `ArrayIndexOutOfBoundsException`; both sides
        // swallow it and carry on with the part-split board Java would have.
        let _ = self.board.insert_via(
            self.via_padstack,
            Point::new(x, y),
            vec![net_no],
            CLEARANCE_CLASS,
            FixedState::Unfixed,
            attach,
        );
    }

    fn move_via(&mut self) {
        let vias = self.board.get_vias();
        if vias.is_empty() {
            self.rng.rnd(2);
            self.rng.rnd(2);
            self.rng.rnd(2);
            return;
        }
        let index = self.rng.rnd(vias.len() as i32) as usize;
        let via = vias[index];
        let mx = self.rng.rnd(201) - 100;
        let my = self.rng.rnd(201) - 100;
        let vector: Vector = IntVector::new(mx, my).into();
        let _ = self.board.move_item_by(via, &vector);
    }

    fn insert_obstacle(&mut self) {
        let layer = self.rand_layer();
        let x = self.rand_x();
        let y = self.rand_y();
        let w = 100 + self.rng.rnd(900);
        self.board.insert_obstacle(
            Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
                x,
                y,
                x + w,
                y + w,
            )))),
            layer,
            CLEARANCE_CLASS,
            FixedState::Unfixed,
        );
    }

    fn restore_snapshot(&mut self) {
        if self.snapshots.is_empty() {
            self.rng.rnd(2);
            return;
        }
        let index = self.rng.rnd(self.snapshots.len() as i32) as usize;
        // `BoardHistoryEntry` stores `board.serialize(false)` and `restoreBoard` answers
        // `BasicBoard.deserialize` of it (BoardHistory.java:148); `deep_copy` is that round trip,
        // so the restored board is hash-identical to the entry the history holds.
        self.board = self.snapshots[index].deep_copy();
    }

    fn refix_item(&mut self) {
        let items = self.board.items_in_board_order();
        if items.is_empty() {
            self.rng.rnd(2);
            self.rng.rnd(2);
            return;
        }
        let index = self.rng.rnd(items.len() as i32) as usize;
        let id = items[index];
        let unfixed = self.rng.rnd(2) == 0;
        if let Some(item) = self.board.get_item_mut(id) {
            item.header_mut().set_fixed_state(if unfixed {
                FixedState::Unfixed
            } else {
                FixedState::ShoveFixed
            });
        }
    }
}
