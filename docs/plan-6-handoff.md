# Plan 6 hand-off — the maze/expansion autorouter (`fr-router`)

Branch `plan-6-router-maze`, 44 implementation commits (Tasks 1-17b) on top of
`main` (`d5ce6d7`, Plan 5 merged 2026-08-29), plus the plan document itself
(`db4477b`) and Task 18's — `git log db4477b..HEAD` is the whole branch. See
`git log` for the tip.

**Read this before Plan 7.** Plan 7 is the pass loop, the fanout and the optimizer;
it does not re-implement anything here, and §10 is the list of what it must build.
Plan 8 owns the CLI/MCP surface and inherits six items (§11). Section 1 is the one
thing to read even if nothing else here is: **the port routes real boards byte-identically
to the Java jar, connection by connection, and the acceptance ladder in §2 is what
that claim means and how to re-check it.**

---

## 1. The headline: exact per-connection parity, not metric parity

Plan ruling 1 deliberately strengthened spec §3, which had asked only for metric
parity (incompletes delta, via delta, zero violations, trace length within ±10 %).
The reason is a fact the spec's survey did not have: **a single-threaded Java
routing run is byte-reproducible and does not depend on `Object.hashCode`.** So an
exact per-connection reference is meaningful, and it is what this plan built.

The acceptance ladder, per connection, in order:

* **(a)** the same `AutorouteAttemptState` *and* the same ripped-item id set;
* **(b)** the same inserted geometry — every new trace's layer, half width and
  polyline corner list, every new via's centre, padstack and layer span, in
  insertion order, **with the same item ids**;
* **(c)** spec §9's metric block — incompletes delta equal, via delta equal,
  `violations == 0`, cumulative trace length within ±10 %.

**Measured result: 369 connections on five boards reach (a), (b) and (c) — all
three rungs, every connection**, at `ripupPassNo` 1, 2 and 4.

| stem | board | connections | (a) | (b) | (c) |
|---|---|---|---|---|---|
| `router-rpi-splitter` | `Issue143-rpi_splitter.dsn` | 8 (of 9) | 8/8 | 8/8 | 8/8 |
| `router-dac2020-bm01` | `Issue508-DAC2020_bm01.dsn` | 294 (all) | 294/294 | 294/294 | 294/294 |
| `router-j2-reference` | `Issue026-J2_reference.dsn` | 45 (all) | 45/45 | 45/45 | 45/45 |
| `router-tutorial-board` | `examples/tutorial_board/tutorial_board.dsn` | 0 | — | — | — |
| `router-ecc83-input` | `Issue649-kicad_ecc83-pp_input_board_v1.dsn` | 22 (all) | 22/22 | 22/22 | 22/22 |
| `router-dac2020-bm01-pass2` | the same board at `ripupPassNo = 2` | 294 (all) | 294/294 | 294/294 | 294/294 |

`router-tutorial-board`'s zero is an assertion, not a gap: its `(network …)` scope
is 438 empty `@:no_net_N` nets, so no item has an unconnected set and the board has
nothing to route. "The port agrees there is nothing here" is a real regression guard
on the DSN reader and on `Board::unconnected_set`.

