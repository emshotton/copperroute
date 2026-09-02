# Plan 1 hand-off — foundation & `fr-geometry`

Branch `plan-1-foundation`, 36 commits on top of `main` (`e1145d1`). Final
whole-branch review: **merge with fixes**; fix wave applied and re-reviewed clean.

## Delivered
- Cargo workspace; `freerouting` binary (clap `route|drc|info|mcp`; stubs exit 3)
  with a Java-faithful `-de/-do` legacy shim; native stdio MCP skeleton.
- `parity` helper crate + `scripts/gen-reference.sh` (references NOT generated —
  needs JDK 25; see "Open items").
- `fr-geometry`: all 34 `geometry/planar` classes; 244 tests; audit script
  (`scripts/audit-geometry-port.sh`, exit 0 against 651 Java declarations);
  Java-vs-Rust differential harness (`scripts/differential/`, baseline in its
  README, reproduced exactly by the final review).
- `docs/java-quirks.md` (34 pinned quirks, totalizations, obligations),
  `docs/cli-legacy-flags.md`, `docs/geometry-library-survey.md`.

## Rulings made during execution (all reversible)
1. Work in place on a branch, not a nested worktree (harness resolves `../freerouting`).
2. Implementers may not install system packages; Java-25 references deferred to the user.
3. Java is the authority over plan test data; changes must cite the Java line; never bend a test to fit a bug.
4. Tautological test placeholders must be pinned before commit.
5. MCP `id: null` treated as a notification — deferred (MCP spec forbids null ids).
6. `IntDirection`/`Direction`/`Line` have **no `Ord`**; Java `compareTo` is exposed as inherent `compare_to` because Java's ordering is not antisymmetric at NULL / degenerate lines. `PartialEq` = Java `equals` (collinear ∧ positive projection, structural shortcut).
7. Lengths use `sqrt(x²+y²)` (`FloatPoint::size`), never `hypot`.
8. `getId`/`hashCode` ports use `wrapping_*`.
9. Interior points give `Line::side_of == OnTheRight` for every border line (differential-tested).
10. `Math.min/max` on doubles → NaN-propagating `java_min/java_max`; `Double.MIN_VALUE` = `f64::from_bits(1)`.
11. `docs/java-quirks.md` is mandatory: every reproduced quirk/totalization gets a row.
12. Totalizations (Rust value where Java crashes) are allowed only when no reachable Java caller observes the difference; otherwise surface the failure (`Result`/panic). Applied: `Polyline::from_lines -> Result<_, PolylineError>`; border-line-free `TileShape::distance` family panics (Task 14's `f64::MAX` withdrawn).
13. The differential harness is preserved in-repo.
14. `PolylineArea` border/holes are `PolylineShapeRef { Tile, Polygon }` (Java passes `IntBox` borders from `DrillPage`/`BoardOutline`).
15. `-de` with a `.json` part maps to a new `--kicad-json` option — a deliberate divergence (Java overloads the design/session slots); documented in `docs/cli-legacy-flags.md`.
16. `gen-reference.sh` disables routing with `--router.enabled=false` (`-mp 0` means *unlimited* passes in Java).

## Parked residuals (final review, no second fix wave)
- Legacy shim: bare `-drc` (no path) errors; Java accepts it and enters DRC-only mode. Plan 8.
- Legacy shim matches flags exactly; Java uses `startsWith` (`-mpx 5` ≡ `-mp 5`). Stricter; documented here only. Plan 8 decides.
- ~~`gen-reference.sh` never executed~~ — RESOLVED after JDK 25 install: `-de/-do` cannot produce unrouted references (see `tests/reference/README.md`); replaced by a jar-linked driver (`scripts/gen-reference/RefWriter.java`). All four fixtures generated; every `unrouted.ses` has 0 wires.
- `docs/cli-legacy-flags.md:73-75`: `-mt` consumer list incomplete (`BatchOptimizerMultiThreaded.java:41`, `GlobalSettings.java:853`); headless path `BatchOptimizer.createForHeadless` never reads `optimizer.maxThreads`.
- `docs/java-quirks.md` process note over-claims `grep "obligation:"` coverage (only the two MCP notes carry the token).

## Obligations for later plans (details in `docs/java-quirks.md`)
- Plan 2 (closed out by Plan 2 Task 16 — see `docs/plan-2-handoff.md`):
  - `PolylineError` → pass abort: **discharged.** `Polyline::from_lines`'s `Err` propagates as
    `BoardError::Normalization` (`crates/fr-board/src/error.rs`); `Board::combine_at_end` aborts
    the pass on it exactly as Java's pass-level `catch (Exception)` does (quirk #22).
  - Memo cache for convex pieces: **discharged in Task 10.** `fr-geometry` still recomputes
    `split_to_convex` per call (deliberately — quirk #30's per-call `Random`), but the memo moved
    up to the item level: `ObstacleAreaData::convex_pieces` / `BoardOutline::keepout_convex_pieces`,
    both `OnceLock<Option<Vec<TileShape>>>`, cleared at the same points that clear the absolute-area
    memo (`docs/java-quirks.md` candidate table).
  - `equals_geometric` not `==` for `Line`: **not discharged — remains a Plan 7 obligation,
    unchanged.** Plan 2 added no new call site: the one place `equals_geometric` is used
    (`Simplex::border_line_index`) predates Plan 2 entirely. The two Java call sites Plan 2 could
    have reached — `TraceTightener.repositionLine` / `TraceTightenerAnyAngle.repositionLine` — are
    shove machinery, explicitly out of Plan 2's scope (self-review: "Deliberately excluded …
    shove/tighten/forced (§9, Plans 6/7)"). The nearest look-alike site, `PolylineTrace.change`'s
    line-by-line diff, compares by **structural** equality on purpose (quirk #74) — not
    `equals_geometric` — because the port's `Line` is a `Copy` value type with no identity to base
    Java's reference comparison on; that quirk documents the resulting divergence and hands Plan 7
    the decision.
  - `border_line_index == None` on box/octagon is a Java stub, not "not a border line":
    **discharged.** `ShapeAndEntrySide::new` (`crates/fr-board/src/structure/shape_entry_side.rs`)
    reads `border_line_index` and documents the stub's `-1`/`None` at the call site rather than
    conflating it with "no such line" (quirk #7's original caller); quirk #68 separately covers the
    constructor's own `!=`-by-reference defect.
  - Marker-hygiene sweep (`// Java bug:` literal prefix): **done in Task 14.** All nine audited
    directories are at 0 missing members; grep confirms no stray casing variants
    (`// Java Bug:`, `//java bug:`, etc.) exist in `crates/fr-board/src`; eight `docs/java-quirks.md`
    citation/count errors found by a method-by-method self-review were corrected (see Task 14's
    report, `.superpowers/sdd/2026-08-28-plan-2-board-model/task-14-report.md` §4).
- Plans 6/7: `catch_unwind`/`Result` boundaries at `AutoroutePassRunner.java:144` (per pass) and `BatchAutorouterThread.java:537` (per item); Java `catch (Exception)` does not catch `StackOverflowError`.
- Plan 5: apply Java flag normalisation (`-oit /100`, `-mp`/`-mt` clamps, `-us`/`-is` folding) in `fr-settings`.
- Plan 8: MCP handler shape (progress sink, cancel token, reader thread); `id: null`; bare `-drc`.

> ## Plan 8 close-out — written by Plan 8 Task 14, the last task of the last plan
>
> **There is no Plan 9.** `docs/plan-8-handoff.md` is the project completion report; this block is
> the status of *this* hand-off's Plan-8 items, written here so a reader of this file does not have
> to go looking.
>
> | item | status |
> |---|---|
> | MCP handler shape — progress sink, cancel token, reader thread | **DISCHARGED, Tasks 11 and 12.** `ToolHandler` is `Box<dyn Fn(&State, Value, &ProgressWriter, &CancelToken) -> Result<Value, RpcError> + Send + Sync>`; a `tools/call` runs on its own thread, `_meta.progressToken` turns on `notifications/progress`, and an inbound `notifications/cancelled` flips the token **while** the tool runs. The jar has none of the three (delta rows 6 and 7) |
> | `id: null` on a parse error | **DISCHARGED, Task 11.** The port answers `{"jsonrpc":"2.0","id":null,"error":{…}}`, compact. Java's `-32700` reply has **no `id` member** at all (Gson drops the JSON-null) and is the one response Jersey pretty-prints — MCP delta row 9, `McpControllerV1.java:152` |
> | bare `-drc` errors in the shim; Java accepts it and enters DRC-only mode | **DISCHARGED as a measurement, and Plan 1's reading was wrong.** Quirk **#263**: a bare `-drc` is *not* DRC mode in Java either. DRC mode is entered only when `drcReportFile != null` (`Freerouting.java:1462`), so a bare `-drc` sets two dead booleans and falls through to `initializeCli`, which dies with "Both an input file and an output file must be specified". The port reproduces that exactly, and `p8t5` compares it |
> | the shim matches flags exactly where Java uses `startsWith` (`-mpx 5` ≡ `-mp 5`) | **DISCHARGED, Task 5, and the decision went the other way.** Ruling **AR** makes the legacy path bug-for-bug, so the port now matches by prefix exactly as Java does — quirk **#259**. `-decoy`/`-diff`/`-drcx` are accepted as `-de`/`-di`/`-drc`, and `-mp -5` is impossible to express, on both programs. `sweep-p8t5.sh`: 86 rows, 86 MATCH |

- Before `fr-dsn` formats floats: `FloatPoint::Display` differs from Java `NumberFormat` above 2^53 and at 4th-digit ties.

## Open items for the user
- Post-parity improvement candidates are listed in `docs/java-quirks.md` (`i128` fast path in `Line::intersection` is the big one) and `docs/geometry-library-survey.md`. Note the latter's `i_overlay` entry is **superseded**: Plan 2 confirmed `ConductionArea`'s `i_overlay`-shaped fill cache is renderer-only and unreachable from routing (quirk #59), so `i_overlay` is not needed unless a renderer is ported later — see `docs/geometry-library-survey.md` §(vi) as amended by Plan 2 Task 16.
