# Via count, via placement and trace length — an exploration of the levers

Worktree `router-via-length-exploration`, branched from `plan-9-post-parity-uncomment` at `aa15a2a`.
Read-only survey plus a settings-only sweep of the release binary; no router code was changed.
Every file:line cite is that tip. The Java clone is cited only where the port reproduces it.

Question: *how would we modify the autorouter so finished boards use fewer vias, better-placed
vias, and shorter traces?* The answer is ranked by measured effect, and the top two need no new
algorithm at all — they are places where the router's own cost model disagrees with the score it
is judged by.

---

## 0. Findings, ranked

| # | finding | measured | size |
|---|---|---|---|
| **F1** | **The optimizer stage still does nothing on any fully routed board.** `#227` woke it, but `optimizer_near_perfect_exit` (`optimizer.rs:432`) returns before the first pass whenever `score × 1.01 ≥ 1000`, i.e. whenever `normalized_score ≥ 990.1`. A routed board scores ~999.98. So the stage runs only on boards with unrouted connections, where its route-work budget then bounds it. | threshold `0`: rpi-splitter vias **9 → 2**; j2-reference + via cost 150: vias **18 → 12**, cost −13 % | S |
| **F2** | **The maze prices a via in a different currency from the score.** Maze: `via_costs × via_radius_in_board_units` (`control.rs` `rebuild_via_info`), ≈ 50 × r_mm millimetres of trace, ~20 mm at r = 0.4 mm, and ×0.1 on pure-SMD nets (`#172`). Score: 50 mm per via. The maze under-prices vias 2.5–4×, and 25–40× on SMD nets. | via cost 150 on bm11: vias **57 → 38** (−33 %), length +0.4 %, incompletes +1 | S |
| **F3** | **The optimizer's accept test trades unequal things.** `ItemRouteResult::new` (`item_route_result.rs:31`) is lexicographic on (incompletes, via count, total length) and never looks at bends. It accepts a re-route that saves 0.1 mm and adds 14 bends. | j2 threshold 0, default via cost: length −2.5 %, bends **38 → 52**, benchmark cost **+8 %** | S |
| **F4** | **The maze bend cost is inert.** `bend_costs` is added in raw board units and clamped to `0.0..=9.9` (`router_settings.rs:192-194`); one millimetre is 10 000 units on `(resolution um 10)`. The score charges 10 per bend. | `default_bend_cost = 9.9` on three stems: bit-identical output | S |
| **F5** | **`normalized_score` is `f32` and cannot resolve one via.** One via moves the score by `50 / (N × 5·10⁶) × 1000`; at N = 100 that is 10⁻⁴, below the f32 ulp near 1000 (6·10⁻⁵ at 999.99). The optimizer's pass-improvement test and the batch loop's best-board restore both compare these `f32`s. | arithmetic; see §3.3 | S |
| **F6** | **Via count on routed boards is mostly the fanout stage's.** With fanout on, every SMD pin gets an escape via before the router runs and the router connects to it for free. | rpi: fanout on 9 vias / off 7; the optimizer (F1) removes 7 of the 9 | M |
| **F7** | **Via candidates are free-space centroids.** A drill's location is the centre of gravity of a convex chunk of a drill page (`page.rs:80-84`) and the path locator uses that point verbatim (`locator.rs:162, :234`). Only the changed-area sweep's `opt_via_location` moves it afterwards. | not measured (needs code) | M |

The sweep script and every cell's SES/manifest/log are under the session scratchpad
(`scratchpad/sweep/`); §5 has the reproduction commands.

---

## 1. Where via count and trace length are decided today

There are four places, in pipeline order. Each one is a lever.

### 1.1 Fanout (`pipeline/fanout.rs`) — vias placed before anything is routed

