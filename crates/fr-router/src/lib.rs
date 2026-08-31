#![forbid(unsafe_code)]

//! `fr-router`: freerouting's maze/expansion autorouter — a behavioral Rust port of
//! `app.freerouting.autoroute.{,maze,expansion,drill,path}` plus the `board/actions` and
//! `board/optimize` classes the router drives (freerouting, clone HEAD).
//!
//! HEAD is the authority for both the sources and the parity jar: HEAD's `autoroute/**` has been
//! refactored away from upstream 2.3.0 (`MazeSearchAlgo` → `maze/MazeSearchEngine`,
//! `LocateFoundConnectionAlgo*` → `path/FoundConnectionLocator*`, and `MazeExpansionEngine`,
//! `MazeRipupResolver` and `AutorouteConnectionRouter` do not exist upstream at all), so a 2.3.0
//! reference would be a different algorithm.
//!
//! See `docs/superpowers/plans/2026-08-29-plan-6-router-maze.md` for the scope, the seam with
//! Plan 7 and the seventeen rulings; `README.md` for the API surface and the parity story.
//!
//! # State
//!
//! **Complete (Task 18 of 18).** What the crate holds is the data-model floor: [`Arena`] and its index
//! newtypes, the per-connection outcome ([`AutorouteAttemptResult`]), the per-item scratch
//! accessors ([`autoroute::item_info`]), and — from Task 2 — the expansion rooms, the doors and
//! [`MazeSearchElement`] ([`autoroute::expansion`]), which is also where `fr-board`'s reserved
//! `TreeObject::Room` becomes real. Task 3 added the geometric heart of the expansion,
//! [`AutorouteSearchTreeExt`] ([`autoroute::tree_ext`]): `completeShape` and `divideLargeRoom`
//! in all three angle regimes, which is where `fr-board`'s search tree stops being read-only for
//! the router. Tasks 4 and 5 added the three neighbour sorters that turn a completed room into
//! its door list, and Task 6 adds [`AutorouteEngine`]
//! ([`autoroute::maze::engine`]) — the room lifecycle: construction, `initConnection`, `clear`,
//! `completeExpansionRoom`, `completeNeighbourRooms`, the door reset and removal, and ruling 7's
//! first recovery boundary. Task 7 adds [`autoroute::drill`]: [`DrillPage`], [`DrillPageArray`]
//! and [`ExpansionDrill`], which is where the maze search gets its layer changes and where
//! `AutorouteEngine`'s three drill hooks stop being stubs. Task 8 adds the four leaf types the
//! maze search is written against ([`AutorouteControl`], [`DestinationDistance`],
//! [`MazeListElement`], [`MazeQueue`]), and Task 9 adds [`board_ext`]: [`RoutingBoardExt`] —
//! the five `RoutingBoard` methods plan-2 ruling 4 left out of `fr-board` — plus the
//! **check-only** half of [`TraceShover`] and [`DrillItemMover`], the shove algorithms the maze
//! consults through `checkForcedTracePolyline`. Task 10 closes their cycle with
//! [`ForcedPadRouter`] and adds [`ForcedViaInserter`], the two-phase gate the maze asks before it
//! places a via. Task 11 adds [`MazeSearchEngine`] itself — construction, `getInstance`, `init`,
//! the pop loop and the four helpers those need, with the expanders still stubs. Task 12 adds the
//! room-door expansion, the A\* cost model and `MazeTraceShover`, and Task 13 the two classes
//! that close `autoroute/maze` — `MazeExpansionEngine` (drill pages, drills and candidate via
//! layers) and `MazeRipupResolver` (the ripup decision and its cost model) — together with
//! [`autoroute::path`]'s [`Connection`], the memoised run of routable items that cost model
//! divides by. With those in place [`MazeSearchEngine::find_connection`] runs end to end. The
//! path locators arrive in Task 14 and the connection inserter in Task 15, with the pull-tight
//! and forced-trace families of Tasks 15a/15b underneath them. **Task 16 closes the loop:**
//! [`AutorouteEngine::autoroute_connection`] runs a whole connection end to end — maze search,
//! locate, ripped-connection deletion, insert — and [`route_connection`] is the Plan 6 half of
//! the seam with Plan 7 (`AutorouteConnectionRouter.route` steps 1-5, ruling 2), which is the
//! entry point Plan 7's pass runner and Task 17's `p6t1` call. **Task 17 turns that loop on a
//! real board:** `scripts/differential/{java/P6T1.java,rust/src/bin/p6t1.rs}` route a DSN
//! connection by connection against the HEAD jar, `scripts/gen-router-reference.sh` commits the
//! jar's answers to `tests/reference/router-*/`, and `tests/reference_parity.rs` walks plan-6
//! ruling 1's ladder over them — **369 connections on five boards, all three rungs, including
//! the inserted geometry and the item ids each connection burned**, at `ripupPassNo` 1, 2 and 4.
//! **Task 18 closes the plan:** the three in-scope Java suites are ported by name
//! (`tests/java_ports.rs`), `tests/fixtures.rs` is the single-pass stand-in for
//! `RoutingFixtureTest`'s assertion family, every audit invocation exits 0 with no `MISSING` and
//! no `UNMAPPED`, and `docs/plan-6-handoff.md` is what Plan 7 starts from. The roster at the foot
//! of this file names each still-deferred class and the plan that owns it.
//!
//! # House rules
//!
//! * **Single-threaded** (ruling 17). No `rayon`, no `std::thread`. `Board` stays `Send + Sync`
//!   for Plan 7's optimizer, but nothing here spawns. Java's `-mt` is parsed, clamped, mirrored
//!   and then read by nothing on the headless path (`docs/java-quirks.md` #143), so there is no
//!   threading policy to reproduce — and a threaded maze would be non-deterministic, which would
//!   dissolve every acceptance criterion in ruling 1.
//! * **No new workspace dependencies** (rulings 5, 16, 17): no `rand` — [`fr_geometry::JavaRandom`]
//!   reproduces `java.util.Random` bit for bit — no `slotmap` — [`Arena`] is 60 lines and, unlike
//!   a generational key, keeps Java's stale-reference semantics — and no `rayon`.
//! * **Deterministic containers, transcribed rather than chosen** (ruling 4, and ruling Y/Z for
//!   the maze queue): a sorted set, never `BinaryHeap`, and [`JavaTreeSet`] rather than
//!   `BTreeSet` wherever the comparator is not a total order — the two keep and order different
//!   elements there. Comparators are transcribed as Java's `<`/`>` chains, never `total_cmp`.
//! * **No GUI, no `FRLogger`, no observers**, and no static mutable state — with **one recorded
//!   exception**, controller ruling AE: `fr_geometry::Line` carries an identity token drawn from
//!   a process-wide `AtomicU64`, because `PolylineTrace.change` compares `Line`s by *reference*
//!   (quirk #74) and the difference is board-observable. One reader, order-independent.
//!   Java's *observers* — `NamedAlgorithm`'s three listener lists and `autoroute/events/**` — are
//!   replaced by one [`pipeline::ProgressSink`] threaded as `&mut dyn` (controller ruling AK), and
//!   plan-7 ruling 11 makes it an observer in the strict sense: no port decision reads it.
//! * **`#![forbid(unsafe_code)]`** in this crate root and in every other workspace crate. The only
//!   `unsafe` left in the repository is the `static mut` PRNG in
//!   `scripts/differential/rust/src/bin/p2t13.rs`, which is a differential driver, not a crate.
//! * `ExpansionCostFactor` is **re-exported** from `fr-settings`, never redeclared (ruling 8 —
//!   the plan-4 obligation).

