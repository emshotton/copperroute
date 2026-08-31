//! Snapshots and the board hash — Task 12's replacement for Java's
//! `board/facade/{BoardSnapshotManager,RoutingBoardUndoFacade}.java`.
//!
//! # What Java's `clone`/`deepCopy` actually do
//!
//! `BasicBoard.clone` (BasicBoard.java:158-161) is `BoardSnapshotManager.deserialize(
//! getSnapshotManager().serialize(false))`: a full Java-serialization round trip of the whole
//! object graph. `RoutingBoard.deepCopy` (RoutingBoard.java:1414-1420) delegates to
//! `RoutingBoardUndoFacade.deepCopy`, which does the *same* round trip and then calls
//! `clearAllItemTemporaryAutorouteData()` and `finishAutoroute()` on the result. So
//! `BasicBoard.clone()`'s port is [`Board::deep_copy`] **minus** those last two steps — not the
//! derived [`Clone`] impl below, which does none of the resets the next section describes.
//!
//! `global-constraints.md` forbids `Serializable` in this port, so [`Board`] derives [`Clone`]
//! instead (`board/mod.rs`), but for a different job: a plain, field-for-field in-memory copy —
//! `ShapeTree`'s arena clones by value, so every [`crate::LeafId`] stays valid — that this task
//! substitutes for Java's `generateSnapshot`/`popSnapshot`/`undo`/`redo` (`board/mod.rs`'s
//! `not ported:` notes on those four; a `board.clone()` is what a Plan-7 caller takes before a
//! trial mutation it might have to revert). A derived `clone()` copies *every* field, including
//! the ones `readObject` resets, exactly as they stood; it is not a port of `BasicBoard.clone()`
//! at all.
//!
//! # The transient fields
//!
//! Java's `readObject` (BasicBoard.java:1388-1400, the hook every deserialization — hence every
//! `clone()`/`deepCopy()` — runs through) resets every field either class marks `transient`
//! rather than restoring it, because a `transient` field is never written to the stream to begin
//! with. Besides the search tree and `normalizeSuppressedNetNos` (`Board`'s struct doc; both
//! covered in the next section), that is:
//!
//! - `BasicBoard.revision` (BasicBoard.java:97) — comes back `0`, the same value a freshly
//!   constructed board starts at (Java's plain `int` default).
//! - `RoutingBoard.changedArea` (RoutingBoard.java:67) — comes back `null`.
//! - `RoutingBoard.shoveFailingObstacle` (RoutingBoard.java:72) — comes back `null`.
//! - `RoutingBoard.shoveFailingLayer` (RoutingBoard.java:73) — **Java bug:** its `= -1` field
//!   initializer is a declaration-site initializer, compiled into every constructor
//!   `RoutingBoard` has; deserialization calls none of them (the object is allocated directly and
//!   only `readObject` runs), so this `transient int` comes back at the language default, `0` —
//!   not the `-1` sentinel a freshly-built board starts with, so a cloned/deep-copied board
//!   disagrees with a fresh one about what "no failing layer yet" looks like. Reproduced
//!   (`docs/java-quirks.md`); [`Board::deep_copy`] sets it to `0` to match Java, not `-1`.
//!
//! [`Board::deep_copy`] resets all four explicitly: the port's `clone()` is an ordinary
//! `#[derive(Clone)]` with no `transient` concept, so left alone it would copy every one of them
//! as they stood on `self`. `changed_area` in particular is Plan 7's: `optChangedArea` early-
//! returns on a `null` one, `PolylineTrace.change` (:994-996) and `TraceShover` (:572) both
//! dereference it, and `deep_copy` runs once per autoroute pass/optimizer task — a stale
//! non-`None` `changed_area` surviving a copy would corrupt the next pass's bookkeeping.
//!
//! # The search-tree question
//!
//! Java's `readObject` (BasicBoard.java:1388-1400, the hook every deserialization — hence every
//! `clone`/`deepCopy` — runs through) does **not** restore the search trees from the serialized
//! stream: `searchTreeManager` is `transient` (BasicBoard.java:94), so `readObject` rebuilds it
//! from scratch and **reinserts every item**:
//!
//! ```text
//! searchTreeManager = new SearchTreeManager(this);
//! normalizeSuppressedNetNos = new HashSet<>();
//! ...
//! for (Item currentItem : this.getItems()) {
//!   currentItem.board = this;
//!   searchTreeManager.insert(currentItem);
//! }
//! ```
//!
//! `getItems()` walks `itemList`, a `ConcurrentSkipListMap` keyed by `Item.compareTo`, whose
//! subtraction is reversed (Item.java:98) — quirk #63, documented on this module's parent. So
//! Java's clone rebuilds the tree by reinserting items in **descending id order**, which is not
//! generally the order the *original* board's tree was built in (items are inserted into the
//! live tree as `insertItem` runs, i.e. roughly in creation order). The clone's tree can
//! therefore have a different physical shape (different `ShapeTree.toArray()` sequence) than the
//! board it was cloned from — verified in the JVM: `scripts/differential/java/P2T11.java` mode
//! 11 dumps `toArray()` of the default tree before and after `board.deepCopy()`, on a board built
//! with removes and reinserts along the way, and the two sequences differ.
//!
//! That would matter here only if the port's `#[derive(Clone)]` — which preserves the *original*
//! tree's exact shape and leaf ids, unlike Java's rebuild — were observably different from
//! rebuilding by reinsertion. It is not, for two reasons:
//!
//! 1. `ShapeTree.toArray()` (the only thing that exposes physical tree shape) has exactly one
//!    caller in the whole Java source: `ShapeTree.statistics`, a diagnostic log method this crate
//!    does not port (`global-constraints.md`: `FRLogger` calls are dropped). No routing or query
//!    logic ever reads raw tree order.
//! 2. Every real query — `overlappingObjects`, `pick_items`, the connectivity family — collects
//!    its result into a `TreeSet`/`BTreeSet` ordered by `(object id, shape index)`
//!    (`docs/superpowers/plans/2026-08-28-plan-2-board-model.md`'s global constraints), which
//!    canonicalizes the result independently of the tree's internal shape. Two trees holding the
//!    same set of leaves answer every such query identically no matter how they were built.
//!
//! So cloning the arena is behaviorally equivalent to Java's rebuild-by-reinsertion for every
//! caller this crate has, and is strictly *more* faithful to the pre-clone board than Java's own
//! clone is (Java's clone can itself diverge in shape from the board it copied). Rebuilding the
//! tree by reinserting items in descending id order — the alternative this module's authors
//! considered — would throw that fidelity away for a property (`toArray()` order) nothing
//! observes, so [`Board::deep_copy`] does not do it.
//!
//! # The hash — the ruling-AH audit
//!
//! `BasicBoard.getHash` (BasicBoard.java:164-166) delegates to `BoardSnapshotManager.getHash`
//! (:58-72), which MD5-hashes `serialize(true)` (:26-43) — the Java-serialized bytes of
//! `board.getTraces()`, `board.getVias()` **and `board.itemList`** (:29-35), in that order.
//!
//! Java bug: `BasicBoard.getHash`'s own javadoc (BasicBoard.java:163) says "an MD5 hash of the
//! board **trace** state", and `BoardSnapshotManager.getHash`'s (:57) says "the board trace-state
//! profile". Both are wrong: the third `writeObject` at :33 writes the whole `itemList`, so the
//! digest covers **every item**, with every non-`transient` field, transitively. That wrong
//! comment is exactly what justified this port's original trace-and-via-only hash (Plan 2), which
//! could not tell two trace-free boards apart at all. `docs/java-quirks.md` #201.
//!
//! Controller ruling AH: **do not** reproduce the bytes or the digest — widen this hash until it
//! covers the field set `serialize(true)` covers, and prove *decision* parity at the three sites
//! where Java compares two hashes (`autoroute/pipeline/BatchFanout.java:152-156`,
//! `autoroute/BoardHistory.java:88-101` `contains` and `:173-186` `getRank`). The driver that
//! proves it is `scripts/differential/run.sh p7t10`; [`Board::diff_traces`] is the tie-break where
//! only the port could collide.
//!
//! ## What `serialize(true)` can actually reach
//!
//! `Item.board` is `public transient BasicBoard board` (Item.java:45), so the stream does **not**
//! drag in the board, its components, its rules or its library: the reachable set is every
//! `Item`'s own non-`transient` fields, transitively. That is what the table below audits.
//!
//! ## The audit table (ruling AH's deliverable)
//!
//! One row per field `serialize(true)` reaches. "Covered" means a change to that field moves
//! [`Board::structural_hash`]; the same table is reproduced in `crates/fr-router/README.md`.
//!
//! | Java serialized field | reached via | port field | covered | test |
//! |---|---|---|---|---|
//! | `Item.id` (Item.java:42) | `itemList` | `ItemHeader::id` | yes | `the_item_id_reaches_the_hash` |
//! | `Item.netNumbers` (:53) | `itemList` | `ItemHeader::net_nos` | yes | `the_net_numbers_reach_the_hash` |
//! | `Item.clearanceClassIndex` (:56) | `itemList` | `ItemHeader::clearance_class` | yes | `the_clearance_class_reaches_the_hash` |
//! | `Item.fixedState` (:61) | `itemList` | `ItemHeader::fixed_state` | yes | `the_fixed_state_reaches_the_hash` |
//! | `Item.componentId` (:50) | `itemList` | `ItemHeader::component_id` | yes | `the_component_id_reaches_the_hash` |
//! | `Item.onTheBoard` (:64) | `itemList` | `ItemHeader::on_the_board` | yes | `the_on_the_board_flag_reaches_the_hash` |
//! | `Item.smallestClearance` (:47) | `itemList` | `ItemHeader::smallest_clearance` | **skipped** — a DRC by-product, see below | `the_smallest_clearance_by_product_does_not_move_the_hash` |
//! | the item's concrete class | `itemList` | [`crate::items::ItemKind`] | yes | `the_four_obstacle_area_kinds_are_distinguishable` |
//! | `Trace.layer` (Trace.java:31) | `getTraces()` | `PolylineTrace::layer` | yes | `the_trace_layer_reaches_the_hash` |
//! | `Trace.halfWidth` (Trace.java:30) | `getTraces()` | `PolylineTrace::half_width` | yes | `the_trace_half_width_reaches_the_hash` |
//! | `PolylineTrace.lines` → `Polyline.lines` → every `Line.a`, `Line.b` (Line.java:12-15) | `getTraces()` | `PolylineTrace::lines`, hashed **line by line** | yes | `two_polylines_with_equal_corners_but_different_lines_hash_differently` |
//! | `Line.dir` (Line.java:17) | — | — | not serialized (`transient`) | — |
//! | *`Line`'s identity token* (plan-6 ruling AE) | — | `Line::identity` | **must not be**, and is not: the fold goes through `Line`'s `Hash`, which is `a`/`b` only | `the_line_identity_token_does_not_reach_the_hash` |
//! | `Via.padstack` (Via.java:48) | `getVias()` | `Via::padstack` (a [`crate::ids::PadstackId`]) | yes | `the_via_centre_and_padstack_reach_the_hash` |
//! | `Via.attachAllowed` (:32) | `getVias()` | `Via::attach_allowed` | yes | `the_via_attach_allowed_flag_reaches_the_hash` |
//! | `Via.isEscapeVia` (:40), `Via.escapeViaSmdLayer` (:46) | `getVias()` | `Via::is_escape_via`, `Via::escape_via_smd_layer` | yes | `the_via_escape_flags_reach_the_hash` |
//! | `DrillItem.center` (DrillItem.java:28) — **a via's** | `getVias()` | `DrillItemData::center` | yes (a via is constructed with it, Via.java:65) | `the_via_centre_and_padstack_reach_the_hash` |
//! | `DrillItem.center` — **a pin's** | `itemList` | `DrillItemData::center` | **skipped** — quirk #200, see below | `filling_the_pin_centre_cache_does_not_move_the_hash` |
//! | `DrillItem.precalculatedMinWidth`/`…FirstLayer`/`…LastLayer` (:34-46) | `itemList` | the three `OnceLock`s | **skipped** — quirk #200, and pure functions of the padstack, which is covered | `filling_the_via_layer_caches_does_not_move_the_hash` |
//! | `Pin.pinIndex` (Pin.java:40) | `itemList` | `Pin::pin_index` | yes | `the_pin_index_reaches_the_hash` |
//! | `Pin.changedTo` (:43) | `itemList` | `Pin::changed_to` | yes | — (`Pin::swap` has no live headless caller; the field is folded in regardless) |
//! | `ObstacleArea.name` (:31), `.relativeArea` (:33), `.layer` (:36), `.translation` (:39), `.rotationInDegree` (:40), `.sideChanged` (:41) | `itemList` | `ObstacleAreaData`'s six | yes | `the_obstacle_area_geometry_and_layer_reach_the_hash`, `the_obstacle_area_placement_fields_reach_the_hash` |
//! | `ConductionArea.isObstacle` (:29), `.isFilled` (:30) | `itemList` | `ConductionArea::is_obstacle`, `::is_filled` | yes | `the_conduction_area_flags_reach_the_hash` |
//! | `ComponentOutline.relativeArea` (:24), `.translation` (:26), `.rotationInDegree` (:27), `.isFront` (:28), `.isCourtyard` (:29), `.isFabrication` (:30), `.isClosed` (:31) | `itemList` | the seven `ComponentOutline` fields (the area through its memoised **absolute** form — see the note below) | yes | `the_component_outline_fields_reach_the_hash` |
//! | `BoardOutline.shapes` (:30), `.keepoutOutsideOutline` (:43) | `itemList` | `BoardOutline::shapes`, `::keepout_outside_outline` | yes | `the_board_outline_shapes_and_keepout_flag_reach_the_hash` |
//! | `BoardOutline.keepoutArea` (:36), `.keepoutLines` (:41) | `itemList` | the `OnceLock`/`Option` | **skipped** — lazy caches, pure functions of `shapes` + `keepoutOutsideOutline`, both covered | — |
//! | `UndoableObjects.objects`' iteration order, `.stackLevel`, `.deletedObjectsStack`, `.redoPossible`, every `UndoableObjectNode.level`/`.undoObject`/`.redoObject` | `itemList` | — | **skipped** — the port has no undo stack, see below | — |
//! | `ObstacleArea.precalculatedAbsoluteArea` (:38), `ConductionArea.cachedBoard*` (:42-43), `Via`/`Pin.precalculatedShapes`, `Item.searchTreesInfo` (:59), `Item.autorouteInfo` (:67), `Item.board` (:45) | — | — | not serialized (`transient`) | — |
//!
//! ### One `covered` row with a caveat: `ComponentOutline`'s area
//!
//! `ComponentOutline` is the one variant whose *relative* area this crate does not expose; only
//! `get_area` (the memoised absolute form) is public, and adding a second accessor would be a
//! second `fr-board` API change this task does not want. `absolute_area_of` is a pure function of
//! the four serialized fields (`relativeArea`, `translation`, `rotationInDegree`, `isFront`) **and
//! one board-level input** — `components.flipStyleRotateFirst` — which `serialize(true)` cannot
//! reach at all (`Item.board` is `transient`). So this hash is very slightly *more* sensitive than
//! Java's here: two boards that differ only in that flag would hash differently in the port and
//! alike in the jar. Harmless, and deliberately not worked around: the flag is set once when the
//! board is built and never changes, and every `getHash` comparison the pipeline makes is between
//! two boards of one run. Filling the memo is likewise invisible — `ComponentOutline`'s
//! `PartialEq` skips it, so `structural_hash` cannot change what `==` answers.
//!
//! ### The three `skipped` rows, argued
//!
//! 1. **`DrillItem.center` for a pin, and the three `precalculated*` memos.** Java's fields are
//!    **not** `transient` and are filled **on demand** — `Pin.getCenter` (Pin.java:92-140) calls
//!    `setCenter` — so in the jar the *first call that asks a pin where it is* changes the board's
//!    hash without changing the board. Quirk **#200**, measured in Plan 7 Task 2: an unrouted
//!    `Issue143-rpi_splitter.dsn` hashes `c21982e8…`, and after one `new BoardStatistics(board)`
//!    `0da46bc9…`; and two boards with byte-identical item lists get different hashes because one
//!    of them ran a failed pass that populated more pin centres. Reproducing that would make
//!    `BoardHistory.contains` — a *membership* test — depend on how many times something has been
//!    measured. **The port does not reproduce it.** A pin's centre is a pure function of its
//!    `componentId` and `pinIndex` (both covered) plus the component's placement, which
//!    `serialize(true)` cannot reach anyway (`Item.board` is `transient`), and no headless caller
//!    moves a pin. The layer/min-width memos are pure functions of the padstack, which is covered.
//! 2. **`Item.smallestClearance`.** `public double`, not `transient`, so the digest sees it — but
//!    `Item.clearanceViolations` (Item.java:451-453) only ever *lowers* it, guarded by
//!    `smallestClearance < 0`, so its value records how many DRC checks have run over the item,
//!    not what the item is. Same shape as #200, same answer.
//! 3. **The undo bookkeeping.** `UndoableObjects` holds `stackLevel`, `redoPossible`, a
//!    `deletedObjectsStack` and a per-node `level`, all non-`transient`; this port has no undo
//!    stack at all (`generateSnapshot`/`popSnapshot`/`undo`/`redo` are `not ported:` on
//!    `board/mod.rs`; a `board.clone()` stands in). The one headless caller that moves them is
//!    `BatchOptimizer.optRouteItem`, which brackets one item's re-route with
//!    `generateSnapshot()` (BatchOptimizer.java:444) and either `popSnapshot()` (:503) or
//!    `undo(null)` (:508) — **balanced**, and no `getHash()` call sits inside that window
//!    (`BatchFanout`'s is in the fanout loop, which never snapshots; `BoardHistory`'s are
//!    per-pass). So the counters are 0 at every hash comparison the pipeline makes. And at
//!    `stackLevel == 0` the two writers are no-ops by construction: `UndoableObjects.saveForUndo`
//!    only builds an undo node when `currentNode.level < this.stackLevel`, and
//!    `UndoableObjects.insert` stamps the node with `stackLevel` itself — so outside the
//!    optimizer's window every `UndoableObjectNode` carries `level = 0` and
//!    `undoObject == redoObject == null`, and there is nothing for the port to be missing.
//!
//! ## What the port's fold is
//!
//! Not Java's *algorithm* (MD5 over a serialized byte stream) — a `u64` from
//! [`std::collections::hash_map::DefaultHasher`] (fixed keys, so it is deterministic within one
//! build) over the fields above, folded **item by item in descending item id**, which is the order
//! `board.itemList.startReadObject()` walks (quirk #63) and therefore the order Java's stream is
//! written in. The fold pairs each field set with its own id rather than combining commutatively,
//! so it is order-sensitive exactly where Java's byte stream is. `totalized:` in spirit: Java's
//! `getHash` returns `null` when the digest throws (BoardSnapshotManager.java:70) and
//! `BoardHistory.contains` then NPEs on `entry.hash.equals(hash)`; MD5 is always available, so
//! that branch is unreachable, and the port's `u64` has no `null` to reproduce.
//!
//! Hash *values* are still not comparable with Java's hex string — only the same-board equality
//! semantics `BoardHistory` and `BatchFanout` actually rely on. See `docs/java-quirks.md` #78 and
//! #201, and `scripts/differential/run.sh p7t10` for the decision-parity evidence.