`BatchFanout::fanout_board` gives every SMD pin an escape trace and a via, inside the
`fanout_min_escape_length..fanout_max_escape_length` window (500 µm .. 3 mm, `control.rs`). The
router then treats those vias as existing items: connecting to one costs the maze nothing, so the
cheapest path for a two-sided net is almost always "pad → fanout via → other layer". Unused fanout
vias are removed only at the end of a pass (`pass_runner.rs:164`, `StopConnectionOption::FanoutVia`),
and `for_routing_job` sets `remove_unconnected_vias = !fanout_enabled` (`batch_autorouter.rs:116-130`).

Measured on rpi-splitter (5 connections, 2 layers): fanout on → 9 vias, fanout off → 7, and the
optimizer, once awake, gets the fanout-on board to **2**. Seven of nine vias were never needed.

### 1.2 The maze (`autoroute/maze/`) — one connection at a time, best-first

`MazeSearchEngine` is A*: `expansion_value` (g) accumulates weighted Euclidean distance between door
midpoints, ripup costs, a bend penalty and via costs; `sorting_value` = g + `DestinationDistance`
(h), an admissible box-distance-plus-minimum-via-cost bound (`destination_distance.rs:120-300`).
The first destination door popped wins (`search.rs:300-420`). The pieces that set via count and
length:

| term | site | value |
|---|---|---|
| trace per unit | `expand.rs:802-812` `weighted_distance(h, v)` | `ExpansionCostFactor` per layer; default 1.0 / 1.0 |
| via | `expansion_engine.rs:118, :164` add `min_normal_via_cost`; `:428` adds `add_via_costs[from][to]` | `via_costs × max(max_via_radius, 1)`, ×0.1 on a pure-SMD net (`control.rs` `rebuild_via_info`); `add_via_costs` is **always 0** (Java `AutorouteControl.java:176` zeroes it too) |
| bend | `expand.rs:770-804` | `bend_costs[layer]`, clamped 0..9.9, added once per >5.7° direction change |
| ripup | `ripup_resolver.rs:64-168` | `ripup_costs(=100 × pass) × half_width ÷ detour × fanout_factor`, randomised from pass 4 |

Two consequences. **The via price is in board units scaled by via radius**, so it is a geometric
accident: the same `via_costs = 50` prices a via at 20 mm of trace with a 0.4 mm via and 12.5 mm
with a 0.25 mm one, while the score (`normalized.rs:7-58`, `benchmark/bench/metrics.py:29`) prices
every via at 50 mm. **The bend price is in board units unscaled**, so 9.9 is noise against a
10 000-unit millimetre, while the score charges 10 mm per bend.

### 1.3 Pull-tight (`board_ext/tightener/`) — local slack removal, already uncapped

Runs after every insertion, every shove and every pass (`docs/plan-9-prep/trace-optimization-map.md`
§1). It shortens the polyline the maze produced and calls `ViaOptimizer::opt_via_location`
(`via_optimizer.rs:13`) on every via in the changed area — the **only** caller
(`tightener/mod.rs:349`). It cannot re-path; it only tightens what the maze chose.

### 1.4 The optimizer (`pipeline/optimizer.rs`) — rip and re-route for quality

`run_batch_loop` (`:484`) visits items in a coordinate sweep (`ReadSortedRouteItems::next`, `:50`:
vias first, then traces not touching a via), and for each one `opt_route_item` (`:238`) deep-copies
the board, rips the item's connection, re-routes it with up to `max_autoroute_passes = 6` passes on a
second autorouter that alternates preferred-direction costs per pass, and keeps the result if
`ItemRouteResult::improved()` says so. This is the one stage whose purpose is fewer vias and shorter
traces, and F1, F3 and F5 are why it currently delivers almost none of that.

---

## 2. F1 in detail — the optimizer's own exit test disables it

`optimizer.rs:432-434`:

```rust
pub fn optimizer_near_perfect_exit(score_before_pass: f32, improvement_threshold: f32) -> bool {
    score_before_pass * (1.0 + improvement_threshold) >= 1000.0
}
```

