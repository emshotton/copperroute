# Routing-research survey — state of the art, assessed for transfer into *this* router

**Status:** exploration deliverable, **uncommitted**. Read-only against the repo; no code changed.
Written to the standard set by `docs/plan-10-prep/unet-astar-assessment.md`: reconstruct the method,
**check the licence before anything else**, name the impedance mismatch against *our* architecture,
rank incorporation options, and recommend a bounded experiment with a **pre-fixed criterion**.

**Scope.** Seven survey areas, 15 ranked candidates, ending in a top-3 "worth a Plan 10 workstream"
list with drafted roadmap-idiom entries and explicit non-goals for the declined families. Numbering
continues the roadmap draft (W1–W19, N1–N16) **and** the Unet-Astar assessment's drafts (W20,
N17–N19), so this file adds **W21–W23** and **N20–N25**.

---

## 0. The three findings that reframe the whole survey

Three facts came out of the licence-and-architecture pass before any paper was read, and each one
changes what is worth pursuing. They are stated first because every table below depends on them.

### 0.1 **Our workspace is `GPL-3.0-or-later` — so KiCad's PNS router is licence-compatible**

`Cargo.toml:11` declares `license = "GPL-3.0-or-later"`. KiCad's push-and-shove router carries the
header *"This program is free software: you can redistribute it and/or modify it under the terms of
the GNU General Public License … either version 3 of the License, or (at your option) any later
version"* (`pcbnew/router/pns_router.h`, CERN / KiCad Developers, author Tomasz Włostowski).

**Consequence, stated plainly:** the brief's caution — *"GPL-and-adjacent-domain — ideas-level only
unless license allows"* — resolves to **the licence allows**. A GPL-3-or-later work may be
incorporated into a GPL-3-or-later work. A port of PNS logic into this tree is legally clean provided
the copyright notices and the GPL notice travel with it and the derivation is documented. This is the
single largest licence finding in the survey, and it moves KiCad PNS from "read only" to "portable".

*Caveat I will not soften:* licence-compatible is not the same as architecturally wise, and §4 below
argues the wise move is still mostly ideas-level. But the constraint that would have forced that
answer does not exist.

### 0.2 The Cadence/Simplex triangulated-space routing patents have **expired**

`US7073151B1` — *"Method and apparatus for identifying a path between a set of source states and a
set of target states in a triangulated space"*, Cadence Design Systems, priority 2002-06-04, granted
2006-07-04 — is **Expired (fee-related), 2023-08-05**. The sibling family (`US6978432`, `US6915499`,
`US7000209`, `US6889371`, `US6948144`, `US6895569`, `US7047512`) shares that 2002–2003 priority
window and therefore the same expiry horizon.

**Consequence:** the patent thicket that made any-angle triangulated/topological routing
untouchable for two decades is gone. That does not make the work small (§7), but it removes the
reason it was never on the table. *I am not a lawyer; a real clearance opinion would confirm the
whole family and check continuations before anyone writes code against it.*

### 0.3 Our ripup has **no history term at all**, and its escalation schedule is a straight line

Reconstructed from our own tree, not from a paper:

* The per-connection ripup cost is
  `ripup_cost = ctrl.ripup_costs × cost_factor ÷ detour × fanout_via_cost_factor`, clamped to
  `[1, i32::MAX/100]` (`crates/copper-router/src/autoroute/maze/ripup_resolver.rs:263-290`).
* `ctrl.ripup_costs` is set **once per connection** as
  `autoroute_control.ripup_costs = start_ripup_costs * ripup_pass_no`
  (`crates/copper-router/src/autoroute/maze/engine.rs:1900`) — a **linear, uniform** escalation over the
  batch pass number, identical for every item on the board.
* The only per-item modulation is `cost_factor` (the obstacle's half-width, or a via's contact
  geometry) and `detour` (the obstacle *connection*'s current detour ratio,
  `ripup_resolver.rs:266-274`) — both read fresh from the **current** board state.
* The `ripup_costs: BTreeMap<ItemId, i32>` threaded through
  `pass_runner.rs:313` → `batch_autorouter.rs:634` → `path/locator.rs:555, :631` is **allocated fresh
  per connection** (`pass_runner.rs:312-313`, inside the item loop) and dropped at the end of that
  connection. It is a backtrack scratch record, **not** accumulated state.
* The one stochastic element is `ripup_pass_no >= 4 && ripup_pass_no % 3 != 0 → detour *= 0.5 + r²`
  (`ripup_resolver.rs:276-281`), seeded from `ctrl.ripup_costs` (`maze/search.rs:126`).

**So: nothing in this router remembers that a particular trace has been ripped up five times
already.** Every pass re-derives the same decision from the same geometry with a slightly larger
scalar. That is precisely the gap negotiated congestion was invented to fill (§1), and it is also
precisely the gap the offline-RL paper measures as *"prior work only performs static costing of
search weights"* (§5). Two independent literatures point at the same hole in our code.

**And the heuristic is obstacle-blind.** `DestinationDistance::calculate` (`maze/destination_distance.rs`)
is a per-layer weighted box distance built from `traceCosts`, `minNormalViaCost` and the destination
bounding boxes — an admissible lower bound that **knows nothing about keepouts or occupied copper**.
On a keepout-pinched board it points straight at the pinch and keeps pointing there until the budget
runs out. That is "corridor blindness", named exactly (§3).

---

## 1. Candidate 1 — Negotiated congestion / PathFinder and its descendants

**What it is.** McMurchie & Ebeling, FPGA'95. Route every net with a shortest-path search over a
shared resource graph; *allow* overuse in early iterations; after each iteration raise the cost of
over-used resources and **accumulate a history term that is never decreased**. Cost of node *n* is
`C_n = (b_n + h_n) · p_n`, where `b_n` is the base cost, `h_n` the accumulated historical overuse and
`p_n` the present-sharing penalty. VPR's production form fixes the schedule: `first_iter_pres_fac`
= 0.0 (first pass may share freely), `initial_pres_fac` = 0.5, `pres_fac_mult` = 1.3 per iteration,
`acc_fac` = 1 (VTR command-line docs). Every iteration rips up **every** net and reroutes it.

**Evidence quality.** The strongest in the survey by longevity: *"by 2000 the PathFinder approach of
negotiated congestion was being used by virtually all FPGA place-and-route algorithms."* It is the
routing core of VPR/VTR, which is itself heavily benchmarked. But **it is FPGA evidence** — a fixed,
discrete, capacity-bounded resource graph — and the PCB adaptations are narrower: Ozdal & Wong's
escape-routing work, and *Multi-Terminal PCB Escape Routing for DMFBs Using Negotiated Congestion*
(UC eScholarship / ICCAD-class), which reports negotiated congestion beating "maze routing coupled
with rip-up and re-route" on multi-terminal escape routing. **No open real-board DRC evidence.**

**Licence.** The algorithm is published, unpatented in any form that survives (1995). VPR/VTR is MIT.
The one modern open PCB implementation, **OrthoRoute** (§13), is **MIT**. Nothing blocks us.

**Impedance mismatch.**

| PathFinder's world | ours | survives? |
|---|---|---|
| A fixed graph of routing resources with integer **capacities**; congestion = `occupancy > capacity` | Gridless rooms and doors created **lazily during the search**; there is no persistent node set to accumulate `h_n` on, and no capacity | **No, not as written** |
| Overuse is **tolerated** mid-run and legalised by iteration | Our board is legal at every instant; a conflict is resolved *now*, by shove or by ripup | **No** — we never have an illegal intermediate state to negotiate out of |
| Every net ripped up and rerouted every iteration | We rip selectively, driven by the maze finding a cheaper path *through* an obstacle | **No** |
| Resource = wire segment | Our stable, persistent, id-bearing objects are **items** (traces, vias) — not rooms | **The history term does**, re-hosted on items |