**The premise is checked, not assumed.** `scripts/gen-router-reference.sh
--verify-hash-modes` regenerates every stem under `-XX:hashCode=0,1,2,3,4` and
requires five byte-identical files; it passes on all five stems. That is stronger
than Plan 5's DRC result, where one stem of eight is genuinely hash-dependent
(quirk #146).

### The one thing that nearly broke it, and what closed it

At `ripupPassNo >= 2` the DAC2020 board diverged at connection k = 267 (six extra
transient item ids) and at k = 270 (real geometry), with pass 1 green. Controller
ruling AD made that Task 17b, and the root cause was **quirk #74**:
`PolylineTrace.change` (`:960`, `:972`) compares `Line`s **by reference**, which
Plan 2 had recorded as unavoidable. The port compared by value, kept a different
number of lines, and produced a different search-tree shape three steps later.

**Controller ruling AE** accepted the fix: `fr_geometry::Line` carries a private
identity token drawn from a process-wide `AtomicU64`. **This is a contract Plan 7
must keep** — see §9.

---

## 2. Delivered

`crates/fr-router` — 30 043 lines of `src`, 23 366 lines of `tests`, a behavioral
port of `app.freerouting.autoroute.{,maze,expansion,drill,path}` plus the
`board/actions` and `board/optimize` classes the router drives, and the two
`ShapeSearchTree` methods only the router calls.

**What it routes today:** one connection at a time — `AutorouteConnectionRouter.route`
steps 1-5: build the `AutorouteControl`, compute the start/dest sets with the plane
swap, `initAutoroute`, `autorouteConnection` (maze search → locate → delete the
ripped connections → insert), read the result.

**What it does not:** the pass loop, the fanout pre-pass, the optimizer,
`ViaOptimizer`, `RoutingBoard.optChangedArea`, `removeItemsAndPullTight`, and the
necked-retry / strict-DRC-rollback / failure-log tail of `route` — all Plan 7's
(ruling 2), all in §10.

### The commits, by task

| task | commits |
|---|---|
| 1 — crate skeleton, `Arena`, `ItemAutorouteInfo`, public `JavaRandom` | `9b63cb1`, `f924f32` |
| 2 — expansion rooms, doors, `TreeObject::Room` in the compensated tree | `7bc2632`, `a4ba6e5` |
| 3 — `completeShape` / `divideLargeRoom`, three angle regimes | `131211f`, `da6f4ea` |
| 4 — `SortedRoomNeighbours` and `JavaTreeSet` (ruling Y) | `ca60e81` |
| 5 — the 45-degree and orthogonal neighbour sorters | `db02973`, `6924123` |
| 6 — `AutorouteEngine`'s room lifecycle | `b4e26f0`, `49757ce` |
| 7 — drill pages, the page array, expansion drills | `664c5c6`, `d811d3c`, `2728edd` |
| 8 — `AutorouteControl`, `DestinationDistance`, `MazeListElement`, `MazeQueue` | `a6655d2`, `2522607` |
| 9 — `RoutingBoardExt` + the check-only shove | `a3bb81b`, `3777aaa` |
| 10 — `ForcedPadRouter.checkForcedPad` and the `ForcedViaInserter` trio | `e43f947`, `36ef57e`, `63c1cec` |
| 10b — the via-insertion chain (controller ruling AA) | `c4648d8`, `b316c45` |
| 11 — `MazeSearchEngine`'s init, queue and pop loop | `599271a`, `5a269ae` |
| 12 — room-door expansion, the A\* cost model, `MazeTraceShover` | `741b259`, `00c7f84` |
| 13 — `MazeExpansionEngine`, `MazeRipupResolver`, `Connection` | `f5cf5c9`, `5028a0c` |
| 14 — the three `FoundConnectionLocator` regimes | `c8e1c5f`, `85fc99b` |
| 15a — the pull-tight family (controller ruling AB) | `cd202d0`, `b0c6874` |
| 15b — `insertForcedTracePolyline`/`Segment`, `springOverObstacles` (ruling AB) | `8af9915`, `446d5a8` |
| 15 — `FoundConnectionInserter`, and the stop-checked `insert_via` | `2458967`, `0492867` |
| 16 — `autorouteConnection` end to end, and `route_connection` | `d361d73`, `5979089` |
| 17 — `p6t1`, the HEAD-jar references, the acceptance ladder | `8205efe`, `1f785b8`, `b6bb417` |
| 17b — quirk #74's `Line` identity token (controller ruling AD) | `ead7902`, `2d96c39` |
| 18 — the ported Java suites, audit to zero, README, quirks, this file | see `git log` |

---

## 3. Public API surface (what Plan 7 calls)

The seam is **one function**. Plan 7's pass runner calls it once per item:

```rust
pub fn route_connection(
    board: &mut Board,
    engine: &mut Option<AutorouteEngine>,   // Java's RoutingBoard.autorouteEngine field
    item: ItemId,
    net_no: i32,
    settings: &RouterSettings,
    trace_costs: &[ExpansionCostFactor],
    ripped: &mut BTreeSet<ItemId>,          // out: the ripped-item id set
    ripup_costs: &mut BTreeMap<ItemId, i32>,// out: per-item ripup costs
    ripup_pass_no: i32,                     // AutorouteConnectionRouter.route:45
    start_ripup_costs: i32,                 // settings.getStartRipupCosts()
    remove_unconnected_vias: bool,          // !settings.isFanoutEnabled() (:46)
    retain_autoroute_database: bool,
    stop: StopCheck<'_>,
) -> AutorouteAttemptResult;
```

`engine` is `&mut Option<…>` because Java's `RoutingBoard.autorouteEngine` is a
nullable field the router creates, reuses across connections when
`retainAutorouteDatabase` is set, and nulls at `finishAutoroute`.

**The one precondition the signature cannot express — read this before writing the
pass runner.** The caller **must** call `board.start_marking_changed_area()`
immediately before every `route_connection`, because
`AutoroutePassRunner.java:224` does, and **quirk #177** makes the presence of
`board.changedArea` observable inside `TraceShover::insert`: with it `None`, the
substitute traces are inserted un-normalised, so the board diverges silently
rather than failing. Leaving it out **routes a different board** — this is not
hygiene. There are **five** `route_connection` call sites in this tree and
**three** of them mark, which is the correct split, not a gap:

| call site | marks? | why |
|---|---|---|
| `scripts/differential/rust/src/bin/p6t1.rs:241` -> `:248` | yes | production-shaped: it is a pass runner in miniature |
| `crates/fr-router/tests/fixtures.rs:154` -> `:158` | yes | production-shaped |
| `crates/fr-router/tests/reference_parity.rs:217` -> `:222` | yes | production-shaped — the acceptance ladder |
| `crates/fr-router/tests/autoroute_connection.rs:1061` (`route_once`) | **no** | mirrors `P6T16Probe.routeSteps1to5`, which does not mark either; the probe is the oracle, so marking here would make the test disagree with the JVM |
| `crates/fr-router/tests/autoroute_connection.rs:1159` | **no** | same mirror, same reason |

The rule for Plan 7 is therefore: **every production-shaped caller marks.** The two
that do not are `P6T16Probe` mirrors and must stay unmarked. A second precondition, weaker: the
`(item, net)` connection list is computed **once**, before any routing, exactly as
`AutoroutePassRunner` computes `autorouteItemList` once per pass — an entry whose
item a later connection ripped up is skipped, not re-derived. The engine's own
entry point, for a caller that has already built its `AutorouteControl`:

```rust
impl AutorouteEngine {
    pub fn autoroute_connection(
        &mut self, board: &mut Board,
        start: &BTreeSet<ItemId>, dest: &BTreeSet<ItemId>,
        ctrl: &AutorouteControl,
        ripped: &mut BTreeSet<ItemId>,
        ripup_costs: Option<&mut BTreeMap<ItemId, i32>>,
        stop: StopCheck<'_>,
    ) -> AutorouteAttemptResult;
}
```

Everything else the crate exports is in `fr_router::prelude` and listed in
`crates/fr-router/README.md`'s type table: `AutorouteControl` + `ViaMask`,
`DestinationDistance`, `MazeListElement`, `MazeQueue`, `MazeSearchEngine` +
`MazeResult` + `ShoveResult`, `MazeExpansionEngine`, `MazeRipupResolver`, the seven
expansion-room / door types plus `ExpansionRoomStore` and `RoomRef`/`ExpandableRef`,
`DrillPage`/`DrillPageArray`/`ExpansionDrill`, `Connection`,
`FoundConnectionLocator` + `ResultItem`, `FoundConnectionInserter`,
`AutorouteSearchTreeExt`, `Arena<T>` and its index newtypes, `JavaTreeSet`, and —
from `board_ext` — `RoutingBoardExt`, `TraceShover`, `DrillItemMover`,
`ForcedPadRouter`, `ForcedViaInserter`, `TraceTightener`, `CheckDrillResult`,
`SpringOverOutcome`. `ExpansionCostFactor` is **re-exported** from `fr-settings`,
never redeclared (ruling 8, the Plan 4 obligation, discharged in Task 1).

### The signature changes this plan made outside `fr-router`

| crate | change | why |
|---|---|---|
| `fr-geometry` | `JavaRandom` made public, with `set_seed`/`next_double` | ruling 5 — `MazeRipupResolver`'s draw must be bit-exact |
| `fr-geometry` | `Line` carries a private identity token; `Line::is_same_object`; `Polyline::from_lines_in_place` | rulings AE and quirk #188 |
| `fr-board` | `ids.rs` gains `ObstacleRoomId`, `ConnectionId`; `AutorouteInfo` gains its body | ruling 15 — ids, not objects |
| `fr-board` | `insert_via_checked`, `insert_escape_via_checked`, `split_traces_checked` — the three original names stay as `\|\| false` wrappers | ruling 6 + plan-3 ruling F |
| `fr-board` | `Board::connect_to_trace_of` (a `TraceSnapshot` entry point) | quirk #186 — Java's dead trace reference |
| `fr-board` | `Board::item_shape_layer`, `ShapeSearchTree::insert_room`/`remove_room` | Plan 2's `TreeObject::Room` obligation |
| `fr-dsn` | none beyond threading the reader's existing `StopCheck` | — |
| `fr-settings` | none | — |

Every Plan 1-5 test stayed green by construction, and `crates/freerouting` was not
touched except for `#![forbid(unsafe_code)]`.

### `#![forbid(unsafe_code)]`, and the one exception (user request, Task 18)

The attribute is the **first line** of all eight workspace crate roots:
`crates/{fr-geometry,fr-board,fr-dsn,fr-settings,fr-drc,fr-router}/src/lib.rs`,
`tests/parity/src/lib.rs` and `crates/freerouting/src/main.rs`. Nothing had to be
rewritten to make it hold — there was no `unsafe` in any crate before the change.

**There is exactly one `unsafe` left in the repository, and it is not in a crate:**
`scripts/differential/rust/src/bin/p2t13.rs` — `static mut STATE: u64 = 0;` at
`:12`, read in an `unsafe` block at `:15`, written in another at `:103`. It is the
`p2t13` driver's private transcription of `java.util.Random`'s LCG, held in a
`static mut` so the shuffle can be called from free functions the way
`Collections.shuffle` is. The differential drivers are deliberately outside the
`forbid` set for that reason;
`scripts/differential/README.md` §"The repository's only `unsafe`" is the
authority, and the "`JavaRandom` is copied into four driver binaries" cleanup in
that file's *Deferred coverage* section would remove it.

**How to re-check it, precisely.** A bare `grep -rn 'unsafe' --include='*.rs'
crates/ tests/` answers **10 lines and that is expected**: the eight
`#![forbid(unsafe_code)]` attributes plus two doc-comment lines in
`crates/fr-router/src/lib.rs:80-81` that describe them. The check that means what
it says is the one for an actual `unsafe` construct:

```sh
grep -rnE 'unsafe (\{|fn |impl |trait )' --include='*.rs' crates/ tests/   # no match, exit 1
grep -rn  'unsafe' --include='*.rs' scripts/differential/rust/               # p2t13.rs:15, :103
```

---

## 4. Rulings

### Plan rulings 1-17 — and what execution did with them

| # | ruling | outcome |
|---|---|---|
| 1 | exact per-connection parity, metrics as the fallback | **confirmed, and the fallback was never needed.** Evidence: 369 connections at rung (b), `--verify-hash-modes` passing on five stems, and `reference_parity.rs::geometry_is_required_where_it_was_reached`, which fails if a stem is ever quietly demoted. |
| 2 | the seam is `AutorouteConnectionRouter.route`, split at step 5 | **held, twice amended.** Rulings AA and AB moved `ForcedPadRouter`'s routing half, `DrillItemMover`'s mutating half, `TraceShover.insert`, `springOverObstacles` and the whole `TraceTightener` family *down* into Plan 6, because the insertion path unconditionally reaches them. `springOverObstacles` stayed Plan 7 under AA and came down under AB. |
| 3 | `RoutingBoardExt` is Plan 6's, not Plan 7's | held (Task 9). |
| 4 | deterministic containers, transcribed rather than chosen | **superseded in part by ruling Y**: a `BTreeSet` is wrong wherever the comparator is not a total order. `JavaTreeSet` replaces it there. |
| 5 | `JavaRandom`, not `rand` | held (Task 1). |
| 6 | cancellation at exactly six sites | held. The six: four in `MazeSearchEngine::init` (`:975` destination set, `:1002`, `:1040`, `:1073`), one in the pop loop (`:323`), one in `DrillPage::get_drills` (`:103`, `splitToConvex`). Each has a test. Ruling AC later fixed that the **inserter** gets `&\|\| false`, because Java checks no stop below `AutorouteEngine.java:265`. |
| 7 | five recovery boundaries | **held as six.** The plan's five are delivered and exercised; ruling AB pulled a sixth Java `catch (Exception)` into scope with `insertForcedTracePolyline` (`board/facade/RoutingBoard.java:787-841`, covering `normalize` and `splitTracesAtKeepPoint`). It is discharged through the **`Result` channel**, not `catch_unwind` — `crates/fr-router/src/board_ext/routing_board_ext.rs:780` and `:787` drop the `Err` and fall through, and no path inside the covered region panics. Correction: `completeExpansionRoom`'s `catch` returns an **empty** collection, not a partial one (quirk #166) — the plan's note said otherwise. The full table is in `crates/fr-router/README.md` § "The six recovery boundaries". |
| 8 | `AutorouteControl` copies out of `RouterSettings` | held, with an undercount: `RouterSettings` has a **third** router-side reader, `getStartRipupCosts` (Task 13). |
| 9 (**ruling H**) | the via re-pointing verdict, defaulting to "accept" | **the default did NOT apply — see §5.** |
| 10 | ban the `&self` cold-cache recompute in `fr-router` | held. `grep -rn "item_tree_shape_ref\|item_tile_shape_ref" crates/fr-router/` finds only a doc comment saying never to use them. |
| 11 | three differential drivers plus a reference generator | held: `p6t1`, `p6t2`, `p6t3` and `scripts/gen-router-reference.sh`, plus twelve `P6T*Probe` JVM probes (§7). The spec §14.2 deviation (a sibling generator rather than the SES one) is the same one Plan 5 made, for the same reason. |
| 12 | three ported Java suites plus the fixture assertion family | held (Task 18): `crates/fr-router/tests/java_ports.rs` and `crates/fr-router/tests/fixtures.rs`. |
| 13 | one audit map per crate, and Plan 3's `fr-board.map` obligation is discharged here | held (Task 18) — and writing the map found seven real gaps, §5. |
| 14 | drop the hard-coded debug net numbers; port the HEAD-only pure-SMD relaxations | held. Quirk #190 records the dropped ones; quirk #172 the ported ones. |
| 15 | `AutorouteInfo` gets a body of ids only | held (Task 1). |
| 16 | hand-rolled `Arena<T>`, not `slotmap` | held. No generation counter, deliberately: a generational key would turn a stale index into `None` where Java reads a live object. |
| 17 | single-threaded, and the crate says so | held. |

### Controller rulings X-AE (made during execution)

| ruling | what it decided | one-line rationale |
|---|---|---|
| **X** | plan rulings 1-17 and the drafter's four open answers accepted: exact per-connection parity, no `rand`/`slotmap`/`rayon`, ruling 9's default, `\|\| false` wrappers for the `fr-board` signature changes, sequential dispatch | the plan's own self-review had already named the cost of each; nothing needed re-deciding before Task 1. |
| **Y** | the maze queue and the neighbour set are `JavaTreeSet` (a port of `java.util.TreeMap`'s red-black tree), not `BTreeSet` | the comparators are non-transitive, so the two container shapes keep and order **different** elements — measured on `run.sh p6t3` mode 3, not argued. |
| **Z** | a plan's quirk numbers are *labels*; the implementer claims the next free id from the register and the ledger records it | the register moved under the plan between drafting and Task 2, and no scheme that pre-assigns ids can survive that. |
| **AA** | Task 10b: `forcedPad`, `TraceShover.insert` and `DrillItemMover.{insert,shoveVias}` come into Plan 6 | `ForcedViaInserter.insert` cannot be ported without them and the maze calls it; ~430 lines re-homed if wrong. |
| **AB** | Tasks 15a/15b: the whole `TraceTightener` family, `PolylineTrace.pullTight`, `insertForcedTracePolyline`/`Segment` and `springOverObstacles` come into Plan 6 | pull-tight is unconditionally on the insertion path, so ruling 1's exact parity is unreachable without it; ~3 000 lines that Plan 7 needed anyway. |
| **AC** | `FoundConnectionInserter` is handed `&\|\| false`, not the caller's stop check | Java checks no stop below `AutorouteEngine.java:265`, so threading one there would stop runs Java completes — quirk #76's hang stays reachable exactly as in Java, and Plan 7 owns the wall clock. |
| **AD** | Task 17b: root-cause the DAC2020 `ripupPassNo >= 2` divergence rather than record it | the code that produces it is *Plan 6's*, and pass-1 parity held only by compensation over an already-divergent tree — a latent result, not a passing one. |
| **AE** | the `Line` identity token is accepted as the port of Java object identity — a recorded exception to "no static mutable state" | nothing reads the counter's *value*, only the equivalence relation it induces, so no output, ordering, hash or serialised form can observe it; Java's own object identity **is** process-global mutable state, so this is the closest model rather than an invention. |
| **H** | the via-info/via-rule re-pointing verdict — see §5 | it closes **against** the default. |

---

## 5. Corrections to the plan discovered during execution

1. **`AutorouteAttemptState` has nine variants at HEAD, not six** (Task 1). Ported
   verbatim; `INSERT_ERROR` is one of the three the plan did not know about, and it
   is what Plan 7's necked retry produces.
2. **Ruling 9 (H) closes AGAINST the re-pointing, and the fix is an `fr-board`
   change Plan 7 owns.** The plan's default was "accept the re-pointing if the
   divergence is unobservable". It is observable.
   `scripts/differential/run.sh p6t1 ../freerouting/fixtures/Issue593-BBD_Mars-64.dsn
   50 1 crates/fr-router/tests/data/ruling-h-redeclare.rules` routes the board with
   a one-line `.rules` file that re-declares its only `(via …)`: **without** the
   file the two sides agree on all 50 connections byte for byte; **with** it they
   first differ at k = 6 and genuinely diverge from k = 8, where the jar lays four
   traces and the port two (cumulative trace length `1401450.8259119983` against
   `1395031.4105961146`). The port routes *shorter*, which is exactly what
   `attachSmdAllowed = true` buys — a via attaching to an SMD pad the jar's detached
   `ViaInfo` forbids. Rungs (a) and (c) still hold; it is (b) that fails.
   **The fix is Plan 7's Task 0** — controller **ruling AL** makes it the first
   task of Plan 7, before anything is built on top of the current ownership model,
   not a loose follow-up: `fr_board::rules::ViaRule` must own its `ViaInfo`s, the
   way Java's holds object references (`ViaRule.java:21`), so that
   `ViaInfos::remove` cannot re-point a rule. Tombstones were considered and
   rejected — they leak the removal into every index walk. No acceptance fixture
   uses a `.rules` file, so the reference set is unaffected either way.
3. **Quirk #74 is REPRODUCED, and Plan 2's "unavoidable" note is withdrawn** (Task
   17b, ruling AD). See §9 for the contract.
4. **`Sorted45DegreeRoomNeighbours` and `SortedOrthogonalRoomNeighbours` compute
   their door dimension**, they do not hard-code 1 (Task 5, review B1). The plan's
   text implied otherwise.
5. **`completeExpansionRoom`'s `catch` returns an empty collection** (quirk #166),
   correcting ruling 7's note. Every caller consumes `Err` as an empty list; **never**
   with `?`.
6. **The `fr-board.map` found seven real gaps** (Task 18). Under the crate-wide
   audit, `Item.setClearanceClassIndex` was being satisfied by
   `ViaInfo::set_clearance_class_index` because its own `renamed:` marker was split
   across two lines and the regex is line-based; `ShapeTree.Leaf.compareTo`,
   `ShapeSearchTree.EntrySortedByClearance.compareTo`,
   `PlanarDelaunayTriangulation.{Corner,Edge.compareTo,TriangleGraph.insert}` and
   `Signum.{of,toString}` had no marker of their own at all. All seven now do. The
   script was **not** weakened, and the map narrows rather than widens: re-pointing
   `BasicBoard` from `board/*.rs` to `items/mod.rs` in a scratch copy makes
   `board/facade` exit 1 with 30+ `MISSING` lines, so the zero is earned. The one
   glob it does use covers all **eight** `board/facade` classes plus `Item` —
   **nine** rows — because `Board` is one Rust type assembled from those eight Java
   classes (plan-2 ruling 1) and its impls are split across `board/*.rs` by subject.
7. **`BasicBoard.areThereItemsOnInactiveLayer` has no caller anywhere in the Java
   tree** and its whole body is one `FRLogger.warn`. Re-pointed from a Plan 6
   deferral to `not ported:`.

---

## 6. Quirks pinned in Plan 6 (`docs/java-quirks.md` #156-#193)

Thirty-eight rows, contiguous, plus a rewrite of #74. The register is at **1..193
with no gaps and no duplicates** (`grep -o '^| [0-9]\+ ' docs/java-quirks.md`), and
the next free id is **#194**.

Grouped by what they are about:

* **Non-total-order comparators and the sets that eat elements** — #160 (three
  copies of a non-transitive `SortedRoomNeighbour.compareTo`), #161 (its final
  tie-break subtracts a *room* id from an *item* id), #170 (`MazeListElement`'s NaN
  fall-through), #171 (a four-key tie makes `TreeSet` drop the element whole).
  These are why `JavaTreeSet` exists.