called at the head of every pass with the default `optimization_improvement_threshold = 0.01`
(`default_settings.rs:79`). A board with zero incompletes and zero violations scores
`1000 − (length_mm + 50·vias + 10·bends) / (N × 5·10⁶) × 1000`; for j2-reference that is 999.9899.
`999.9899 × 1.01 = 1009.99 ≥ 1000`, so the stage exits before pass 1. It exits on **every** board
whose score is ≥ 990.1 — every board with no incompletes. Java has the same line
(`BatchOptimizer.java:181-183`, "Stop if potential improvement is less than threshold"); it is not in
`docs/java-quirks.md`, and the `#227` fix could not surface it because `#227`'s pins run on
incomplete boards.

The second use of the threshold, `pass_improvement < improvement_threshold` (`:598`), has the same
shape: the relative change of a number that is 1000 minus a rounding error. Both tests reason about
the *unrouted penalty's* scale while the stage only ever moves the *cost* term.

**Measured, threshold set to 0 (settings only, no code):**

| stem | cell | incompl. | vias | length mm | bends | benchmark cost¹ | s |
|---|---|---|---|---|---|---|---|
| rpi-splitter | base | 0 | 9 | 112.4 | 11 | 672 | 0.1 |
| rpi-splitter | `oit=0` | 0 | **2** | 111.8 | 12 | **332** | 1.3 |
| rpi-splitter | `oit=0`, via 150 | 0 | 4 | 110.1 | 10 | 410 | 6.2 |
| j2-reference | base | 0 | 18 | 379.9 | 38 | 1660 | 0.4 |
| j2-reference | `oit=0` | 0 | 18 | 370.4 | **52** | 1790 | 7.0 |
| j2-reference | `oit=0`, via 150 | 0 | **12** | 376.7 | 47 | **1447** | 25.1 |
| bm11 (`-mp 2`) | base | 13 | 57 | 1613.0 | 255 | 7013 | 93.9 |
| bm11 (`-mp 2`) | `oit=0` | 13 | 57 | 1613.0 | 255 | 7013 | 78.6 |

¹ `length_mm + 50·vias + 10·bends`, the score's cost term (`metrics.py:29-37`).

bm11 is unchanged because its score is 918, below 990.1: the stage already ran there and, bounded by
`PORT_OPTIMIZER_ROUTE_WORK_BUDGET` on an incomplete board, accepted nothing (`base` == `noopt`
byte for byte, 75 s spent). rpi and j2 are the routed-board case the exit test was hiding.

**Fix sketch (S).** Replace both threshold tests with tests on the cost term in `f64`:
`cost = length_mm × trace_cost + via_costs × vias + bend_penalty × bends`, from
`BoardStatistics` fields that already exist. Near-perfect exit: `cost == 0`. Pass exit:
`(cost_before − cost_after) / cost_before < threshold`. The setting keeps its name and default; its
meaning becomes "relative improvement of the routing cost", which is what the log line already
claims it is. Cost: run time. j2 went 0.4 s → 7 s (25 s with via cost 150); the stage's deep copy
per item is what W3 is about, and W3's "acceptance rate" input is now measurable on routed boards.

---

## 3. One currency: make the maze, the optimizer and the score agree

F2, F3, F4 and F5 are the same defect seen from four sites: three cost models that should be one.

### 3.1 F2 — via price

`control.rs` `rebuild_via_info`:

```rust
let mut via_cost_factor = self.max_via_radius;
via_cost_factor = java_max(via_cost_factor, 1.0);
if pure_smd_net { via_cost_factor *= 0.1; }
self.min_normal_via_cost = f64::from(via_costs) * via_cost_factor;
self.min_cheap_via_cost = 0.8 * self.min_normal_via_cost;
```

`via_costs` is the same field the score reads as "millimetres per via". The maze reads it as
"units per unit of via radius". Proposed: `min_normal_via_cost = via_costs × units_per_mm ×
trace_cost_per_unit`, so one via costs exactly `via_costs` millimetres of trace in both places, and
the pure-SMD `×0.1` becomes `router.scoring.smd_via_cost_factor` with an A/B'd default (that is
`#172` / W9, and the T18 verdict decides it). `units_per_mm` is `board.communication.resolution`
and `unit`, the same conversion `BoardStatistics::compute` uses (`statistics.rs:135-145`).

