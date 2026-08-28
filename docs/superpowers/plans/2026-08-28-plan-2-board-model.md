# Plan 2 — `fr-board` (items, rules, library, search trees, board) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port freerouting's board *model* layer — `board/model/**`, `board/facade/**`, `board/searchtree/**`, `board/trace/**`, `board/state/{ChangedArea,Communication}`, `rules/**`, `core/library/**`, and `datastructures/{ShapeTree,MinAreaTree,PlanarDelaunayTriangulation,TimeLimit}` — as the `fr-board` crate, with an arena board, enum items, one angle-parameterised search tree, Java-faithful trace normalisation, and `Board: Clone` snapshots.

**Architecture:** Spec §6. `Board` owns `items: BTreeMap<ItemId, Item>` keyed by the Java item id (deterministic iteration; Java's `ConcurrentHashMap` order is JVM-dependent so nothing may depend on it). No back-pointers: every Java method that reads `this.board` takes `&BoardRules`/`&LayerStructure`/`&Board` explicitly, or is a `Board` method taking `ItemId`. The search tree lives inside `Board` and is updated by the same `&mut` methods that mutate items (remove → mutate → insert, exactly Java's protocol). Java's three angle-specific tree subclasses collapse into one `ShapeSearchTree` with an `AngleRestriction`. Shove/tighten/forced-route (`board/optimize/*`, `board/actions/{ForcedPadRouter,ForcedViaInserter,DrillItemMover,MoveComponent}`) and `RoutingBoard`'s shove entry points are **Plans 6/7** per spec §9; interactive-only code (undo/redo stack, observers, graphics boxes, info printers, item picking, `BoardComparator`) is not ported.

**Tech Stack:** Rust 2024; `fr-geometry` (Plan 1); `thiserror`; no `slotmap` (ruling below); `rayon` not yet.

**Spec:** `docs/superpowers/specs/2026-08-27-freerouting-rust-port-design.md` (§3, §5, §6, §10, §14). Also binding: `docs/plan-1-handoff.md` (rulings + obligations) and `docs/java-quirks.md`.

**Later plans:** 3 `fr-dsn`, 4 `fr-settings`, 5 `fr-drc`, 6–7 `fr-router`, 8 `fr-core` + surfaces.

## Global Constraints

All Plan 1 constraints and rulings remain in force (see `docs/plan-1-handoff.md` §Rulings). Additionally:

