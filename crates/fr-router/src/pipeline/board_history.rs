//! `autoroute/BoardHistory.java` (202 lines) — the ranked, capped cache of board snapshots the
//! pass loop's best-board policy restores from (controller ruling AF).

use fr_board::prelude::Board;
use fr_settings::ScoringSettings;

use crate::score::BoardStatistics;

/// Port of `autoroute.BoardHistory` (BoardHistory.java:22-201): the ranked, capped cache of board
/// snapshots that `AutorouteBatchLoop`'s best-board policy restores from.
///
/// **Controller ruling AF** ports it; plan-6 ruling 13 had rostered it `// not ported:` on spec
/// §2's "undo store" exclusion, which it is not — it is the pass loop's best-board memory, and
/// without it the board written to SES is the *last* pass's rather than the *best* pass's
/// (`AutorouteBatchLoop.java:525-547`).
///
/// # What is not ported
///
/// Java's `synchronized` methods and its `ReentrantReadWriteLock` (`:34`) are dropped: the whole
/// pipeline is single-threaded (survey §3.4, quirk #216 — `-mt` is dead on every path), so there
/// is no second thread for them to exclude. Rust's `&mut self` / `&self` split says the same
/// thing at compile time, and it says it in the same places: every Java method that takes the
/// write lock (or is `synchronized` and mutates) is `&mut self` here, and every read-lock method
/// is `&self` — **except `restoreBoard`**, which mutates under a *read* lock and so has to be
/// `&mut self` (quirk #198).
// not ported: `BoardHistory.rwLock` (BoardHistory.java:34) and the `synchronized` modifiers — the pipeline is single-threaded.
#[derive(Debug, Clone)]
pub struct BoardHistory {
    /// Java `boards` (`:32`), a `Collections.synchronizedList(new ArrayList<>())`.
    ///
    /// A plain [`Vec`], and it must stay one: `restoreBoard` sorts it **in place** (`:143`), so
    /// the order is state, not an implementation detail — `getRank` reads it (quirk #198).
    boards: Vec<BoardHistoryEntry>,
    /// Java `maxHistorySize` (`:31`).
    max_history_size: usize,
    /// Java `scoringSettings` (`:33`).
    scoring: ScoringSettings,
}

/// Port of the **private nested** `BoardHistory.BoardHistoryEntry` (BoardHistory.java:188-201) —
/// the one that is actually used.
///
/// `autoroute/BoardHistoryEntry.java`'s public top-level class of the same name is **shadowed and
/// dead** (quirk #199): `BoardHistory` declares this nested class at `:188`, which shadows the
/// import, so the top-level class's live `RoutingBoard`, `BoardStatistics`, `Instant.now()` and
/// `Comparable` are reachable from nothing. It is rostered `// not ported:` in
/// `crates/fr-router/src/lib.rs`.
// renamed: the nested `BoardHistoryEntry(RoutingBoard, ScoringSettings)` constructor (BoardHistory.java:195-200) -> `BoardHistoryEntry::new`.
#[derive(Debug, Clone)]
pub struct BoardHistoryEntry {
    /// Java `byte[] board = board.serialize(false)` (`:190`, `:196`).
    ///
    /// **Ruling 8**: the port has no `Serializable`, so the snapshot is a [`Board`] value. This
    /// is a plain [`Clone`] — a field-for-field copy, which is what `serialize(false)` captures —
    /// and *not* [`Board::deep_copy`]: the resets that Java's `readObject` performs happen on the
    /// **deserialize** side, and so they happen in [`BoardHistory::restore_board`], at `:148`.
    /// See that method.
    pub board: Board,
    /// Java `hash` (`:191`, `:197`) — `board.getHash()`.
    ///
    /// `u64` rather than Java's hex MD5 string: [`Board::structural_hash`] is a same-board
    /// membership test over the port's own item fields, not a port of MD5-over-serialized-bytes
    /// (`crates/fr-board/src/board/snapshot.rs`, controller ruling AH). Only the *equality
    /// pattern* across entries is comparable with Java's, never the value.
    pub hash: u64,
    /// Java `score` (`:192`, `:198`) — `new BoardStatistics(board).getNormalizedScore(...)`.
    pub score: f32,
    /// Java `restoreCount` (`:193`, `:199`), the only non-`final` field of the entry.
    pub restore_count: i32,
}