**Measured with the setting alone** (which also moves the score's own weight, so `cost` below is
recomputed at 50/via for comparability):

| stem | via_costs | incompl. | vias | length mm | cost @50/via | s |
|---|---|---|---|---|---|---|
| bm11 | 10 | 16 | 97 | 1536.5 | 9327 | 79 |
| bm11 | 50 | 13 | 57 | 1613.0 | 7013 | 94 |
| bm11 | 150 | 14 | **38** | 1619.2 | **6019** | 62 |
| bm11 | 500 | 13 | 37 | 1747.4 | 6287 | 73 |
| j2 | 50 / 150 / 500 | 0 | 18 | 379.9 | 1660 | 0.4 |
| rpi | 50 / 150 / 500 | 0 | 9 | 112.4 | 672 | 0.1 |

On the via-heavy board the price is a strong lever with a clear optimum region (150: −33 % vias for
+0.4 % length, at the cost of one more incomplete at `-mp 2`; 500 buys nothing more and adds 8 %
length). On the two small boards it does nothing because their vias are fanout vias (§1.1), which
the maze never pays for. The `+1 incomplete` is the warning: a dearer via is a harder search on a
congested board, so the price wants a **schedule** — full price while the board is completing,
relaxed on the ripup passes that are still failing (`ripup_pass_no` is already in `AutorouteControl`).
That is one axis of W21's cost-model portfolio and a natural W1 variant.

### 3.2 F3 — the optimizer's accept test

`item_route_result.rs:31-41` accepts on `(incomplete ↓) then (vias ↓) then (length ↓)`, strictly.
j2 with the stage awake: length −9.5 mm, bends +14 → the score says −140 + 9.5 = worse. The fix is
the same `f64` cost from §2: accept when `incomplete_after ≤ incomplete_before` **and**
`cost_after < cost_before`. This also lets the optimizer trade one via for up to 50 mm of trace,
which the lexicographic test forbids in both directions, and it makes `improvement_percentage`'s
integer-division bug (`via_count_after / via_count_before` on `i32`) irrelevant.

### 3.3 F5 — `f32` scores as decision inputs

`normalized_score` is `f32` by contract (survey §9.1 keeps it for the manifest). It is also read as
a decision input in three places:

| site | decision | resolution needed |
|---|---|---|
| `optimizer.rs:549` `optimizer_near_perfect_exit` | run the stage at all | one via (§2) |
| `optimizer.rs:598` `pass_improvement < threshold` | another pass | one via |
| `batch_loop.rs:176` `bh.max_score() > board_score_after`, `board_history.rs` | restore the best board | one via |

One via is `50 / (N × 5·10⁶) × 1000 = 10⁻² / N` score points; `f32` near 1000 has an ulp of
`6.1 × 10⁻⁵`. Beyond N ≈ 160 connections, two boards that differ by one via compare **equal**, and
`BoardHistory` then keeps whichever came first. Keep the `f32` for the manifest; give the decisions
an `f64` `BoardCost` (or compare `(incompletes, violations, cost_f64)` tuples) computed from the
same statistics. Small change, no goldens beyond the ones §2 already moves.

### 3.4 F4 — bend price

`bend_costs` is added in board units (`expand.rs:797`) and clamped to `0.0..=9.9`. Scale it as
§3.1 scales the via: `bend_cost_units = default_bend_cost × units_per_mm`, so `default_bend_cost`
means "millimetres of trace per bend", and set the default to the score's `bend_penalty = 10`.
Then the maze, the optimizer and the score charge the same 10 mm per bend. Do this **after** §3.1
and measure alone: the 5.7° detector at `expand.rs:791-797` fires on every 45° corner, and a real
bend price will pull traces toward fewer, longer segments — good for the score, and a visible
change in geometry on every 45° board.

---

