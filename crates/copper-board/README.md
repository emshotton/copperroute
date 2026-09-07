# copper-board

The board model: items, design rules, the component/net/padstack library,
the search trees that index items geometrically, and `Board` itself —
everything a PCB design needs *before* routing runs. Use
`copper_board::prelude::*` to bring in every public type.

The crate sits on `copper-geometry` and depends on `num-bigint` and `thiserror`
besides. It must not depend on `tracing`: invariant guards are
`debug_assert!`s, and there is no diagnostic logging.

## Layout

| Module | What it holds |
|---|---|
| `board/` | `Board` and its operations, split by subject: insertion and removal (`mod.rs`), connectivity queries (`connectivity.rs`), clearance queries (`clearance.rs`, `query.rs`), trace normalisation (`normalize.rs`, `trace_normalize.rs`), the changed-area tracker (`changed_area.rs`), the trace-entry bookkeeping shoves use (`shape_trace_entries.rs`), snapshots and the one-level undo journal (`snapshot.rs`), the two clearance overrides applied after a load (`clearance_override.rs`), and `Communication` — the coordinate unit and resolution the board was read in |
| `items/` | the `Item` enum and its nine variants, `ItemHeader`, `ItemCtx`, `PolylineTrace`, the drill items (`Pin`, `Via`), the area items, `AutorouteInfo` (the router's per-item scratch, kept as ids), `ClearanceViolation` |
| `structure/` | `LayerStructure`/`Layer`, `Components`/`Component`, `BoardOutline`, `AngleRestriction`, `FixedState`, `Unit`, `ShapeEntrySide` |
| `rules/` | `BoardRules`, `ClearanceMatrix`, `Nets`/`Net`, `NetClasses`/`NetClass`, `ViaInfos`/`ViaInfo`/`ViaRule`, `DrcConstraints`/`DrcSeverity` |
| `library/` | `BoardLibrary`, `Packages`/`Package`/`PackagePin`, `Padstacks`/`Padstack`, `LogicalParts`/`LogicalPart`/`PartPin`, `Keepout` |
| `searchtree/` | `ShapeSearchTree` (a compensated search tree of item shapes on one clearance class), `SearchTreeManager` (one tree per clearance class in use) |
| `datastructures/` | `ShapeTree` (the minimum-area tree under the search trees), `PlanarDelaunayTriangulation`, `TimeLimit`/`StopCheck` |
| `ids.rs` | the newtype ids: `ItemId`, `RoomId`, `ObstacleRoomId`, `ConnectionId`, `DrillId`, `TreeId`, `NetClassId`, `ViaInfoId`, `ViaRuleId`, `PadstackId`, and `TreeObject { Item, Room }`, the key type a search tree stores |

### `Board`

`Board` owns `items: BTreeMap<ItemId, Item>`, `components`, `rules`,
`library`, `communication`, `bounding_box`, `trees: SearchTreeManager`, the
optional `changed_area`, and the shove-failure scratch
(`shove_failing_obstacle`, `shove_failing_layer`). It is `Send + Sync +
Clone`. There is no undo/redo stack: callers that need to roll back clone
the board (`Board::clone` for a shallow snapshot, `Board::deep_copy` for one
whose memoised caches are reset). The one exception is the single undo level
the optimizer opens around each item it tries to improve
(`begin_undo_journal`, `discard_undo_journal`, `undo_from_snapshot`, over
`board::snapshot::UndoJournal`), which replays the attempt's item changes
back through the **live** search trees so the trees end up in the shape they
would have had without the attempt.

### The `Item` enum

`Item`'s nine variants are `Trace`, `Via`, `Pin`, `ObstacleArea`,
`ConductionArea`, `ViaObstacleArea`, `ComponentObstacleArea`,
`ComponentOutline` and `BoardOutline`. Every variant shares one `ItemHeader`:
id, layer set, clearance class, fixed state, net numbers, component number.
The inherent `impl Item` block in `items/mod.rs` is the common method table,
dispatching on the variant.

### `ItemCtx`

Items hold no pointer back to their board. `ItemCtx` bundles the four
immutable references an item method needs — library, rules, components,
bounding box — and is passed as a parameter; `crate::board::item_ctx!` builds
one from a `&Board`. It deliberately excludes `items` and the search trees,
so an item method holding an `ItemCtx` cannot alias `Board::items` while
`Board` iterates it.

## Invariants

- **Descending item iteration.** `Board::items` is a `BTreeMap`, which
  iterates ascending; every walk whose result depends on visiting order —
  search-tree rebuilds, writers, DRC — goes through
  `Board::items_in_board_order` (`items.values().rev()`), so items are always
  visited newest-first. This is load-bearing: `ShapeTree`'s insertion
  heuristic makes the resulting tree shape a function of insertion order, and
  the tree shape decides the order the router meets obstacles in.
- **`OnceLock`, not `Cell`/`RefCell`, for lazy memo fields** (precalculated
  areas, convex-piece caches, drill-item tile-shape caches). Accessors stay
  `&self` and the type stays `Send + Sync` without a lock, matching how the
  board is used: never mutably aliased while being read.
- **`ShapeTree::insert_tiles` returns `Vec<Option<LeafId>>`.** A shape with no
  bounding tile shape under the tree's angle restriction inserts nothing at
  that index but keeps the list the same length as the shape list. Do not
  `.flatten()` it away: callers zip it against the original shape list.
- **`Polyline::from_lines` errors propagate.** Any operation that re-runs the
  normalising `Polyline` constructor (`translate_by`, `turn_90_degree`,
  `change_placement_side`, trace normalisation, `PolylineTrace::split*`)
  threads the `Result` out rather than emptying the trace on failure. Inside
  `Board::normalize_trace` an `Err` becomes `BoardError::Normalization`.
- **Stop checks on the walks that may not terminate.** Trace splitting and
  normalisation can loop on pathological ladders of same-net traces, so
  `Board::insert_via_checked`, `insert_escape_via_checked` and
  `split_traces_checked` take a `StopCheck` (`&dyn Fn() -> bool`) and answer
  `BoardError::Stopped` when it trips. The unchecked forms pass `|| false`.
- **Clearance matrix values round up to even** on write
  (`ClearanceMatrix::set_value`), and the matrix is not required to be
  symmetric; readers ask for the `(class_a, class_b, layer)` triple they mean.

## Post-load overrides

`Board::apply_copper_to_edge_clearance_override` and
`Board::apply_hole_clearance_override` (`board/clearance_override.rs`) are
applied once after a design is loaded, by `copper_router::pipeline::prepare_board`.
The first appends a `board_edge` clearance class carrying the configured
copper-to-edge clearance (default 500 µm) and re-points the outline at it
unless the outline already carries an explicit class; the second does the same
for hole keepouts when a hole clearance is configured. The order is
load-bearing, because both append a class and `board_edge` gets the lower
index.

## Tests

`cargo test -p copper-board` runs the unit tests plus twenty integration suites,
one per subject: `areas_and_outlines`, `autoroute_info`, `board_builder`,
`board`, `clearance_matrix`, `clearance_override`, `clearance_violations`,
`component`, `conduction`, `consistency`, `drill_items`,
`expansion_room_tree`, `layer_structure`, `min_area_tree`, `polyline_trace`,
`search_tree`, `shape_trace_entries`, `snapshot`, `trace_normalize`.

`consistency.rs` is the property suite: insert/remove round trips,
`deep_copy`, 45- versus 90-degree tile shapes (compared with
`TileShape::contains_tile`, since both angle families compute the same
axis-aligned bounding box), `normalize_traces` idempotence on a fixture that
needs normalising, descending item iteration, and
`Board: Send + Sync + Clone`. `snapshot.rs` pins what `structural_hash`
covers — the whole item graph, walked in board order — and what it
deliberately leaves out (on-demand caches whose state records how often the
board has been measured rather than what it is).

Tests that read a board fixture take it from `tests/data/` or, for the larger
boards, from `tests/corpus`.

## Conventions this crate shares with the workspace

**`#![forbid(unsafe_code)]`** sits in the crate root, as it does in every
workspace crate.
