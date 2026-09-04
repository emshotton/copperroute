//! [`Board`]: the board itself — the item list, the search trees, the rules, the library and the
//! protocol every later plan mutates them through.
//!
//! # Which Java class is which
//!
//! Java splits the board across a base class, a subclass and five collaborator façades that the
//! base class creates lazily and delegates to:
//!
//! ```text
//! BasicBoard ── BoardItemRepository       (item list, insert/remove)
//!   │        ── BoardConnectivityQueries  (net/component queries)
//!   │        ── BoardSnapshotManager      (serialize, hash, undo stack)
//!   └ RoutingBoard ── RoutingBoardOperations   (changed area, remove + pull tight)
//!                  ── RoutingBoardSearchFacade (checkTraceSegment, checkMoveItem, picking)
//!                  ── RoutingBoardUndoFacade   (undo/redo/deepCopy)
//! ```
//!
//! The façades hold nothing but a back-pointer to the board (`BoardItemRepository.java:27`,
//! `BoardConnectivityQueries.java:16`, `RoutingBoardOperations.java:20`,
//! `RoutingBoardSearchFacade.java:23`) and every one of their methods is package-private, so they
//! are not types here: their bodies are inherent methods of [`Board`], split across this module's
//! files by subject rather than by Java class. Each body names the Java file and lines it came
//! from.
//!
//! `RoutingBoard` is likewise not a separate type. Plan 2 ruling 4 splits it: its non-shove state
//! ([`Board::changed_area`], `check_trace_segment`, the failure-log hook, the shove-failure
//! fields) lands here, and its shove/forced-via/pull-tight entry points arrive in Plan 7 as an
//! extension trait in `fr-router`.
//!
//! # The board back-pointer
//!
//! `global-constraints.md` forbids `Item.board`. Every Java body that read it becomes either a
//! [`Board`] method taking an [`ItemId`] (the connectivity family, `connectivity.rs`) or a method
//! that takes the piece of the board it actually needed — [`ItemCtx`], which this module builds
//! from four of its own fields.
//!
//! # Item iteration order (quirk #63)
//!
//! `board.itemList` is an `UndoableObjects`, backed by a `ConcurrentSkipListMap` keyed by
//! `Item.compareTo`, whose subtraction is reversed (Item.java:98). Every Java walk of the item
//! list therefore runs in **descending item id**. [`Board::items`] is a `BTreeMap`, which
//! iterates *ascending*, so every port of such a walk here uses [`Board::items_in_board_order`]
//! (or `self.items.values().rev()`). This is load-bearing: it decides the structure of every
//! tree `SearchTreeManager::get_autoroute_tree` builds.

// The rest of Java's `board/state` package, whose two ported members are `ChangedArea` and
// `Communication`:
//
// not ported: `BoardObserverAdaptor.activate` (BoardObserverAdaptor.java) — `global-constraints.md` forbids board observers.
// not ported: `BoardObserverAdaptor.deactivate` — board observers.
// not ported: `BoardObserverAdaptor.isActive` — board observers.
// not ported: `BoardObserverAdaptor.notifyNew` — board observers.
// not ported: `BoardObserverAdaptor.notifyDeleted` — board observers.
// not ported: `BoardObserverAdaptor.notifyChanged` — board observers.
// not ported: `BoardObserverAdaptor.notifyMoved` — board observers.
// added in Plan 3: `board/state/CoordinateTransform.java`'s `boardToUser` and `userToBoard` — the board-to-user unit transform the DSN reader and the SES writer need.
// not ported: `board/state/BoardComparator.compare` and its nested `ComparisonResult` (BoardComparator.java, 758 loc) — **controller ruling AS, closed by Plan 8 Task 14.** This marker was a deferral through Plans 5-7 on the theory that the result-manifest/report layer (spec §10) would want it. Plan 8 built that whole layer and does not: Task 4's `RoutingResultManifest` (`crates/fr-core/src/manifest.rs`) carries **one** board's statistics and never a diff of two — `grep -n BoardComparator` over `core/results/RoutingResultManifest.java` is empty — and Task 12's `info`/`board_info` report the same single-board `BoardSummary`. Reachability in the jar: `grep -rn BoardComparator src/main/java` names only `Freerouting.compareBoardFiles`/`loadBoardFromFile` (`Freerouting.java`, the `--compare-boards=` developer flag) and `management/HeadlessBoardManager.compareCounterpartBoardIfPresent` (`:172-237`, quirk AE's racing diagnostic thread) — both rostered, neither on any `-de`/`-do`/`-drc` path. `crates/fr-drc/src/lib.rs` carries the twin line at the crate plan-5 ruling 13 was made in.

pub mod changed_area;
pub mod clearance;
pub mod clearance_override;
pub mod communication;
pub mod connectivity;
pub mod normalize;
pub mod query;
pub mod shape_trace_entries;
pub mod snapshot;
pub mod trace_normalize;

use std::borrow::Cow;
use std::collections::BTreeMap;

use fr_geometry::{Area, IntBox, Point, Polyline, PolylineShapeRef, TileShape, Vector};

pub use changed_area::ChangedArea;
pub use clearance_override::{
    BOARD_EDGE_CLEARANCE_CLASS_NAME, DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM,
    HOLE_EDGE_CLEARANCE_CLASS_NAME,
};
pub use communication::{Communication, WriteResolution};
pub use connectivity::StopConnectionOption;
pub use normalize::MAX_NORMALIZE_ITERATIONS;
pub use shape_trace_entries::ShapeTraceEntries;
pub use trace_normalize::MAX_NORMALIZATION_DEPTH;

use crate::ids::{ItemId, TreeId};
use crate::items::{
    ComponentObstacleArea, ComponentOutline, ConductionArea, Item, ItemCtx, ItemHeader,
    ObstacleArea, ObstacleAreaData, Pin, PolylineTrace, Via, ViaObstacleArea,
};
use crate::library::BoardLibrary;
use crate::rules::BoardRules;
use crate::searchtree::SearchTreeManager;
use crate::structure::{BoardOutline, Components, FixedState, LayerStructure};

/// Builds an [`ItemCtx`] from a board's own fields.
///
/// A macro rather than a `fn ctx(&self)` on purpose: a method borrows the *whole* board, so it
/// could not be combined with `self.items.get_mut(...)` or `&mut self.trees` in the same
/// expression. Expanded inline, the four field borrows are disjoint from `items` and `trees` and
/// the borrow checker accepts them.
macro_rules! item_ctx {
    ($board:expr) => {
        $crate::items::ItemCtx {
            library: &$board.library,
            components: &$board.components,
            rules: &$board.rules,
            bounding_box: &$board.bounding_box,
            max_tree_shape_width: $board.max_tree_shape_width,
        }
    };
}
pub(crate) use item_ctx;

