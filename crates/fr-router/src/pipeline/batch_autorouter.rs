//! [`BatchAutorouter`] — the port of `autoroute/pipeline/BatchAutorouter.java` (565 lines), the
//! object the pass runner and the optimizer both drive (Plan 7 Task 8).
//!
//! Task 8 lands the **scaffold**: the constants (`:38-64`), the field block (`:66-107`), both
//! constructors (`:110-158`), the five accessors (`:164-182`), the five `NamedAlgorithm` identity
//! members, `getImpactedPoints` (`:283-297`), `enforceStrictDrc` (`:305-329`), `isFanoutTimedOut`
//! (`:331-333`), `shouldFireBoardUpdate` (`:335-343`), `removeTails` (`:487-503`),
//! `autorouteItem` (`:507-514`) and `calculateIncompleteCount` (`:556-564`). `getAutorouteItems`
//! and `autoroutePass` are Task 9's, `runBatchLoop` and
//! `autoroutePassesForOptimizingItem` Task 10's / Task 13's; each is a marker at the foot of
//! this file.
//!
//! # Ruling AJ, in one place
//!
//! `retainAutorouteDatabase` (`:63-64`, `:151-154`) is
//! `Boolean.getBoolean("freerouting.benchmark.retain_autoroute_database")` — a benchmark-only
//! system property, `false` on every production and parity path, and hard-coded `false` at
//! `BatchAutorouterThread.java:90`. Controller ruling AJ makes it **permanently `false` in the
//! port, with no setter**: [`BatchAutorouter::BENCHMARK_RETAIN_AUTOROUTE_DATABASE`] is a `const`,
//! [`crate::route_connection_full`] passes `false` unconditionally, and the five
//! `RoutingBoard.additionalUpdateAfterChange` sites in `fr-board` carry
//! `// not reachable:` markers rather than deferrals. `crates/fr-router/tests/batch_autorouter.rs`
//! pins both halves: a behavioural test that the flag behaved as `false`, and a source grep that
//! no setter exists.
//!
//! # What is deliberately not here
//!
//! * the three listener lists of `NamedAlgorithm` (`:26-31`) — controller ruling AK replaces them
//!   with [`crate::pipeline::ProgressSink`];
//! * the nine `profile*` counters (`:96-105`) and every `System.nanoTime()` block that writes
//!   them — all guarded by `-Dfreerouting.benchmark.profile`, default `false`;
//! * `RoutingJob` (`:78`) — Plan 8's, together with the CLI that builds it.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use fr_board::StopConnectionOption;
use fr_board::datastructures::StopCheck;
use fr_board::items::Item;
use fr_board::{Board, BoardError, ItemId};
use fr_drc::DesignRulesChecker;
use fr_geometry::Point;
use fr_settings::{ExpansionCostFactor, RouterSettings};

use crate::autoroute::attempt::{AutorouteAttemptResult, AutorouteAttemptState};
use crate::autoroute::maze::engine::{AutorouteEngine, route_connection_full};
use crate::board_ext::RoutingBoardExt;
use crate::error::RouterError;
use crate::pipeline::airline::{ItemDistanceCache, calculate_item_distance_cached};
use crate::pipeline::board_history::BoardHistory;
use crate::pipeline::failure_log::RoutingFailureLog;
use crate::pipeline::pass_runner::AutoroutePassRunner;
use crate::pipeline::stop::{ProgressThrottler, RouterBudget};
use crate::pipeline::{NamedAlgorithmType, ProgressSink, RouterStop};
use crate::score::BoardStatistics;

