//! T17: read-only instrumentation for quirk #193's three stale tree-index guards.
//!
//! Quirk #193 (`docs/java-quirks.md`) is three HEAD-only guards that each admit, in their own
//! comment, that an item's tree-shape indices can go stale **while the maze search is running**:
//!
//! | tag | Java | port |
//! |-----|------|------|
//! | `G1a` | `MazeSearchEngine.java:653-659` — `treeEntryNo` past the item's tree-shape count | [`crate::autoroute::maze::MazeSearchEngine::expand_to_target_doors`] |
//! | `G1b` | `MazeSearchEngine.java:660-668` — `getTraceConnectionShape` answers `null` | the same function, one `continue` later |
//! | `G2` | `ItemAutorouteInfo.java:57-79` — `expansionRoomArr` is **resized** mid-search, then an index past it returns `null` | [`crate::autoroute::item_info::get_expansion_room`] |
//! | `G3` | `MazeTraceShover.java:62-67` — a `traceCornerNo` beyond `lines.length - 2` | [`crate::autoroute::maze::MazeTraceShover::check_shove_trace_line`] |
//!
//! `G1a`/`G1b` are one Java guard site with two exits; they are counted separately because they
//! answer different questions (a *shrunk* shape array versus a shape that exists but has no trace
//! connection shape).
//!
//! # This module changes nothing
//!
//! Every recorder is a pure read behind [`on`], which is a process-lifetime `bool` read from the
//! `P9T17_STALE` environment variable — the same shape as
//! [`crate::autoroute::maze::queue::p7t14b_maze_ledger`]'s `P7T14B_MAZE` ledger gate (quirk #229).
//! With the variable unset every recorder is a single `LazyLock` deref and a branch, and no guard
//! decision reads anything this module holds. `instrumentation_changes_no_board_byte`
//! (`crates/fr-router/tests/stale_index.rs`) is the gate that keeps it that way.
//!
//! # Why the counters are not fields on `RouterCounters`
//!
//! Task 17's brief names `fr_router::pipeline::RouterCounters` as the place to hang them. It is
//! not available: that type is a **byte-parity DTO** whose nine fields are asserted against the
//! JVM's reflected declaration order by
//! `crates/fr-router/tests/stop_and_progress.rs::router_counters_field_list_matches_java`, and a
//! tenth field would fail that assertion and change the progress payload. The counters live here
//! instead, in a module Java has no counterpart for, and nothing outside the tests and the
//! report generator reads them.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

/// The four guard exits, in the order the report tabulates them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Guard {
    /// `MazeSearchEngine.java:653-659` — `treeEntryNo >= item.treeShapeCount(tree)`.
    G1aTreeEntryOutOfRange = 0,
    /// `MazeSearchEngine.java:660-668` — `getTraceConnectionShape` answered `null`.
    G1bNullConnectionShape = 1,
    /// `ItemAutorouteInfo.java:59-66` — the array was **reallocated** over a live prefix.
    G2RoomArrayResized = 2,
    /// `ItemAutorouteInfo.java:68-76` — the index is past the (possibly just resized) array.
    G2RoomIndexOutOfRange = 3,
    /// `MazeTraceShover.java:62-67` — `traceCornerNo > lines.length - 2`.
    G3TraceCornerOutOfRange = 4,
}

impl Guard {
    /// The tag the report and the tests use.
    pub const ALL: [Guard; 5] = [
        Guard::G1aTreeEntryOutOfRange,
        Guard::G1bNullConnectionShape,
        Guard::G2RoomArrayResized,
        Guard::G2RoomIndexOutOfRange,
        Guard::G3TraceCornerOutOfRange,
    ];

    /// The short name used in the report tables.
    pub fn tag(self) -> &'static str {
        match self {
            Guard::G1aTreeEntryOutOfRange => "G1a-tree-entry-out-of-range",
            Guard::G1bNullConnectionShape => "G1b-null-connection-shape",
            Guard::G2RoomArrayResized => "G2-room-array-resized",
            Guard::G2RoomIndexOutOfRange => "G2-room-index-out-of-range",
            Guard::G3TraceCornerOutOfRange => "G3-trace-corner-out-of-range",
        }
    }
}