/// Port of `BasicBoard` (`board/facade/BasicBoard.java`) plus the non-shove half of
/// `RoutingBoard` (`board/facade/RoutingBoard.java`): a board with geometric items, the search
/// trees that index them and the rules they must satisfy.
///
/// not ported: `BasicBoard.itemList`'s `UndoableObjects` undo **stack** (BasicBoard.java:70) —
/// the port stores a plain `BTreeMap` (plan-rulings.md #1); Task 12 replaced Java's
/// snapshot/undo/redo with [`Board::clone`]/[`Board::deep_copy`] (`board/snapshot.rs`), and
/// Plan 7 Task 14c gave the **one** level `BatchOptimizer.optRouteItem` opens back to Java
/// ([`Board::begin_undo_journal`]/[`Board::undo_from_snapshot`], over
/// [`crate::board::snapshot::UndoJournal`]). See the markers below.
///
/// `BasicBoard.updateBox` (BasicBoard.java:103) is the rectangle a Swing renderer has to
/// repaint; `fr-board` is headless, so its three accessors go:
///
/// not ported: `BasicBoard.resetGraphicsUpdateBox` (BasicBoard.java:1142-1144) — GUI repaint region.
/// not ported: `BasicBoard.getGraphicsUpdateBox` (BasicBoard.java:1147-1149) — GUI repaint region.
/// not ported: `BasicBoard.joinGraphicsUpdateBox` (BasicBoard.java:1152-1157) — GUI repaint region.
///
/// `global-constraints.md` forbids board observers, and `Communication.observers` is not ported
/// either, so:
///
/// not ported: `BasicBoard.startNotifyObservers` (BasicBoard.java:1160-1164) — no observers.
/// not ported: `BasicBoard.endNotifyObservers` (BasicBoard.java:1167-1171) — no observers.
/// not ported: `BasicBoard.observersActive` (BasicBoard.java:1174-1182) — no observers.
///
/// not ported: `BasicBoard.serialize` (:153-155), `deserialize` (:139-141) and the private
/// `readObject` (:1388-1400) — Java serialization.
///
/// not ported: `BasicBoard.DominantSide` (:1482-1486), a renderer-only enum with no member that
/// this crate reads.
///
/// not ported: `RoutingBoard.getStatistics` (RoutingBoard.java:1410-1412) — it constructs a
/// `core.scoring.BoardStatistics`, which is outside this crate.
///
/// not ported: `RoutingBoard.isMaintainingAutorouteDatabase` (:1384-1386) and
/// `setMaintainingAutorouteDatabase` (:1392-1398) — both are package-private and read only
/// `autorouteEngine`, which is Plan 6's.
///
/// not ported: `app.freerouting.board.actions.ItemSelectionFilter`, the parameter of
/// `BasicBoard.pickItems` (:1086) — interactive-GUI selection state, as
/// `Item.isSelectedByFilter` already records. [`Board::pick_items`] returns everything and its
/// callers filter by item kind.
///
// Task 12 replaces Java's `UndoableObjects` snapshot stack (the whole of
// `board/facade/BoardSnapshotManager.java` and `RoutingBoardUndoFacade.java`) with
// `Board::clone`/`Board::deep_copy` (`board/snapshot.rs` has the full account, including why the
// search trees clone rather than rebuild):
// renamed: `BasicBoard.clone` (BasicBoard.java:158-161) -> `Board::deep_copy` minus the
// `clearAllItemTemporaryAutorouteData`/`finishAutoroute` tail (`board/snapshot.rs` module doc,
// "What Java's `clone`/`deepCopy` actually do") — both Java methods round-trip through the
// same `serialize`/`deserialize`, so both reset every transient field (`revision`,
// `normalizeSuppressedNetNos`, the search tree, `changedArea`, `shoveFailingObstacle`,
// `shoveFailingLayer`); `deepCopy` additionally clears autoroute scratch and calls
// `finishAutoroute`. The derived `impl Clone for Board` is not this method — it is Task 12's
// substitute for the *state* half of `generateSnapshot`/`undo` below, which clears nothing.
// renamed: `BasicBoard.getHash` (BasicBoard.java:164-166) -> `Board::structural_hash`.
// ported: `BasicBoard.diffTraces` (BasicBoard.java:168-171) -> `Board::diff_traces`.
// ported: `RoutingBoard.deepCopy` (RoutingBoard.java:1414-1420) -> `Board::deep_copy`.
// **Plan 7 Task 14c (ruling BA) narrowed that substitution rather than replacing it.** The item
// state still comes from a `Board::deep_copy`, but the *side effects* of `undo` do not: Java
// replays the failed attempt's item changes through the **live** search trees, which re-pairs the
// same leaves into a different `MinAreaTree` topology, and
// `ShapeSearchTree45Degree.completeShape` reads that topology directly (quirk #229). So the three
// entry points below are ported at the one level `BatchOptimizer.optRouteItem` opens, over
// `crate::board::snapshot::UndoJournal`:
// renamed: `BasicBoard.generateSnapshot` (:1289-1292) -> `Board::begin_undo_journal`, which the
// caller pairs with its own `Board::deep_copy` (plan-7 ruling 8's substitute for the state half).
// renamed: `BasicBoard.popSnapshot` (:1297-1300) -> `Board::discard_undo_journal`.
// renamed: `BasicBoard.undo` (:1233-1240) -> `Board::undo_from_snapshot`, which takes the
// pre-attempt board by value where Java reads the previous level off the `UndoableObjects` stack.
// renamed: the private `BasicBoard.applyUndoRedoSideEffects` (:1255-1287) ->
// `Board::undo_from_snapshot`'s second half.
// not ported: `BasicBoard.redo` (:1246-1253) and `RoutingBoard`'s override — interactive redo,
// which `global-constraints.md` excludes along with the rest of the GUI, and which no headless
// caller reaches (`BatchOptimizer.java:509` is the only `undo` on that path, and nothing follows
// it).
//
// The autoroute engine is Plan 6:
// not ported: `BasicBoard.additionalUpdateAfterChange` (BasicBoard.java:1222-1226) — an empty stub whose whole body is the `RoutingBoard` override below.
// renamed: `RoutingBoard.additionalUpdateAfterChange` (RoutingBoard.java:96-118) -> `fr_router::board_ext::RoutingBoardExt::additional_update_after_change` — it takes an `AutorouteEngine`, which `fr-board` cannot name (plan-2 ruling 4, plan-6 ruling 3).
// not ported: `BasicBoard.areThereItemsOnInactiveLayer` (BasicBoard.java:1443-1462) — it takes an `AutorouteControl`, returns `void`, and its whole body is one `FRLogger.warn("There is an item on an inactive layer.")` plus a local `hasSomethingOnInactiveLayer` that is assigned and never read. **No caller anywhere in the Java tree** (`grep -rn areThereItemsOnInactiveLayer src/main src/test` finds only the declaration), so porting it would add a headless log line to nothing. Plan 6 Task 18 re-pointed this marker (it had been a Plan 6 deferral) after reading the body.
// renamed: `RoutingBoard.initAutoroute` (RoutingBoard.java:882-897) -> `fr_router::board_ext::RoutingBoardExt::init_autoroute`; the engine is passed in and handed back where Java reads and writes its `autorouteEngine` field.
// ported: `RoutingBoard.finishAutoroute` (RoutingBoard.java:899-905) -> `Board::finish_autoroute`
// (`board/snapshot.rs`), empty until Plan 6 gives `Board` the `autoroute_engine` field it clears.
// renamed: `RoutingBoard.autoroute` (RoutingBoard.java:911-971) -> `fr_router::route_connection_full` (`autoroute/maze/engine.rs`), reached through `fr_router::pipeline::BatchAutorouter::autoroute_item`; the method's body *is* that call — it builds an `AutorouteControl`, swaps start and destination for a plane net, sets the `TimeLimit`, calls `initAutoroute` and then `autorouteConnection` — and HEAD's own pipeline reaches it as `AutorouteConnectionRouter.route`, which is where Plan 7 Tasks 8 and 9 ported it. Landed in Plan 7 Task 9.
// renamed: `RoutingBoard.fanout` (RoutingBoard.java:978-1110) -> `fr_router::board_ext::RoutingBoardExt::fanout` — the SMD escape router `BatchFanout` calls once per pin, which plan-6 ruling 2 puts above the seam with the rest of the batch loop. It needs an `AutorouteEngine` (`initAutoroute` at `:1059`, `autorouteConnection` at `:1069`) and a `TraceTightener` (`optChangedArea` at `:1101`), neither of which `fr-board` can name. Landed in Plan 7 Task 11.
// renamed: `RoutingBoard.optChangedArea` (both overloads, RoutingBoard.java:151-190) -> `fr_router::board_ext::RoutingBoardExt::{opt_changed_area, opt_changed_area_with_keep_point}` (Rust has no overloading); its body is `RoutingBoardOperations.optChangedArea` (:52-79), which builds a `TraceTightener` — `fr-router`'s type — and runs its `optChangedArea` sweep. Landed in Plan 7 Task 5.
// renamed: `RoutingBoard.removeItemsAndPullTight` (RoutingBoard.java:124-127 -> RoutingBoardOperations.java:81-120) -> `fr_router::board_ext::RoutingBoardExt::remove_items_and_pull_tight`; the removal half stays here as `Board::remove_items_marking_changed_area`, and the `combineTraces` + `optChangedArea` tail needs a `TraceTightener` — `fr-router`'s type — so the whole method is presented there. Landed in Plan 7 Task 8.
// not ported: `RoutingBoard.moveDrillItem` (RoutingBoard.java:252-295) — **GUI only**, and the plan's scan ruling 3 was wrong about who calls it.
// Ruling 3 folded this method into Plan 7 Task 6 because "both `optViaLocation` and
// `optPlaneOrFanoutVia` move vias through it". They do not: both call
// `DrillItemMover.insert(via, delta, 9, 9, null, board)` and `DrillItemMover.check(...)`
// **directly** (ViaOptimizer.java:136, :244, :282), and Plan 6 Task 10b already landed both as
// `fr_router::board_ext::DrillItemMover::{insert, check}`. A fresh grep of the whole Java tree at
// port time — `grep -rn moveDrillItem src/main/java src/test` — finds exactly two hits: the
// declaration here, and `board/actions/MoveComponent.java:156` inside `MoveComponent.insert`
// (:143-176), whose own only caller is `gui/interactive/DragItemState.java:56-61` — a mouse
// drag. Nothing on the headless path reaches it, so it is rostered under Plan 7's "No GUI"
// constraint rather than ported. Its body is `clearShoveFailingObstacle` + un-fixing the
// SHOVE_FIXED contacts + a `tidyRegion` and `DrillItemMover.insert` + `optChangedArea`; every
// piece of that already exists in the port, so a later caller (if the GUI is ever ported) can
// assemble it without new machinery. **Not a quirk row** — this is a "no headless caller" ruling,
// not a Java bug; the reasoning is `.superpowers/sdd/2026-08-30-plan-7-router-batch/task-6-report.md`
// §2 and `crates/fr-router/README.md`'s Task 6 section.
// not ported: `RoutingBoard.forcedVia` (RoutingBoard.java:312-352) — **GUI only**, the same
// answer `moveDrillItem` got two markers above, and measured the same way. Plan 7 Task 17 re-ran
// the grep the plan's owner table asked for: `grep -rn forcedVia src/main/java src/test` at the
// clone's HEAD finds exactly **two** hits — the declaration here, and
// `gui/interactive/Route.java:294` inside the interactive route state. Nothing on the headless
// path reaches it. The plan's table offered `// renamed:` -> Plan 6's `ForcedViaInserter` "if it
// is the whole body": it is not. The body is `clearShoveFailingObstacle` + `startMarkingChangedArea`
// + `ForcedViaInserter.insert` + a `tidyWidth`-clipped `optChangedArea` (`:334-350`), i.e. the
// GUI's own wrapper around three methods the port already has
// (`Board::clear_shove_failing_obstacle`, `Board::start_marking_changed_area`,
// `fr_router::board_ext::ForcedViaInserter::insert`,
// `fr_router::board_ext::RoutingBoardExt::opt_changed_area`). A `renamed:` would claim the
// wrapper landed, which would be false; a Plan 8 deferral marker would claim Plan 8 owes it,
// which is also false — Plan 8 is headless too. **Not a quirk row**: a "no headless caller" ruling, not a
// Java bug.
// renamed: `RoutingBoard.checkForcedTracePolyline` (RoutingBoard.java:405-448) -> `fr_router::board_ext::RoutingBoardExt::check_forced_trace_polyline`; it drives `TraceShover.check`, which lives in `fr-router` because the router is its only caller.
// renamed: `RoutingBoard.insertForcedTraceSegment` (RoutingBoard.java:361-402) and `insertForcedTracePolyline` (:456-876) -> `fr_router::board_ext::RoutingBoardExt::{insert_forced_trace_segment, insert_forced_trace_polyline}` — the mutating half of the `TraceShover`, which lives in `fr-router` for the same reason `checkForcedTracePolyline` does, plus an `AutorouteEngine` for the `PolylineTrace.change` in its pull-tight tail. **Controller ruling AB** moved them out of Plan 7 into Plan 6 Task 15b, because `FoundConnectionInserter:176` and its five `tryNeckDown` / `insertFanoutMicroNeckdown` call sites are on the autoroute path.
#[derive(Debug, Clone, PartialEq)]
pub struct Board {
    /// Java `BasicBoard.itemList` (BasicBoard.java:70), as a map keyed by the Java item id
    /// (plan-rulings.md #1). **Iterates ascending**; see the module docs on quirk #63.
    pub items: BTreeMap<ItemId, Item>,
    /// Java `BasicBoard.components` (BasicBoard.java:73).
    pub components: Components,
    /// Java `BasicBoard.rules` (BasicBoard.java:79). It also carries the layer structure — see
    /// [`Board::layer_structure`].
    pub rules: BoardRules,
    /// Java `BasicBoard.library` (BasicBoard.java:82).
    pub library: BoardLibrary,
    /// Java `BasicBoard.communication` (BasicBoard.java:88).
    pub communication: Communication,
    /// Java `BasicBoard.boundingBox` (BasicBoard.java:91).
    pub bounding_box: IntBox,
    /// Java `BasicBoard.searchTreeManager` (BasicBoard.java:94).
    // renamed: BasicBoard.searchTreeManager -> Board::trees (task brief naming).
    pub trees: SearchTreeManager,
    /// Java `RoutingBoard.changedArea` (RoutingBoard.java:67); `null` becomes `None`.
    pub changed_area: Option<ChangedArea>,
    // renamed: `RoutingBoard.failureLog` (RoutingBoard.java:64, constructed at `:91`) -> a caller-owned `fr_router::pipeline::RoutingFailureLog`, threaded as a parameter of `AutoroutePassRunner::run_single_thread` — `fr-board` must not depend on `fr-router`, and that is where the type lives. Landed in Plan 7 Task 9, which **deleted** the `Vec<String>` hook that stood here (`docs/plan-6-handoff.md` §10.4): nothing ever read or wrote it, and keeping a second, differently-typed log beside the real one would be a place for the two to disagree. The divergence is unobservable — Java's field is `final`, so no path can swap one board's log for another's, and its only reader is a log-message guard at `AutoroutePassRunner.java:272-273`.
    /// Java `RoutingBoard.shoveFailingObstacle` (RoutingBoard.java:72), as an id.
    pub shove_failing_obstacle: Option<ItemId>,
    /// Java `RoutingBoard.shoveFailingLayer` (RoutingBoard.java:73), initialised to `-1`.
    pub shove_failing_layer: i32,

    /// Java `BasicBoard.normalizeSuppressedNetNos` (BasicBoard.java:96): the nets whose
    /// normalisation hit [`MAX_NORMALIZE_ITERATIONS`] on this board, and which
    /// [`Board::normalize_traces`] refuses to touch again.
    ///
    /// Java's field is `transient` and its only reset is in `readObject`
    /// (BasicBoard.java:1392), i.e. a board that comes back through
    /// `BoardSnapshotManager.deserialize` — which is what Java's `clone`/`deepCopy` is — starts
    /// with an empty set. That is what its own log message means by "on this board candidate".
    ///
    /// **`pub` where Java's field is `private`**, and deliberately so: Java's only reset is
    /// inside a `readObject` this port does not have, so [`Board::deep_copy`] (`board/snapshot.rs`)
    /// has to clear it explicitly, and a test has no other way to drive
    /// [`Board::normalize_traces`]' suppressed-net branch (BasicBoard.java:713-727) — the
    /// oscillation cap that fills the set is unreachable on any board the suite can build.
    pub normalize_suppressed_net_nos: std::collections::BTreeSet<i32>,

    /// Java `BasicBoard.revision` (BasicBoard.java:97). `u64` rather than `int`: the counter only
    /// ever increases and nothing compares it against a negative value.
    revision: u64,
    /// Java `BasicBoard.maxTraceHalfWidth` (BasicBoard.java:106), initialised to 1000.
    max_trace_half_width: i32,
    /// Java `BasicBoard.minTraceHalfWidth` (BasicBoard.java:109), initialised to 10000.
    min_trace_half_width: i32,
    /// The section width `ShapeSearchTree.calculateTreeShapes(ObstacleArea)` uses
    /// (ShapeSearchTree.java:916-920), resolved once from [`Self::communication`]; see
    /// [`ItemCtx::max_tree_shape_width`].
    max_tree_shape_width: f64,

    /// The port's stand-in for the *one* undo level `BatchOptimizer.optRouteItem` opens: the top
    /// entry of `UndoableObjects.deletedObjectsStack` (UndoableObjects.java:27) plus the
    /// per-node `level`/`undoObject` bookkeeping (`UndoableObjectNode`, :328-340) that decides
    /// which items `undo` cancels and which it restores. `None` means `stackLevel == 0`, where
    /// Java's writers are no-ops by construction (`board/snapshot.rs`' module doc, "The undo
    /// bookkeeping"). See [`Board::begin_undo_journal`] and [`Board::undo_from_snapshot`].
    undo_journal: Option<crate::board::snapshot::UndoJournal>,
}

impl Board {
    // -- construction ---------------------------------------------------------------------------