/// Port of `autoroute.pipeline.BatchAutorouter` (BatchAutorouter.java:36-565) — the object the
/// pass runner and the optimizer both drive.
///
/// # The board is a parameter, not a field
///
/// Java's `NamedAlgorithm.board` (`NamedAlgorithm.java:37`) is a mutable field that
/// `applyStrictDrcAfterRoute` even *reassigns* (`AutorouteConnectionRouter.java:251`). The port
/// has one `Board`, owned by the caller and threaded as `&mut Board`, exactly as
/// [`crate::route_connection`] takes it — see [`BatchAutorouter::enforce_strict_drc`] for the one
/// place where that difference is visible and why it is a `// totalized:` rather than a
/// divergence.
///
/// # `withPreferredDirections` is a constructor argument, not a field
///
/// The plan's draft listed `with_preferred_directions` among the fields at `:116`. It is not one:
/// `:130` is a constructor **parameter** whose only effect is which of the two `traceCosts`
/// arrays `:138-148` builds. Java wins; the port has the same parameter and the same field.
#[derive(Debug)]
pub struct BatchAutorouter<'a> {
    // -- NamedAlgorithm's two surviving fields (NamedAlgorithm.java:33-37) ----------------------
    /// `NamedAlgorithm.settings` (`NamedAlgorithm.java:33`).
    settings: &'a RouterSettings,

    // -- BatchAutorouter.java:66-70 ------------------------------------------------------------
    /// `final boolean removeUnconnectedVias` (`:66`). The `RoutingJob` constructor derives it as
    /// `!settings.isFanoutEnabled()` (`:115`); `autoroutePassesForOptimizingItem` passes `true`
    /// unconditionally (`:258`).
    remove_unconnected_vias: bool,
    /// `final AutorouteControl.ExpansionCostFactor[] traceCosts` (`:67`), built by `:138-148`.
    trace_costs: Vec<ExpansionCostFactor>,
    /// `final boolean retainAutorouteDatabase` (`:68`), assigned from
    /// `BENCHMARK_RETAIN_AUTOROUTE_DATABASE` at `:154`.
    ///
    /// Ruling AJ: the port has **no setter**, so this is always
    /// [`BatchAutorouter::BENCHMARK_RETAIN_AUTOROUTE_DATABASE`], i.e. `false`. It is a field
    /// rather than a bare `const` read so that the accessor `:172-174` has something to answer
    /// and the transcription of `:68` is visible.
    retain_autoroute_database: bool,
    /// `final int startRipupCosts` (`:69`).
    start_ripup_costs: i32,
    /// `final int tracePullTightAccuracy` (`:70`).
    trace_pull_tight_accuracy: i32,

    // -- BatchAutorouter.java:79-95 ------------------------------------------------------------
    /// `int totalItemsRouted` (`:79`), incremented by `AutoroutePassRunner.java:222` and read by
    /// `:214`'s `settings.maxItems` test — quirk #202's site, which is Task 9's.
    pub total_items_routed: i32,
    /// `boolean fanoutTimedOut` (`:80`), written by `AutorouteBatchLoop.java:173` (Task 10) and
    /// read by [`BatchAutorouter::is_fanout_timed_out`].
    pub fanout_timed_out: bool,
    /// `int initialUnroutedCount` (`:89`), written by `AutorouteBatchLoop.java:63`.
    pub initial_unrouted_count: i32,
    /// `Instant sessionStartTime` (`:92`), written by `AutorouteBatchLoop.java:62`.
    ///
    /// Reporting only — no routing decision reads it, so it is not one of ruling AI's clocks.
    pub session_start_time: Option<Instant>,
    /// `boolean isOptimizerAutorouter` (`:95`), set by `autoroutePassesForOptimizingItem`
    /// (`:263`) and read by `AutorouteBatchLoop.java:42, :374` (Task 10).
    pub is_optimizer_autorouter: bool,

    // -- BatchAutorouter.java:94, 106-107 ------------------------------------------------------
    /// `long lastBoardUpdateTimestamp` (`:94`) — the state behind
    /// [`BatchAutorouter::should_fire_board_update`], held as Task 4's
    /// [`ProgressThrottler::board_update_gate`] rather than as a bare timestamp so the 250 ms
    /// literal is [`RouterBudget::board_update_throttle_ms`] and a driver can pin it.
    board_update_gate: ProgressThrottler,
    /// `BoardStatistics progressStatistics` (`:106`), rebuilt every
    /// [`BatchAutorouter::PROGRESS_STATISTICS_ITEM_INTERVAL`] items by
    /// `AutoroutePassRunner.updateProgress` (`:496-501`) — Task 9's writer.
    pub progress_statistics: Option<BoardStatistics>,
    /// `int progressItemsSinceStatistics` (`:107`), the counter beside it.
    pub progress_items_since_statistics: i32,

    /// Controller ruling AI's budget knob. **Not a Java field**: Java writes the four throttles as
    /// literals (`TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP` at `:43`, the `250` at `:338`), and ruling
    /// AI makes them parameters so every `p7t*` parity run can disable the wall clock on both
    /// sides. Default [`RouterBudget::default`] is Java's four values exactly.
    budget: RouterBudget,
    // not ported: `BatchAutorouter.random` (`:83`, `new Random(0)` at `:135`) — its only reader is
    // `AutoroutePassRunner.java:59`'s `shuffle(clonedAutorouteItemList, router.random)`, inside
    // `runMultiThread` (`:40-149`), the dead multithreaded path (quirk **#143**, extended by
    // Plan 7 Task 17 — the plan's *label* for this was #216, which the register spent on
    // `alreadyRoutedBoardHashes`; plan ruling AM's
    // "General" clause). `runSingleThread` never shuffles. A tree-wide grep for `router.random`
    // answers that one line.
    // not ported: `BatchAutorouter.airLine` (`:86`) — the airline of the connection being routed,
    // drawn by the GUI. Its five writers are `AutoroutePassRunner.java:45, 81, 142, 146, 164, 329,
    // 333` and `AutorouteConnectionRouter.java:70`, and its only reader is the public
    // `getAirLine` accessor (`:519-527`), which nothing on the headless path calls
    // (`global-constraints.md`: no GUI).
    // not ported: the nine `profile*` counters (`:96-105`) and `resetPassProfile` /
    // `logBenchmarkProfile` (`:196-238`) — every writer sits behind
    // `isBenchmarkProfileEnabled()`, i.e. `-Dfreerouting.benchmark.profile`, default `false`.
    // not ported: `BatchAutorouter.reusableAutorouteItemList` and `reusableHandledItems`
    // (`:73-74`) — allocation reuse for `getAutorouteItems` (`:345-409`), which is Task 9's; the
    // port's `getAutorouteItems` returns a fresh `Vec`, and Java clears both at `:347-348` before
    // every use, so the reuse is invisible.
    // not ported: `BatchAutorouter.connectionRouter`, `passRunner`, `batchLoop` (`:75-77`) —
    // three helper objects whose only state is a back-pointer to this one (their constructors are
    // `AutorouteConnectionRouter.java:26-28`, `AutoroutePassRunner.java:36-38` and
    // `AutorouteBatchLoop.java:33-35`). The port's counterparts are free functions and structs
    // that take `&mut BatchAutorouter`, so there is no field to hold.
    // not ported: `NamedAlgorithm.job` (`BatchAutorouter.java:78`) — **closed by Plan 8 Task 14.**
    // The field is a back-pointer to `core/RoutingJob`, the CLI/MCP job record. Plan 8 Task 1
    // ported that record as `fr_core::RoutingJob`, and the dependency direction is why no field
    // appears here: `fr-core` composes `fr-router` (spec §4), so the router cannot hold a job.
    // Java's own readers of this field on the batch path are `job.logWarning`/`job.logInfo`, which
    // `global-constraints.md` drops with the rest of `FRLogger`, and `job.thread`, which quirk Y's
    // rostered monitor thread is the only consumer of. What a caller actually needs from the job
    // — the settings, the budget, the stop token — arrives through
    // [`BatchAutorouter::for_routing_job`]'s three arguments instead.
}

impl<'a> BatchAutorouter<'a> {
    // ---------------------------------------------------------------------------------------------
    // The constants — BatchAutorouter.java:38-64
    // ---------------------------------------------------------------------------------------------

    /// `BOARD_RANK_LIMIT = BoardHistory.MAX_HISTORY_SIZE` (`:40`) — "the lowest rank of the board
    /// to be selected to go back to. Must not exceed `BoardHistory.MAX_HISTORY_SIZE` so the check
    /// can actually fire."
    ///
    /// Written as the reference Java writes, not as the literal `30`, so the two cannot drift.
    pub const BOARD_RANK_LIMIT: usize = BoardHistory::MAX_HISTORY_SIZE;
    /// `MAXIMUM_TRIES_ON_THE_SAME_BOARD = 3` (`:42`).
    pub const MAXIMUM_TRIES_ON_THE_SAME_BOARD: i32 = 3;
    /// `TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP = 1000` (`:43`) — the `optChangedArea` budget, which
    /// controller ruling AI turns into [`RouterBudget::opt_changed_area_ms`]. The constant is
    /// transcribed anyway because it is `RouterBudget::default`'s value and a reader looking for
    /// `:43` must find it.
    pub const TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP: i32 = 1000;
    /// `STOP_AT_PASS_MINIMUM = 8` (`:46`) — "the minimum number of passes to complete the board,
    /// unless all items are routed."
    pub const STOP_AT_PASS_MINIMUM: i32 = 8;
    /// `STOP_AT_PASS_MODULO = 4` (`:49`).
    pub const STOP_AT_PASS_MODULO: i32 = 4;
    /// `STAGNATION_PASS_LIMIT = 10` (`:52`).
    pub const STAGNATION_PASS_LIMIT: i32 = 10;
    /// `FANOUT_RECOVERY_STAGNATION_PASSES = 3` (`:54`).
    pub const FANOUT_RECOVERY_STAGNATION_PASSES: i32 = 3;
    /// `PROGRESS_STATISTICS_ITEM_INTERVAL = 10` (`:57`).
    pub const PROGRESS_STATISTICS_ITEM_INTERVAL: i32 = 10;
    /// `STAGNATION_SCORE_THRESHOLD = 0.5F` (`:60`) — an `f32`, as Java writes it, because the
    /// score it is compared against is `getNormalizedScore`'s `float`.
    pub const STAGNATION_SCORE_THRESHOLD: f32 = 0.5;

    // not reachable: BatchAutorouter.isBenchmarkProfileEnabled (a Java benchmark-only system property, and the port exposes no setter)
    /// `BENCHMARK_PROFILE_ENABLED = Boolean.getBoolean("freerouting.benchmark.profile")`
    /// (`:61-62`).
    ///
    /// `Boolean.getBoolean` reads a **system property**, so this is `false` unless the JVM was
    /// started with `-Dfreerouting.benchmark.profile=true`. Ruling AJ's pattern applies to both of
    /// `:61-64`: the port hard-codes `false` and offers nothing that could flip it, so every
    /// `System.nanoTime()` block it guards is dead and is rostered `// not ported:` above.
    pub const BENCHMARK_PROFILE_ENABLED: bool = false;

