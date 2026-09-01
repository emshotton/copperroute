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
//! substitutes for the *state* half of Java's `generateSnapshot`/`undo` (a `board.clone()` is
//! what a Plan-7 caller takes before a trial mutation it might have to revert). **Plan 7 Task 14c
//! added the other half**: [`Board::begin_undo_journal`], [`Board::discard_undo_journal`] and
//! [`Board::undo_from_snapshot`] port `BasicBoard.{generateSnapshot, popSnapshot, undo}` and
//! `applyUndoRedoSideEffects` at the one level `BatchOptimizer.optRouteItem` opens, because the
//! *side effects* of `undo` — which items leave and re-enter the live search trees, and in what
//! order — cannot be reconstructed from a clone, and the tree's shape is a function of them
//! (quirk #229). A derived `clone()` copies *every* field, including
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
//! `board.getTraces()`, `board.getVias()` **and `board.itemList`** — the three `writeObject` calls
//! at :31-33 — in that order.
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
//! The reachable set is every `Item`'s own non-`transient` fields, **transitively** — and the
//! transitive closure escapes the item graph exactly once, so the two halves have to be stated
//! separately:
//!
//! * **What is not reached.** `Item.board` is `public transient BasicBoard board` (Item.java:45),
//!   so the stream does not drag in `BasicBoard` itself, and with it neither `board.components`
//!   nor `board.rules` nor `board.library.packages`. Nothing else points at them either: a `Pin`
//!   knows its component only as an `int` `componentId` (Item.java:50), and no item field is
//!   typed `Component`, `BoardRules`, `Net` or `Package`.
//! * **What *is* reached, and is not obvious.** `Via.padstack` is `private Padstack padstack`
//!   (Via.java:48) — **not** `transient` — and `Padstack implements Serializable`
//!   (Padstack.java:16) holding `private final Padstacks padstackList` (:33), which is itself
//!   `Serializable` (Padstacks.java:10) and holds `public final LayerStructure boardLayerStructure`
//!   (:13) and the `Vector<Padstack>` of **every** padstack on the board (:16);
//!   `LayerStructure` is `Serializable` too (LayerStructure.java:6), as is each `Layer` (:6).
//!   So on any board carrying one via, `serialize(true)` writes the whole padstack library and the
//!   layer structure. That subgraph is audited in its own rows below, and it is where the fourth
//!   skipped `#200`-shaped cache lives.
//!
//! With those two statements the closure is finite, and that is what the table below audits.
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
//! | *`Line`'s identity token* (plan-6 ruling AE) | — | `Line::identity` | **must not be**, and is not: the fold goes through `Line`'s `Hash`, which is `a`/`b` only | `the_line_identity_token_does_not_reach_the_hash` |
//! | `Via.padstack` (Via.java:48) | `getVias()` | `Via::padstack` (a [`crate::ids::PadstackId`]) | yes, and it is what covers the whole padstack subgraph below | `the_via_centre_and_padstack_reach_the_hash` |
//! | `Padstack.{name, id, attachAllowed, placedAbsolute, shapes, holeOnly}` (Padstack.java:18, 19, 22, 28, 30, 41) | `Via.padstack` | [`crate::library::Padstack`]'s six | **covered by reduction** — see below | `the_via_centre_and_padstack_reach_the_hash` (the `smd`/`thru` pair) |
//! | `Padstack.padstackList` (:33) → `Padstacks.{boardLayerStructure, padstacks}` (Padstacks.java:13, 16) → `LayerStructure.layers` (LayerStructure.java:8) → `Layer.{name, isSignal}` (Layer.java:9, 15) | `Via.padstack` | `Board::library.padstacks`, `BoardRules::layer_structure` | **covered by reduction** — see below | — |
//! | `Padstack.cachedDrillRadius` (:44) | `Via.padstack` | — | **skipped** — quirk #200's shape again, and **global**, see below | — |
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
//! | `IntOctagon.precalculatedToSimplex` (IntOctagon.java:54) | every `relativeArea`/`shapes` row above, and `Padstack.shapes` | — | **skipped** — the one non-`transient` lazy cache in `geometry/planar`, see below | — |
//! | `Item.board` (:45), `Item.searchTreesInfo` (:59), `Item.autorouteInfo` (:67), `Via.precalculatedShapes` (Via.java:49), `Via.autorouteDrillInfo` (:52), `Pin.precalculatedShapes` (Pin.java:45), `ObstacleArea.precalculatedAbsoluteArea` (:38), `ConductionArea.{cachedBoardRevision, cachedBoardFillArea}` (:42-43), `ComponentOutline.precalculatedAbsoluteArea` (:25), `Line.dir` (Line.java:17), `Polyline`'s three (Polyline.java:24-26), `Simplex`'s (Simplex.java:22-26), `PolygonShape`'s (PolygonShape.java:22-25), `PolylineArea`'s (PolylineArea.java:19) | — | — | not serialized (`transient`) — **this row is exhaustive**: it is every `transient` field the closure above can touch | — |
//!
//! ### The padstack subgraph: `covered by reduction`, argued
//!
//! The port stores a via's padstack as a [`crate::ids::PadstackId`] (the Plan 2 "no object
//! references between model objects" rule) and keeps the `Padstacks` table in `Board::library`,
//! which the fold does not walk. That is **not** a gap, for a reason that has to be stated rather
//! than assumed: within one board the id **determines** the whole subgraph.
//!
//! * `Padstacks.padstacks` is append-only — `Padstacks.add` (Padstacks.java:54-59) is its only
//!   writer, and nothing removes or rewrites an entry, so index *i* names the same padstack for
//!   the life of the board.
//! * Every `Padstack` field the digest reaches is `final` (`name`, `id`, `attachAllowed`,
//!   `placedAbsolute`, `shapes`, `padstackList`) **except two**, and neither moves: `holeOnly`
//!   (:41) has **no writer at all** in `src/main` — `ShapeSearchTree.java:1049` is its one reader
//!   and nothing ever assigns it, so it is `false` on every board the jar builds, which is
//!   exactly what `Padstack::new` hard-codes here; and `cachedDrillRadius` is the skipped row
//!   below.
//! * `Padstacks.boardLayerStructure`, `LayerStructure.layers` and each `Layer`'s two fields are
//!   `final` and fixed when the board is built.
//!
//! So for the comparison ruling AH's three sites actually make — two boards **of one run**, which
//! share one `Padstacks` object — "same padstack id" and "same serialized padstack subgraph" are
//! the same statement, and hashing the id is hashing the subgraph. What the reduction gives up is
//! only the cross-*library* case: two boards built from different DSNs whose padstack tables differ
//! at the same index would collide in the port where the jar tells them apart. No reader can
//! produce that pair (`BoardHistory` and `BatchFanout` both compare boards of one run), and
//! `Board::diff_traces` is ruling AH's tie-break if one ever appears.
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
//! two boards of one run.
//!
//! **[`Board::structural_hash`] is itself one of the fillers**, and that is worth stating rather
//! than leaving implicit: the `ComponentOutline` arm calls `outline.get_area(&ctx)`, which is
//! `self.absolute_area.get_or_init(..)` (`items/area.rs`), so taking a hash writes a
//! [`std::sync::OnceLock`] through `&self`. It is the only such write in the fold, it is
//! idempotent, and it is invisible — `ComponentOutline`'s `PartialEq` skips that field, so
//! `structural_hash` cannot change what `==` answers.
//!
//! **The invariant that keeps all of this true, stated once so a later edit has to break it
//! deliberately: every field this fold reads is also compared by the corresponding `PartialEq`,
//! and `ComponentOutline`'s absolute area is the one recorded exception** (it hashes the derived
//! form where `PartialEq` compares the relative one). So `a == b` implies equal hashes — the
//! direction a `#[derive(Hash)]` or a `HashMap` key would depend on — while the converse is only
//! what a hash is. Neither [`Board`] nor `ItemHeader` implements [`Hash`] today; if one ever
//! does, this paragraph is the thing to re-check first.
//!
//! ### The five `skipped` rows, argued
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
//! 2. **`Padstack.cachedDrillRadius`** (Padstack.java:44). `private Double`, **not** `transient`,
//!    written lazily by `getDrillRadius` (:99, :110) and read at :73 — a third instance of exactly
//!    the #200 shape, and in principle the **worst** of them, because a `Padstack` is *shared*:
//!    filling it anywhere would move the digest of every board holding a via on that padstack, in
//!    every history entry at once. Skipped, and it is a memo over a regex parse of the padstack's
//!    own `final` `name`, which the reduction above already covers, so nothing is lost.
//!
//!    **It is also the one #200-shaped field that does not need neutralising in the differential
//!    driver, and the reason is worth recording.** Its two headless readers,
//!    `ShapeSearchTree.{drillHoleObstacle, drillHoleClearanceDelta}` (:1019, :1044) — plus
//!    `ForcedViaInserter.java:368` — are all reached from `calculateTreeShapes(DrillItem)`, i.e.
//!    from *inserting a drill item into the search tree*, and both are gated on
//!    `board.rules.getHoleClearance() > 0`. So on any board the memo is either never filled (hole
//!    clearance disabled) or filled while the DSN reader inserts the board's pins — in both cases
//!    **before the first `getHash()` of the run**, and constant thereafter. That is why
//!    `P7T10.normalizeByProducts` does not touch it and `p7t10` is still 0 diffs over
//!    10 × 2 000 steps with hundreds of via insertions and 250 snapshot restores per run.
//! 3. **`IntOctagon.precalculatedToSimplex`** (IntOctagon.java:54). `private Simplex`, **not**
//!    `transient`, filled on the first `toSimplex()` (:560-568) — and the *only* non-`transient`
//!    lazy cache in the whole of `geometry/planar` (every other `precalculated*` there is
//!    `transient`; `Circle`'s is commented out). It is reachable from every `relativeArea`,
//!    `BoardOutline.shapes` and `Padstack.shapes` row above. Skipped for the same reason as the
//!    others and with a stronger one on top: it is a pure function of the octagon's eight `final`
//!    `int` bounds, all of which the fold already hashes through `TileShape`'s derived `Hash`.
//! 4. **`Item.smallestClearance`.** `public double`, not `transient`, so the digest sees it — but
//!    `Item.clearanceViolations` (Item.java:451-453) only ever *lowers* it, guarded by
//!    `smallestClearance < 0`, so its value records how many DRC checks have run over the item,
//!    not what the item is. Same shape as #200, same answer.
//! 5. **The undo bookkeeping.** `UndoableObjects` holds `stackLevel`, `redoPossible`, a
//!    `deletedObjectsStack` and a per-node `level`, all non-`transient`; the port holds a
//!    one-level [`UndoJournal`] of ids instead (Plan 7 Task 14c) and
//!    [`Board::structural_hash`] does not read it. The one headless caller that moves them is
//!    `BatchOptimizer.optRouteItem`, which brackets one item's re-route with
//!    `generateSnapshot()` (BatchOptimizer.java:444) and either `popSnapshot()` (:503) or
//!    `undo(null)` (:509) — **balanced**, and no `getHash()` call sits inside that window
//!    (`BatchFanout`'s is in the fanout loop, which never snapshots; `BoardHistory`'s are
//!    per-pass). So the counters are 0 at every hash comparison the pipeline makes. And at
//!    `stackLevel == 0` the two writers are no-ops by construction: `UndoableObjects.saveForUndo`
//!    only builds an undo node when `currentNode.level < this.stackLevel`, and
//!    `UndoableObjects.insert` stamps the node with `stackLevel` itself — so outside the
//!    optimizer's window every `UndoableObjectNode` carries `level = 0` and
//!    `undoObject == redoObject == null`, and there is nothing for the port to be missing —
//!    which is exactly why [`Board::begin_undo_journal`] can be a one-level `Option` rather than
//!    a stack, and why an absent journal is the faithful representation of `stackLevel == 0`.
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

