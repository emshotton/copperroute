# Freerouting Rust Port — Design Spec

**Date:** 2026-08-27
**Status:** Approved for planning

## 1. Goal

A behavioral port of the freerouting PCB autorouter (Java, v2.3.x) to Rust, with
**no GUI**. Two surfaces only:

1. A headless CLI (`freerouting route|drc|info`, plus a legacy `-de/-do` shim).
2. A native stdio MCP server (`freerouting mcp`).

Primary use case: the KiCad workflow — KiCad exports Specctra DSN, freerouting
routes it, KiCad imports the resulting SES.

Reference implementation: the Java clone at `../freerouting` (sibling folder).
Its `fixtures/` (148 `.dsn` + companions) and `src/test/java/.../fixtures/`
regression tests define acceptance.

## 2. Scope

### In scope (v1)
- Read: Specctra DSN, `.rules`, SES (for incremental re-route).
- Write: SES, DSN, result-manifest JSON, DRC report (KiCad-schema JSON).
- Routing pipeline: fanout → autoroute passes → optimizer passes, all three
  angle regimes (90°, 45°, any-angle), ripup/reroute, pull-tight, shove,
  via optimisation, multithreaded optimizer.
- DRC: clearance violations, net incompletes, unconnected items.
- Settings: `RouterSettings` with layered merge
  defaults → DSN `(autoroute_settings)` → SES → `.rules` → env → CLI.
- Cancellation (timeout, MCP cancel) and progress events.

### Out of scope
- GUI, interactive editing state, undo store (`UndoableObjects`),
  `BoardHistory` beyond logging, graphics dirty rectangles.
- `.frb` binary snapshots, Eagle `.scr`, KiCad session JSON output.
- REST API, HTTP/SSE/WebSocket MCP transports, sessions/job queue.
- Analytics/telemetry, version check, persistent `freerouting.json` in a
  user-data dir, log files on disk.
- `src_v19/` (frozen historical tree) — reference only.

## 3. Parity contract

- **Bit-parity** (byte-identical after whitespace normalisation) for:
  DSN read → DSN write; DSN read → SES write (unrouted); canonical board-info
  JSON dump; DRC violation list (sorted).
- **Metric parity** for the router, per fixture: nets completed ≥ Java
  reference, zero DRC violations, via count and total trace length within a
  tolerance (default ±10%) of the Java reference metrics.
- Single-threaded runs with a fixed seed are deterministic run-to-run.

## 4. Workspace layout

```
freerouting-rs/
  Cargo.toml                 # workspace, edition 2024
  crates/
    fr-geometry/             # planar geometry, exact-arithmetic fallback
    fr-board/                # items, layers, components, rules, search tree, board facade
    fr-dsn/                  # DSN lexer/parser/writer, .rules reader, SES reader/writer, CoordinateTransform
    fr-drc/                  # DesignRulesChecker, ClearanceViolation, NetIncompletes, KiCad JSON report
    fr-router/               # maze engine, expansion rooms, drill pages, batch fanout/autoroute/optimizer, tighteners, shover, via optimizer
    fr-settings/             # RouterSettings (Option<T> fields), layered merge, env/CLI/DSN/SES/rules sources
    fr-core/                 # RoutingPipeline, CancelToken, ProgressSink, RoutingResult, BoardStatistics, result manifest
    freerouting/             # binary: clap CLI + legacy shim + stdio MCP
  tests/
    parity/                  # harness code
    reference/               # Java-generated reference outputs + metrics.json
  scripts/gen-reference.sh   # regenerates tests/reference from ../freerouting jar
```

Dependency direction (strict): `freerouting → fr-core → fr-router → fr-drc →
fr-board → fr-geometry`; `fr-dsn` and `fr-settings` depend only on `fr-board`
and `fr-geometry`.

External crates: `clap`, `serde`, `serde_json`, `schemars`, `num-bigint`,
`num-rational`, a polygon-boolean crate (`i_overlay` or `geo`), `rayon`,
`rand` (seeded `StdRng`), `slotmap`, `thiserror`, `tracing`.

## 5. Geometry (`fr-geometry`)

Faithful port of `geometry/planar`:
`IntPoint`, `IntVector`, `IntDirection`, `Line`, `LineSegment`, `Polyline`,
`TileShape` trait with `IntBox` / `IntOctagon` / `Simplex`, `PolygonShape`,
`PolylineArea`, `Circle`, `Ellipse`, `FloatPoint`, `FloatLine`, `Limits`.