use std::collections::BTreeSet;
use std::collections::hash_map::DefaultHasher;
use std::fmt::Write as _;
use std::hash::{Hash, Hasher};

use fr_geometry::{Area, PolylineShapeRef, Shape, Vector};

use crate::ids::ItemId;
use crate::items::{Item, ObstacleAreaData};

use super::Board;

/// A [`std::fmt::Write`] that feeds everything written to it straight into a [`Hasher`].
///
/// The fallback for the one geometry value [`Board::structural_hash`] can meet that implements
/// neither [`Hash`] nor a structural accessor pair: a [`Vector::Rational`]. `fr-geometry`'s
/// `BigInt` coordinates are `Clone`-only, and Plan 7 makes no `fr-geometry` API change, so the
/// derived `Debug` — a complete structural rendering — is folded in instead, allocating nothing.
/// Unreachable in practice: every `translation` on a corpus board is an `IntVector`.
struct HashWriter<'a, H: Hasher>(&'a mut H);

impl<H: Hasher> std::fmt::Write for HashWriter<'_, H> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.write(s.as_bytes());
        Ok(())
    }
}

/// Folds `value`'s `Debug` rendering into `hasher`, followed by a terminator so that two
/// renderings cannot run together into a third.
fn hash_debug<H: Hasher, T: std::fmt::Debug + ?Sized>(value: &T, hasher: &mut H) {
    write!(HashWriter(hasher), "{value:?}").expect("HashWriter never fails");
    0xffu8.hash(hasher);
}

