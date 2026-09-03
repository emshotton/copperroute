# Multi-threading survey — where parallelism would pay, under a hard determinism constraint

**Status:** investigation only. Nothing here is a task. Written on `plan-9-post-parity`; no source
file was touched. Measurements are from a **scratch clone** (`cargo build --release`), never from
the main checkout or the two live worktrees.

**The binding constraint, restated so no reader can miss it (user ruling):**

> Same input + same settings → **byte-identical output**, across runs and across platforms.

Parallelism is admissible here **only** where it provably cannot reorder an observable result.
Every row below carries its determinism argument in full, or says plainly that it has none. Where a
row would change bytes, it says which of the two escapes it takes: (a) it is **opt-in and the
default path is bit-for-bit today's**, so no golden moves; or (b) it needs a **golden regeneration
plus a determinism re-proof**, and the row says what that proof is.

Secondary user priority, and it outranks speed: **routing quality > wall-clock**. So §3 is about
buying *quality* at fixed wall-clock, and it is where the single best idea in this document lives.

---

## 0. What was measured, and on what

| | |
|---|---|
| Binary | `cargo build --release --bin freerouting` in a scratch clone of `plan-9-post-parity` |
| Host | Darwin 25.5.0, arm64 |
| Workload | `Issue508-DAC2020_bm01.dsn -mp 4 --router.fanout.enabled=true --router.optimizer.enabled=true` |
| Wall / user / peak RSS | **27.02 s / 26.53 s / 53.4 MB** |
| Profile | `sample(1)`, 20 s at 1 ms, **15 865 samples on exactly one thread** |

The profile confirms plan-8 handoff delta row 13 from the outside: there is **one** thread in the
whole process (`Thread_283614`, the main thread). No worker, no runtime, no pool.

**Inclusive share of the routed run** (per-symbol inclusive, recursion-deduped, % of 15 865):

| symbol | samples | % | site |
|---|---:|---:|---|
| `AutoroutePassRunner::run_single_thread` | 14 575 | **91.9** | `pipeline/pass_runner.rs:151` |
| `maze::engine::route_connection_full` | 13 387 | 84.4 | `autoroute/maze/engine.rs` |
| `MazeSearchEngine::occupy_next_element` | 10 418 | **65.7** | `autoroute/maze/search.rs` |
| `expand_to_room_doors` | 6 924 | 43.6 | `autoroute/maze/expand.rs` |
| `AutorouteEngine::complete_expansion_room` | 4 859 | 30.6 | `autoroute/maze/engine.rs` |
| `tree_ext::complete_shape_45` | 4 023 | 25.4 | `autoroute/tree_ext.rs` |
| shape-tree queries (`…with_rooms` + `…with_clearance`) | ~5 564 | ~35 | `fr-board/searchtree/shape_search_tree.rs:1078` |
| `ForcedPadRouter::check_forced_pad` | 2 963 | 18.7 | `board_ext/forced_pad_router.rs:61` |
| `expand_to_other_layers` → `ForcedViaInserter::check_layer` | 2 233 → 2 212 | **14.1** | `autoroute/maze/expansion_engine.rs:379, :680` |
| `TraceShover::check` | 2 108 | 13.3 | `board_ext/trace_shover.rs` |
| `TraceTightener::opt_changed_area` (pull-tight) | 1 667 | 10.5 | `board_ext/tightener/mod.rs` |
| `BoardStatistics::compute` | 1 203 | 7.6 | `score/statistics.rs:103` |
| `BatchFanout::fanout_board` | 1 057 | 6.7 | `pipeline/fanout.rs:1249` |

**Two measurements that settle four candidates before they are argued** (§5):

* DRC over the **routed** DAC board — the exact referee run `quality-ab.sh` performs —
  `-de …dsn …ses -drc out.json`: **0.05 s**. That is **0.2 %** of the 27 s route.
* DSN parse + DRC on the three largest fixtures in the corpus (`Issue187-processor.Z80.dsn`,
  1.99 MB; `Issue103-Board-Routed.dsn`, 1.02 MB; `Issue690-kit-dev-coldfire…`, 0.41 MB):
  **0.56 / 0.56 / 0.58 s**. Parse+write is under 2 % of a routed run on the biggest board we own.