pub mod arena;
pub mod autoroute;
pub mod board_ext;
pub mod error;
pub mod java_tree_set;
pub mod pipeline;
pub mod score;

pub use arena::{Arena, DoorId, DrillId, IncompleteRoomId, PageId, TargetDoorId};
pub use autoroute::{
    AutorouteAttemptResult, AutorouteAttemptState, AutorouteControl, AutorouteEngine,
    AutorouteSearchTreeExt, CompleteFreeSpaceExpansionRoom, Connection, DestinationDistance,
    DrillPage, DrillPageArray, ExpandableRef, ExpansionDoor, ExpansionDrill, ExpansionRoomStore,
    FoundConnectionInserter, FoundConnectionLocator, FreeSpaceExpansionRoom,
    IncompleteFreeSpaceExpansionRoom, MazeAdjustment, MazeExpansionEngine, MazeListElement,
    MazeQueue, MazeResult, MazeRipupResolver, MazeSearchElement, MazeSearchEngine,
    ObstacleExpansionRoom, ResultItem, RoomRef, ShoveResult, TargetItemExpansionDoor, ViaMask,
    route_connection, route_connection_full,
};
pub use board_ext::{
    CheckDrillResult, DrillItemMover, ForcedPadRouter, ForcedViaInserter, RoutingBoardExt,
    SpringOverOutcome, TraceShover,
};
pub use error::RouterError;
pub use java_tree_set::JavaTreeSet;
pub use pipeline::{
    BatchAutorouter, BoardHistory, BoardHistoryEntry, NamedAlgorithmType, NoopProgressSink,
    PassRecord, ProgressSink, ProgressThrottler, RouterBudget, RouterCounters, RouterStop,
    RoutingEvent, StopRequestState, TaskState,
};
pub use score::{
    BoardStatistics, BoardStatisticsBends, BoardStatisticsBoard,
    BoardStatisticsClearanceViolations, BoardStatisticsComponents, BoardStatisticsConnections,
    BoardStatisticsFanout, BoardStatisticsItems, BoardStatisticsLayers, BoardStatisticsNets,
    BoardStatisticsPads, BoardStatisticsTraces, BoardStatisticsVias, Rectangle2DFloat,
};