/// `Vector`, which is not [`Hash`] because its rational arm holds `BigInt`s.
fn hash_vector<H: Hasher>(vector: &Vector, hasher: &mut H) {
    match vector {
        Vector::Int(v) => {
            0u8.hash(hasher);
            v.hash(hasher);
        }
        Vector::Rational(_) => {
            1u8.hash(hasher);
            hash_debug(vector, hasher);
        }
    }
}

/// `PolylineShape`'s two implementations (`PolylineShape.java`): a convex tile or a polygon.
fn hash_polyline_shape<H: Hasher>(shape: &PolylineShapeRef, hasher: &mut H) {
    match shape {
        PolylineShapeRef::Tile(tile) => {
            0u8.hash(hasher);
            tile.hash(hasher);
        }
        PolylineShapeRef::Polygon(polygon) => {
            1u8.hash(hasher);
            polygon.corners().hash(hasher);
        }
    }
}

/// `Shape`'s three implementations (`Shape.java`). `TileShape` and `Circle` are [`Hash`];
/// `PolygonShape` is not (its `Vec<Point>` has no derived `Eq`), so its corners are hashed
/// directly — every one of which is a [`Hash`] `Point`.
fn hash_shape<H: Hasher>(shape: &Shape, hasher: &mut H) {
    match shape {
        Shape::Tile(tile) => {
            0u8.hash(hasher);
            tile.hash(hasher);
        }
        Shape::Polygon(polygon) => {
            1u8.hash(hasher);
            polygon.corners().hash(hasher);
        }
        Shape::Circle(circle) => {
            2u8.hash(hasher);
            circle.hash(hasher);
        }
    }
}