## 4. Via placement and fanout — the M-sized levers

### 4.1 F6 — stop paying for fanout vias twice

The router never prices a fanout via, so it cannot decide *not* to use one. Options, cheapest first:

1. **Let the optimizer strip them** — it already does (`autoroute_passes_for_optimizing_item`
   re-routes with `remove_unconnected_vias = true`), and F1 is the only reason it has not been
   doing so. rpi 9 → 2 vias is this.
2. **Route without fanout first, fan out what failed.** `is_fanout_enabled` is a plain flag
   (`router_settings.rs:237`); a "fanout as fallback" mode runs `fanout_board` only for pins that
   are still unconnected after pass 1. j2 fanout-off is already better than fanout-on (16 vias
   / 371.5 mm vs 18 / 379.9 mm) and rpi likewise (7 / 100.1 vs 9 / 112.4), with no incompletes on
   either. Needs a measurement on the fanout-dependent boards (bm11, cnh) before it can be a default.
3. **Price the fanout via inside the maze.** When the target door is a fanout via with one
   contact, add `min_normal_via_cost` to the door's expansion value, so a two-sided net compares
   "reuse the escape" against "route on the pad's own layer" honestly. `MazeRipupResolver::
   calc_fanout_via_ripup_cost_factor` already knows how to recognise one (`ripup_resolver.rs:16`).

### 4.2 F7 — where a via lands

`DrillPage::get_drills` splits a page's free space into convex shapes and puts one `ExpansionDrill`
at each shape's centre of gravity (`page.rs:80-84`), or at an SMD pin centre when `attach_smd`.
`expand_to_drill` prices the drill by the *nearest point* of the shape to the incoming door
(`expansion_engine.rs`, `expand_to_drill`), but `FoundConnectionLocator` places the via at `drill.location`
(`locator.rs:162, :234`). So the maze chooses a drill for a cost it never pays: the trace bends to
the chunk's centroid, and only `opt_via_location` — which runs on the changed area, not the board,
and moves a via only along its two contact traces — pulls it back.

Levers:

1. **Place at the nearest point, not the centroid.** Carry the `nearest_point` the expansion
   already computed into the `MazeSearchElement` and have the locator use it. The
   `ForcedViaInserter::check` at insertion (`inserter.rs:996`) still validates clearance.
   S–M; changes every via's position on every board.
2. **One drill per chunk is coarse on open boards.** Large free chunks get one candidate; a page is
   `max_drill_page_width` wide. Subdividing large chunks (or a second candidate at the chunk's
   nearest point to the airline) raises via-placement resolution at the cost of queue size.
3. **A final via sweep.** Call `opt_via_location` over every unfixed via after the optimizer, not
   only over the changed area; and let it consider the *third* option its two-trace case does not —
   moving the via to the intersection of the two traces' airlines. Plan 9 Task 16 (M2) already
   hardens the routine; this is a second caller and one more candidate.

### 4.3 Route order and ripup — covered elsewhere

Items are routed shortest-airline-first (`airline.rs:51`); ripup cost is linear in the pass number
with no history term; the long nets routed last pay for everyone's detours. These are W7 (the sort),
W21 / E3 (history term, escalation schedule, pin-access reservation) and W22 / E1 (an obstacle-aware
heuristic) in `docs/plan-10-prep/routing-research-survey.md`, and this exploration adds only that
they should be measured *after* §2–§3, because a stage that finally rips and re-routes for quality
changes what the first-pass order is worth.

---

## 5. What to do first, and what it costs