**The surviving idea, in one sentence.** *Give each ripped item a monotonically accumulating history
term, so an item that has been ripped repeatedly becomes progressively cheaper to rip (or more
expensive to keep), and the pass loop stops re-making the same mistake.*

That is a two-line change of shape at `ripup_resolver.rs:263` — multiply by (or add) a term read from
a `BTreeMap<ItemId, i32>` that **survives the connection**, updated once per pass. It is integer,
pure, and deterministic. It is *not* PathFinder; it is PathFinder's one durable insight ported onto
our own persistent identifiers.

**Incorporation shape.** Ideas-only, **size S–M**. New setting, default off (the W1 pattern), own
goldens when on.

**Verdict: PURSUE** — as part of experiment **E3** (§16.3).

---

## 2. Candidate 2 — TritonRoute / OpenROAD `drt`: windowed search-and-repair with marker-cost history

**What it is.** The open-source detailed router of OpenROAD (Kahng, Wang, Xu; TCAD 2020). After pin
access and track assignment it runs **seven outer iterations** of detailed routing. Each iteration
partitions the design into **7×7 GCell non-overlapping clips**, one *worker* per clip, and **shifts
the partition by −4 GCells on alternate iterations** so clip boundaries get optimised too. Each
worker owns three nested boxes — *standard* (what it may modify), *DRC* (what markers it counts),
*extended* (what it reads for checking) — copies that neighbourhood locally, and runs up to `maxIter`
= `(1,4,4,4,4,4,4)` rip-up-and-reroute rounds **entirely in local memory**, terminating early the
moment the clip is DRC-clean, then commits.

Inside a worker the cost has two parts. **Object cost** — a cheap, deliberately pessimistic
pre-DRC penalty on edges near existing shapes; it has **no history** and is added/subtracted as
objects appear and vanish. **Marker cost** — applied around actual DRC markers after each call to the
DRC engine; it **has history within the worker**: *"a marker cost is added to an edge and decayed
over time (currIter), but is never subtracted due to the removal of a specific marker."* The A* edge
cost is `length + 8×length if object cost + 64×length if marker cost`, with an extra penalty on a
DRC-lookup-table match; `h` is Manhattan distance with a 4× layer-change weight. Nets in a worker are
**ordered by distance to the nearest marker**, and — the detail I would steal first — **the pin
access of every not-yet-rerouted net is reserved** (its preferred up-via's object cost is added as if
used) so that net *i* cannot block net *j>i*'s only way out of its pad.

**Evidence quality — the best in the survey.** ISPD18 and ISPD19 contest benchmarks, industrial
libraries, real spacing/EOL/cut rules, and the metric is **DRC violations counted by a real checker**:
0 DRVs on ISPD18 and 0 on all but one ISPD19 case (as restated in arXiv 2512.03594). This is the only
candidate whose acceptance bar is the same *kind* of bar as ours.

**Licence: BSD-3-Clause** (`OpenROAD/LICENSE`, "Copyright (c) 2018-2025, The OpenROAD Authors").
Permissive, and permissive-into-GPL-3 is fine. **We may read it, port it, and translate it**, with
attribution.

**Impedance mismatch.**

| TritonRoute | ours | survives? |
|---|---|---|
| 3-D grid graph on **tracks**, preferred routing direction per layer | Gridless rooms/doors, any-angle capable, no track abstraction | **No** — the grid is load-bearing for both cost types |
| Object cost = pessimistic spacing shadow on grid **edges** | We have no edges to decorate; clearance is exact and shape-based | **No** — and we do not need it, our clearance model is already exact |
| Marker cost = penalty near a **DRC marker from a real checker**, with in-worker history | Our router sees **two** rule types (`copper-drc` emits `clearance` + `hole_clearance`; roadmap **W11**) and never re-checks mid-route | **The mechanism does — and it is W11's missing consumer** |
| 7×7 GCell clip decomposition, shifted alternately | We have no spatial decomposition of any kind; every pass is whole-board | **Yes, and this is the transferable structure** |
| Pin-access reservation for unrouted nets | Our fanout stage exists but nothing reserves escape room for later nets | **Yes, cleanly** |
| C++, threaded workers | `#![forbid(unsafe_code)]`, ruling AM, byte-identical output | Workers are **sequential-safe**: clips are disjoint by construction, so a deterministic serial sweep in a fixed clip order is byte-identical regardless |

**The surviving idea, in one sentence.** *After the batch loop stops, sweep the board in a fixed
order of bounded windows and, inside each window, run a small local rip-up-and-reroute driven by a
history-carrying penalty around the actual violations, terminating the window as soon as it is clean.*

**Incorporation shape.** **Port-shaped at the level of structure, ideas-level at the level of code**
(their costs are grid-edge decorations we cannot host). **Size L**, and it is genuinely
**PENDING on W11 phase 2** — a marker cost needs markers, and today we can only mark two rule types.

**Verdict: PURSUE** — experiment **E2** (§16.2); the highest-value *structural* import in the survey.

---

## 3. Candidate 3 — Portal-based true-distance heuristics (PTDH / PBS) and the funnel algorithm

**What it is.** Goldenberg, Felner, Sturtevant, Schaeffer (SoCS). Partition a search space into
**regions**; the states with neighbours in two or more regions are **portals**. Precompute the true
distance between **all pairs of portals** and store it. At runtime,

```
h(a, b) = min over portals pA of A, pB of B  [ hA(a, pA) + d(pA, pB) + hB(pB, b) ]
```

