# Trace-optimization map — freerouting-rs (branch `plan-9-post-parity`, tip `6c56932`)

Read-only survey. Question: *after a connection is routed, what post-processing / trace
optimization / shoving moves it toward a more direct path — and how much runs by default?*

All file:line cites are the committed tip of `plan-9-post-parity`. The Java clone is referenced
only for the parity mapping the port's own comments carry.

---

## Verdict up front

**Is there meaningful post-routing trace optimization running by DEFAULT today? — PARTIAL (yes for
pull-tight, no for the batch optimizer).**

- **Runs by default:** the **pull-tight / via-reposition / corner-smooth sweep** (`opt_changed_area`)
  fires after every routing pass (via `remove_tails`) and after every shove, and — because Plan 9
  Task 1 turned its wall-clock budget off (`RouterBudget::default().opt_changed_area_ms = 0`,
  `crates/fr-router/src/pipeline/stop.rs:571`, wired at `crates/freerouting/src/commands/route.rs:993`)
  — it now **runs to completion** rather than being abandoned mid-sweep. This genuinely straightens
  traces, repositions vias and smooths corners on the routed board.
- **Does NOT run by default (inert):** the **batch optimizer stage** (`BatchOptimizer::run_batch_loop`,
  `crates/fr-router/src/pipeline/optimizer.rs:1077`) — the stage that rips up and re-routes finished
  items purely to improve them — is quirk **#227**: it visits every item, routes **zero** passes per
  item, rejects every one, and returns the board unchanged, at one whole-board deep-copy per item.
  Task 9 (in flight) is what wakes it.

So: the local slack-removal optimizer is real and on; the global rip-up-and-reroute-for-quality
optimizer is present but dead. That is the "partial".

---

## 1. Pull-tight / trace tightening — RUNS BY DEFAULT

**What it does.** Straightens a routed trace by pulling slack out of it: the tightener repositions
the polyline's lines while re-checking DRC (`reposition_lines` / `pull_tight_polyline`), loops
`while (newResult != prevResult)` until the trace stops changing, and also **repositions vias** and
**smooths end corners** in the same sweep.

**Where it lives.**
- The three regime tighteners: `crates/fr-router/src/board_ext/tightener/{tightener_90.rs,
  tightener_45.rs, base.rs}`, dispatched by `TraceTightener` in
  `crates/fr-router/src/board_ext/tightener/mod.rs` (`pull_tight_opt` at `:380`,
  `pull_tight_polyline` at `:402`).
- The area driver: `TraceTightener::opt_changed_area`,
  `crates/fr-router/src/board_ext/tightener/mod.rs:208`. Its inner loop (`:288-370`) walks every
  item in the changed region: a **trace** is `pull_tight`ed (`:299`) then, if that fails, its end
  corners smoothed (`:316`); a **via** is offered to `ViaOptimizer::opt_via_location` (`:349`).
  Outer `while something_changed` (`:223`) repeats until the region is stable.
- The single-trace entry that runs right after each insertion:
  `RoutingBoardExt::insert_forced_trace_polyline` pull-tightens the trace it just inserted
  (`crates/fr-router/src/board_ext/routing_board_ext.rs:906`).

**When it runs.**
- **After every pass**, and after every rip: `BatchAutorouter::remove_tails`
  (`crates/fr-router/src/pipeline/batch_autorouter.rs:598`) calls `board.opt_changed_area(...)`
  (`:605`) after `remove_trace_tails`. `remove_tails` is called from the pass runner
  (`pass_runner.rs:403`, `:405`), the batch loop (`batch_loop.rs:465`, `:546`) and the optimizer's
  own autorouter (`batch_autorouter.rs:974`).
- **Per connection, at insertion time**: `insert_forced_trace_polyline` tightens the just-inserted
  trace (`routing_board_ext.rs:906`), and the changed-area sweep after a shove re-tightens whatever
  the shove disturbed.

**What controls its iterations.**
- **Accuracy**: `trace_pull_tight_accuracy` (default 500,
  `crates/fr-router/src/pipeline/batch_autorouter.rs:354`) — the `min_translate_dist`, i.e. how far
  a line/via must be movable to bother.
- **Budget (the important knob)**: `opt_changed_area_ms`. Java hard-codes
  `TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP = 1000` and inlines it (quirk **#234**), abandoning the
  pull-tight part-way through on wall clock — making the jar's own output machine-speed-dependent.
  **The port default is `0`** = Java's own "off" value (`TraceTightener` builds a `TimeLimit` only
  `if timeLimit > 0`), so **the sweep always runs to completion**. See
  `crates/fr-router/src/pipeline/stop.rs:507-571` and its long `// fixed: T1 (#234)` note. This is a
  strict quality gain the port already has and the jar does not.