| order | change | files | Δrefs | measured expectation |
|---|---|---|---|---|
| 1 | F1: cost-term exit tests | `optimizer.rs` | B, C on every routed stem | rpi vias 9→2, j2 with (2) vias 18→12 |
| 2 | F2: via price in millimetres, SMD factor a setting | `control.rs`, `scoring_settings.rs` | B, C, R everywhere | bm11 vias −33 % |
| 3 | F3 + F5: `f64` cost for accept / pass / restore | `item_route_result.rs`, `optimizer.rs`, `batch_loop.rs`, `board_history.rs` | B, C | removes j2's +14 bends |
| 4 | F4: bend price in millimetres | `control.rs`, `router_settings.rs` | B, C, R on 45° boards | unmeasured |
| 5 | F6: fanout as fallback, fanout-via pricing | `batch_loop.rs`, `fanout.rs`, `expand.rs` | B, C | j2 −2 vias, −8 mm |
| 6 | F7: via at nearest point; final via sweep | `page.rs`, `locator.rs`, `via_optimizer.rs`, `run.rs` | B, C, R | unmeasured |

1–4 are settings-and-arithmetic changes with one common test shape: a directed board on which the
old test says "stop" or "reject" and the new one says "go", plus the G2 A/B
(`scripts/quality-ab.sh`) and a `bench run` on the `d3-a` tier, reading `via_total`, `trace_length_mm`,
`bend_count` and `cpu_s`. The run-time cost of 1 is the price of an optimizer that works, and W3
already has the design for paying it in parallel.

## 6. What landed on `via-and-trace-length`

Steps 1–4 of §5 are implemented on the branch that carries this document; 5 and 6 are not.

| finding | change | where |
|---|---|---|
| F1 | the optimizer stage exits before a pass only when the routing cost is zero, and its pass-improvement test is the relative reduction of that cost (a completed connection counts as full improvement) | `optimizer.rs` `optimizer_nothing_to_improve`, `apply_pass_improvement` |
| F2 | **split by stage** (see §7): the routing stage keeps Java's price, `via_costs` times the largest via radius in board units; the optimizer's re-router prices a via at `via_costs` millimetres of trace. The pure-SMD tenth is `router.scoring.smd_via_cost_factor` (default `0.1`) under both | `control.rs` `ViaPricing`, `batch_autorouter.rs` `autoroute_item`, `scoring_settings.rs` |
| F3 | an optimizer item is accepted when the board's routing penalty falls — unrouted and violation penalties plus the cost term, in `f64` | `item_route_result.rs`, `normalized.rs` `routing_penalty` |
| F4 | `default_bend_cost` and a layer's `bend_cost` mean millimetres of trace per bend; the clamp ceiling is 100. The default stays `0.0` | `control.rs`, `router_settings.rs` |
| F5 | the board history ranks and restores by the `f64` penalty; the `f32` score stays the manifest's number | `board_history.rs`, `batch_loop.rs` |

**Measured after the change**, defaults only, same three stems and the same cost column as §2:

| stem | before (vias / mm / bends / cost) | after | via 25 | via 100 | bend 10 | bend 25 |
|---|---|---|---|---|---|---|
| rpi-splitter | 9 / 112.4 / 11 / 672 | **2 / 108.8 / 11 / 319** | 2 / 108.8 / 12 / 329 | 4 / 115.3 / 12 / 435 | 0 / 128.7 / 19 / 319 | 6 / 109.8 / 7 / 480 |
| j2-reference | 18 / 379.9 / 38 / 1660 | **18 / 375.2 / 33 / 1605** | same | 18 / 375.1 / 33 / 1605 | 18 / 380.3 / 24 / 1520 | 14 / 376.7 / 28 / 1357 |
| bm11 (`-mp 2`, 13 incompl.) | 57 / 1613 / 255 / 7013 | **38 / 1653 / 252 / 6073** | 47 / 1609 / 245 / 6409 | 36 / 1691 / 230 / 5791 | 41 / 1623 / 200 / 5673, **14 incompl.** | 56 / 1620 / 210 / 6520, **14 incompl.** |

A scaled bend cost of 10 mm buys 5–7 % of cost on j2 and bm11 but costs bm11 a completion at two passes,
which is why the default stays at zero: the knob is now meaningful, and the corpus A/B decides the
default. The run-time cost of F1 is what §2 predicted: j2 0.4 s → 4.4 s, bm11's optimizer stage
62 s against 16 s without it.