/// `AutorouteControl.ExpansionCostFactor` (`autoroute/maze/AutorouteControl.java:287`), the
/// per-layer horizontal/vertical trace-cost pair.
///
/// Re-exported from `fr-settings`, where `RouterSettings.getTraceCosts` — its only producer —
/// lives. Plan-6 ruling 8 forbids a second declaration; this discharges the plan-4 obligation
/// recorded on the `fr-settings` definition.
pub use fr_settings::ExpansionCostFactor;

/// Re-exports every public type of the crate, for `use fr_router::prelude::*;`.
pub mod prelude {
    pub use crate::{
        Arena, AutorouteAttemptResult, AutorouteAttemptState, AutorouteControl, AutorouteEngine,
        AutorouteSearchTreeExt, BatchAutorouter, BoardHistory, BoardHistoryEntry, BoardStatistics,
        BoardStatisticsBends, BoardStatisticsBoard, BoardStatisticsClearanceViolations,
        BoardStatisticsComponents, BoardStatisticsConnections, BoardStatisticsFanout,
        BoardStatisticsItems, BoardStatisticsLayers, BoardStatisticsNets, BoardStatisticsPads,
        BoardStatisticsTraces, BoardStatisticsVias, CompleteFreeSpaceExpansionRoom, Connection,
        DestinationDistance, DoorId, DrillId, DrillItemMover, DrillPage, DrillPageArray,
        ExpandableRef, ExpansionCostFactor, ExpansionDoor, ExpansionDrill, ExpansionRoomStore,
        FoundConnectionInserter, FoundConnectionLocator, FreeSpaceExpansionRoom,
        IncompleteFreeSpaceExpansionRoom, IncompleteRoomId, JavaTreeSet, MazeAdjustment,
        MazeExpansionEngine, MazeListElement, MazeQueue, MazeResult, MazeRipupResolver,
        MazeSearchElement, MazeSearchEngine, NamedAlgorithmType, NoopProgressSink,
        ObstacleExpansionRoom, PageId, PassRecord, ProgressSink, ProgressThrottler,
        Rectangle2DFloat, ResultItem, RoomRef, RouterBudget, RouterCounters, RouterError,
        RouterStop, RoutingBoardExt, RoutingEvent, ShoveResult, SpringOverOutcome,
        StopRequestState, TargetDoorId, TargetItemExpansionDoor, TaskState, TraceShover, ViaMask,
        route_connection, route_connection_full,
    };
}