/// The board mutations the router performs, tagged at the site so a guard fire can name the last
/// one that preceded it. `None` is "nothing has mutated the board since the counters were reset".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Mutation {
    /// No router mutation has been recorded yet.
    None = 0,
    /// `MazeSearchEngine.reduceTraceShapesAtTiePins` (`:970-971`, `:1210-1276`) — the only board
    /// write **inside** `init`, and the one that shortens a trace under a live room array.
    TiePinReduction = 1,
    /// `AutorouteEngine.autorouteConnection:260` — `removeItems` over the ripped connections.
    RipupRemoveItems = 2,
    /// `AutorouteEngine.autorouteConnection:262-263` — `removeTraceTails` per changed net.
    RemoveTraceTails = 3,
    /// `FoundConnectionInserter` — `insertForcedTracePolyline` and the pull-tights around it.
    ForcedTraceInsert = 4,
    /// `optChangedArea` — the post-insert pull-tight sweep.
    OptChangedArea = 5,
}

impl Mutation {
    const COUNT: usize = 6;

    /// The tag the report tables use.
    pub fn tag(self) -> &'static str {
        match Mutation::from_u8(self as u8) {
            Mutation::None => "none-yet",
            Mutation::TiePinReduction => "reduceTraceShapesAtTiePins",
            Mutation::RipupRemoveItems => "removeItems (ripup)",
            Mutation::RemoveTraceTails => "removeTraceTails",
            Mutation::ForcedTraceInsert => "insertForcedTracePolyline",
            Mutation::OptChangedArea => "optChangedArea",
        }
    }

    fn from_u8(value: u8) -> Mutation {
        match value {
            1 => Mutation::TiePinReduction,
            2 => Mutation::RipupRemoveItems,
            3 => Mutation::RemoveTraceTails,
            4 => Mutation::ForcedTraceInsert,
            5 => Mutation::OptChangedArea,
            _ => Mutation::None,
        }
    }
}

/// Whether the instrumentation is on. Process-lifetime, read once from `P9T17_STALE`.
///
/// Off is the default and the shipped configuration: the differential harness, the reference
/// generators and the test suite all run without the variable, so no committed byte depends on it.
pub fn on() -> bool {
    static ON: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| std::env::var_os("P9T17_STALE").is_some());
    *ON || force_on()
}

/// The in-process override the tests use, because `P9T17_STALE` is read once per process and a
/// test binary cannot set it before its own `LazyLock` runs deterministically.
fn force_on() -> bool {
    FORCED.load(Ordering::Relaxed)
}

static FORCED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Turns the instrumentation on for this process. Test-facing; the binary uses `P9T17_STALE`.
pub fn set_on(value: bool) {
    FORCED.store(value, Ordering::Relaxed);
}

static FIRES: [AtomicU64; 5] = [const { AtomicU64::new(0) }; 5];

/// How many times each guard's **test** was evaluated, fired or not.
///
/// Without this a zero in [`FIRES`] is ambiguous between "the guard was reached and never tripped"
/// and "the guard is on a path the corpus does not walk", and those are opposite findings. Every
/// guard site records a visit immediately before its test.
static VISITS: [AtomicU64; 5] = [const { AtomicU64::new(0) }; 5];

/// `guard × preceding mutation` — flattened `[Guard::ALL.len() * Mutation::COUNT]`.
static FIRES_BY_MUTATION: [AtomicU64; 5 * Mutation::COUNT] = [const { AtomicU64::new(0) }; 30];

static MUTATIONS: [AtomicU64; Mutation::COUNT] = [const { AtomicU64::new(0) }; 6];

static LAST_MUTATION: AtomicU8 = AtomicU8::new(0);

/// Whether the stale index was **recoverable** — the shape the door names still exists under a
/// different index — or **lost**. See [`record_guard`]'s `delta` argument.
static RECOVERABLE: [AtomicU64; 5] = [const { AtomicU64::new(0) }; 5];