    /// Port of the `BasicBoard(IntBox, LayerStructure, PolylineShape[], int, BoardRules,
    /// Communication)` constructor (BasicBoard.java:119-136) and the `RoutingBoard` constructor
    /// that chains to it (RoutingBoard.java:83-92).
    ///
    /// Java allocates an empty `BoardLibrary` and `Components` here and lets the DSN reader fill
    /// them afterwards through the public fields; this port takes both, because a programmatic
    /// board's pins and vias have to resolve their padstacks the moment they are inserted.
    ///
    /// Java's `layerStructure` parameter is dropped: `BasicBoard`'s constructor stores the *same*
    /// object in `this.layerStructure` and hands it to `BoardRules`, and this port's
    /// [`BoardRules`] already owns a copy — see [`Board::layer_structure`].
    ///
    /// Java's `rules.nets.setBoard(this)` (BasicBoard.java:134) is not ported; the five `Net`
    /// methods that used the back-pointer are [`Board::net_terminal_items`], [`Board::net_pins`],
    /// [`Board::net_items`], [`Board::net_trace_length`] and [`Board::net_via_count`].
    ///
    /// The last thing the constructor does is insert the `BoardOutline` item
    /// (BasicBoard.java:135), so a fresh board is never empty: the outline takes id 1.
    ///
    /// # Via padstacks (quirks #42-43)
    ///
    /// `BoardLibrary.viaPadstacks` is `null` until `setViaPadstacks`/`addViaPadstack` is called,
    /// and `removeViaPadstack`/`getMirroredViaPadstack` then throw. Nothing in `Board`'s own
    /// insert path consults that list — [`Board::insert_via`] takes a
    /// [`PadstackId`](crate::ids::PadstackId) directly — but a caller that will use those two
    /// must populate the library before handing it over.
    pub fn new(
        outline_shapes: Vec<PolylineShapeRef>,
        outline_clearance_class: usize,
        bounding_box: IntBox,
        rules: BoardRules,
        library: BoardLibrary,
        components: Components,
        communication: Communication,
    ) -> Board {
        // ShapeSearchTree.java:916-920: `50000`, lowered — `Math.min`, so never raised — to
        // `500 * communication.getResolution(MIL)` when the board names a host CAD system.
        let mut max_tree_shape_width = crate::items::DEFAULT_MAX_TREE_SHAPE_WIDTH;
        if communication.host_cad_exists() {
            max_tree_shape_width = max_tree_shape_width
                .min(500.0 * communication.get_resolution(crate::structure::Unit::Mil));
        }
        let mut board = Board {
            items: BTreeMap::new(),
            components,
            rules,
            library,
            communication,
            bounding_box,
            trees: SearchTreeManager::new(),
            changed_area: None,
            shove_failing_obstacle: None,
            shove_failing_layer: -1,
            normalize_suppressed_net_nos: std::collections::BTreeSet::new(),
            revision: 0,
            max_trace_half_width: 1000,
            min_trace_half_width: 10000,
            max_tree_shape_width,
            undo_journal: None,
        };
        board.insert_outline(outline_shapes, outline_clearance_class);
        board
    }

    /// Java `BasicBoard.layerStructure` (BasicBoard.java:85).
    ///
    /// Not a field here: `BasicBoard`'s constructor stores one `LayerStructure` object in both
    /// `board.layerStructure` and `board.rules.layerStructure` (BasicBoard.java:126-127 plus
    /// `BoardRules`' own constructor), so the two are always the same stack.
    pub fn layer_structure(&self) -> &LayerStructure {
        self.rules.layer_structure()
    }

    /// Port of `BasicBoard.getLayerCount` (BasicBoard.java:1077-1079).
    pub fn get_layer_count(&self) -> usize {
        self.layer_structure().count()
    }