/// `Area`'s two implementations (`Area.java`): a hole-free `Shape`, or a `PolylineArea`'s border
/// plus its holes. Structural, so that no part of the geometry is outside the fold.
fn hash_area<H: Hasher>(area: &Area, hasher: &mut H) {
    match area {
        Area::Shape(shape) => {
            0u8.hash(hasher);
            hash_shape(shape, hasher);
        }
        Area::Polyline(polyline_area) => {
            1u8.hash(hasher);
            hash_polyline_shape(polyline_area.get_border(), hasher);
            polyline_area.get_holes().len().hash(hasher);
            for hole in polyline_area.get_holes() {
                hash_polyline_shape(hole, hasher);
            }
        }
    }
}

/// The six non-`transient` `ObstacleArea` fields (ObstacleArea.java:31-41), shared by all four
/// area kinds. `precalculatedAbsoluteArea` (:38) is `transient` and absent from Java's stream, so
/// this hashes the **relative** area — exactly the field `serialize(true)` writes.
fn hash_obstacle_area<H: Hasher>(area: &ObstacleAreaData, hasher: &mut H) {
    area.name().hash(hasher);
    hash_area(area.get_relative_area(), hasher);
    area.get_layer().hash(hasher);
    hash_vector(area.get_translation(), hasher);
    area.get_rotation_in_degree().to_bits().hash(hasher);
    area.get_side_changed().hash(hasher);
}