impl BoardHistoryEntry {
    /// Port of `BoardHistoryEntry(RoutingBoard, ScoringSettings)` (BoardHistory.java:195-200).
    ///
    /// The three initialisers run **in Java's order**, and that order is observable:
    /// `new BoardStatistics(board)` mutates the board it measures (it builds two
    /// `DesignRulesChecker`s, plan-5 ruling 8), so the snapshot at `:196` is taken *before* the
    /// score at `:198`, not after.
    fn new(board: &mut Board, scoring: &ScoringSettings) -> BoardHistoryEntry {
        // BoardHistory.java:196 — `board.serialize(false)`; ruling 8's clone.
        let snapshot = board.clone();
        // BoardHistory.java:197.
        let hash = board.structural_hash();
        // BoardHistory.java:198.
        let score = BoardStatistics::new(board).normalized_score(scoring);
        BoardHistoryEntry {
            board: snapshot,
            hash,
            score,
            // BoardHistory.java:199.
            restore_count: 0,
        }
    }
}

impl BoardHistory {
    /// `MAX_HISTORY_SIZE` (BoardHistory.java:29) `= 30`.
    ///
    /// An associated `const`, not only a field default: `BatchAutorouter.java:40` is
    /// `static final int BOARD_RANK_LIMIT = BoardHistory.MAX_HISTORY_SIZE;`, so Task 10 writes
    /// `BoardHistory::MAX_HISTORY_SIZE` for it.
    pub const MAX_HISTORY_SIZE: usize = 30;

    /// Port of `BoardHistory(ScoringSettings)` (BoardHistory.java:37-39): the default cap.
    pub fn new(scoring: &ScoringSettings) -> BoardHistory {
        BoardHistory::with_capacity(scoring, BoardHistory::MAX_HISTORY_SIZE)
    }

    /// Port of the package-private `BoardHistory(ScoringSettings, int)` (BoardHistory.java:42-45),
    /// "intended for unit tests only" — `BoardHistoryTest.sizeCapEvictsWorstEntry` is its only
    /// caller in `src/test`, and nothing in `src/main` calls it.
    ///
    /// `usize` where Java has an `int`: a negative cap would make Java's `:53`
    /// `boards.size() >= maxHistorySize` true on an empty list, i.e. "always at capacity", but
    /// neither Java caller can produce one (the public constructor passes `30`, the test passes
    /// `1` and `30`), so the state is unreachable rather than reproduced.
    pub fn with_capacity(scoring: &ScoringSettings, max_history_size: usize) -> BoardHistory {
        BoardHistory {
            boards: Vec::new(),
            max_history_size,
            scoring: scoring.clone(),
        }
    }