and *"if the local heuristics are admissible, then the portal heuristic is admissible."* Sibling
constructions: differential heuristics (`h(a,b) = max_p |d(a,p) − d(p,b)|` over pivots), and
abstraction-based true-distance heuristics generally. Adjacent and complementary: **Anya** (Harabor &
Grastien, ICAPS'13) — provably **optimal any-angle** pathfinding by searching over *intervals* rather
than points, entirely online, no preprocessing — and the **funnel / string-pulling** algorithm, which
computes the exact Euclidean shortest path through a sequence of L convex portals in **O(L)**.

**Evidence quality.** Not routing evidence — game/robotics pathfinding, benchmarked on grid maps. But
these are **theorems, not benchmarks**: admissibility is proved, not measured, and that is exactly
what makes them safe here. Open implementations of Anya and of the funnel algorithm are plentiful and
variously licensed; **we would implement from the published construction, not copy.**

**Why this is the closest architectural match in the entire survey.** *Our rooms are regions. Our
doors are portals.* `crates/copper-router/src/autoroute/maze/` is a portal graph search that has never
been treated as one.

**Impedance mismatch — and it is real.**

| PTDH assumes | we have | survives? |
|---|---|---|
| A **static** graph, partitioned once, portals enumerable up front | Rooms and doors are built **lazily during the search** (`complete_neighbour_rooms` even restarts its walk because completion adds doors to the room being walked — roadmap N9, `maze/engine.rs:1063`) | **No — not on the live room graph.** The abstraction must be a *separate*, cheap, per-pass structure |
| All-pairs portal distances stored | |P|² memory on a dense board is not free | Manageable: the abstraction is coarse by design |
| Single-layer 2-D grid | Multi-layer with via costs | Fine — the coarse graph gets layers and a via edge weighted by `min_normal_via_cost` |
| Distances are *the* metric | Our cost is `ExpansionCostFactor` per layer/direction plus bend and via costs | Fine, **provided every coarse edge weight is a lower bound** of any real path crossing it |

**The construction that is actually admissible.** Build, once per pass, a **coarse per-layer
occupancy abstraction** in which a coarse cell is passable if *any* part of it is free of fixed
obstacles, give each coarse edge the *minimum* trace cost of that layer times the geometric edge
length, give each layer-change edge `min_normal_via_cost`, then run one **backward integer Dijkstra**
from the destination set. The resulting field is an **admissible, consistent** lower bound on the
true cost-to-go, because every real path is over-charged relative to this optimistic abstraction.
Substitute it for — or take the `max` with — `DestinationDistance::calculate`'s box distance at the
one hook site (`maze/expand.rs:802-817`).

**The surviving idea, in one sentence.** *Replace an obstacle-blind admissible heuristic with an
obstacle-aware admissible heuristic, so the search stops pouring its budget into a pinched corridor
that is actually blocked and walks the legal detour on its first attempt.*

**Why this strictly dominates the Unet-Astar corridor prior (W20).** W20's prior is **inadmissible by
construction** — that is why the assessment had to fence it into `sorting_value` and forbid it from
`expansion_value` (`unet-astar-assessment.md` §3a). An admissible obstacle-aware `h` needs **no
fence**: it may enter the real cost model, it cannot return a worse-than-optimal path, and its whole
benefit is that it aims at the *legal* corridor rather than a *learned guess* at one. Same lever,
sound instead of heuristic, and it needs neither a neural network nor a training corpus.

**And the funnel algorithm is the second half.** Once the search returns a **door sequence**, the
exact shortest any-angle path through that sequence is O(L) by string-pulling. Our `pull-tight`
optimiser is an iterative geometric relaxation reaching for the same answer; a funnel pass over the
door sequence computes it directly. Worth measuring as a *comparison*, not as a replacement.

**Incorporation shape.** Ideas-only, implemented from the published construction. **Size M** for the
coarse-abstraction heuristic; **S** for a funnel-vs-pull-tight comparison harness.

**Verdict: PURSUE** — experiment **E1** (§16.1). **This is my highest-conviction recommendation.**

---

## 4. Candidate 4 — KiCad PNS (push-and-shove), now known to be licence-compatible

**What it is.** `pcbnew/router/` — ~40 files, GPL-3-or-later, CERN-originated. Architecture:
`NODE` (a copy-on-write board view supporting speculative branches), `LINE`/`JOINT`/`ITEM` topology,
`WALKAROUND` (route around an obstacle), `SHOVE` (push the obstacle aside), `DRAGGER` /
`MULTI_DRAGGER` / `COMPONENT_DRAGGER`, `OPTIMIZER`, `TOPOLOGY`, `DIFF_PAIR*`, `MEANDER*`.

Two design details are worth naming because they are exactly the shape of decision our router makes
badly:

* **`WALKAROUND` runs three policies concurrently** — `WP_CW`, `WP_CCW`, `WP_SHORTEST`
  (`pns_walkaround.h`, `MaxWalkPolicies = 3`) — with an iteration limit, a length-limit switch and a
  `m_lengthExpansionFactor = 10.0` cap. It does not guess which way round an obstacle to go; it tries
  both and keeps the better one.
* **`SHOVE` carries an explicit policy set** — `SHP_SHOVE`, `SHP_WALK_FORWARD`, `SHP_WALK_BACK`,
  `SHP_IGNORE`, `SHP_DONT_OPTIMIZE`, `SHP_DONT_LOCK_ENDPOINTS`, `SHP_REVERSED` — with status codes
  including `SH_TRY_WALK`, i.e. *"shoving failed here; fall back to walking around"*
  (`pns_shove.h`). Shove and walkaround are **alternatives selected per obstacle**, not a fixed order.
* `OPTIMIZER` exposes graded **effort levels** (`MERGE_SEGMENTS`, `MERGE_OBTUSE`, `SMART_PADS`,
  `FANOUT_CLEANUP`, `KEEP_TOPOLOGY`, `LIMIT_CORNER_COUNT`, `REQUIRE_OBTUSE_ANGLES`, …) and a
  `COST_ESTIMATOR` over **length + corner cost** with an explicit `IsBetter(other, lengthTolerance,
  cornerTolerance)` — a tolerance-based accept test, not a strict improvement test.

**Evidence quality.** No papers, no benchmark tables — but the strongest *field* evidence available:
it is the interactive router hundreds of thousands of people use daily on real boards judged by
KiCad's own DRC, which is **our referee**.

**Licence: GPL-3.0-or-later — compatible with our `GPL-3.0-or-later` workspace (§0.1).** A port is
legally clean with notices preserved.

**Impedance mismatch.**

| PNS | ours | survives? |
|---|---|---|
| **Interactive**: one line at a time, a human supplying intent, latency budget in milliseconds | Batch, whole-board, unattended | Structure does not transfer; **policies do** |
| `NODE` copy-on-write speculative branches | We `deep_copy` whole boards in the optimiser (`pipeline/optimizer.rs`, W3) | An idea worth noting; a rewrite-sized change |
| C++, `boost`, KiCad's `SHAPE_*` geometry | `copper-geometry`, `#![forbid(unsafe_code)]`, no new deps | A line-by-line port is not on |
| Walkaround's CW/CCW/shortest triple | Our maze picks one path and commits | **Yes — and this is a deterministic multi-policy lever** |
| Shove ↔ walk fallback per obstacle | Our shover has its own recursion depths (`max_shove_trace_recursion_depth`, `maze/control.rs:141`) but no "give up and go around instead" alternative arm | **Yes** |

**The surviving idea, in one sentence.** *When an obstacle must be dealt with, evaluate more than
one resolution policy — shove, walk clockwise, walk anticlockwise — and keep the cheapest, instead of
committing to the first one that succeeds.*

**Incorporation shape.** Ideas-level first (**size M**), with a **narrow, attributed port** available
as a fallback if a specific geometric primitive proves fiddly — which the licence now permits.

**Verdict: PURSUE at ideas level** (folded into the W23 draft, §17.3). **DECLINE the wholesale port**
(N20): interactive-router structure around a human's intent is not a batch autorouter, and
maintaining a second geometry stack forever is the Unet-Astar §3c mistake in a nicer licence.

---

## 5. Candidate 5 — Cost-weight *scheduling* (arXiv 2512.03594, offline RL — with the RL removed)

**What it is.** *Accelerating Detailed Routing Convergence through Offline Reinforcement Learning*
(Dec 2025). Trains a conservative Q-learning model to pick, per iteration, four routing cost weights
— `drcCost`, `markerCost`, `fixedShapeCost`, `markerDecay` — in OpenROAD's detailed router. 5 782
routing runs over 17 designs (10 OpenROAD Design Suite + 7 ISPD18) generate the offline dataset via
perturbation and Sobol sampling; evaluation is on **10 unseen ISPD19 designs**.

**Result: 5 % fewer iterations on average (up to 31 %), 1.56× average runtime (up to 3.01×), DRV count
maintained or improved in all cases.**

**The finding that matters to us is not the RL.** It is the premise, which the authors state as an
observation about the *baselines*: *"prior detailed routers statically schedule the cost weights used
in their routing algorithms, meaning they do not change in response to the design or technology … In
the case of Dr. CU, the violation costs are static during the entire algorithm. For OpenROAD, the
costs are adjusted based solely on the current ripup iteration number."* And their headline
qualitative result: *"not only are the current weight schedules suboptimal, but the conventional
wisdom for weight selection is dramatically different from the weights that our RL model selects."*

**Ours is the weaker of the two baselines they criticise.** `start_ripup_costs * ripup_pass_no`
(`maze/engine.rs:1900`) is "adjusted solely by the iteration number", and the constant was inherited,
not measured. The paper is independent evidence that there is real headroom in that line, obtainable
**without any model at all** — by searching the schedule.

**Evidence quality.** Good and honest: real benchmarks, a real DRC bar, unseen test set, and the
paper names the cases where it *loses* (`ispd19_test9` runtime up). Weaknesses: **code and weights
release is not stated**, the benefit is convergence speed rather than final quality, and the training
set is 17 designs.

**Licence.** arXiv preprint; no code located, so **ideas-only regardless**.

**Impedance mismatch.** Their weights decorate a grid; ours is a single scalar multiplying a
geometric factor. But "the schedule of the escalation across passes is a tunable with headroom" is
architecture-independent. **In-process ML inference remains barred** by non-goal **N18** (new
workspace dependency; float-backend non-determinism).

**The surviving idea, in one sentence.** *Our ripup-cost escalation schedule is a linear function
nobody has ever measured; search it deterministically, and run several schedules as a portfolio.*

**Incorporation shape.** Ideas-only, **size S**, and it **rides W1's machinery for free**: W1 already
proposes N complete passes on independent board clones selected by `argmax(normalized_score,
−variant_index)`. A variant that differs by **cost schedule** rather than by work-list permutation is
the same fork/join with a different pure function of `(pass, variant)`.

**Verdict: PURSUE** — folded into **E3** (§16.3) and the W21 draft. Cheapest quality lever in the survey.

---

## 6. Candidate 6 — Pin-access reservation for not-yet-routed nets (TritonRoute, isolated)

**What it is.** Broken out of §2 because it is separable and much smaller. Inside a repair worker,
before routing net *i*, TritonRoute **adds the object cost of every unrouted net's preferred pin
access (an up via) as if it were already used**, then subtracts it for the net currently being
routed. Explicit rationale: *"we would like to avoid the i-th net blocking the pin access of the
j-th (j > i) net."*

**Evidence quality.** Inherits §2's — it is a component of a router that reaches 0 DRVs on ISPD18.
No isolated ablation is published, which I note as a genuine weakness.

**Licence.** BSD-3 (OpenROAD). Portable.

**Impedance mismatch.** Small. We already compute fanout escapes per pin (`pipeline/fanout.rs`, 1 776
lines) and already have a per-connection cost hook. What we lack is a *soft reservation*: an additive
cost, not a keepout, on the escape geometry of pins whose net is still unrouted in this pass, cleared
for the net being routed. It is a cost-field change with no geometry change, so shove and DRC are
untouched.

**Why it targets our measured weakness.** The **15 hard losses on marginal-connectivity boards** are
by definition boards where the last few nets have nowhere to go. "The last few nets have nowhere to
go" and "earlier nets sealed their pads" are the same sentence.

**The surviving idea, in one sentence.** *Charge the router for consuming the escape route of a pin
whose net has not been routed yet.*

**Incorporation shape.** Ideas-only. **Size S–M.** Setting, default off, own goldens.

**Verdict: PURSUE** — the cheapest single shot at the hard-loss class; folded into **E3**.

---

## 7. Candidate 7 — Topological / rubber-band routing (SURF; Dai, Dayan, Kong, Sato; and the triangulated-space line)

**What it is.** SURF (UC Santa Cruz, ~1991) represents interconnect as a **rubber-band sketch (RBS)**
— a canonical representation of planar topological routing in which wires are elastic bands anchored
at obstacles — routes topologically first, then *pulls the bands taut* into geometry. *"The first
router ever reported that uses a rubber-band sketch to represent the interconnect"*, with a
mathematical formulation used to **prove the correctness of the shortest-path algorithm**, and
explicit support for *"rectilinear, octilinear and any-angle wiring rules"*. Dai/Kong/Sato's
companion work gives a **routability test on the sketch** — decide whether a topology can be
geometrically realised *before* embedding it. The commercial descendants are Altium's **Situs**
(triangulate the space between obstacles, weave through obstacle pairs, then hand the topological
path to a push-and-shove engine to realise it) and the Cadence/Simplex triangulated-space patent
family (§0.2, **expired**).

**Evidence quality.** Historically decisive but **old** (1991–1992), MCM/VLSI rather than PCB, and
with no reproducible modern benchmark. The commercial descendants have shipped for twenty years,
which is strong but unquotable evidence. **No open implementation found.**

**Licence / patents.** Papers are publications. The blocking patent family has **expired** (§0.2).
Situs is proprietary and closed.

**Impedance mismatch — the largest in the survey, and it cuts both ways.**

| SURF | ours | survives? |
|---|---|---|
| Route **topology first**, geometry later; the sketch is the primary artefact | We produce geometry immediately; there is no topological intermediate anywhere in the tree | **No** — this is a different pipeline, not a different function |
| Rubber-band sketch data structure, taut-pulling as the embedding step | `pull-tight` exists but operates on already-embedded geometry | Partial: pull-tight is the *ending* of their pipeline without its beginning |
| Routability test on a topology **before** embedding | We discover unroutability by exhausting a budget | **This is the single most valuable idea here** — and it is inseparable from the sketch |
| Planar / per-layer topological reasoning | Multi-layer with vias and shove | Their layer handling is the weak part of the original work |

**The surviving idea, in one sentence.** *Decide the topology — which side of each obstacle each net
passes — as a first-class, cheap, testable decision, and only then commit geometry.*

**Incorporation shape.** **XL — it is a second router**, exactly the shape the Unet-Astar assessment
declined as option (c). A rubber-band front end would need its own data structure, its own routability
test, its own embedder, its own goldens, and would have to coexist with the room/door maze forever.

**Verdict: WATCH, and record the patent clearance.** Genuinely transformative for corridor blindness
in principle; not affordable as a Plan 10 workstream, and premature before **E1** tells us whether an
admissible obstacle-aware heuristic already recovers most of that benefit at 5 % of the cost. If E1
succeeds and a residue of *topological* (not geometric) failures remains, this is where to look next.
**Record as N21** — declined *for Plan 10*, not declined forever, with §0.2's clearance attached so
nobody re-litigates the patent question.

---

## 8. Candidate 8 — Net ordering by learned policy (IJCAI'25 transformer RL; MCTS+DRL; XRoute)

**What it is.** Zhou, Zhuo, Zhou & Wen, *Transformer-based Reinforcement Learning for Net Ordering in
Detailed Routing*, IJCAI-25 (doi 10.24963/ijcai.2025/1055): a transformer policy that learns net
orderings from failure/success experience, reporting *"reduce the number of design rule violations
and routing cost with comparable wirelength and via count, with comparison to state-of-the-art
approaches."* Neighbours: **XRoute** (arXiv 2305.13823, an RL environment for net selection built on
a detailed router), MCTS + deep RL circuit routing (*"33.3 % higher success rate than traditional
A*-based approach"*), and attention-based REINFORCE orderings.

**The premise is exactly our R1 experience** and the authors state it plainly: *"their performances
are sensitive to the order of nets to be routed, especially for those sequential routers with
ripup-and-reroute scheme … net ordering strategies mainly rely on experts' knowledge to design
heuristics."*

**Evidence quality — weak where it counts.** The IJCAI abstract states improvements but the
proceedings page carries **no baseline router named, no benchmark named, and no numbers**. The MCTS
result's *"33.3 % higher success rate"* is against a plain A\*, not a competitive router. No code,
weights or licence located for any of them. This is the Unet-Astar pattern with better provenance:
claim stated, evidence not inspectable.

**Impedance mismatch.** Fatal on our constraints, not on our architecture. In-process inference is
barred (**N18**: new workspace dependency; float-backend variance defeats byte-identity). An offline
per-board committed ordering artefact is conceivable but would be per-board, not per-router, and
therefore worthless as a product feature.

**The surviving idea, in one sentence.** *Ordering is a first-class lever with real headroom — so
search it, deterministically and without a model.*

**Which is W1, already.** W1's variant portfolio permutes the work list as a pure function of
`(pass, variant)` and keeps the winner. It captures the entire *actionable* content of this
literature at zero determinism risk and zero new dependencies. **The literature's contribution here
is corroboration of W1's premise, and that is worth having.**

**Verdict: DECLINE the learned policies (N22); cite them as evidence for W1.** Watch the IJCAI paper
for a code release that would make the claim checkable.

---

## 9. Candidate 9 — SAT / SMT / ILP exact local repair

**What it is.** Encode a small routing window exactly and solve it. Threads found: SMT for FPGA
detailed routing constraints (Z3); SAT-based switchbox/channel formulations solving *all* channels
simultaneously; the industrial pattern of selecting the connections in the vicinity of a failed
connection and handing that neighbourhood to a SAT solver; Kahng's ILP-based intra-layer parallel /
inter-layer sequential detailed routing (cited as [18] in the TritonRoute paper); and ILP for PCB
meander shifting (arXiv 1705.04984).

**Why it is attractive on paper.** It attacks the **last-few-incompletes** class head-on and with a
completeness guarantee: if the window is unroutable, the solver *proves* it, which is information our
router can never produce today (we only ever exhaust a budget).

**Evidence quality.** Diffuse. Real results exist inside grid/track formulations; **nothing found
that solves a gridless any-angle window with real PCB clearance rules.**

**Licence.** Mixed and mostly irrelevant, because the blocker is upstream of licensing.

**Impedance mismatch — decisive.**

1. **The encoding does not exist.** SAT/ILP needs a **finite** decision set. Our world is gridless and
   any-angle with a real clearance matrix; producing a sound finite encoding means inventing a
   discretisation whose solutions are guaranteed legal in the continuous world. That is a research
   contribution in itself, not an import.
2. **A solver is a new workspace dependency**, barred outside ruling BK. Writing our own is XL.
3. **Determinism.** Modern SAT/ILP solvers are deterministic given fixed seeds and single-threaded
   configuration, but "deterministic across platforms and library versions" is a much stronger claim
   and floating-point LP relaxations do not honour it. A pure-integer CP/SAT core could; an LP-based
   ILP could not.

**The surviving idea, in one sentence.** *Bound the repair to a small window and terminate it on a
proof of local cleanliness — which is exactly what TritonRoute's worker does (§2) with an A\* instead
of a solver, and therefore with none of the three blockers.*

**Verdict: DECLINE (N23), with the door left open.** §2 buys the same structural benefit at a
fraction of the risk. Revisit only if E2's windowed repair lands, works, and leaves a residue of
windows that a complete method would provably close.

---

## 10. Candidate 10 — Global routing guides (cuGR / CUGR2-EDGE) and the missing global stage

**What it is.** CUGR (CUHK, DAC'20) and **CUGR 2.0 / EDGE** (Liu & Young, DAC'23) are
**detailed-routability-driven global routers**: they produce per-net *routing guides* — coarse
corridors — whose quality is *"solely determined by the final detailed routing results"*. The ISPD18/19
contests institutionalise the split: *"the global routing guide is provided associated to each
benchmark, and detailed routers are required to honor the routing guides as much as possible
meanwhile minimize DRC violations."*

**The structural observation about us is the point.** **Our router has no global-routing stage at
all.** Every connection goes straight from ratsnest to a full-detail gridless maze search. The entire
VLSI flow says that is the wrong shape at scale: decide corridors cheaply and globally with congestion
in view, then route in detail inside them. This is *also* what the Unet-Astar paper was groping at
with a learned corridor (**W20**) — the VLSI answer is that you compute the corridor with a
congestion-aware global router, not predict it.

**Evidence quality.** Strong within VLSI; contest-validated. **Licence:** cu-gr / dr-cu ship the
**CU-SD licence, adapted from BSD** — permissive, no non-commercial clause in the text I read.

**Impedance mismatch.** Their guides are GCell corridors on a track grid; ours would have to be
regions in a gridless plane. And a guide is only worth having if the detailed router is *constrained*
by it — which changes every route on the board. **Size L–XL**, and it substantially overlaps W20.

**Verdict: WATCH.** Note it as the principled endpoint of the W20 corridor idea, and re-open it only
if **E1** shows that an admissible obstacle-aware heuristic (§3) is *not* enough — because E1 buys a
large share of the same benefit at size M without restructuring the pipeline.

---

## 11. Candidate 11 — Dr. CU (CUHK detailed router)

**What it is.** The other leading open detailed router; ISPD18/19-class; A\*/Dijkstra per net with
violation costs added to grid nodes and rip-up-and-reroute. arXiv 2512.03594's own characterisation:
*"in the case of Dr. CU, the violation costs are static during the entire algorithm."*

**Evidence quality:** contest-grade. **Licence:** CU-SD (BSD-derived), permissive.
**Impedance mismatch:** same grid-graph dependence as §2, without §2's clip decomposition or history
term — i.e. it is §2 minus the two ideas we want.

**Verdict: WATCH.** No idea here that §2 does not supply in a better form. Useful as a second opinion
if we ever want to compare two independent implementations of a violation-cost scheme.

---

## 12. Candidate 12 — PCBWorld (arXiv 2607.05915), a KiCad-grounded RL/LLM benchmark

**What it is.** An open, engine-grounded PCB routing **environment** built on KiCad: agents route
through KiCad's native operations guided by its **DRC feedback**; three datasets in native
`.kicad_pcb` (two synthetic generators + **679 real open-source boards**); **eight engine-checked
metrics** scoring any completed board regardless of routing method. Headline result: an RL policy
trained only on synthetic boards **transferred zero-shot to real boards, "approaching rule-based
routers"** — i.e. after all that, it approaches but does not beat a conventional router.

**Evidence quality.** Honest and unusually well-shaped: real boards, real DRC, and a result that does
not oversell. **Licence:** CC0 on the paper; the code/data licence needs confirming before use.

**Impedance mismatch.** It contributes **no routing algorithm**. As a *benchmark*, it is largely
subsumed by what we already own: **1 304 boards** with a `kicad-cli pcb drc` referee
(`benchmark/bench/referee/kicad.py`, 23 routing-relevant violation types) and human-routed via/length
references (roadmap §1.2). Their 679 real boards are a **subset-shaped** resource, not a superset.

**Verdict: DECLINE as a workstream (N24); WATCH as a corpus source.** If Plan 10 ever wants boards our
corpus lacks, check for non-overlap first. Its *actual* value to us is rhetorical: it is
third-party confirmation that KiCad-DRC-on-real-boards is the right acceptance bar — the bar we
already chose.

---

## 13. Candidate 13 — OrthoRoute (MIT): what happens when you *do* port PathFinder to a PCB

**What it is.** A GPU-accelerated PathFinder implementation for KiCad: Manhattan lattice (horizontal
on alternating layers, vertical on the others), GPU parallel Dijkstra inside a sequential per-net
loop over a shared congestion map, iteratively raising the cost of over-used edges and ripping up the
worst offenders. **MIT licence.** Requires CUDA 12 or Apple Silicon.

**Evidence quality — and the reason this entry exists.** The author's own reported numbers, stated
without spin: **≈1 % clean-pass rate and ≈30 % routability on small standard boards**, *"not a
general-purpose PCB autorouter"*, *"useful to about five people on the planet"*, *"never trust the
autorouter, but at least this one is fast."* Scope is explicitly *"extremely large, dense, highly
regular multilayer backplanes and BGA escape patterns."*

**What it proves, and it is worth more than a win would be.** **Negotiated congestion is not, by
itself, a clean-pass mechanism.** A faithful PathFinder on a PCB with a lattice discretisation and no
shove scores ~1 % against the same class of referee we use. This is the strongest available caution
against treating §1 as a headline lever: the history term belongs in our ripup as a *modifier*, and
claims of transformative gain should be disbelieved until measured on our corpus.

**Verdict: DECLINE as code (grid lattice, GPU, wrong problem class); KEEP as evidence.** It is the
control arm for §1's experiment, and it is why E3's criterion (§16.3) is deliberately modest.

---

## 14. Candidate 14 — Commercial "AI autorouters" (Quilter, DeepPCB, JITX)

**What it is.** Quilter: RL for combinatorial search + classical solvers for physics + cloud-scale
parallel candidate exploration + "Physics Rule Checks" applied *during* generation. DeepPCB and JITX
are adjacent SaaS.

**Evidence quality: none usable.** Quilter's own essay — *The PCB Autorouter Was the Right Idea.
Completion Rate Was the Wrong Target* — provides **no quantitative routing-quality metric and no
comparative data**; it references a demo project with no benchmarks. Closed source, closed weights,
closed benchmarks.

**The one idea worth keeping, and I think it is genuinely good:** *"a 100 % connected board isn't
necessarily a working board … connecting pins is not the same as producing a board an engineer can
trust."* Their proposed metrics — DRC **plus** return-path continuity, controlled impedance, coupling,
manual-cleanup effort — is a real argument that our clean-pass + connectivity + via-ratio triple is
necessary but not sufficient. It costs nothing to record that as a known limitation of our scoring.

**Verdict: DECLINE (N25).** No method, no code, no evidence. Note the metric critique in the roadmap's
measurement section and move on.

---

## 15. Candidate 15 — The learned-prior family (Unet-Astar, RouteNet-lineage congestion CNNs, GNN/diffusion routing)

**What it is.** Covered in full by `docs/plan-10-prep/unet-astar-assessment.md`. The surrounding
literature — RouteNet-style routability/congestion CNNs, GNN congestion predictors, *URoute*,
FPGA net-level routability prediction — is overwhelmingly **prediction of congestion for placement
feedback**, not routing decisions, and the 2024 robustness work (arXiv 2403.00103) reports these
predictors are sensitive to *"valid and imperceptible perturbations"*.

Applying the assessment's own skepticism checklist to the 2023–2026 crop: synthetic benchmarks
(frequently), DRC-free scoring (frequently), no code/weights/licence (usually). The exceptions in
this survey — PCBWorld (§12) and the offline-RL paper (§5) — both score against a real engine, and
both are useful **for their non-ML content**.

**Verdict: DECLINE, already recorded as N17–N19.** No new evidence found in 2023–2026 that changes
that assessment. **Nothing in this family has yet published DRC-clean results on real boards beating
a conventional router**; PCBWorld's honest *"approaching rule-based routers"* is the high-water mark.

---

## 16. The three bounded experiments

Each has a **pre-fixed decision criterion**. All three are instrumentation-or-setting shaped, default
off, and take M3 as the baseline (roadmap §4 item 4).

### 16.1 **E1 — Obstacle-aware admissible heuristic** *(highest conviction)*

> **Does replacing the obstacle-blind destination distance with an obstacle-aware admissible lower
> bound close the corridor-blindness losses?**

* **Step 1 (S, ~1 day) — measure the hypothesis before building it.** Extend the counter proposed as
  Unet-Astar E1 (`maze/search.rs:531-568`, behind `instrument::on()`): per connection record elements
  **popped**, elements **pushed**, final path length, **and** — this is the addition — the **ratio of
  the final path's true cost to the initial `h` at the start element**. Call it **heuristic slack**.
  A slack near 1.0 means the heuristic was nearly perfect and there is nothing to win. A slack of 3×
  means the search spent its whole life discovering that the straight line was blocked.
  Run over the `regression` + `dac2020` tiers, then a ~200-board sample, **and separately over the 15
  hard-loss boards and `docs/plan-9-prep/fixtures/real-world-screen-keys/`** (W12's fixture).
* **Step 2 (M) — build it, gated off.** A coarse per-layer occupancy abstraction, integer edge
  weights that are provably lower bounds (min trace cost per layer × geometric length; via edge =
  `min_normal_via_cost`), one backward integer Dijkstra from the destination set per connection or
  per net, and `h := max(box_distance, coarse_field)` at `maze/expand.rs:802-817`. Because the new
  term is **admissible**, it may enter the real cost model — no `sorting_value`-only fence is needed.
* **Determinism.** Clean by construction: integer Dijkstra over a fixed abstraction is a pure
  function of `(board, settings, net)`. It **does move goldens** when enabled (a different equal-cost
  path may be found first), so it ships as a setting defaulting to off, with its own goldens and its
  own two-run identity check — the W1 pattern.
* **Decision criterion, fixed in advance.**
  * **Hard-loss boards show median heuristic slack ≥ 2.0** → build step 2 and A/B against M3.
    **PROMOTE to a workstream (W22) if the A/B closes ≥ 3 of the 15 hard losses with no regression in
    corpus clean-pass rate.**
  * **Median slack < 1.3 on the hard-loss boards** → the heuristic is not what is failing there;
    **decline the family**, record `docs/decisions/D-nnn` with the measurement, and put the budget
    into E2.
  * Between 1.3 and 2.0 → build it but hold it as an off-by-default setting; do not spend a milestone.
* **Why this first.** It is the only candidate that is simultaneously (a) an exact architectural match
  to what we already have (rooms are regions, doors are portals), (b) **provably safe** — admissible,
  so it cannot return a worse path, unlike every learned or heuristic corridor prior, (c) free of new
  dependencies and of every determinism hazard in N18, and (d) aimed at a weakness we have **measured**
  rather than one we have read about.

### 16.2 **E2 — Bounded-window repair with violation history**

> **Does a post-batch sweep of bounded windows, each running a small local rip-up-and-reroute driven
> by a history-carrying penalty around actual violations, convert incompletes and DRC failures into
> clean boards?**

* **Shape (L).** After `batch_loop` stops: partition the board into fixed-size windows in a fixed
  order; per window, copy the neighbourhood, add a penalty around each violation and each incomplete
  endpoint, order the affected connections **by distance to the nearest violation** (TritonRoute's
  rule), rip and reroute them locally up to `maxIter` rounds, decay the penalty by round but **never
  subtract it for a specific fixed violation**, and stop the window the instant it is clean. Then
  shift the partition by half a window and sweep again — the alternate-offset trick that repairs
  window boundaries.
* **PENDING on W11 phase 2.** A violation-driven repair needs violations. Today `copper-drc` emits
  **two** types against the referee's **23** (roadmap W11). E2 can start on `clearance` +
  `hole_clearance` + unconnected endpoints, but its ceiling is W11's coverage, and this dependency
  must be stated in the row rather than discovered halfway.
* **Determinism.** Windows are disjoint by construction and swept in a fixed order, so a sequential
  sweep is byte-identical without any threading argument. New setting, default off, own goldens.
* **Decision criterion, fixed in advance.** On the corpus against M3, with the setting on:
  **corpus clean-pass rate up by ≥ 2 points OR ≥ 5 of the 15 hard losses converted, at ≤ 1.5× median
  cpu** → promote to a Plan 10 workstream. **Otherwise decline and record the measurement.**
  The cpu bound is in the criterion deliberately: the roadmap's priority is quality first and speed
  last, but a repair pass that doubles runtime for a fraction of a point is a bad trade, not a win.

### 16.3 **E3 — The escalation schedule, the history term, and pin-access reservation**

> **Three S-sized cost-model changes at one hook, A/B'd together and separately, riding W1's
> portfolio machinery.**

* **(a) Schedule.** Replace `start_ripup_costs * ripup_pass_no` (`maze/engine.rs:1900`) with a
  configurable schedule and search it: geometric (`start × k^pass`, cf. VPR's `pres_fac_mult` = 1.3),
  delayed-start (cf. VPR's `first_iter_pres_fac` = 0.0 — *let the first pass ignore ripup entirely*),
  and step schedules. Evidence for headroom: §5.
* **(b) History.** A `BTreeMap<ItemId, i32>` that **survives the pass**, incremented when an item is
  ripped, read as an extra factor at `ripup_resolver.rs:263`. Evidence: §1. Caution: §13.
* **(c) Pin-access reservation.** An additive cost on the escape geometry of pins whose net is
  unrouted this pass, cleared for the net being routed. Evidence: §6. Aimed at the hard-loss class.
* **How they are run.** Each is a pure function of `(board, settings, pass)`; each is a variant axis
  W1 can enumerate. If W1 lands first, E3 costs almost nothing beyond writing the three functions.
* **Decision criterion, fixed in advance.** Per arm, over the corpus against M3:
  **keep any arm that improves routed-connection count or clean-pass rate with no regression in the
  other and ≤ 1.1× median cpu; discard the rest and record why.** Expect (c) to be the winner on hard
  losses and (a) to be the winner on aggregate; **expect (b) alone to be small** — §13 is the reason
  that expectation is written down in advance rather than after the fact.

---

## 17. Top 3 — worth a Plan 10 workstream, with drafted roadmap-idiom entries

### 17.1 W22 — Obstacle-aware admissible heuristic for the maze search — **PENDING on E1**

**Benefit: QUALITY (headline candidate) · Size: S (E1 step 1) then M · PENDING-on: E1's measurement**

**Evidence.** This file §3, §16.1. External lineage: portal-based true-distance heuristics
(Goldenberg, Felner, Sturtevant & Schaeffer, SoCS), differential/abstraction-based true-distance
heuristics, Anya (Harabor & Grastien, ICAPS'13) and the funnel/string-pulling algorithm — **all
implemented from published constructions; no code is copied from any implementation.** Sites:
`crates/copper-router/src/autoroute/maze/destination_distance.rs` (the heuristic being replaced),
`maze/expand.rs:802-817` (the hook), `maze/search.rs:531-568` (E1's counter).

**The design, stated as ours.** Build, once per pass, a coarse per-layer occupancy abstraction whose
edge weights are provable lower bounds of any real path crossing them. Run one backward integer
Dijkstra from the destination set. Take `h := max(existing box distance, coarse field)`. Because the
term is **admissible and consistent, it enters the real cost model** — it needs no `sorting_value`
fence, unlike **W20**.

**Relationship to W20.** W22 **supersedes** W20 if E1 succeeds. W20 biases the queue with an
*inadmissible* guess and must be fenced out of `expansion_value` to stay safe; W22 sharpens the
*admissible* bound and is safe by proof. If both are built, W22 goes first and W20 is re-scoped to
"is there residual aim to win after the bound is tight?" — which may be nothing.

**Why it moves goldens.** It does, when enabled: a different equal-cost path may be found first.
Setting defaults to off; `tests/reference/**` and `two_runs_of_every_ci_stem_are_byte_identical` hold
at the default; the on-path gets its own goldens and its own two-run check (the **W1** pattern).

**Determinism.** Clean. Integer Dijkstra over a fixed abstraction, a pure function of
`(board, settings, net)`. No floats introduced beyond the single conversion at the hook. **No neural
network under any variant of this row** (N18).

**Open questions.** How coarse before the bound goes slack and the benefit vanishes? Per connection
or per net (amortisation vs tightness)? Does a funnel/string-pull over the returned door sequence
beat `pull-tight` on length, on via count, or on neither?

### 17.2 W23 — Bounded-window repair pass with violation history — **PENDING on E2 and W11 phase 2**

**Benefit: QUALITY (headline candidate) · Size: L · PENDING-on: W11 phase 2, then E2's measurement**

**Evidence.** This file §2, §6, §16.2. External lineage: TritonRoute / OpenROAD `drt`
(Kahng, Wang & Xu, TCAD 2020), **BSD-3-Clause — readable, portable and translatable with
attribution**; the clip decomposition, the marker-cost-with-history, the order-by-distance-to-marker
rule and the pin-access reservation are theirs and are cited as such. Sites:
`crates/copper-router/src/pipeline/batch_loop.rs:174` (where the sweep is appended),
`crates/copper-router/src/autoroute/maze/ripup_resolver.rs:263` (the cost hook),
`crates/copper-drc/` (the violations that drive it).

**The design, stated as ours.** A post-batch sweep of fixed-size, fixed-order windows; per window a
local rip-and-reroute of the connections touching a violation, ordered by distance to the nearest
violation, under a penalty that accumulates within the window and decays by round but is never
subtracted for a specific fixed violation; terminate the window on cleanliness; sweep again at a
half-window offset.

**Why it is the natural consumer of W11.** W11 phase 3 asks the router to *see* the rules it is judged
on. W23 is the mechanism that *acts* on what it sees. Sequenced together, W11 phase 3 first, they are
the strongest quality pairing this survey found. **W23 is worth less than its size suggests until
`copper-drc` emits more than two violation types**, and that dependency is the row's main risk.

**Determinism.** Windows are disjoint and swept in a fixed order; a sequential sweep is byte-identical
with no threading argument required. Setting, default off, own goldens.

**Open questions.** Window size in board units versus in connection count. Whether "incomplete
endpoint" is a first-class marker alongside DRC violations (it should be — it is the hard-loss class).
Whether the sweep belongs before or after the optimiser stage once `#227` makes the optimiser real.

### 17.3 W21 — Cost-model portfolio: escalation schedule, ripup history, pin-access reservation — **PENDING on E3, rides W1**

**Benefit: QUALITY · Size: S–M (three arms) · PENDING-on: W1's fork/join, then E3**

**Evidence.** This file §1, §5, §6, §13, §16.3, and §0.3's reading of our own code. External lineage:
PathFinder (McMurchie & Ebeling, FPGA'95) and VPR/VTR's schedule constants for arm (a); arXiv
2512.03594 for the *premise* that static schedules are suboptimal — **ideas only, the RL is a non-goal
under N18**; TritonRoute (BSD-3) for arm (c). Sites: `maze/engine.rs:1900` (the schedule),
`maze/ripup_resolver.rs:263-290` (the cost), `pipeline/fanout.rs` (escape geometry for arm (c)),
`pipeline/pass_runner.rs:151` (W1's fork point).

**The design, stated as ours.** Three independent, pure, integer cost-model terms, each a function of
`(board, settings, pass)` — a configurable escalation schedule, a per-item ripup-history factor that
survives the pass, and a soft reservation on unrouted nets' pin escapes. Each is a **W1 variant axis**:
if W1 lands, running four schedules deterministically in parallel costs one core each and the winner
is picked by `argmax(normalized_score, −variant_index)` exactly as W1 already specifies.

**Why it moves no golden at the default.** Identity behaviour at the default settings (linear
schedule, no history, no reservation) is literally today's code.

**Determinism.** Clean; integer throughout; no clock, no thread-dependent value, no float introduced.

**Open questions.** Does the history term interact badly with the existing `detour`-based modulation
and the pass-4 randomisation (`ripup_resolver.rs:276-281`)? Should the randomisation be *replaced* by
the history term rather than stacked on it — a deterministic memory instead of a seeded coin?

---

## 18. Non-goals to add to roadmap §3

| # | non-goal | why it is declined | source |
|---|---|---|---|
| **N20** | **Porting KiCad's PNS router wholesale as an alternative routing engine** | **Not a licence problem — the licences are compatible (both GPL-3.0-or-later, §0.1) — an architecture problem.** PNS is an *interactive* router built around a human supplying intent one line at a time; its `NODE` copy-on-write world, its geometry stack and its latency budget are all shaped by that. Importing it means maintaining a second geometry library and a second router forever, which is the Unet-Astar §3c mistake in a better licence. **Its policy ideas are pursued instead, under W21/W23** | this file §4 |
| **N21** | **Building a topological / rubber-band front end (SURF-lineage) for Plan 10** | XL: it is a second pipeline (sketch, routability test, embedder, goldens) coexisting with the room/door maze forever, on 1991–1992 evidence with no open implementation and no modern PCB benchmark. **Declined for Plan 10, not forever** — and the blocking Cadence/Simplex patent family has **expired** (§0.2), so the clearance question does not need re-litigating when it is revisited. Revisit only if **E1** succeeds and a *topological* residue remains | this file §7, §0.2 |
| **N22** | **Learned net-ordering policies (transformer RL, MCTS+DRL, XRoute) in the router** | In-process inference is barred by **N18** (new workspace dependency; float-backend variance defeats cross-platform byte-identity). Independently, the evidence is not inspectable: the IJCAI'25 paper states improvements with **no named baseline router, no named benchmark and no numbers**, and no code, weights or licence was located for it or its neighbours. **The actionable content — "ordering has real headroom" — is already W1**, which gets it deterministically and without a model | this file §8 |
| **N23** | **A SAT / SMT / ILP solver for exact local repair** | Three blockers, in order: (1) **no sound encoding exists** — a finite decision set for a gridless any-angle world with a real clearance matrix is a research contribution, not an import; (2) a solver is a **new workspace dependency** outside ruling BK, and writing one is XL; (3) cross-platform byte-identity is defensible for a pure-integer CP/SAT core and **not** for anything with an LP relaxation. **W23 buys the same "bounded window, terminate on clean" structure with none of the three.** Revisit only if W23 lands and leaves a residue a complete method would provably close | this file §9 |
| **N24** | **Adopting PCBWorld as a benchmark or environment** | It contributes **no routing algorithm**, and as a corpus it is subset-shaped against what we own: 679 real boards against our 1 304, scored by KiCad DRC — which is our referee already (`benchmark/bench/referee/kicad.py`). Its headline ML result is *"approaching rule-based routers"*, i.e. it does not beat one. **Watch as a source of boards our corpus lacks; check non-overlap before importing any** | this file §12 |
| **N25** | **Commercial "AI autorouter" methods (Quilter, DeepPCB, JITX) as a source of technique** | Closed source, closed weights, closed benchmarks, and **no quantitative routing-quality claim we could check** — Quilter's own methodology essay contains none. Nothing here is assessable, so nothing here is adoptable. *The one idea worth keeping is a critique of metrics, not a method: DRC-clean plus connected is necessary but not sufficient for a board an engineer trusts. Record it as a known limitation of our scoring, not as a workstream* | this file §14 |

---

## 19. What I would do, if only one thing

**E1 step 1 (§16.1): measure heuristic slack on the 15 hard-loss boards.** One counter, one report,
one day, no algorithm change, no dependency, no ruling — and it discriminates between the two stories
we have been telling ourselves about corridor blindness. If the slack is large, **W22** is a
principled, provably safe, dependency-free quality lever aimed exactly at our measured weakness, and
it makes **W20** redundant before W20 is built. If the slack is small, we have learned something
expensive-to-guess for the price of a day, and the budget goes to **W23** and **W11** instead.

Everything else in this survey is either bigger, later, or waiting on that number.

---

## 20. Sources

**Negotiated congestion.**
[PathFinder, McMurchie & Ebeling, FPGA'95 (ACM)](https://dl.acm.org/doi/10.1145/201310.201328) ·
[PathFinder cost equations (Lafayette CADApps)](https://sites.lafayette.edu/cadapps/main-page/pathfinder-fpga-routing-algorithm/) ·
[VTR/VPR routing options — `pres_fac`, `acc_fac`](https://docs.verilogtorouting.org/en/latest/vpr/command_line_usage/) ·
[Negotiated A* for FPGAs, Tessier](http://www.ecs.umass.edu/ece/tessier/fpd98.pdf) ·
[Multi-terminal PCB escape routing using negotiated congestion](https://escholarship.org/uc/item/9862j8q0) ·
[OrthoRoute (MIT)](https://github.com/bbenchoff/OrthoRoute)

**Detailed routing.**
[TritonRoute: The Open Source Detailed Router (TCAD 2020)](https://vlsicad.ucsd.edu/Publications/Journals/j133.pdf) ·
[OpenROAD `drt` docs](https://openroad.readthedocs.io/en/latest/main/src/drt/README.html) ·
[OpenROAD LICENSE (BSD-3)](https://github.com/The-OpenROAD-Project/OpenROAD) ·
[Dr. CU (CU-SD licence)](https://github.com/cuhk-eda/dr-cu) ·
[cu-gr / CUGR2](https://github.com/cuhk-eda/cu-gr) ·
[ISPD 2018 contest](https://dl.acm.org/doi/10.1145/3177540.3177562) ·
[ISPD 2019 contest](https://dl.acm.org/doi/10.1145/3299902.3311067) ·
[Accelerating Detailed Routing Convergence through Offline RL, arXiv 2512.03594](https://arxiv.org/pdf/2512.03594)

**Topological / any-angle.**
[Topological routing in SURF (Dai & Dayan)](https://dl.acm.org/doi/pdf/10.1145/127601.127622) ·
[Routability of a rubber-band sketch (Dai, Kong, Sato)](https://dl.acm.org/doi/pdf/10.1145/127601.127623) ·
[US7073151B1 — triangulated-space routing, Cadence, **expired 2023-08-05**](https://patents.google.com/patent/US7073151B1/en) ·
[Altium Situs topological autorouter](https://resources.altium.com/p/automated-pcb-routing-with-situs-topological-autorouter) ·
[Portal-Based True-Distance Heuristics (SoCS)](https://ojs.aaai.org/index.php/SOCS/article/download/18169/17960/21685) ·
[Anya — An Optimal Any-Angle Pathfinding Algorithm (ICAPS'13)](https://users.cecs.anu.edu.au/~dharabor/data/papers/harabor-grastien-icaps13.pdf) ·
[Differential heuristics (Red Blob Games)](https://www.redblobgames.com/pathfinding/heuristics/differential.html) ·
[Funnel algorithm / navmesh path smoothing](https://arongranberg.com/astar/documentation/4_0_9_2cdcee7/class_pathfinding_1_1_funnel_modifier.php)

**Push-and-shove.**
[KiCad PNS source (GPL-3-or-later)](https://github.com/KiCad/kicad-source-mirror/tree/master/pcbnew/router) ·
[PNS namespace reference](https://docs.kicad.org/doxygen/namespacePNS.html) ·
[KiCad interactive router overview](https://deepwiki.com/KiCad/kicad-source-mirror/2.4-interactive-router)

**Learning-based.**
[Transformer-based RL for Net Ordering, IJCAI-25](https://www.ijcai.org/proceedings/2025/1055) ·
[XRoute Environment, arXiv 2305.13823](https://arxiv.org/pdf/2305.13823) ·
[Circuit Routing using MCTS and Deep RL](https://par.nsf.gov/servlets/purl/10382005) ·
[PCBWorld, arXiv 2607.05915](https://arxiv.org/abs/2607.05915) ·
[RouteNet (ICCAD'18)](https://dl.acm.org/doi/10.1145/3240765.3240843) ·
[Robustness of ML congestion predictors, arXiv 2403.00103](https://arxiv.org/pdf/2403.00103)

**Metrics critique.**
[Quilter — "Completion Rate Was the Wrong Target"](https://www.quilter.ai/blog/pcb-autorouter-was-the-right-idea)

**SAT/SMT/ILP.**
[Solving constraints in FPGA detailed routing using SMT](https://www.researchgate.net/publication/304416927_Solving_constraints_in_FPGA_detailed_routing_using_SMT) ·
[ILP-based meander alleviation in PCB routing, arXiv 1705.04984](https://arxiv.org/pdf/1705.04984)