impl Board {
    /// Port of `BasicBoard.clone` (BasicBoard.java:158-161), `RoutingBoard.deepCopy`
    /// (RoutingBoard.java:1414-1420) and `RoutingBoardUndoFacade.deepCopy`
    /// (RoutingBoardUndoFacade.java:45-63) in one method — the module doc explains why the two
    /// Java methods share almost this whole body (the same `readObject`) and differ only in the
    /// last two steps.
    ///
    /// `self.clone()` (the derived `impl Clone for Board`) copies every field as-is, including
    /// the ones Java's `readObject` resets; this method is `clone()` plus those resets — the
    /// search tree is handled by the clone itself (module doc, "The search-tree question"),
    /// `normalize_suppressed_net_nos`/`revision`/`changed_area`/`shove_failing_obstacle`/
    /// `shove_failing_layer` are reset explicitly below (module doc, "The transient fields") —
    /// plus the two things `RoutingBoardUndoFacade.deepCopy` adds on top of the plain
    /// `BasicBoard.clone()` round trip: `clearAllItemTemporaryAutorouteData()`
    /// (`Self::clear_autoroute_scratch`) and `finishAutoroute()` (`Self::finish_autoroute`).
    pub fn deep_copy(&self) -> Board {
        let mut copy = self.clone();

        // Every field Java's `readObject` (BasicBoard.java:1388-1400) resets rather than
        // restores from the stream, because none of them survive Java serialization
        // (`transient`) — module doc, "The transient fields".
        copy.normalize_suppressed_net_nos.clear();
        copy.revision = 0; // BasicBoard.java:97.
        copy.changed_area = None; // RoutingBoard.java:67.
        copy.shove_failing_obstacle = None; // RoutingBoard.java:72.
        // Java bug: the language default, not the `-1` sentinel — module doc's
        // `shoveFailingLayer` entry has the full argument.
        copy.shove_failing_layer = 0; // RoutingBoard.java:73.

        copy.clear_autoroute_scratch();
        copy.finish_autoroute();
        copy
    }