    /// Port of `add` (BoardHistory.java:48-80): "adds a routing board to history if it improves
    /// overall score or space permits".
    ///
    /// Two things about it are load-bearing and are reproduced rather than tidied:
    ///
    /// * **Under capacity there is no score gate at all.** `:53`'s `if` guards the *whole* gate,
    ///   so any board whose hash is new enters, however bad it scores. The javadoc's "if it
    ///   improves overall score" is only true at capacity.
    /// * **At capacity the board's score is computed twice.** `:56` computes it for the gate and
    ///   `:79`'s `new BoardHistoryEntry(...)` computes it again at `:198`. Each computation
    ///   builds two `DesignRulesChecker`s and mutates the board, so collapsing them to one would
    ///   be a different program, not a faster one.
    ///
    /// `&mut Board` because [`BoardStatistics::new`] is `&mut Board` (Task 1's finding 2:
    /// `Item.clearanceViolations` lowers the board's smallest clearance and advances the search
    /// tree's entry counter).
    pub fn add(&mut self, board: &mut Board) {
        // BoardHistory.java:49-51.
        if self.contains(board) {
            return;
        }

        // BoardHistory.java:53.
        if self.boards.len() >= self.max_history_size {
            // BoardHistory.java:54-56 — the first of the two score computations.
            let new_score = BoardStatistics::new(board).normalized_score(&self.scoring);

            // BoardHistory.java:58-68: the worst-scoring entry by a linear scan. `<`, so the
            // **first** entry of a tie wins and stays the eviction candidate.
            let mut worst_index = 0;
            let mut worst_score = self.boards[0].score;
            for i in 1..self.boards.len() {
                if self.boards[i].score < worst_score {
                    worst_score = self.boards[i].score;
                    worst_index = i;
                }
            }

            // BoardHistory.java:70-75: only a **strictly** better board evicts. `<=` here is why
            // a board that ties the worst entry is dropped rather than swapped in.
            if new_score <= worst_score {
                return;
            }
            // BoardHistory.java:76.
            self.boards.remove(worst_index);
        }

        // BoardHistory.java:79 — the second score computation, inside the entry's constructor.
        let entry = BoardHistoryEntry::new(board, &self.scoring);
        self.boards.push(entry);
    }

    /// Port of `clear` (BoardHistory.java:83-85).
    pub fn clear(&mut self) {
        self.boards.clear();
    }

    /// Port of `contains` (BoardHistory.java:88-101): true if some entry carries this board's
    /// hash.
    pub fn contains(&self, board: &Board) -> bool {
        // BoardHistory.java:89.
        let hash = board.structural_hash();
        // BoardHistory.java:92-97.
        for entry in &self.boards {
            if entry.hash == hash {
                return true;
            }
        }
        false
    }

    /// Port of `remove` (BoardHistory.java:104-112): drops the **first** entry with this board's
    /// hash and returns; a second entry with the same hash would survive, which no caller can
    /// produce because [`Self::add`]'s `contains` gate makes the hashes distinct.
    pub fn remove(&mut self, board: &Board) {
        // BoardHistory.java:105.
        let hash = board.structural_hash();
        // BoardHistory.java:106-111.
        for i in 0..self.boards.len() {
            if self.boards[i].hash == hash {
                self.boards.remove(i);
                return;
            }
        }
    }

    /// Port of `getMaxScore` (BoardHistory.java:115-128).
    ///
    /// **Java bug: `BoardHistory.getMaxScore`** — the accumulator starts at `0` (`:118`), not at
    /// `-inf`, so an empty history answers `0` and so does a history whose every board scores
    /// negatively. `AutorouteBatchLoop.java:306` compares `bh.getMaxScore() > boardScoreAfter`
    /// with a **strict** `>`, so an empty history never triggers a restore — which is why the
    /// loop's `bh.size() >= STOP_AT_PASS_MINIMUM` guard at `:298` is belt-and-braces rather than
    /// load-bearing. Quirk #197; do not "improve" the seed to [`f32::NEG_INFINITY`].
    // renamed: `BoardHistory.getMaxScore` -> `BoardHistory::max_score` (the plan's interface table names it that; Rust getters drop the `get_`).
    pub fn max_score(&self) -> f32 {
        // BoardHistory.java:118.
        let mut max_score = 0.0f32;
        // BoardHistory.java:119-123. A bare `>`, as Java has it: a NaN score never wins, and
        // `-0.0 > 0.0` is false, so a `-0.0`-scoring history still answers `+0.0`.
        for entry in &self.boards {
            if entry.score > max_score {
                max_score = entry.score;
            }
        }
        max_score
    }