- Coordinates are `i32`; intermediates `i64`/`i128`.
- Overflow beyond `Limits` promotes to `RationalPoint` / `RationalVector` /
  `BigIntDirection` backed by `num-bigint`, exactly where Java does.
- **No `f64` in intersection/containment paths.** `FloatPoint` only where
  Java uses it (heuristic distances, angle checks).

## 6. Board model (`fr-board`)

- `Board` owns `items: SlotMap<ItemId, Item>`. All cross-references are
  `ItemId`; no back-pointers.
- `Item` is an enum: `Trace(PolylineTrace) | Via | Pin | ObstacleArea |
  ConductionArea | ComponentOutline | ViaObstacleArea`, with a shared header
  (`id`, `net_nos`, `clearance_class`, `fixed_state`, layer range, `component`).
- Search tree: one `ShapeSearchTree` (min-area-increase R-tree, ported from
  `datastructures/ShapeTree` + `MinAreaTree` + `board/searchtree`) with
  `AngleRestriction` selecting the tile shape. The Java 90°/45°/any parallel
  class hierarchies collapse into `match` on the enum. Entries keyed by
  `(ItemId, shape_index)`; rebuilt per item on insert/remove.
- Search tree lives inside `Board`; all mutation goes through `&mut Board`
  methods that keep it in sync. No interior mutability.
- `Board: Clone` replaces `RoutingBoard.deepCopy()` (serialization) for
  optimizer workers and pass snapshots.
- Rules: `BoardRules`, `ClearanceMatrix`, `Net(s)`, `NetClass(es)`, `ViaInfo`,
  `ViaRule`, `DefaultItemClearanceClasses` ported directly.
- Copper pours (`ConductionArea`): polygon boolean ops via a Rust crate
  instead of `java.awt.geom.Area`; parity is metric (area, point containment).
- `PolylineTrace::normalize` ports the documented depth limit and the
  `combine_at_end` invariants from AGENTS.md §Trace Normalisation.

## 7. I/O (`fr-dsn`)

- Hand-written lexer ported from `SpecctraDsnStreamReader`; parser ported
  from `io/specctra/parser/*` (`Structure`, `Network`, `Wiring`, `Library`,
  `Package`, `Component`, `Shape`, `Rule`, `NetClass`, `AutorouteSettings`).
- `CoordinateTransform` for DSN unit ↔ board integer units.
- Writers: `DsnWriter`, `SesWriter`. Readers: `SesReader`, `RulesReader`.
- Output formatting matches Java byte-for-byte (same number formatting,
  ordering, indentation) — this is what bit-parity tests check.

## 8. DRC (`fr-drc`)

Port of `DesignRulesChecker` (`get_all_clearance_violations` is the source of
truth, not statistics counters), `ClearanceViolation`, `NetIncompletes`,
`UnconnectedItems`, `AirLine`. Report serialised in the KiCad DRC JSON schema
exactly as Java's `-drc` mode emits.

## 9. Router (`fr-router`)

Faithful ports of:
- Maze: `AutorouteEngine`, `MazeSearchEngine`, `MazeExpansionEngine`,
  `MazeRipupResolver`, `MazeTraceShover`, `AutorouteControl`,
  `DestinationDistance`, maze list/search elements.
- Expansion: `ExpansionRoom`, `ExpansionDoor`, `CompleteFreeSpaceExpansionRoom`,
  `SortedRoomNeighbours` (+45°/orthogonal via `AngleRestriction`).
- Drill: `DrillPage`, `DrillPageArray`, `ExpansionDrill`.
- Path: `FoundConnectionLocator` (three regimes), `FoundConnectionInserter`,
  `Connection`.
- Batch: `BatchFanout`, `AutorouteBatchLoop`, `AutoroutePassRunner`,
  `BatchAutorouter`, `BatchOptimizer` (+ multithreaded via `rayon`),
  `AutorouteConnectionRouter`, `AutorouteAirlineCalculator`.
- Optimise: `TraceTightener{,45,90,AnyAngle}`, `TraceShover`, `ViaOptimizer`.
- Strategies as enums: item selection `Sequential | Random | Prioritized`,
  board update `Greedy | Global | Hybrid(m,n)`.

Public entry points: `batch_fanout`, `batch_autoroute`, `batch_optimize`, each
`(&mut Board, &Ctx) -> PassStats`.

## 10. Core (`fr-core`)

- `RoutingPipeline::run(board, settings, cancel, progress) -> RoutingResult`
  mirroring `RoutingPipeline.createForHeadless`.