---

## 1. The finding that changes the shape of the problem

Survey §6.1 lists four **preconditions for §6's parallelism**, and names **#61** among them: Java's
`ShapeSearchTree.lastGeneratedEntryId` is a `private static int` shared by every board in the JVM,
which the port already fixed to a per-`SearchTreeManager` `u64`. The natural reading — and the one
plan-8 handoff §9 encodes — is that this counter is *the* global sequential state, and that any
parallel tree query would draw from it racily and reorder results.

**That reading is wrong, and the counter is not a blocker at all.** Read
`crates/fr-board/src/searchtree/shape_search_tree.rs:1100-1146`:

```rust
*entry_counter += 1;
sorted_items.insert(EntrySortedByClearance { clearance: current_clearance,
                                             entry_id: *entry_counter, entry });
…
for sorted in sorted_items {          // ordered by (clearance, entry_id)
    …
    if current_offset_shape.intersects(&tmp_offset_shape) { result.push(sorted.entry); }
}                                     // ← the entry_id is DROPPED here
```

The id is drawn **only inside one call**, consumed **only** as the second key of a `BTreeSet` local
to that call, and **never escapes it** — `result` pushes `sorted.entry`, not `sorted.entry_id`. All
ids in one call are consecutive and assigned in tree-iteration order, so the ordering the sort
produces is invariant under adding any constant to every id in the call. Consequently:

> **A per-thread `entry_counter` starting at any value yields byte-identical query results.**
> The counter must only be *monotone within a call*, not globally sequenced.

Two consequences worth carrying into any future ruling:

1. #61 should be struck from the precondition list. It was already fixed, it stays fixed for
   quirk-parity reasons (#30 and #61 must not be "restored"), but it is **not** load-bearing for
   parallelism.
2. The *real* blocker is far more mundane and is stated nowhere: **the board's read paths are
   `&mut Board`**, and one of them writes. `Board::clearance_violations` is `&mut self`
   (`fr-board/src/board/clearance.rs:37`); `overlapping_items_with_clearance` is `&mut self` only
   to carry the counter (`fr-board/src/board/query.rs:382-395`); and `ForcedPadRouter::
   check_forced_pad` (`board_ext/forced_pad_router.rs:61,76-79`) genuinely mutates —
   `board.set_shove_failing_obstacle(outline)`, a **last-writer-wins diagnostic field**. Any read
   parallelism must first split "the query needs a counter" (trivially threadable, per above) from
   "the check records a diagnostic on the board" (not threadable without returning it instead).

---

## 2. What Java's own multithreading did, and exactly why it is non-deterministic

Read for the record, per the brief. `BatchAutorouter.autoroutePassMultiThread` (`BatchAutorouter.java:411-413`)
has **no caller anywhere in the tree**; it delegates to `AutoroutePassRunner.runMultiThread`
(`AutoroutePassRunner.java:40-149`). The design is a **portfolio race**, and the *idea* is good:

* deep-copy the board `maxThreads` times (`:55`);
* shuffle each copy's autoroute item list with `router.random` — which is **seeded**,
  `this.random = new Random(0)` at `BatchAutorouter.java:135`, and drawn sequentially, so the
  shuffles themselves are reproducible;
* route each copy on its own daemon thread at `Thread.MIN_PRIORITY` (`:71-74`);
* keep the best board by normalized score (`:125-135`).

It is nonetheless **known-non-deterministic**, for four reasons visible in the source:

1. **The timed join is the killer.** `autorouterThread.join(TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP)`
   (`:94`) joins with a **1000 ms** bound and then *falls through regardless*. The very next
   statement, `boardHistory.add(autorouterThread.getBoard())` (`:107`), serialises a board that a
   still-running daemon thread is concurrently mutating. On any board where a pass takes longer
   than one second — i.e. every board this survey measured — the winner is chosen from **torn
   snapshots**, and which tear you get is a function of machine speed and scheduler luck.
2. **The threads are `MIN_PRIORITY` daemons** (`:73-74`), so how much of each variant completes
   before the 1 s bound is decided entirely by host load.
3. **Two different "best" selections disagree.** `router.board = boardHistory.restoreBestBoard()`
   (`:137`) re-sorts `BoardHistory` by score (`BoardHistory.java:141`), while `anyProgress` comes
   from a separately computed `bestThread` (`:125-141`). The board that is kept and the thread that
   reports progress need not be the same variant.
4. **Shared global RNG under concurrency.** Quirk #30: `splitToConvex`'s `Random` is `static` in
   Java, so shape division is drawn from one generator that N router threads interleave on. The
   *geometry* is therefore non-reproducible independently of everything above.

Note what is **not** on that list: the seeded shuffle. Upstream got the seeding right and then
threw the determinism away on a wall-clock join and a shared static. That is why §3 below is
worth doing rather than avoiding — **the design is salvageable; the implementation was not.**

Also relevant, and rostered rather than ported: `BatchOptimizerMultiThreaded` (366 lines) +
`OptimizeRouteTask` (100 lines). Its winner selection is `synchronized boolean isWinningCandidate`
over a `HashMap<Integer, ItemRouteResult>` fed by a `ThreadPoolExecutor` and released by a
`CountDownLatch` — first-finisher-wins over an unordered map. Not salvageable in that shape.

---

## 3. QUALITY at fixed wall-clock — the best idea in this document

### 3.1 The deterministic variant portfolio

| | |
|---|---|
| **Site** | `crates/fr-router/src/pipeline/pass_runner.rs:151` (`run_single_thread`), driven from `crates/fr-router/src/pipeline/batch_loop.rs:174` |
| **Unit of parallelism** | one **complete autoroute pass** on an independent `Board` clone, per variant |
| **Speedup class** | **none — this buys quality, not speed.** N-way wider search at ~1× wall-clock up to core count |
| **Determinism** | **provable, and the default path does not move a byte** — see below |
| **Size** | **L** |
| **Verdict** | **DO — but Plan 10.** The setting + seeded-PRNG plumbing could land in Plan 9 group 18 |

This is Java's `runMultiThread` with every one of §2's four defects removed:

* **Variant seeds are fixed, not raced.** Variant *i* of pass *p* is seeded from the pair
  `(p, i)` through a stable named algorithm — ruling BK as amended: **not** `StdRng`/`SmallRng`
  (they are explicitly excluded for lack of cross-version stability), but an in-tree splitmix64/LCG
  or `rand_chacha`/`rand_pcg`. The item-ordering permutation for variant *i* is then a pure
  function of `(p, i)` and the item list.
* **Each variant is the existing sequential code, unchanged, on its own `Board`.** No shared
  mutable state at all: not the room arena, not the search trees, not `entry_counter`, not
  `shove_failing_obstacle`. `Board::deep_copy` already exists and is used per-item by the optimizer.
* **Selection is a total order with an index tie-break.** `argmax` over
  `(normalized_score, ‑variant_index)` — never "whoever finished first", never `BoardHistory`'s
  restore-count state, and never two selections that can disagree.
* **No clock anywhere.** Nothing joins with a timeout; every variant runs to completion. This is
  the whole of Java's defect (1) and (2), deleted.

The result is identical whether the variants run on 1 thread or 64, on arm64 or x86-64, on a busy
machine or an idle one — because the only inputs are the board, the settings, and `(p, i)`.

**Why no golden moves.** Make the setting `router.pass_variants`, **default `1`**, and define
variant 0's permutation as the identity. Then the default path is *literally the code that runs
today* — `run_single_thread` over `get_autoroute_items()` in its current order — and every
`tests/reference/**` byte, the two-run identity check
(`scripts/gen-cli-reference.sh:512` `--verify-two-runs`), and
`cli_e2e.rs::two_runs_of_every_ci_stem_are_byte_identical` all hold unchanged. `pass_variants > 1`
is a **new mode** that gets its own goldens when someone asks for it, and its determinism proof is
the argument above plus a two-run identity check at N=8 on the 29 stems.

**Precondition, and it is not optional.** `RouterBudget::default().fanout_ms_per_pin` is still
**10 000 ms** (`pipeline/stop.rs:529-534`) — a live wall clock in the *user-facing* budget, which
`quality-ab.sh`'s own header states "*does* change what gets routed on a big board". Variants that
share a machine contend, contention lengthens a pin's fanout, and a per-pin wall clock turns that
into different bytes. So the portfolio requires the fanout budget to be bounded by **work**, not by
clock — the same fix #234 already applied to `opt_changed_area_ms`, applied to the one field that
survived it. **Any parallelism proposal in this document inherits this precondition.**

**Cost.** 53 MB peak RSS on DAC2020 → ~8 variants ≈ 400 MB. Acceptable.

### 3.2 Optimizer: speculative per-item evaluation, sequential commit

| | |
|---|---|
| **Site** | `crates/fr-router/src/pipeline/optimizer.rs:~560-660` (`opt_route_item`), item loop at `:437` |
| **Unit** | evaluation of the next K items against the current board; **commit stays sequential** |
| **Speedup class** | acceptance-rate dependent. If ~10 % of items improve, ≈ K-way on the other 90 % |
| **Determinism** | by construction — see below |
| **Size** | **L** |
| **Verdict** | **DO — Plan 10, strictly after group 9 / #227** |

Today this stage is free: #227 records that the optimizer "runs, visits every item, and changes
nothing", because `autoroutePassesForOptimizingItem` runs **zero** passes per item. Group 9's fix
makes it real, and it is the single largest quality change in the catalogue — and it will make
this loop the hottest thing in the program, because each item costs **one whole-board deep copy**
(`optimizer.rs:~620`, `board.deep_copy()`) plus a rip, a re-route, and a
`calculate_incomplete_count` (a throw-away `DesignRulesChecker` over the whole board).

The loop is sequential *in appearance* — item *n+1* sees whatever item *n* committed — but
**most items are rejected**, and a rejected item leaves the board exactly as it found it. That is
the classic optimistic-parallel shape:

1. Evaluate items *n … n+K‑1* concurrently, each on its own `deep_copy` of the **same** base board.
2. Commit in **index order**. On the first *accept*, commit it, discard every speculative result
   whose evaluation base is now stale, and re-evaluate from *n+1* against the new base.
3. Rejections commit nothing, so a run of rejections retires K items for one item's latency.

**Determinism argument:** the committed sequence is byte-identical to the sequential one, because
(a) commits happen in the same index order, (b) a speculative result is only ever committed if its
base board is exactly the board the sequential run would have handed it, and (c) any result whose
base was invalidated is *discarded and recomputed*, never accepted. There is no tie-breaking and
no clock. This one **can** be made byte-identical to the post-#227 sequential golden — which means
it needs **no separate goldens**, only the goldens group 9 regenerates anyway.

---

## 4. Speed opportunities, ranked by honesty

### 4.1 Harness: `quality-ab.sh` quality + referee lanes — S, zero risk

| | |
|---|---|
| **Site** | `scripts/quality-ab.sh:511-560` (`measure_one`), driven over 29 stems |
| **Unit** | one stem per worker process |
| **Speedup class** | **~1.25×** on the harness — and that ceiling is structural, see below |
| **Determinism** | **nil risk**, conditionally |
| **Size** | **S** |
| **Verdict** | **DO — Plan 9 tail** |

Per stem the script performs 1 quality run + 1 referee DRC + `REPEATS`(=3) timed runs. The corpus
`cpu_s` total is **111.4 s** (`benchmark/baselines/stem-times.tsv`, 29 rows; DAC2020 alone is 18.0 s),
so a sweep is ≈ 4 × 111 s ≈ **450 s**.

The controller's sketch is exactly right and the arithmetic is worth stating, because it caps the
prize: **the timing lane must stay sequential** (3 × 111 s = 334 s, irreducible — CPU-time
measurement under contention is not a measurement), so only the quality lane and the referee are
parallelisable. Fanning those out collapses ~111 s to the longest stem (~20 s), saving **~90 s of
450 s ≈ 20-25 %**. Worth an afternoon; not worth a week.

**Determinism argument.** Separate processes, per-stem scratch paths already
(`$SCRATCH/$family-$stem.*`), rows appended and then sorted by `(family, stem)` before the tsv is
written — merge order is already canonical. The quality lane sets `FR_ROUTER_BUDGET=disabled`
(`:539`), which sets `fanout_ms_per_pin` to `i32::MAX`, so §3.1's contention hazard **cannot** fire
there. That conditional is load-bearing: parallelising a lane that runs the *default* budget would
be a correctness bug, not a speedup. Encode it as an assertion in the script, not a comment.

### 4.2 Harness: reference regeneration — S, one real trap

| | |
|---|---|
| **Site** | `scripts/gen-cli-reference.sh:541` (`each_row`), `scripts/gen-batch-reference.sh:511`, and `--verify-two-runs` at `:512` |
| **Unit** | one stem per worker (two-runs mode: 2 routes per stem) |
| **Speedup class** | near-linear to core count on a 13-stem (CLI) / 8-stem (batch) corpus; `--verify-two-runs` doubles the work and benefits most |
| **Determinism** | **safe only with the budget pinned** |
| **Size** | **S** |
| **Verdict** | **DO — Plan 9 tail, with the guard** |

This is where a naive fan-out **does** change golden bytes. These generators run the port in its
*default* configuration deliberately — the whole point of a reference is "what a user gets" — and
the default budget still carries `fanout_ms_per_pin = 10 000` (§3.1). N concurrent routes on M
cores lengthen each other's per-pin fanout, and a board that trips the 10 s limit under contention
routes differently from one that did not. `--verify-two-runs` would then report exactly what it was
built to report — "*Something in this run depends on the machine rather than on the board*" — and
it would be **right**.

So: either land the work-bounded fanout budget first (§3.1's precondition, which retires the hazard
outright), or cap concurrency at one route per physical core and re-run `--verify-two-runs`
serially as the acceptance step. The first is the real answer.

### 4.3 Maze: per-layer via feasibility in `expand_to_other_layers` — L, marginal

| | |
|---|---|
| **Site** | `crates/fr-router/src/autoroute/maze/expansion_engine.rs:379` and `:680`; `board_ext/forced_via_inserter.rs:41` |
| **Unit** | one candidate `(layer, ViaInfo)` feasibility check |
| **Speedup class** | 14.1 % of the run is here; with 3-4 layers × a handful of via infos, realistically **8-10 %** wall |
| **Determinism** | achievable, at a price — see below |
| **Size** | **L** |
| **Verdict** | **defer to Plan 10, lowest confidence in this document** |

`check_layer_with_any_matching_via` (`:680-730`) loops over the via rule's `ViaInfo`s and, for each,
calls `ForcedViaInserter::check_layer`, which returns a `CheckDrillResult`. The **values** are
order-independent — each is a pure function of `(board, layer, via_info, room_shape, location)` —
and §1 proves the tree query's counter is offset-invariant, so per-thread counters are free.

What stops it being a clean win is three things, and they are all real:

1. `ForcedPadRouter::check_forced_pad` **writes the board**:
   `board.set_shove_failing_obstacle(outline)` at `forced_pad_router.rs:76-79`. That is
   last-writer-wins state a later step reads. Threading requires returning the diagnostic out of
   the check and having the sequential caller apply the *winner's* — a signature change through
   `check_layer` → `check_forced_pad` → `ShapeTraceEntries::store_items`.
2. The whole path is plumbed `&mut Board` (both `search: &mut MazeSearchEngine` and
   `board: &mut Board`). Converting to `&Board` + an explicit counter/diagnostic out-param is a
   refactor of the hottest and most parity-fragile code in the port.
3. `check_forced_pad` takes `time_limit: Option<&TimeLimit>` — another wall clock in the inner
   loop, inheriting §3.1's precondition.

**Honest verdict:** an L-sized refactor of the code with the least slack in it, for 8-10 %. If
`pass_variants` (§3.1) lands, the cores are better spent there. Recorded so the candidate is not
lost; not recommended.

---

## 5. KEEP-SEQUENTIAL — twelve verdicts

**1. The maze frontier — `MazeSearchEngine::occupy_next_element`** (`autoroute/maze/search.rs`;
65.7 % of the run). A best-first search over a priority queue where each expansion pushes into the
queue the next expansion reads. Parallel expansion reorders the frontier, and the frontier order
*is* the route. No deterministic scheme short of §3.1's whole-pass portfolio.

**2. Room completion — `complete_neighbour_rooms`** (`autoroute/maze/engine.rs:1036`; 30.6 %
inclusive via `complete_expansion_room`). Look at `:1063`: completing one neighbour resets
`index = 0` and restarts the door walk, because completion **adds doors to the room being walked**.
A restart-on-mutation iteration over a shared arena is the definition of not-parallel.

**3. Shove / push — `TraceShover::check` / `::insert`** (`board_ext/trace_shover.rs`; 13.3 %). The
shove *is* the board mutation; its result is the state the next check reads.

**4. Trace pull-tight — `TraceTightener::opt_changed_area`** (`board_ext/tightener/mod.rs`; 10.5 %).
Mutates the board in place per connection, over an area the previous connection changed.

**5. The fanout per-pin loop** (`pipeline/fanout.rs:1468-1500`; 6.7 %). `board.fanout(…)` mutates
the board for the next pin, and the loop reads `board.start_marking_changed_area()` between pins.
Speculation is conceivable; 6.7 % does not pay for it.

**6. The batch pass loop** (`pipeline/batch_loop.rs:174`). Passes are sequential *by definition* —
pass *n+1* consumes the board pass *n* produced. Not a candidate; §3.1 parallelises *within* a pass
for this reason.

**7. DRC per-item pair checks** (`fr-drc/src/checker.rs:102-136`,
`fr-board/src/board/clearance.rs:37`). The brief flags these as "look independent", and they are —
the loop is `for id in items_in_board_order() { for v in board.clearance_violations(id) }`, dedup'd
into a `BTreeSet` keyed on the sorted id pair, so the merge order is already canonical and a
parallel version would be provably identical. **It is still not worth doing: measured 0.05 s** on
the routed DAC board — **0.2 %** of the 27 s route. #153's caching fix (group 19) is the right
lever here, and it is sequential.

**8. Delaunay / ratsnest per-net build** (`fr-drc/src/checker.rs:408`,
`fr-drc/src/net_incompletes.rs:70`). The single cleanest embarrassingly-parallel unit in the
codebase: `NetIncompletes::new(net_number, items, board: &Board)` — **immutable board**, one
triangulation per net, results collected into an index-ordered `Vec`, merge order therefore
canonical for free. And it lives entirely inside item 7's 0.05 s. A textbook example of a perfect
determinism story attached to no measurable time.

**9. DSN / SES parse and write** (`fr-dsn`, 23 890 lines). Measured **0.56-0.58 s** on the three
largest fixtures we own, against a 27 s route. Under 2 %. The lexer is a table-driven DFA
(`lexer/tables.rs`, 3 360 lines) and the writer is a single ordered walk; both would be awkward to
split and neither would show up.

**10. `BoardStatistics::compute`** (`score/statistics.rs:103`; 7.6 %). Its cost is almost entirely
`get_all_clearance_violations` (item 7) plus an item walk. The fix on the table is **#153** — stop
re-running it once per `BoardStatistics` between board mutations — which removes the work rather
than spreading it. Cache first; never thread what you can delete.

**11. `cargo test` lanes.** Already parallel. `grep` for `test-threads` / `test_threads` across
`*.toml`, `*.sh`, `*.rs` returns **nothing**, and `.cargo/config.toml` sets only an sccache note.
libtest's default thread-per-core is in force. No work owed.

**12. Java's `autoroutePassMultiThread` / `BatchOptimizerMultiThreaded`.** Rostered
`// not ported:` and must stay that way (§2). Nothing in it should be ported *as written*; §3.1
takes the idea and leaves the implementation.

---

## 6. Cross-references

| row | what it says | how this survey lands against it |
|---|---|---|
| **#143** (plan-8 handoff §9, delta 14) | `-mt` is dead **everywhere**, not merely headless; neither thread-count field is read on any live path | Confirmed at source (§2). Any threading policy is a **recorded product decision**, not a task detail. Nothing here proposes reviving `-mt`; §3.1 wants a new, honestly-named `router.pass_variants` |
| **#124** (survey §6.1; group 21) | `validate` and `normalizeMaxThreads` disagree about `maxThreads == 0` → a zero-sized pool | Still a precondition **if** a pool is ever built from a settings field. §3.1's setting should be validated in its own right and not inherit `maxThreads` |
| **#30, #61** (survey §288, ALREADY-FIXED) | "must not be restored; preconditions for any future parallelism" | **#30 stands** (a shared static RNG under concurrency is exactly Java's defect 4). **#61 does not** — §1 proves the counter is offset-invariant. Recommend amending the row |
| **#198** | the read-lock-that-writes | The port's analogue is broader than the row suggests: `&mut Board` on *read* paths, and one genuine write (`set_shove_failing_obstacle`). §1, §4.3 |
| **#234** (group 1, landed) | the `optChangedArea` wall clock, defaulted to `0` | Landed and load-bearing. But it fixed **one** of two clocks — see the next row |
| **#227** (group 9) | the optimizer stage runs and changes nothing | Makes §3.2 both possible and necessary: the stage becomes the hot loop the moment it starts working |
| **#235** | `RoutingFailureLog.shouldSkip`'s only caller is on the dead multithreaded path | Consistent with §2: the give-up policy was written *for* the portfolio. If §3.1 lands, #235's policy question returns with it |
| **plan-8 §6.1 / ruling AM** | "**No threading policy. No rayon, no threads.** A threaded maze would be non-deterministic and would dissolve every acceptance criterion in ruling 1" | **Unchallenged for the maze** — §5 item 1 agrees, and §3.1 explicitly does *not* thread the maze: it runs N whole, unmodified, sequential mazes. The ruling's stated reason does not reach a portfolio of independent sequential runs, but lifting it is a **controller decision**, not this survey's |
| **survey §7.2 / `--verify-two-runs`** | two runs of every stem must be byte-identical | The one gate every row here is measured against. §4.2 is the only proposal that could break it, and §4.2 says how |

**Dependency ruling flagged, as asked:** `rayon` is not in `Cargo.toml` and there is currently no
`rand` either. §3.1 needs **both** a threading primitive (`std::thread::scope` suffices — the
variants are a fixed-size fork/join with no work-stealing, so `rayon` buys nothing here and
`std` avoids the dependency question entirely) and a **PRNG**, which under ruling BK must be a
stable named algorithm: an in-tree splitmix64 is ~20 lines and adds no dependency at all.
Recommendation: **`std::thread::scope` + an in-tree splitmix64. No new dependencies.**

---

## 7. Ranked shortlist — what I would actually do

| # | item | site | class | determinism | size | where |
|---|---|---|---|---|---|---|
| 1 | **Deterministic variant portfolio** — N seeded pass variants on independent board clones, `argmax(score, ‑index)` | `pipeline/pass_runner.rs:151`, `pipeline/batch_loop.rs:174` | **quality**, N-way wider search at ~1× wall-clock | provable; `pass_variants=1` default is today's bytes → **no golden moves** | L | Plan 10 (plumbing could ride group 18) |
| 2 | **`quality-ab.sh` quality+referee fan-out** | `scripts/quality-ab.sh:511-560` | ~1.25× (capped by the sequential timing lane) | nil — separate processes, canonical merge, `FR_ROUTER_BUDGET=disabled` already set | S | **Plan 9 tail** |
| 3 | **Reference-generator fan-out** (`gen-cli`, `gen-batch`, `--verify-two-runs`) | `gen-cli-reference.sh:541`, `gen-batch-reference.sh:511` | near-linear to core count | safe **only** once the fanout budget is work-bounded, or with a per-core cap + serial verify | S | **Plan 9 tail**, gated |
| 4 | **Optimizer speculative per-item eval, sequential commit** | `pipeline/optimizer.rs:437, ~560-660` | ≈ K-way over the ~90 % of items that are rejected | byte-identical to the sequential post-#227 golden by construction → **no separate goldens** | L | Plan 10, after group 9 |
| 5 | **Per-layer via feasibility in `expand_to_other_layers`** | `autoroute/maze/expansion_engine.rs:379, :680` | 8-10 % wall | achievable — needs `&Board` refactor + returning `shove_failing_obstacle` instead of writing it | L | Plan 10, **lowest confidence; would not fight for it** |

**Cross-cutting precondition for 1, 3 and 5:** bound `fanout_ms_per_pin` by **work**, not by wall
clock (`pipeline/stop.rs:529-534`). It is the last live machine-speed dependency in the default
CLI budget — #234 fixed its twin and left this one standing.
