# The stale tree-index report — quirk #193

Plan 9 Task 17. **A discovery workstream, not a fix.** Nothing in this task changes a routing
decision; #193's three guards are exactly where Plan 6 transcribed them and #193's register status
stays `pinned`.

Ruling BP7 makes this file Task 8's first read: T8 opens by recording, one line per row of its fix
list, `unaffected` / `subsumed by #193's finding <n>` / `narrowed: <what changed>`. The
[subsumption table](#the-subsumption-table-for-task-8s-nine-rows) below is that input.

---

## 1. The three guards, and the four exits they have

#193's register row names three HEAD-only sites. One of them has two exits that answer different
questions, so the measurement counts four — five rows, because `ItemAutorouteInfo`'s resize and its
out-of-range `null` are separately interesting.

| tag | Java | port | what the guard refuses |
|-----|------|------|------------------------|
| **G1a** | `MazeSearchEngine.java:653-659` | `autoroute/maze/expand.rs`, `expand_to_target_doors` | a `TargetItemExpansionDoor` whose `treeEntryNo` is `>= item.treeShapeCount(autorouteSearchTree)` |
| **G1b** | `MazeSearchEngine.java:660-668` | the same function, one `continue` later | `getTraceConnectionShape(tree, treeEntryNo)` answered `null` |
| **G2-resize** | `ItemAutorouteInfo.java:59-66` | `autoroute/item_info.rs`, `prepare_room_slot` | nothing — it **reallocates** `expansionRoomArr` and `System.arraycopy`s the overlapping prefix |
| **G2-oob** | `ItemAutorouteInfo.java:68-76` | the same function | an index past the (possibly just resized) array; Java logs `FRLogger.warn` and returns `null` |
| **G3** | `MazeTraceShover.java:62-67` | `autoroute/maze/trace_shover.rs`, `check_shove_trace_line` | a `traceCornerNo` beyond `lines.length - 2` |

**G1a and G1b are not the same test.** G1a bounds the index against the **search tree's** shape
count (`item.treeShapeCount(autorouteSearchTree)`); G1b's refusal comes from
`PolylineTrace.getTraceConnectionShape:919`, which bounds it against the trace's **own**
`tileShapeCount()`. Those are different numbers whenever clearance compensation splits a trace's
tree shapes, so G1b is live exactly when the two disagree. The port reproduces both bounds verbatim
(`crates/fr-board/src/items/trace.rs:492`).

---

## 2. The instrumentation

`crates/fr-router/src/autoroute/instrument.rs` — a plain module, no `#[cfg(feature)]`, gated by
`P9T17_STALE` (or `instrument::set_on` in-process for tests), in the shape of the existing
`P7T14B_MAZE` ledger (quirk #229). It records, per guard:

* **visits** — how many times the guard's test was *evaluated*. Without this a zero fire count is
  unreadable: "reached and never tripped" and "never reached" are opposite findings.
* **fires** — how many times it tripped.
* **the preceding board mutation** — the last of five router-side board writes tagged at its own
  site: `reduceTraceShapesAtTiePins`, `removeItems` (ripup), `removeTraceTails`,
  `insertForcedTracePolyline`, `optChangedArea`.
* **a recoverability verdict** at the site, and a bounded sample of the first 64 fire records.

> **The brief's `RouterCounters` interface is not available, and the counters do not live there.**
> `fr_router::pipeline::RouterCounters` is a byte-parity DTO whose nine fields are asserted against
> the JVM's reflected declaration order by
> `crates/fr-router/tests/stop_and_progress.rs::router_counters_field_list_matches_java`; a tenth
> field would fail that assertion and change the serialized progress payload. This is a correction
> to the plan's "interfaces produced", not a deviation from its intent.

**The byte-identity gate.** `stale_index.rs::instrumentation_changes_no_board_byte` routes all
eight batch stems twice, counters off and on, and requires a byte-identical SES. Green on all
eight. Every recorder is a read behind `instrument::on()`; no guard decision reads anything the
module holds.

---

## 3. The measurement

Release build, all eight batch stems of `tests/reference/router-fixtures.txt`, through the same
ruling-AW ladder `batch_parity.rs` uses (`resolve_headless` → `prepare_board` → `run_pipeline`).

| stem | G1a | G1b | G2-resize | G2-oob | G3 | **fires (all five)** |
|------|----:|----:|----------:|-------:|---:|---------------------:|
| `router-rpi-splitter` | 76 | 76 | 1 042 | 1 042 | 111 | **0** |
| `router-dac2020-bm01` | 13 210 | 13 210 | 336 796 | 336 796 | 39 896 | **0** |
| `router-j2-reference` | 2 587 | 2 587 | 17 843 | 17 843 | 1 053 | **0** |
| `router-tutorial-board` | 0 | 0 | 0 | 0 | 0 | **0** |
| `router-ecc83-input` | 26 | 26 | 89 | 89 | 4 | **0** |
| `router-fanout-bm11` | 14 952 | 14 952 | 73 088 | 73 088 | 5 461 | **0** |
| `router-strict-drc-cnh` | 3 665 | 3 665 | 60 219 | 60 219 | 7 353 | **0** |
| `router-empty-board` | 0 | 0 | 0 | 0 | 0 | **0** |
| **total** | **34 516** | **34 516** | **489 077** | **489 077** | **53 878** | **0 / 1 101 064** |

The cells are **visits**; the last column is the fire count, which is zero in every cell of every
guard on every stem.

`router-tutorial-board` and `router-empty-board` route nothing (438 empty `@:no_net_N` nets and a
board with no routable signal layer respectively), so their zeros are the board, not a missing
probe. The other six reach every guard site, three of them hundreds of thousands of times.

### The mutation census

| stem | tie-pin reduction | ripup `removeItems` | `removeTraceTails` | `insertForcedTracePolyline` | `optChangedArea` |
|------|------------------:|--------------------:|-------------------:|----------------------------:|-----------------:|
| `router-rpi-splitter` | **0** | 0 | 0 | 141 | 5 |
| `router-dac2020-bm01` | **0** | 131 | 150 | 9 017 | 196 |
| `router-j2-reference` | **0** | 6 | 6 | 845 | 15 |
| `router-ecc83-input` | **0** | 0 | 0 | 67 | 14 |
| `router-fanout-bm11` | **0** | 36 | 38 | 2 653 | 82 |
| `router-strict-drc-cnh` | **0** | 32 | 39 | 3 296 | 174 |

**`reduceTraceShapesAtTiePins` reduces nothing on any corpus board.** That is a finding in its own
right: `MazeSearchEngine.init:970-971` is the *only* board write inside the maze search, and on the
corpus it is a no-op. Everything else in the census happens strictly after the search that could
have been corrupted by it.

---

## 4. The characterisation — why the indices do not go stale

**Three sentences.**

1. **The staleness is real, and it is reachable.** `stale_index.rs`'s directed case takes a routed
   trace, records the tree-shape index a door would have held, shortens the trace through
   `Board::replace_trace_geometry` (the port of `PolylineTraceSearchTreeAdapter.replaceGeometry`,
   the path every pull-tight, shove and split takes), and the recorded index **is** then out of
   range. The guards are not defending against nothing.
2. **It cannot reach the guards, because Java throws away the thing that would notice.**
   `replaceGeometry:38` calls `clearDerivedData()`, which sets `autorouteInfo = null` and drops the
   precalculated tree shapes (`Item.java:1056-1065`); `Item.getTreeShape:212-226` additionally
   *re-derives* on an out-of-range index and only then answers `null`. So a shape-count change and
   the destruction of `expansionRoomArr` are the same event, and G2 can never see a live prefix
   under a changed count. The directed test asserts both halves in one run.
3. **And the window is empty anyway.** Every mutation in the census lands **outside** the search
   whose indices it could invalidate: `reduceTraceShapesAtTiePins` runs at `init`'s first two lines,
   *before* any room or door is created (`MazeSearchEngine.java:970-971` against `:973+`); the ripup
   and the tail removal run at `autorouteConnection:260-263`, after the maze search has returned;
   the inserter and `optChangedArea` run after that. Doors and rooms live in one `AutorouteEngine`,
   which lives for one connection. **There is no board write between the first door's construction
   and the last door's use.**

The register row's improvement column — *"find out why the indices go stale and fix that"* — is
therefore answered as: **on this codebase, at HEAD, over this corpus, they do not.** The guards are
a defence against a window that the ordering of `init`, `autorouteConnection` and
`clearDerivedData` has already closed.

### What this does not prove

* **It is a corpus result, not a proof.** Sixteen boards, eight routed stems, 1.1 million guard
  evaluations, zero fires. A board with a multi-net tie pin that actually reduces (census column 1
  is zero everywhere) would exercise the one in-`init` mutation, and none exists in the corpus.
  A construction that reuses an `AutorouteEngine` across connections would reopen the window.
* **It is a port result, and the Java side is inferred, not measured.** A fired guard `continue`s
  past a target door and changes the search; `batch_parity.rs` gets a **byte-identical SES against
  the HEAD jar on all eight stems**, which is strong evidence the jar's guards fire the same zero
  times. It is not conclusive — a fire whose skipped door was dominated anyway would be invisible in
  the SES.
* **The Java history was not read.** The worktree isolation forbids git operations against the Java
  clone, so *why* HEAD added the three guards (which crash report, which issue) is unanswered. The
  comments say what: G1a and G2 exist to suppress `FRLogger.warn`s
  (`PolylineTrace.java:920`, `ItemAutorouteInfo.java:69`) that were firing somewhere. See
  [concerns](#6-open-questions).

---

## 5. The subsumption table for Task 8's nine rows

**Ruling BP7's blanket argument first**, because it covers all nine at once: a symptom cannot be
downstream of a guard that never ran. All three of #193's guards fired **zero** times in 1 101 064
evaluations across the eight stems, and Task 8's fix list is measured on those same boards. **No row
of Task 8's fix list has any part of its measured symptom explained by #193's mechanism.** The
per-row lines below give the mechanism-level reason as well, so that the verdict does not rest on
the corpus result alone.

| # | verdict | evidence |
|---|---------|----------|
| **#159** | **unaffected** | `ShapeSearchTree90Degree.completeShape` drops a room by a **shape** decision in the 90-degree override; no tree-shape *index* is involved and the room is dropped at completion, not looked up later. The regime has no corpus board at all (T8 supplies the fixture), so it cannot even share the measurement. |
| **#160 + #161** | **unaffected** | A **comparator** defect: `SortedRoomNeighbour.compareTo` is not a total order and its `TreeSet` drops elements it calls equal (481 per 2 000). The drop happens at `add`, before any index is dereferenced. #161's item-id/room-id subtraction is a key choice, not a stale value. |
| **#164** | **unaffected** | A **compile-time overload binding** (`otherRoom(room)` binds the narrowing signature) plus a missing `touchingSides` length check. Both are static properties of `removeCompleteExpansionRoom`; neither reads a tree-shape index. |
| **#163** | **unaffected** | A loop variable (`currentCorner`) never advanced, so the last side is skipped: 7 doors where 8 are due. Purely local to `Sorted45DegreeRoomNeighbours`; JVM-verified independently of any mutation. |
| **#171 + #170** | **unaffected** | A four-key tie in `MazeListElement.compareTo` answering `0`, and a `NaN` `sortingValue` breaking transitivity. Both are ordering defects at `TreeSet.add`. The `door.getId()` hash they involve is #156/#167's identity hazard, not #193's index hazard. |
| **#165 + #166** | **unaffected** | The closest thematic neighbour, and still a different object. #165 is **room lifetime** — rooms abandoned while still wired to live doors, and `completeExpansionRoom` handing `completeShape` a room that is not in the tree it queries. #193's three guards test **indices into an item's shape array**; none of them tests room membership, so no #193 guard could have caught or masked #165, and none fired. #166 is a `catch` returning an empty collection, independent of both. |
| **#178** | **unaffected** | `MazeSearchEngine.init` ignoring the `boolean` its own `add` returns. A discarded return value in `init`, evaluated before the first room exists. |
| **#156 + #167 + #158** | **unaffected** | Three ids that are hashes over mutable state. The measurement is relevant but does **not** narrow the row: #156's stated hazard is *aliasing and overflow* (`indexInItem >= 1024`, item id `>= 2²¹`) rather than index drift, and it stands whether or not the index moves; #167 is JVM-pinned as live (`id=-29760001 → -29759999 → -29759998` across two `getDrills` calls) and its mutating field is `netNumber`, which #193 does not touch; #158's id moves when the **room's own** shape is replaced by `completeShape`/`divideLargeRoom`, an in-engine operation with no board write. The row keeps all three parts and its fix sketch is unchanged. |
| **#162** | **unaffected** | Non-termination in `calculateNewIncompleteRooms` when a room's shape has more border lines than its `toSimplex()`. A loop-bound defect over a room's own geometry; reachable at 0.4 % of room completions with no board mutation in sight. |

**Nine rows, nine `unaffected`.** The plan anticipated this outcome in as many words: *"If every row
reads `unaffected`, that sentence is the record and the fix list stands at nine."* It does. **Task 8
drops nothing, narrows nothing, and its fix list stands at nine rows.**

---

## 6. Open questions

1. **Why does HEAD have these guards?** Unanswered — the Java clone's git history is out of reach
   from an isolated worktree. The comments point at two `FRLogger.warn` sites
   (`PolylineTrace.getTraceConnectionShape:920`, `ItemAutorouteInfo.getExpansionRoom:69`), which
   reads as "somebody's log was noisy" rather than "somebody's board crashed". Worth one `git log
   -L` from a non-isolated session before any decision acts on this report.
2. **Is the GUI the real caller?** Every claim here is about the headless batch path. The
   interactive board editor re-enters the autorouter with a board a human has just modified, and
   `AutorouteEngine`'s lifetime there is not this port's concern (no GUI, `global-constraints.md`).
   If the guards were added for the GUI, the corpus can never show it.
3. **`reduceTraceShapesAtTiePins` is a no-op on the whole corpus.** Zero reductions on six routed
   boards. That is either "no corpus board has a multi-net tie pin contacting a foreign trace" or a
   port defect in the predicate at `:157-162`. It is *not* a #193 question and it is **not** in Task
   8's fix list; it deserves its own survey row.

---

## 7. Recommendation for #193's register row

The plan asks for three named options. In preference order:

1. **Leave the guard and document it — recommended.** The guards are free (five branches on paths
   already walked 1.1 million times), they cost nothing measurable, and they are the parity jar's
   own code. The register row's improvement column should be rewritten from *"find out why the
   indices go stale and fix that"* to *"measured: they do not, on this corpus — the ordering of
   `init`, `autorouteConnection` and `clearDerivedData` closes the window; keep the guards as the
   cheap assertion they are, and cite this report."* **Status stays `pinned`.**
2. **Make the index a generation-checked handle.** Give each item a shape-array generation counter
   and store `(index, generation)` in `TargetItemExpansionDoor` and `ObstacleExpansionRoom`. This
   would turn the silent `continue` into a decidable "stale" — but it changes the port's data model
   away from the jar's for a condition measured at zero, and it cannot be validated against the jar.
   **Not recommended before there is a board that fires a guard.**
3. **Re-derive the index at use.** Cheapest to write and the worst of the three: re-deriving would
   *change routing* wherever a guard fires, which is nowhere, so it buys nothing and risks a
   divergence the corpus cannot detect.

**The one thing to do before any of the three:** answer open question 1. A crash report attached to
these guards would change the recommendation; the absence of one is currently an absence of
evidence, not evidence of absence.

---

## 8. Reproducing

```
FR_SLOW_PARITY=1 cargo nextest run --release -p fr-router --test stale_index --run-ignored all --no-capture
```

Four tests, all green:

* `the_three_guards_are_counted_on_every_router_stem` — §3's two tables, asserted exactly;
* `instrumentation_changes_no_board_byte` — the byte-identity gate, eight stems;
* `a_shortened_trace_makes_a_recorded_index_stale_and_clears_the_array_that_held_it` — §4's
  directed case;
* `the_stem_table_matches_the_fixture_file` — the stem table against `router-fixtures.txt`.

In a debug build the first three are `#[ignore]`d (they need a routed board); `cargo nextest run -p
fr-router` runs 596 tests and skips them.