**One behaviour to know about.** The optimizer's per-item re-router runs `autoroute_pass` over
every open connection on the board, not only the ripped item's. Now that the stage is awake and
accepts on the penalty, a run with `router.enabled = false` and the optimizer on **routes the
board anyway** (rpi-splitter: 0 incompletes, 2 vias, against 5 incompletes with the optimizer off).
"Router off" therefore means "optimizer off" too when an unrouted output is wanted; the MCP
composition test says so explicitly.

**References.** Every routed reference moved: families B, C and R were re-cut from the port
(`FR_REGOLDEN=<label>` on `batch_parity`, `reference_parity` and `cli_e2e` writes them; the metas
are re-stamped by hand), and four JVM transcripts whose routes were priced by radius became port
goldens (`p10-autoroute-connection.txt`, `p10-opt-changed-area.txt`, `p10-board-history.txt`, and
the `p9t16-via-optimizer.txt` sections). The maze unit transcripts keep Java's price by setting it
explicitly in their probe controls, because the mechanics they pin do not depend on it.

### Reproduction

Release binary from this worktree, `FR_ROUTER_BUDGET=disabled` (the quality lane, ruling AI), one
cell per settings combination:

```sh
freerouting route fixtures/Issue026-J2_reference.dsn -o out.ses --max-passes 99 \
  --result-json out.json \
  --set router.fanout.enabled=true --set router.optimizer.enabled=true \
  --set router.optimizer.optimization_improvement_threshold=0 \
  --set router.scoring.via_costs=150
```

`normalized_score`, `board_statistics.vias.total_count`, `traces.total_length_mm` and
`bends.total_count` are read from `out.json`; the cost column is `length + 50·vias + 10·bends`.
bm11 is `Issue730-DAC2020_bm11.dsn` at `--max-passes 2`, rpi is `Issue143-rpi_splitter.dsn` at 8.
Incompletes here are the manifest's own count, not the referee's; the G2 harness must be the gate.

## 7. The corpus said no to the score's price in the maze