    /// Port of `RoutingBoard.clearAllItemTemporaryAutorouteData` (RoutingBoard.java:1240-1249):
    /// clears every item's `autorouteInfo`.
    ///
    /// Java walks `itemList` through `startReadObject`/`readObject`, its undo-stack-aware
    /// iterator; the port's items live in a plain `BTreeMap`, so a direct `values_mut` walk reads
    /// (and mutates) every one, which is the same set Java's iterator produces.
    // renamed: RoutingBoard.clearAllItemTemporaryAutorouteData -> Board::clear_autoroute_scratch
    // (this task's brief names it).
    fn clear_autoroute_scratch(&mut self) {
        for item in self.items.values_mut() {
            item.clear_autoroute_info();
        }
    }

    /// Port of `RoutingBoard.finishAutoroute` (RoutingBoard.java:899-905): "clears the auto-route
    /// database in case it was retained" by clearing `autorouteEngine`.
    ///
    /// Empty, and it stays empty: plan-6 ruling 3 puts the engine outside `Board` (it lives in
    /// `fr-router`, which `fr-board` cannot name), so the real `clear(); autorouteEngine = null`
    /// is `fr_router::board_ext::RoutingBoardExt::finish_autoroute`, which consumes the engine
    /// value its caller holds. This hook stays because [`Board::deep_copy`] calls it at Java's
    /// line, and on a board with no engine field there is nothing left for it to do.
    // renamed: `RoutingBoard.finishAutoroute`'s engine half (RoutingBoard.java:901-904) -> `fr_router::board_ext::RoutingBoardExt::finish_autoroute`.
    fn finish_autoroute(&mut self) {}

