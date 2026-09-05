# Unet-Astar — assessment for Plan 10

**Status:** exploration deliverable, **uncommitted**, written on branch `plan-9-post-parity`.
Read-only against this repo; no code was changed. Subject:
[`Firesuiry/Unet-Astar-For-PCB-Routing`](https://github.com/Firesuiry/Unet-Astar-For-PCB-Routing),
paper *"Unet-Astar: A Deep Learning-Based Fast Routing Algorithm for Unified PCB Routing"*,
**IEEE Access**, 2023 (arnumber 10274949).

**Sources actually used.** The IEEE PDF is **not** readable from here — `ieeexplore.ieee.org/document/10274949`
returned an empty body and the `stamp/stamp.jsp` PDF returned **HTTP 418** (bot block); the
ResearchGate mirror returned **HTTP 403**. There is **no arXiv preprint**. So the paper's claims below
come from **search-engine abstracts/snippets of the IEEE Access landing page and the ResearchGate
record**, and everything else — the entire method description — is **reconstructed from the code**, a
`--depth 50` clone at
`/private/tmp/.../scratchpad/unet-astar/` (HEAD `c839c53`, 2024-01-16; last code commit `6de4431`,
2023-08-21). Every mechanism claim below cites a file and line in that clone. **Where the code and
the abstract disagree, this document trusts the code and says so.**

---

## 1. What it is, precisely

### 1.1 The pipeline

A **grid maze router in Python/NumPy**, with a U-Net bolted onto the A* cost function.

| stage | where | what it does |
|---|---|---|
| Parse | `dsn_resolve.py:80-208`, `problem.py:133-200` | Bespoke Specctra-DSN reader → `Problem` (board bbox, pins, padstacks, nets, **one** global `grid`/`clearance`/`line_width`, one `via_radius`) |
| Decompose | `solver/base_solver.py:907` `steiner_tree_net_divide` | Every net is cut into independent **two-pin** "steiner_nets". The router never sees a multi-pin net |
| Rasterize | `base_solver.py:519` `generate_obstacle`, `:191` `resolution_change` | Obstacles become a `(layer, x, y)` **`int` bitmap** downsampled by `resolution`. Production runs `resolution_solve(8, 100)` — an **8× downsample** (`base_solver.py:333-338`) |
| Route | `solver/astar/multilayer_astar.py:205` / `multilayer_nn_astar.py:26` | Per pair, a multi-layer 8-connected A*, **sequentially in net index order**, one pass |
| Guide | `base_solver.py:997-1083` + `multilayer_nn_astar.py:60-63` | The U-Net part — see §1.2 |
| Report | `base_solver.py:435-456` | wirelength, via count, connectivity rate, runtime. **`'设计规则违例': -1`** — the DRC-violation field is a hardcoded `-1` and is never computed |

### 1.2 The U-Net's role — a per-connection cost discount, not a path proposer

The network does **not** propose routes. It predicts a **"recommended region"** (`recommend_area`) —
a per-layer, per-cell score in `[0,1]` — and the A* **subtracts** it from `f`:

```python
# solver/astar/multilayer_nn_astar.py:60-63
if self.recommend_area is not None:
    recommend_score = self.recommend_area[neighbor.data]
    neighbor.fscore = (neighbor.fscore - self.liner_nn_power * recommend_score) * (
            1 - self.multi_nn_power * recommend_score)
```

with the **paper's own settings** `liner_nn_power=800`, `multi_nn_power=0`, `skip_percent=0.3`
(`main.py:76-78`, `:85-86`, `:110-115` — and the abstract snippet independently states
*"liner-power=800, skip-percent=0.3"*, so code and paper agree here). A discount of up to **800** against
a per-step `g` of ~1–5 (`multilayer_astar.py:88-104`) is not a nudge: it makes the search **essentially
greedy along the predicted corridor**. It is applied to `f` only (`g` is untouched), so it is a
**weighted/focal-A\* expansion-order change** — the heuristic becomes wildly **inadmissible** and the
returned path is **not** shortest under their own cost model.

**Training target = "cells on a near-optimal path".** `network/genrate_sample.py:19-25` states the label
rule outright: solve the net with **Lee/wave propagation**, keep every cell whose shortest distance is
within *(optimum + 0.5·linewidth + 1)*, label those `1` and the rest `0`
(`LeeSolver2`, `genrate_sample.py:152-270`; labels written as `result_<id>.npy`, `:110-112`). So the
U-Net is a learned approximation of *"which cells lie in the near-optimal corridor"* — a **search-space
reduction**, whose intended benefit is **fewer expansions**, i.e. **speed**.

**Network.** `network/unet.py:29-124` — `ResNetUNet(in_ch=3, out_ch=2)`, a symmetric U-Net with 8
down/8 up stages plus a 3-layer FC bottleneck, `256×256` input. Input channels =
1 start/goal point-feature map + 2 obstacle layers; output = 2 recommend maps. **The model is
hard-wired to 2-layer boards and a 256×256 window** (`base_solver.py:1002` `WIDTH = 256`,
`main.py:71` `ResNetUNet(3, 2)`).

**Guidance is applied to a minority of connections**, by three gates:
* `astar_nn_solver.py:22-25` — no guidance for the **first 30 %** of nets (`skip_percent`);
* `base_solver.py:1012-1013` — **`return None` if the pair's dx or dy exceeds 256 cells** (long nets get nothing);
* `base_solver.py:1022-1052` — the window is cropped to 256×256 around the pair's midpoint, so all context beyond it is invisible.

### 1.3 A*, grid, ordering, ripup

* **A\* variant:** 8-connected in-layer + full-layer via moves; `heuristic_cost_estimate` is octile
  distance + 1 if not yet on an end layer (`multilayer_astar.py:78-86`); `distance_between` is Euclidean
  with `VIA_COST = 5`, a **+3 bend penalty**, and a **+5000 soft penalty for entering an occupied
  cell** (`:88-104`). Sibling solvers exist and are not the paper's: JPS (`solver/astar/jps3d0.py`),
  ant colony (`ant_solver.py`), rectangle decomposition (`solver/rect_solver.py`).
* **Grid resolution:** DSN units ×10, then **÷8** in production (`base_solver.py:333`). Clearance is an
  **L∞ square-window `.any()` test** on the bitmap (`multilayer_astar.py:122-133`), not a Euclidean or
  shape-based check.
* **Single-net:** yes. Two-pin pairs, routed independently, in index order (`base_solver.py:243-256`).
* **Ordering / ripup:** **there is no ripup and no rip-and-reroute loop.** `solve()`
  (`base_solver.py:333-360`) calls `resolution_solve(8, 100)` **once**; every multi-resolution and
  re-route iteration in the method body is **commented out**. `cross_check()` (`:367-433`) only *reports*
  overlaps and repopulates `pending_nets`; nothing consumes it. Conflicts are tolerated via the +5000
  penalty, so **the output routinely contains shorted/overlapping traces** and the reported metric that
  would catch it is the hardcoded `-1`.
* **Notable confound in their own A/B:** `MazeNNSolver.astar` **hard-filters** impassable neighbours
  (`multilayer_nn_astar.py:48 if not self.is_pass(...): continue`) while the baseline `MazeSolver.astar`
  does **not** (`multilayer_astar.py:223-242`, penalty only). The NN arm therefore searches a strictly
  smaller space than the baseline **before** any prediction is used. Some part of the reported speedup is
  this, not the network.

### 1.4 Benchmarks and claimed gains

* **Claim (from the abstract snippet):** *"approximately a **70 % improvement in runtime speed**
  compared to the old router"*, for all given test cases. A secondary contribution is a
  **parameterized random routing-problem generator**.
* **What was actually measured, per the code:** `main.py:51-96` `compare()` loads **pickled
  `RandomProblem` instances** from `D:\dataset` / `Z:\` — *synthetic, randomly generated 2-layer boards*
  (`problem.py:14-40`: random bbox 1000–5000, `pin_density=0.15`, `obs_density=0.1`, `l=2`) — **not** the
  three real `.dsn` files in `dsn文件/`, which are loaded and then **overwritten** by a pickle on the very
  next line (`main.py:30-31`, `:63-64`). Per-net A/B is done honestly (`base_solver.py:756-785` routes each
  net **twice**, with and without `recommend_area`, and records `ori_*` vs `new_*` search area and time),
  and `_route` logs expansions (`base_solver.py:102-117`).
* **Reported metrics:** wirelength, vias, connectivity rate, runtime (`base_solver.py:435-456`).
  **No DRC. No clean-pass rate. No comparison against a human reference or any other router.** The
  "old router" is their own baseline A*.

**Bottom line on what it is:** a **speed** technique — a learned corridor prior that reduces A*
expansions on a coarse grid — validated on **synthetic** boards against **itself**, with **no DRC
evidence at all**.

---

## 2. The impedance mismatch, honestly

| their world | our world | survives translation? |
|---|---|---|
| Cells on an 8×-downsampled `int` bitmap; 8-connected + via moves (`multilayer_astar.py:135-184`) | **Gridless** best-first expansion over **rooms and doors** with shove and ripup (`crates/fr-router/src/autoroute/maze/expand.rs`, `search.rs`) | **No.** Their state space *is* a raster. Ours has no cells to score |
| One global `clearance` + `line_width`, L∞ square-window `.any()` (`multilayer_astar.py:122-133`) | Real clearance matrix, per-net-class rules, padstacks, keepouts, and the KiCad referee on top (roadmap §1.2 item 3, **W11**) | **No** |
| Overlaps allowed with a +5000 penalty; no ripup; DRC field hardcoded `-1` | Clean-pass rate under `kicad-cli pcb drc` is the headline quality metric | **No** — their acceptance bar is one we would score as failure |
| Net = one two-pin pair (`base_solver.py:907`) | Multi-pin nets, ratsnest, fanout, ordering, ripup passes, optimizer | **No** |
| U-Net hard-wired to **2 layers**, `256×256` window, no guidance for nets spanning >256 cells or the first 30 % | Corpus is 1304 boards: **1196× 2-layer**, 75× 4-layer, plus 1/3/6/8/16-layer (`benchmark/corpus/manifest.json`) | Layer count is *nearly* compatible; the window and the gates are not |
| Python + PyTorch + CUDA; float32 NN output steers the search | `#![forbid(unsafe_code)]`, **no new workspace dependencies** except ruling BK, **byte-identical output across runs and platforms** (roadmap §2 constraints) | **No, in-process** — see §3(b) |
| No SES/session writer anywhere in the tree (verified: no `.ses` emitter exists) | Our corpus is judged by re-importing routed output into KiCad | **No** — their router's output **cannot be scored by our referee at all** without us writing an exporter for them |

**Verdict.** Almost nothing survives as *code*: their grid raster, their L∞ clearance and their
crossing-tolerant acceptance bar are all load-bearing, and each one is exactly the thing our router
does not do. What survives is **one idea, in one sentence** — *bias the expansion order toward a
precomputed corridor, and pay for it in `f` only, never in `g`* — and that idea is neither novel to
this paper nor dependent on a neural network.

---

## 3. Incorporation options, ranked

Sizes use the roadmap's S/M/L/XL idiom. "Determinism verdict" is against the Global Constraint:
*same input + settings → byte-identical output across runs and platforms*.

### (a) Ideas-only — a corridor prior on `sorting_value`, no NN — **RANKED 1**

**Size: M** (plus **S** for the headroom measurement that must precede it, §5).

Compute a cheap, deterministic per-connection **corridor field** — e.g. distance from the airline
segment, congestion from the current ratsnest, or a coarse per-layer occupancy grid built once per
pass — and add it as a term at the one hook site that already exists:

```
crates/fr-router/src/autoroute/maze/expand.rs:802-817
    let expansion_value = from_element.expansion_value + add_costs + bend_cost_penalty + weighted_distance(...);
    let sorting_value  = expansion_value + destination_distance.calculate_from_point(...);   // <- the hook
```

**The rule that makes it safe:** the term goes into **`sorting_value` only**, never `expansion_value`.
`expansion_value` is the accumulated real cost that ripup and backtracking read; `sorting_value` is
*only* the queue key (`maze/queue.rs:136-137`, `list_element.rs:78-101`). Biasing the key changes
**which** admissible path is found first; biasing `g` would corrupt the cost model — which is precisely
the mistake their `liner_nn_power=800` makes (§1.2).

* **Determinism verdict: CLEAN.** The field is a pure function of `(board, settings, net)`. `sorting_value`
  is already `f64` compared with raw `<`/`>` (`list_element.rs:78-101`, quirk `#170`), so an extra `f64`
  term adds **no new class** of risk — but the field should be computed in integer or fixed-point and
  converted once, to keep it that way. Gate it behind a setting defaulting to **off**, so
  `tests/reference/**` and `two_runs_of_every_ci_stem_are_byte_identical` do not move (the **W1** pattern).
* **Quality-evidence path:** A/B over the 1304-board corpus against **M3**, scored on clean-pass rate
  (KiCad referee), routed-connection count, via ratio vs the human reference, and wall clock.
* **Plan 10 workstream:** the drafted **W20** (§6). Sits beside **W1**: W1 buys width, this buys aim.

### (b) Offline NN guidance as a committed input artifact — **RANKED 3**

**Size: L in-tree, plus an out-of-tree training project (weeks, GPU).**

Run inference out-of-process, emit a per-net cost field next to the `.dsn`, and have the router read it
as **input data**.

* **Determinism verdict: only conditionally admissible, and the condition is the whole point.**
  * **In-process inference is inadmissible.** `candle` or `ort` is a **new workspace dependency**, which
    the Global Constraint forbids except under ruling BK (which covers a seeded RNG, nothing else) — and
    `ort` pulls ONNX Runtime, i.e. C++ and, in practice, `unsafe`, against `#![forbid(unsafe_code)]`.
  * **Float-backend variance is real, and I will not soften it.** Fixed weights do **not** give
    byte-identical outputs across backends. Multi-threaded GEMM reorders reductions; SIMD width, FMA
    contraction, BLAS/library version and CPU vs GPU all change the last bits. Quantizing the field into
    coarse buckets makes a flip *rare*, not *impossible* — a value sitting on a bucket boundary flips, and
    "rare" is not a determinism guarantee.
  * **The one form that is clean:** the field is **not computed at route time at all**. It is a file,
    produced once, committed or shipped alongside the board, and hashed into the run's inputs. Then
    "same input → same output" holds **by construction**, and the NN is a data-preparation tool that
    never appears in the router's dependency graph. This is the only version worth considering.
* **Blocked on the training side regardless:** **no weights ship** (`.gitignore:*.pth`; `best_val_model.pth`
  is referenced at `main.py:68`, `:100` and is **absent**), and their training corpus is a
  `D:\dataset` of ~40 000 synthetic samples that would have to be regenerated (`run_train.py`,
  `network/genrate_sample.py`).
* **Quality-evidence path:** identical to (a). Which is the argument against doing (b) first — **(a) tests
  the same hypothesis at a fraction of the cost.**
* **Plan 10 workstream:** a **PENDING-on-W20** follow-on. Only reachable if W20's heuristic prior measurably
  wins *and* its residual error is plausibly learnable.

### (c) Full algorithm port as an alternative router mode — **DECLINE**

**Size: XL.** It is not a port, it is a **second router**: a raster world, an L∞ clearance model, a
crossing-tolerant acceptance bar, no ripup, no shove, no optimizer, no multi-pin nets. It would fail our
referee by design, would need its own goldens, and would have to be maintained forever beside the real
one. Determinism verdict: achievable but irrelevant. **Recommend recording as a non-goal (§6, N17).**

### (d) Benchmark-only — run *their* router as a competitor column — **DECLINE** (blocked, and the result would not mean anything)

**Size: L before a single number comes out**, and the number would be uninterpretable.

Four blockers, in order:
1. **No weights** (§3(b)) — so first regenerate ~40 k synthetic training samples and train three models.
2. **No SES/session writer exists anywhere in their tree.** We would have to write their exporter
   ourselves before our referee could see a single board.
3. **Their output is not DRC-clean by construction** — overlaps are a +5000 penalty, not a constraint, and
   there is no ripup (§1.3). The clean-pass column would be ~0, which tells us nothing we do not already
   know from reading the code.
4. **The `java-current` precedent does not transfer.** That column is admissible because the jar solves
   *our* problem under *our* rules and can be scored by *our* referee (roadmap §1.5, Option A). This one
   cannot. It is not a competitor; it is a different game.

**If the question is only "does the 70 % claim hold"**, that is answerable far more cheaply and entirely
inside their repo, on their synthetic generator, with **no** work in ours — and it is still the wrong
question for us, because 70 % of *their* runtime is 70 % of a Python grid A*, and our maze is a
different animal.

---

## 4. License and provenance

* **LICENSE: ABSENT.** There is **no** `LICENSE`, `LICENCE`, `COPYING` or `NOTICE` file at any path in the
  repository, and the `README.md` carries no license grant (it asks only for a star and a citation).
  GitHub's terms allow viewing and forking **on GitHub**; they grant **no** copyright license to reuse the
  code elsewhere. **Consequence, stated plainly: we may read this code and learn from it. We may not copy
  it, translate it, or port it — a line-by-line Rust translation is a derivative work.** Only the
  *published ideas* (from the IEEE Access paper, which is a publication, not a code grant) are usable, and
  they must be re-expressed independently. **This alone removes options (c) and (d) from the table** and
  restricts (a) to what §2 already concluded it is: one idea, re-implemented from scratch against our own
  data structures. Asking the author for an explicit license would be the only way to change this.
* **Research-code health.**
  * **Does it run?** All modules **byte-compile** (`python3 -m py_compile`, clean). Running the paper
    configuration does **not** work as checked out: `main.py:__main__` calls `compare()`, which needs
    `best_val_model.pth` (**absent**) and hardcoded Windows paths `D:\dataset` and `Z:\`
    (`main.py:73`, `:80`, `:104`); `solve()` hardcodes
    `D:\develop\PCB\network\dataset\<hash>\problem.pkl` (`main.py:31`); `network/data_loader.py:19-20`
    still points at a stray `/home/jyz/.../骏马/` image path. It also allocates a **1 GiB named
    shared-memory segment** at import time (`base_solver.py:167-170`) and ships `kill_python.bat`.
  * **Pinned deps?** **No.** `requirements.txt` is 9 bare package names, no versions, no lockfile, and no
    trailing newline — and it **omits** `scipy` and `numpy`, both imported
    (`network/data_loader.py:10 from scipy.ndimage import gaussian_filter`; `numpy` everywhere).
  * **Weights?** **No** — `*.pth` is gitignored.
  * **Tests?** None. `test/` holds four scratch scripts (`snake.py`, `sharemem.py`, …).
  * **Dead/abandoned surface:** the whole multi-resolution and rip-and-reroute strategy is commented out in
    `solve()`; `ant_solver.py`, `rect_solver.py`, `jps*` and a `web/` Flask UI are unused siblings. Last
    code commit **2023-08-21**; the only later commit is a README edit.
  * **Paper vs code:** the two hyperparameters the abstract states (`liner-power=800`, `skip-percent=0.3`)
    **do** match `main.py`. But the abstract's *"for all given test cases"* maps, in the code, to
    **pickled random synthetic 2-layer boards**, not to the three real `.dsn` files in the repo (§1.4), and
    the claimed benefit is **runtime only** — there is no quality claim to inherit, because the DRC field
    was never computed.

---

## 5. Recommendation — one bounded experiment

> **E1 — Measure the corridor headroom in our own maze, before building any corridor prior.**

Their entire contribution is *"A\* expands too many nodes; a learned corridor makes it expand fewer."*
On a `256×256` grid that is self-evidently true. **On a room-and-door graph it may already be false** —
our expansion units are rooms, not cells, and there may be nothing left to prune. **Nobody has
measured this, and every option in §3 is worthless if the headroom is small.** So measure it first.

* **What:** add a counter to the maze search (`crates/fr-router/src/autoroute/maze/search.rs`, beside the
  existing tracing at `:531-568`) recording, per connection: elements **popped**, elements **pushed**, and
  the length of the **final backtracked path**. Define **corridor waste** = `popped / path_length`. Run it
  over a corpus slice (the `regression` + `dac2020` tiers, then a ~200-board sample), grouped by board
  size and net count. Instrumentation only — behind the existing `instrument::on()` switch, off by
  default, **no golden moves**.
* **Cost:** **S — about 1 day.** One counter, one report, no algorithm change, no dependency, no ruling.
* **Decision criterion, fixed in advance:**
  * **median corridor waste ≥ ~10× and a heavy tail** → there is real search to prune; **promote W20 (§3a)
    to a full Plan 10 workstream** and A/B a heuristic corridor prior against M3.
  * **median < ~5×** → our room graph is already near-corridor; **decline the whole family**, record it as
    a `docs/decisions/D-nnn` with this measurement as its evidence, and spend the budget on **W11**
    (make the router see the rules it is judged on) and **W1** (search wider at 1× wall clock), which are
    the roadmap's own two headline quality levers and do not depend on anyone else's research.
* **Why this and not the corridor prior directly:** the roadmap's binding priority is **quality first,
  speed last** (§2 preamble). A corridor prior is, on its authors' own evidence, a **speed** technique with
  **no** quality claim; for us it only becomes a quality lever if the freed budget is converted into more
  search — and **W1 already converts idle cores into width at ~1× wall clock without a neural network, a
  new dependency, or a determinism argument.** E1 is the cheapest thing that can tell us whether the
  aim-narrowing lever is even worth building beside it.

---

## 6. Roadmap-ready draft entries

*Drop-in for `docs/plan-10-prep/roadmap-draft.md` §2 and §3. Numbering assumes W1–W19 and N1–N16 stand.*

### W20 — Corridor-biased expansion order (`sorting_value` prior) — **PENDING on E1**

**Benefit: SPEED (convertible to quality only via W1) · Size: S (E1) then M · PENDING-on: E1's measurement**

**Evidence.** `docs/plan-10-prep/unet-astar-assessment.md` (this file). External lineage: *Unet-Astar*,
IEEE Access 2023 — **idea only; the repository carries no license and nothing may be copied from it**
(§4). Sites: `crates/fr-router/src/autoroute/maze/expand.rs:802-817` (the cost hook),
`maze/search.rs:531-568` (E1's counter), `maze/queue.rs:136-137` and
`maze/list_element.rs:78-101` (the ordering that would change).

**The design, stated as ours.** Compute a deterministic per-connection corridor field from data we
already own — airline geometry, ratsnest congestion, coarse per-layer occupancy — and add it to
**`sorting_value` only**, never to `expansion_value`. The queue key changes; the cost model does not.
Off by default, so `tests/reference/**` and `two_runs_of_every_ci_stem_are_byte_identical` hold
unchanged (the **W1** pattern).

**Why it moves no golden.** New setting, default off, identity behaviour at the default.

**Determinism.** Clean: a pure function of `(board, settings, net)`, computed in fixed-point and
converted once. **No neural network at route time, under any variant of this row** — in-process
inference is a new workspace dependency (forbidden outside ruling BK) and float-backend variance
defeats cross-platform byte-identity outright (§3b).

**Open questions.** E1's verdict is the gate. If the prior wins, does its residual error justify a
learned field **shipped as a committed input artifact** (never as computation)? That is a separate row,
not this one.

### Non-goals to add to §3

| # | non-goal | why it is declined | source |
|---|---|---|---|
| **N17** | **Porting the Unet-Astar router, in whole or in part, as an alternative router mode** | **Its repository has no license** — a translation is a derivative work we have no grant for (§4). Independently: it is a grid raster router with an L∞ clearance model, no ripup, no shove, no multi-pin nets, and a hardcoded `-1` in place of a DRC metric. It would fail our referee by construction | this file §2, §3c, §4 |
| **N18** | **Adding a neural-network inference dependency (`candle`, `ort`, …) to the router** | New workspace dependency outside ruling BK; `ort` implies C++/`unsafe` against `#![forbid(unsafe_code)]`; and fixed weights do **not** yield byte-identical float output across backends, threadings and library versions, which defeats the cross-platform determinism constraint. A learned field is admissible **only** as a committed **input artifact**, never as route-time computation | this file §3b |
| **N19** | **Running Unet-Astar as a benchmark competitor column beside `java-current`** | It ships no weights, has **no SES/session writer at all**, and tolerates overlaps by design, so our referee cannot score it and its clean-pass column would be ~0. The `java-current` precedent does not transfer: that column is admissible because the jar solves *our* problem under *our* rules (roadmap §1.5) | this file §3d |
