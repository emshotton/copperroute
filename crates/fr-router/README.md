# fr-router

The autorouter. It routes, fans out and optimises a **whole board**:
`pipeline::run_pipeline` sequences the fanout pre-pass, the routing pass loop
and the optimizer, and everything under it — the maze search, its expansion
rooms, the shove and pull-tight families, the via optimizer — is this crate's
too. Use `fr_router::prelude::*` to bring in every public type.

The crate depends on `fr-board`, `fr-geometry`, `fr-settings`, `fr-drc` (for
the unrouted report and the score's clearance block) and `thiserror`, and on
nothing else. `fr-dsn` and `parity` are dev-dependencies, for fixtures.

## Entry points

```rust,ignore
pub fn run_pipeline(
    board: &mut Board,
    settings: &RouterSettings,
    stop: &RouterStop,
    budget: RouterBudget,
    progress: &mut dyn ProgressSink,
) -> Result<PipelineResult, RouterError>;
```

`prepare_board(&mut Board, &RouterSettings)` applies the two post-load
clearance overrides and is called once per loaded board, before
`run_pipeline`. `PipelineResult` carries the router and optimizer
`TaskState`s, the pass counts, the fanout summary, one `PassRecord` per
routing pass (`pass`, `score`, `incomplete_count`, `clearance_violations`,
`via_count`, `trace_count`), the final `BoardStatistics`, and whether any
stage's budget expired. `build_unrouted_report(&mut Board)` writes the
diagnostic text naming every incomplete connection.

Below the pipeline, `route_connection` routes one connection through an
`AutorouteEngine` — maze search, locate, delete the ripped connections,
insert — and `route_connection_full` adds the necked retry, the strict-DRC
rollback and the failure-log write around it.

## Layout

| Module | What it holds |
|---|---|
| `pipeline/run.rs` | `run_pipeline`, `PipelineResult`, `normalize_router_algorithm` |
| `pipeline/batch_loop.rs` | `AutorouteBatchLoop::run` — how many passes run, which board survives them, when to give up; `BatchLoopResult`, `BatchLoopExit`, the stagnation windows |
| `pipeline/pass_runner.rs`, `batch_autorouter.rs` | one routing pass: the work list (`BatchAutorouter::autoroute_items`, sorted ascending by airline length), the per-item loop, progress events |
| `pipeline/fanout.rs` | `BatchFanout::fanout_board` and `fanout_pass`, the SMD-pin escape pre-pass, with `FanoutComponent`/`FanoutPin` ordering and the `FanoutRunSummary` |
| `pipeline/optimizer.rs` | `BatchOptimizer::run_batch_loop`, `opt_route_pass`, `opt_route_item` — re-route one item at a time and keep the result only if the score improves, rolling back through `fr-board`'s undo journal otherwise |
| `pipeline/board_history.rs` | `BoardHistory`, the pass loop's best-board memory (up to 30 clones, ranked by score) |
| `pipeline/stop.rs` | `RouterStop` (the three-state stop flag), `RouterBudget` (the wall-clock knobs), `PassRecord`, `ProgressThrottler`, `DeterministicWorkBudget` |
| `pipeline/airline.rs`, `counters.rs`, `failure_log.rs`, `item_route_result.rs`, `unrouted_report.rs`, `board_prep.rs` | airline distance, `RouterCounters`, `RoutingFailureLog`, `ItemRouteResult`, the unrouted report, `prepare_board` |
| `autoroute/maze/` | `AutorouteEngine` (the room lifecycle), `MazeSearchEngine` (the search frame, `expand.rs` its body), `MazeExpansionEngine` (drill and layer expansion), `MazeRipupResolver` (the ripup decision and cost), `MazeTraceShover` (the check-only shove probe), `AutorouteControl`, `DestinationDistance`, `MazeListElement`, `MazeQueue` |
| `autoroute/expansion/` | the expansion rooms (`FreeSpaceExpansionRoom`, `IncompleteFreeSpaceExpansionRoom`, `CompleteFreeSpaceExpansionRoom`, `ObstacleExpansionRoom`), doors (`ExpansionDoor`, `TargetItemExpansionDoor`), the three neighbour sorters (any-angle, 45°, orthogonal), and `ExpansionRoomStore`, the arenas they live in |
| `autoroute/drill/` | `DrillPage`, `DrillPageArray`, `ExpansionDrill` — the pages that manufacture the search's layer changes |
| `autoroute/path/` | `FoundConnectionLocator` (three angle regimes) turns a `MazeResult` into corner lists; `FoundConnectionInserter` inserts them; `Connection` |
| `autoroute/tree_ext.rs` | `AutorouteSearchTreeExt` — `complete_shape` and `divide_large_room`, the two search-tree operations only the router needs |
| `board_ext/` | `RoutingBoardExt`, the extension trait over `fr_board::Board` for everything that needs an engine or a shove: `init_autoroute`/`finish_autoroute`, `insert_forced_trace_polyline`/`_segment`, `opt_changed_area`, `remove_items_and_pull_tight`, `fanout`; and the classes it drives — `TraceShover`, `DrillItemMover`, `ForcedPadRouter`, `ForcedViaInserter`, `ViaOptimizer`, and the pull-tight family in `tightener/` (base, 90°, 45°, any-angle) |
| `score/` | `BoardStatistics` and its blocks, `normalized_score`, the JSON DTOs |
| `visualization.rs` | opt-in SVG capture of the maze search: `RoutingVisualizationOptions`, `start_routing_visualization`, the `RoutingVisualizationGuard` whose `finish()` returns a `RoutingVisualizationSummary`. When no recorder is active the capture hooks the search calls cost one relaxed atomic load; `docs/routing-visualizer.md` describes the frames and the viewer |
| `arena.rs` | `Arena<T>` and its index newtypes |
| `error.rs` | `RouterError` |

## The stop machine and the budget

`RouterStop` is a **three**-state flag, not a boolean, and the distinction
is load-bearing:

| write | state | effect |
|---|---|---|
| `request_stop()` | `ALL` | the router **and** the optimizer stop |
| `request_stop_auto_router()` | `NONE -> AUTO_ROUTER_ONLY` | the router stops between connections; **the optimizer still runs** |

`is_stop_requested()` is `== ALL` and gates the optimizer stage;
`is_stop_auto_router_requested()` is `!= NONE` and is what the pass loop and
the item loop read. Every ordinary exit from the pass loop — the pass cap,
"cannot improve", both stagnation windows — raises `AUTO_ROUTER_ONLY`, so
the optimizer runs after a normal routing run and is skipped only after a
full cancel. Nothing resets the flag between stages; `run_pipeline` hands
both stages the same `RouterStop`.

`RouterBudget` holds the wall-clock knobs: `opt_changed_area_ms` (the
tightener's time limit, default 1000), `fanout_ms_per_pin`,
`board_update_throttle_ms` and `progress_throttle_ms`.
`RouterBudget::disabled()` turns every one of them off, which is what every
reproducibility test passes — a run with a live budget is a function of
machine speed. `RouterStop::poll_deadline` has exactly one production call
site, the job-level check at the head of the pass loop; the per-stage clocks
(fanout, optimizer) write only their own `is_timed_out` and never the stop
flag.

## House rules

* **Single-threaded.** No `rayon`, no `std::thread`. `Board` stays
  `Send + Sync`, but nothing here spawns, and `max_threads` is read by
  nothing. A threaded maze would be non-deterministic and would dissolve
  every reproducibility test in the tree.
* **No new workspace dependencies.** Not `rand` (see `SplitMix64` in
  `fr-geometry`), not `slotmap` (see `Arena` below), not `rayon`.
* **Deterministic containers.** The maze queue is a sorted set (`BTreeSet`),
  never a `BinaryHeap`: the search pops the least element and re-inserts
  mutated elements. Every sorted container is a `BTreeSet`, which is sound
  because every comparator is a total order: float keys compare with
  `total_cmp`, giving `NaN` a fixed position rather than letting it sort
  arbitrarily. That covers the neighbour sorters' `SortedRoomNeighbour`,
  `MazeListElement`, `BatchFanout`'s pin order and the queue — each orders and
  dedups identically on every run. `MazeQueue::push` is guarded: an element
  outside the fanout escape window is refused, and the boolean is discarded at
  the one site that discards it, so `init` can succeed with an empty queue.
* **No GUI, no logging, no observers, no clock, and no static mutable state,
  with one recorded exception:** `fr_geometry::Line`'s identity token, which
  `Board::change_trace` reads through `Line::is_same_object` to decide how
  many search-tree leaves to reuse. The contract: a new token wherever a new
  `Line` is built, `Copy` — the same token — wherever the same line is passed
  on. Nothing reads the token's *value*, so no output, ordering, hash or
  serialised form can observe it.
* **`#![forbid(unsafe_code)]`**, as in every workspace crate.

## Cancellation: six sites, and why a seventh is a bug

`AutorouteEngine::is_stop_requested` — the per-connection time limit or the
caller's stop flag — is consulted at exactly six places: four inside
`MazeSearchEngine::init` (the destination-set walk and three seeding loops),
the head of the pop loop before the queue is touched, and
`DrillPage::get_drills`' convex split. Each has a test. Adding a seventh
makes a run stop earlier than the loop shape implies, which changes the
board: the divergence is not "we stopped sooner", it is "we routed something
else". Two consequences:

* **The inserter is handed `&|| false`.** Nothing below the maze search
  checks cancellation; threading the caller's check into
  `FoundConnectionInserter` would abort inserts that should complete, after
  the ripped items have already been removed — a worse board than either
  outcome.
* **`Board::split_traces_checked` consults the check once**, inside the entry
  walk that may not terminate, not once per picked trace.

The per-connection wall clock belongs to `route_connection`, which builds a
`TimeLimit` from the settings; `AutorouteBatchLoop` owns everything outside
that.

## The recovery boundaries

Panics are the error channel for geometry that has no meaningful degraded
value, and the router catches them at exactly these boundaries:

| # | site | mechanism | what it answers |
|---|---|---|---|
| 1 | `AutorouteEngine::complete_expansion_room` | `Result`, and **`Err` means an empty collection** — every caller uses `unwrap_or_default()`, never `?` | no rooms |
| 2 | maze construction in `autoroute_connection` | `catch_unwind` | `FAILED` |
| 3 | `find_connection` | `catch_unwind`, nested inside 2 because the search borrows the engine for its whole life | `FAILED` |
| 4 | the locator | `catch_unwind` | `FAILED` |
| 5 | the whole of `route_connection` | `catch_unwind` | a bare `FAILED` |
| 6 | `insert_forced_trace_polyline`'s normalise and split | the `Result` channel: the `Err` is dropped and the trace stays as it was | a silent skip |
| 7 | `AutoroutePassRunner::run_single_thread` | `catch_unwind` around the whole pass body, degrading to `Ok(false)` — **not** per item: a failure inside the item loop ends the pass rather than skipping one item | the pass ends |
| 8 | `AutorouteBatchLoop::run` when no layer is both active and a signal layer | the **only one that propagates**: fires `TaskState::Cancelled`, then returns `RouterError::NoRoutableLayer` | an error |

The boundaries catch panics, not stack overflows. Everything *below* the
maze search deliberately panics rather than degrading, because the necked
retry in `route_connection_full` is the real recovery. `panic = "abort"`
must stay unset in the release profile: `AutorouteEngine::drill_page_drills`
restores the drill-page grid through `catch_unwind`/`resume_unwind`.

## `Arena<T>`, and why not `slotmap`

The autoroute object graph is cyclic: a room holds its doors, a door holds
both of its rooms, a drill holds one room per layer. `Rc<RefCell<…>>` would
make the engine's borrow of a `Send + Sync` `Board` unrepresentable, so every
room, door, drill and page lives in an `Arena<T>` — a `Vec<Option<T>>`
addressed by a `u32` index — and the newtypes `RoomId`, `ObstacleRoomId`,
`IncompleteRoomId`, `DoorId`, `TargetDoorId`, `DrillId`, `PageId` and
`ConnectionId` are those indices.

It has **no generation counter**, deliberately. `Arena::remove` leaves a
permanent hole and **indices are never reused**: `insert` always appends, so
an index stays valid, or permanently empty, for the arena's whole life. A
long run's arena grows to the total number of objects ever created rather
than the live count. `Arena::clear` drops the indices as well and is only
sound between connections, when no id from the old arena survives.
`DrillPage::invalidate` frees its own drill ids, because pages are
invalidated once per changed item and leaving the slots would grow the arena
for the whole run.

## The maze search in outline

1. `AutorouteControl::from_settings` copies the numbers the search reads —
   trace half widths and costs per layer, via costs and masks, ripup costs,
   the fanout escape window — so the search never touches `RouterSettings`.
2. `MazeSearchEngine::init` seeds the queue with the start items' expansion
   rooms and target doors. A seed refused by the escape window is dropped.
3. The pop loop takes the cheapest `MazeListElement` — ordered by sorting
   value, then expansion value, then door id — and expands it: through its
   room's doors (`expand.rs`), to a drill page or a drill (`MazeExpansionEngine`),
   or through a rippable obstacle (`MazeRipupResolver`, which prices the
   ripup from the obstacle's half width, its detour and the pass number, with
   a pseudo-random scaling from pass 4 on). The A\* cost is the expansion
   value plus a bend penalty (charged above `sin² > 0.01`, about 5.7°) plus
   `DestinationDistance`'s lower bound.
4. `FoundConnectionLocator` walks the backtrack chain into corner lists per
   layer, in the board's angle regime.
5. `FoundConnectionInserter` derives the vias from the layer changes, inserts
   each corner run through `insert_forced_trace_polyline` (which shoves,
   springs over obstacles and pull-tightens), connects the ends, and
   normalises.

`ExpansionDoor`'s id is a hash of its two rooms' ids, not an identity, and it
is the third sort key of the queue; the same is true of `ObstacleExpansionRoom`
and `IncompleteFreeSpaceExpansionRoom`. Only `CompleteFreeSpaceExpansionRoom`
carries a true counter id. Complete-room ids **skip** — the counter ticks once
per neighbour calculation, including ones that build no room — and the arena
index is a different number; both are minted in creation order.

## The pass loop, fanout and optimizer in outline

**The work list** is every routable item with an unconnected set, in board
order, sorted ascending by airline distance. A two-net item that qualifies
on both nets enters the list twice and is routed on every net each time.

**`AutorouteBatchLoop::run`** repeats passes until the pass cap, until a
pass routes and fails nothing, until the score stops improving over the
stagnation window, or until the stop flag rises. `BoardHistory` remembers
the best boards seen; the loop restores from it mid-run when a pass makes
the board worse and swaps the best board in at the end, so the board written
out is the *best* pass's, not the *last* pass's. `restore` is a
`Board::deep_copy` — memoised caches reset, the item-id counter preserved.

**`BatchFanout`** runs first when enabled: components sorted by pin count
descending then id, each SMD pin escaped through `RoutingBoardExt::fanout`
under a per-pin time budget, up to a fixed number of passes or until a pass
changes nothing.

**`BatchOptimizer::run_batch_loop`** runs after routing unless the stop flag
is `ALL`. Each pass visits the routed items in a fixed order, re-routes each
one with its trace removed, and keeps the result only if the board score
rises; otherwise it rolls the item back through `Board::undo_from_snapshot`,
which replays the attempt's changes back through the live search trees. A
pass that improves nothing ends the stage.

`route_connection_full`'s tail — `opt_changed_area` (the tightener sweep
over the region a connection changed), the necked retry with a narrower
trace, and the strict-DRC rollback that removes a connection whose insert
created a violation — runs after every connection.

Two guards on the fanout path are the router's own rather than inherited
behaviour: the micro-neckdown fallback never emits a trace narrower than
the design's minimum net-class width, and the work list is airline-sorted.
`benchmark/reports/java-regressions-2026-09.md` measures why.

## The score

`score::BoardStatistics` is the per-board metric block: board, layers,
items, components, pads, nets, connections (with the incomplete count),
traces (total length, per layer), bends, vias, clearance violations (from
`fr-drc`) and fanout. `normalized_score` folds it into one number from the
`ScoringSettings` weights; the pass loop, the optimizer and `BoardHistory`
rank boards by it, and the result manifest and DRC report carry it.

## Tests

`cargo test -p fr-router` runs the unit tests plus fifty-odd integration
suites, one per subject; the file names in `tests/` say which. Three lanes
are worth knowing:

* **Synthetic boards** — the majority. Small hand-built boards in each
  suite, no external files.
* **Committed transcripts** — `tests/data/*.txt`, replayed line by line by
  the suite of the same name (`engine_rooms`, `maze_search`, `maze_expand`,
  `maze_drills`, `locator`, `inserter`, `board_ext`, `tightener`,
  `forced_via`, `via_optimizer*`, `opt_changed_area`, `connection_to_pin`,
  `board_history`, `fanout*`, `optimizer*`, `pass_runner`,
  `clearance_override`, …). Each transcript records the expected state after
  every step of a scripted scenario.
* **Whole-board references** — `reference_parity.rs` routes each stem of
  `tests/reference/router-fixtures.txt` connection by connection and
  compares every attempt's state, geometry and metrics against
  `tests/reference/<stem>/router.jsonl`; `batch_parity.rs` runs the whole
  pipeline on each stem of `tests/reference/cli-fixtures.txt` and compares
  the per-pass `PassRecord`s and the final session bytes against
  `batch.passes.jsonl` and `batch.ses`. Four stems run in CI; the slow
  ones are `#[cfg_attr(debug_assertions, ignore)]` and run under
  `FR_SLOW_PARITY=1 cargo test --release`. `fixtures.rs` is the routing
  smoke test over the same boards; `strict_drc.rs` pins that the strict-DRC
  rollback adds no violation to a board that starts with some.
  `fanout_tie_break.rs` routes a hand-written four-pad board whose fanout
  has an exact distance tie and pins the outcome, because no corpus board
  contains such a tie.

The reference lanes need the fixture corpus in a sibling `../freerouting`
checkout (`FREEROUTING_JAVA_DIR` overrides the location) and skip with a
printed message without it. Every reproducibility test passes
`RouterBudget::disabled()`.

```sh
cargo test -p fr-router                                   # the CI lane
FR_SLOW_PARITY=1 cargo test -p fr-router --release        # plus the slow stems
scripts/gen-batch-reference.sh                            # regenerate tests/reference/<stem>/batch.*
scripts/gen-router-reference.sh                           # regenerate tests/reference/<stem>/router.*
```

Both generators write a `meta.txt` per stem recording how the reference was
cut; `tests/reference/README.md` explains the lanes and when a family is
re-cut.