// =================================================================================================
// The deferral roster for `autoroute/*.java` (the package root).
//
// `scripts/audit-port.sh` reads these markers, so a deferral is a gate rather than a silence:
// `grep -rn "added in Task" crates/fr-router/src` lists everything still owed. The subpackages
// (`autoroute/{maze,expansion,drill,path}`, `board/actions`, `board/optimize`) are rostered by the
// modules that will hold them; `scripts/audit-map/fr-router.map` records where each class lands,
// and Task 18 runs the whole audit to zero MISSING and zero UNMAPPED.
// =================================================================================================

// --- No longer deferred: `autoroute/BoardHistory.java` --------------------------------------------
//
// `BoardHistory` is **ported** — `pipeline::BoardHistory`, Plan 7 Task 2 under controller ruling
// AF — and the eleven `// not ported: BoardHistory.*` lines that stood here are deleted with it.
// Plan-6 ruling 13 had rostered the class on spec §2's "undo store" exclusion; ruling AF overturns
// that reading. It is not an undo store: it is the pass loop's best-board memory
// (`AutorouteBatchLoop.java:284, 298-320, 525-547`), and dropping it would write the *last* pass's
// board to the SES instead of the *best* pass's. `scripts/audit-map/fr-router.map` now points the
// class at `pipeline/board_history.rs`, so the audit checks it there rather than here.

// --- Out of scope for Plan 6 and Plan 7 alike (ruling 13's not-ported roster) ---------------------
//
// `autoroute/BoardHistoryEntry.java` (33 loc) stays deferred, and permanently: the public,
// `Comparable` top-level class — which holds a live `RoutingBoard`, a `BoardStatistics` and an
// `Instant.now()` — is **shadowed** by `BoardHistory`'s own `private static class
// BoardHistoryEntry` (BoardHistory.java:188), declared in the same compilation unit, so the
// import at BoardHistory.java:1-12 never resolves to it and no caller anywhere in `src/main` or
// `src/test` reaches it. Quirk #199. The entry the port does have is the **nested** one
// (`pipeline::BoardHistoryEntry`, `board_history.rs`), which has no `compareTo` at all.
// not ported: `BoardHistoryEntry.compareTo` — `autoroute/BoardHistoryEntry.java:29-32`, the shadowed and unreachable top-level twin (quirk #199).
//
// `autoroute/AutorouteDiagnostic.java`: a GUI overlay sink (it declares no public methods of its
// own — the marker records the class). `autoroute/PerformanceProfiler.java`: a timing sink whose
// only output is `FRLogger`.
// not ported: `AutorouteDiagnostic` — a GUI overlay sink (`global-constraints.md`: no GUI).
//
// `autoroute/maze/MazeFanoutDiagnostics.java` (44 loc): the fanout search's `FRLogger.trace`
// payload builder. It is package-private and declares no public methods, so `audit-port.sh` has
// nothing to require of it — this line is what makes `scripts/audit-map/fr-router.map`'s
// `MazeFanoutDiagnostics -> lib.rs` row point at a marker rather than at nothing. The call sites
// it is dropped from carry their own `not ported:` at
// `src/autoroute/maze/search.rs` (`MazeSearchEngine.fanoutDiagnostics`, `:65`).
// not ported: `MazeFanoutDiagnostics` — an `FRLogger.trace` payload builder (ruling 13's roster).
// not ported: `PerformanceProfiler.start`
// not ported: `PerformanceProfiler.end`
// not ported: `PerformanceProfiler.reset`
// not ported: `PerformanceProfiler.recordPass`
// not ported: `PerformanceProfiler.recordConfiguration`
// not ported: `PerformanceProfiler.printResults`
// not ported: `PerformanceProfiler.PassInfo`
//
// `autoroute/BoardUpdateStrategy.java` and `autoroute/ItemSelectionStrategy.java` are bare enums
// with no methods; they are `RouterSettings` fields and are already ported in `fr-settings`.
// not ported: `BoardUpdateStrategy` — the enum is `fr_settings::RouterSettings`' field type.
// not ported: `ItemSelectionStrategy` — likewise.