- Java authority for this plan: `../freerouting/src/main/java/app/freerouting/{board,rules,core/library,datastructures}/**` at the clone's HEAD (the v2.3.0 jar for the differential harness has the pre-rename layout: `app.freerouting.board.BasicBoard`, `app.freerouting.library.*` — check with `javap` before writing drivers).
- **Java wins over plan text.** Every test-data change cites the Java line; never bend a test to fit a bug. Reproduced Java bugs get a `// Java bug:` marker and a row in `docs/java-quirks.md`; crash→value totalizations are allowed only where no reachable Java caller observes the difference (Plan 1 ruling), otherwise `Result`/`Option`/panic.
- Item keys: `ItemId(u32)` = the Java item id (from `ItemIdGenerator`: starts at 1, wraps to 1 at `i32::MAX / 2`). Storage `BTreeMap<ItemId, Item>`. Java `Leaf.compareTo` = `(object id, shape index)`; every tree result set is a `BTreeSet` ordered that way.
- No `board` back-pointers, no observers, no `Serializable`. `Board: Clone`; `clone()` must not copy per-run autoroute scratch (`autoroute_info`) — it is `Option<…>` reset to `None` by `Board::deep_copy()` (which is `clone()` + `clear_autoroute_scratch()`; search-tree entries ARE cloned since leaf ids are arena indices that clone with the tree).
- `ShapeSearchTree.lastGeneratedEntryId` is a Java **static** used as a `TreeSet` tie-break in clearance queries; port it as a per-`SearchTreeManager` counter, document as a deliberate determinism choice (quirks row).
- Trace normalisation: `MAX_NORMALIZATION_DEPTH = 16` (`PolylineTraceNormalization.java:16`; AGENTS.md's "34" is stale — note in quirks), `MAX_NORMALIZE_ITERATIONS = 2000` (`BasicBoard.java:64`). `Polyline::from_lines` is `Result` (Plan 1 ruling): inside normalisation an `Err` propagates as `BoardError::Normalization` — it must never be swallowed into an empty trace.
- `fr-board` must not depend on `tracing`; diagnostic `FRLogger` calls are dropped, invariant-guard logs become `debug_assert!`.
- Every task: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, plus `scripts/audit-port.sh board` (Task 1 extends the Plan 1 audit script to this crate) must be clean before commit. Reports must be verified against the committed tree.
- Commit trailer:
  ```
  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_015f1ubhBBpiWHAooDsso97Z
  ```

## Rulings made while writing this plan (recorded here so they reach the user)

1. **`BTreeMap<ItemId, Item>` keyed by Java id instead of `slotmap`** (spec §6 said SlotMap). Reason: Java ids are semantically load-bearing (`Leaf` ordering, `getId` at 55 call sites, SES writer, hash detection); a slotmap key would need a parallel id field anyway. **Amended after Task 10 (quirk #63):** Java's `UndoableObjects.objects` is a `ConcurrentSkipListMap` ordered by the *reversed* `Item.compareTo`, so `board.itemList` iterates in deterministic **descending** id order — not hash order as first assumed. Every port of a Java `itemList` walk must iterate `items.values().rev()` (descending); `MinAreaTree` insertion order decides tree structure, so ascending iteration would silently reshape every tree. Cost if wrong: O(log n) lookups instead of O(1) — negligible at PCB scale.
2. **Tree objects are an enum `TreeObject { Item(ItemId), Room(RoomId) }`** because Java's autoroute inserts `CompleteFreeSpaceExpansionRoom` into the same `ShapeTree`. `RoomId` is reserved here; Plan 6 populates it. Ordering: Java `SearchTreeObject` ordering — read `Item.compareTo` and the room's `compareTo` in Plan 6; here `Item` variants order by id, and `Item < Room`.
3. **`ConductionArea` fill cache (`java.awt.geom.Area`) is not ported** — survey confirmed it is renderer-only; `i_overlay` is therefore not added in Plan 2. `docs/geometry-library-survey.md` is amended by Task 16.
4. **`RoutingBoard` is split:** its non-shove state (`changed_area`, `check_trace_segment`, `remove_items_and_pull_tight`'s *removal* half, failure log hook, `max_trace_half_width` bookkeeping) lands here as `Board`; the shove/forced-via/pull-tight entry points arrive in Plan 7 as an extension trait `RoutingBoardExt` in `fr-router`.

## File Structure

```
crates/fr-board/
  Cargo.toml                      deps: fr-geometry, thiserror
  src/lib.rs                      pub mod list + prelude
  src/ids.rs                      ItemId, RoomId, TreeObject, TreeId, ItemIdGenerator, Ordering impls
  src/error.rs                    BoardError { Normalization(PolylineError), … }
  src/structure/mod.rs
  src/structure/layer.rs          Layer, LayerStructure, Unit, AngleRestriction, FixedState
  src/structure/component.rs      Component, Components
  src/structure/board_outline.rs  BoardOutline item
  src/structure/entry_side.rs     ShapeEntrySide, ShapeAndEntrySide
  src/library/mod.rs              Padstack(s), Package(s), LogicalPart(s), BoardLibrary
  src/rules/mod.rs
  src/rules/clearance_matrix.rs   ClearanceMatrix (dense [class][class][layer] i32)
  src/rules/nets.rs               Net, Nets
  src/rules/net_class.rs          NetClass, NetClasses, DefaultItemClearanceClasses, ItemClass
  src/rules/via.rs                ViaInfo, ViaInfos, ViaRule
  src/rules/board_rules.rs        BoardRules
  src/datastructures/shape_tree.rs        ShapeTree<O> arena (nodes, leaves), MinAreaTree insertion
  src/datastructures/delaunay.rs          PlanarDelaunayTriangulation
  src/datastructures/time_limit.rs        TimeLimit, StopCheck (fn() -> bool)
  src/items/mod.rs                enum Item, ItemHeader, ItemKind tags, Connectable dispatch
  src/items/header.rs
  src/items/trace.rs              PolylineTrace (geometry half)
  src/items/trace_normalize.rs    combine_at_start/end, normalize (depth 16)
  src/items/drill.rs              DrillItem shared, Via, Pin
  src/items/area.rs               ObstacleArea, ConductionArea, ViaObstacleArea, ComponentObstacleArea, ComponentOutline
  src/searchtree/mod.rs
  src/searchtree/shape_search_tree.rs     ShapeSearchTree (angle-parameterised), calculate_tree_shapes per item, queries
  src/searchtree/manager.rs               SearchTreeManager (default + compensated trees), entry-id counter
  src/searchtree/trace_entries.rs         ShapeTraceEntries
  src/board/mod.rs                Board (BasicBoard + non-shove RoutingBoard): fields, insert/remove, queries
  src/board/connectivity.rs       BoardConnectivityQueries
  src/board/normalize.rs          normalize_traces (2000 cap)
  src/board/snapshot.rs           deep_copy, structural hash (BoardHistory support)
  src/board/changed_area.rs       ChangedArea
  src/board/communication.rs      Communication { unit, resolution, id_gen, host_cad/version }
  tests/board_builder.rs          test helper: programmatic boards (padstack + pins + traces)
  tests/*.rs                      ported Java tests (see tasks)
scripts/audit-port.sh             generalised audit (package → crate) replacing audit-geometry-port.sh
scripts/differential/java/B*.java + rust/src/bin/b*.rs   board-level drivers (Task 15)
```

---

### Task 1: Crate skeleton, ids, structure enums, generalised audit script

**Files:** `crates/fr-board/{Cargo.toml,src/lib.rs,src/ids.rs,src/error.rs,src/structure/{mod.rs,layer.rs}}`, `scripts/audit-port.sh` (generalise `audit-geometry-port.sh`: args `<java-subpath> <rust-crate-src>`; keep the old script as a one-line wrapper), workspace `Cargo.toml` (add member).
**Java:** `board/model/structure/{Layer,LayerStructure,Unit,AngleRestriction,FixedState}.java`, `board/actions/ItemIdGenerator.java`, `board/searchtree/SearchTreeObject.java`, `datastructures/IdGenerator.java`.

**Interfaces produced:**
- `ItemId(pub u32)` (`Copy, Ord, Hash, Display`); `RoomId(pub u32)`; `enum TreeObject { Item(ItemId), Room(RoomId) }` with `Ord` = Item-by-id, then Room-by-id, `Item < Room`; `TreeId(pub u32)`.
- `ItemIdGenerator { last: u32 }` — `new_id()` port of `ItemIdGenerator.newId` (wrap at `i32::MAX/2` → restarts at 1; test the wrap).
- `Layer { name: String, is_signal: bool }`; `LayerStructure { layers: Vec<Layer> }` with `count()`, `get_no(name) -> Option<usize>`, `signal_layer_count()`, `get_signal_layer_no(i)`, `get_layer_no_of_signal_layer(i)` … (port every public method).
- `enum Unit { Mil, Inch, Mm, Um }` with `scale(value, from, to)` and `from_string` exactly as Java (including `Unit.java`'s conversion constants).
- `enum AngleRestriction { None, FortyFiveDegree, NinetyDegree }` with Java's int mapping; `enum FixedState { Unfixed, ShoveFixed, UserFixed, SystemFixed }` with `Ord` (Java compares ordinals: `UNFIXED < SHOVE_FIXED < USER_FIXED < SYSTEM_FIXED`).
- `BoardError` enum: `Normalization(#[from] fr_geometry::PolylineError)`, `InvalidLayer(usize)`, `UnknownItem(ItemId)`.

**Tests (write first):** `ids::tests::generator_wraps_like_java` (`last = i32::MAX/2` → next is 1); `TreeObject` ordering (`Item(5) < Item(7) < Room(1)`); `Unit::scale(1.0, Inch, Mil) == 1000.0`, `scale(25.4, Mm, Inch) == 1.0` (check Java's constants — `Unit.java` may use 0.0254 factors; assert what Java computes); `FixedState::Unfixed < FixedState::SystemFixed`; `LayerStructure::get_no("F.Cu")`. Audit script: `scripts/audit-port.sh board/model/structure crates/fr-board/src` must exit 0 at the end of the task (only the five files above are in scope now — the script takes an optional file glob).

Steps: tests → fail → implement → pass → fmt/clippy/test/audit → commit `feat(board): crate skeleton, ids, layer structure, audit script`.

---

### Task 2: Rules — `ClearanceMatrix`, `Net(s)`, `NetClass(es)`, `ViaInfo(s)`/`ViaRule`, `BoardRules`

**Files:** `src/rules/*.rs`; `tests/clearance_matrix.rs` (port `src/test/java/app/freerouting/rules/ClearanceMatrixTest.java`).
**Java:** `rules/*.java` (1,955 lines).

**Interfaces produced:**
- `ClearanceMatrix { layer_count, class_names: Vec<String>, values: Vec<i32> /* [class_j][class_i][layer] flattened */, max_value_on_layer: Vec<i32> }` — `new(class_count, &LayerStructure, names)`, `get_value(class_i, class_j, layer, add_safety_margin: bool) -> i32` porting the **J-then-I** indexing (`ClearanceMatrix.java:131`) and `CLEARANCE_SAFETY_MARGIN = 16`; `set_value`, `max_value(class, layer)`, `clearance_compensation_value(class, layer)`, `append_class`, `remove_class`, `get_no(name) -> Option<usize>`, `is_layer_dependent`, `equals_default…` — every public method.
- `Net { name, subnet_no, net_no: i32, contains_plane, net_class: NetClassId }`; `Nets { nets: Vec<Net> }` with `MAX_LEGAL_NET_NO = 9_999_999`, `HIDDEN_NET_NO = 10_000_001`, `new_net`, `get(net_no) -> Option<&Net>`, `get_by_name`, `max_net_no`; net→class by index (`NetClassId(usize)`) — no object references.
- `ItemClass` enum + `DefaultItemClearanceClasses([usize; N])`; `NetClass { name, trace_half_width: Vec<i32> /* per layer */, active_routing_layer: Vec<bool>, default_item_clearance_classes, is_ignored_by_autorouter, via_rule: ViaRuleId, trace_clearance_class, shove_fixed, pull_tight, ignore_cycles_with_areas, min/max_trace_length }`; `NetClasses`.
- `ViaInfo { name, padstack: PadstackId, clearance_class, attach_smd_allowed }`, `ViaInfos`, `ViaRule { name, vias: Vec<ViaInfoId> }` with `EMPTY` and `get_layer_range(from, to, &Padstacks)`.
- `BoardRules { clearance_matrix, nets, via_infos, via_rules, net_classes, trace_angle_restriction, ignore_conduction: bool (default true), min/max_trace_half_width, pin_edge_to_turn_dist: f64, use_slow_autoroute_algorithm, hole_clearance }` and its public methods (`get_default_net_class`, `get_trace_half_width(net_no, layer)`, `clearance_value(c1, c2, layer)`, …). `PadstackId`/`ViaInfoId`/`ViaRuleId`/`NetClassId` are `usize` newtypes; the library (Task 4) owns padstacks.

**Tests:** port `ClearanceMatrixTest` exactly; add `get_value_uses_j_then_i_indexing` (set `[i=1][j=2][layer 0] = 100` via `set_value` and assert `get_value(1,2,0,false) == 100` and `get_value(2,1,0,false)` equals whatever Java's symmetric `set_value` produces — read it), `safety_margin_adds_16`, `nets_hidden_net_constant`, `via_rule_empty`.

Commit `feat(board): rules — clearance matrix, nets, net classes, via rules, board rules`.

---

### Task 3: `ShapeTree<O>` / `MinAreaTree` arena and `TimeLimit`

**Files:** `src/datastructures/{shape_tree.rs,time_limit.rs}`; `tests/min_area_tree.rs` (port `datastructures/MinAreaTreeConcurrencyTest.java` semantics — the concurrency part becomes a single-threaded determinism test).
**Java:** `datastructures/{ShapeTree,MinAreaTree,TimeLimit,Stoppable,ArrayStack}.java`.

**Interfaces produced:**
- `ShapeTree<O: Copy + Ord + Hash> { bounding_directions: ShapeBoundingDirections, nodes: Vec<Node<O>>, root: Option<NodeId>, leaf_count: usize }`; `Node = Inner { bounds, parent, first, second } | Leaf { bounds, parent, object: O, shape_index: usize }`; `LeafId(usize)` (arena index; freed slots reused via a free list — Java's identity semantics don't leak).
- `insert(&mut self, object: O, shapes: &[RegularTileShape]) -> Vec<LeafId>` (Java `insert(Storable)` calls `object.getTreeShape(this, i)` for each i — here the caller supplies the shapes), `insert_leaf(object, shape_index, bounds) -> LeafId`, `remove(&mut self, &[LeafId])`, `overlaps(&self, shape: &RegularTileShape) -> BTreeSet<TreeEntry<O>>` where `TreeEntry { object, shape_index }` orders `(object, shape_index)`, `leaf_bounds(LeafId)`, `leaf_count()`.
- MinAreaTree `position_locate`: at each inner node, union bounds with the new leaf's, pick the child with minimal **area increase after union** (ties → first child), replace the found leaf by an inner node holding old+new (`MinAreaTree.java` — port verbatim, including the `firstChild` tie rule). Removal relinks sibling to grandparent and shrinks ancestor bounds only while strictly smaller.
- `TimeLimit { end: Instant }` with `is_exceeded()`; `pub type StopCheck<'a> = &'a dyn Fn() -> bool;` (replaces `Stoppable`).

**Tests:** insert 8 boxes in a fixed order, assert `overlaps` returns the exact `BTreeSet` Java returns (derive by hand from the min-area rule on a small example — write out the expected tree shape in the test comment); remove a leaf and re-query; determinism: same inserts → identical node layout (`Debug` string equality) across two trees; `leaf_count`. (The Java/Rust differential driver in Task 15 covers larger cases.)

Commit `feat(board): ShapeTree/MinAreaTree arena and TimeLimit`.

---

### Task 4: Library — `Padstack(s)`, `Package(s)`, `LogicalPart(s)`, `BoardLibrary`

**Files:** `src/library/mod.rs` (+ submodules if >600 lines).
**Java:** `core/library/*.java` (888 lines).

**Interfaces produced:** `Padstack { no: usize, name, shapes: Vec<Option<Shape>> /* per layer */, attach_allowed, placed_absolute }` with `from_layer()`, `to_layer()`, `get_shape(layer)`; `Padstacks` (Vec + name lookup, `add`, `get(no)`, `get_by_name`); `Package { no, name, pins: Vec<PackagePin { name, padstack_no, relative_location: Vector, rotation_in_degree: f64 }>, outline: Vec<Shape>, keepouts, via_keepouts, place_keepouts, is_front }` with `get_pin(name)`, `pin_count()`; `Packages`; `LogicalPart(s)`; `BoardLibrary { padstacks, packages, logical_parts, via_padstacks: Vec<usize> }` with `get_via_padstack(name)`, `add_via_padstack`, `remove_via_padstack`, `get_padstack`, `get_package`. Port every public method. `Shape` is `fr_geometry::Shape`.

**Tests:** build a 2-layer padstack, assert `from_layer/to_layer`; package pin lookup by name; `BoardLibrary::via_padstacks` order after add/remove matches Java (`Vector` semantics).

Commit `feat(board): library — padstacks, packages, logical parts`.

---

### Task 5: `Item` enum, `ItemHeader`, `Connectable`, and `Components`

**Files:** `src/items/{mod.rs,header.rs}`, `src/structure/component.rs`.
**Java:** `board/model/items/{Item,Connectable,BoardItemType}.java`, `board/actions/ItemSearchTreesInfo.java`, `board/model/structure/{Component,Components}.java`.

**Interfaces produced:**
- `ItemHeader { id: ItemId, net_nos: Vec<i32>, clearance_class: usize, fixed_state: FixedState, component_id: i32, on_the_board: bool, tree_entries: HashMap<TreeId, TreeEntries { leaves: Vec<LeafId>, shapes: Vec<TileShape> }>, autoroute_info: Option<Box<AutorouteInfo>> /* opaque placeholder type defined here as an empty struct; Plan 6 replaces */, smallest_clearance: f64 (-1 sentinel) }`.
- `enum Item { Trace(PolylineTrace), Via(Via), Pin(Pin), ObstacleArea(ObstacleArea), ConductionArea(ConductionArea), ViaObstacleArea(ViaObstacleArea), ComponentObstacleArea(ComponentObstacleArea), ComponentOutline(ComponentOutline), BoardOutline(BoardOutline) }` — variants are structs defined in Tasks 6–8; this task creates them as **stubs with only the header** and fills them in later (or defines the enum in Task 5 and adds variants per task — choose the latter; document).
- Methods on `Item` porting `Item.java`'s public API that needs no board: `id()`, `header()/header_mut()`, `net_nos()`, `net_count()`, `contains_net(n)`, `shares_net(&Item)`, `nets_equal(&Item)`, `assign_net_no`, `remove_from_net`, `get_fixed_state/set_fixed_state`, `is_user_fixed/is_shove_fixed/is_deletion_forbidden`, `clearance_class()`, `set_clearance_class`, `component_id()`, `is_on_the_board()`, `kind() -> ItemKind`, `first_layer()/last_layer()/is_on_layer(l)/shape_layer(i)`, `bounding_box()`, `tile_shape_count()`, `get_tile_shape(i)`, `is_routable()`, `is_obstacle(&Item, &BoardRules) -> bool` (needs `rules.ignore_conduction`), `is_trace_obstacle`, `is_drillable(net)`, `tree_shape_count(TreeId)`, `get_tree_shape(TreeId, i)`, `set_tree_entries(TreeId, …)`, `clear_tree_entries(TreeId)`, `translate_by(&Vector)`, `turn_90_degree`, `rotate_approx`, `change_placement_side`, `copy(new_id)`, `validate()`.
- Board-dependent `Item` methods (`get_normal_contacts`, `get_connected_set`, `get_unconnected_set`, `get_connection_items`, `clearance_violations`, `is_tail`, `normal_contact_point`, `first_common_layer`, `get_ratsnest_corners`) live on `Board` in Task 11 (`Board::normal_contacts(ItemId)` …), because they query the search tree.
- `Connectable` dispatch: `Item::as_connectable() -> Option<ConnectableRef>` for Trace/Via/Pin/ConductionArea.
- `Component { no, name, location: Option<Point>, rotation_in_degree, is_front, package_no (front/back), logical_part, position_fixed }`, `Components { components: Vec<Component>, flip_style_rotate_first: bool }` with Java's public methods.

**Tests:** header net operations (`assign_net_no` on an item with 0/1/2 nets — Java `Item.assignNetNo` semantics), `FixedState` predicates, `TreeObject` from item, `Components::add/get/get_by_name`.

Commit `feat(board): Item enum, header, Connectable, Components`.

---

### Task 6: Drill items — `DrillItem` shared, `Via`, `Pin`

**Files:** `src/items/drill.rs`.
**Java:** `board/model/items/{DrillItem,Via,Pin}.java` (416+274+706).

**Interfaces produced:** `DrillItemData { center: Point, min_width_cache: Option<…> }` embedded in `Via { hdr, drill: DrillItemData, padstack: PadstackId, attach_allowed, is_escape_via, escape_via_smd_layer: i32 (-1), shapes_cache: Option<Vec<Shape>> }` and `Pin { hdr, drill, pin_no: usize, changed_to: Option<ItemId> /* pin-swap alias; Java `changedTo = this` → None */, shapes_cache }`. Methods: `center()`, `get_shape(layer, &BoardLibrary, &Components)`, `first_layer/last_layer` (via padstack), `is_on_layer`, `get_min_width`, `get_padstack`, `is_tie_pin`, Pin's `get_pin_no`, `get_component_pin_name`, `get_package_pin_relative_location`, `get_exit_restriction`/`get_trace_exit_restrictions` (port `Pin.java`'s exit restriction math verbatim — it consumes `pin_edge_to_turn_dist`), `is_smd`, `net_no_of_pin`, `swap_pin`. Anything needing the search tree (`get_normal_contacts`) is on `Board`.

**Tests:** a THT pin on a 2-layer padstack: `first_layer==0`, `last_layer==1`, shape on each layer translated by component location and rotated by component rotation (build via Task-4 library + Task-5 components); SMD via with `attach_allowed`; `Pin::get_trace_exit_restrictions` on a rectangular pad — assert the four restriction directions Java computes (derive by hand from `Pin.java`; cite lines).

Commit `feat(board): DrillItem, Via, Pin`.

---

### Task 7: Areas and outlines — `ObstacleArea` family, `ConductionArea` (model only), `ComponentOutline`, `BoardOutline`

**Files:** `src/items/area.rs`, `src/structure/board_outline.rs`.
**Java:** `board/model/items/{ObstacleArea,ConductionArea,ViaObstacleArea,ComponentObstacleArea,ComponentOutline}.java`, `board/model/structure/BoardOutline.java`.

**Interfaces produced:** `ObstacleArea { hdr, name, relative_area: Area, layer: usize, translation: Vector, rotation_in_degree: f64, side_changed: bool, absolute_area_cache: Option<Area> }` with `get_area()`, `get_relative_area()`, `get_layer()`, `translate_by`, `turn_90_degree`, `rotate_approx`, `change_placement_side`, `tile_shape_count` (= `get_area().split_to_convex().len()`), `get_tile_shape(i)`, `bounding_box`; `ConductionArea { base: ObstacleArea, is_obstacle: bool, is_filled: bool }` with `get_trace_connection_shape`, `is_obstacle(&Item, &BoardRules)`; `ViaObstacleArea`, `ComponentObstacleArea`; `ComponentOutline { hdr, relative_area, translation, rotation, is_front, is_courtyard, is_fabrication, is_closed, cache }`; `BoardOutline { hdr, shapes: Vec<PolygonShape>, keepout_outside_outline: bool }` with `get_bounding_box`, `get_outline_shape`, `shape_count`, `get_tile_shape(i)` (Java `BoardOutline` computes keepout tiles via `PolylineArea` cutout — port; needs `clearance` from rules → method takes `&BoardRules`/`&LayerStructure` where Java reads `this.board`). **Not ported:** `ConductionArea.ensureDetailedFillCache`/`getDetailedFillArea` (`// not ported: renderer-only; java.awt.geom.Area boolean ops` + quirks row).

**Tests:** obstacle area tile count for an L-shaped polygon (2), translation applied to absolute area; `ConductionArea::is_obstacle` toggles with `rules.ignore_conduction`; `BoardOutline` keepout tiles for a rectangular outline with `keepout_outside_outline = true` (count and bounding boxes as Java derives them).

Commit `feat(board): obstacle/conduction areas, component and board outlines`.

---

### Task 8: `PolylineTrace` — geometry half

**Files:** `src/items/trace.rs`.
**Java:** `board/model/items/Trace.java` (484), `board/trace/PolylineTrace.java` (geometry parts: constructor, `polyline()`, `first_corner/last_corner`, `corner_count`, `get_half_width`, `get_layer`, `tile_shape_count/get_tile_shape` via `offset_shapes`, `split(IntOctagon)` → `Vec<PolylineTrace>`? (Java `split` inserts into the board — that half is Task 9/11), `is_tail`, `get_length`, `contains(Point)`, `nearest_point`, `translate_by`, `turn_90_degree`, `change(Polyline)`, `copy`), `board/trace/PolylineTraceGeometry.java`, `board/trace/PolylineTraceSearchTreeAdapter.java` (becomes `Board`-side in Task 11).

**Interfaces produced:** `PolylineTrace { hdr, half_width: i32, layer: usize, polyline: Polyline }` + the pure-geometry public methods above; `PolylineTraceGeometry` free functions (`corner_count`, `is_tail(_, &connections)`… as Java). `split` is `fn split_polyline(&self, clip: &IntOctagon) -> Result<Option<Vec<Polyline>>, PolylineError>` (pure part) — the board insertion wrapper is Task 9.

**Tests:** port `src/test/java/app/freerouting/board/PolylineTraceSplitTest.java` (409 lines — all cases that don't need a board; the board-dependent ones move to Task 9).

Commit `feat(board): PolylineTrace geometry`.

---

### Task 9: Trace normalisation — `combine_at_start/end`, `normalize` (depth 16), `Board::split_trace`

**Files:** `src/items/trace_normalize.rs`, `src/board/normalize.rs` (needs `Board` from Task 11 — **this task is executed after Task 11**; keep numbering, dispatch order is 1,2,3,4,5,6,7,8,10,11,9,12,13…).
**Java:** `board/trace/PolylineTrace.java` (`combine`, `combineAtStart` :201, `combineAtEnd` :341, `normalize` :801), `board/trace/PolylineTraceNormalization.java`, `BasicBoard.normalizeTraces` (:709), AGENTS.md §"Trace Normalisation".

**Interfaces produced:** `Board::combine_trace(&mut self, id) -> Result<bool, BoardError>`, `combine_trace_at_start/end`, `Board::normalize_trace(&mut self, id, clip: Option<&IntOctagon>) -> Result<bool, BoardError>` (recursion depth capped at `MAX_NORMALIZATION_DEPTH = 16`, over-depth returns `Ok(false)` as Java's post-fix behaviour), `Board::normalize_traces(&mut self) -> Result<(), BoardError>` bounded by `MAX_NORMALIZE_ITERATIONS = 2000`, `Board::split_trace(&mut self, id, clip: &IntOctagon) -> Result<Vec<ItemId>, BoardError>`. The `combine_at_end` "path 2 requires tree entries in the default tree" guard is ported as `Option` handling (no null). `normalize_suppressed_net_nos` support.

**Tests:** port `fixtures/CombineStackOverflowTest.java` (build the board programmatically — its DSN fixture can't be read until Plan 3; if the test is DSN-only, reproduce its geometry by hand from the fixture's wiring section and say so) and the board-dependent half of `PolylineTraceSplitTest`; add `normalize_depth_cap_returns_false_not_error`, `normalize_traces_terminates_within_2000`, `from_lines_err_propagates_as_BoardError_Normalization` (construct the quirk-#22 line set through a trace and assert the error surfaces — Plan 1 ruling).

Commit `feat(board): trace normalisation with Java depth/iteration caps`.

---

### Task 10: `ShapeSearchTree` (angle-parameterised) and `SearchTreeManager`

**Files:** `src/searchtree/{shape_search_tree.rs,manager.rs}`.
**Java:** `board/searchtree/{ShapeSearchTree,ShapeSearchTree45Degree,ShapeSearchTree90Degree,SearchTreeManager}.java`.

**Interfaces produced:**
- `ShapeSearchTree { id: TreeId, angle: AngleRestriction, tree: ShapeTree<TreeObject>, compensated_clearance_class: usize }` — `bounding_directions()` (Orthogonal for 90°, FortyFive otherwise), `calculate_tree_shapes(&Item, &BoardRules, &BoardLibrary, &Components) -> Vec<RegularTileShape>` dispatching per item kind with the 45°/90° overrides (drill items and areas forced to octagons in 45°; boxes in 90°; `offset_shapes` vs `offset_box` for traces), `insert_item(&mut self, &mut Item, …)` (computes shapes, inserts leaves, stores `TreeEntries` on the item header), `remove_item(&mut self, &mut Item)`, `overlapping_objects(shape, layer, items: &ItemMap) -> BTreeSet<TreeObject>`, `overlapping_tree_entries(shape, layer, …)`, `overlapping_tree_entries_with_clearance(shape, layer, ignore_net_nos, clearance_class, items, rules, entry_counter: &mut u64) -> Vec<TreeEntry>` porting the 1.2×-enlarge + sort-by-clearance-then-entry-id + half-enlarge walk exactly, `overlapping_items_with_clearance`, `change_entries`, `merge_entries_in_front/at_end`, `reuse_entries_after_cutout`, `change_item_shape`, `offset_shapes`. `complete_shape`/`divide_large_room` (expansion rooms) are **Plan 6** — leave `// added in Plan 6:` markers.
- `SearchTreeManager { default_tree: ShapeSearchTree, compensated: Vec<ShapeSearchTree>, clearance_compensation_used: bool, next_entry_id: u64 }` — `new(&BoardRules)`, `get_default_tree()`, `get_autoroute_tree(clearance_class, &BoardRules) -> &mut ShapeSearchTree` (creates the 90°/45°/base tree per `rules.trace_angle_restriction`), `insert(&mut Item, …)` into every tree + `on_the_board = true`, `remove(&mut Item)`, `reinsert_tree_shapes(&mut Item)`, `is_clearance_compensation_used()`, `set_clearance_compensation_used`.

**Tests:** build a 2-layer board with two pins and a trace (helper from `tests/board_builder.rs`, created here); assert `overlapping_objects` on the trace's bounding box returns the pins + trace in `(id, shape_index)` order; `overlapping_tree_entries_with_clearance` excludes same-net pins when their net is in `ignore_net_nos`; 90° manager builds an `IntBox`-keyed tree, 45° an octagon-keyed one (inspect `leaf_bounds`); the entry-id tie-break is monotonic across two queries.

Commit `feat(board): ShapeSearchTree and SearchTreeManager`.

---

### Task 11: `Board` — fields, insert/remove protocol, queries, connectivity, communication, changed area

**Files:** `src/board/{mod.rs,connectivity.rs,communication.rs,changed_area.rs}`, `tests/board_builder.rs` (extend).
**Java:** `board/facade/{BasicBoard,BoardItemRepository,BoardConnectivityQueries,RoutingBoardOperations (non-shove),RoutingBoardSearchFacade}.java`, `board/facade/RoutingBoard.java` (non-shove fields), `board/state/{ChangedArea,Communication}.java`, `board/searchtree/ShapeTraceEntries.java`, `board/model/structure/{ShapeEntrySide,ShapeAndEntrySide}.java`.

**Interfaces produced:**
- `Board { items: BTreeMap<ItemId, Item>, components, rules: BoardRules, library: BoardLibrary, layer_structure, communication: Communication, bounding_box: IntBox, trees: SearchTreeManager, revision: u64, min/max_trace_half_width, changed_area: Option<ChangedArea>, failure_log: Vec<String> /* hook; Plan 6 types */, shove_failing_obstacle: Option<ItemId>, shove_failing_layer: i32 }`.
- Construction: `Board::new(outline_shapes: Vec<PolygonShape>, bounding_box, layer_structure, rules, library, communication)` inserting the `BoardOutline` item exactly as `BasicBoard`'s constructor does.
- Insert/remove exactly as `BoardItemRepository.insertItem`/`removeItem`: clamp clearance class → assign id if 0 → `items.insert` → `trees.insert` → `revision += 1`; remove: bail on `is_deletion_forbidden` → `trees.remove` → `items.remove`. Typed constructors: `insert_trace(polyline, layer, half_width, net_nos, clearance_class, fixed_state) -> ItemId`, `insert_via(...)`, `insert_pin(...)`, `insert_obstacle(...)`, `insert_conduction(...)`, `insert_component_outline(...)`.
- Queries: `get(ItemId) -> Option<&Item>`, `get_mut`, `items()` (id order), `overlapping_objects(shape, layer)`, `overlapping_items_with_clearance(shape, layer, ignore_nets, cl_class)`, `clearance_value(c1, c2, layer)`, `get_trace_half_width(net, layer)`, `get_connectable_items(net)`, `get_items_of_net(net)`, `get_component_items(component_no)`, `get_pins`, `get_vias`, `get_traces` — port `BasicBoard`'s public query set.
- Connectivity (`BoardConnectivityQueries` + the board-dependent `Item` methods): `normal_contacts(id) -> BTreeSet<ItemId>`, `connected_set(id, ignore_areas) -> BTreeSet<ItemId>`, `unconnected_set(id)`, `connection_items(id)`, `first_common_layer`, `is_tail(id)`, `normal_contact_point(a, b)`, `clearance_violations(id)`? (DRC — Plan 5; leave marker), `ratsnest_corners`.
- `check_trace_segment` (`RoutingBoardSearchFacade`) with `ShapeTraceEntries` + `ShapeEntrySide`/`ShapeAndEntrySide` ported (they are needed by `check_trace_segment` and by Plan 7's shover).
- `ChangedArea` (per-layer `IntBox` accumulation, `join`, `get_area(layer)`, `remove_items`?), `RoutingBoardOperations`' non-shove parts: `mark_changed_area(shape, layer)`, `remove_items(ids) -> ChangedArea`, `set_changed_area_layer_count`.
- `Communication { unit: Unit, resolution: i32, id_gen: ItemIdGenerator, host_cad: String, host_version: String }` (no observers, no parser info beyond host strings — DSN metadata is Plan 3's).
- `revision()`, `increment_revision()`.

**Tests:** port `board/BoardServiceCharacterizationTest.java` and `board/PinObstacleTest.java` (programmatic boards); insert/remove keeps tree in sync (`overlapping_objects` before/after); `remove` refuses `SystemFixed` items; `normal_contacts` for pin–trace–via chain; `connected_set` across layers through a via; `check_trace_segment` free vs blocked.

Commit `feat(board): Board with insert/remove protocol, queries, connectivity`.

---

### Task 12: Snapshots and the board hash

**Files:** `src/board/snapshot.rs`; `tests/board_history.rs` (port `autoroute/BoardHistoryTest.java`'s hash/snapshot expectations that don't need the router).
**Java:** `board/facade/{RoutingBoardUndoFacade (deepCopy only),BoardSnapshotManager}.java`, `autoroute/BoardHistory.java` (read for the hash contract only).

**Interfaces produced:** `impl Clone for Board` (derive where possible; `ShapeTree` arena clones by value so `LeafId`s stay valid); `Board::deep_copy(&self) -> Board` = `clone()` + `clear_autoroute_scratch()` (`autoroute_info = None` on every item) + `finish_autoroute()` hook; `Board::structural_hash(&self) -> u64` — a deterministic hash over `(id, layer, half_width, polyline corners)` of every trace and `(id, layer range, center, padstack)` of every via, in id order (Java hashes the serialized byte stream; the port defines its own deterministic function — `// totalized:`-style note + quirks row: hash *values* are not comparable with Java, only same-board equality semantics are). `Snapshot`/`pop_snapshot` become `let snap = board.clone()` at the Plan-7 call sites — document.

**Tests:** `deep_copy_is_independent` (mutate copy, original unchanged, tree queries diverge), `deep_copy_clears_autoroute_scratch`, `hash_equal_for_equal_boards_and_differs_after_trace_change`, `hash_stable_across_clone`.

Commit `feat(board): deep_copy and structural board hash`.

---

### Task 13: `PlanarDelaunayTriangulation`

**Files:** `src/datastructures/delaunay.rs`.
**Java:** `datastructures/PlanarDelaunayTriangulation.java` (978 lines; sole caller `drc/NetIncompletes` — the ratsnest/airline computation Plan 5 needs).

**Interfaces produced:** `PlanarDelaunayTriangulation::new(corners: &[DelaunayCorner { object: ItemId, point: Point }]) -> Self`, `get_edge_lines() -> Vec<DelaunayEdge { start, end, length }>`, `check()` (Java's validation), the private edge-flip machinery ported verbatim (exact `Point` arithmetic; `FloatPoint::inside_circle` where Java uses it — cite lines).

**Tests:** 4 corners of a square → 5 edges (which diagonal Java picks — derive from its insertion order and flip rule; pin it); collinear triple; duplicate points; `check()` passes on a random 50-point set (LCG) and the edge count equals `3n - 3 - h` for the hull size `h` computed independently in the test.

Commit `feat(board): PlanarDelaunayTriangulation`.

---

### Task 14: Port-completeness audit to zero for the whole crate; markers; prelude; README

**Files:** `crates/fr-board/README.md`, `src/lib.rs` prelude, `// not ported:`/`// renamed:`/`// added in Plan N:` markers; `scripts/audit-port.sh` run over `board/model board/facade board/searchtree board/trace board/state rules core/library datastructures`.
**Rule:** every public Java method in scope is present, deferred with `// added in Plan 6:`/`7`/`5` (shove/forced/DRC/expansion-room members), or `// not ported: <reason>` (interactive/GUI/observer/serialization members), verified by grep of Java callers outside the ported packages. Iterate the audit to exit 0 without weakening it.

Commit `feat(board): audit to zero, prelude, README`.

---

### Task 15: Board-level differential harness (Java ↔ Rust) and consistency tests

**Files:** `scripts/differential/java/B1.java` (+ `B1.rs` in `scripts/differential/rust/src/bin/`), `crates/fr-board/tests/consistency.rs`.
**Method:** the Java driver constructs a `BasicBoard` programmatically against the **2.3.0 jar** (`javap` the constructors: `BasicBoard(IntBox, LayerStructure, PolylineShape[], int, BoardRules, Communication)` or whatever 2.3.0 has), builds a 2-layer library with 2 padstacks, inserts N random pins/vias/traces from a shared xorshift stream, runs `normalizeTraces`, then dumps: every item (`id, kind, layer(s), net, clearance class, bounding box, tile shape count and each tile's bounding box`), the default tree's `overlappingObjects` result for 50 random query boxes on each layer, and `overlappingTreeEntriesWithClearance` results (as `(id, shape_index)` lists) for 50 random shapes/classes. The Rust twin does the same through `Board`. `run.sh b1 <seed> <n>` diffs. Expected differences must be zero except documented quirks; record the baseline in the README table. **Note:** the Java `clearance` query's entry-id tie-break depends on the static counter — start both from 0 per process and document.

**Consistency tests** (`tests/consistency.rs`, LCG, no `rand`): insert/remove round-trips leave `leaf_count` and query results unchanged; `deep_copy` equal hash; 45°-tree leaf bounds contain the 90°-tree's for the same items; `normalize_traces` idempotent (second call changes nothing).

Commit `test(board): Java/Rust differential driver and consistency tests`.

---

### Task 16: Docs — quirks rows, hand-off, survey amendment

**Files:** `docs/java-quirks.md` (rows for: `MAX_NORMALIZATION_DEPTH` 16 vs AGENTS.md 34; `lastGeneratedEntryId` static → per-manager counter; `ConcurrentHashMap` item order (JVM-dependent) → id order; `ClearanceMatrix` J-then-I indexing; `ConductionArea` fill cache not ported; structural hash not byte-comparable; any Java bugs found in Tasks 2–13), `docs/plan-2-handoff.md` (rulings, parked residuals, obligations for Plans 3–8), `docs/geometry-library-survey.md` (amend: `i_overlay` not needed for routing), `docs/plan-1-handoff.md` (tick the Plan 2 obligations that this plan discharged: `PolylineError` → `BoardError::Normalization`; `equals_geometric` used at the trace sites; `border_line_index == None` handling in `ShapeAndEntrySide`; memo cache for convex pieces — implement as `absolute_area_cache`/`tile shapes cache` on items, or record as still open).

Commit `docs(board): quirks, hand-off, survey amendment`.

---

## Dispatch order and sizing

1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 10 → 11 → 9 → 12 → 13 → 14 → 15 → 16. Tasks 2, 5, 10, 11 are the large ones (opus); 1, 3, 4, 6, 7, 12, 13 medium (opus for 6/7/13 because of Java-fidelity reading; sonnet for 1/3/4/12); 14, 16 small (sonnet). Reviewers: opus for 2, 5, 8–13, 15; sonnet otherwise. Every review of Tasks 8–13 should run the Task-15 differential driver once it exists (Task 15 may be pulled forward to right after Task 11 if the reviewer wants it — the controller decides).

## Plan self-review

Spec coverage: §6 arena/enum/search-tree/clone → Tasks 5, 10, 11, 12; §5 no-`f64` invariant inherited via `fr-geometry`; §10 cancellation → `StopCheck`/`TimeLimit` (Task 3); §14 tests → ported Java tests per task + Task 15 harness. Deliberately excluded (with spec citation): shove/tighten/forced (§9, Plans 6/7), DRC (`clearance_violations`, Plan 5), DSN metadata (Plan 3), expansion rooms in the tree (Plan 6). Type names are fixed in the File Structure and Interfaces blocks; Task 9 is explicitly sequenced after 11 because it needs `Board`.