    // not reachable: BatchAutorouter.isRetainAutorouteDatabase (a Java benchmark-only system property, and the port exposes no setter)
    /// `BENCHMARK_RETAIN_AUTOROUTE_DATABASE =
    /// Boolean.getBoolean("freerouting.benchmark.retain_autoroute_database")` (`:63-64`).
    ///
    /// **Controller ruling AJ.** The property is unset on every production and parity path, and
    /// `BatchAutorouterThread.java:90` hard-codes the same `false`. Setting it `true` would make
    /// `AutorouteEngine.maintainDatabase` true (`RoutingBoard.initAutoroute:892`), which is the
    /// single gate on `RoutingBoard.additionalUpdateAfterChange` (`:100-102`) — the five
    /// `fr-board` sites that now carry `// not reachable:` markers instead of deferrals.
    ///
    /// The port therefore hard-codes `false` **and exposes no way to change it**: there is no
    /// setter, no builder argument and no environment read.
    /// `crates/fr-router/tests/batch_autorouter.rs`'s
    /// `retain_autoroute_database_is_false_on_every_path` proves the behaviour and
    /// `retain_autoroute_database_has_no_setter` proves the roster.
    pub const BENCHMARK_RETAIN_AUTOROUTE_DATABASE: bool = false;

    // ---------------------------------------------------------------------------------------------
    // NamedAlgorithm's identity — BatchAutorouter.java:423-441, :460-463
    // ---------------------------------------------------------------------------------------------
    //
    // Five one-line `return "literal";` overrides of `NamedAlgorithm`'s abstract members. The port
    // has no `NamedAlgorithm` trait to override — controller ruling AK replaces the class's other
    // half, the three listener lists, with `ProgressSink` — so the five become associated
    // constants, which is what the plan calls "Task 4's five consts".
    //
    // renamed: `BatchAutorouter.getId` (`:423-426`) -> `BatchAutorouter::ID`, an associated const.
    // renamed: `BatchAutorouter.getName` (`:428-431`) -> `BatchAutorouter::NAME`.
    // renamed: `BatchAutorouter.getVersion` (`:433-436`) -> `BatchAutorouter::VERSION`.
    // renamed: `BatchAutorouter.getDescription` (`:438-441`) -> `BatchAutorouter::DESCRIPTION`.
    // renamed: `BatchAutorouter.getType` (`:460-463`) -> `BatchAutorouter::TYPE`.

    /// `getId()` (`:423-426`).
    pub const ID: &'static str = "freerouting-router";
    /// `getName()` (`:428-431`).
    pub const NAME: &'static str = "Freerouting Auto-router";
    /// `getVersion()` (`:433-436`).
    pub const VERSION: &'static str = "1.0";
    /// `getDescription()` (`:438-441`).
    pub const DESCRIPTION: &'static str = "Freerouting Auto-router v1.0";
    /// `getType()` (`:460-463`).
    pub const TYPE: NamedAlgorithmType = NamedAlgorithmType::Router;

    // ---------------------------------------------------------------------------------------------
    // The two constructors — BatchAutorouter.java:110-158
    // ---------------------------------------------------------------------------------------------