    /// Port of `restoreBoard(int)` (BoardHistory.java:134-155): the best board whose
    /// `restoreCount` is at most `max_allowed_restore_count`, or `None`.
    ///
    /// **Java bug: `BoardHistory.restoreBoard`** — it mutates under a *read* lock: `:139` takes
    /// `rwLock.readLock()`, `:143` **sorts `boards` in place** and `:147` increments the chosen
    /// entry's `restoreCount`. Single-threaded, so the lock is unobservable — but the sort and
    /// the increment are not: [`Self::rank`]'s answer depends on how many restores have happened
    /// and in what order, and `AutorouteBatchLoop.java:315-320` **breaks the whole pass loop**
    /// when `getRank(...) > BOARD_RANK_LIMIT`. Quirk #198.
    ///
    /// The sort is `(o1, o2) -> Float.compare(o2.score, o1.score)` — **descending**, with
    /// [`java_float_compare`]'s semantics, through a **stable** sort (`List.sort` is TimSort;
    /// [`slice::sort_by`] is stable too), so entries that tie keep their relative order and a
    /// tie's *first* entry is the one whose `restoreCount` rises first.
    ///
    /// `&mut self` where Java's method is a read-lock method, because it is the one that mutates.
    pub fn restore_board(&mut self, max_allowed_restore_count: i32) -> Option<Board> {
        // BoardHistory.java:135-137: `<= 0` means unlimited. Note it is `<=`, not `==`, so a
        // negative budget is unlimited too.
        let max_allowed_restore_count = if max_allowed_restore_count <= 0 {
            i32::MAX
        } else {
            max_allowed_restore_count
        };

        // BoardHistory.java:143.
        self.boards
            .sort_by(|o1, o2| java_float_compare(o2.score, o1.score));

        // BoardHistory.java:145-150.
        for entry in &mut self.boards {
            if entry.restore_count <= max_allowed_restore_count {
                entry.restore_count += 1;
                // BoardHistory.java:148 — `BasicBoard.deserialize(entry.board)`. The transient
                // resets Java's `readObject` performs (BasicBoard.java:1388-1400) happen here,
                // on the deserialize, which is why [`Board::deep_copy`] is called on the
                // *restore* rather than on the snapshot. `deep_copy`'s two extra steps beyond a
                // plain deserialize — `clearAllItemTemporaryAutorouteData` and `finishAutoroute`,
                // which belong to `RoutingBoardUndoFacade.deepCopy` — are no-ops on a round trip:
                // `Item.autorouteInfo` is `transient` (Item.java:67), so a deserialized board's
                // is already null, and the port's `finish_autoroute` is empty (plan-6 ruling 3
                // puts the engine outside `Board`).
                return Some(entry.board.deep_copy());
            }
        }
        // BoardHistory.java:151.
        None
    }

    /// Port of `restoreBestBoard` (BoardHistory.java:158-160) — `restoreBoard(0)`, i.e. no
    /// restore-count budget at all.
    pub fn restore_best_board(&mut self) -> Option<Board> {
        self.restore_board(0)
    }

    /// Port of `size` (BoardHistory.java:163-170).
    pub fn size(&self) -> usize {
        self.boards.len()
    }

    /// Port of `getRank(RoutingBoard)` (BoardHistory.java:173-186): the **1-indexed position in
    /// the current list order** of the entry carrying this board's hash, or `-1`.
    ///
    /// Order-dependent, and deliberately so: the list is in insertion order until the first
    /// [`Self::restore_board`] sorts it, and in score order after (quirk #198). `i32` rather than
    /// `Option<usize>` because `AutorouteBatchLoop.java:315-320` compares the answer against
    /// `BOARD_RANK_LIMIT` with `>`, and `-1 > 30` is the "not found, do not break" answer that
    /// falls out of the sentinel.
    // renamed: `BoardHistory.getRank` -> `BoardHistory::rank` (the plan's interface table names it that; Rust getters drop the `get_`).
    pub fn rank(&self, board: &Board) -> i32 {
        // BoardHistory.java:174.
        let hash = board.structural_hash();
        // BoardHistory.java:177-181.
        for (i, entry) in self.boards.iter().enumerate() {
            if entry.hash == hash {
                // BoardHistory.java:179 — `return i + 1`, 1-indexed. The list is capped at
                // `max_history_size` entries, so the conversion cannot lose an index in practice;
                // the saturating form is here so it cannot lose one in principle either.
                return i32::try_from(i + 1).unwrap_or(i32::MAX);
            }
        }
        // BoardHistory.java:182.
        -1
    }