* **Mutable sort keys** — #156 (`ObstacleExpansionRoom.getId` aliases at 1024 tree
  shapes), #158 (`IncompleteFreeSpaceExpansionRoom.getId` over a mutable shape),
  #167 (`DrillPage.getId` hashes a field `getDrills` overwrites).
* **Non-termination** — #162 (`calculateNewIncompleteRooms` when `toSimplex()`
  drops a border line; reproduced, and it hangs), #168 (a cancelled `splitToConvex`
  memoises the page as having no drills).
* **Silent skips and wrong indices** — #159, #163, #164, #165, #169, #174, #175,
  #176, #179, #184, #192.
* **Null and cast hazards Java reaches** — #157, #173, #177, #181, #185, #186.
* **Dead or unreachable code Java ships** — #180 (the `SKIPPED` arm), #182
  (`avoidAcidTraps` is `if (true) return`), #183 (the any-angle end-corner `bend`
  branch is unreachable).
* **HEAD-only behaviour with no upstream counterpart** — #172 (the pure-SMD
  relaxations, routing-visible, ported because HEAD is the parity jar), #193 (three
  "stale tree index during routing" guards, load-bearing, all three reached by the
  corpus).
* **Crossed parameters** — #187 (`connectToTrace` stubs are sized from the *other*
  end's layer).