use super::{Board, item_ctx};

/// The port's stand-in for the `UndoableObjects` bookkeeping of **one** undo level: the top
/// entry of `deletedObjectsStack` (UndoableObjects.java:27) plus, per item, the two facts
/// `UndoableObjectNode.level`/`.undoObject` (:330-333) encode — was this node stamped with the
/// current `stackLevel`, and does it carry a previous state.
///
/// Three id sets is all `BasicBoard.undo` needs, because the *values* Java stores (the live
/// `Item` and the `Item.clone()` `saveForUndo` takes) are both already in the caller's snapshot
/// board: `saveForUndo` clones before the **first** modification after the snapshot, and a
/// pre-snapshot item that is simply deleted is unchanged when it goes. See
/// [`Board::undo_from_snapshot`], which is the only reader.
///
/// not ported: `UndoableObjects.redo` (UndoableObjects.java:183-229), `redoPossible`,
/// `disableRedo` (:293-311) and `UndoableObjectNode.redoObject` — interactive redo, which
/// `global-constraints.md` excludes with the rest of the GUI, and which no headless caller can
/// reach (the only `undo` on that path is `BatchOptimizer.java:509`, and nothing follows it).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UndoJournal {
    /// Nodes stamped with `stackLevel` and carrying no undo object: the items inserted since
    /// the snapshot. `undo` cancels each and restores nothing for it.
    pub(crate) created: BTreeSet<ItemId>,
    /// Nodes stamped with `stackLevel` that carry an undo object: pre-snapshot items modified
    /// in place by one of Java's five `saveForUndo` call sites. `undo` cancels the live item
    /// **and** restores the snapshot's.
    pub(crate) saved: BTreeSet<ItemId>,
    /// The delete list itself, in deletion order — a `LinkedList` in Java (:133), appended to by
    /// `delete` (:117,:121). `undo` restores these after the `saved` ones, in this order.
    pub(crate) deleted: Vec<ItemId>,
}

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
        // Port-only, and the same reason as the rest of this block: `UndoableObjects`' undo
        // bookkeeping does not survive Java's serialization round trip either (the module doc's
        // "The undo bookkeeping" row). A snapshot board never records.
        copy.undo_journal = None;
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

    // =============================================================================================
    // The one undo level `BatchOptimizer.optRouteItem` opens — Plan 7 Task 14c, ruling BA
    // =============================================================================================

    /// Port of `BasicBoard.generateSnapshot` (BasicBoard.java:1290-1292) ->
    /// `BoardSnapshotManager.generateSnapshot` (:75-78) -> `UndoableObjects.generateSnapshot`
    /// (UndoableObjects.java:131-136), narrowed to the `itemList` half.
    ///
    /// Java pushes an empty delete list and bumps `stackLevel`; this opens the journal that
    /// [`Board::undo_from_snapshot`] reads. The **item state** still comes from the caller's
    /// [`Board::deep_copy`] (plan-7 ruling 8): what the journal adds is the two things a clone
    /// cannot reconstruct — *which* items `undo` touches, and in *what order* — because
    /// `applyUndoRedoSideEffects` replays them through the live search trees and
    /// `MinAreaTree`'s insertion heuristic makes the tree a function of that order (quirk #229).
    ///
    /// Re-entrancy is not modelled: Java's stack is a `Vector` and this is one level, because
    /// `optRouteItem` is the only headless caller and it brackets one item's re-route with a
    /// balanced `generateSnapshot`/`popSnapshot`-or-`undo` pair (`board/snapshot.rs`' module
    /// doc, "The undo bookkeeping"). A second `begin_undo_journal` before the first is closed
    /// therefore discards the first, which is what a one-level stack must do.
    pub fn begin_undo_journal(&mut self) {
        self.undo_journal = Some(UndoJournal::default());
    }

    /// Port of `BasicBoard.popSnapshot` (BasicBoard.java:1298-1300) ->
    /// `UndoableObjects.popSnapshot` (UndoableObjects.java:232-270), narrowed the same way: at
    /// one level, "the situation cannot be restored anymore" is exactly "drop the journal".
    ///
    /// Java's body also re-levels every node it kept and joins the two top delete lists; with a
    /// single level there is no second list to join and every surviving node goes back to level
    /// 0, which is what an absent journal means here.
    pub fn discard_undo_journal(&mut self) {
        self.undo_journal = None;
    }

    /// The open journal, or `None` at `stackLevel == 0`. Test-only: [`Board::undo_from_snapshot`]
    /// reads the field directly, and the *order* the journal records — the whole point of the
    /// type — is not observable from outside the crate, so the tests that pin it need this.
    #[cfg(test)]
    pub(crate) fn undo_journal(&self) -> Option<&UndoJournal> {
        self.undo_journal.as_ref()
    }

    /// Port of `UndoableObjects.saveForUndo` (UndoableObjects.java:273-291): record that a
    /// pre-snapshot item is about to be modified **in place**, so `undo` can put its previous
    /// state back.
    ///
    /// Java stores a `clone()` of the item; the port stores only the id, because the previous
    /// state is already in the caller's snapshot board — `saveForUndo` is called before the
    /// *first* modification after the snapshot (`currentNode.level < this.stackLevel` guards the
    /// second one), so Java's clone and `snapshot.items[id]` are the same board state.
    ///
    /// Java's guard has a second effect this reproduces: an item **created** since the snapshot
    /// already carries `level == stackLevel`, so its `saveForUndo` builds no undo node at all
    /// and `undo` simply cancels it.
    ///
    /// Called at each of Java's five call sites; see the `saveForUndo` markers in
    /// `board/trace_normalize.rs`, `board/shape_trace_entries.rs`, `board/mod.rs` and
    /// `items/header.rs`.
    pub(crate) fn save_for_undo(&mut self, id: ItemId) {
        if let Some(journal) = self.undo_journal.as_mut() {
            // UndoableObjects.java:281 — `currentNode.level < this.stackLevel`.
            if !journal.created.contains(&id) {
                journal.saved.insert(id);
            }
        }
    }

    /// `UndoableObjects.insert`'s journal half (UndoableObjects.java:66-70): the new node is
    /// stamped with `stackLevel`, which is what makes `undo` cancel it.
    pub(crate) fn journal_insert(&mut self, id: ItemId) {
        if let Some(journal) = self.undo_journal.as_mut() {
            journal.created.insert(id);
        }
    }

    /// `UndoableObjects.delete`'s journal half (UndoableObjects.java:114-124), in Java's three
    /// cases and in Java's order — the delete list is a `LinkedList`, so it is *deletion* order,
    /// and that is the order `applyUndoRedoSideEffects` re-inserts in.
    ///
    /// * the node's level is below `stackLevel` (a pre-snapshot item, untouched): the node
    ///   itself joins the delete list (`:118`);
    /// * the node is at `stackLevel` with an undo node (a pre-snapshot item already modified in
    ///   place): the **undo** node joins it (`:121`) — same id, and the same board state the
    ///   port's snapshot holds, so the port appends the same id and forgets the `saveForUndo`;
    /// * the node is at `stackLevel` with no undo node (created since the snapshot): nothing
    ///   joins the list, and the id also leaves `created`, so `undo` neither cancels nor
    ///   restores it. Java gets that by `objects.remove` (`:125`) taking the node out of the map
    ///   the cancel loop walks.
    pub(crate) fn journal_remove(&mut self, id: ItemId) {
        if let Some(journal) = self.undo_journal.as_mut() {
            if journal.created.remove(&id) {
                return;
            }
            journal.saved.remove(&id);
            journal.deleted.push(id);
        }
    }

    /// Port of `BasicBoard.undo(Set<Integer>)` (BasicBoard.java:1233-1240) — the
    /// `changedNets == null` call `BatchOptimizer.java:509` makes — together with
    /// `UndoableObjects.undo` (UndoableObjects.java:143-171) and the private
    /// `BasicBoard.applyUndoRedoSideEffects` (BasicBoard.java:1255-1287).
    ///
    /// # What this restores, and what it deliberately leaves alone
    ///
    /// Java's `undo` touches exactly two things — `components` (`:1234`) and `itemList`
    /// (`:1237`) — and then fixes the **live** search trees up item by item. Every other field
    /// of the board keeps the value the failed attempt left it with: the id generator (so the
    /// burned ids stay burned), `revision`, `changedArea`, `min`/`maxTraceHalfWidth`,
    /// `normalizeSuppressedNetNos`, `shoveFailingObstacle`, `shoveFailingLayer`, and the
    /// `autorouteInfo` of every item `undo` does *not* restore. Assigning a whole snapshot board
    /// back rolls all of those back too — that is what Task 14b measured as quirk #229 and what
    /// the Task 14b review's binding inventory added to it — so this takes only `components` and
    /// the item map's difference from `snapshot` and leaves `self` alone otherwise.
    ///
    /// # The order is the divergence, so it is spelled out
    ///
    /// `UndoableObjects.undo` walks `objects.values()` — a `ConcurrentSkipListMap` keyed by
    /// `Item.compareTo`, whose subtraction is reversed (Item.java:98, quirk #44) — so the
    /// cancelled items and the undo objects of the modified ones come out in **descending item
    /// id**, in one pass; the delete list is appended after them, in **deletion order**. Then
    /// `applyUndoRedoSideEffects` runs `searchTreeManager.remove` over the cancelled list
    /// (`:1262`) and `searchTreeManager.insert` over the restored list (`:1276`). Both are
    /// leaf-level `MinAreaTree` operations whose result depends on the order they arrive in, and
    /// `ShapeSearchTree45Degree.completeShape` (`:152-274`) reads the resulting topology
    /// directly, so this order is load-bearing rather than cosmetic.
    ///
    /// # The state a restored item is in
    ///
    /// Java's restored object is either the original `Item` — whose `searchTreeManager.remove`
    /// at `BoardItemRepository.java:193` already ran `clearSearchTreeEntries()` — or an
    /// `Item.clone()` (Item.java:259-266), which copies `onTheBoard` and deliberately **not**
    /// `searchTreesInfo` (`:263` is commented out). Either way it arrives at `:1276` with no
    /// tree entries and no cached tree shapes, so this clears both on the item it takes out of
    /// the snapshot before handing it to the trees, and clears `autorouteInfo` afterwards
    /// exactly as `:1277` does.
    ///
    /// `changedNets` is Java's out-parameter and is always `null` on this path
    /// (`BatchOptimizer.java:509`), so the two `changedNets != null` blocks (`:1265-1269`,
    /// `:1280-1284`) have nothing to write to. The observer notifications (`:1263`, `:1278`) are
    /// dropped with the rest of the observers (`global-constraints.md`).
    // renamed: `BasicBoard.undo` (BasicBoard.java:1233-1240) -> `Board::undo_from_snapshot`, which
    // takes the pre-attempt board by value where Java reads it off the `UndoableObjects` stack.
    // renamed: the private `BasicBoard.applyUndoRedoSideEffects` (BasicBoard.java:1255-1287) ->
    // this method's second half (Rust has no private-helper visibility to preserve here, and
    // `redo` — its only other caller — is not ported).
    pub fn undo_from_snapshot(&mut self, snapshot: Board) {
        let journal = self.undo_journal.take().unwrap_or_default();
        let mut snapshot_items = snapshot.items;

        // BasicBoard.java:1234 — `this.components.undo(this.communication.observers)`. Nothing
        // on the optimizer path moves a component, so this is the identity in practice; it is
        // written because Java writes it, and `Components`' own undo stack is not ported.
        self.components = snapshot.components;

        // UndoableObjects.java:148-160, in one pass over `objects.values()`: descending item id
        // (quirk #44). `cancelled` is every node at `stackLevel`; `restored`'s first segment is
        // the undo object of each of those that has one, i.e. the items modified in place.
        let cancelled: Vec<ItemId> = journal
            .created
            .iter()
            .chain(journal.saved.iter())
            .copied()
            .collect::<BTreeSet<ItemId>>()
            .into_iter()
            .rev()
            .collect();
        // UndoableObjects.java:161-168: the delete list, appended after them in deletion order.
        let restored: Vec<ItemId> = journal
            .saved
            .iter()
            .rev()
            .copied()
            .chain(journal.deleted.iter().copied())
            .collect();

        // BasicBoard.java:1259-1270 — `searchTreeManager.remove(currentItem)` on the LIVE trees,
        // in cancelled order. The item is still in the map here, exactly as Java's cancelled
        // object is still the live `Item` its collection holds.
        for id in &cancelled {
            if let Some(item) = self.items.get_mut(id) {
                self.trees.remove(item);
            }
        }

        // `itemList.undo`'s effect on the map (UndoableObjects.java:148-168): the created nodes
        // drop below the visible level (`readObject` skips `level > stackLevel`, :56-59), and
        // every restored node goes back in under its own id.
        for id in &journal.created {
            self.items.remove(id);
        }
        for id in &restored {
            if let Some(item) = snapshot_items.remove(id) {
                self.items.insert(*id, item);
            }
        }

        // BasicBoard.java:1272-1285 — `currentItem.board = this` (the port has no back-pointer),
        // `searchTreeManager.insert(currentItem)` and `currentItem.clearAutorouteInfo()`, in
        // restored order.
        let ctx = item_ctx!(self);
        for id in &restored {
            let Some(item) = self.items.get_mut(id) else {
                continue;
            };
            // The state Java's restored object arrives in — see the doc comment.
            item.clear_tree_entries();
            item.set_on_the_board(false);
            // BasicBoard.java:1275.
            self.trees.insert(item, &ctx);
            // BasicBoard.java:1277.
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
    /// (the three `writeObject` calls at :31-33) through Java object serialization — i.e. the whole item graph, not just the traces
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
            // is deliberately absent — the module doc's `smallestClearance` skipped row.
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
                    // state, not the lazily filled cache the module doc's `DrillItem.center` skipped row
                    // is about.
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
                    // `DrillItem.center` is **not** hashed here: the module doc's
                    // `DrillItem.center` skipped row (quirk #200). `component_id` above and
                    // `pin_index` here determine it.
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
                    // the module doc's skipped rows.
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

    // ---------------------------------------------------------------------------------------
    // Plan 7 Task 14c: the one undo level `BatchOptimizer.optRouteItem` opens (quirk #229)
    // ---------------------------------------------------------------------------------------

    #[test]
    fn undo_from_snapshot_cancels_the_inserted_items_and_restores_the_removed_ones() {
        let mut board = board();
        let kept = insert_trace(&mut board, 1, 100, 200);
        let removed = insert_trace(&mut board, 1, 300, 400);

        let snapshot = board.deep_copy();
        board.begin_undo_journal();
        let inserted = insert_trace(&mut board, 1, 500, 600);
        board.remove_item(removed);
        assert!(board.get_item(inserted).is_some());
        assert!(board.get_item(removed).is_none());

        board.undo_from_snapshot(snapshot);

        // `UndoableObjects.undo` (:148-168): the node stamped with `stackLevel` drops below the
        // visible level, and the delete list goes back in.
        assert!(board.get_item(inserted).is_none());
        assert!(board.get_item(removed).is_some());
        assert!(board.get_item(kept).is_some());
        assert_eq!(board.undo_journal(), None);
    }

    #[test]
    fn undo_from_snapshot_puts_the_restored_item_back_in_the_live_trees() {
        let mut board = board();
        let trace = insert_trace(&mut board, 1, 300, 400);
        let ctx = board.ctx();
        let shape = board
            .get_item(trace)
            .expect("the trace")
            .get_tile_shape(board.default_tree_id(), 0, &ctx)
            .expect("its tile shape");
        let object = crate::ids::TreeObject::Item(trace);

        let snapshot = board.deep_copy();
        board.begin_undo_journal();
        board.remove_item(trace);
        assert!(!board.overlapping_objects(&shape, Some(0)).contains(&object));

        board.undo_from_snapshot(snapshot);

        // BasicBoard.java:1276 — `searchTreeManager.insert(currentItem)` on the **live** tree.
        assert!(board.overlapping_objects(&shape, Some(0)).contains(&object));
        assert!(board.get_item(trace).expect("the trace").is_on_the_board());
        assert!(board.validate_item(trace));
    }

    #[test]
    fn undo_from_snapshot_leaves_every_field_java_leaves_alone() {
        let mut board = board();
        let untouched = insert_trace(&mut board, 1, 100, 200);
        let removed = insert_trace(&mut board, 1, 300, 400);

        let snapshot = board.deep_copy();
        board.begin_undo_journal();

        // The attempt burns an id, fills scratch on an item the undo will *not* restore, and
        // writes the three fields `Board::deep_copy` resets but `BasicBoard.undo` never touches.
        let burned = board.new_item_id();
        board
            .get_item_mut(untouched)
            .expect("the trace")
            .get_autoroute_info();
        board.normalize_suppressed_net_nos.insert(7);
        board.shove_failing_obstacle = Some(removed);
        board.shove_failing_layer = 3;
        let revision_before_undo = board.revision();
        board.remove_item(removed);

        board.undo_from_snapshot(snapshot);

        // The ids the failed attempt burned stay burned (BasicBoard.undo:1233-1240).
        assert_eq!(board.new_item_id().0, burned.0 + 1);
        // The three transient fields `deep_copy` resets and `undo` does not.
        assert!(board.normalize_suppressed_net_nos.contains(&7));
        assert_eq!(board.shove_failing_obstacle, Some(removed));
        assert_eq!(board.shove_failing_layer, 3);
        // `revision` keeps counting; the removal above bumped it and the undo does not undo that.
        assert!(board.revision() > revision_before_undo);
        // `clearAutorouteInfo` runs on the **restored** items only (BasicBoard.java:1277) — not,
        // as `Board::deep_copy`'s `clearAllItemTemporaryAutorouteData` would, on every item.
        assert!(
            board
                .get_item(untouched)
                .expect("the trace")
                .get_autoroute_info_pur()
                .is_some()
        );
        assert!(
            board
                .get_item(removed)
                .expect("the restored trace")
                .get_autoroute_info_pur()
                .is_none()
        );
    }

    #[test]
    fn the_journal_records_removals_in_deletion_order_and_insertions_as_a_set() {
        let mut board = board();
        let a = insert_trace(&mut board, 1, 100, 200);
        let b = insert_trace(&mut board, 1, 300, 400);
        let c = insert_trace(&mut board, 1, 500, 600);

        board.begin_undo_journal();
        let created = insert_trace(&mut board, 1, 700, 800);
        // Deliberately not id order: `UndoableObjects`' delete list is a `LinkedList`, so
        // `applyUndoRedoSideEffects` re-inserts in *deletion* order (:1276), and that order is
        // what decides the resulting `MinAreaTree` topology — quirk #229's whole mechanism.
        board.remove_item(c);
        board.remove_item(a);
        board.remove_item(b);
        // An item created since the snapshot and then deleted leaves no trace at all: Java's
        // `delete` adds nothing to the list and `objects.remove` takes its node out of the map
        // the cancel loop walks.
        board.remove_item(created);

        let journal = board.undo_journal().expect("open");
        assert_eq!(journal.deleted, vec![c, a, b]);
        assert!(journal.created.is_empty());
        assert!(journal.saved.is_empty());
    }

    #[test]
    fn combining_two_traces_records_a_save_for_undo_and_the_undo_puts_the_short_one_back() {
        let mut board = board();
        // Two collinear traces that meet at (300, 100): `combine_trace` joins them, which is
        // Java's `PolylineTrace.combine` -> `itemList.saveForUndo(this)` (:275/:417) plus a
        // `removeItem` of the absorbed one.
        let first = insert_trace(&mut board, 1, 100, 300);
        let second = insert_trace(&mut board, 1, 300, 600);

        let snapshot = board.deep_copy();
        board.begin_undo_journal();
        assert!(board.combine_trace(first).expect("combine"));

        let journal = board.undo_journal().expect("open");
        assert!(
            journal.saved.contains(&first),
            "the surviving trace was modified in place, so `undo` must cancel it and restore \
             the pre-attempt one"
        );
        assert_eq!(journal.deleted, vec![second]);
        let ctx = board.ctx();
        let combined_len = board
            .get_item(first)
            .expect("the survivor")
            .bounding_box(&ctx)
            .width();

        board.undo_from_snapshot(snapshot);

        assert!(board.get_item(second).is_some());
        let ctx = board.ctx();
        assert!(
            board
                .get_item(first)
                .expect("the survivor")
                .bounding_box(&ctx)
                .width()
                < combined_len,
            "the in-place modification is rolled back to the snapshot's geometry"
        );
        assert!(board.validate_item(first));
        assert!(board.validate_item(second));
    }

    #[test]
    fn discard_undo_journal_is_pop_snapshot_and_deep_copy_never_records() {
        let mut board = board();
        insert_trace(&mut board, 1, 100, 200);
        board.begin_undo_journal();
        assert!(board.undo_journal().is_some());

        // `Board::deep_copy` is a `readObject` round trip: the copy starts at `stackLevel == 0`.
        let copy = board.deep_copy();
        assert_eq!(copy.undo_journal(), None);

        board.discard_undo_journal();
        assert_eq!(board.undo_journal(), None);
        // With no journal open, the hooks are no-ops: `stackLevel == 0` is exactly where Java's
        // `saveForUndo`/`delete` write nothing either.
        let trace = insert_trace(&mut board, 1, 300, 400);
        board.remove_item(trace);
        assert_eq!(board.undo_journal(), None);
    }
}