    /// The entries in list order — the state `restoreBoard`'s in-place sort rearranges.
    ///
    /// `pub` where Java's `boards` (`:32`) is `private`: Java's own `BoardHistoryTest` cannot see
    /// it either, which is why `scripts/differential/java/probes/P7T2Probe.java` reaches it by
    /// reflection. The port's tests replay that probe's transcript, so they need the same view,
    /// and a read-only slice is the smallest one that gives it.
    pub fn entries(&self) -> &[BoardHistoryEntry] {
        &self.boards
    }
}

/// `java.lang.Float.compare(float, float)` — transcribed, because neither
/// [`f32::partial_cmp`] nor [`f32::total_cmp`] is it.
///
/// ```text
/// public static int compare(float f1, float f2) {
///     if (f1 < f2) return -1;           // Neither val is NaN, thisVal is smaller
///     if (f1 > f2) return 1;            // Neither val is NaN, thisVal is larger
///     int thisBits    = Float.floatToIntBits(f1);
///     int anotherBits = Float.floatToIntBits(f2);
///     return (thisBits == anotherBits ?  0 : (thisBits < anotherBits ? -1 : 1));
/// }
/// ```
///
/// `floatToIntBits` collapses every NaN to the canonical `0x7fc00000`, whose *signed* `int` value
/// is above `+Infinity`'s `0x7f800000`, so **NaN sorts above everything**; and `-0.0`'s
/// `0x80000000` is negative as an `int`, so **`-0.0 < +0.0`**. `partial_cmp` answers `None` for
/// the NaN cases and `Equal` for the zeros; `total_cmp` agrees with Java everywhere except a
/// **negative** NaN, which it places below `-Infinity` where Java places it above `+Infinity`.
/// On the corpus the difference is unreachable — `getNormalizedScore` never answers NaN there,
/// because HEAD guards `maximumScore <= 0f` (Task 1's finding 1) — but it is not hypothetical:
/// Task 1's synthetic `S5` reaches `Infinity - Infinity` and the JVM renders `NaN`. The
/// transcription is Java's statement for statement either way, so the question does not have to
/// be re-asked at each new caller.
///
/// `pub` so the next `Float.compare` site in this crate reuses it instead of copying it.
// renamed: the JDK's `java.lang.Float.compare` -> this function; it is runtime-library code rather than freerouting code, so it has no `audit-port.sh` row.
pub fn java_float_compare(f1: f32, f2: f32) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    if f1 < f2 {
        return Ordering::Less;
    }
    if f1 > f2 {
        return Ordering::Greater;
    }
    // `floatToIntBits` collapses **every** NaN to `0x7fc00000` before comparing, where
    // `f32::to_bits` (i.e. `floatToRawIntBits`) would keep a sign bit or a payload; the
    // canonicalisation is what puts a negative NaN above `+Infinity` rather than below
    // `-Infinity`. The comparison itself is on Java's *signed* `int`, which is what makes
    // `-0.0`'s `0x80000000` compare below `+0.0`'s `0`.
    let this_bits = float_to_int_bits(f1);
    let another_bits = float_to_int_bits(f2);
    this_bits.cmp(&another_bits)
}

/// `java.lang.Float.floatToIntBits` — [`f32::to_bits`] with every NaN collapsed to the canonical
/// `0x7fc00000`, as a **signed** `int`.
fn float_to_int_bits(value: f32) -> i32 {
    if value.is_nan() {
        return 0x7fc0_0000;
    }
    i32::from_ne_bytes(value.to_bits().to_ne_bytes())
}