* **Aliasing** — #188 (`new Polyline(Line[])` normalises the caller's array in
  place; six Plan 6 sites read the array back), and #74's rewrite.
* **Test-harness and diagnostic quirks** — #189 (`TestingSettings.setMaxPasses`
  is first-writer-wins, which is why `Dac2020Bm01RoutingTest`'s 194 is a *one-pass*
  bound), #190 (hard-coded debug net numbers 33/66/67/94/98 in production routing
  code), #191 (`calculateCheapDistance` mutates and restores a shared field).

Two rows were added to the **totalization** table (`ForcedPadRouter.calcCheckShapeForFromSide`'s
null `offsetShape`, and `TargetItemExpansionDoor`'s null tree shape). The other 35
`// totalized:` markers in this crate are unreachable arms and say so at the site —
the register's own rule, restated in `docs/java-quirks.md` §Process notes.

---

## 7. Test and fixture inventory

`cargo test --workspace` is green. The crate's suites:

| file | what it covers |
|---|---|
| `tests/java_ports.rs` | **ruling 12's three ported Java suites**, one method for one method: `MazeListElementTest`, `SortedRoomNeighboursFactoryTest`, `RoutableLayersSafetyCheckTest` |
| `tests/fixtures.rs` | **ruling 12's fixture assertion family** — one pass over the first *k* connections, `incomplete_connections <= bound` and `clearance_violations == 0`. Spec §14.3's smoke test `dac2020_bm01_one_pass_two_items_leaves_at_most_194_incompletes` runs in ordinary CI |
| `tests/reference_parity.rs` | the acceptance ladder against the committed HEAD-jar references |
| `tests/skeleton.rs`, `arena` unit tests | Task 1 |
| `tests/expansion_rooms.rs`, `tests/tree_ext.rs` | Tasks 2, 3 |
| `tests/sorted_neighbours.rs`, `tests/sorted_neighbours_regimes.rs` | Tasks 4, 5 |
| `tests/engine_rooms.rs`, `tests/drill.rs`, `tests/maze_drills.rs` | Tasks 6, 7 |
| `tests/control.rs`, `tests/destination_distance.rs`, `tests/maze_list_element.rs`, `tests/maze_queue.rs` | Task 8 |
| `tests/board_ext.rs`, `tests/forced_via.rs`, `tests/tightener.rs` | Tasks 9, 10, 10b, 15a, 15b |
| `tests/maze_search.rs`, `tests/maze_expand.rs`, `tests/ripup.rs` | Tasks 11, 12, 13 |
| `tests/locator.rs`, `tests/inserter.rs`, `tests/autoroute_connection.rs` | Tasks 14, 15, 16 |

**The three ported Java suites live in one file on purpose** (`tests/java_ports.rs`),
so that `grep -rn MazeListElementTest crates/` gives one answer. The *extended*
assertions each subject needs — the four tie-breaks, quirks #170/#171, the
non-transitive comparator, `AutorouteControl`'s whole JVM field transcript — stay in
the task files that produced them, which the header names.

Spec §14.1's other three named suites are **not** ported and are not this plan's:
`StrictDrcEnforcementTest`, `BatchAutorouterDebugTest` and `autoroute/pipeline/*`
all drive `autoroute/pipeline`, i.e. Plan 7.

**What needs the sibling Java checkout** (`$FREEROUTING_JAVA_DIR`, default
`../freerouting`): `tests/fixtures.rs`, `tests/reference_parity.rs`, `tests/control.rs`
and every test that reads a DSN fixture. They call `parity::require_java_dir()` and
skip with a message when it is absent.

---

## 8. The differentials

Three driver pairs, twelve JVM probes, all against the clone's **HEAD** build (the
pinned 2.3.0 jar of plan-3 ruling 10 is not used anywhere in this plan — HEAD's
`autoroute/**` is a different algorithm). `scripts/differential/run.sh` prints the
jar it used in its header line; read it.

| driver / probe | what it pins |
|---|---|
| `p6t2` | `completeShape` / `divideLargeRoom` in all three angle regimes |
| `p6t3` | the three neighbour sorters, the non-transitive comparator, `JavaTreeSet` |
| `p6t1` | the acceptance ladder — a whole board, connection by connection |
| `P6T17bProbe` | quirk #74's `Line` reference comparison and the `keepAt` counts it changes |
| `P6T6Probe` … `P6T16Probe` (12 files) | one per task, transcripts committed under `crates/fr-router/tests/data/` |

**The pre-existing DIFF status of `t14`, `t15`, `t16r` and `e15` is unchanged by
this plan** and is documented in `scripts/differential/README.md` §"Known, expected
diffs": `e15` 3 diff lines (quirk #21, non-finite width), `t15` 44 (42 cosmetic
`EXC:` tag differences plus 2 sign-of-zero, quirk #14), `t16r` 72 (`LineSegment`/
`offsetBox` with an out-of-range `no`), `t14` 145 (`Simplex.EMPTY` and degenerate
shapes, all in the totalization table). None of them is a Plan 6 regression, and
Task 17b re-verified them after the `Line` identity-token change.

---

## 9. The `Line` identity-token contract (ruling AE) — Plan 7 must keep this

`fr_geometry::Line` carries a private `identity: u64` taken from a process-wide
`AtomicU64`. It exists because `PolylineTrace.change` (`board/trace/PolylineTrace.java:960`,
`:972`) compares `Line`s with `!=` — **reference** comparison in Java — and which
lines it keeps changes the trace, the search tree and every route after it.

The contract, in two clauses:

* **A new token wherever Java allocates a new `Line`.** Every constructor and every
  transform that Java writes as `new Line(...)` mints a fresh token.