/// The first [`SAMPLE_LIMIT`] fire records, verbatim, for the report's worked examples.
static SAMPLES: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// How many fire records the sample buffer keeps. Bounded so a long batch run cannot grow it.
pub const SAMPLE_LIMIT: usize = 64;

/// Tags a board mutation the router just performed. Call **after** the mutation lands.
pub fn note_mutation(mutation: Mutation) {
    if !on() {
        return;
    }
    LAST_MUTATION.store(mutation as u8, Ordering::Relaxed);
    MUTATIONS[mutation as usize].fetch_add(1, Ordering::Relaxed);
}

/// Records that a guard's test was **evaluated**. Call at the site, before the test, on every
/// pass — the denominator a fire count is a fraction of.
pub fn record_visit(guard: Guard) {
    if !on() {
        return;
    }
    VISITS[guard as usize].fetch_add(1, Ordering::Relaxed);
}

/// Records one guard fire.
///
/// `index` is the index the caller asked for and `available` is what the item currently has, so
/// `index >= available` is the staleness itself. `recoverable` is the investigator's verdict at
/// the site: `true` when the datum the guard refused could be re-derived from the item as it now
/// stands (the item is still on the board and still has shapes), `false` when it is genuinely
/// gone (the item has no shapes at all, or is no longer on the board).
pub fn record_guard(guard: Guard, item: u64, index: usize, available: usize, recoverable: bool) {
    if !on() {
        return;
    }
    let slot = guard as usize;
    FIRES[slot].fetch_add(1, Ordering::Relaxed);
    let last = LAST_MUTATION.load(Ordering::Relaxed);
    FIRES_BY_MUTATION[slot * Mutation::COUNT + last as usize].fetch_add(1, Ordering::Relaxed);
    if recoverable {
        RECOVERABLE[slot].fetch_add(1, Ordering::Relaxed);
    }
    if let Ok(mut samples) = SAMPLES.lock()
        && samples.len() < SAMPLE_LIMIT
    {
        samples.push(format!(
            "{} item={item} index={index} available={available} recoverable={recoverable} after={}",
            guard.tag(),
            Mutation::from_u8(last).tag(),
        ));
    }
}

/// One guard's row of the report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardRow {
    /// Which guard.
    pub guard: Guard,
    /// How many times its test was evaluated.
    pub visits: u64,
    /// How many times it fired.
    pub fires: u64,
    /// How many of those fires the site judged recoverable.
    pub recoverable: u64,
    /// `(mutation, count)` for every mutation that has preceded at least one fire.
    pub by_mutation: Vec<(Mutation, u64)>,
}

/// The whole counter set, read out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// One row per guard, in [`Guard::ALL`] order.
    pub guards: Vec<GuardRow>,
    /// How many of each mutation the router performed.
    pub mutations: Vec<(Mutation, u64)>,
    /// The first [`SAMPLE_LIMIT`] fire records.
    pub samples: Vec<String>,
}

impl Snapshot {
    /// The total number of guard fires.
    pub fn total_fires(&self) -> u64 {
        self.guards.iter().map(|row| row.fires).sum()
    }

    /// The fire count of one guard.
    pub fn fires(&self, guard: Guard) -> u64 {
        self.guards
            .iter()
            .find(|row| row.guard == guard)
            .map_or(0, |row| row.fires)
    }

    /// How many times one guard's test was evaluated.
    pub fn visits(&self, guard: Guard) -> u64 {
        self.guards
            .iter()
            .find(|row| row.guard == guard)
            .map_or(0, |row| row.visits)
    }
}