The first 605-board run of the branch (`via-and-trace-length-full-r2` on the workbench, against
`full-r1` at the branch's base, both at `-mp 10` with a 300 s job timeout) was read at 191 boards:

| metric over the 191 boards | r1 | r2 | change |
|---|---|---|---|
| vias | 4051 | 2859 | −29 % |
| wirelength mm | 196272 | 197039 | +0.4 % |
| unrouted | 354 | 467 | **+32 %** |
| violations | 100 | 96 | −4 |
| cpu s | 9623 | 17682 | +84 % |

Twenty boards lost completions. Reproduced locally on four of them, two mechanisms:

1. **A via priced at 50 mm is too dear for the search.** The maze hunts for 50 mm same-layer
   detours before it takes a layer change, so congested boards route slower and complete less.
   FT231X: 0 unrouted before, 4–7 after; priced at 20 mm (about Java's price for a 0.4 mm via)
   it comes back to 1. Two boards that finished in 73 s now hit the 300 s timeout.
2. **The optimizer stage is expensive on large routed boards.** Hardware_Playground routes in
   3 s; the stage then spends 215–280 s stripping four vias, and on the slower host it ran past
   the job timeout and the harness killed it with no output. The stage's search budget
   (`max_search_steps`) trips inside the pull-tight sweep, where `route_connection_full` still
   `expect`s success; the panic is caught one frame up and the pass ends, so the budget bounds
   the stage but not tidily.

**Decision: the via price is split by stage.** Routing prices a via as Java does, by padstack
radius, because completion is that stage's job and the score's price costs completions. The
optimizer's re-router prices it in the score's currency, because a candidate it rejects costs
nothing and most of the via gain measured on the fixtures came from that stage stripping fanout
vias. `ViaPricing` in `control.rs` carries the choice; `BatchAutorouter::autoroute_item` picks it
from `is_optimizer_autorouter`.

**The full run**, all 605 boards, base against branch at `8e7310b`: vias 15 223 → 11 234 (−26 %),
violations 497 → 448, wirelength +0.2 %, unrouted 2 766 → 3 271 (+18 %; 74 boards better, 72
worse, 4 with no output), cpu 34 432 s → 59 862 s. The 72 boards that lost completions are the
fixture set for the two fixes below.

**The overrun, found.** Three gaps let a run outlive its job deadline, and all three are fixed
on the branch, each pinned by a test: the optimizer polled the deadline only inside its per-item
re-router's pass loop, which never runs when the ripped item leaves no open connection behind;
a search that began just before the deadline ran to its own per-connection time limit, up to
100 s × 2^(pass−1) inside the re-router, because the stop check between expansions only read a
flag nobody had polled (`RouterStop::is_stopped_or_expired` polls it); and every optimizer item
paid two whole-board DRC passes for a count the statistics already carried. The re-router's
work-list sweep, which walked every item's connected set on the board up to six times per item,
is now limited to the nets that can have an open connection. With a real deadline
(`router.job_timeout`; the native CLI's `--timeout` flag is not it) the tail past the deadline is
under two seconds on the worst boards, and sk9822 routes fully inside a 60 s job.

**What the optimizer still costs.** On large routed boards the stage is expensive because every
item pays a whole-board deep copy, a rip, a re-route and two statistics passes: 28 s per item on
sk9822 before the work-list filter. That is W3's problem and the reason the corpus cpu roughly
doubled; the job deadline is the bound.

## 8. The optimizer's own cost, profiled and cut

Sampling the optimizer stage on Hardware_Playground and sk9822 (release binary, `sample` on the
main thread, attributed to the phases of `opt_route_item`) put about 85 % of the stage in board
statistics and 13 % in the maze search on both boards. The snapshot, undo, rip and combine steps
were under 3 %. Two things were behind the 85 %:

- **Statistics nobody read.** The re-router's pass runner computed a whole-board statistics pass
  at the start of every pass, another (with clearance violations) at the end, refreshed it every
  ten items, and counted incompletes twice more for the progress counters. The field they landed
  in was never read. An optimizer item runs up to six such passes, and the optimizer's own pass
  loop added three more unused computations. All of it is gone; the optimizer's re-router now
  reports no incomplete count in its pass counters (the optimizer reports it per item itself).
- **The fanout census inside the two counts the item needs.** `BoardStatistics` walked every SMD
  pin's connected set (`unconnected_set`, a full item scan per pin) to report escape counts the
  optimizer never looks at: 55 % of a statistics pass on Hardware_Playground, 93 % on sk9822.
  `BoardStatistics::for_routing_decisions` skips that block.

With those gone the two incomplete counts were 45 % of an item, all of it the DRC rebuilding
every net's item list, connected sets and Delaunay triangulation twice per item. The count is now
carried: the pass counts the board once, and each item adjusts the carried count for the nets its
re-route touched, which the undo journal names (`Board::journaled_nets`, the nets of every item
created, changed or removed in the journal's window). The adjustment recounts only those nets, on
the snapshot and on the re-routed board, with the same per-net item lists the full pass builds
(`DesignRulesChecker::incomplete_count_for_nets`), so the number is the full pass's number; a
debug assertion checks that on every item, and
`the_optimizers_carried_incomplete_counts_match_a_full_count` checks it on the routed rpi board.

| board | before | statistics cut | count carried |
|---|---|---|---|
| Hardware_Playground, optimizer done | 69.1 s | 20.3 s | 13.7 s |
| sk9822, vias at the 5 min job deadline | 82 | 50 | 48 |

Both boards route to the same wires, vias and length as before on Hardware_Playground; sk9822
gets further inside the same deadline. Every output the goldens pin is unchanged: the removed
work fed nothing, and the carried count equals the recomputed one.

What is left per item is the maze search (38 %, of which almost half is inserting the found
path: shove and pull-tight), the recount of the touched nets, tail removal (4 %) and the
connectivity queries under all of them (`normal_contacts`, a shape-tree query per trace end).