    /// Port of `BatchAutorouter(StoppableThread, RoutingBoard, RouterSettings, boolean, boolean,
    /// int, int)` (`:125-158`) — the seven-argument constructor both production paths reach.
    ///
    /// `board` is read and not stored: `:142` needs `board.getLayerCount()` for the
    /// no-preferred-direction cost array and nothing else in the constructor touches it.
    /// `budget` is controller ruling AI's knob and has no Java counterpart; pass
    /// [`RouterBudget::default`] for Java's own literals.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        board: &Board,
        settings: &'a RouterSettings,
        remove_unconnected_vias: bool,
        with_preferred_directions: bool,
        start_ripup_costs: i32,
        pull_tight_accuracy: i32,
        budget: RouterBudget,
    ) -> BatchAutorouter<'a> {
        // :135 — `new Random(0)`; see the `not ported:` marker on the field block.
        // :137.
        // :138-148.
        let trace_costs = if with_preferred_directions {
            // :139.
            settings.get_trace_costs()
        } else {
            // :141-147 — "remove preferred direction": one layer's minimum cost used in both
            // directions.
            (0..board.get_layer_count())
                .map(|i| {
                    let current_min_cost = settings.get_preferred_direction_trace_costs(i);
                    ExpansionCostFactor {
                        horizontal: current_min_cost,
                        vertical: current_min_cost,
                    }
                })
                .collect()
        };

        BatchAutorouter {
            settings,
            // :137.
            remove_unconnected_vias,
            trace_costs,
            // :152-154. Ruling AJ — the `const`, never a parameter.
            retain_autoroute_database: BatchAutorouter::BENCHMARK_RETAIN_AUTOROUTE_DATABASE,
            // :150-151.
            start_ripup_costs,
            trace_pull_tight_accuracy: pull_tight_accuracy,
            // Java's `int`/`boolean`/`Instant` fields start at their zero values.
            total_items_routed: 0,
            fanout_timed_out: false,
            initial_unrouted_count: 0,
            session_start_time: None,
            is_optimizer_autorouter: false,
            board_update_gate: ProgressThrottler::board_update_gate(
                budget.board_update_throttle_ms,
            ),
            progress_statistics: None,
            progress_items_since_statistics: 0,
            budget,
            // :155-157 — the three helper objects; see the field block's `not ported:` marker.
        }
    }

    /// Port of `BatchAutorouter(RoutingJob)` (`:110-122`) — the delegating constructor the
    /// headless pipeline uses.
    ///
    /// `RoutingJob` is Plan 8's (spec §13), so the port takes the three values `:112-120` reads
    /// off it — `job.board`, `job.routerSettings` and, through them, the four derived arguments:
    ///
    /// | Java | value |
    /// |---|---|
    /// | `:115` `removeUnconnectedVias` | `!settings.isFanoutEnabled()` |
    /// | `:116` `withPreferredDirections` | `true`, a literal |
    /// | `:117` `startRipupCosts` | `settings.getStartRipupCosts()` |
    /// | `:118-120` `pullTightAccuracy` | `settings.tracePullTightAccuracy`, or **500** when it is `null` |
    ///
    /// `:121`'s `this.job = job` is the `// not ported:` row on the field block above.
    pub fn for_routing_job(
        board: &Board,
        settings: &'a RouterSettings,
        budget: RouterBudget,
    ) -> BatchAutorouter<'a> {
        BatchAutorouter::new(
            board,
            settings,
            // :115.
            !settings.is_fanout_enabled(),
            // :116.
            true,
            // :117.
            settings.get_start_ripup_costs(),
            // :118-120. The `null` fallback is Java's own literal, **not**
            // `RouterSettings.validate`'s (`RouterSettings.java:958-963`), which repairs an
            // out-of-range value to the same 500 by a different route.
            settings.trace_pull_tight_accuracy.unwrap_or(500),
            budget,
        )
    }

    // ---------------------------------------------------------------------------------------------
    // The accessors — BatchAutorouter.java:160-182, :331-333, :465-473
    // ---------------------------------------------------------------------------------------------

    /// `isBenchmarkProfileEnabled()` (`:160-162`).
    pub fn is_benchmark_profile_enabled() -> bool {
        BatchAutorouter::BENCHMARK_PROFILE_ENABLED
    }

    /// `isRemoveUnconnectedVias()` (`:164-166`).
    pub fn is_remove_unconnected_vias(&self) -> bool {
        self.remove_unconnected_vias
    }

    /// `getTraceCosts()` (`:168-170`).
    pub fn get_trace_costs(&self) -> &[ExpansionCostFactor] {
        &self.trace_costs
    }

    /// `isRetainAutorouteDatabase()` (`:172-174`) — always `false`; see
    /// [`BatchAutorouter::BENCHMARK_RETAIN_AUTOROUTE_DATABASE`].
    pub fn is_retain_autoroute_database(&self) -> bool {
        self.retain_autoroute_database
    }

    /// `getStartRipupCosts()` (`:176-178`).
    pub fn get_start_ripup_costs(&self) -> i32 {
        self.start_ripup_costs
    }

    /// `getTracePullTightAccuracy()` (`:180-182`).
    pub fn get_trace_pull_tight_accuracy(&self) -> i32 {
        self.trace_pull_tight_accuracy
    }

    /// `isFanoutTimedOut()` (`:331-333`).
    pub fn is_fanout_timed_out(&self) -> bool {
        self.fanout_timed_out
    }

    /// `getInitialUnroutedCount()` (`:465-468`).
    pub fn get_initial_unrouted_count(&self) -> i32 {
        self.initial_unrouted_count
    }

    /// `getSessionStartTime()` (`:470-473`).
    pub fn get_session_start_time(&self) -> Option<Instant> {
        self.session_start_time
    }

    /// The settings the pass runner and the connection router read off `router.settings`.
    pub fn settings(&self) -> &'a RouterSettings {
        self.settings
    }

    /// Controller ruling AI's budget, as handed to the constructor.
    pub fn budget(&self) -> RouterBudget {
        self.budget
    }

    // ---------------------------------------------------------------------------------------------
    // getImpactedPoints — BatchAutorouter.java:283-297
    // ---------------------------------------------------------------------------------------------

    /// Port of `getImpactedPoints(Item)` (`:283-297`): the points a routed item "touches", used
    /// by the pass runner's progress reporting.
    ///
    /// The four `instanceof` arms are transcribed in Java's order. **The fourth is dead** and the
    /// port keeps it anyway: `DrillItem`'s only subclasses in the whole tree are `Via` and `Pin`
    /// (`grep -rl "extends DrillItem"` answers exactly `board/model/items/Via.java` and
    /// `Pin.java`), and both are matched two arms earlier. It is transcribed rather than dropped
    /// because a reader comparing the two files must find `:293-295`, and because a future
    /// `DrillItem` subclass would make it live in Java.
    ///
    /// Two deviations from Java's `Point[]`, both unreachable and both named rather than hidden:
    ///
    /// * an id the board does not know answers the empty `Vec`, which is Java's `new Point[0]` at
    ///   `:296` — Java would have NPE'd on a `null` item, but no caller can hand it one;
    /// * Java's `:285` always builds a **two-element** array for a trace, whose entries may be
    ///   `null`; the port pushes only the corners that exist, so a degenerate polyline would give
    ///   0 or 1 points where Java gives 2 nulls. `PolylineTrace.firstCorner`/`lastCorner` read
    ///   `polyline.corner(0)` / `corner(cornerCount - 1)` on a polyline the constructor refuses to
    ///   build with fewer than two corners (`BasicBoard.java:185-187`), so neither is ever
    ///   `null` on a board item, and the only consumer — `AutoroutePassRunner`'s progress
    ///   reporting — reads the array's length rather than indexing it.
    pub fn impacted_points(board: &Board, item: ItemId) -> Vec<Point> {
        let ctx = board.ctx();
        match board.get_item(item) {
            // :284-286.
            Some(Item::Trace(trace)) => {
                let mut result = Vec::new();
                if let Some(first) = trace.first_corner() {
                    result.push(first);
                }
                if let Some(last) = trace.last_corner() {
                    result.push(last);
                }
                result
            }
            // :287-289.
            Some(Item::Via(via)) => vec![via.get_center()],
            // :290-292.
            Some(Item::Pin(pin)) => vec![pin.get_center(&ctx)],
            // :293-295 — dead; see the doc comment.
            // :296.
            _ => Vec::new(),
        }
    }

    // ---------------------------------------------------------------------------------------------
    // enforceStrictDrc — BatchAutorouter.java:299-329
    // ---------------------------------------------------------------------------------------------

    /// Port of `enforceStrictDrc(RoutingBoard, int, int)` (`:305-329`): "if any trace/via inserted
    /// by the connection that just routed (item id above `maxItemIdBefore`) carries a clearance
    /// violation, rip the whole set of new items and report the connection FAILED, so the pass
    /// counts it as not routed and later passes (higher ripup costs) retry it. Returns null when
    /// the connection is clean and may be kept."
    ///
    /// `None` is Java's `null` — keep the connection. `Some(result)` is the rejection, whose
    /// `details` string is Java's, character for character, because
    /// `AutorouteConnectionRouter.route` returns it to the pass runner and Task 9's failure log
    /// writes it out.
    ///
    /// # The id test is the port's own monotone counter
    ///
    /// Java reads `board.communication.idGenerator.maxGeneratedId()` before the route
    /// (`AutorouteConnectionRouter.java:83`) and compares `currentItem.getId() > maxItemIdBefore`
    /// after. The port's [`ItemId`] comes from the same monotone counter
    /// (`fr_board::ids::ItemIdGenerator`), and Plan 6 Task 17's acceptance line "the item ids each
    /// connection burned" proves the two agree connection by connection over the corpus.
    ///
    /// # The walk order
    ///
    /// `board.getConnectableItems(netNo)` is `BoardConnectivityQueries.java:23-35` over
    /// `itemList`, i.e. **descending item id** (quirk #63), and
    /// [`Board::get_connectable_items`] answers the same order. It decides the order
    /// `newItems` is built in and therefore the order `removeItems` deletes in.
    pub fn enforce_strict_drc(
        board: &mut Board,
        route_net_no: i32,
        max_item_id_before: ItemId,
    ) -> Option<AutorouteAttemptResult> {
        // :307-308.
        let mut new_items: Vec<ItemId> = Vec::new();
        let mut has_violation = false;
        // :309.
        for current_item in board.get_connectable_items(route_net_no) {
            // :310-314.
            if current_item <= max_item_id_before
                || !matches!(
                    board.get_item(current_item),
                    Some(Item::Trace(_) | Item::Via(_))
                )
            {
                continue;
            }
            // :315.
            new_items.push(current_item);
            // :316-318. `!hasViolation &&` short-circuits, so Java stops asking once one item has
            // answered — the port keeps the short circuit because `clearanceViolations()` is the
            // expensive half.
            if !has_violation && !board.clearance_violations(current_item).is_empty() {
                has_violation = true;
            }
        }
        // :320-322.
        if !has_violation {
            return None;
        }
        // :323.
        let removed = new_items.len();
        board.remove_items(new_items);
        // :324-328.
        Some(AutorouteAttemptResult::with_details(
            AutorouteAttemptState::Failed,
            format!(
                "strict_drc: connection ripped because {removed} new item(s) included clearance \
                 violations"
            ),
        ))
    }

    // ---------------------------------------------------------------------------------------------
    // shouldFireBoardUpdate — BatchAutorouter.java:335-343
    // ---------------------------------------------------------------------------------------------

    /// Port of `shouldFireBoardUpdate()` (`:335-343`): "limit updates to 4 times per second
    /// (250 ms)".
    ///
    /// Java's body is `System.currentTimeMillis() - lastBoardUpdateTimestamp > 250`, with the
    /// timestamp rewritten only when the gate fires. That is exactly
    /// [`ProgressThrottler::board_update_gate`], whose `250` is
    /// [`RouterBudget::board_update_throttle_ms`] so a driver can pin it (ruling AI).
    ///
    /// Progress only — ruling 11 forbids any port decision from reading it.
    ///
    /// The plan's draft gave this method a `budget` parameter. Java has none, and a per-call
    /// budget would rebuild the gate and lose the timestamp; the budget is a constructor argument
    /// instead, which is where Java's literal lives.
    pub fn should_fire_board_update(&self) -> bool {
        self.board_update_gate.should_update()
    }

    /// [`BatchAutorouter::should_fire_board_update`] with the clock supplied — the seam
    /// [`ProgressThrottler::should_update_at`] exists for. No Java counterpart; tests only.
    pub fn should_fire_board_update_at(&self, now: Instant) -> bool {
        self.board_update_gate.should_update_at(now)
    }

    // ---------------------------------------------------------------------------------------------
    // removeTails — BatchAutorouter.java:487-503
    // ---------------------------------------------------------------------------------------------

    /// Port of `removeTails(Item.StopConnectionOption)` (`:487-503`): mark, strip every trace/via
    /// stub of every net, then pull the changed area tight.
    ///
    /// Three statements, in Java's order:
    ///
    /// 1. `:489` `board.startMarkingChangedArea()` — without it `optChangedArea` returns at
    ///    `RoutingBoardOperations.java:61-63` and the sweep never runs;
    /// 2. `:490` `board.removeTraceTails(-1, stopConnectionOption)` — `-1` is "all nets"
    ///    (`RoutingBoard.java:1200`'s `netNumber > 0` test);
    /// 3. `:492-498` `board.optChangedArea(new int[0], null, tracePullTightAccuracy, traceCosts,
    ///    thread, TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP)` — all nets, **no clip shape** (ruling 9:
    ///    `null` runs the sweep), this router's accuracy and cost array, and the 1000 ms budget
    ///    that is [`RouterBudget::opt_changed_area_ms`].
    ///
    /// The two `System.nanoTime()` blocks (`:488`, `:491`, `:499-502`) are the profile counters —
    /// see the module doc.
    ///
    /// `Err` propagates: Java throws out of `removeTraceTails`/`combineTraces` the same way, and
    /// both of this method's callers (`AutoroutePassRunner.java:298-302` and
    /// `BatchAutorouter.java:276`) are inside a caller-level boundary rather than swallowing it.
    pub fn remove_tails(
        &self,
        board: &mut Board,
        engine: Option<&mut AutorouteEngine>,
        stop_connection_option: StopConnectionOption,
        stop: StopCheck<'_>,
    ) -> Result<(), BoardError> {
        // :489.
        board.start_marking_changed_area();
        // :490.
        board.remove_trace_tails(-1, stop_connection_option)?;
        // :492-498.
        board.opt_changed_area(
            engine,
            &[],
            None,
            self.trace_pull_tight_accuracy,
            Some(&self.trace_costs),
            stop,
            self.budget.opt_changed_area_ms,
        )
    }

    // ---------------------------------------------------------------------------------------------
    // autorouteItem — BatchAutorouter.java:505-514
    // ---------------------------------------------------------------------------------------------

    /// Port of `autorouteItem(Item, int, SortedSet<Item>, Map<Item,Integer>, int)` (`:507-514`):
    /// "tries to route an item on a specific net. Returns true, if the item is routed." — one
    /// delegation to `connectionRouter.route`, which is [`route_connection_full`].
    ///
    /// The five values Java reads off `router` inside `route:38-47` are handed over here, so the
    /// wrapper's signature stays the one plan-7 ruling 2 fixed.
    #[allow(clippy::too_many_arguments)]
    pub fn autoroute_item(
        &self,
        board: &mut Board,
        engine: &mut Option<AutorouteEngine>,
        item: ItemId,
        route_net_no: i32,
        ripped_item_list: &mut BTreeSet<ItemId>,
        ripup_costs: &mut BTreeMap<ItemId, i32>,
        ripup_pass_no: i32,
        stop: StopCheck<'_>,
    ) -> AutorouteAttemptResult {
        // :513.
        route_connection_full(
            board,
            engine,
            item,
            route_net_no,
            self.settings,
            &self.trace_costs,
            ripped_item_list,
            ripup_costs,
            ripup_pass_no,
            self.start_ripup_costs,
            self.remove_unconnected_vias,
            self.trace_pull_tight_accuracy,
            self.budget,
            stop,
        )
    }

    // ---------------------------------------------------------------------------------------------
    // getAutorouteItems — BatchAutorouter.java:345-409
    // ---------------------------------------------------------------------------------------------

    /// Port of `getAutorouteItems(RoutingBoard)` (`:345-409`) — the pass's work list.
    ///
    /// Walks `board.itemList` **descending** (`:351-357`, quirk #63: `UndoableObjects.objects` is
    /// a `ConcurrentSkipListMap` keyed by `Item`, whose `compareTo` is `other.id - id`,
    /// `Item.java:95-101`), keeps `Connectable` items that are **not** routable and not already
    /// in `handledItems` (`:357-360`), and for each net index of such an item computes
    /// `getConnectedSet(netNo)` (`:363-365`), marking every member with `netCount() <= 1` as
    /// handled (`:366-370`). The item is appended when its connected set is smaller than
    /// `board.connectableItemCount(netNo)` and it has no ignored nets (`:375`), and **not** when
    /// the net contains a plane and the connected set already holds a `ConductionArea`
    /// (`:383-389`).
    ///
    /// # Java bug (plan-7 ruling 10, quirk #213), **fixed here**: an item was routed once per
    /// qualifying net **times** its whole net count
    ///
    /// `:390`'s `autorouteItemList.add(currentItem)` is **inside** the net loop, so a
    /// two-net item that qualifies on both nets appears **twice** in the list — and
    /// `AutoroutePassRunner.java:202, :207` then loops over *every* net index of *each*
    /// appearance, so that item was routed **four** times in one pass. Worse, the inner index is
    /// a fresh `0..netCount()` walk rather than the index that qualified, so the pair actually
    /// routed at appearance *a*, index *b* had nothing to do with the reason the item was
    /// enqueued.
    ///
    /// It is not merely wasteful: each repeat runs against the board the previous one left, so
    /// the extra attempts route real connections and rip real traces.
    ///
    // Java bug: `BatchAutorouter.getAutorouteItems` (`:390`) — the append is inside the per-net loop, so a multi-net item enters the work list once per qualifying net and `AutoroutePassRunner:202,207` then routes it netCount times per appearance (quirk #213).
    // fixed: T9 (#213) — the work list carries `(ItemId, net number)` **pairs**, which is the
    // register row's own `List<Map.Entry<Item,Integer>>`. The append still happens once per
    // *qualifying* net, because that is what "once per net that actually needs routing" means;
    // what is gone is `AutoroutePassRunner:207`'s fresh `0..netCount()` walk per appearance, so a
    // two-net item that qualifies on both nets is routed **twice** — once for each qualifying net
    // — instead of four times on net indices unrelated to the ones that qualified it.
    ///
    /// # `&self, &Board`, not `&mut self, &mut Board`
    ///
    /// The plan's sketch took both by `&mut`. Nothing here mutates: Java's two mutations are
    /// `reusableAutorouteItemList.clear()` and `reusableHandledItems.clear()` (`:347-348`), the
    /// allocation-reuse fields the port's field block rosters `// not ported:` because the port
    /// returns a fresh [`Vec`] and Java clears both before every use. Narrowing costs a caller
    /// nothing — a `&mut Board` reborrows — and it is what lets the pass runner hold the list
    /// while it mutates the board.
    ///
    /// # The second element is a net **number**, not a net index
    ///
    /// `:364`'s `currentNetNumber = currentItem.getNetNumber(i)` is what qualified the entry at
    /// `:375`, and it is what `AutoroutePassRunner:239` hands to `autorouteItem`. Carrying the
    /// *number* rather than the index `i` is what makes the fix a fix: `RoutingBoard.
    /// reduceNetsOfRouteItems` can change an item's net list between connections (quirk #211), so
    /// an index re-read later in the pass can name a different net, which is the second half of
    /// what `:207` got wrong.
    ///
    /// **Corpus-latent.** No corpus board has a multi-net routable-candidate item, so every entry
    /// is `(item, item.getNetNumber(0))` and the pass walks exactly the pairs it walked before —
    /// which is why the evidence for this row is the directed test rather than a stem.
    pub fn autoroute_items(&self, board: &Board) -> Vec<(ItemId, i32)> {
        self.autoroute_items_with_handled(board).0
    }

    /// [`BatchAutorouter::autoroute_items`], with the `handledItems` set it built.
    ///
    /// **Not a Java method** — it is the observation seam `scripts/differential/rust/src/bin/p7t1.rs`
    /// needs. Java's driver reads the same set off the private `reusableHandledItems` field
    /// (`:74`) with `Field.setAccessible(true)` **after** the call, which works because Java
    /// reuses the collection and clears it at `:348`; the port allocates a fresh set per call
    /// (that field is `not ported:`), so there is nothing for a driver to reflect into and the
    /// set is returned instead. `getAutorouteItems`' own answer is `.0` and is unaffected.
    ///
    /// # `run.sh p7t1` MISMATCHes on the `ITEM` order **by design**, since Plan 9 Task 2
    ///
    /// The driver prints one `ITEM` line per work-list entry, in list order, and that order is
    /// what the sort below changes. **A `p7t1` MISMATCH confined to the order of the `ITEM`
    /// lines is R1 (#293) working, not the port drifting** — the jar has no sort, the port
    /// restores it, and the two therefore disagree on this seam for as long as the register row
    /// says `fixed: T2`. What still has to agree, and what a reader should check before calling
    /// a `p7t1` diff a defect:
    ///
    /// * the **multiset** of `ITEM` lines — same ids, same multiplicities. Quirk #213's
    ///   duplicate entries are membership, not order, and the sort is stable, so a duplicated
    ///   entry is still two `ITEM` lines with one id;
    /// * the `HANDLED` set, which this method builds before the sort and which the sort does not
    ///   touch;
    /// * every other line the driver prints.
    ///
    /// The same holds for `p7t2`, `p7t5`, `p7t9` and `p8t1`, which route a board and so inherit
    /// the order downstream. `crates/fr-router/README.md`'s "The work list is airline-sorted"
    /// section says it once more for a reader who arrives from the driver rather than from here.
    pub fn autoroute_items_with_handled(
        &self,
        board: &Board,
    ) -> (Vec<(ItemId, i32)>, BTreeSet<ItemId>) {
        // :347-350. The port allocates rather than reusing; see the doc.
        // fixed: T9 (#213) — `(item, qualifying net number)` pairs, Java's
        // `List<Map.Entry<Item,Integer>>`.
        let mut autoroute_item_list: Vec<(ItemId, i32)> = Vec::new();
        let mut handled_items: BTreeSet<ItemId> = BTreeSet::new();

        // :351-357 — `itemList.startReadObject()` / `readObject(it)`, i.e. descending item id.
        for current_item in board.items_in_board_order() {
            let Some(item) = board.get_item(current_item) else {
                continue;
            };
            // :357 — the bare `instanceof Connectable`, **not** `Item.isConnectable()`: the
            // latter also demands `netCount() > 0` (Item.java:868-871), and Java does not.
            if item.as_connectable().is_none() {
                continue;
            }
            // :358-360.
            if item.is_routable() || handled_items.contains(&current_item) {
                continue;
            }

            // :363 — "let's go through all nets of this item".
            for i in 0..item.net_count() {
                // :364.
                let current_net_number = item.get_net_number(i);
                // :365 — the one-argument `getConnectedSet`, i.e. `stopAtPlane = false`
                // (Item.java:596-598).
                let connected_set = board.connected_set(current_item, current_net_number, false);
                // :366-370.
                for connected in &connected_set {
                    if board
                        .get_item(*connected)
                        .is_some_and(|c| c.net_count() <= 1)
                    {
                        handled_items.insert(*connected);
                    }
                }
                // :372.
                let net_item_count = board.connectable_item_count(current_net_number);

                // :375-377. Java short-circuits, so `hasIgnoredNets` — which dereferences
                // `nets.get(netNumber)` without a null check (Item.java:1244) — is reached only
                // on an item whose connected set is short.
                if connected_set.len() >= net_item_count || board.has_ignored_nets(current_item) {
                    continue;
                }

                // :378, :383-389. `net != null && net.containsPlane()`, then "skip items whose
                // connected set already contains a `ConductionArea` (copper pour)". Items not yet
                // connected to the plane are still enqueued so they can be routed to the pour in
                // this pass.
                let net = board.rules.nets.get(current_net_number);
                if net.is_some_and(|net| net.contains_plane()) {
                    let already_connected_to_plane = connected_set
                        .iter()
                        .any(|id| matches!(board.get_item(*id), Some(Item::ConductionArea(_))));
                    if already_connected_to_plane {
                        continue;
                    }
                }

                // :390. Once per qualifying net — and, since the fix, carrying the net that
                // qualified it rather than leaving the pass runner to guess (quirk #213).
                autoroute_item_list.push((current_item, current_net_number));
                // :391-402 is the `FRLogger.debug` payload; not ported.
            }
        }

        // -----------------------------------------------------------------------------------
        // R1 (register row #293): the shortest-airline-first ordering, restored.
        //
        // `:407` returns the list in `board.itemList`'s **descending-id** walk order (quirk #63)
        // and nothing sorts it, because commit `933d2980` ("Remove useSlowAlgorithm parameter
        // from autorouter", v2.2.0) deleted
        // `autorouteItemList.sort(Comparator.comparingDouble(this::calculateItemDistance))` with
        // the note "Disabled in v2.3 because it negatively impacts convergence compared to v1.9
        // (natural order)". `benchmark/reports/java-regressions-2026-09.md` §"Regression 1"
        // measures the note wrong: fully-connected on its small tier drops 0.81 -> 0.75 at
        // v2.2.0 and never recovers, and restoring the sort alone at HEAD brings it back to
        // 0.76 -> 0.82 *and is slightly faster*. The three methods the deleted line called are
        // still in `AutorouteAirlineCalculator.java` with no caller — see
        // `crate::pipeline::airline`'s module doc.
        //
        // **Unconditionally**, because no tier measured worse with it, and **stably**, because
        // `List.sort` is a TimSort and keeps equal keys in insertion order: ties therefore stay
        // in the descending-id walk order this loop just built, so the sort is a refinement of
        // today's order rather than a second, independent reordering. `f64::total_cmp` is
        // `Double.compare`'s total order — the one `Comparator.comparingDouble` uses — including
        // its treatment of `Double.MAX_VALUE` (an item on no net, `airline.rs:163-165`) and of
        // the exact `0` an already-connected item gets (`:172-174`).
        //
        // The key is computed **once per element**, over one shared
        // [`ItemDistanceCache`](crate::pipeline::airline::ItemDistanceCache), and sorted with the
        // ids — where Java's `comparingDouble` re-extracts it on every comparison and re-walks
        // the board inside each. `calculateItemDistance` is a pure function of a board this
        // method does not mutate, so the order is identical and only the bill differs; the
        // cache's own doc lists the three things it memoises and why each was measured.
        //
        // Java bug: `BatchAutorouter.getAutorouteItems` (`:345-409`) — the work list is returned in `board.itemList` descending-id order because `933d2980` deleted the `calculateItemDistance` sort; the commit's own justification is contradicted by measurement (quirk #293).
        // fixed: T2 (#293) — the sort is restored here, ascending, stable, unconditional.
        // The key is a property of the **item**, so it is computed once per `(item, net)` pair
        // (quirk #213's shape) over the shared cache, and the sort's stability then keeps an
        // item's own pairs in the qualifying order this loop produced them in.
        let mut cache = ItemDistanceCache::default();
        let mut keyed: Vec<(f64, (ItemId, i32))> = autoroute_item_list
            .iter()
            .map(|entry| {
                (
                    calculate_item_distance_cached(board, entry.0, &mut cache),
                    *entry,
                )
            })
            .collect();
        keyed.sort_by(|a, b| a.0.total_cmp(&b.0));
        let autoroute_item_list: Vec<(ItemId, i32)> =
            keyed.into_iter().map(|(_, entry)| entry).collect();

        // :407.
        (autoroute_item_list, handled_items)
    }

    // ---------------------------------------------------------------------------------------------
    // autoroutePass — BatchAutorouter.java:415-421
    // ---------------------------------------------------------------------------------------------

    /// Port of `autoroutePass(int passNo)` (`:415-421`): "auto-routes one ripup pass of all items
    /// of the board. Returns false, if the board is already completely routed." — one delegation
    /// to [`AutoroutePassRunner::run_single_thread`].
    ///
    /// Java reads everything else off `this`; the port hands the four things the runner cannot
    /// reach through `&mut self` — the board, the failure log (whose ownership note is on
    /// [`RoutingFailureLog`]), the stop flag and the progress sink — straight through.
    pub fn autoroute_pass(
        &mut self,
        board: &mut Board,
        failure_log: &mut RoutingFailureLog,
        pass_no: i32,
        stop: &RouterStop,
        progress: &mut dyn ProgressSink,
    ) -> Result<bool, RouterError> {
        // :421.
        AutoroutePassRunner::run_single_thread(board, self, failure_log, pass_no, stop, progress)
    }

    // ---------------------------------------------------------------------------------------------
    // autoroutePassesForOptimizingItem — BatchAutorouter.java:245-281
    // ---------------------------------------------------------------------------------------------

    /// Port of the static `autoroutePassesForOptimizingItem(RoutingJob, int, int, int, boolean,
    /// RoutingBoard, RouterSettings)` (`:245-281`): "auto-routes ripup passes until the board is
    /// completed or the auto-router is stopped by the user, or if `maxPassCount` is exceeded. Is
    /// currently used in the optimize via batch pass. Returns the number of passes to complete the
    /// board or `maxPassCount + 1`, if the board is not completed."
    ///
    /// This is the optimizer's **own** autorouter: a second [`BatchAutorouter`], built fresh per
    /// optimized item, with three things the pass loop's router does not have.
    ///
    /// 1. **`removeUnconnectedVias = true`, unconditionally** (`:258`). The `RoutingJob`
    ///    constructor derives it as `!settings.isFanoutEnabled()` (`:115`); here it is a literal,
    ///    so the optimizer strips fanout vias even on a board whose routing stage kept them. That
    ///    is what makes `removeTails(NONE)` at `:276` the *whole*-tail removal rather than
    ///    `FanoutVia`'s partial one (`AutoroutePassRunner.java:298-302` picks between the two on
    ///    exactly this flag).
    /// 2. **`withPreferredDirections` is the caller's** (`:259`), i.e. `optRoutePass`' alternating
    ///    `passNo % 2 != 0` — so every second optimizer pass routes with the direction costs
    ///    flattened.
    /// 3. **`isOptimizerAutorouter = true`** (`:263`), which `AutorouteBatchLoop.java:42, :374`
    ///    read. Nothing in *this* loop reads it; it is set for a `AutorouteBatchLoop.run` that is
    ///    never entered on this path, and the port sets it because the field exists and a reader
    ///    comparing the two files must find `:263`.
    ///
    /// # Java bug (quirk #225): the empty `if` at `:271-273`
    ///
    /// `if (stillUnroutedItems && !isStopAutoRouterRequested() && updatedRoutingBoard == null) {}`
    /// has an **empty body**, and its third conjunct cannot be true: `updatedRoutingBoard` is
    /// dereferenced unconditionally at `:256`, inside the constructor call five lines above, so a
    /// `null` board would have thrown a `NullPointerException` before the loop was entered. The
    /// branch is dead twice over — no body to run, and a guard that cannot pass — and the port
    /// omits it.
    ///
    // Java bug: `BatchAutorouter.autoroutePassesForOptimizingItem` (`:271-273`) — an empty `if` body whose `updatedRoutingBoard == null` conjunct is unreachable, because `:256` dereferences the same reference (quirk #225).
    // fixed: T9 (#225) — the row's suggested fix is "delete the `if`", and the port has never
    // carried it: the omission **is** the deletion and this marker is its record. The row's
    // alternative — writing the fanout-recovery body `AutorouteBatchLoop.java:435-439` grew into
    // it — is a different program and is not taken. **No behaviour change either side**, because
    // the branch has no body and a guard that cannot pass.
    ///
    /// # The failure log is local, and that is not observable
    ///
    /// Java's is `router.board.failureLog`, a `final` field of the board being optimized, so the
    /// optimizer's passes write into the same logbook the routing stage filled. The port owns the
    /// log at the caller (see [`RoutingFailureLog`]'s ownership note) and there is no caller-side
    /// log to thread here, so this builds one per call. The difference is unobservable: the log is
    /// write-only apart from `getFailureCount`, whose one reader is a dropped log-message guard
    /// (`AutoroutePassRunner.java:273`).
    ///
    /// # `Result<i32, …>`, not `Result<(), …>`
    ///
    /// The brief's sketch returns nothing. Java returns the pass count (`:280`), and although
    /// `optRouteItem:466` discards it, the number is what `p7t8 item` prints on both sides — so
    /// the port answers it and the driver compares it.
    #[allow(clippy::too_many_arguments)]
    pub fn autoroute_passes_for_optimizing_item(
        board: &mut Board,
        settings: &RouterSettings,
        max_pass_count: i32,
        ripup_costs: i32,
        trace_pull_tight_accuracy: i32,
        with_preferred_directions: bool,
        stop: &RouterStop,
        budget: RouterBudget,
        progress: &mut dyn ProgressSink,
    ) -> Result<i32, RouterError> {
        // :253-261.
        let mut router_instance = BatchAutorouter::new(
            board,
            settings,
            // :258 — unconditional.
            true,
            // :259.
            with_preferred_directions,
            // :260.
            ripup_costs,
            // :261.
            trace_pull_tight_accuracy,
            budget,
        );
        // :262 — `routerInstance.job = job`; Plan 8's, see the field block's marker.
        // :263.
        router_instance.is_optimizer_autorouter = true;

        // :265-266.
        let mut still_unrouted_items = true;
        let mut current_pass_no: i32 = 1;
        let mut failure_log = RoutingFailureLog::new();

        // :267-275.
        while still_unrouted_items
            && !stop.is_stop_auto_router_requested()
            && current_pass_no <= max_pass_count
        {
            // :270.
            still_unrouted_items = router_instance.autoroute_pass(
                board,
                &mut failure_log,
                current_pass_no,
                stop,
                progress,
            )?;
            // :271-273 — the empty `if`; see the doc comment.
            // :274.
            current_pass_no += 1;
        }

        // :276 — `NONE`, not `FANOUT_VIA`: this router always removes unconnected vias.
        router_instance.remove_tails(board, None, StopConnectionOption::None, &|| {
            stop.is_stop_requested()
        })?;
        // :277-279.
        if !still_unrouted_items {
            current_pass_no -= 1;
        }
        // :280.
        Ok(current_pass_no)
    }

    // ---------------------------------------------------------------------------------------------
    // calculateIncompleteCount — BatchAutorouter.java:556-564
    // ---------------------------------------------------------------------------------------------

    /// Port of `calculateIncompleteCount(RoutingBoard)` (`:556-564`): a throw-away
    /// `DesignRulesChecker` over the whole board, `calculateAllIncompletes()`, then
    /// `getIncompleteCount()`.
    ///
    /// Java's `new DesignRulesChecker(board, null)` is the port's
    /// [`DesignRulesChecker::new`] — plan ruling 3 is why `fr-drc` is a real dependency. The
    /// explicit `calculateAllIncompletes()` at `:559` is kept even though
    /// [`DesignRulesChecker::get_incomplete_count`] would compute it lazily, because Java's call
    /// order is what decides which of the checker's caches are warm.
    pub fn calculate_incomplete_count(board: &mut Board) -> usize {
        // :558.
        let mut temp_drc = DesignRulesChecker::new(board);
        // :559.
        temp_drc.calculate_all_incompletes();
        // :563.
        temp_drc.get_incomplete_count()
    }
}