// --- Plan 7's, not this plan's (ruling 2 puts the pass loop and the failure log there) ------------
//
// `autoroute/RoutingFailureLog.java`: read and written by `AutoroutePassRunner`, above the seam.
// added in Plan 7: `RoutingFailureLog.recordFailure`
// added in Plan 7: `RoutingFailureLog.shouldSkip`
// added in Plan 7: `RoutingFailureLog.shouldGiveUp`
// added in Plan 7: `RoutingFailureLog.getFailureCount`
// added in Plan 7: `RoutingFailureLog.getUnroutableItems`
// added in Plan 7: `RoutingFailureLog.hasUnroutableItems`
// added in Plan 7: `RoutingFailureLog.clear`
// added in Plan 7: `RoutingFailureLog.toString`
// added in Plan 7: `RoutingFailureLog.ItemFailureInfo`
//
// `autoroute/ItemRouteResult.java`: the per-item scorecard the pass runner sorts on.
// added in Plan 7: `ItemRouteResult.itemId`
// added in Plan 7: `ItemRouteResult.incompleteCount`
// added in Plan 7: `ItemRouteResult.incompleteCountBefore`
// added in Plan 7: `ItemRouteResult.viaCount`
// added in Plan 7: `ItemRouteResult.traceLength`
// added in Plan 7: `ItemRouteResult.improved`
// added in Plan 7: `ItemRouteResult.improvedOver`
// added in Plan 7: `ItemRouteResult.improvementPercentage`
// added in Plan 7: `ItemRouteResult.lengthReduced`
// added in Plan 7: `ItemRouteResult.viaCountReduced`
// added in Plan 7: `ItemRouteResult.updateImproved`
// added in Plan 7: `ItemRouteResult.compareTo`