* **`Copy` — the same token — wherever Java passes the same reference on.** A
  `Line` moved, copied or stored keeps its identity. `Polyline::from_lines_in_place`
  exists for exactly this (quirk #188): `new Polyline(Line[])` writes flipped lines
  back into the caller's array, and those flipped entries are new objects.

And two guard rails:

* The token has **one reader**, `Line::is_same_object`, with **one caller**,
  `Board::change_trace`. Nothing reads its value; only the equivalence relation it
  induces is observable. Anything that uses it to stand in for `==` is a bug.
* Runs stay bit-reproducible, because no output, ordering, hash or serialised form
  can see a token.

`scripts/differential/java/p6t17b-bisect.patch` is the committed instrumentation
that found it, if the question ever recurs.

**Status, Plan 7 Task 8b + ruling AY (2026-08-31): the contract is measured
correct, and it is *not* what the `router-dac2020-bm01` `--steps=1-8` divergence
was — which is now CLOSED.** The patch's level-7 `CHG` ledger prints
`indexOfFirstDifferentLine`, `indexOfLastDifferentLine` and both keep counts for
every `PolylineTrace.change` call; over the first 175 connections of that board at
`ripupPassNo = 1` there are **1 499** such calls and **all 1 499 agree** between
the jar and the port. The divergence was quirk #210 — an ascending walk of a
contact set that Java's `TreeSet<Item>` walks descending, in
`TraceTightener45.smoothenStartCornerAtTrace` — and had nothing to do with `Line`
identity. Ruling AY fixed it (`scan_contacts` is `.rev()`ed) and the board now
MATCHes 294/294 at both passes. (The earlier observation that forcing
`is_same_object` to `false` moves the first divergence to connection 83 still
holds — it holds on top of a corrected contact walk too — but that is what a
deliberately-wrong identity model does, not evidence about the right one.)

---

## 10. Obligations for Plan 7

> **CLOSED, 2026-09-01 — Plan 7 Task 17's tick.** Every numbered item below carries a
> status line naming the Plan 7 task and commit that discharged it, and every "Must know"
> bullet carries one saying whether Plan 7 honoured it, widened it or found it wrong.
> **Two items were found not to exist rather than built** — `BatchAutorouterThread`'s
> per-item recovery boundary (item 2) and the multithreaded optimizer family (item 4) —
> and both are recorded on quirk #143's extended row. The headline Plan 7 hands on:
> **whole-board SES byte parity against the HEAD jar on eight stems**, all three rungs of
> ruling 1's ladder green, hash-mode independent (five `-XX:hashCode` modes, one digest
> each) and `--verify-driver`-clean against the bare jar. `docs/plan-7-handoff.md` is what
> Plan 8 starts from. **This file is otherwise frozen**; only status lines were added.

The register in `docs/java-quirks.md` is authoritative; this is the working list.
The marker inventory, with the scope spelled out — **`crates/*/src` is the code,
`crates/` additionally sweeps the READMEs' prose about the markers**, so the two
numbers differ and only the first is an inventory:

| grep | `crates/*/src` | `crates/` | of which `fr-router/src` | of which its `lib.rs` roster |
|---|---|---|---|---|
| `grep -rn "added in Plan 7" …` | **60** | 75 | 40 | 33 |
| `grep -rn "added in Plan 8" …` | **13** | 16 | 2 | 2 |
| `grep -rn "obligation:" …` | **54** | 70 | 28 | 0 |
| `grep -rn "pub seam:" …` | **8** | 10 | 8 | 0 |

`crates/fr-router/README.md` §"The 28 `obligation:` markers" tabulates the
`obligation:` column class by class with the verdict and the evidence for each.

> **Plan 7 Task 17's tick — the same greps, on the committed tree at the close of
> Plan 7 (2026-09-01):**
>
> | grep | `crates/*/src` | `crates/*/tests` | `crates/` |
> |---|---|---|---|
> | `added in Plan 7` | **0** | **0** | 3, all past-tense prose |
> | `added in Plan 8` | 32 (30 markers proper + 2 prose) | 0 | 40 |
> | `obligation:` | 60 | 6 | 86 |
> | `pub seam:` | **14** | 0 | 22 |
> | `not reachable:` (new in Plan 7, ruling AJ) | 18 | 2 | 23 |
>
> The `pub seam:` count is the one that needs explaining: the plan expected six
> (eight minus the two scan ruling 4 predicted would close). One closed
> (`AutorouteControl::from_settings`, Task 11), one did **not** and deliberately so
> (`AutorouteAttemptResult::is_routed` — the pass loop `match`es on
> `AutorouteAttemptState` directly, 13 sites, as Java's `result.state == ROUTED`
> chains do), and seven were **added**: two for ruling AP's `CancelToken` seam in
> `pipeline/stop.rs` and five for `ViaOptimizer`'s Java-private methods, which an
> integration test and a `scripts/differential` binary both call from outside the
> crate. 8 − 1 + 7 = 14. The row-by-row reconciliation is in
> `crates/fr-router/README.md` §"The `pub seam:` gate, reconciled".


`// pub seam:` is new in the Plan 6 final fix wave (finding S7) and is the answer
to a question Plan 7 will otherwise have to re-ask: **why is this `pub` item
`pub` when nothing calls it?** `rustc`'s `dead_code` lint is blind to an uncalled
`pub` item in a library, so every such item now says on its own line what its Java
declaration is and who will call it — Plan 7's fanout pre-pass for
`AutorouteControl::from_settings`, Plan 7's pass loop for
`AutorouteAttemptResult::is_routed`, and "no caller in the Java tree either" for
the six that are pure surface fidelity. A ninth was removed rather than marked;
see the README section that lists all eight.

### Must build

1. **`AutorouteConnectionRouter.route` steps 6-8** — **DONE, Plan 7 Task 8**, as
   `fr_router::route_connection_full`. **Do not re-implement steps 1-5** —
   `route_connection` is what 369 connections of byte-identical evidence attach to,
   and the wrapper is additive (plan-7 ruling 2).

   *Correction, made by that task.* The range this line gave,
   `AutorouteConnectionRouter.java:160-233`, is **wrong**: `:160` is `route`'s
   closing brace and nothing in the 255-line file spans `:160-233` as a unit. Read
   out of HEAD, the decomposition is step 6 `:95-121` (`optChangedArea` on
   `ROUTED`), step 7 `:123-145` -> `retryConnectionNecked` `:162-241`, step 8
   `:147-153` -> `applyStrictDrcAfterRoute` `:243-254`, plus the
   `maxItemIdBeforeRoute` / `strictDrcBoardSnapshot` of `:83-85` that only those
   three read. And **"the failure-log write" is not this class's**: it is Task 9's
   `AutoroutePassRunner.runSingleThread` (`:260-289`). `crates/fr-router/src/lib.rs`
   carried the same wrong range and is re-pointed.
2. **The pass loop**: `AutoroutePassRunner`, `AutorouteBatchLoop`, `BatchAutorouter`,
   `BatchAutorouterThread`. With it, the two recovery boundaries *above* the six
   this plan built: `AutoroutePassRunner.java:144` (per pass) and
   `BatchAutorouterThread.java:537` (per item). Both catch `Exception`, not
   `Throwable`, so neither recovers from a stack overflow — quirk #27 crashes both
   languages.
   > **Status (2026-08-30): partly superseded by Plan 7 ruling AM (no rayon/multithread).**
   > `BatchAutorouterThread` is rostered `// not ported:` — zero callers in `src/main`
   > or `src/test`. Its `:537` boundary therefore does not exist on any live path, and
   > Plan 7's pre-flight scan (ruling 9) found `AutoroutePassRunner.java:144` is the
   > catch of the equally-dead `runMultiThread`: `runSingleThread` has no try/catch at
   > all. **Plan 7 adds one recovery boundary, not two**, and it propagates.
   >
   > **Status (2026-08-31): the second half of the line above is WRONG, and Plan 7
   > Task 9 (`3e65333`) disproved it against Java** — independently re-verified in that
   > task's review. `:144` really does close the dead `runMultiThread` (`:40-149`), so
   > the first half stands; but `runSingleThread` (`:151-336`) opens its **own** `try`
   > at `:156` and closes it with `catch (Exception e) { job.logError(…); airLine =
   > null; return false; }` at `:331-335`, wrapping the whole method body. The
   > pre-flight scan itself said only "no **per-item** try", which is true; the
   > over-generalisation to "no try/catch at all" entered this line, the plan's Task 9
   > note 11 and the task brief.
   >
   > So **Plan 7 adds two boundaries, and only one of them propagates**: this one
   > *degrades* to `false` (the plan-6 ruling 7 shape — `AutoroutePassRunner::
   > run_single_thread`, one `catch_unwind` around the whole body, **not** per item),
   > and `AutorouteBatchLoop`'s `NoRoutableLayer` (plan-7 ruling 7) propagates. The
   > current text is in `crates/fr-router/README.md`'s recovery-boundary table and in
   > `crates/fr-router/src/pipeline/pass_runner.rs`'s method doc, which quotes the
   > `sed` window. This file is otherwise frozen; only this status line is added.