// =================================================================================================
// The deferral roster for `autoroute/pipeline/BatchAutorouter.java`
// =================================================================================================

// renamed: `BatchAutorouter.runBatchLoop` (`:479-481`) -> [`crate::pipeline::AutorouteBatchLoop::run`], Plan 7 Task 10. Java's method is `return batchLoop.run();` over a `BatchAutorouter` field the constructor built (`:104`); the port's `run` **builds the router itself** — as `RoutingPipeline`'s constructor does at `RoutingPipeline.java:34` — so there is no object for a wrapper to delegate through and the one-line method collapses into the loop it names. Task 15's `run_pipeline` calls `AutorouteBatchLoop::run` where Java calls `runBatchLoop`.
// renamed: `BatchAutorouter.buildUnroutedConnectionsReport` (`:483-485`) -> [`crate::pipeline::build_unrouted_report`], **Plan 7 Task 15**. Java's method is one delegation, `return AutorouteUnroutedReport.build(board);`, and it is package-private, so `audit-port.sh`'s public scan never names it either way. Its two callers are `AutorouteBatchLoop.java:457` and `:487` — the loop's two stagnation arms — which is why Plan 7 Task 10 landed a stub behind an `obligation:` marker and Task 15 discharged it with the real report. The port collapses the delegation the way it collapses `runBatchLoop`: [`crate::pipeline::AutorouteBatchLoop`] calls `build_unrouted_report` directly.
// not ported: `BatchAutorouter.getAirLine` (`:516-527`) — the GUI airline accessor of the `not ported:` `airLine` field. Task 9 measured plan ruling 6's claim and confirms it: the field's only writers are `AutoroutePassRunner.java:45, 81, 142, 146, 164, 329, 333` (all `= null`) and `AutorouteConnectionRouter.java:70`, and this accessor is its only reader — nothing headless calls it, so [`crate::pipeline::calculate_airline`] is ported for the audit and has no caller.
// not ported: `BatchAutorouter.autoroutePassMultiThread` (`:411-413`) — one delegation to `AutoroutePassRunner.runMultiThread`, the dead multithreaded path (quirk **#143**, extended by Plan 7 Task 17 — the plan's *label* for this was #216, which the register spent on `alreadyRoutedBoardHashes`; `grep -rn autoroutePassMultiThread src/main src/test` answers the declaration and nothing else).
// not ported: `BatchAutorouter.setAirLine` (`:192-194`) — the writer of the `not ported:` `airLine` field.
// not ported: `BatchAutorouter.addProfileMazeSearchNanos` (`:184-186`), `BatchAutorouter.addProfileOptChangedAreaNanos` (`:188-190`), `BatchAutorouter.resetPassProfile` (`:196-207`), `BatchAutorouter.logBenchmarkProfile` (`:209-238`) — the benchmark profile, all four guarded by `isBenchmarkProfileEnabled()`, i.e. `-Dfreerouting.benchmark.profile`, default `false`.
// not ported: `BatchAutorouter.threadIndexToLetter` (`:536-554`) — a log-string helper (`0 -> "A"`, `26 -> "AA"`) for the multithreaded pass. Its only callers are `AutoroutePassRunner.java`'s `log*` helpers on the `runMultiThread` path, which `global-constraints.md` drops with the rest of `FRLogger`; a tree-wide `grep -rn threadIndexToLetter src/main` answers `BatchAutorouter.java:536` (the declaration) and `AutoroutePassRunner.java:66, 92` (both inside `runMultiThread`, `:40-149`).

/// `RouterStop` is the port of the `StoppableThread` this class holds as `NamedAlgorithm.thread`
/// (`NamedAlgorithm.java:25`). It is **not** a field of [`BatchAutorouter`]: plan-6 ruling 6 makes
/// cancellation a per-call [`StopCheck`], so every method that Java would have read the field in
/// takes one instead. This alias exists so the roster above can name the type a reader is looking
/// for.
pub type BatchAutorouterStop = RouterStop;