// --- `autoroute/pipeline/**`: Plan 7's whole surface (ruling 2's seam) ----------------------------
//
// Sixteen classes, ~2 900 loc, and every one of them sits *above* `AutorouteEngine::
// autoroute_connection` / [`route_connection`]. `audit-port.sh` does not recurse, so the
// `autoroute` invocation never reaches them; the roster is what records them instead. Rulings AA
// and AB moved `ForcedPadRouter`'s routing half, `DrillItemMover`'s mutating half,
// `TraceShover.insert` and the whole `TraceTightener` family *into* Plan 6, so — unlike the plan
// as written — those are ported and are not on this list.
// `AutorouteConnectionRouter` is **ported in full** by Plan 7 Task 8: steps 1-5 are
// [`route_connection`] and the whole of `route` is [`route_connection_full`], with
// `retryConnectionNecked` (`:162-241`) and `applyStrictDrcAfterRoute` (`:243-254`) beside it in
// `autoroute/maze/engine.rs`, where `scripts/audit-map/fr-router.map` points the class.
// The line that stood here said "steps 6-8 … (`AutorouteConnectionRouter.java:160-233`)", and
// **that range is wrong** — `:160` is `route`'s closing brace and nothing in the 255-line file
// spans `:160-233` as a unit. The decomposition read out of HEAD is step 6 `:95-121`, step 7
// `:123-145`, step 8 `:147-153`. `docs/plan-6-handoff.md` §10.1 carries the same wrong range,
// and its "and the failure-log write" belongs to Task 9's `AutoroutePassRunner.runSingleThread`
// (`:260-289`), not to this class.
// added in Plan 7: `AutoroutePassRunner.runPass`, `AutoroutePassRunner.onBoardUpdatedEvent` — the per-pass item loop and its `catch (Exception)` recovery boundary (`AutoroutePassRunner.java:144`).
// added in Plan 7: `AutorouteBatchLoop.run` — the pass loop, and the `IllegalArgumentException` `RoutableLayersSafetyCheckTest` asserts (`AutorouteBatchLoop.java:52-55`).
// `BatchAutorouter` itself is [`pipeline::BatchAutorouter`] from Plan 7 Task 8 — the constants,
// the field block, both constructors, the five accessors, the five `NamedAlgorithm` identity
// members, `getImpactedPoints`, `enforceStrictDrc`, `isFanoutTimedOut`, `shouldFireBoardUpdate`,
// `removeTails`, `autorouteItem` and `calculateIncompleteCount`. `scripts/audit-map/fr-router.map`
// gained a second row for the class so both files are searched. `getInitialUnroutedCount` and
// `getSessionStartTime` are ported there too, so they are **not** on the line below; what is still
// owed is rostered **in `pipeline/batch_autorouter.rs`** beside the code, not here:
// added in Plan 7: `BatchAutorouter.runBatchLoop`, `BatchAutorouter.autoroutePassesForOptimizingItem`, `BatchAutorouter.getAirLine`.
// added in Plan 7: `BatchAutorouterThread.getBoard`, `BatchAutorouterThread.getRoutedCount`, `BatchAutorouterThread.getFailedCount`, `BatchAutorouterThread.addBoardUpdatedEventListener`, `BatchAutorouterThread.fireBoardUpdatedEvent` — including the per-item `catch (Exception)` boundary at `BatchAutorouterThread.java:537`.
// added in Plan 7: `BatchFanout.fanoutBoard`, `BatchFanout.compareTo`, `BatchFanout.fromBoardStatistics`, `BatchFanout.toString`, `BatchFanout.EscapeStatistics`, `BatchFanout.FanoutPassStatus`, `BatchFanout.FanoutRunSummary`.
// added in Plan 7: `BatchOptimizer.runBatchLoop`, `BatchOptimizer.createForGui`, `BatchOptimizer.createForHeadless`, `BatchOptimizer.getCurrentPosition`, `BatchOptimizer.getId`, `BatchOptimizer.isTimedOut`.
// added in Plan 7: `BatchOptimizerMultiThreaded.getNumTasks`, `BatchOptimizerMultiThreaded.getNumTasksFinished`, `BatchOptimizerMultiThreaded.getWinningCandidateScore`, `BatchOptimizerMultiThreaded.isWinningCandidate` — behind quirk #143: `-mt` is not a threading policy on the headless path, so Plan 7 must not make one out of it.
// added in Plan 7: `OptimizeRouteTask.run`, `OptimizeRouteTask.clean`, `OptimizeRouteTask.getItem`, `OptimizeRouteTask.getRouteResult`.
// added in Plan 7: `AutorouteAirlineCalculator` and `AutorouteRuntimeMetrics` — both package-private with no public members; the lines record the classes.
// added in Plan 7: `AutorouteUnroutedReport.build` — the stagnation report; it is a consumer of `fr-drc`, whose `crates/fr-drc/src/lib.rs` carries the same line.
// added in Plan 8: `RoutingPipeline.createForHeadless`, `RoutingPipeline.createForGui`, `RoutingPipeline.run`, `RoutingPipeline.getAutorouter`, `RoutingPipeline.getOptimizer`, `RoutingPipeline.addStageListener`, `RoutingPipeline.addBoardUpdatedEventListener`, `RoutingPipeline.addTaskStateChangedEventListener` — the wiring from the CLI/MCP surface into Plan 7's stages (spec §13); Plan 7 delivers the stages, Plan 8 the caller.
// not ported: `NamedAlgorithm.addBoardSnapshotEventListener`, `NamedAlgorithm.addBoardUpdatedEventListener`, `NamedAlgorithm.addTaskStateChangedEventListener`, `NamedAlgorithm.fireBoardSnapshotEvent`, `NamedAlgorithm.fireBoardUpdatedEvent`, `NamedAlgorithm.fireTaskStateChangedEvent` — the three listener lists (`NamedAlgorithm.java:26-31`) and their `add`/`fire` pairs; controller ruling AK replaces the whole mechanism with spec §10's [`pipeline::ProgressSink`], landed in Plan 7 Task 4.
// The two enums `NamedAlgorithmType` and `TaskState` are **ported** — [`pipeline::NamedAlgorithmType`] and [`pipeline::TaskState`], Plan 7 Task 4 — because `ProgressSink`'s `RoutingEvent::TaskStateChanged` carries both and `PipelineResult` reports a `TaskState`. The `// not ported:` line that stood here ("bare enums with no methods") is deleted with them.