/// Reads the counters out without clearing them.
pub fn snapshot() -> Snapshot {
    let guards = Guard::ALL
        .iter()
        .map(|guard| {
            let slot = *guard as usize;
            let by_mutation = (0..Mutation::COUNT)
                .map(|m| {
                    (
                        Mutation::from_u8(u8::try_from(m).unwrap_or(0)),
                        FIRES_BY_MUTATION[slot * Mutation::COUNT + m].load(Ordering::Relaxed),
                    )
                })
                .filter(|(_, count)| *count > 0)
                .collect();
            GuardRow {
                guard: *guard,
                visits: VISITS[slot].load(Ordering::Relaxed),
                fires: FIRES[slot].load(Ordering::Relaxed),
                recoverable: RECOVERABLE[slot].load(Ordering::Relaxed),
                by_mutation,
            }
        })
        .collect();
    let mutations = (0..Mutation::COUNT)
        .map(|m| {
            (
                Mutation::from_u8(u8::try_from(m).unwrap_or(0)),
                MUTATIONS[m].load(Ordering::Relaxed),
            )
        })
        .filter(|(_, count)| *count > 0)
        .collect();
    let samples = SAMPLES.lock().map(|s| s.clone()).unwrap_or_default();
    Snapshot {
        guards,
        mutations,
        samples,
    }
}

/// Clears every counter. The tests call it between cases; the binary calls it once at start-up.
pub fn reset() {
    for slot in 0..Guard::ALL.len() {
        FIRES[slot].store(0, Ordering::Relaxed);
        VISITS[slot].store(0, Ordering::Relaxed);
        RECOVERABLE[slot].store(0, Ordering::Relaxed);
        for m in 0..Mutation::COUNT {
            FIRES_BY_MUTATION[slot * Mutation::COUNT + m].store(0, Ordering::Relaxed);
        }
    }
    for counter in &MUTATIONS {
        counter.store(0, Ordering::Relaxed);
    }
    LAST_MUTATION.store(0, Ordering::Relaxed);
    if let Ok(mut samples) = SAMPLES.lock() {
        samples.clear();
    }
}

/// Renders the snapshot as the report's per-stem block.
pub fn render(stem: &str, snapshot: &Snapshot) -> String {
    let mut out = format!("=== stem {stem} ===\n");
    for row in &snapshot.guards {
        out.push_str(&format!(
            "{:<30} visits={:<10} fires={:<8} recoverable={:<8} by-mutation={}\n",
            row.guard.tag(),
            row.visits,
            row.fires,
            row.recoverable,
            if row.by_mutation.is_empty() {
                "-".to_string()
            } else {
                row.by_mutation
                    .iter()
                    .map(|(m, c)| format!("{}={c}", m.tag()))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        ));
    }
    out.push_str("mutations ");
    if snapshot.mutations.is_empty() {
        out.push('-');
    } else {
        out.push_str(
            &snapshot
                .mutations
                .iter()
                .map(|(m, c)| format!("{}={c}", m.tag()))
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    out.push('\n');
    for sample in &snapshot.samples {
        out.push_str("sample ");
        out.push_str(sample);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The counters are process-global, so the two tests below cannot share a process without
    /// serialising. `cargo nextest` gives each test its own; this keeps `cargo test` honest too.
    static SERIAL: Mutex<()> = Mutex::new(());

    #[test]
    fn a_recorder_is_inert_while_the_instrumentation_is_off() {
        let _serial = SERIAL.lock();
        // T17: the byte-identity property in miniature — with the gate off nothing is written.
        set_on(false);
        reset();
        record_guard(Guard::G1aTreeEntryOutOfRange, 7, 3, 1, false);
        note_mutation(Mutation::TiePinReduction);
        assert_eq!(snapshot().total_fires(), 0);
        assert!(snapshot().mutations.is_empty());
    }

    #[test]
    fn a_fire_is_attributed_to_the_last_mutation() {
        let _serial = SERIAL.lock();
        set_on(true);
        reset();
        note_mutation(Mutation::TiePinReduction);
        record_guard(Guard::G1aTreeEntryOutOfRange, 7, 3, 1, false);
        note_mutation(Mutation::RipupRemoveItems);
        record_guard(Guard::G1aTreeEntryOutOfRange, 8, 4, 2, true);
        let snapshot = snapshot();
        set_on(false);
        assert_eq!(snapshot.fires(Guard::G1aTreeEntryOutOfRange), 2);
        let row = &snapshot.guards[0];
        assert_eq!(row.recoverable, 1);
        assert_eq!(
            row.by_mutation,
            vec![
                (Mutation::TiePinReduction, 1),
                (Mutation::RipupRemoveItems, 1)
            ]
        );
    }
}
