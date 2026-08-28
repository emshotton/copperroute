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

pub mod changed_area;
pub mod communication;
pub mod connectivity;
pub mod query;
pub mod shape_trace_entries;

use std::collections::BTreeMap;

use fr_geometry::{Area, IntBox, Point, Polyline, PolylineShapeRef, TileShape, Vector};

pub use changed_area::ChangedArea;
pub use communication::Communication;
pub use connectivity::StopConnectionOption;
pub use shape_trace_entries::ShapeTraceEntries;

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
/// not ported: `BasicBoard.itemList`'s `UndoableObjects` undo stack (BasicBoard.java:70) — the
/// port stores a plain `BTreeMap` (plan-rulings.md #1) and Plan 2's Task 12 replaces Java's
/// snapshot/undo/redo with `Board::clone`. See the `added in Task 12:` markers below.
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
// `board/facade/BoardSnapshotManager.java` and `RoutingBoardUndoFacade.java`) with `Board::clone`:
// added in Task 12: `BasicBoard.clone` (BasicBoard.java:158-161) — the deep copy.
// added in Task 12: `BasicBoard.getHash` (BasicBoard.java:164-166) — the board hash.
// added in Task 12: `BasicBoard.diffTraces` (BasicBoard.java:169-171) — the trace-id diff.
// added in Task 12: `BasicBoard.generateSnapshot` (BasicBoard.java:1290-1292) — the undo stack.
// added in Task 12: `BasicBoard.popSnapshot` (BasicBoard.java:1298-1300) — the undo stack.
// added in Task 12: `BasicBoard.undo` (BasicBoard.java:1233-1240) and its `RoutingBoard` override.
// added in Task 12: `BasicBoard.redo` (BasicBoard.java:1246-1253) and its `RoutingBoard` override.
// added in Task 12: the private `BasicBoard.applyUndoRedoSideEffects` (BasicBoard.java:1255-1287).
// added in Task 12: `RoutingBoard.deepCopy` (RoutingBoard.java:1418-1420).
//
// Trace normalisation is Task 9, which Plan 2 dispatches after this task:
// added in Task 9: `BasicBoard.combineTraces` (BasicBoard.java:683-706).
// added in Task 9: `BasicBoard.normalizeTraces` (BasicBoard.java:709-795).
// added in Task 9: `BasicBoard.normalizeAllTraces` (BasicBoard.java:798-885).
// added in Task 9: `BasicBoard.splitTraces` (BasicBoard.java:891-907).
//
// The autoroute engine is Plan 6:
// added in Plan 6: `BasicBoard.additionalUpdateAfterChange` (BasicBoard.java:1227) and its `RoutingBoard` override (RoutingBoard.java:96-118).
// added in Plan 6: `BasicBoard.areThereItemsOnInactiveLayer` (BasicBoard.java:1443-1462) — takes an `AutorouteControl`.
// added in Plan 6: `RoutingBoard.initAutoroute` (RoutingBoard.java:882-897).
// added in Plan 6: `RoutingBoard.finishAutoroute` (RoutingBoard.java:900-905).
// added in Plan 6: `RoutingBoard.autoroute` (RoutingBoard.java:911-971).
// added in Plan 6: `RoutingBoard.fanout` (RoutingBoard.java:978-1110).
// added in Plan 7: `RoutingBoard.optChangedArea` (both overloads, RoutingBoard.java:151-190) — its body is `RoutingBoardOperations.optChangedArea` (:52-79), which runs the `TraceTightener`.
// added in Plan 7: `RoutingBoard.removeItemsAndPullTight` (RoutingBoard.java:124-127) — the removal half is `Board::remove_items_marking_changed_area`; the `combineTraces` + `optChangedArea` tail is Plan 7's.
// added in Plan 7: `RoutingBoard.moveDrillItem` (RoutingBoard.java:252-295) — `DrillItemMover`.
// added in Plan 7: `RoutingBoard.forcedVia` (RoutingBoard.java:312-352) — `ForcedViaInserter`.
// added in Plan 7: `RoutingBoard.insertForcedTraceSegment` (RoutingBoard.java:361-402), `checkForcedTracePolyline` (:408-448) and `insertForcedTracePolyline` (:456-876) — the `TraceShover`.
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
    /// Java `RoutingBoard.failureLog` (RoutingBoard.java:64), an
    /// `autoroute.RoutingFailureLog`. Plan 6 owns that type; the field is a `Vec<String>` hook
    /// until then, as the task brief asks.
    // added in Plan 6: `autoroute.RoutingFailureLog`, the real element type.
    pub failure_log: Vec<String>,
    /// Java `RoutingBoard.shoveFailingObstacle` (RoutingBoard.java:72), as an id.
    pub shove_failing_obstacle: Option<ItemId>,
    /// Java `RoutingBoard.shoveFailingLayer` (RoutingBoard.java:73), initialised to `-1`.
    pub shove_failing_layer: i32,

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
        // ShapeSearchTree.java:916-920: `50000`, unless the board names a host CAD system.
        let max_tree_shape_width = if communication.host_cad_exists() {
            500.0 * communication.get_resolution(crate::structure::Unit::Mil)
        } else {
            crate::items::DEFAULT_MAX_TREE_SHAPE_WIDTH
        };
        let mut board = Board {
            items: BTreeMap::new(),
            components,
            rules,
            library,
            communication,
            bounding_box,
            trees: SearchTreeManager::new(),
            changed_area: None,
            failure_log: Vec::new(),
            shove_failing_obstacle: None,
            shove_failing_layer: -1,
            revision: 0,
            max_trace_half_width: 1000,
            min_trace_half_width: 10000,
            max_tree_shape_width,
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
    pub fn new_item_id(&mut self) -> ItemId {
        self.communication.id_gen.new_id()
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
        // BoardItemRepository.java:161.
        let ctx = item_ctx!(self);
        let inserted = self
            .items
            .get_mut(&id)
            .expect("Board::insert_item: just inserted");
        self.trees.insert(inserted, &ctx);
        // added in Plan 6: `board.additionalUpdateAfterChange(item)` (BoardItemRepository.java:165
        // -> RoutingBoard.java:96-118), which invalidates the autoroute expansion rooms.
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
        // added in Plan 6: `board.additionalUpdateAfterChange(item)`
        // (BoardItemRepository.java:192).
        // BoardItemRepository.java:193.
        let item = self
            .items
            .get_mut(&id)
            .expect("Board::remove_item: present, just checked");
        self.trees.remove(item);
        // BoardItemRepository.java:194.
        self.items.remove(&id);
        // BoardItemRepository.java:198.
        self.revision += 1;
        true
    }

    /// Port of `BasicBoard.removeItems` (BasicBoard.java:636-647) and the identical
    /// `BoardItemRepository.removeItems` (BoardItemRepository.java:202-212): removes every item
    /// it is allowed to, and reports whether all of them went.
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
        // added in Task 9: `newTrace.normalize(clipShape)` (BasicBoard.java:222-241), where
        // `clipShape` is `changedArea.getArea(layer)` when a changed area is being marked. Java
        // swallows a normalisation failure with a warning; Plan 2 ruling 10 makes it a
        // `BoardError::Normalization` instead.
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
    pub fn insert_via(
        &mut self,
        padstack: crate::ids::PadstackId,
        center: Point,
        net_nos: Vec<i32>,
        clearance_class: usize,
        fixed_state: FixedState,
        attach_allowed: bool,
    ) -> ItemId {
        let id = self.new_item_id();
        let via = Via::new(
            ItemHeader::new(id, net_nos, clearance_class, 0, fixed_state),
            padstack,
            center,
            attach_allowed,
        );
        self.insert_item(Item::Via(via))
        // added in Task 9: the `splitTraces(center, layer, netNumber)` loop over the padstack's
        // layer range (BasicBoard.java:287-293), which needs `Trace.split`.
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
    ) -> ItemId {
        let id = self.new_item_id();
        let mut via = Via::new(
            ItemHeader::new(id, net_nos, clearance_class, 0, fixed_state),
            padstack,
            center,
            true,
        );
        // BasicBoard.java:319-320.
        via.is_escape_via = true;
        via.escape_via_smd_layer = smd_layer as i32;
        self.insert_item(Item::Via(via))
        // added in Task 9: the `splitTraces` loop at BasicBoard.java:322-328.
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

    /// Port of `BasicBoard.cumulativeTraceLength` (BasicBoard.java:675-677).
    pub fn cumulative_trace_length(&self) -> f64 {
        self.items
            .values()
            .rev()
            .filter_map(|item| match item {
                Item::Trace(trace) => Some(trace.get_length()),
                _ => None,
            })
            .sum()
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
    /// (RoutingBoard.java:1252-1277), copied line for line — see quirk #50.
    //
    // Java bug: the guard at RoutingBoard.java:1254 is `if (getIgnoreConduction() != value)
    // return;`, so the method only does anything when the flag *already equals* the requested
    // value, and the store at :1273 writes `!value`, the negation of what it just pushed into
    // every conduction area. Together they make `rules.ignoreConduction` a latch that alternates
    // with the per-item flag rather than mirroring it. Both lines are reproduced verbatim; see
    // docs/java-quirks.md row 50.
    pub fn change_conduction_is_obstacle(&mut self, value: bool) {
        // RoutingBoard.java:1254-1256.
        if self.rules.get_ignore_conduction() != value {
            return;
        }
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
        // RoutingBoard.java:1273.
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
    /// Java's `board.itemList.saveForUndo(this)` (Item.java:301) and its observer notification
    /// (:307-310) are both dropped — no undo stack (Task 12) and no observers.
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
        let contact_trace_info: Vec<(usize, i32, usize)> = if is_drill_item {
            self.normal_contacts(id)
                .into_iter()
                .filter_map(|contact_id| match self.items.get(&contact_id) {
                    Some(Item::Trace(trace)) => Some((
                        trace.get_layer(),
                        trace.get_half_width(),
                        trace.hdr.clearance_class(),
                    )),
                    _ => None,
                })
                .collect()
        } else {
            Vec::new()
        };

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

    /// Port of `Item.validate` (Item.java:796-807) and its one override, `Trace.validate`
    /// (Trace.java:447-456), which additionally rejects a trace whose first and last corner are
    /// equal. Java's two `FRLogger.warn` calls are dropped.
    pub fn validate_item(&self, id: ItemId) -> bool {
        let Some(item) = self.items.get(&id) else {
            return true;
        };
        let ctx = self.ctx();
        let default_tree = self.default_tree_id();
        let mut result = self.trees.validate_entries(item);
        for i in 0..item.tile_shape_count(&ctx) {
            match item.get_tile_shape(default_tree, i, &ctx) {
                Some(shape) if !shape.is_empty() => {}
                // Java's `getTileShape(i).isEmpty()` NPEs on a `null` shape; a `None` here is the
                // same inconsistency the check exists to catch, so it counts as a failure.
                _ => result = false,
            }
        }
        if let Item::Trace(trace) = item
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
    /// Java's `board.components.get(componentId)` returns `null` for a component id it does not
    /// know — except that [`Components::get`](crate::structure::Components::get) *panics* out of
    /// range, faithfully to `Vector.elementAt` (quirk #49). The bounds check therefore lives
    /// here, and answers Java's `true` for a missing component.
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
    pub fn net_trace_length(&self, net_number: i32) -> f64 {
        self.get_connectable_items(net_number)
            .into_iter()
            .filter_map(|id| match self.items.get(&id) {
                Some(Item::Trace(trace)) => Some(trace.get_length()),
                _ => None,
            })
            .sum()
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
    /// The elements are `Net::toString`, which is `Net.name` (Net.java:161-164).
    pub fn all_net_names(&self, id: ItemId) -> String {
        let names: Vec<&str> = self
            .all_nets(id)
            .into_iter()
            .filter_map(|net_number| self.rules.nets.get(net_number))
            .map(|net| net.name.as_str())
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
    // `remove_items_marking_changed_area`; the pull-tight half is `added in Plan 7:` above.
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
            let default_tree = self.default_tree_id();
            let shapes: Vec<(TileShape, usize)> = (0..item.tile_shape_count(&ctx))
                .filter_map(|i| {
                    item.get_tile_shape(default_tree, i, &ctx)
                        .map(|shape| (shape, item.shape_layer(i, &ctx)))
                })
                .collect();
            let net_nos = item.net_nos().to_vec();
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