3. **The fanout pre-pass** (`BatchFanout`, `RoutingBoard.fanout`). It is the only
   thing that sets `ctrl.isFanout`, which is why two of this plan's coverage
   obligations cannot be discharged below the seam
   (`locator.rs:267`, and half of `engine.rs:1374`).
   > **Status (2026-08-30): carried into Plan 7 Tasks 11-12.** Plan 7's pre-flight scan
   > found neither coverage obligation named anywhere in that plan; Plan 7 Task 17 now
   > records each as discharged or still open.
   >
   > **Status (2026-09-01): the fanout pre-pass is BUILT (Tasks 11 and 12) and BOTH
   > coverage obligations are DISCHARGED — Plan 7 Task 17 measured each with a counter
   > on the committed tree, then removed the counter.**
   > * `locator.rs:267` — the `FoundConnectionLocator` fanout arm (`:124-129`, and the
   >   `atFanoutEnd` short-circuit at `:142-144`). Plan 6 measured **zero** entries over
   >   369 connections. Plan 7 measures **32**, all in
   >   `crates/fr-router/tests/batch_parity.rs`'s `the_ci_stems_climb_the_whole_ladder`
   >   — **ordinary CI**, the four `java_dir`-gated stems, no `FR_SLOW_PARITY` needed —
   >   and every one of those runs is SES byte-identical to the HEAD jar.
   > * `engine.rs:1374` (now `:1433`) — the `StopConnectionOption` choice. Plan 6
   >   discharged the *choice* (311 of 369 connections take the `FanoutVia` arm) but not
   >   the **difference** between the arms, which needs a real fanout via. Plan 7
   >   measures the difference firing **once**, at the `break` in
   >   `Board::connection_items` (Item.java:735), in
   >   `crates/fr-router/tests/reference_parity.rs`'s `steps_one_to_eight_matches_the_jar`,
   >   where the port is byte-identical to the jar. Both markers carry this text at the
   >   code.
4. **The optimizer**: `BatchOptimizer`, `BatchOptimizerMultiThreaded`,
   `OptimizeRouteTask`, `ItemRouteResult`, `RoutingFailureLog` (whose `fr-board`
   field is a `Vec<String>` hook today).
   > **Status (2026-08-30): partly superseded by Plan 7 ruling AM (no rayon/multithread).**
   > `BatchOptimizerMultiThreaded` and `OptimizeRouteTask` are rostered `// not ported:` —
   > reachable only from `BatchOptimizer.createForGui`. `BatchOptimizer`, `ItemRouteResult`
   > and `RoutingFailureLog` are built (Plan 7 Tasks 9, 13, 14); the `Vec<String>` hook is
   > deleted or re-pointed in Plan 7 Task 9.
   >
   > **Status (2026-08-31): the hook is DELETED, in Plan 7 Task 9.** `RoutingFailureLog` and
   > `ItemRouteResult` landed as `fr_router::pipeline::{failure_log, item_route_result}`, and
   > `Board::failure_log` — the `Vec<String>` — is gone: a tree-wide `grep -rn failure_log crates`
   > at port time found exactly three hits, its declaration, its initialiser and one
   > `assert!(board.failure_log.is_empty())` in `crates/fr-board/tests/board.rs`, i.e. **no reader
   > and no writer**. Re-pointing it was not possible in any case — the real type lives in
   > `fr-router`, which `fr-board` must not depend on — so the log is a caller-owned parameter of
   > `AutoroutePassRunner::run_single_thread`, recorded as a `// renamed:` at
   > `crates/fr-board/src/board/mod.rs`.
5. **`ViaOptimizer.optViaLocation`**, and with it `RoutingBoard.optChangedArea`
   (both overloads) and `RoutingBoard.removeItemsAndPullTight` — the batch callers
   of the tightener family this plan already ported. Quirk #34's `equals_geometric`
   at `TraceTightener*.repositionLine` was **discharged in Task 15a**; what is left
   is the callers.
   > **Status (2026-09-01): ALL DONE — Plan 7 Task 17's tick.** `ViaOptimizer` is ported
   > **whole**, not just `optViaLocation`: Task 6 landed `optViaLocation` (`:33-158`),
   > `optPlaneOrFanoutVia` (`:161-296`) and `isWithinTolerance` (`:719-732`), Task 7 the
   > three `repositionVia` overloads (`:302-365`, `:367-429`, `:434-713`), and the class
   > dropped out of the "deliberately absent" audit invocation into the main
   > `board/optimize` glob. `optChangedArea`'s two overloads are Task 5's
   > (`RoutingBoardExt::{opt_changed_area, opt_changed_area_with_keep_point}`) and
   > `removeItemsAndPullTight` is Task 8's. Evidence: `p7t3` 15/15 including mode 4 (the
   > `ViaOptimizer` arm), `p7t4` 21/21 + 20/20. Two quirk rows came out of it, **#206**
   > and **#207**, both latent on the corpus and recorded as measured rather than as
   > cleared.
6. **`RoutingBoardExt` gains** `opt_changed_area` and the pull-tight tail of
   `removeItemsAndPullTight`. The five `PolylineTrace.change` →
   `additionalUpdateAfterChange` call sites in `fr-board` carry `added in Plan 7:`
   markers; they are no-ops on every path Plan 6 runs, because `maintainDatabase`
   is `false` in every production and parity run
   (`BatchAutorouter.java:63-64,151-154`), and Task 16 measured that.
   > **Status (2026-09-01): DONE, and the five markers CLOSED as dead rather than ported
   > — Plan 7 Task 17's tick.** `RoutingBoardExt` gained `opt_changed_area` /
   > `opt_changed_area_with_keep_point` (Task 5), `remove_items_and_pull_tight` (Task 8)
   > and `fanout` (Task 11). The five `additionalUpdateAfterChange` sites were settled by
   > **controller ruling AJ** in Task 8: `maintainDatabase` is not merely `false` in
   > practice, it is reachable only through the Java benchmark-only system property
   > `retainAutorouteDatabase`, so the hook is dead on **every** live path in both
   > languages. All five became `// not reachable:` markers — a marker kind ruling AJ
   > introduced for exactly this shape — with a grep and a behavioural test each, and
   > **no `fr-board` signature changed**. `crates/fr-board/src/items/trace.rs`'s prose
   > about the convention stays.
7. **The `ConnectionToPin` trio** — `check`, `correct`, `swapConnectionToPin` in the
   tightener family. `pinEdgeToTurnDist` is `-1` throughout Plan 6, which is what
   keeps them out of reach here.
   > **Status (2026-09-01): DONE in Plan 7 Task 5 — all three, and the plan's scan was
   > wrong about one of them.** Plan 7's pre-flight scan ruling 5 recorded
   > `checkConnectionToPin` as already ported in Plan 6 and told Task 5 to reuse it; a
   > workspace search for the name, for `TraceExitRestriction` and for the method's body
   > found nothing but the two deferral markers in `crates/fr-board/src/items/trace.rs`.
   > Java won over plan text: all three landed as `PolylineTraceExt::{check,correct,
   > swap}_connection_to_pin`, pinned by `p7t6` MATCH on all five modes. Two quirk rows
   > came with them: **#205** (both methods accept `pinEdgeToTurnDist == 0`, which is
   > `BoardRules`' own seed, but their only caller demands `> 0`) and the register's
   > record of why `-1` kept them unreachable in Plan 6.
8. ~~**The `fr-board` fix ruling H decided**: `ViaRule` owning its `ViaInfo`s (§5.2).
   Controller **ruling AL** makes this **Plan 7's Task 0** — it changes an ownership
   model every later task builds on, so it goes first, and its acceptance is the
   `Issue593` repro in §5.2 turning from DIFF into MATCH.~~ — **DONE in Plan 7 Task 0.**
   `ViaRule` now holds `Vec<ViaInfo>`; the §5.2 repro
   (`run.sh p6t1 ../freerouting/fixtures/Issue593-BBD_Mars-64.dsn 50 1
   crates/fr-router/tests/data/ruling-h-redeclare.rules`) is **MATCH on all 50
   connections**, k = 6 and k = 8 included, transcript at
   `crates/fr-router/tests/data/p7t0-ruling-h-match.txt`. The *via-rule* half of the
   register row (`Network.addViaRule` → `NetClass.viaRule`) is untouched and still open.

### Must know