---

## 2. The optimizer stage — PRESENT BUT INERT (quirk #227; Task 9 wakes it)

**What it does (intended).** `BatchOptimizer` is the batch post-pass that visits every unfixed item
in a stable sorted order (`ReadSortedRouteItems`), and for each item **rips its connections and
re-routes them** with a dedicated second autorouter (`autoroute_passes_for_optimizing_item`,
`batch_autorouter.rs:~875`) that strips fanout vias and alternates direction costs — keeping the
result only if the board score improves (`opt_route_item`, `optimizer.rs:537`).

**Where it lives.** `crates/fr-router/src/pipeline/optimizer.rs` — stage loop `run_batch_loop`
(`:1077`), pass `opt_route_pass`, item `opt_route_item` (`:537`). The per-item re-router is
`BatchAutorouter::autoroute_passes_for_optimizing_item`.

**When it runs.** As a **batch post-pass** after the routing stage, once per pipeline run.

**Why it is currently inert (the #227 mechanism — `docs/java-quirks.md:271`).** The routing stage
and the optimizer stage **share one stop flag** (`RouterStop`, three-state:
`NONE`/`AUTO_ROUTER_ONLY`/`ALL`), and **nothing ever lowers it**. Every ordinary exit from the pass
loop raises `AUTO_ROUTER_ONLY` (quirk #214's five arms: max-passes, "not able to improve", rank
limit, two stagnation windows). Then:
- `run_batch_loop`'s loop guard reads `is_stop_requested()` = **ALL** — so an `AUTO_ROUTER_ONLY`
  stop lets the stage **start** and the loop run (`optimizer.rs:1160`, the `// Java bug:` note; guard
  at `:1167`).
- But every `opt_route_item` calls `autoroute_passes_for_optimizing_item`, whose loop head reads
  `!is_stop_auto_router_requested()` = `!= NONE` — which is **true**, so it runs **zero** passes per
  item.
- Each item therefore rips its connections, `remove_tails` runs, the board measures strictly worse,
  `result.improved()` is false (`optimizer.rs:691`, `route_improved = !stop.is_stop_requested() &&
  result.improved()`), and the clone snapshot is restored (`:705`).

Net: the stage spends **one whole-board deep-copy per item** and returns the board it was given.
Measured, six `OPT-ITEM` lines all `improved=false`, `OPT-RESULT` shape == `ROUTED` shape.

**What Task 9's #227 fix changes** (`.superpowers/sdd/2026-09-03-plan-9-post-parity/task-9-brief.md`,
commit 2 of 8). Give the optimizer a **stage-scoped stop**: reset/replace the flag in the stage
prologue (or have `autoroute_passes_for_optimizing_item` read a stage flag, not the job's) so the
per-item re-router actually runs passes. Ordered **#214 → #227 → #202**: #214 first carries the
real exit reason out of the loop (`BatchLoopExit`) so a normal finish is `FINISHED` not `CANCELLED`;
#227 then makes the stage do work; #202 makes `--max-items` stop the router without also silencing
the optimizer. The brief calls #227 *"the single largest quality change in the catalogue — an entire
optimization stage begins working"*; it changes the geometry (B), routed length (R) and via count of
**every** board, raises `cpu_s` on every stem, and the binding acceptance test requires the
normalized score to **RISE on every stem** (`optimizer.rs` test
`an_auto_router_only_stop_still_runs_the_optimizer` asserts ≥1 `improved=true` and that the board
shape changes).

**Will it move traces post-routing? Yes** — once #227 lands, the optimizer will rip and re-route
finished items and keep the shorter/cheaper result. Until it lands, it moves nothing.

---

## 3. Shove / push-aside — DURING routing only; no post-hoc "shove toward better path"

Shoving (`crates/fr-router/src/board_ext/trace_shover.rs`, `forced_via_inserter.rs`,
`forced_pad_router.rs`) exists **to make room while inserting a new connection** — `insert_forced_*`
pushes existing traces/vias aside so the new trace/via can land, then pull-tightens the disturbed
changed area. There is **no mechanism that shoves an already-routed, already-satisfied trace toward a
more direct path** as a standalone improvement step. The only thing that re-touches settled traces
for quality is (a) the pull-tight sweep of §1 (local slack removal, not re-pathing) and (b) the batch
optimizer of §2 (which rips and re-routes, not shoves) — and (b) is inert.

---

## 4. "Optimize on completion" / rip-up-and-reroute-for-quality

The batch loop's rip-ups are **rip-up-for-completion**: `batch_loop.rs` rips items that are
blocking incomplete connections and re-routes to *complete* the board, guided by ripup costs and
stagnation counters. The only stage whose purpose is to re-route an **already-complete** net purely
to improve it (shorter, fewer vias/bends) is the **BatchOptimizer of §2 — which is inert (#227)**.
So today: **no** default reroute-for-quality. After Task 9: yes.

---

## 5. Via / corner / bend reduction

- **Via reposition**: `ViaOptimizer::opt_via_location` (`crates/fr-router/src/board_ext/via_optimizer.rs`),
  called **only** from inside `opt_changed_area` (`tightener/mod.rs:349`) — so it **runs by default**
  as part of every post-pass sweep, gated on `trace_costs.is_some()` (which `remove_tails` passes).
- **Corner smoothing / bend reduction**: `smoothen_end_corners_at_trace` in the same sweep
  (`tightener/mod.rs:316`), plus the tighteners' own line-repositioning which removes redundant
  bends. Runs by default.
- **Unconnected-via removal**: `remove_unconnected_vias` flag on the autorouter
  (`batch_autorouter.rs:369`) — a cleanup, not a directness pass; only the optimizer's re-router
  strips fanout vias, and that path is behind #227.
- There is **no separate standalone via-count-reduction post-pass** independent of the tightener
  sweep and the (inert) optimizer.

---

## Likely-cause ranking — why "finished traces take non-optimal routes"

1. **The batch optimizer is inert (#227).** The stage that would rip a completed net and re-route it
   shorter / with fewer vias does nothing today. This is the single biggest reason a finished route
   keeps whatever the maze first produced. — `optimizer.rs:1160`, quirk #227.
2. **Cost model prefers the maze's first legal path.** Best-first maze search commits to the first
   completing path under the current cost terms; pull-tight only removes *local* slack from that
   path — it cannot re-path around to a globally shorter/fewer-via route. Directness is therefore
   bounded by the maze's route choice plus local tightening. (Roadmap W1 attacks exactly this by
   searching wider; W6 questions the via-cost policy.)
3. **No post-hoc reroute-for-quality** (§4) — every rip today is for completion, not improvement.

(The pull-tight iteration cap is **not** a likely cause in the port: #234/T1 already removed the
wall-clock cap for default runs, so the sweep completes. It *would* be a cause on the stock jar.)

---

## If you wanted more direct traces, the levers are…

| Lever | Code site | Plan 9 / Plan 10 workstream |
|---|---|---|
| **Wake the batch optimizer** (rip + reroute finished items, keep if score rises) — the biggest single win | `optimizer.rs:1077` `run_batch_loop`; stage-scoped stop; `opt_route_item:537` | **Plan 9 Task 9 / #227** (in flight); then **W3** sizes the speculative per-item eval |
| **Search the maze wider** so the *first* committed path is better (variant portfolio / wider expansion) | maze engine + pass loop; `router.pass_variants` | **W1** (deterministic variant portfolio) |
| **Tune the cost model** — via cost policy, direction costs — so shorter/fewer-via paths win | trace-cost factors; the pure-SMD via ×0.1 relaxation (#172, T18) | **W6** (via count on our own terms); **W9** (revisit T18 policy defaults) |
| **Keep pull-tight running to completion** (already done) — don't reintroduce the wall clock | `stop.rs:571` `opt_changed_area_ms = 0`; `route.rs:993` | **Plan 9 Task 1 / #234** (landed) |
| **Enforce the real DRC rules during routing** so the cost function knows which candidates are legal, making a wider search pay off | routing cost + DRC | **W11** (port KiCad routing-relevant DRC), interacts with W1 |
| **Via reposition / corner smoothing** already on in the sweep; extend or make the via optimizer a standalone pass | `via_optimizer.rs`; `tightener/mod.rs:316,349` | **W6**; Plan 9 Task 16 (M2) hardens the via optimizer |

**One-line takeaway:** the port already straightens traces locally by default (pull-tight + via
reposition + corner smoothing, now uncapped); the *global* directness win — ripping and re-routing
finished nets for quality — is dead until Task 9's #227 lands, and is then sized/scaled by Plan 10's
W3 (optimizer), W1 (wider search) and W6 (via cost).