// --- `core/**`: the four classes Plan 7 Task 4 reaches ------------------------------------------
//
// No `audit-port.sh` invocation covers `core/`'s package root (the workspace's thirty cover
// `core/library` and `core/scoring` only), so this block is a roster rather than a gate — it is
// here because Task 4 ports out of `core/` and a reader must be able to see what it took and what
// it left.
//
//   * `core/StopRequestState.java` (11) and `core/StoppableThread.java`'s flag and four methods
//     (`:8`, `:20-42`) are [`pipeline::StopRequestState`] and [`pipeline::RouterStop`].
//   * `core/RouterCounters.java` (48) is [`pipeline::RouterCounters`], nine fields in Java's
//     declaration order.
//   * `core/ProgressThrottler.java` (32) is [`pipeline::ProgressThrottler`], together with
//     `BatchAutorouter.shouldFireBoardUpdate`'s one-millisecond-stricter twin
//     (`autoroute/pipeline/BatchAutorouter.java:335-343`).
//
// `StoppableThread`'s one dropped method, `run`, is rostered **in `pipeline/stop.rs`** and not
// here: `audit-port.sh`'s per-class mode searches only the class's mapped path
// (`scripts/audit-map/fr-router.map` points `StoppableThread` at `pipeline/stop.rs`), so a marker
// in this file would not be found by a scoped `core` invocation. Task 4 review S3.

// --- `autoroute/events/**`: observers, replaced by `pipeline::ProgressSink` ----------------------
//
// Six files, ~130 loc, all of them `java.util.EventObject` subclasses and their listener
// interfaces. Nothing in the maze reads them: `AutorouteEngine.autorouteConnection` brackets its
// work with `fireBoardSnapshotEvent`-style calls at `:254-270` and the port drops them (ruling 2's
// "no observers"). Spec §10 gives the port a `ProgressSink` instead — [`pipeline::ProgressSink`]
// and [`pipeline::RoutingEvent`], landed in **Plan 7 Task 4** under controller ruling AK, with one
// `RoutingEvent` variant per `fire*` call the port keeps. Ruling 11: no port decision reads it.
// not ported: `BoardSnapshotEvent.getBoard` — and the class itself.
// not ported: `BoardSnapshotEventListener` — a one-method listener interface.
// not ported: `BoardUpdatedEvent.getBoard`, `BoardUpdatedEvent.getBoardStatistics`, `BoardUpdatedEvent.getRouterCounters`.
// not ported: `BoardUpdatedEventListener` — a one-method listener interface.
// not ported: `TaskStateChangedEvent.getTaskState`, `TaskStateChangedEvent.getPassNumber`, `TaskStateChangedEvent.getBoardHash`.
// not ported: `TaskStateChangedEventListener` — a one-method listener interface.

// --- `board/optimize/ViaOptimizer`: Plan 7's, and the last of that directory ---------------------
//
// The other five files of `board/optimize` are this crate's (`TraceShover` and the four
// `TraceTightener*`, controller ruling AB), and the `board/optimize` audit invocation names them.
// `ViaOptimizer` is deliberately outside that glob: its one public method is called from
// `TraceTightener.optChangedArea:160-165`, which Plan 7 Task 5 landed as
// `board_ext::TraceTightener::opt_changed_area`. **The class is ported whole**, in
// `board_ext::via_optimizer`, and the audit-map row points there: Plan 7 Task 6 landed the entry
// half (`optViaLocation:33-158`, `optPlaneOrFanoutVia:161-296`, `isWithinTolerance:719-732`) and
// Plan 7 Task 7 the three `repositionVia` overloads (`:302-365` as
// `reposition_via_toward_location`, `:367-429` as `reposition_via_check_candidate`, `:434-713` as
// `reposition_via_general` — Java overloads on the argument list and Rust does not). `p7t3` mode 4
// and `p7t4` modes 0-6 are the measurement: 0 diffs.