* **`board.start_marking_changed_area()` before every `route_connection`** —
  `AutoroutePassRunner.java:224`. Quirk #177 makes the presence of
  `board.changedArea` observable inside `TraceShover::insert`, so omitting it
  routes a **different board**, silently. §3 states it beside the signature and
  tabulates all five `route_connection` call sites: the **three production-shaped
  callers do it** (`p6t1.rs`, `tests/fixtures.rs`, `tests/reference_parity.rs`) and
  the **two that do not** (`tests/autoroute_connection.rs:1061` and `:1159`) are
  deliberate mirrors of `P6T16Probe.routeSteps1to5`, which does not mark either —
  the probe is the oracle there, so marking would break the mirror. A grep that
  finds three-of-five is finding the right answer, not a gap. This is the single
  item most likely to make a Plan 7 pass runner diverge without failing.
* **`max_passes == 0` means unlimited** (quirk #140). Not "zero passes".
  > **Honoured, Plan 7 Task 10** (`AutorouteBatchLoop::run`'s cap check reproduces
  > `:270-274`'s `maxPasses != null && maxPasses > 0 && currentPass > maxPasses`) and
  > **Task 15** (`run_pipeline`'s fanout-only mode is `maxPasses = 0` *with* the routing
  > stage skipped, which is a different thing again and is documented at the call site).
* **`-mt` is not a threading policy on the headless path** (quirk #143) — and Plan 7
  Task 17 widened that to **anywhere**: `BatchAutorouter.autoroutePassMultiThread` has no
  caller in `src/main` or `src/test`, so `RouterSettings.maxThreads` has no live reader at
  all and `BatchAutorouterThread.java` (621 loc) has zero live callers. All five
  multithread classes are `// not ported:` in `crates/fr-router/src/lib.rs` with the greps
  beside them, the `autoroute/pipeline` audit prints them as five `ROSTERED` lines so the
  deferral cannot go silent, and there is no `rayon` in Plan 7 (controller ruling AM). The
  original text:
  `BatchOptimizer.createForHeadless` (`:51-53`) never reads the field, and the
  `> 1` gate at `:58` is inside the *GUI* factory. Do not invent one — a threaded
  maze would be non-deterministic and would dissolve every acceptance criterion in
  ruling 1. `Board` is `Send + Sync` for the optimizer, and that is the only reason.
* **Quirk #76's hang is reachable, exactly as in Java** (ruling AC): the inserter
  is handed `&|| false` because Java checks no stop below `AutorouteEngine.java:265`.
  Plan 7 owns the wall clock, and it is the right place for it.
  > **Status (2026-09-01): the wall clock EXISTS and does NOT close the hang — Plan 7
  > Task 17's tick, and this needs saying plainly.** Controller ruling AI gave Plan 7
  > `RouterStop`'s deadline and `RouterBudget`, and `pipeline/stop.rs` documents the six
  > read sites. **None of them is below `AutorouteEngine.java:265`**, because Java has
  > none there either, and inventing one would change the room set and break parity. So
  > quirk **#162**'s non-terminating `calculateNewIncompleteRooms` is **still unguarded
  > and still reachable**, in both languages, and Plan 7 confirmed controller answer 5:
  > it stays unfixed. The only protection is the caller's wall clock —
  > `scripts/gen-batch-reference.sh` and `gen-router-reference.sh` bound every stem with
  > `timeout(1)`, and the slow tests carry their own bound. A hang row is an XDIFF-by-hang
  > with the quirk cited, exactly as Plan 6 handled them. **Plan 8's CLI inherits this**:
  > a `--job-timeout` that is not a watchdog cannot interrupt it.
* **Quirk #82 (Delaunay in-circle degenerate on axis-aligned input) is still not
  fixed and must not be.** It now has three consumers.
* **The eight re-marked coverage obligations** in `crates/fr-router/README.md`'s
  marker table are grouped there into "Plan 7's, not a fixture's", "a fixture the
  corpus does not have" and "deeper than 369 connections go". Four of them become
  reachable the moment Plan 7's fanout and item selection exist.
  > **Status (2026-09-01): Plan 7 Task 16 discharged one (quirk #221, via the
  > `router-fanout-bm11` stem) and re-marked four with the measured reason they are still
  > out of reach; Task 17 discharged the two §10.3 ones above. The README's table is the
  > running record.**

---

## 11. Obligations for Plan 8

> **Amended 2026-09-01 by Plan 7 Task 17.** Items 1-4 all moved or changed shape during
> Plan 7; `docs/plan-7-handoff.md`'s obligation register supersedes this list, and each
> item below says how.

1. ~~**`RoutingPipeline.createForHeadless`** and the rest of the pipeline wiring —
   Plan 7 delivers the stages, Plan 8 the caller.~~ — **MOVED INTO PLAN 7 (Task 15,
   controller ruling AK).** The whole class is accounted for: `run` is
   `fr_router::pipeline::run_pipeline`, `createForHeadless` collapses into its setup as a
   `// renamed:`, and `createForGui`, `getAutorouter`, `getOptimizer` and the three
   `add*Listener` methods are `// not ported:` in `pipeline/run.rs`. **Plan 8 has no
   `RoutingPipeline` surface left to wrap** — only `PipelineResult`, which it wraps as
   `fr-core`'s `RoutingResult`.
2. **`CancelToken` → this crate's `StopCheck`.** Six checked sites (ruling 6), each
   with a test; the type is a borrowed `&dyn Fn() -> bool`, so a `CancelToken` drops
   straight in.
   > **Amended by Plan 7 (rulings AI and AP).** There is now a *second* thing a
   > `CancelToken` has to join: `fr_router::pipeline::RouterStop`, a **three-state** stop
   > flag (`None`/`AutoRouterOnly`/`All`, quirk #202) plus ruling AI's deadline. The two
   > `// pub seam:` lines in `pipeline/stop.rs` mark where. **The three states must not
   > collapse to a bool** — `--max-items` writes `ALL` and `--max-passes` writes
   > `AUTO_ROUTER_ONLY`, and `RoutingPipeline.java:117` tests only the first, which is why
   > `--max-items` silently disables the optimizer stage and `--max-passes` does not
   > (quirk #202/#214). And the deadline's six read sites are **two** mechanisms: one
   > job-level flag and four per-stage clocks that end only their own stage. Both
   > distinctions are load-bearing.
3. **`ProgressSink` replacing the dropped observers.** `autoroute/events/**` (six
   files) and `NamedAlgorithm`'s six `add*Listener`/`fire*` methods are on the
   `not ported:` roster in `crates/fr-router/src/lib.rs`, by name.
4. ~~**`BoardStatistics`**~~ — **BUILT IN PLAN 7 Task 1** (controller ruling AG), as
   `fr_router::score`: the computing constructor, the ten DTOs, `calculateScore`,
   `getMaximumScore` and `getNormalizedScore`, pinned by `p7t7` against the HEAD jar under
   four `ScoringSettings` presets. What is left for Plan 8 is ruling 4's surface — the
   `byte[]`/`FileFormat` constructor, `countOccurrences`, the Gson `toString`,
   `BoardScoreBreakdown` and `ScoringWeightComparison` — eight `// added in Plan 8:`
   markers in `score/mod.rs`. The original text: `tests/fixtures.rs` and `p6t1`'s metric
   block stand in for it. `BoardStatistics.connections.incompleteCount` is
   `DesignRulesChecker.getIncompleteCount()` (`BoardStatistics.java:271`), which the
   port already computes.
5. **`crates/fr-dsn/src/parser/wiring.rs:596`** — `read_via_scope` still calls the
   unchecked `insert_via` wrapper and should pass the reader's own
   `normalize_time_limit`-backed check. It is the last line of the ladder-hang
   obligation; Plan 6 Task 10b built the seam and the controller re-pointed the
   wiring to Plan 8 because it changes the DSN reader's behaviour under the 105-file
   corpus.
6. **The CLI/MCP surface**: legacy-flag value normalisation wiring
   (`crates/freerouting/src/legacy.rs` still forwards raw values) and MCP
   concurrency (`mcp/server.rs`, `mcp/stdio.rs` cannot express progress or
   cancellation). Both are register rows already.

---

## 12. Parked residuals, by task

| task | residual |
|---|---|
| 2 | `RoomId` is the arena index, not Java's room id; order-exact, and every consumer resolves through the store. |
| 6 | `completeExpansionRooms` is **not** the set of complete rooms that exist (quirk #165) — the port keeps Java's list beside the arena. |
| 7 | drill arena ids would alias if a cleared engine were reused; unreachable, because the engine is nulled after `clear`. **Do not "fix" it by clearing drills.** |
| 10b | `EntryPoint.trace` is snapshotted where Java holds a live reference. |
| 12 | the `MazeTraceShover` door-section collector, `roomShapeIsThick`'s `Via` arm and the two `withNeckdown` sites were transcription-only at the time; **all discharged by Task 17's corpus**, with counts in the README. |
| 14 | `FoundConnectionLocator.connectionItems` is never `null`, so the `SKIPPED` arm is dead (quirk #180). Plan 7 must not resurrect it. |
| 15 | `get_instance` answers `Result<Option<FoundConnectionInserter>, BoardError>`: `Err` propagates as a bare `FAILED`, `Ok(None)` is a message `FAILED`, and there is no `SKIPPED`. |
| 16 | `Err` below `AutorouteEngine.java:260` panics rather than degrading — Plan 7's necked retry is the handler. |
| 17 | eight coverage obligations re-marked with the measurement that says why no corpus board reaches them. |
| 17 (N1) | **`p6t1`'s two `quote` helpers disagree about `null`, and Task 18 closed it as unreachable.** Java's `P6T1.java:502-505` renders a `null` argument as the bare token `null`; the Rust twin's `p6t1.rs:266` renders `result.details` through `unwrap_or("")`, i.e. `""`. The two would differ on a null `details` — and **Java cannot produce one**: `AutorouteAttemptResult.java:10-19` sets `this.details = ""` in the one-argument constructor and takes a string in the other, and all 21 `new AutorouteAttemptResult(…)` sites in `src/main/java` pass either no details or a literal/concatenation, never `null` (`BatchFanout.java:335,352` guard `details == null` defensively over a field that cannot be). So the null branch of the Java driver's `quote` is dead for this field, and the port's `Option::None` is the correct model of Java's `""` — which is what `crates/fr-router/src/autoroute/attempt.rs:95-102` already documents. Recorded, with a comment at the twin's site, rather than "fixed": rendering `null` there would be the divergence. |

---

## 13. Evidence

**Prerequisites for anything that touches Java.** A sibling checkout of the Java
repo at `../freerouting` (`FREEROUTING_JAVA_DIR`), its built jar at
`../freerouting/build/libs/freerouting-current-executable.jar` (`FREEROUTING_JAR`
— run `./gradlew build` in the clone if it is missing), and a JDK 25
(`JAVA25_HOME`, e.g. `/opt/homebrew/opt/openjdk@25`). `p3t3`/`p3t15` additionally
need the pinned `tools/freerouting-2.3.0.jar`. Tests that need the checkout call
`parity::require_java_dir()` and skip with a message rather than failing.
`scripts/differential/README.md` §Running it is the full list.

| claim | how to re-check |
|---|---|
| the workspace is green | `cargo test --workspace` — **1 863 passed, 0 failed, 12 ignored**, 92 binaries |
| the acceptance ladder holds | `cargo test --release --workspace` — **1 873 passed, 0 failed, 1 ignored** (the one is `a_four_rung_ladder_never_finishes_normalizing`, quirk #76's non-terminating reproduction, which exists precisely because it cannot pass); `-p fr-router --test reference_parity` alone for just the ladder |
| the fixture bounds hold | `cargo test --release -p fr-router --test fixtures` — 6 passed, ~14 s. **Not** `-- --ignored`: `debug_assertions` is off in release, so the `cfg_attr` does not apply and `--ignored` filters all six out |
| the ported Java suites pass | `cargo test -p fr-router --test java_ports` |
| the port matches the jar live | `./scripts/differential/run.sh p6t1 ../freerouting/fixtures/Issue143-rpi_splitter.dsn 8` (and the same with `100000 2` / `100000 4` on the DAC2020 board) |
| the references are portable across JVMs | `./scripts/gen-router-reference.sh --verify-hash-modes` — five stems, five modes, one file each |
| no crate has an unaccounted-for Java method | the 29 `scripts/audit-port.sh` invocations listed in `crates/fr-router/README.md` §"Every audit invocation in the workspace" |
| `fr-board`'s per-class map holds | the nine 4-argument invocations, zero `MISSING`, zero `UNMAPPED` |
| Plan 6's markers are all consumed | `grep -rn "added in Plan 6" crates/` returns nothing |
| ruling 10 is honoured | `grep -rn "item_tree_shape_ref\|item_tile_shape_ref" crates/fr-router/` returns one doc comment forbidding them |
| the quirk register is contiguous | `grep -o '^| [0-9]\+ ' docs/java-quirks.md` — 193 rows, 1 to 193, no gaps, no duplicates |
| no crate contains an `unsafe` construct | `grep -rnE 'unsafe (\{\|fn \|impl \|trait )' --include='*.rs' crates/ tests/` — **no match, exit 1**. A bare `grep -rn unsafe …` over the same paths answers **10** lines and that is expected: the eight `#![forbid(unsafe_code)]` attributes plus two doc-comment lines at `crates/fr-router/src/lib.rs:80-81`. §3's "`forbid(unsafe_code)`" subsection has both commands and the one exception |
| clippy is clean | `cargo clippy --workspace --all-targets -- -D warnings` — exit 0 |
| every differential driver is at its documented state | see the table below |
| `cargo doc` has not regressed | `cargo doc --workspace --no-deps` — see the baseline below |

### The `cargo doc --workspace --no-deps` warning baseline (exit 0)

| crate | warnings |
|---|---|
| `fr-board` | 1 (`write` is both a function and a macro) |
| `fr-drc` | 5 (private intra-doc links) |
| `fr-dsn` | 19 (private intra-doc links, mostly `read_network_scope`'s helpers) |
| `fr-settings` | 3 |
| **`fr-router`** | **0** — Task 18 fixed the three it had (one private link, two unresolved) |
| `fr-geometry`, `freerouting`, `parity` | 0 |

### Every differential driver, measured on the committed tree

`JAVA25_HOME=/opt/homebrew/opt/openjdk@25 ./scripts/differential/run.sh <driver>`,
default arguments, jar `../freerouting/build/libs/freerouting-current-executable.jar`
(and `tools/freerouting-2.3.0.jar` for `p3t3`/`p3t15`, plan-3 ruling 10):

| driver | result |
|---|---|
| `d17`, `p2t3`, `p2t3r`, `p2t10`, `p2t11`, `p2t13`, `p2t15`, `p3t2`, `p3t3`, `p3t15`, `p4t1`, `p5t1`, `p5t2` | **MATCH** |
| `p6t2` (26 979 lines), `p6t3` (7 597 lines) | **MATCH** |
| `p6t1` on `Issue143-rpi_splitter.dsn` at `max_items` = 1, 2, 3, 4, 5, 6, 7, 8 | **MATCH**, all eight |
| `e15` | DIFF, **3 lines — the documented count** (quirk #21, non-finite width) |
| `t15` | DIFF, **44 lines — the documented count** (42 cosmetic `EXC:` tags + 2 sign-of-zero, quirk #14) |
| `t16r` | DIFF, **72 lines — the documented count** (`LineSegment`/`offsetBox` with an out-of-range `no`) |
| `t14` | DIFF, **145 lines — the documented count** (`Simplex.EMPTY` and degenerate shapes, all in the totalization table) |

The four DIFF drivers are **pre-existing and unchanged by this plan**;
`scripts/differential/README.md` §"Known, expected diffs" is where the numbers are
documented, and Task 17b re-verified them after the `Line` identity-token change.

---

## 14. Open items for the user

1. **Ruling H's fix is an `fr-board` change nobody has made — scheduled as Plan 7's
   Task 0 by controller ruling AL.** The decision is made (§5.2): stop re-pointing.
   Until `ViaRule` owns its `ViaInfo`s, a `.rules` file that re-declares an existing
   `(via …)` routes differently from Java. No shipped path does that today, and no
   acceptance fixture does — which is exactly why it must go first, before more code
   depends on the index model.
2. **The `Line` identity token is a genuine, recorded exception to "no static
   mutable state"** (§9). It is the only one in the workspace, it is invisible to
   every output, and it is the reason the DAC2020 board matches at ripup passes 2
   and 4 — but somebody other than the controller should agree it is the right
   trade.
3. **`tests/fixtures.rs` runs one board in CI and five only in release.** A release
   test job would run all six in about 14 seconds; until then, four of the five
   bounds are checked by hand.
4. **Quirk #162 hangs, deliberately.** `calculateNewIncompleteRooms` does not
   terminate on ~0.4 % of random room shapes, in Java and in the port. Plan 7's wall
   clock is the containment; there is no fix that keeps parity.