    /// The [`ItemCtx`] this board's items are evaluated in — Java's `Item.board`, reduced to the
    /// five things a ported item body reads from it.
    ///
    /// Not a Java method. Mutating methods build the same value inline through the private
    /// `item_ctx!` macro, because a `&self` method would borrow the whole board.
    pub fn ctx(&self) -> ItemCtx<'_> {
        item_ctx!(self)
    }

    /// The default search tree's id, for the [`Item`] methods that take one.
    ///
    /// Not a Java method: Java writes `board.searchTreeManager.getDefaultTree()` and passes the
    /// tree object.
    pub fn default_tree_id(&self) -> TreeId {
        self.trees.get_default_tree().id()
    }

    // -- revision -------------------------------------------------------------------------------

    /// Port of `BasicBoard.getRevision` (BasicBoard.java:143-145).
    // renamed: BasicBoard.getRevision -> Board::revision (the crate drops `get_` prefixes that
    // carry no information).
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Port of `BasicBoard.incrementRevision` (BasicBoard.java:148-150).
    pub fn increment_revision(&mut self) {
        self.revision += 1;
    }

    // -- the insert/remove protocol (BoardItemRepository) ----------------------------------------

    /// A fresh item id, Java's `board.communication.idGenerator.newId()` — the `id <= 0` branch
    /// of the `Item` constructor (Item.java:86-90), which every typed inserter below runs.
    ///
    /// Not a Java method: Java allocates the id inside `Item`'s constructor, which needs the
    /// board; this port's item constructors take the resolved id.
    //
    // Plan 7 Task 8b: `#[track_caller]` carries the *caller's* `file:line` into the level-7 `ID`
    // ledger below — the port's answer to `P6T17bProbe`'s `StackWalker` label on the Java side.
    // The attribute changes no behaviour; it only makes `Location::caller()` name the call site
    // instead of this line.
    #[track_caller]
    pub fn new_item_id(&mut self) -> ItemId {
        let id = self.communication.id_gen.new_id();
        // Plan 7 Task 8b's level-7 ledger — `ID`. Off unless `P7T8B_IDS` is set; stderr only.
        // Quirk #210.
        if p7t8b_ids_ledger() && id.0 >= p7t8b_ids_from() {
            let caller = std::panic::Location::caller();
            eprintln!("ID {} {}:{}", id.0, caller.file(), caller.line());
            if p7t8b_ids_backtrace() {
                eprintln!(
                    "IDBT {}\n{}",
                    id.0,
                    std::backtrace::Backtrace::force_capture()
                );
            }
        }
        id
    }

    /// Port of `BoardItemRepository.insertItem` (BoardItemRepository.java:135-167), which
    /// `BasicBoard.insertItem` (BasicBoard.java:1219-1221) delegates to.
    ///
    /// The order is Java's and matters: clamp the clearance class, put the item in the item list,
    /// insert it into every search tree (which is what sets `onTheBoard`), then bump the
    /// revision.
    ///
    /// Java's `item == null` guard (BoardItemRepository.java:137-139) is the caller's `Option`
    /// here. Its `FRLogger.trace` debug window (:139-150,228-232) and the `FRLogger.warn` on the
    /// out-of-range clearance class (:156) are dropped (`global-constraints.md`).
    pub fn insert_item(&mut self, mut item: Item) -> ItemId {
        // BoardItemRepository.java:152-158. `clearanceClassIndex() < 0` cannot arise for a
        // `usize`, and `rules`/`clearanceMatrix` are never null in this port.
        if item.clearance_class() >= self.rules.clearance_matrix.get_class_count() {
            item.set_clearance_class(0, &self.rules);
        }
        let id = item.id();
        debug_assert!(
            id.0 > 0,
            "Board::insert_item: item ids come from Board::new_item_id and start at 1 \
             (ItemIdGenerator.java:37-55)"
        );
        // BoardItemRepository.java:160.
        self.items.insert(id, item);
        // `itemList.insert`'s undo half (UndoableObjects.java:66-70) — see
        // `Board::journal_insert`; a no-op outside `BatchOptimizer.optRouteItem`'s window.
        self.journal_insert(id);
        // BoardItemRepository.java:161.
        let ctx = item_ctx!(self);
        let inserted = self
            .items
            .get_mut(&id)
            .expect("Board::insert_item: just inserted");
        self.trees.insert(inserted, &ctx);
        // not reachable: RoutingBoard.additionalUpdateAfterChange (retainAutorouteDatabase is a Java benchmark-only system property)
        //
        // The Java call is `BoardItemRepository.java:165 -> RoutingBoard.java:96-118`, which
        // invalidates the autoroute expansion rooms an inserted item overlaps. It needs an
        // `AutorouteEngine`, which `fr-board` cannot name (plan-2 ruling 4/11), so it could only
        // be made at the call site — and this one has thirteen call sites, all *inside*
        // `fr-board`, so the engine would have to be threaded through every typed inserter that
        // reaches it.
        //
        // **Plan 7 Task 8 settled that it never has to be** (controller ruling AJ).
        // `RoutingBoard.additionalUpdateAfterChange` returns at `:100-102` unless
        // `autorouteEngine.maintainDatabase`, and `maintainDatabase` is the constructor argument
        // `RoutingBoard.initAutoroute:892` passes, i.e. `BatchAutorouter.retainAutorouteDatabase`
        // (`BatchAutorouter.java:63-64,151-154`) — the benchmark-only system property
        // `freerouting.benchmark.retain_autoroute_database`, `false` on every production and
        // parity path, and hard-coded `false` at `BatchAutorouterThread.java:90`. The port has no
        // setter for it at all: `fr_router::route_connection_full` passes `false` unconditionally,
        // pinned by `crates/fr-router/tests/batch_autorouter.rs`'s
        // `retain_autoroute_database_is_false_on_every_path` and its `…_has_no_setter` sibling.
        // So the body this marker stands for is dead code in the port, not deferred work.
        // BoardItemRepository.java:166.
        self.revision += 1;
        id
    }

    /// Port of `BoardItemRepository.removeItem` (BoardItemRepository.java:170-199), which
    /// `BasicBoard.removeItem` (BasicBoard.java:588-590) delegates to.
    ///
    /// Java's `item == null` guard and its `isDeletionForbidden` refusal (:189-191) both return
    /// silently; this reports which happened, because the callers that care
    /// (`BasicBoard.removeItems`, BasicBoard.java:636-647) re-test the predicate themselves and a
    /// `bool` lets a Rust caller skip that. `false` means "nothing was removed".
    ///
    /// Note the tree removal comes **before** the item-list removal (:193-194): the item must
    /// still be reachable while its leaves are being dropped.
    // renamed: `void removeItem(Item)` -> `remove_item(ItemId) -> bool`; Java's silent
    // no-op paths are the `false` results.
    pub fn remove_item(&mut self, id: ItemId) -> bool {
        let Some(item) = self.items.get(&id) else {
            // BoardItemRepository.java:171-173.
            return false;
        };
        // BoardItemRepository.java:189-191.
        if item.is_deletion_forbidden(&self.rules) {
            return false;
        }
        // not reachable: RoutingBoard.additionalUpdateAfterChange (retainAutorouteDatabase is a Java benchmark-only system property)
        // (BoardItemRepository.java:192) — see the marker on `insert_item` above for why ruling AJ
        // makes the whole family dead rather than deferred.
        // BoardItemRepository.java:193.
        let item = self
            .items
            .get_mut(&id)
            .expect("Board::remove_item: present, just checked");
        self.trees.remove(item);
        // BoardItemRepository.java:194.
        self.items.remove(&id);
        // `itemList.delete`'s undo half (UndoableObjects.java:76-125) — see
        // `Board::journal_remove`; a no-op outside `BatchOptimizer.optRouteItem`'s window.
        self.journal_remove(id);
        // BoardItemRepository.java:198.
        self.revision += 1;
        true
    }

    /// Port of `BasicBoard.removeItems` (BasicBoard.java:636-647) and the identical
    /// `BoardItemRepository.removeItems` (BoardItemRepository.java:202-212): removes every item
    /// it is allowed to, and reports whether all of them went.
    // totalized: Java's parameter is a `Collection<Item>` of live references, so "an item the
    // board does not have" cannot arise there (a `null` element would NPE at
    // `isDeletionForbidden`). This takes `ItemId`s, because the port keys items by id, and skips
    // an id the board does not have. See docs/java-quirks.md.
    pub fn remove_items(&mut self, ids: impl IntoIterator<Item = ItemId>) -> bool {
        let mut result = true;
        for id in ids {
            let Some(item) = self.items.get(&id) else {
                continue;
            };
            if item.is_deletion_forbidden(&self.rules) || item.is_user_fixed() {
                // BasicBoard.java:639-641.
                result = false;
            } else {
                self.remove_item(id);
            }
        }
        result
    }

    // -- the typed inserters (BasicBoard) --------------------------------------------------------

    /// Port of `BasicBoard.insertOutline` (BasicBoard.java:576-580).
    pub fn insert_outline(
        &mut self,
        outline_shapes: Vec<PolylineShapeRef>,
        clearance_class: usize,
    ) -> ItemId {
        let id = self.new_item_id();
        // BoardOutline.java:46-49: `new int[0]`, component 0, SYSTEM_FIXED.
        let outline = BoardOutline::new(
            ItemHeader::new(id, Vec::new(), clearance_class, 0, FixedState::SystemFixed),
            outline_shapes,
        );
        self.insert_item(Item::BoardOutline(outline))
    }

    /// Port of `BasicBoard.insertTraceWithoutCleaning` (BasicBoard.java:178-202).
    ///
    /// Returns `None` for Java's two `null` returns: a polyline with fewer than two corners
    /// (:185-187), and a closed trace that is not at least `USER_FIXED` (:191-195).
    pub fn insert_trace_without_cleaning(
        &mut self,
        polyline: Polyline,
        layer: usize,
        half_width: i32,
        net_nos: Vec<i32>,
        clearance_class: usize,
        fixed_state: FixedState,
    ) -> Option<ItemId> {
        // BasicBoard.java:185-187.
        if polyline.corner_count() < 2 {
            return None;
        }
        let id = self.new_item_id();
        let layer_count = self.get_layer_count();
        let new_trace = PolylineTrace::new(
            ItemHeader::new(id, net_nos, clearance_class, 0, fixed_state),
            polyline,
            layer,
            half_width,
            Some(layer_count),
        );
        // BasicBoard.java:191-195.
        if new_trace.first_corner() == new_trace.last_corner()
            && (fixed_state as u8) < (FixedState::UserFixed as u8)
        {
            return None;
        }
        let nets_normal = new_trace.hdr.nets_normal();
        self.insert_item(Item::Trace(new_trace));
        // BasicBoard.java:197-200.
        if nets_normal {
            self.max_trace_half_width = self.max_trace_half_width.max(half_width);
            self.min_trace_half_width = self.min_trace_half_width.min(half_width);
        }
        Some(id)
    }

    /// Port of `BasicBoard.insertTrace(Polyline, …)` (BasicBoard.java:209-242): insert, then
    /// normalise inside the changed area.
    ///
    /// This is one of the two places a normalisation failure does **not** propagate: Java wraps
    /// `newTrace.normalize(clipShape)` in its own `catch (Exception)` (:230-241) — "the segment
    /// is skipped and the connection may remain unrouted" — and quirk #22's
    /// `ArrayIndexOutOfBoundsException` is exactly such an exception. The port swallows the
    /// [`BoardError`](crate::BoardError) at the same line. The other is
    /// [`Board::change_trace`](crate::Board::change_trace)
    /// (`crates/fr-board/src/board/trace_normalize.rs`), where Java catches the same way
    /// (`PolylineTrace.changeTrace`, PolylineTrace.java:1000-1004); every *other* caller of
    /// [`Board::normalize_trace`] threads the error out.
    // not ported: the `FRLogger.warn`/`FRLogger.debug` pair in that catch block
    // (BasicBoard.java:233-240).
    pub fn insert_trace(
        &mut self,
        polyline: Polyline,
        layer: usize,
        half_width: i32,
        net_nos: Vec<i32>,
        clearance_class: usize,
        fixed_state: FixedState,
    ) -> Option<ItemId> {
        let id = self.insert_trace_without_cleaning(
            polyline,
            layer,
            half_width,
            net_nos,
            clearance_class,
            fixed_state,
        )?;
        // BasicBoard.java:222-229: the clip shape is the changed area of this layer, when one is
        // being marked.
        let clip_shape = self.changed_area.as_ref().map(|area| area.get_area(layer));
        let _ = self.normalize_trace(id, clip_shape.as_ref());
        Some(id)
    }

    /// Port of the `BasicBoard.insertTrace(Point[], …)` overload (BasicBoard.java:248-262).
    ///
    /// Java's out-of-range warning per point (:256-258) is dropped; it does not change the
    /// result.
    // renamed: the `Point[]` overload -> insert_trace_at_points (Rust has no overloading).
    pub fn insert_trace_at_points(
        &mut self,
        points: &[Point],
        layer: usize,
        half_width: i32,
        net_nos: Vec<i32>,
        clearance_class: usize,
        fixed_state: FixedState,
    ) -> Option<ItemId> {
        let polyline = Polyline::from_points(points);
        self.insert_trace(
            polyline,
            layer,
            half_width,
            net_nos,
            clearance_class,
            fixed_state,
        )
    }

    /// Port of `BasicBoard.insertVia` (BasicBoard.java:268-295).
    ///
    /// Delegates to [`Self::insert_via_checked`] with a `|| false` stop check, exactly as Plan 3
    /// did for `normalize_all_traces`, so every Plan 2-5 caller is untouched.
    pub fn insert_via(
        &mut self,
        padstack: crate::ids::PadstackId,
        center: Point,
        net_nos: Vec<i32>,
        clearance_class: usize,
        fixed_state: FixedState,
        attach_allowed: bool,
    ) -> Result<ItemId, crate::BoardError> {
        self.insert_via_checked(
            padstack,
            center,
            net_nos,
            clearance_class,
            fixed_state,
            attach_allowed,
            &|| false,
        )
    }

    /// [`Self::insert_via`] under a [`StopCheck`](crate::datastructures::StopCheck), threaded
    /// into the `splitTraces` loop at `BasicBoard.java:287-293` and through it into
    /// `PolylineTrace.split`'s entry re-walk — **the loop quirk #76 never leaves**.
    ///
    /// This is what closes `docs/plan-3-handoff.md`'s ruling F: plan-3 gave `split_trace` and
    /// `normalize_all_traces` a stop check and deferred "the other caller" until one existed.
    /// Plan-6 ruling 6 names it: `ForcedViaInserter.insert`
    /// (`board/actions/ForcedViaInserter.java:348`) reaches `BasicBoard.insertVia` from inside
    /// the router, on a board the router has been shoving traces around on. A trip answers
    /// [`BoardError::Stopped`](crate::BoardError::Stopped); the via is already inserted and the
    /// board is left part-split, exactly as Java's would be if its `split` threw.
    // renamed: `BasicBoard.insertVia` under a `StopCheck` -> `Board::insert_via_checked` (plan-6 ruling 6, closing plan-3 ruling F; the unchecked `Board::insert_via` delegates to it with `|| false`).
    #[allow(clippy::too_many_arguments)]
    pub fn insert_via_checked(
        &mut self,
        padstack: crate::ids::PadstackId,
        center: Point,
        net_nos: Vec<i32>,
        clearance_class: usize,
        fixed_state: FixedState,
        attach_allowed: bool,
        stop: crate::datastructures::StopCheck<'_>,
    ) -> Result<ItemId, crate::BoardError> {
        let id = self.new_item_id();
        let via = Via::new(
            ItemHeader::new(id, net_nos.clone(), clearance_class, 0, fixed_state),
            padstack,
            center.clone(),
            attach_allowed,
        );
        self.insert_item(Item::Via(via));
        // BasicBoard.java:287-293. Note the exclusive upper bound, one layer narrower than
        // `insertEscapeVia`'s.
        let (from_layer, to_layer) = self.padstack_layer_range(padstack);
        for layer in from_layer..to_layer {
            for net_number in &net_nos {
                self.split_traces_checked(&center, layer as usize, *net_number, stop)?;
            }
        }
        Ok(id)
    }

    /// Port of `BasicBoard.insertEscapeVia` (BasicBoard.java:310-330): a via sitting on an SMD
    /// pad, which uses SMD-to-SMD clearance on `smd_layer` and normal via clearance elsewhere.
    ///
    /// Note the layer loop is `fromLayer..=toLayer` here (:324), one layer wider than
    /// `insertVia`'s `fromLayer..toLayer` (:289).
    pub fn insert_escape_via(
        &mut self,
        padstack: crate::ids::PadstackId,
        center: Point,
        net_nos: Vec<i32>,
        clearance_class: usize,
        fixed_state: FixedState,
        smd_layer: usize,
    ) -> Result<ItemId, crate::BoardError> {
        self.insert_escape_via_checked(
            padstack,
            center,
            net_nos,
            clearance_class,
            fixed_state,
            smd_layer,
            &|| false,
        )
    }

    /// [`Self::insert_escape_via`] under a [`StopCheck`](crate::datastructures::StopCheck), for
    /// the same reason as [`Self::insert_via_checked`] (plan-6 ruling 6). Nothing in Plan 6 calls
    /// it yet — `ForcedViaInserter.insert` reaches `insertVia` — but ruling 6 names all three
    /// entry points, and leaving one of them uncancellable would be a hole the next caller falls
    /// into.
    // renamed: `BasicBoard.insertEscapeVia` under a `StopCheck` -> `Board::insert_escape_via_checked` (plan-6 ruling 6, closing plan-3 ruling F; the unchecked `Board::insert_escape_via` delegates to it with `|| false`).
    #[allow(clippy::too_many_arguments)]
    pub fn insert_escape_via_checked(
        &mut self,
        padstack: crate::ids::PadstackId,
        center: Point,
        net_nos: Vec<i32>,
        clearance_class: usize,
        fixed_state: FixedState,
        smd_layer: usize,
        stop: crate::datastructures::StopCheck<'_>,
    ) -> Result<ItemId, crate::BoardError> {
        let id = self.new_item_id();
        let mut via = Via::new(
            ItemHeader::new(id, net_nos.clone(), clearance_class, 0, fixed_state),
            padstack,
            center.clone(),
            true,
        );
        // BasicBoard.java:319-320.
        via.is_escape_via = true;
        via.escape_via_smd_layer = smd_layer as i32;
        self.insert_item(Item::Via(via));
        // BasicBoard.java:322-328 — `fromLayer..=toLayer`, one layer wider than `insertVia`'s.
        let (from_layer, to_layer) = self.padstack_layer_range(padstack);
        for layer in from_layer..=to_layer {
            for net_number in &net_nos {
                self.split_traces_checked(&center, layer as usize, *net_number, stop)?;
            }
        }
        Ok(id)
    }

    /// `padstack.fromLayer()` / `padstack.toLayer()` (BasicBoard.java:288-289), resolved through
    /// the board's library.
    ///
    /// Not a Java method: Java's two inserters hold the `Padstack` itself, while this port keys
    /// it by [`PadstackId`](crate::ids::PadstackId).
    fn padstack_layer_range(&self, padstack: crate::ids::PadstackId) -> (i32, i32) {
        let padstack = self
            .library
            .padstacks
            .get(padstack)
            .expect("Board::insertVia: the padstack of an inserted via is in the library");
        (padstack.from_layer(), padstack.to_layer())
    }

    /// Port of `BasicBoard.insertPin` (BasicBoard.java:336-346).
    pub fn insert_pin(
        &mut self,
        component_id: i32,
        pin_index: i32,
        net_nos: Vec<i32>,
        clearance_class: usize,
        fixed_state: FixedState,
    ) -> ItemId {
        let id = self.new_item_id();
        let pin = Pin::new(
            ItemHeader::new(id, net_nos, clearance_class, component_id, fixed_state),
            pin_index,
        );
        self.insert_item(Item::Pin(pin))
    }

    /// Port of `BasicBoard.insertObstacle(Area, int, int, FixedState)`
    /// (BasicBoard.java:352-363).
    pub fn insert_obstacle(
        &mut self,
        area: Area,
        layer: usize,
        clearance_class: usize,
        fixed_state: FixedState,
    ) -> ItemId {
        self.insert_obstacle_of_component(
            area,
            layer,
            Vector::ZERO,
            0.0,
            false,
            clearance_class,
            0,
            None,
            fixed_state,
        )
    }

    /// Port of the nine-argument `BasicBoard.insertObstacle` (BasicBoard.java:369-398): an
    /// obstacle area belonging to a component.
    // renamed: the component overload -> insert_obstacle_of_component (Rust has no overloading).
    #[allow(clippy::too_many_arguments)] // Java's parameter list, kept.
    pub fn insert_obstacle_of_component(
        &mut self,
        area: Area,
        layer: usize,
        translation: Vector,
        rotation_in_degree: f64,
        side_changed: bool,
        clearance_class: usize,
        component_id: i32,
        name: Option<String>,
        fixed_state: FixedState,
    ) -> ItemId {
        let id = self.new_item_id();
        let obstacle = ObstacleArea::new(
            ItemHeader::new(id, Vec::new(), clearance_class, component_id, fixed_state),
            ObstacleAreaData::new(
                area,
                layer,
                translation,
                rotation_in_degree,
                side_changed,
                name,
            ),
        );
        self.insert_item(Item::ObstacleArea(obstacle))
    }

    /// Port of `BasicBoard.insertViaObstacle(Area, int, int, FixedState)`
    /// (BasicBoard.java:404-415).
    pub fn insert_via_obstacle(
        &mut self,
        area: Area,
        layer: usize,
        clearance_class: usize,
        fixed_state: FixedState,
    ) -> ItemId {
        self.insert_via_obstacle_of_component(
            area,
            layer,
            Vector::ZERO,
            0.0,
            false,
            clearance_class,
            0,
            None,
            fixed_state,
        )
    }

    /// Port of the nine-argument `BasicBoard.insertViaObstacle` (BasicBoard.java:421-450).
    #[allow(clippy::too_many_arguments)] // Java's parameter list, kept.
    pub fn insert_via_obstacle_of_component(
        &mut self,
        area: Area,
        layer: usize,
        translation: Vector,
        rotation_in_degree: f64,
        side_changed: bool,
        clearance_class: usize,
        component_id: i32,
        name: Option<String>,
        fixed_state: FixedState,
    ) -> ItemId {
        let id = self.new_item_id();
        let obstacle = ViaObstacleArea::new(
            ItemHeader::new(id, Vec::new(), clearance_class, component_id, fixed_state),
            ObstacleAreaData::new(
                area,
                layer,
                translation,
                rotation_in_degree,
                side_changed,
                name,
            ),
        );
        self.insert_item(Item::ViaObstacleArea(obstacle))
    }

    /// Port of `BasicBoard.insertComponentObstacle(Area, int, int, FixedState)`
    /// (BasicBoard.java:456-467).
    pub fn insert_component_obstacle(
        &mut self,
        area: Area,
        layer: usize,
        clearance_class: usize,
        fixed_state: FixedState,
    ) -> ItemId {
        self.insert_component_obstacle_of_component(
            area,
            layer,
            Vector::ZERO,
            0.0,
            false,
            clearance_class,
            0,
            None,
            fixed_state,
        )
    }

    /// Port of the nine-argument `BasicBoard.insertComponentObstacle`
    /// (BasicBoard.java:473-502).
    #[allow(clippy::too_many_arguments)] // Java's parameter list, kept.
    pub fn insert_component_obstacle_of_component(
        &mut self,
        area: Area,
        layer: usize,
        translation: Vector,
        rotation_in_degree: f64,
        side_changed: bool,
        clearance_class: usize,
        component_id: i32,
        name: Option<String>,
        fixed_state: FixedState,
    ) -> ItemId {
        let id = self.new_item_id();
        let obstacle = ComponentObstacleArea::new(
            ItemHeader::new(id, Vec::new(), clearance_class, component_id, fixed_state),
            ObstacleAreaData::new(
                area,
                layer,
                translation,
                rotation_in_degree,
                side_changed,
                name,
            ),
        );
        self.insert_item(Item::ComponentObstacleArea(obstacle))
    }

    /// Port of `BasicBoard.insertComponentOutline` (BasicBoard.java:505-538).
    ///
    /// Java's two `null` returns are `None`: a missing area (:515-518) and an unbounded one
    /// (:519-522).
    #[allow(clippy::too_many_arguments)] // Java's parameter list, kept.
    pub fn insert_component_outline(
        &mut self,
        area: Area,
        is_front: bool,
        translation: Vector,
        rotation_in_degree: f64,
        component_id: i32,
        is_courtyard: bool,
        is_fabrication: bool,
        is_closed: bool,
        fixed_state: FixedState,
    ) -> Option<ItemId> {
        // BasicBoard.java:519-522.
        if !area.is_bounded() {
            return None;
        }
        let id = self.new_item_id();
        // ComponentOutline.java:46: the constructor passes `new int[0], 0`.
        let outline = ComponentOutline::new(
            ItemHeader::new(id, Vec::new(), 0, component_id, fixed_state),
            area,
            is_front,
            translation,
            rotation_in_degree,
            is_courtyard,
            is_fabrication,
            is_closed,
        );
        Some(self.insert_item(Item::ComponentOutline(outline)))
    }

    /// Port of `BasicBoard.insertConductionArea` (BasicBoard.java:545-573).
    #[allow(clippy::too_many_arguments)] // Java's parameter list, kept.
    pub fn insert_conduction_area(
        &mut self,
        area: Area,
        layer: usize,
        net_nos: Vec<i32>,
        clearance_class: usize,
        is_obstacle: bool,
        fixed_state: FixedState,
    ) -> ItemId {
        let id = self.new_item_id();
        let conduction = ConductionArea::new(
            ItemHeader::new(id, net_nos, clearance_class, 0, fixed_state),
            ObstacleAreaData::new(area, layer, Vector::ZERO, 0.0, false, None),
            is_obstacle,
        );
        self.insert_item(Item::ConductionArea(conduction))
    }

    /// Port of `BasicBoard.makeConductive` (BasicBoard.java:1188-1216): turns an obstacle area
    /// into a conduction area on `net_number`.
    ///
    /// Java removes the old item and inserts a brand-new `ConductionArea` built from its
    /// geometry, with `isObstacle` hard-coded to `true` (:1210). `None` if `id` is not an
    /// [`Item::ObstacleArea`] — Java's parameter is typed `ObstacleArea`, so that cannot arise
    /// there.
    pub fn make_conductive(&mut self, id: ItemId, net_number: i32) -> Option<ItemId> {
        let Some(Item::ObstacleArea(area)) = self.items.get(&id) else {
            return None;
        };
        let clearance_class = area.hdr.clearance_class();
        let component_id = area.hdr.get_component_id();
        let fixed_state = area.hdr.get_fixed_state();
        let data = ObstacleAreaData::new(
            area.get_relative_area().clone(),
            area.get_layer(),
            area.get_translation().clone(),
            area.get_rotation_in_degree(),
            area.get_side_changed(),
            area.name().map(str::to_string),
        );
        let new_id = self.new_item_id();
        let new_item = ConductionArea::new(
            ItemHeader::new(
                new_id,
                vec![net_number],
                clearance_class,
                component_id,
                fixed_state,
            ),
            data,
            true,
        );
        // BasicBoard.java:1213-1214, in Java's order.
        self.remove_item(id);
        Some(self.insert_item(Item::ConductionArea(new_item)))
    }

    // -- item-list queries (BoardItemRepository / BoardConnectivityQueries) -----------------------

    /// Port of `BasicBoard.getItem(int)` (BasicBoard.java:598-600) and
    /// `BoardItemRepository.getItem` (BoardItemRepository.java:48-59).
    pub fn get_item(&self, id: ItemId) -> Option<&Item> {
        self.items.get(&id)
    }

    /// [`Self::get_item`] for the callers that mutate. No Java counterpart — Java's `getItem`
    /// hands back the live object.
    pub fn get_item_mut(&mut self, id: ItemId) -> Option<&mut Item> {
        self.items.get_mut(&id)
    }

    /// Port of `BasicBoard.getItems` (BasicBoard.java:603-605) and
    /// `BoardItemRepository.getItems` (BoardItemRepository.java:62-72) — **in Java's order**,
    /// i.e. descending id (quirk #63).
    pub fn get_items(&self) -> impl DoubleEndedIterator<Item = &Item> {
        self.items.values().rev()
    }

    /// The item ids in `board.itemList` order: descending (quirk #63).
    ///
    /// Not a Java method; it exists so a `&mut self` walk can collect the ids first and then
    /// touch the items one at a time.
    pub fn items_in_board_order(&self) -> Vec<ItemId> {
        self.items.keys().rev().copied().collect()
    }

    /// Port of `BasicBoard.getOutline` (BasicBoard.java:583-585) and
    /// `BoardItemRepository.getOutline` (BoardItemRepository.java:34-45): the **first**
    /// `BoardOutline` in item-list order, i.e. the one with the highest id (quirk #63).
    pub fn get_outline(&self) -> Option<ItemId> {
        self.items
            .iter()
            .rev()
            .find(|(_, item)| matches!(item, Item::BoardOutline(_)))
            .map(|(id, _)| *id)
    }

    /// Port of `BasicBoard.getConductionAreas` (BasicBoard.java:650-652).
    pub fn get_conduction_areas(&self) -> Vec<ItemId> {
        self.ids_where(|item| matches!(item, Item::ConductionArea(_)))
    }

    /// Port of `BasicBoard.getPins` (BasicBoard.java:655-657).
    pub fn get_pins(&self) -> Vec<ItemId> {
        self.ids_where(|item| matches!(item, Item::Pin(_)))
    }

    /// Port of `BasicBoard.getSmdPins` (BasicBoard.java:660-662): the pins that live on exactly
    /// one layer.
    pub fn get_smd_pins(&self) -> Vec<ItemId> {
        let ctx = self.ctx();
        self.ids_where(|item| {
            matches!(item, Item::Pin(_)) && item.first_layer(&ctx) == item.last_layer(&ctx)
        })
    }

    /// Port of `BasicBoard.getVias` (BasicBoard.java:665-667).
    pub fn get_vias(&self) -> Vec<ItemId> {
        self.ids_where(|item| matches!(item, Item::Via(_)))
    }

    /// Port of `BasicBoard.getTraces` (BasicBoard.java:670-672).
    pub fn get_traces(&self) -> Vec<ItemId> {
        self.ids_where(Item::is_trace)
    }

    /// Port of `BasicBoard.cumulativeTraceLength` (BasicBoard.java:675-677) ->
    /// `BoardItemRepository.cumulativeTraceLength` (BoardItemRepository.java:124-133).
    ///
    /// # `fold(0.0, …)`, not `.sum()`
    ///
    /// Java is a plain accumulator loop that starts at `double result = 0`, i.e. **positive**
    /// zero. Rust's `impl Sum for f64` folds from **`-0.0`** (so that summing `[-0.0]` answers
    /// `-0.0`), and `-0.0` is what an empty iterator therefore returns — which
    /// `Double.toString` renders as `"-0.0"` where Java renders `"0.0"`. It is invisible the
    /// moment any non-zero length is added (`-0.0 + x == x`), so the divergence is exactly
    /// "a board with no traces", plus the degenerate "a board whose every trace has length
    /// `-0.0`". Measured by `scripts/differential/run.sh p7t2` on
    /// `examples/tutorial_board/tutorial_board.dsn`, which routes nothing and so keeps an empty
    /// trace list all the way to the end of the pass. Fixed in Plan 7 Task 9.
    pub fn cumulative_trace_length(&self) -> f64 {
        self.items
            .values()
            .rev()
            .filter_map(|item| match item {
                Item::Trace(trace) => Some(trace.get_length()),
                _ => None,
            })
            .fold(0.0, |result, length| result + length)
    }

    /// Port of `BasicBoard.getNon45DegreeTraceCount` (BasicBoard.java:1465-1475).
    pub fn get_non_45_degree_trace_count(&self) -> usize {
        self.items
            .values()
            .rev()
            .filter(|item| match item {
                Item::Trace(trace) => !trace.polyline().is_multiple_of_45_degree(),
                _ => false,
            })
            .count()
    }

    /// Port of `BasicBoard.deleteAllTracksAndVias` (BasicBoard.java:1403-1419).
    ///
    /// Java deletes straight from `itemList` without touching the search trees — a genuine
    /// inconsistency, but it is what the method does, and the tree entries would dangle in Java
    /// too. This port cannot leave dangling leaves (they are arena handles owned by the tree), so
    /// it removes each item from the trees first.
    // totalized: Java's `itemList.delete` leaves the search trees holding leaves for the deleted
    // traces and vias; the port removes them, because a leaf here is an arena entry the tree owns.
    pub fn delete_all_tracks_and_vias(&mut self) {
        for id in self.items_in_board_order() {
            let Some(item) = self.items.get(&id) else {
                continue;
            };
            if !matches!(item, Item::Trace(_) | Item::Via(_)) {
                continue;
            }
            let item = self
                .items
                .get_mut(&id)
                .expect("Board::delete_all_tracks_and_vias: present, just checked");
            self.trees.remove(item);
            self.items.remove(&id);
            // BasicBoard.java:1412/:1416 — `itemList.delete(currentItem)`, whose undo half is
            // `Board::journal_remove`.
            self.journal_remove(id);
        }
    }

    /// Port of `BasicBoard.unfillConductionAreas` (BasicBoard.java:1426-1440).
    pub fn unfill_conduction_areas(&mut self) {
        self.rules.set_ignore_conduction(true);
        for item in self.items.values_mut().rev() {
            if let Item::ConductionArea(area) = item {
                area.set_is_filled(false);
                area.set_is_obstacle(false);
            }
        }
        self.reinsert_tree_items();
    }

    /// Port of `RoutingBoard.changeConductionIsObstacle(boolean)`
    /// (RoutingBoard.java:1252-1277): "sets, if all conduction areas on the board are obstacles
    /// for route of foreign nets" — see quirk #50.
    ///
    /// The per-item `ConductionArea::is_obstacle` flag is what
    /// [`Item::is_obstacle`](crate::items::Item::is_obstacle) /
    /// `is_trace_obstacle` / `is_drillable` actually read, so this method decides whether copper
    /// pours obstruct foreign-net routing — the most user-visible boolean on a KiCad board with
    /// ground pours.
    //
    // Java bug: the guard at RoutingBoard.java:1254 is `if (getIgnoreConduction() != value)
    // return;` — the method does nothing unless `rules.ignoreConduction` is in one particular
    // relation to the argument, so it alternates rather than mirrors. See docs/java-quirks.md #50.
    //
    // fixed: T10 (#50) — the guard is removed. **Decided, as the row's sketch asks, and the
    // decision is not quite the sketch's; here is the reasoning.**
    //
    // The flag's name is `ignoreConduction`, and the name is the specification: ignoring
    // conduction is the opposite of treating it as an obstacle. Two independent sites agree.
    // `BasicBoard.unfillConductionAreas:1427` writes `setIgnoreConduction(true)` beside
    // `setIsObstacle(false)` on every area, and the method's single Java caller,
    // `GuiBoardRoutingSettings.setIgnoreConduction(value):33-37`, calls
    // `changeConductionIsObstacle(!value)`. So `ignoreConduction == !isObstacle`, and the
    // `setIgnoreConduction(!value)` store at `:1273` is **correct** and stays. The register's
    // sketch — "guard `==`, store `value`" — is coherent only under the other reading, in which
    // the field means "conduction is obstacle"; that reading contradicts the field's name and both
    // of those sites, so it is not adopted. The sketch's real instruction, "decide what the flag
    // means and make it mean it", is what is followed.
    //
    // What is genuinely broken is the **guard**, which reads the board-level flag as a proxy for
    // the per-item ones. They desynchronise — `unfillConductionAreas` writes every area while
    // this method writes only signal-layer ones, and a loaded board arrives desynchronised
    // already (`p2t11_board` has `ignoreConduction = true` beside `isObstacle = true`). Once out
    // of sync the guard silently drops the user's request: on a board at the `ignoreConduction =
    // true` default (`BoardRules::new`), `change(false)` returns immediately, so "stop treating
    // pours as obstacles" does nothing at all, and the next call flips which of the two arguments
    // works. Removing it makes the method mean what it says: it always applies `value`, and it is
    // idempotent because the per-item `getIsObstacle() != value` test inside the loop — the one
    // test that reads the state it is about to change — already gates both the write and
    // `somethingChanged`.
    //
    // The `currentLayer.isSignal` restriction at `:1267` is **kept**, deliberately, and the
    // register row carries the reason as an open question: removing it would change whether a
    // *plane* layer's pour obstructs foreign nets, which is a different decision with a different
    // blast radius, and nothing in Plan 9 measures it. This method has no caller in the port
    // (its Java caller is the GUI), so the change here moves no routed byte on its own; it is a
    // correctness fix on the API a host would drive.
    pub fn change_conduction_is_obstacle(&mut self, value: bool) {
        let mut something_changed = false;
        // RoutingBoard.java:1259-1272, in item-list order (descending id).
        for item in self.items.values_mut().rev() {
            if let Item::ConductionArea(area) = item {
                let is_signal = self.rules.layer_structure().layers[area.get_layer()].is_signal;
                if is_signal && area.get_is_obstacle() != value {
                    area.set_is_obstacle(value);
                    something_changed = true;
                }
            }
        }
        // RoutingBoard.java:1273 — `ignoreConduction` is the negation of `isObstacle`; see above.
        self.rules.set_ignore_conduction(!value);
        if something_changed {
            self.reinsert_tree_items();
        }
    }

    /// Port of `SearchTreeManager.reinsertTreeItems` (SearchTreeManager.java:186-200) at board
    /// level: the item-list walk Java does inside the manager.
    ///
    /// Not a Java method of `BasicBoard`; it is the board half of the manager call that
    /// `unfillConductionAreas` (BasicBoard.java:1439) and `changeConductionIsObstacle`
    /// (RoutingBoard.java:1275) both make.
    pub fn reinsert_tree_items(&mut self) {
        let mut items = std::mem::take(&mut self.items);
        let ctx = item_ctx!(self);
        // quirk #63: `board.itemList` order is descending id.
        let mut refs: Vec<&mut Item> = items.values_mut().rev().collect();
        self.trees.reinsert_tree_shapes(&mut refs, &ctx);
        drop(refs);
        self.items = items;
    }

    /// Port of `SearchTreeManager.setClearanceCompensationUsed`
    /// (SearchTreeManager.java:89-108) at board level: the item-list walk Java does inside the
    /// manager (`removeAllBoardItems`/`insertAllBoardItems`, :202-231).
    ///
    /// Not a `BasicBoard` method — Java's callers reach the manager directly — but the item list
    /// it needs is the board's, in `board.itemList` order (descending id, quirk #63).
    pub fn set_clearance_compensation_used(&mut self, value: bool) {
        let mut items = std::mem::take(&mut self.items);
        let ctx = item_ctx!(self);
        let mut refs: Vec<&mut Item> = items.values_mut().rev().collect();
        self.trees
            .set_clearance_compensation_used(value, &mut refs, &ctx);
        drop(refs);
        self.items = items;
    }

    /// Port of `BoardOutline.generateKeepoutOutside(boolean)`'s search-tree half
    /// (BoardOutline.java:229-243): set the flag, then re-insert the outline so its tree shapes
    /// are recomputed.
    ///
    /// Java's guard is `if (board == null || board.searchTreeManager == null)`
    /// (BoardOutline.java:238-240) — on a board there is always a manager, so both halves run.
    /// `false` if `id` is not a board outline, or if the flag already had that value
    /// (BoardOutline.java:231-233, in which case Java returns before touching the trees).
    pub fn generate_keepout_outside(&mut self, id: ItemId, value: bool) -> bool {
        let Some(Item::BoardOutline(outline)) = self.items.get(&id) else {
            return false;
        };
        if outline.keepout_outside_outline_generated() == value {
            return false;
        }
        let mut item = self
            .items
            .remove(&id)
            .expect("Board::generate_keepout_outside: present, just checked");
        // BoardOutline.java:241: `searchTreeManager.remove(this)` first, so the old shapes go.
        self.trees.remove(&mut item);
        if let Item::BoardOutline(outline) = &mut item {
            outline.generate_keepout_outside(value);
        }
        let ctx = item_ctx!(self);
        // BoardOutline.java:242.
        self.trees.insert(&mut item, &ctx);
        self.items.insert(id, item);
        true
    }

    /// Port of `Item.changeClearanceClassIndex` (Item.java:937-950): the field write plus the
    /// search-tree re-insert that `setClearanceClassIndex` alone does not do.
    ///
    /// Java only re-inserts when clearance compensation is on (Item.java:946-949), because that
    /// is the only case where the stored tree shapes depend on the clearance class.
    pub fn change_clearance_class_index(&mut self, id: ItemId, index: usize) -> bool {
        if !self.items.contains_key(&id) {
            return false;
        }
        {
            let rules = &self.rules;
            let item = self
                .items
                .get_mut(&id)
                .expect("Board::change_clearance_class_index: present, just checked");
            item.set_clearance_class(index, rules);
            // Item.java:945.
            item.clear_derived_data();
        }
        // Item.java:946-949.
        if self.trees.is_clearance_compensation_used() {
            let mut item = self
                .items
                .remove(&id)
                .expect("Board::change_clearance_class_index: present, just checked");
            self.trees.remove(&mut item);
            let ctx = item_ctx!(self);
            self.trees.insert(&mut item, &ctx);
            self.items.insert(id, item);
        }
        true
    }

    /// Port of `Item.moveBy(Vector)` (Item.java:300-311): remove from the trees, translate,
    /// insert again.
    ///
    /// Java's `board.itemList.saveForUndo(this)` (Item.java:301) is `Board::save_for_undo`
    /// (Plan 7 Task 14c); its observer notification (:307-310) is dropped — no observers.
    ///
    /// `DrillItem` overrides this (DrillItem.java:95-145) to also draw a trace from the old
    /// centre to the new one on every layer where the drill item was contacting a trace; that
    /// override is reproduced here, except for the `board.insertTrace` tail's normalisation,
    /// which is Task 9's.
    pub fn move_item_by(&mut self, id: ItemId, vector: &Vector) -> Result<bool, crate::BoardError> {
        let Some(item) = self.items.get(&id) else {
            return Ok(false);
        };
        // DrillItem.java:96-110: remember the contact situation *before* the move.
        let is_drill_item = item.is_drill_item();
        let old_center = is_drill_item.then(|| self.drill_center(id).expect("a drill item"));
        //
        // Java's `Set<TraceInfo> contactTraceInfo` is a `TreeSet` whose `compareTo` is
        // `other.layer - this.layer` (DrillItem.java:412-414) — it compares the **layer alone**,
        // descending. So two contacting traces on the same layer collapse into one entry (the
        // first one added wins, as `TreeSet.add` keeps the incumbent), and the connecting traces
        // are then inserted in descending layer order. The `BTreeMap` below reproduces both:
        // `or_insert` keeps the incumbent, and the iteration is reversed.
        let mut by_layer: std::collections::BTreeMap<usize, (i32, usize)> =
            std::collections::BTreeMap::new();
        if is_drill_item {
            // `getNormalContacts()` is a `TreeSet<Item>`, i.e. descending id (quirk #44), which
            // is the order the "first one added wins" rule is resolved in.
            for contact_id in self.normal_contacts(id).into_iter().rev() {
                if let Some(Item::Trace(trace)) = self.items.get(&contact_id) {
                    by_layer
                        .entry(trace.get_layer())
                        .or_insert((trace.get_half_width(), trace.hdr.clearance_class()));
                }
            }
        }
        let contact_trace_info: Vec<(usize, i32, usize)> = by_layer
            .into_iter()
            .rev()
            .map(|(layer, (half_width, clearance_class))| (layer, half_width, clearance_class))
            .collect();

        // Item.java:301 — `board.itemList.saveForUndo(this)`; see `Board::save_for_undo`.
        self.save_for_undo(id);
        let mut item = self
            .items
            .remove(&id)
            .expect("Board::move_item_by: present, just checked");
        // Item.java:302-304.
        self.trees.remove(&mut item);
        let translated = item.translate_by(vector);
        let ctx = item_ctx!(self);
        self.trees.insert(&mut item, &ctx);
        let net_nos = item.net_nos().to_vec();
        self.items.insert(id, item);
        translated.map_err(crate::BoardError::Normalization)?;

        if let Some(old_center) = old_center {
            // DrillItem.java:113-144.
            let new_center = self.drill_center(id).expect("still a drill item");
            let mut connect_points = vec![old_center.clone()];
            if let (Point::Int(from), Point::Int(to)) = (&old_center, &new_center) {
                let add_corner = match self.rules.trace_angle_restriction {
                    crate::structure::AngleRestriction::NinetyDegree => {
                        Some(from.ninety_degree_corner(to, true))
                    }
                    crate::structure::AngleRestriction::FortyFiveDegree => {
                        Some(from.fortyfive_degree_corner(to, true))
                    }
                    crate::structure::AngleRestriction::None => None,
                };
                if let Some(Some(corner)) = add_corner {
                    connect_points.push(Point::Int(corner));
                }
            }
            connect_points.push(new_center);
            for (layer, half_width, clearance_class) in contact_trace_info {
                self.insert_trace_at_points(
                    &connect_points,
                    layer,
                    half_width,
                    net_nos.clone(),
                    clearance_class,
                    FixedState::Unfixed,
                );
            }
        }
        Ok(true)
    }

    /// Port of the package-private `PolylineTraceSearchTreeAdapter.hasDefaultEntries`
    /// (PolylineTraceSearchTreeAdapter.java:22-26): do both traces already have entries in the
    /// default tree?
    pub fn trace_has_default_entries(&self, first: ItemId, second: ItemId) -> bool {
        let default_tree = self.default_tree_id();
        [first, second].into_iter().all(|id| {
            self.items
                .get(&id)
                .is_some_and(|item| item.get_search_tree_entries(default_tree).is_some())
        })
    }

    /// Port of the package-private `PolylineTraceSearchTreeAdapter.replaceGeometry`
    /// (PolylineTraceSearchTreeAdapter.java:34-40): swap a trace's polyline through the safe
    /// full-remove/reinsert path.
    ///
    /// The order is Java's and the Java comment says it is significant: remove the old entries
    /// first, then change the geometry and drop the derived data, then insert.
    pub fn replace_trace_geometry(&mut self, id: ItemId, new_polyline: Polyline) -> bool {
        if !matches!(self.items.get(&id), Some(Item::Trace(_))) {
            return false;
        }
        let mut item = self
            .items
            .remove(&id)
            .expect("Board::replace_trace_geometry: present, just checked");
        self.trees.remove(&mut item);
        item.clear_tree_entries();
        if let Item::Trace(trace) = &mut item {
            trace.set_polyline(new_polyline);
        }
        item.clear_derived_data();
        let ctx = item_ctx!(self);
        self.trees.insert(&mut item, &ctx);
        self.items.insert(id, item);
        true
    }

    /// Port of the package-private `PolylineTraceSearchTreeAdapter.mergeEntriesInFront`
    /// (PolylineTraceSearchTreeAdapter.java:42-50).
    pub fn merge_trace_entries_in_front(
        &mut self,
        from_trace: ItemId,
        to_trace: ItemId,
        joined_polyline: &Polyline,
        from_entry_no: usize,
        to_entry_no: usize,
    ) -> bool {
        self.with_two_traces(from_trace, to_trace, |trees, rules, from, to| {
            trees.merge_entries_in_front(
                from,
                to,
                joined_polyline,
                from_entry_no,
                to_entry_no,
                rules,
            );
        })
    }

    /// Port of the package-private `PolylineTraceSearchTreeAdapter.mergeEntriesAtEnd`
    /// (PolylineTraceSearchTreeAdapter.java:52-60).
    pub fn merge_trace_entries_at_end(
        &mut self,
        from_trace: ItemId,
        to_trace: ItemId,
        joined_polyline: &Polyline,
        from_entry_no: usize,
        to_entry_no: usize,
    ) -> bool {
        self.with_two_traces(from_trace, to_trace, |trees, rules, from, to| {
            trees.merge_entries_at_end(
                from,
                to,
                joined_polyline,
                from_entry_no,
                to_entry_no,
                rules,
            );
        })
    }

    /// Port of the package-private `PolylineTraceSearchTreeAdapter.changeEntries`
    /// (PolylineTraceSearchTreeAdapter.java:62-66).
    pub fn change_trace_entries(
        &mut self,
        id: ItemId,
        new_polyline: &Polyline,
        keep_at_start_count: usize,
        keep_at_end_count: usize,
    ) -> bool {
        let Some(Item::Trace(_)) = self.items.get(&id) else {
            return false;
        };
        let mut item = self.items.remove(&id).expect("present, just checked");
        if let Item::Trace(trace) = &mut item {
            self.trees.change_entries(
                trace,
                new_polyline,
                keep_at_start_count,
                keep_at_end_count,
                &self.rules,
            );
        }
        self.items.insert(id, item);
        true
    }

    /// Takes two traces out of the item map so both can be borrowed mutably at once, runs `f`,
    /// and puts them back. Not a Java method: Java's two traces are independent references.
    fn with_two_traces(
        &mut self,
        from_id: ItemId,
        to_id: ItemId,
        f: impl FnOnce(&mut SearchTreeManager, &BoardRules, &mut PolylineTrace, &mut PolylineTrace),
    ) -> bool {
        if from_id == to_id
            || !matches!(self.items.get(&from_id), Some(Item::Trace(_)))
            || !matches!(self.items.get(&to_id), Some(Item::Trace(_)))
        {
            return false;
        }
        let mut from_item = self.items.remove(&from_id).expect("present, just checked");
        let mut to_item = self.items.remove(&to_id).expect("present, just checked");
        if let (Item::Trace(from), Item::Trace(to)) = (&mut from_item, &mut to_item) {
            f(&mut self.trees, &self.rules, from, to);
        }
        self.items.insert(from_id, from_item);
        self.items.insert(to_id, to_item);
        true
    }

    // -- the lazily filled tree-shape cache (Item.java:194-238) ---------------------------------

    /// Port of the private `Item.getPrecalculatedTreeShapes(ShapeTree)` (Item.java:227-238): the
    /// cached array for `tree`, **computed and stored on first use**.
    ///
    /// This is the lazy fill Task 10 could not do: `ShapeSearchTree::calculate_tree_shapes` needs
    /// the tree, the item and the board context at once, which only `Board` holds. `None` if the
    /// board has no such item; otherwise the length of the cached array.
    fn fill_tree_shapes(&mut self, id: ItemId, tree: TreeId) -> Option<usize> {
        let item = self.items.get(&id)?;
        if let Some(shapes) = item.header().get_precalculated_tree_shapes(tree) {
            return Some(shapes.len());
        }
        let search_tree = self.trees.trees().find(|t| t.id() == tree)?;
        let ctx = item_ctx!(self);
        let shapes = search_tree.calculate_tree_shapes(item, &ctx);
        let len = shapes.len();
        self.items
            .get_mut(&id)
            .expect("Board::fill_tree_shapes: present, just read")
            .set_precalculated_tree_shapes(tree, shapes);
        Some(len)
    }

    /// Port of `Item.treeShapeCount(ShapeTree)` (Item.java:203-210), **with** the lazy fill —
    /// `Item::tree_shape_count` reads the cache only.
    pub fn item_tree_shape_count(&mut self, id: ItemId, tree: TreeId) -> usize {
        self.fill_tree_shapes(id, tree).unwrap_or(0)
    }

    /// Port of `Item.shapeLayer(int)` (Item.java:809-814) for callers outside this crate:
    /// [`Item::shape_layer`] needs an [`ItemCtx`], which only a `Board`
    /// can build.
    ///
    /// `None` is an id the board does not hold — Java would have NPE'd on the `Item` reference
    /// its caller already has. Added for `autoroute.expansion.ObstacleExpansionRoom.getLayer`
    /// (`autoroute/expansion/ObstacleExpansionRoom.java:38-41`), which recomputes the layer on
    /// every call rather than caching it beside the shape.
    pub fn item_shape_layer(&self, id: ItemId, index: usize) -> Option<usize> {
        let ctx = item_ctx!(self);
        Some(self.items.get(&id)?.shape_layer(index, &ctx))
    }

    /// Port of `Item.getTreeShape(ShapeTree, int)` (Item.java:212-226), **with** the lazy fill
    /// and its one `clearDerivedData()` retry (Item.java:218-221) — `Item::get_tree_shape` reads
    /// the cache only.
    pub fn item_tree_shape(&mut self, id: ItemId, tree: TreeId, index: usize) -> Option<TileShape> {
        let len = self.fill_tree_shapes(id, tree)?;
        if index >= len {
            // Item.java:218-221: drop everything derived and recompute once.
            self.items.get_mut(&id)?.clear_derived_data();
            if index >= self.fill_tree_shapes(id, tree)? {
                return None;
            }
        }
        self.items.get(&id)?.get_tree_shape(tree, index).cloned()
    }

    /// Port of `Item.getTileShape(int)` (Item.java:194-201) — [`Self::item_tree_shape`] against
    /// the default tree — and its one override, `ObstacleArea.getTileShape`
    /// (ObstacleArea.java:197-205), which the three area subclasses inherit and which splits the
    /// area itself instead of consulting a tree.
    pub fn item_tile_shape(&mut self, id: ItemId, index: usize) -> Option<TileShape> {
        {
            let ctx = item_ctx!(self);
            let item = self.items.get(&id)?;
            match item {
                Item::ObstacleArea(i) => return i.get_tile_shape(index, &ctx),
                Item::ConductionArea(i) => return i.get_tile_shape(index, &ctx),
                Item::ViaObstacleArea(i) => return i.get_tile_shape(index, &ctx),
                Item::ComponentObstacleArea(i) => return i.get_tile_shape(index, &ctx),
                _ => {}
            }
        }
        let default_tree = self.default_tree_id();
        self.item_tree_shape(id, default_tree, index)
    }

    /// Port of `Item.getTreeShape(ShapeTree, int)` (Item.java:212-226) for the `&self` callers:
    /// the cached shape, or the recomputed one when the cache is cold.
    ///
    /// Java *stores* what it recomputes (Item.java:233-236); this cannot, because `&self` cannot
    /// mutate the item. Use [`Self::item_tree_shape`] wherever the caller already has `&mut
    /// self` — it stores, so a cold cache costs one `calculateTreeShapes` instead of one per
    /// call. The result is the same either way.
    pub fn item_tree_shape_ref(
        &self,
        id: ItemId,
        tree: TreeId,
        index: usize,
    ) -> Option<Cow<'_, TileShape>> {
        let item = self.items.get(&id)?;
        let search_tree = self.trees.trees().find(|t| t.id() == tree)?;
        search_tree.get_tree_shape(item, index, &self.ctx())
    }

    /// Port of `Item.getTileShape(int)` (Item.java:194-201) for the `&self` callers — see
    /// [`Self::item_tree_shape_ref`] — together with `ObstacleArea.getTileShape`
    /// (ObstacleArea.java:197-205), which splits the area itself and never consults a tree.
    pub fn item_tile_shape_ref(&self, id: ItemId, index: usize) -> Option<Cow<'_, TileShape>> {
        let ctx = self.ctx();
        let item = self.items.get(&id)?;
        match item {
            Item::ObstacleArea(i) => return i.get_tile_shape(index, &ctx).map(Cow::Owned),
            Item::ConductionArea(i) => return i.get_tile_shape(index, &ctx).map(Cow::Owned),
            Item::ViaObstacleArea(i) => return i.get_tile_shape(index, &ctx).map(Cow::Owned),
            Item::ComponentObstacleArea(i) => return i.get_tile_shape(index, &ctx).map(Cow::Owned),
            _ => {}
        }
        self.item_tree_shape_ref(id, self.default_tree_id(), index)
    }

    /// Port of `DrillItem.getTileShapeOnLayer(int)` (DrillItem.java:252-260) for the `&self`
    /// callers — see [`Self::item_tree_shape_ref`].
    pub fn drill_item_tile_shape_on_layer_ref(
        &self,
        id: ItemId,
        layer: usize,
    ) -> Option<Cow<'_, TileShape>> {
        let ctx = self.ctx();
        let item = self.items.get(&id)?;
        if !item.is_drill_item() {
            return None;
        }
        let from_layer = item.first_layer(&ctx);
        if layer < from_layer || layer > item.last_layer(&ctx) {
            return None;
        }
        self.item_tile_shape_ref(id, layer - from_layer)
    }

    /// Port of `DrillItem.getTileShapeOnLayer(int)` (DrillItem.java:252-260), **with** the lazy
    /// fill its `getTileShape(layer - firstLayer())` inherits — `Via::get_tile_shape_on_layer`
    /// and `Pin::get_tile_shape_on_layer` read the cache only.
    ///
    /// `None` for Java's out-of-range warning path and for an item that is not a drill item.
    pub fn drill_item_tile_shape_on_layer(
        &mut self,
        id: ItemId,
        layer: usize,
    ) -> Option<TileShape> {
        let ctx = self.ctx();
        let item = self.items.get(&id)?;
        if !item.is_drill_item() {
            return None;
        }
        let from_layer = item.first_layer(&ctx);
        let to_layer = item.last_layer(&ctx);
        if layer < from_layer || layer > to_layer {
            return None;
        }
        self.item_tile_shape(id, layer - from_layer)
    }

    /// Port of `Item.validate` (Item.java:796-807) and its one override, `Trace.validate`
    /// (Trace.java:447-456), which additionally rejects a trace whose first and last corner are
    /// equal. Java's two `FRLogger.warn` calls are dropped.
    ///
    /// `&mut self` because `getTileShape` fills the tree-shape cache on the way
    /// (Item.java:227-238), and `changeClearanceClassIndex` leaves it cold.
    pub fn validate_item(&mut self, id: ItemId) -> bool {
        let ctx = self.ctx();
        let Some(item) = self.items.get(&id) else {
            return true;
        };
        let mut result = self.trees.validate_entries(item);
        let shape_count = item.tile_shape_count(&ctx);
        for i in 0..shape_count {
            match self.item_tile_shape(id, i) {
                Some(shape) if !shape.is_empty() => {}
                // Java's `getTileShape(i).isEmpty()` NPEs on a `null` shape; a `None` here is the
                // same inconsistency the check exists to catch, so it counts as a failure.
                _ => result = false,
            }
        }
        if let Some(Item::Trace(trace)) = self.items.get(&id)
            && trace.first_corner() == trace.last_corner()
        {
            // Trace.java:451-454.
            result = false;
        }
        result
    }

    // -- half widths, clearance, geometry --------------------------------------------------------

    /// Port of `BasicBoard.getMaxTraceHalfWidth` (BasicBoard.java:1118-1120).
    pub fn get_max_trace_half_width(&self) -> i32 {
        self.max_trace_half_width
    }

    /// Port of `BasicBoard.getMinTraceHalfWidth` (BasicBoard.java:1123-1125).
    pub fn get_min_trace_half_width(&self) -> i32 {
        self.min_trace_half_width
    }

    /// Port of `BasicBoard.clearanceValue(int, int, int)` (BasicBoard.java:1110-1115).
    ///
    /// Java's `rules == null || clearanceMatrix == null` guard returns 0; neither can be null in
    /// this port, so only the delegation remains.
    pub fn clearance_value(&self, class1: usize, class2: usize, layer: usize) -> i32 {
        self.rules
            .clearance_matrix
            .get_value(class1, class2, layer, true)
    }

    /// Port of `BoardRules.getTraceHalfWidth(int, int)`, reached through the board the way
    /// Java's callers do.
    pub fn get_trace_half_width(&self, net_number: i32, layer: usize) -> i32 {
        self.rules.get_trace_half_width(net_number, layer)
    }

    /// Port of `BasicBoard.getBoundingBox()` (BasicBoard.java:1128-1130).
    pub fn get_bounding_box(&self) -> IntBox {
        self.bounding_box
    }

    /// Port of the `BasicBoard.getBoundingBox(Collection<Item>)` overload
    /// (BasicBoard.java:1133-1139).
    // renamed: the collection overload -> get_bounding_box_of_items (Rust has no overloading).
    pub fn get_bounding_box_of_items(&self, ids: impl IntoIterator<Item = ItemId>) -> IntBox {
        let ctx = self.ctx();
        let mut result = IntBox::EMPTY;
        for id in ids {
            if let Some(item) = self.items.get(&id) {
                result = result.union(&item.bounding_box(&ctx));
            }
        }
        result
    }

    /// Port of `BasicBoard.contains(Point)` (BasicBoard.java:1102-1104).
    pub fn contains(&self, point: &Point) -> bool {
        point.is_contained_in(&self.bounding_box)
    }

    /// Java's `((DrillItem) item).getCenter()`, dispatched over the two drill variants.
    ///
    /// Not a Java method: `DrillItem.getCenter` (DrillItem.java:59-66) is one method there, but
    /// the port's `Via::get_center` needs no context and `Pin::get_center` does.
    pub fn drill_center(&self, id: ItemId) -> Option<Point> {
        let ctx = self.ctx();
        match self.items.get(&id)? {
            Item::Via(via) => Some(via.get_center()),
            Item::Pin(pin) => Some(pin.get_center(&ctx)),
            _ => None,
        }
    }

    /// Port of `ComponentObstacleArea.isFront` (ComponentObstacleArea.java:83-87):
    /// `component == null || component.placedOnFront()`.
    ///
    /// Java's expression reads as "`null` means front", but
    /// [`Components::get`](crate::structure::Components::get) *panics* out of range, faithfully
    /// to `Vector.elementAt` (quirk #49) — so on an item that belongs to no component Java
    /// throws rather than taking its own `null` branch. Verified on the JVM
    /// (`scripts/differential/java/P2T11.java` mode 6).
    // totalized: the bounds check lives here and answers the `component == null` value, `true`,
    // where Java throws `ArrayIndexOutOfBoundsException`. See docs/java-quirks.md.
    pub fn component_obstacle_area_is_front(&self, id: ItemId) -> bool {
        let Some(Item::ComponentObstacleArea(area)) = self.items.get(&id) else {
            return true;
        };
        let component_id = area.hdr.get_component_id();
        if component_id < 1 || component_id as usize > self.components.count() {
            // Java's `component == null` half.
            return true;
        }
        self.components.get(component_id).placed_on_front()
    }

    /// Port of `Item.componentName` (Item.java:346-355). `None` is Java's `null` for an item that
    /// belongs to no component.
    pub fn item_component_name(&self, id: ItemId) -> Option<&str> {
        let item = self.items.get(&id)?;
        let component_id = item.component_id();
        // Item.java:351-353.
        if component_id <= 0 {
            return None;
        }
        Some(&self.components.get(component_id).name)
    }

    // -- net queries (`rules/Net.java`'s board-walking methods) ------------------------------------

    /// Port of `Net.getTerminalItems` (Net.java:75-91): the connectable items of this net that
    /// are not routable, i.e. its pins and conduction areas.
    pub fn net_terminal_items(&self, net_number: i32) -> Vec<ItemId> {
        self.ids_where(|item| {
            item.as_connectable().is_some() && item.contains_net(net_number) && !item.is_routable()
        })
    }

    /// Port of `Net.getPins` (Net.java:94-110).
    pub fn net_pins(&self, net_number: i32) -> Vec<ItemId> {
        self.ids_where(|item| matches!(item, Item::Pin(_)) && item.contains_net(net_number))
    }

    /// Port of `Net.getItems` (Net.java:113-127): every item of the net, connectable or not.
    pub fn net_items(&self, net_number: i32) -> Vec<ItemId> {
        self.ids_where(|item| item.contains_net(net_number))
    }

    /// Port of `Net.getTraceLength` (Net.java:130-140). Note it walks
    /// `board.getConnectableItems(netNumber)`, not the whole item list.
    ///
    /// `fold(0.0, …)` rather than `.sum()`, for the reason
    /// [`Board::cumulative_trace_length`] gives: Java's `Net.getCumulativeTraceLength`
    /// (Net.java:131) is an accumulator loop starting at positive zero, and Rust's `Sum for f64`
    /// starts at `-0.0`. A net with no trace is the reachable case.
    pub fn net_trace_length(&self, net_number: i32) -> f64 {
        self.get_connectable_items(net_number)
            .into_iter()
            .filter_map(|id| match self.items.get(&id) {
                Some(Item::Trace(trace)) => Some(trace.get_length()),
                _ => None,
            })
            .fold(0.0, |result, length| result + length)
    }

    /// Port of `Net.getViaCount` (Net.java:143-152).
    pub fn net_via_count(&self, net_number: i32) -> usize {
        self.get_connectable_items(net_number)
            .into_iter()
            .filter(|id| matches!(self.items.get(id), Some(Item::Via(_))))
            .count()
    }

    /// Port of `Item.hasIgnoredNets` (Item.java:1241-1255).
    ///
    /// Java dereferences `nets.get(netNumber)` without a null check, so an item on a net number
    /// the net list does not know throws a `NullPointerException`; the `expect` reproduces that.
    pub fn has_ignored_nets(&self, id: ItemId) -> bool {
        let Some(item) = self.items.get(&id) else {
            return false;
        };
        item.net_nos().iter().any(|net_number| {
            let net = self.rules.nets.get(*net_number).expect(
                "Item.hasIgnoredNets: nets.get(netNumber) is null — Java throws a \
                 NullPointerException here too (Item.java:1244)",
            );
            self.rules
                .net_classes
                .get(net.get_net_class())
                .is_ignored_by_autorouter
        })
    }

    /// Port of `Item.getAllNets` (Item.java:1271-1281). Unlike `hasIgnoredNets`, this one *does*
    /// skip a net number the net list does not know (Item.java:1276).
    pub fn all_nets(&self, id: ItemId) -> Vec<i32> {
        let Some(item) = self.items.get(&id) else {
            return Vec::new();
        };
        item.net_nos()
            .iter()
            .filter(|net_number| self.rules.nets.get(**net_number).is_some())
            .copied()
            .collect()
    }

    /// Port of `Item.getAllNetNames` (Item.java:1283-1288): the net names joined with `,`, or
    /// the literal `"no nets"` when the item is on none.
    ///
    /// The elements are `Net::toString` (Net.java:54-56), i.e. `"Net #<n> (<name>)"` — not the
    /// bare name.
    pub fn all_net_names(&self, id: ItemId) -> String {
        let names: Vec<String> = self
            .all_nets(id)
            .into_iter()
            .filter_map(|net_number| self.rules.nets.get(net_number))
            .map(ToString::to_string)
            .collect();
        if names.is_empty() {
            return "no nets".to_string();
        }
        names.join(",")
    }

    // -- the changed area (RoutingBoardOperations, non-shove half) --------------------------------

    /// Port of `RoutingBoardOperations.startMarkingChangedArea`
    /// (RoutingBoardOperations.java:26-30).
    pub fn start_marking_changed_area(&mut self) {
        if self.changed_area.is_none() {
            self.changed_area = Some(ChangedArea::new(self.get_layer_count()));
        }
    }

    /// Port of `RoutingBoardOperations.joinChangedArea` (RoutingBoardOperations.java:32-36).
    pub fn join_changed_area(&mut self, point: &fr_geometry::FloatPoint, layer: usize) {
        if let Some(changed_area) = &mut self.changed_area {
            changed_area.join(point, layer);
        }
    }

    /// The `TileShape` half of the same call — Java writes `board.changedArea.join(shape, layer)`
    /// inline (e.g. RoutingBoardOperations.java:100).
    // renamed: `ChangedArea.join(TileShape, int)` reached through the board ->
    // `mark_changed_area` (task brief naming).
    pub fn mark_changed_area(&mut self, shape: &TileShape, layer: usize) {
        if let Some(changed_area) = &mut self.changed_area {
            changed_area.join_shape(shape, layer);
        }
    }

    /// Port of `RoutingBoardOperations.markAllChangedArea`
    /// (RoutingBoardOperations.java:38-50): every corner of the board's bounding box on every
    /// layer.
    pub fn mark_all_changed_area(&mut self) {
        self.start_marking_changed_area();
        let box_ = self.bounding_box;
        let corners = [
            fr_geometry::FloatPoint::new(f64::from(box_.ll.x), f64::from(box_.ll.y)),
            fr_geometry::FloatPoint::new(f64::from(box_.ur.x), f64::from(box_.ll.y)),
            fr_geometry::FloatPoint::new(f64::from(box_.ur.x), f64::from(box_.ur.y)),
            fr_geometry::FloatPoint::new(f64::from(box_.ll.x), f64::from(box_.ur.y)),
        ];
        for layer in 0..self.get_layer_count() {
            for corner in &corners {
                self.join_changed_area(corner, layer);
            }
        }
    }

    /// Re-sizes the changed area to the board's current layer count, discarding what was marked.
    ///
    /// Not a Java method; the task brief asks for it because Java re-creates the `ChangedArea`
    /// with `board.getLayerCount()` every time `startMarkingChangedArea` finds a `null`
    /// (RoutingBoardOperations.java:27-29), and a board whose layer count changed would otherwise
    /// keep an under-sized array.
    pub fn set_changed_area_layer_count(&mut self, layer_count: usize) {
        self.changed_area = Some(ChangedArea::new(layer_count));
    }

    /// Port of the **removal half** of `RoutingBoardOperations.removeItemsAndPullTight`
    /// (RoutingBoardOperations.java:81-110): mark every shape of every removable item into the
    /// changed area, remove it, and collect the nets that changed.
    ///
    /// Returns `(all_removed, changed_nets)`. Java's `tidyRegion` accumulation (:101-103) and the
    /// `combineTraces` + `optChangedArea` tail (:111-118) are Plan 7's, together with the
    /// `TraceTightener` they feed.
    // renamed: the removal half of `removeItemsAndPullTight` ->
    // `remove_items_marking_changed_area`; the pull-tight half **landed in Plan 7 Task 8** as
    // `fr_router::board_ext::RoutingBoardExt::remove_items_and_pull_tight`, which is where the
    // `renamed:` marker for the whole method now sits (module head, above).
    pub fn remove_items_marking_changed_area(
        &mut self,
        ids: impl IntoIterator<Item = ItemId>,
    ) -> (bool, std::collections::BTreeSet<i32>) {
        let mut result = true;
        let mut changed_nets = std::collections::BTreeSet::new();
        self.start_marking_changed_area();
        for id in ids {
            let Some(item) = self.items.get(&id) else {
                continue;
            };
            if item.is_deletion_forbidden(&self.rules) || item.is_user_fixed() {
                result = false;
                continue;
            }
            let ctx = self.ctx();
            let shape_layers: Vec<usize> = (0..item.tile_shape_count(&ctx))
                .map(|i| item.shape_layer(i, &ctx))
                .collect();
            let net_nos = item.net_nos().to_vec();
            let shapes: Vec<(TileShape, usize)> = shape_layers
                .into_iter()
                .enumerate()
                .filter_map(|(i, layer)| self.item_tile_shape(id, i).map(|shape| (shape, layer)))
                .collect();
            for (shape, layer) in shapes {
                self.mark_changed_area(&shape, layer);
            }
            self.remove_item(id);
            changed_nets.extend(net_nos);
        }
        (result, changed_nets)
    }

    // -- the shove-failure fields (RoutingBoard) ---------------------------------------------------

    /// Port of `RoutingBoard.getShoveFailingObstacle` (RoutingBoard.java:1359-1361).
    pub fn get_shove_failing_obstacle(&self) -> Option<ItemId> {
        self.shove_failing_obstacle
    }

    /// Port of `RoutingBoard.setShoveFailingObstacle` (RoutingBoard.java:1363-1365).
    pub fn set_shove_failing_obstacle(&mut self, id: Option<ItemId>) {
        self.shove_failing_obstacle = id;
    }

    /// Port of `RoutingBoard.getShoveFailingLayer` (RoutingBoard.java:1367-1369).
    pub fn get_shove_failing_layer(&self) -> i32 {
        self.shove_failing_layer
    }

    /// Port of `RoutingBoard.setShoveFailingLayer` (RoutingBoard.java:1371-1373).
    pub fn set_shove_failing_layer(&mut self, layer: i32) {
        self.shove_failing_layer = layer;
    }

    /// Port of the private `RoutingBoard.clearShoveFailingObstacle`
    /// (RoutingBoard.java:1375-1378).
    pub fn clear_shove_failing_obstacle(&mut self) {
        self.shove_failing_obstacle = None;
        self.shove_failing_layer = -1;
    }

    /// Port of `RoutingBoard.clearAllItemTemporaryAutorouteData`
    /// (RoutingBoard.java:1241-1250).
    pub fn clear_all_item_temporary_autoroute_data(&mut self) {
        for item in self.items.values_mut().rev() {
            item.clear_autoroute_info();
        }
    }

    // -- private helpers ---------------------------------------------------------------------------

    /// The ids of every item matching `predicate`, in `board.itemList` order (descending id).
    fn ids_where(&self, predicate: impl Fn(&Item) -> bool) -> Vec<ItemId> {
        self.items
            .iter()
            .rev()
            .filter(|(_, item)| predicate(item))
            .map(|(id, _)| *id)
            .collect()
    }
}