- `Ctx { settings, cancel: CancelToken, progress: &dyn ProgressSink, rng seed }`.
- `CancelToken`: `Arc<AtomicBool>` + optional `Instant` deadline; checked
  wherever Java checks `is_stop_requested()`, including inside maze search.
- `ProgressSink::on_event(RoutingEvent)` with events: pass start/end, net
  routed/failed, optimizer improvement, final stats.
- `RoutingResult { board, stats, incompletes, drc_violations, timed_out }`.
- Result manifest JSON compatible with Java's `RoutingResultManifest`.
- Threads: `max_threads` (`-mt`); `0` or `1` = single-threaded, deterministic.

## 11. Settings (`fr-settings`)

- `RouterSettings` struct with every field `Option<T>`, derived `Serialize`,
  `Deserialize`, `JsonSchema`, with doc comments as descriptions.
- Merge order (later wins, only `Some` values override):
  defaults → DSN `(autoroute_settings)` → SES → `.rules` → env
  (`FREEROUTING__SECTION__FIELD`, lists comma-separated) → CLI.
- No persistent config file.

## 12. CLI (`freerouting` binary)

```
freerouting route <in.dsn> -o <out.ses> [--rules f] [--ses prev.ses]
                  [--max-passes N] [--timeout S] [--threads N]
                  [--result-json f] [--set section.field=value]...
freerouting drc   <in.dsn> [--ses f] [--rules f] [-o report.json]
freerouting info  <in.dsn>
freerouting mcp
```

- Legacy shim: if argv contains `-de`, `-do`, `-drc`, or other Java short flags
  (`-mp`, `-oit`, `-mt`, `-us`, `-hr`, `-is`, `-inc`, `-dr`, `-di`, `-l`,
  `--section.field=value`), argv is rewritten to the subcommand form before
  clap parses. `freerouting -de in.dsn -do out.ses -mp 100` works unchanged.
- Exit codes as Java: 0 iff COMPLETED or TIMED_OUT and output written; 1 else.
- Logging to stderr via `tracing`; `-v` / `--log-level`.
- `drc` and `info` write JSON to stdout when no `-o`.

## 13. MCP (`freerouting mcp`)

- Native stdio JSON-RPC 2.0, newline-delimited. Stdout reserved for protocol;
  all logs to stderr.
- Methods: `initialize`, `ping`, `tools/list`, `tools/call`,
  `notifications/progress` (outbound during routing), `notifications/cancelled`
  (inbound → `CancelToken`).
- Tools (synchronous; no job model):
  - `route_board { dsn_path | dsn_text, ses_path?, rules_path?, output_path?,
    settings? } → { ses_path | ses_text, stats, incompletes,
    drc_violation_count, timed_out }`
  - `check_drc { dsn_path | dsn_text, ses_path?, rules_path? } → KiCad report`
  - `board_info { dsn_path | dsn_text } → layers/nets/components summary`
  - `list_settings {} → RouterSettings schema with defaults + descriptions`
- Tool input schemas generated with `schemars` from the settings struct so CLI
  and MCP cannot drift.
- File-path variants write to disk and return paths, keeping large SES bodies
  out of the model context unless text is requested.

## 14. Testing

1. **Unit tests** per crate, ported from Java tests where they exist;
   TDD for each module.
2. **Bit-parity tests** (`tests/parity`, references in `tests/reference`,
   generated by `scripts/gen-reference.sh` from the Java jar):
   DSN round-trip, unrouted SES, board-info JSON, DRC list.
3. **Router metric-parity tests** (`#[ignore]`, `cargo test -- --ignored`):
   the 29 Java fixture routing tests with identical pass limits; assertions
   as in §3. `Dac2020Bm01` runs in normal CI as the smoke test.
4. **End-to-end**: CLI `route` on `tutorial_board.dsn` (output re-read by
   `SesReader`), legacy shim, MCP over spawned-process pipes.

## 15. Build order

Bottom-up, each layer parity-tested before the next, with the harness and
CLI/MCP skeleton stood up first:

1. Workspace, CLI/MCP skeleton, parity harness + `gen-reference.sh`.
2. `fr-geometry`.
3. `fr-board` (items, rules, search tree).
4. `fr-dsn` (parser, writers) → DSN round-trip + unrouted SES parity.
5. `fr-settings`.
6. `fr-drc` → DRC parity.
7. `fr-router` maze + expansion + path (first routed boards).
8. `fr-router` batch loop, fanout, optimizer, tighteners → metric parity.
9. `fr-core` pipeline, cancellation, progress, manifest.
10. CLI complete (incl. shim), MCP complete, e2e tests.