    /// Port of `BasicBoard.getHash` (BasicBoard.java:164-166) / `BoardSnapshotManager.getHash`
    /// (:58-72), which is an **MD5 hex string over `serialize(true)`** (:26-43).
    ///
    /// `serialize(true)` writes `board.getTraces()`, `board.getVias()` **and `board.itemList`**
    /// (:29-35) through Java object serialization — i.e. the whole item graph, not just the traces
    /// the method's own comment claims (`Java bug:` on that comment, module doc and
    /// `docs/java-quirks.md` #201). Controller ruling AH: **do not** reproduce the bytes or the
    /// digest; cover the field set that serialization covers, and prove *decision* parity at the
    /// three sites where Java compares two hashes. The field-by-field audit — including the three
    /// fields deliberately left out and why — is the table in this module's docs; the decision
    /// parity is `scripts/differential/run.sh p7t10`.
    ///
    /// Deterministic within one build (a `u64` from
    /// [`std::collections::hash_map::DefaultHasher`], whose keys are fixed), and **not**
    /// comparable with Java's hex string by value: only the same-board equality semantics
    /// `BoardHistory.contains`/`getRank` and `BatchFanout:152-156` rely on are preserved.
    ///
    /// Descending item id, which is what `board.itemList.startReadObject()` walks (quirk #63) and
    /// therefore the order Java's stream is written in; the fold is not commutative, so it is
    /// order-sensitive exactly where Java's byte stream is.
    pub fn structural_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        let ctx = self.ctx();
        // The stream's own length, so a board that is a strict prefix of another cannot collide
        // with it — Java gets that from the serialized collection headers.
        self.items.len().hash(&mut hasher);
        // `board.itemList.startReadObject()` order (quirk #63), i.e. descending id.
        for (id, item) in self.items.iter().rev() {
            id.hash(&mut hasher);
            // The concrete class, which Java's stream carries as the object's class descriptor.
            item.kind().hash(&mut hasher);
            // -- `Item`'s own non-transient fields (Item.java:41-67); `smallestClearance` (:47)
            // is deliberately absent, module doc's skipped row 2.
            item.net_nos().hash(&mut hasher);
            item.clearance_class().hash(&mut hasher);
            item.get_fixed_state().hash(&mut hasher);
            item.component_id().hash(&mut hasher);
            item.is_on_the_board().hash(&mut hasher);
            match item {
                Item::Trace(trace) => {
                    trace.get_layer().hash(&mut hasher);
                    trace.get_half_width().hash(&mut hasher);
                    // `Polyline.lines`, not `corners()`: Java serializes every `Line`'s `a` and
                    // `b` (Line.java:12-15), and two polylines can share their corners while
                    // their defining end points differ. `Line`'s `Hash` is `a`/`b` only, so the
                    // identity token (plan-6 ruling AE) cannot reach this.
                    trace.polyline().hash(&mut hasher);
                }
                Item::Via(via) => {
                    // A via is always constructed with its centre (Via.java:65), so this is real
                    // state, not the lazily filled cache the module doc's skipped row 1 is about.
                    via.get_center().hash(&mut hasher);
                    // The padstack id stands in for Java's serialized `Padstack` object, and
                    // determines the layer span the three `precalculated*` memos hold.
                    via.get_padstack_id().hash(&mut hasher);
                    via.attach_allowed.hash(&mut hasher);
                    via.is_escape_via.hash(&mut hasher);
                    via.escape_via_smd_layer.hash(&mut hasher);
                }
                Item::Pin(pin) => {
                    pin.get_pin_index().hash(&mut hasher);
                    pin.get_changed_to().hash(&mut hasher);
                    // `DrillItem.center` is **not** hashed here: module doc's skipped row 1
                    // (quirk #200). `component_id` above and `pin_index` here determine it.
                }
                Item::ObstacleArea(area) => hash_obstacle_area(&area.area, &mut hasher),
                Item::ViaObstacleArea(area) => hash_obstacle_area(&area.area, &mut hasher),
                Item::ComponentObstacleArea(area) => hash_obstacle_area(&area.area, &mut hasher),
                Item::ConductionArea(area) => {
                    hash_obstacle_area(&area.area, &mut hasher);
                    area.get_is_obstacle().hash(&mut hasher);
                    area.get_is_filled().hash(&mut hasher);
                }
                Item::ComponentOutline(outline) => {
                    // The memoised absolute area is a pure function of Java's serialized
                    // `relativeArea`, `translation`, `rotationInDegree` and `isFront`, and is the
                    // only form this crate exposes; the other three flags follow.
                    hash_area(outline.get_area(&ctx), &mut hasher);
                    hash_vector(outline.get_translation(), &mut hasher);
                    outline.get_rotation_in_degree().to_bits().hash(&mut hasher);
                    outline.is_front().hash(&mut hasher);
                    outline.is_courtyard().hash(&mut hasher);
                    outline.is_fabrication().hash(&mut hasher);
                    outline.is_closed().hash(&mut hasher);
                }
                Item::BoardOutline(outline) => {
                    outline.shape_count().hash(&mut hasher);
                    for index in 0..outline.shape_count() {
                        match outline.get_shape(index) {
                            Some(shape) => hash_polyline_shape(shape, &mut hasher),
                            // Unreachable: `index` comes from `shape_count()`.
                            None => 0xfeu8.hash(&mut hasher),
                        }
                    }
                    outline
                        .keepout_outside_outline_generated()
                        .hash(&mut hasher);
                    // `keepoutArea`/`keepoutLines` are lazy caches derived from those two —
                    // module doc's skipped row list.
                }
            }
        }
        hasher.finish()
    }

    /// Port of `BasicBoard.diffTraces` (BasicBoard.java:169-171) / `BoardSnapshotManager.diffTraces`
    /// (:86-100): the number of trace ids that appear in exactly one of the two boards — the
    /// symmetric difference of the two id sets.
    ///
    /// Ruling AH makes this the **tie-break** where two distinct boards could collide only in the
    /// port, so Plan 7 Task 3 re-audited it line by line against `:86-100` and **changed nothing**.
    /// Java builds a `HashSet<Integer>` of `board`'s trace ids, then walks `compareTo`'s
    /// incrementing on a miss and *removing* on a hit, and finally adds what is left; the
    /// `BTreeSet::remove` below is both halves of that test in one call, because it answers
    /// `false` exactly when `contains` would. The two line-number citations were five and one
    /// lines stale and are corrected above; the body is Plan 2's, untouched.
    pub fn diff_traces(&self, compare_to: &Board) -> usize {
        let mut trace_ids: BTreeSet<ItemId> = self.get_traces().into_iter().collect();
        let mut result = 0usize;
        for id in compare_to.get_traces() {
            if !trace_ids.remove(&id) {
                result += 1;
            }
        }
        result + trace_ids.len()
    }
}

#[cfg(test)]
mod tests {
    use fr_geometry::{IntBox, Point, Polyline, PolylineShapeRef, TileShape};

    use crate::ids::ItemId;
    use crate::library::{BoardLibrary, Packages, Padstacks};
    use crate::rules::{BoardRules, ClearanceMatrix};
    use crate::structure::{Components, FixedState, Layer, LayerStructure};
    use crate::{Board, Communication};

    fn board() -> Board {
        let layers = LayerStructure::new(vec![Layer::new("Top", true)]);
        let clearance_matrix = ClearanceMatrix::get_default_instance(&layers, 10);
        let mut rules = BoardRules::new(layers.clone(), clearance_matrix);
        rules.create_default_net_class();
        let outline = vec![PolylineShapeRef::Tile(TileShape::Box(IntBox::from_coords(
            0, 0, 1000, 1000,
        )))];
        Board::new(
            outline,
            0,
            IntBox::from_coords(0, 0, 1000, 1000),
            rules,
            BoardLibrary::new(Padstacks::new(layers), Packages::new()),
            Components::new(),
            Communication::default(),
        )
    }

    fn insert_trace(board: &mut Board, net_number: i32, x1: i32, x2: i32) -> ItemId {
        board
            .insert_trace_without_cleaning(
                Polyline::from_points(&[Point::new(x1, 100), Point::new(x2, 100)]),
                0,
                10,
                vec![net_number],
                0,
                FixedState::Unfixed,
            )
            .expect("a straight two-corner trace")
    }

