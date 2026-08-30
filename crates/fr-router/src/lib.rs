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
//! **Task 17 of 18.** What exists so far is the data-model floor: [`Arena`] and its index
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
//! the inserted geometry and the item ids each connection burned**. The roster at the foot of
//! this file names each still-deferred class and the plan that owns it.
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
//! * **No GUI, no `FRLogger`, no observers**, and no static mutable state.
//! * `ExpansionCostFactor` is **re-exported** from `fr-settings`, never redeclared (ruling 8 —
//!   the plan-4 obligation).

pub mod arena;
pub mod autoroute;
pub mod board_ext;
pub mod error;
pub mod java_tree_set;

pub use arena::{Arena, DoorId, DrillId, IncompleteRoomId, PageId, TargetDoorId};
pub use autoroute::{
    AutorouteAttemptResult, AutorouteAttemptState, AutorouteControl, AutorouteEngine,
    AutorouteSearchTreeExt, CompleteFreeSpaceExpansionRoom, Connection, DestinationDistance,
    DrillPage, DrillPageArray, ExpandableRef, ExpansionDoor, ExpansionDrill, ExpansionRoomStore,
    FoundConnectionInserter, FoundConnectionLocator, FreeSpaceExpansionRoom,
    IncompleteFreeSpaceExpansionRoom, MazeAdjustment, MazeExpansionEngine, MazeListElement,
    MazeQueue, MazeResult, MazeRipupResolver, MazeSearchElement, MazeSearchEngine,
    ObstacleExpansionRoom, ResultItem, RoomRef, ShoveResult, TargetItemExpansionDoor, ViaMask,
    route_connection,
};
pub use board_ext::{
    CheckDrillResult, DrillItemMover, ForcedPadRouter, ForcedViaInserter, RoutingBoardExt,
    SpringOverOutcome, TraceShover,
};
pub use error::RouterError;
pub use java_tree_set::JavaTreeSet;

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
        AutorouteSearchTreeExt, CompleteFreeSpaceExpansionRoom, Connection, DestinationDistance,
        DoorId, DrillId, DrillItemMover, DrillPage, DrillPageArray, ExpandableRef,
        ExpansionCostFactor, ExpansionDoor, ExpansionDrill, ExpansionRoomStore,
        FoundConnectionInserter, FoundConnectionLocator, FreeSpaceExpansionRoom,
        IncompleteFreeSpaceExpansionRoom, IncompleteRoomId, JavaTreeSet, MazeAdjustment,
        MazeExpansionEngine, MazeListElement, MazeQueue, MazeResult, MazeRipupResolver,
        MazeSearchElement, MazeSearchEngine, ObstacleExpansionRoom, PageId, ResultItem, RoomRef,
        RouterError, RoutingBoardExt, ShoveResult, SpringOverOutcome, TargetDoorId,
        TargetItemExpansionDoor, TraceShover, ViaMask, route_connection,
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

// --- Out of scope for Plan 6 and Plan 7 alike (ruling 13's not-ported roster) ---------------------
//
// `autoroute/BoardHistory.java` + `autoroute/BoardHistoryEntry.java`: the undo/history store, which
// spec §2 puts out of scope for the whole port. It keeps a ranked list of serialised board
// snapshots so the batch loop can restore the best one; nothing in the maze search reads it.
// not ported: `BoardHistory.add`
// not ported: `BoardHistory.BoardHistoryEntry`
// not ported: `BoardHistory.clear`
// not ported: `BoardHistory.contains`
// not ported: `BoardHistory.getMaxScore`
// not ported: `BoardHistory.getRank`
// not ported: `BoardHistory.remove`
// not ported: `BoardHistory.restoreBestBoard`
// not ported: `BoardHistory.restoreBoard`
// not ported: `BoardHistory.size`
// not ported: `BoardHistoryEntry.compareTo`
//
// `autoroute/AutorouteDiagnostic.java`: a GUI overlay sink (it declares no public methods of its
// own — the marker records the class). `autoroute/PerformanceProfiler.java`: a timing sink whose
// only output is `FRLogger`.
// not ported: `AutorouteDiagnostic` — a GUI overlay sink (`global-constraints.md`: no GUI).
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