// ---- Plan 7 Task 8b: the level-7 item-id ledger -----------------------------------------------
//
// Instrumentation, not behaviour: `false` unless `P7T8B_IDS` is set in the environment, read once
// into a `LazyLock`, and its only caller is the `eprintln!` in [`Board::new_item_id`]. The Java
// side of the pair is `P6T1`'s `IdTracer`, the decorating `IdGenerator` that
// `scripts/differential/java/P6T1.java` swaps into `board.communication.idGenerator` under the
// same variable. Both write to **stderr**. Quirk #210.
fn p7t8b_ids_ledger() -> bool {
    static ON: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| std::env::var_os("P7T8B_IDS").is_some());
    *ON
}

/// `P7T8B_IDS_FROM` — the lowest item id the `ID` ledger prints, so a run can be narrowed to one
/// connection's allocations without carrying a quarter of a million lines of board load.
fn p7t8b_ids_from() -> u32 {
    static FROM: std::sync::LazyLock<u32> = std::sync::LazyLock::new(|| {
        std::env::var("P7T8B_IDS_FROM")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    });
    *FROM
}

/// `P7T8B_IDS_BT` — adds a captured backtrace to every printed `ID` line, the port's answer to
/// the Java `IdTracer`'s `StackWalker` signature. Expensive; use it with `P7T8B_IDS_FROM`.
fn p7t8b_ids_backtrace() -> bool {
    static ON: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| std::env::var_os("P7T8B_IDS_BT").is_some());
    *ON
}
// ---- end Plan 7 Task 8b -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// `Board` has to be shareable across threads: Plan 6's autorouter runs board copies in
    /// parallel (`global-constraints.md`).
    #[test]
    fn board_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Board>();
    }
}