    #[test]
    fn deep_copy_is_independent() {
        let mut board = board();
        insert_trace(&mut board, 1, 100, 500);
        let mut copy = board.deep_copy();

        // A full `PartialEq` would fail here: `deep_copy` deliberately resets the transient
        // bookkeeping the module doc's "The transient fields" section describes (`revision` in
        // particular has already advanced past the two inserts above), so the items and the
        // structural hash are what should agree.
        assert_eq!(copy.items, board.items);
        assert_eq!(copy.structural_hash(), board.structural_hash());

        let trace = board.get_traces()[0];
        copy.remove_item(trace);

        assert_ne!(copy, board);
        assert!(board.get_item(trace).is_some());
        assert!(copy.get_item(trace).is_none());

        // The copy's tree is its own: a query the original still answers with the trace, the
        // copy no longer does (the outline's own tile shape may still overlap the probe, so the
        // assertion checks for the trace specifically rather than emptiness).
        let ctx = board.ctx();
        let shape = board
            .get_item(trace)
            .expect("the trace")
            .get_tile_shape(board.default_tree_id(), 0, &ctx)
            .expect("its tile shape");
        let object = crate::ids::TreeObject::Item(trace);
        assert!(board.overlapping_objects(&shape, Some(0)).contains(&object));
        assert!(!copy.overlapping_objects(&shape, Some(0)).contains(&object));
    }

    #[test]
    fn deep_copy_clears_autoroute_scratch() {
        let mut board = board();
        let trace = insert_trace(&mut board, 1, 100, 500);
        board
            .get_item_mut(trace)
            .expect("the trace")
            .get_autoroute_info();
        assert!(
            board
                .get_item(trace)
                .expect("the trace")
                .get_autoroute_info_pur()
                .is_some()
        );

        let copy = board.deep_copy();
        assert!(
            copy.get_item(trace)
                .expect("the trace")
                .get_autoroute_info_pur()
                .is_none()
        );
        // `clone()` alone does not clear it — only `deep_copy` does.
        assert!(
            board
                .get_item(trace)
                .expect("the trace")
                .get_autoroute_info_pur()
                .is_some()
        );
    }

    #[test]
    fn deep_copy_clears_normalize_suppressed_net_nos() {
        let mut board = board();
        board.normalize_suppressed_net_nos.insert(3);
        let copy = board.deep_copy();
        assert!(copy.normalize_suppressed_net_nos.is_empty());
        assert!(board.normalize_suppressed_net_nos.contains(&3));
    }

    #[test]
    fn deep_copy_resets_transient_bookkeeping() {
        // Module doc, "The transient fields": `revision`, `changed_area`,
        // `shove_failing_obstacle` and `shove_failing_layer` are all Java `transient` fields
        // `readObject` resets rather than restores, so `deep_copy` must reset them explicitly —
        // `self.clone()` alone would carry every one of these over unchanged.
        let mut board = board();
        let trace = insert_trace(&mut board, 1, 100, 500);
        assert_ne!(
            board.revision(),
            0,
            "the two inserts above must have advanced it"
        );

        board.start_marking_changed_area();
        assert!(board.changed_area.is_some());
        board.shove_failing_obstacle = Some(trace);
        board.shove_failing_layer = 3;

        let copy = board.deep_copy();

        assert_eq!(copy.revision(), 0);
        assert!(copy.changed_area.is_none());
        assert!(copy.shove_failing_obstacle.is_none());
        // Java bug (module doc): the reset value is `0`, the `int` default — not the `-1`
        // sentinel a freshly constructed board starts with — because deserialization never runs
        // `shoveFailingLayer`'s `= -1` field initializer.
        assert_eq!(copy.shove_failing_layer, 0);

        // `self` is untouched: `deep_copy` must not mutate the board it is called on.
        assert_ne!(board.revision(), 0);
        assert!(board.changed_area.is_some());
        assert_eq!(board.shove_failing_obstacle, Some(trace));
        assert_eq!(board.shove_failing_layer, 3);
    }

    #[test]
    fn hash_stable_across_clone() {
        let mut board = board();
        insert_trace(&mut board, 1, 100, 500);
        let clone = board.clone();
        assert_eq!(board.structural_hash(), clone.structural_hash());
        let copy = board.deep_copy();
        assert_eq!(board.structural_hash(), copy.structural_hash());
    }

    #[test]
    fn hash_equal_for_equal_boards_and_differs_after_trace_change() {
        let mut board_a = board();
        insert_trace(&mut board_a, 1, 100, 500);
        let mut board_b = board();
        insert_trace(&mut board_b, 1, 100, 500);
        assert_eq!(board_a.structural_hash(), board_b.structural_hash());

        insert_trace(&mut board_b, 2, 600, 900);
        assert_ne!(board_a.structural_hash(), board_b.structural_hash());
    }

    #[test]
    fn diff_traces_counts_ids_present_in_exactly_one_board() {
        let mut board_a = board();
        insert_trace(&mut board_a, 1, 100, 500);
        let trace_b = insert_trace(&mut board_a, 2, 600, 900);

        let mut board_b = board_a.clone();
        assert_eq!(board_a.diff_traces(&board_b), 0);

        board_b.remove_item(trace_b);
        assert_eq!(board_a.diff_traces(&board_b), 1);
        assert_eq!(board_b.diff_traces(&board_a), 1);

        insert_trace(&mut board_b, 3, 200, 300);
        assert_eq!(board_a.diff_traces(&board_b), 2);
    }
}
