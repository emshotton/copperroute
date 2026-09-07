# Plan 10 roadmap — FIRST DRAFT

**Status:** draft, written mid-Plan-9 (branch `plan-9-post-parity`, HEAD `4d3e507`; Tasks 0–4 landed
or landing, Task 6 in flight, M1 accepted-with-gap under ruling BV). **Uncommitted.** This is a
**roadmap**, not an implementation plan: it names candidate workstreams with their evidence, benefit
class, size, dependencies and open questions. Several rows are deliberately **PENDING** a Plan 9
milestone that will answer them, and a row's shape may change entirely when that milestone lands.

**It is finalized after Plan 9 closes (Task 25).** §4 is the checklist the finalizer runs. Every
claim below is cited `file:section` so the finalizer can verify rather than trust.

---

## 1. Purpose — Plan 10 is the fork

**Plan 10 is a total fork from freerouting (Java).** From its first commit the project no longer
follows, tracks, mirrors or references the Java program in any way. This is a complete split, and it
is the frame that unifies every workstream below.

Three user decisions, taken separately, are **one coherent posture** and should be read as one:

| decision | consequence |
|---|---|
| **The fork** (this section) | Java stops being an authority, an oracle, a reference and a source of evidence |
| **No upstream contributions** (user, 2026-09-03) | We do not send fixes back; the fork is not a fork-with-a-return-path. See non-goal **N1** |
| **No Java in comments** (workstream **W18**) | The last place Java survives in the tree — the provenance markers — is removed from the code |

Plans 1–8 built a byte-exact port because parity was the only available acceptance evidence. Plan 9
spent that evidence deliberately: it froze the jar's answers once, then broke parity 121 times on
purpose, and replaced "the jar is the oracle" with the port's own goldens (survey §7, "The harness
transition"). **Plan 10 finishes the sentence**: the program stops being *a port of* anything and
becomes a router that is judged on its own output.

### 1.1 What retires as a reference — three sealed archives

Each of these is **kept** (deleting them would destroy the project's own history) and **never
consulted by any gate**. "Sealed archive" means: readable by a human doing archaeology, referenced by
no test, no script, no CI lane, and no roadmap entry.

| archive | what it is today | what it becomes |
|---|---|---|
| `tests/reference-frozen/java-head-2026-09/` | The one-shot freeze of the jar's answers, 212 blobs, read-only since Plan 9 Task 1 (ruling BP1 / BL8) | Already **write-locked and read by nothing** — BL8's `grep -rn "reference-frozen" crates/ scripts/` returns prose only. Plan 10 changes nothing except the README's tense |
| The `--against-jar` driver hatches | `scripts/differential/run.sh --against-jar` and the converted drivers' jar arm (accept wave, `progress.md:91`: *"`--against-jar` restores the old arm byte-exact"*) | **Removed.** A hatch that restores a jar comparison is a live Java dependency in the harness. The converted drivers keep only their port-golden arm |
| The `java-278fe14` bench view | The synthetic java-only run dir rebuilt by `benchmark/scripts/make-java-view.sh` (605 cells @ sha `278fe14123c4`), the baseline of every Plan 9 milestone compare | **Frozen historical numbers.** Plan 10 milestones compare against **M3** (the port's own last position), not against the jar |

### 1.2 What quality stands on instead

Four instruments, none of which mentions Java:

1. **The port's own goldens** — `tests/reference/**`, regenerated for cause under Plan 9's BT cadence,
   plus the differential drivers in their converted port-golden form.
2. **The two-run identity check** — `scripts/gen-cli-reference.sh:512` `--verify-two-runs` and
   `cli_e2e.rs::two_runs_of_every_ci_stem_are_byte_identical`. This is the determinism gate, and it is
   the reason **W2** exists.
3. **The KiCad referee** — `benchmark/bench/referee/kicad.py`, `kicad-cli pcb drc` over the routed
   board with a project built by `benchmark/vendor/kicad/legacy_rules.py`. It is an **independent**
   judge: it was never written by this project and does not care what the router intended.
4. **The PCBench human references** — the human-routed via/length counts the corpus carries
   (`via_ratio` against the human reference, e.g. register row `#296`'s 0.9636 → 0.9807 mean).

**Consequence, and it is the point:** the KiCad referee currently judges rules the router never sees.
That gap is now the single largest quality lever we own, and it is workstream **W11**.

### 1.3 The decision rule

**Behaviour questions are decided on merit and measurement. Never on "what does Java do".** Plan 9
already half-took this step (Global Constraint, "Java no longer wins": *"Java is still the authority
on what the Java program does, but it is no longer the authority on what the port should do"*).
**Plan 10 drops the surviving half too**: Java is not an authority on anything, including on what the
Java program does, because that question stops being interesting.

This **retroactively simplifies several PENDING entries**, and each says so at its row: a question of
the form "should we restore the thing Java deleted / bisect Java's history to find when it broke" is
re-worded as "does this help *our* router, measured on *our* corpus". See **W6**, **W7**, **W9**,
**W10**.

**Cross-check applied to this document:** no entry below instructs anyone to read Java source. Where
an entry previously leaned on a Java mechanism as its evidence, the evidence is now (a) the
**historical** register row, cited as history, and (b) **our own measurements**. If the finalizer
finds an entry that cannot state its case without opening the clone, that entry is not ready.

### 1.4 `docs/java-quirks.md` closes; drafting its successor

The register (292 rows) becomes a **closed historical document — the fork's origin story**. It is
not edited again, its status column is frozen at whatever Task 25's final sweep leaves, and its own
first rule ("do not fix these until parity is proven", already rewritten once at Task 0) is replaced
by a header saying: *this describes a program we forked from; nothing here is a live obligation.*

**Live tracking needs a Java-free home.** Proposed shape, to be ratified at finalization:

* `docs/decisions/` — one file per decision, id `D-nnn`, with: the question, the measurement that
  answered it, the option taken, the options declined and why. This is where W9's policy defaults,
  W7's sort gate, W10's periodic-tree verdict and W13's keepout experiment land.
* `docs/defects/` — one file per open defect, id `B-nnn`, with a reproduction and a directed test
  name. A defect closes when its test passes; there is no "status cell" to keep in sync.
* **No `// Java bug:` successor in the code.** Under W18 the code carries no provenance markers at
  all; a defect is found by its test, not by a grep over comments.
* The migration is mechanical for the rows that are still live (the `candidate` set, W16) and is
  *not* performed for the ~290 historical rows — they stay where they are, in the closed document.

### 1.5 The Java clone, and the `java-current` bench candidate — two options, both admissible

The clone at `/Users/em/Development/freerouting/freerouting` stops being a dependency of anything.
What happens to its bench candidate is a **product call for the finalizer**, and both answers are
defensible:

* **Option A — keep it as a third-party competitor.** `java-current` becomes exactly what any other
  external router would be in the benchmark: a comparison column, run when someone is curious, with
  no authority. Cost: the harness keeps a JVM path and a jar under `benchmark/binaries/`. Benefit:
  "are we better than the thing we forked from" stays answerable, which is a fair question for a
  release note.
* **Option B — drop it.** Delete the candidate, the jar and the JVM plumbing; the corpus keeps the
  human references and the KiCad referee, which is all a quality gate needs. Cost: the comparison is
  gone and would be expensive to rebuild.

**Recommendation: A, with a hard rule** — `java-current` may appear in a report and may **never**
appear in a gate, a milestone acceptance criterion, or a test. If that rule cannot be enforced
mechanically, take B.

### 1.6 Finalization triggers — which Plan 9 milestone closes which question

| trigger | where it is defined | what it decides | workstreams it unblocks or kills |
|---|---|---|---|
| **T8 close** (group 8, `#159` lands) | plan § "Task 8", steps 2 and "Register"; register row `#295` (`docs/java-quirks.md:341`) | Whether a periodic exhaustive search tree buys anything once `#159` is fixed; whether the real-world screen-keys corridor closes | **W10**, **W12** |
| **T9 close** (`#227` — the optimizer stage starts doing work) | plan § "Task 9"; survey §4.2; multithreading-survey §3.2 | Turns the optimizer from free into the hottest loop in the program; supplies the acceptance rate the speculative-eval design depends on | **W3** (sizes it, or kills it) |
| **M2** (end of T16) | plan § "Task 16"; ledger `progress.md:92` | The `#296` via column; `#231`'s stem-A/B follow-up | **W6**, **W13** |
| **T17 report** | plan § "Task 17"; deliverable `docs/plan-9-prep/stale-index-report.md` | `#193`'s three-option recommendation. The row's status **stays `pinned`** by design | **W8** (its whole shape) |
| **T18 — five verdicts** | plan § "Task 18", "The abandonment rule" | `#172`, `#235`, `#104`, `#210` and the `#44+#63+#74` ordering flip: each confirmed or **measured and declined** | **W9**; also decides whether `copper_geometry::Line`'s identity counter survives |
| **T18 — the `#296` work** | ruling BV (`progress.md:84`); register row `#296` | How much of the +3.80 % corpus via inflation T18 explains | **W6** (Plan 10 inherits the residue, re-scoped per §1.3) |
| **T18 — the large-board sort policy row** | ruling BV: *"the large-tier R1 attribution question FILED TO TASK 18 as a policy row (net-count-gated sort …, A/B'd at M3)"* | Whether the airline-first sort should be gated by net count | **W7** |
| **M3** (end of T18) | plan § "Task 18" MILESTONE block | `overall.verdict`, hard losses, D3-small vs M1, the M2→M3 cpu ratio | **The baseline every Plan 10 A/B is taken against** — replaces the jar view |
| **T19 close** | plan § "Task 19"; ruling BU(c) (`progress.md:72`): *"the port-DRC-lacks-`track_width` gap gets a register row — controller note appended to T19's brief with ownership"* | How much of the referee's rule set the port's own DRC checks after Plan 9, and the id of the gap row | **W11** (its whole size) |
| **T23 / T24 close** | plan § "Task 23" (DJ1, ~110 std-replacement sites), § "Task 24" (DJ2, ~150 deletions) | How many Java-shaped shims and `java_*` identifiers survive | **W18**, **W19** (their starting inventory) |
| **Plan 9 tail** | ledger `progress.md:101`: multithreading-survey shortlist items **2 and 3** "assigned to the plan's tail" — **no written task owns them** | Whether the harness fan-out lands inside Plan 9 | **W17** |
| **T25 close** | plan § "Task 25", parts 2, 3.4, 3.7 | The final-sweep parked list; which drivers retired and which converted; the divergence table incl. **the legacy-flag ramp's expiry date**; the final marker census | **W14**, **W15**, **W16**, **W18**, and §1.1's archive sealing |

### 1.7 One thing Plan 10 must **not** be used for

`…/2026-09-03-plan-9-post-parity.md` § "Task 25", part 2: *"A finding that cannot be fixed is a
recorded, closed decision with its evidence — the Plan 8 precedent — **never a deferral to a Plan
10**."* Plan 10 does not inherit Plan 9's review findings. If a row below turns out to be a Plan 9
review finding in disguise, it belongs in Plan 9's fix wave and must be struck from here.

---

## 2. Workstream candidates

**Standing priority, unchanged and binding: routing quality first, robustness second, speed last.**
(`docs/plan-8-handoff.md` §9 preamble, *"better routing beats speed"*; ruling BO, `progress.md:9`,
*"hard routing metrics first, speed second"*; imported roadmap §6.1, *"Performance work — real
parallel passes included — is explicitly last"*.)

**Constraints Plan 10 inherits unless a ruling lifts them** (plan § "Global Constraints"):
byte-for-byte determinism across runs and platforms; `#![forbid(unsafe_code)]`; the `catch_unwind`
boundary count stays **seven**; no static mutable state; no new workspace dependencies except under
ruling BK (stable named seeded RNG; `StdRng`/`SmallRng` excluded); explicit-path staging;
commit-message verification blocks. **Ruling AM** ("no threading policy, no rayon, no threads") is
the one constraint several rows ask the controller to *narrow* — see **W1**.

**The two headline candidates are W11 and W1.** Both attack clean-pass rate from opposite ends —
W11 by making the router see the rules it is judged against, W1 by searching wider for the same
wall-clock. Everything else supports, enables or tidies.

---

### W11 — Port KiCad's routing-relevant DRC rules and enforce them during routing

**Benefit: QUALITY (headline) · Size: L–XL · PENDING-on: T19's outcome and its gap-row id**

**The gap, measured.** The referee counts **23** routing-relevant violation types
(`benchmark/bench/referee/kicad.py:38-47`, `ROUTING_DRC_TYPES`: `clearance`, `hole_clearance`,
`hole_near_hole`, `track_width`, `annular_width`, `via_diameter`, `via_dangling`, `track_dangling`,
`shorting_items`, `tracks_crossing`, `items_not_allowed`, `copper_edge_clearance`, `copper_sliver`,
`isolated_copper`, `connection_width`, `drill_out_of_range`, `microvia_drill_out_of_range`,
`zones_intersect`, `zone_has_empty_net`, `starved_thermal`, `npth_copper_clearance`, `padstack`,
`unconnected_items`). The port's own DRC emits **two** violation types plus unconnected items —
`clearance` and `hole_clearance` (`crates/copper-drc/src/report/mod.rs:119`,
`crates/copper-drc/src/report/json.rs:93-124`; the crate's whole check surface is `checker.rs` +
`unconnected.rs`). **The router therefore optimises against a two-rule world and is graded against a
twenty-three-rule one.**

**The proven example.** R2 (`#294`) stopped the micro-neckdown fallback emitting sub-minimum traces.
Its entire benefit is a `track_width` improvement — a rule `copper-drc` **cannot express**. Ruling BU(a)
(`progress.md:72`) had to send the adjudication to the corpus for exactly this reason: *"ADJUDICATION
= M1 (the KiCad referee counts `track_width`, so R2's benefit is visible there)"*, and BU(c) filed
the gap as an owed register row. M1 then measured the shape of the trade: large-tier DRC-clean
**0.386 → 0.812** against connected **0.371 → 0.322** (`progress.md:83`). A router that could see the
width rule would not have to buy legality with connectivity — it would route legally in the first
place.

**The rule set is already enumerated, in our own tree.** `benchmark/vendor/kicad/legacy_rules.py`
builds each corpus board's `.kicad_pro` and names the fifteen board-wide floors the referee then
enforces (`_ZEROED_RULE_KEYS`: `min_clearance`, `min_connection`, `min_copper_edge_clearance`,
`min_hole_clearance`, `min_hole_to_hole`, `min_microvia_diameter`, `min_microvia_drill`,
`min_silk_clearance`, `min_text_height`, `min_text_thickness`, `min_through_hole_diameter`,
`min_track_width`, `min_via_annular_width`, `min_via_diameter`,
`solder_mask_to_copper_clearance`), plus the per-net-class `clearance` / `trace_width` / `via_dia` /
`via_drill` / `uvia_*` values it reconstructs. **This is a written specification we already own**, and
`kicad-cli` 10.0.3 is present on the machine (`progress.md:52`).

**Three layers, and they should be three phases.**

1. **Represent** the rules — extend `copper_board::BoardRules` (and the DSN/KiCad readers that fill it)
   with the floors it does not carry. *Note the tension:* `BoardRules::get_min_trace_half_width()`
   exists (`crates/copper-board/src/rules/board_rules.rs:148`) and R2 already consumes it, so the
   minimum-width floor is **half-present**; most of the other fourteen are absent entirely.
2. **Check** them — `copper-drc` grows a check per rule, each with its own violation type, so the port's
   DRC report and the referee can be reconciled row for row. **Acceptance for this phase is
   agreement with `kicad-cli`, per board, per type**, on the corpus — a genuinely independent oracle,
   and the first one this project has had that it did not write itself.
3. **Enforce** them during routing — the expensive, valuable half. Rules the maze and the shover must
   respect (width floors, annular width, copper-edge clearance, hole-to-hole) become part of the cost
   or the feasibility test, not a post-hoc report.

**Dependencies and cautions.**
* **PENDING T19** for phases 1–2's starting point: T19 rewrites `copper-drc`'s report surface, deletes
  `smallest_clearance`, and lands `--fail-on-violations` and `--unit`. Building on it before it lands
  is rework.
* Phase 3 moves every routed golden and is a milestone-sized change; it wants its own M-bench.
* **Do not let phase 2 become the deliverable.** A better report that changes no route is a Tier-3
  improvement wearing a Tier-2 badge; the clean-pass rate only moves in phase 3.
* Interacts with **W1**: a wider search is worth more when the scoring function knows which candidates
  are illegal. If both land, sequence W11 phase 3 first.

**Open questions.** Which of the 23 types are genuinely routing-controllable versus artefacts of the
corpus's legacy boards (the referee's own comment flags this: *"a 4-port USB hub board with 84 total
DRC errors and only 3 routing-type ones"*). Whether the port should read `.kicad_pro` directly rather
than only the DSN's rules. Whether zone-related types (`zones_intersect`, `starved_thermal`,
`isolated_copper`) are in scope at all for a router that does not fill zones.

---

### W1 — The deterministic variant portfolio (`router.pass_variants`)

**Benefit: QUALITY (headline) · Size: L · PENDING-on: W2, a ruling-AM narrowing, T18**

**Evidence.** `docs/plan-9-prep/multithreading-survey.md` §3.1 and shortlist row 1 (§7). Site:
`crates/copper-router/src/pipeline/pass_runner.rs:151` (`run_single_thread`), driven from
`crates/copper-router/src/pipeline/batch_loop.rs:174`.

**The design, stated as ours** (its lineage is a historical note in the survey, not an input): run N
complete autoroute passes on independent `Board` clones, each with a work-list permutation that is a
pure function of `(pass, variant)` through a stable named PRNG, and keep the winner by
`argmax(normalized_score, −variant_index)`. No clock anywhere; nothing joins with a timeout; every
variant runs to completion. The result is identical on 1 thread or 64, on arm64 or x86-64, on a busy
machine or an idle one, because the only inputs are the board, the settings and `(pass, variant)`.

**What it buys.** N-way wider search at **~1× wall-clock** up to core count. It is the only row in the
survey that buys *quality* rather than speed.

**Why it moves no golden.** Setting defaults to `1`; variant 0's permutation is the identity; the
default path is literally today's code. `tests/reference/**`, `--verify-two-runs` and
`cli_e2e.rs::two_runs_of_every_ci_stem_are_byte_identical` hold unchanged. `pass_variants > 1` is a
**new mode** with its own goldens and its own determinism proof (survey §3.1 plus a two-run identity
check at N=8 over the 29 stems).

**Dependencies / preconditions.**
1. **W2 is not optional.** `RouterBudget::default().fanout_ms_per_pin` is still `10_000`
   (`crates/copper-router/src/pipeline/stop.rs:529-534`, read at draft time). Variants contending on one
   machine lengthen a pin's fanout, and a per-pin wall clock turns that into different bytes.
2. **A controller decision on ruling AM.** The ruling's stated reason ("a threaded maze would be
   non-deterministic") does not reach a portfolio of N whole, unmodified, sequential mazes — but
   narrowing it is the controller's call. **PENDING a ruling.**
3. **Ruling BK** governs the PRNG: survey §6 recommends `std::thread::scope` + an in-tree splitmix64
   (~20 lines). **No new dependency; no `rayon`.**
4. `#124` (a zero-sized pool surviving validation, Plan 9 Task 21): the new setting is validated in
   its own right and inherits nothing.

**Open questions.** Did T18 take the setting + seeded-PRNG plumbing (survey §3.1 suggests it could
ride group 18)? If so W1 shrinks to the fork/join and the selection. Memory: 53 MB peak RSS on
DAC2020 → ~8 variants ≈ 400 MB — re-measure after `#227` makes the optimizer real.

---

### W2 — Work-bound the fanout budget (`fanout_ms_per_pin`) — **UNOWNED, assigned here**

**Benefit: ROBUSTNESS (determinism) · Size: S–M · PENDING-on: nothing**

**Evidence.** `docs/plan-9-prep/multithreading-survey.md` §7 ("Cross-cutting precondition for 1, 3
and 5"), §3.1, §4.2. Site: `crates/copper-router/src/pipeline/stop.rs:529-534`.

**Ownership finding.** `#234` (Plan 9 Task 1) removed **one** of the two live wall clocks in the
default CLI budget and left this one standing — confirmed at ruling BR(b) (`progress.md:56`: *"seam
HALF-redundant: `fanout_ms_per_pin` still differs"*). **No Plan 9 task owns it** (`grep
fanout_ms_per_pin` over the plan and the register returns nothing but Plan 7 ruling-AI prose at
`docs/java-quirks.md:554`), and the survey's recommendation to open a register row
(`progress.md:101`) was never acted on. **Plan 10 takes it.** Under §1.4 it does not become a
`java-quirks.md` row at all: it becomes the successor tracker's first defect entry.

**What it buys.** It is the **last live machine-speed dependency in the default CLI budget**.
Removing it (a) unblocks W1 and W5, (b) makes the reference-generator fan-out safe outright rather
than "safe with a per-core cap and a serial verify" (survey §4.2), and (c) removes the class of
irreproducibility the two-run identity check exists to catch — which, post-fork, is the *only*
determinism evidence we have.

**Preconditions / cautions.** It **changes what the router computes** on a board that trips the limit
today, so it moves goldens and needs its own A/B — precisely the reason `#234`'s site comment gives
for not taking it at Task 1 (`stop.rs:520-527`). Sequence it early, for the same reason Task 1 came
first in Plan 9.

---

### W3 — Optimizer: speculative per-item evaluation, sequential commit

**Benefit: SPEED (quality-neutral by construction) · Size: L · PENDING-on: T9 / `#227`**

**Evidence.** `docs/plan-9-prep/multithreading-survey.md` §3.2, shortlist row 4. Site:
`crates/copper-router/src/pipeline/optimizer.rs:437` (the item loop) and `~:560-660` (`opt_route_item`).

**Why it is PENDING.** `#227` records that the optimizer today "runs, visits every item, and changes
nothing" (survey §4.2). Group 9's fix makes it real — *"the single largest quality change in the
catalogue"* — and each item then costs one whole-board `deep_copy` plus a rip, a re-route and a
`calculate_incomplete_count`. **T9's measured acceptance rate is the input to this row's entire
benefit estimate** (survey §3.2 assumes ~10 % accept → ≈ K-way on the other 90 %). A high accept rate
shrinks or kills the row.

**Determinism.** Byte-identical to the sequential post-`#227` golden **by construction**: commits in
index order, a speculative result committed only if its base board is the one the sequential run
would have handed it, invalidated results discarded and recomputed. **No separate goldens.**

**Dependencies.** W2 (same cross-cutting precondition); strictly after T9 lands and publishes its A/B
under `benchmark/baselines/ab/`.

---

### W4 — The `&Board` read-path refactor

**Benefit: ROBUSTNESS (enabler) · Size: M–L · folds naturally into W19**

**Evidence.** `docs/plan-9-prep/multithreading-survey.md` §1. Two halves that must be separated:

* **The counter half is free.** `entry_counter`
  (`crates/copper-board/src/searchtree/shape_search_tree.rs:1100-1146`) is drawn only inside one call,
  consumed only as the second key of a call-local `BTreeSet`, and **never escapes** — `result` pushes
  `sorted.entry`, not `sorted.entry_id`. **A per-thread counter starting at any value yields
  byte-identical query results.** This myth-busts a long-standing precondition: the counter was never
  a parallelism blocker (survey §1, and §6's `#30, #61` row).
* **The real blocker:** the board's *read* paths take `&mut Board`, and one of them writes.
  `Board::clearance_violations` is `&mut self` (`crates/copper-board/src/board/clearance.rs:37`);
  `overlapping_items_with_clearance` is `&mut self` only to carry the counter
  (`crates/copper-board/src/board/query.rs:382-395`); and `ForcedPadRouter::check_forced_pad` genuinely
  mutates — `board.set_shove_failing_obstacle(outline)`, a **last-writer-wins diagnostic field**
  (`board_ext/forced_pad_router.rs:61,76-79`).

**What it buys on its own merits:** a read that cannot write is a smaller contract, the diagnostic
becomes a returned value instead of hidden state, and the whole shape is what an idiomatic Rust
codebase would have had from the start — which is why **W19 should absorb it** rather than schedule
it twice.

**Cautions.** It touches "the hottest and most parity-fragile code in the port" (survey §4.3). Gate it
on `structural_hash` equality and the two-run identity check.

---

### W5 — Per-layer via feasibility in `expand_to_other_layers`

**Benefit: SPEED (8–10 % wall) · Size: L · PENDING-on: W4, W2 — and on W1's outcome**

**Evidence.** `docs/plan-9-prep/multithreading-survey.md` §4.3, shortlist row 5. Sites:
`crates/copper-router/src/autoroute/maze/expansion_engine.rs:379` and `:680`;
`board_ext/forced_via_inserter.rs:41`. 14.1 % of the profiled run is here.

**The survey's verdict, carried forward:** *"an L-sized refactor of the code with the least slack in
it, for 8-10 %. If `pass_variants` lands, the cores are better spent there. Recorded so the candidate
is not lost; not recommended."* Keep it as a recorded candidate with **lowest confidence**; do not
schedule it unless W1 is declined.

---

### W6 — The via count: close it on our own terms

**Benefit: QUALITY · Size: M (re-scoped by §1.3) · PENDING-on: M2, T18**

**Evidence.** Register row `#296` (`docs/java-quirks.md:342`), elevated at `progress.md:92`: *"via
inflation SURVIVES R1+R2 at corpus scale — 15539→16129 (+3.80 %, d3-c +4.19 %) vs the pre-fix port;
the stem A/B's −4.6 % was stem-local."*

**The fork re-scopes this row, and honestly.** The row as written asks for a **bisect of Java's
history, v2.2.4 → v2.3.0**. Under §1.3 that is archaeology, not a fix input: it would tell us when
*their* program changed, and we would still have to decide on merit whether our via count is right.
So Plan 10's version of the question is:

> Against the **human reference** and the **KiCad referee**, is our via usage good — and if not, which
> of our own cost terms is responsible?

The instruments are already in place: `via_ratio` against the human reference (0.9636 → 0.9807 mean
at M1), the per-D3-subset totals, and `benchmark/baselines/ab/quality-ab-T*.tsv`'s `via_total` /
`via_through` / `via_blind` / `via_buried` columns. The first suspect is our own via cost policy —
the pure-SMD relaxation that multiplies the via cost factor by `0.1` (Plan 9's `#172`, T18) — and
that is a knob we own and can A/B directly.

**Tension, flagged not resolved.** T18 may still run the Java bisect inside Plan 9, because Plan 9's
constraints still name it. If it does and it finds something, take the finding — it costs nothing to
accept a result already paid for. Plan 10 simply does not *commission* more of it.

---

### W7 — Should the airline-first work-list sort be gated by net count?

**Benefit: QUALITY · Size: M · PENDING-on: T18 / M3**

**Evidence.** Ruling BV (`progress.md:84`): M1 was accept-with-gap and *"the large-tier R1 attribution
question FILED TO TASK 18 as a policy row (net-count-gated sort per the report's conservative option,
A/B'd at M3)"*. The numbers (`progress.md:83`): large connected **0.371 → 0.322** against large
DRC-clean **0.386 → 0.812**.

**Post-fork wording.** The original row argued from a Java commit's note and its contradiction. That
argument is now history. The live question is ours alone: **on large boards, does sorting the work
list by airline distance help or hurt, and is a net-count gate the right shape of answer** (versus a
continuous weight, versus a different key entirely)? A gate whose threshold is a literal is a smell;
if M3 keeps one, Plan 10 turns it into a measured setting.

**Plan 10's share.** Struck if M3 confirms the unconditional sort. Otherwise: the gate's threshold as
a measured setting, plus the interaction with W11 phase 3 (a router that knows the width rule may not
need the connectivity trade at all).

---

### W8 — `#193`: act on the stale tree-index recommendation

**Benefit: QUALITY / ROBUSTNESS · Size: M–L · PENDING-on: T17's report**

**Evidence.** Plan § "Task 17": three guards each admit *"an item's tree-shape indices go stale while
the search is running"*; they silently `continue`, and one **resizes `expansionRoomArr` mid-search**.
*"The guards convert a corrupted search into a quietly worse route."* Task 17 is a **discovery**
task and says so: *"this task does not fix `#193`, and saying so is the point"* — the deliverable is
`docs/plan-9-prep/stale-index-report.md`, per guard per stem: fire count, preceding mutation,
recoverability verdict, and a recommendation among three named options (re-derive the index at use;
make it a generation-checked handle; leave the guard and document it).

**Plan 10 acts on it.** Size is entirely T17's output: option 1 is S–M, option 2 is L (it touches
every tree-index holder and is a natural part of **W19**), option 3 closes the row. **PENDING the
report — do not pre-commit to an option here.** Note that this is a defect our code has *now*,
independent of where it came from: it needs no historical justification to be worth fixing.

---

### W9 — Re-examine the policy defaults T18 decided

**Benefit: QUALITY · Size: M · PENDING-on: M3's six verdicts**

**Evidence.** Plan § "Task 18" and its abandonment rule: each of the five remaining rows is committed
separately, measured at G2 separately, carried into M3, and *"a row whose G2 shows a hard loss is
reverted in the same task, its register row is set back to `pinned` with the measurement recorded,
and Task 25's report names it as measured and declined"*. The plan predicts its own most likely
decline: `#44 + #63 + #74`, the ordering flip.

**Why a declined row is still a candidate.** Two of the remaining rows ship as **settings with today's
behaviour as the default** (`router.smd_via_relaxation` on and
`router.failure_give_up_threshold` disabled). A default decided from a **single** M3 measurement is a
defensible first answer, not a final one — and post-fork none of the three has any claim to its
current default beyond that measurement. Plan 10 re-measures them against the post-Plan-9 baseline,
with the rest of the catalogue underneath them, which is the same argument that put Task 18 last.

**Coupled decision.** If `#74` is abandoned, `copper_geometry::Line`'s identity counter survives as the
port's **only** static-mutable-state exception. Post-fork the counter has no parity justification at
all, so deleting it becomes a straightforward W19 item — it just needs a value comparison that
performs acceptably.

---

### W10 — Is a periodic exhaustive search tree worth running?

**Benefit: QUALITY (possibly none) · Size: S–M · PENDING-on: T8's measurement after `#159`**

**Evidence.** Register row `#295` (`docs/java-quirks.md:341`); plan § "Task 8" answers it and closes
the row. The remains in our tree are three reader-less lines
(`crates/copper-board/src/rules/board_rules.rs:61, :514, :519`).

**Post-fork wording.** Not "should we restore what they deleted" — **"does periodically running the
exhaustive tree instead of the 45-degree one improve our routes enough to pay for a 4× cost on every
fourth pass?"** T8 answers it *after* `#159` lands, because the exhaustive tree's `completeShape`
keeps the room the 90-degree override drops, so the two questions are entangled.

**Three outcomes, one of which reaches Plan 10.** (a) No benefit → the flag and both accessors are
deleted inside Plan 9 (or, failing that, by W19). (b) A benefit → it becomes a real routing-policy
setting with a cost model, which is Plan-10-shaped. (c) Unmeasurable (no fixture reaches the regime)
→ Plan 10 builds the fixture. **PENDING-T8 for which.**

---

### W12 — The real-world corridor: rooms and doors the maze still misses

**Benefit: QUALITY · Size: M · PENDING-on: T8, then T9/T10**

**Evidence.** `progress.md:54`: *"REAL-WORLD EVIDENCE ADDED (user board):
`docs/plan-9-prep/fixtures/real-world-screen-keys/` — both implementations miss a ~6 mm legal
top-edge corridor (5V, 70/71). Door-drop hypothesis (`#160`/`#161`/`#163`). Task 8's brief gets a
controller note: append this board to its stem A/B with the `incomplete_count == 0` post-fix
assertion; **if unfixed by T8, escalate to T9/T10 with the same file**."* Task 2 already measured
that R1+R2 do **not** close it (`progress.md:71`: 1→3 incompletes).

**Post-fork note.** The original evidence line reads "*both* implementations miss it" — that was
useful when we needed to know the miss was not a port defect. It is no longer relevant: **a legal
corridor our router does not use is our defect, full stop.** The fixture stands; the comparison does
not.

**Plan 10's share.** The escalation ladder ends at T10. If T8/T9/T10 all leave it open, Plan 10 owns
it as a **diagnosis** workstream — why does a legal corridor never become a door? — not a fix row.

---

### W13 — Does the 500 µm board-edge keepout apply at all?

**Benefit: QUALITY · Size: S (plus one corpus run) · PENDING-on: T16's stem A/B**

**Evidence.** Survey §11 question 3 recommends making the option continuous, keeping 500 µm as the
default *"then measure removing it as its own experiment"*. Ruling BP3 (`progress.md:14`) demoted the
corpus arm to a Task-16 stem A/B, and the plan's dispatch section says a corpus number *"would be a
controller-authorised fourth run and is not authorised here"* — so the experiment is deferred out of
Plan 9 by construction.

**Plan 10's share.** If T16's 29-stem A/B shows signal, run the corpus arm against M3. The cost is one
605-board bench run on the workbench, not a code change. Note the interaction with **W11**: the
referee has its own `copper_edge_clearance` rule, so once the router checks that rule directly, a
blanket 500 µm keepout may be the wrong instrument entirely.

---

### W14 — The deprecation ramps come due

**Benefit: ROBUSTNESS / product predictability · Size: S · PENDING-on: T25's divergence table**

Two "one release" promises made inside Plan 9, both falling due after it:

1. **The legacy CLI flag ramp.** Survey §11 question 4: fix the native form immediately, *"give the
   legacy flag form one release of 'matched by prefix, warned as deprecated' before it becomes
   exact"*. Task 25 part 3.7 writes *"exact flag matching with the legacy ramp and **its expiry
   date**"* into the divergence table. Plan 10 executes the expiry.
2. **The `Legacy` DRC reader.** Survey §11 question 6: keep the reader one release, make
   `KiCad` the only spelling written. Task 19 makes `KiCad` the only **writer**; the reader's deletion
   is the second half. **Post-fork this is trivially decided**: a reader whose only purpose is to
   parse a format the forked-from program emitted has no constituency here.

**Open question.** Both are user-visible breaking changes and want a version boundary (likely
`v1.2.0`) plus release-note text. Confirm the expiry date T25 actually wrote before scheduling.

---

### W15 — Measurement-integrity residue

**Benefit: ROBUSTNESS (measurement) · Size: S · PENDING-on: verification at T25**

Small, and currently unowned by any Plan 9 task:

1. **`bench compare` pools same-named candidates across runs.** `benchmark/bench/compare.py:68-98`
   (`collect`) appends every run dir's cells into `cells[candidate][board]` and `aggregate` medians
   the list — so a compare whose `--runs` name two run dirs both containing `rs-main` silently
   **averages two milestones into one column**. The ledger queued exactly this (`progress.md:42`:
   *"bench `compare.py` same-name-averaging fix (controller inline commit + test — no plan task owns
   it)"*) and **it has not landed**: `git log main..HEAD -- benchmark/bench/compare.py` is empty at
   draft time and the code is unchanged. It never corrupted a milestone (each compare named one java
   view + one rs run), but it is why BP3 had to note the M1→M2 delta is read from two compare JSONs
   (`progress.md:30`). **Post-fork it becomes load-bearing**: with the java view retired, *every*
   Plan 10 compare is rs-vs-rs and the pooling hazard is live on the main path. Fix it before the
   first Plan 10 milestone.
2. **`scripts/differential/rust/src/bin/p8t0.rs` does not compile** — pre-existing, filed by Task 4
   as a final-sweep ticket (`progress.md:86`, concern 5). **Judgement: sweep-shaped, not
   Plan-10-shaped**; listed only so the finalizer can confirm the sweep took it.
3. **Sealing the archives** (§1.1): removing the `--against-jar` hatches and re-writing the frozen
   tree's README in the past tense. S, and it is the fork's most visible single commit.

---

### W16 — Register residue: the `candidate` rows with no owner

**Benefit: mixed · Size: S each · PENDING-on: T25's final status sweep**

From `docs/java-quirks.md` § "Improvement candidates" — rows no Plan 9 task claims. **Under §1.4
these migrate into the successor tracker; the register itself closes.**

| row | class | note |
|---|---|---|
| `BigInteger` fallback → `i128` where products provably fit (`line.rs`, `int_point.rs`, `rational_*`) | SPEED | Also a micro-perf non-goal in the imported roadmap §6.1. **Speed is last**; carry it, do not schedule it. Post-fork it also stops being "what Java promotes" and becomes a plain question about our own arithmetic |
| Lawful `Ord` on `IntDirection`/`Line` once NULL is defined as minimum | ROBUSTNESS | Deletes `sort_by` shims; a natural **W19** item — check whether DJ1 (T23) already took it |
| `pub(crate)` fields on `RationalPoint`/`BigIntDirection`; `RationalVector::determinant` rename; `Hash` on `Vector`/`Direction` | ROBUSTNESS | Three one-line hygiene rows; fold into **W19** |
| The lexer's two-`static`-fields divergence; `DsnScanner` converting at construction | — | These rows exist to record divergences *we already made*. Post-fork they are simply how our lexer works, and the rows retire with the register |
| `#236` — the score breakdown reads raw board units where the live score uses mm | QUALITY (reporting) | Plan 9 Task 20 records the decision (default: not ported) and sets the row to `candidate`. Porting a correct breakdown is a Plan 10 row, and post-fork it is *designing* one, not porting one |
| `#251` — `BoardStatistics.host` as `Option<String>` (ruling **BD**) | — | **CHECKED: owned.** Plan 9 **Task 20** takes it (plan § "Task 20", commit 2: *"Land with `#248`"*). **Not Plan 10's** |

---

### W17 — Harness fan-out (survey shortlist items 2 and 3) — only if the Plan 9 tail declines

**Benefit: SPEED (harness only) · Size: S · PENDING-on: Plan 9 tail / T25**

**Evidence.** `docs/plan-9-prep/multithreading-survey.md` §4.1, §4.2. Item 2: fan out
`scripts/quality-ab.sh:511-560`'s quality + referee lanes — **~1.25×**, ceiling structural because the
timing lane must stay sequential (3 × 111 s irreducible); zero determinism risk *conditionally*,
because that lane already sets `COPPERROUTE_ROUTER_BUDGET=disabled` (`:539`) — **encode that as an assertion,
not a comment**. Item 3: fan out `gen-cli-reference.sh:541` / `gen-batch-reference.sh:511` —
near-linear, but **safe only once W2 lands**, or with a per-core cap and a serial `--verify-two-runs`
as the acceptance step.

**Why it is here.** `progress.md:101` assigned both to "the plan's tail", but **no written Plan 9 task
owns either**. If the tail takes them, strike this row.

---

### W18 — Comments: the code documents itself

**Benefit: readability / maintainability · Size: L · PENDING-on: a provenance ruling; pairs with W19**

**The philosophy, as given.**

1. **Any comment is a failure of the code to have sensible naming and clear structure.** Code is its
   own documentation.
2. **A comment may exist only to flag unexpected behaviour** — the genuine surprise a reader could not
   infer from the code.
3. **No comment points at Java, and no comment mentions Java at all.**
4. **No comment references previous work, tasks, or plans.**

**(a) This is refactoring-first, not a deletion sweep.** Where a comment explains something, the first
move is **renaming and restructuring until the comment is unnecessary**, and deletion comes only
after. A sweep that deletes explanatory comments without changing the code destroys knowledge and
leaves the same unreadable code behind. That is why this workstream **pairs with, and probably merges
into, W19** — per crate, one effort: rewrite the internals idiomatically and the comments fall away as
a consequence.

**(b) The provenance layer exits the code entirely — and this is the load-bearing decision.** The
port's provenance system lives in **non-doc comments**, and a gate counts them:

* `// Java bug:` — **≥ 165 sites** across 8 crates; Task 25's check is `grep -rn '// Java bug:' crates`
  ≥ 165 (amendment A11: 153 code + 12 prose).
* `// fixed: T<n> (#id)` — one per site per fixed register row, checked **per row and per site** by
  `crates/copper-core/tests/register.rs` (amendment A17; the per-site version was proved fail-before /
  pass-after at Task 0's fix round, `progress.md:40`).
* `// not ported:`, `// totalized:`, `// renamed:`, `// obligation:` — the roster markers the
  `audit-port.sh` map machinery reads.
* Java `file:line` citations in thousands of doc comments and module docs.

Under the fork **all of it leaves the code**. History's home becomes `docs/java-quirks.md` (closed,
§1.4), the SDD ledgers, the hand-offs, and git history — every one of which keeps the information
without putting it in front of a maintainer reading a function.

**The gate inverts, and that should be the proposal.** Today `register.rs` asserts markers are
**present**. Post-sweep the gate asserts they are **absent**: a test (or a clippy-style lint, or a CI
grep) that fails if `crates/` contains `Java`, `java_`, a `#<digits>` quirk citation, or a `T<n>` task
reference. That keeps a mechanical check on exactly the property we now care about, and it is
cheaper than the one it replaces.

**Sequencing consequence:** the inversion cannot happen while Plan 9's register discipline is live —
Task 25's completion report *depends* on the marker census. **W18 starts after T25 merges, never
before.**

**(c) Doc strings survive, under the same philosophy.** A doc comment says **what a thing is and what
its contract is** — the invariant a caller must uphold, the units, the panic conditions. It does not
say where it came from, which task changed it, or what another program does. Rewriting the doc
comments is the larger half of the work by volume: the port's module docs are dense with Java
`file:line` provenance (`stop.rs`'s budget table and `snapshot.rs`'s field audit are the two biggest).
Each needs to be re-authored as a description of *our* design.

**(d) A written keep-criterion for the "unexpected behaviour" carve-out** — draft, for a reviewer to
apply mechanically:

> **Keep a comment only if:** a maintainer who has the code and the docs, but not the history, would
> otherwise change this line and break something — and the reason cannot be expressed in a name, a
> type, a function boundary, or an assertion.
>
> **Corollaries.** If the surprise can be caught by a test, write the test and delete the comment; the
> test name is the comment. If the surprise is a domain fact (a units convention, a numerical
> tolerance, an external format's requirement), it belongs in the doc comment as part of the
> contract, not in a body comment. If the comment argues *why the obvious alternative is wrong*, try
> the alternative first — if it genuinely breaks, the failing test is the record.

**Tensions to state plainly, not resolve here.**
* Some non-doc comments carry **constraint arguments** the codebase convention explicitly allows
  (things that cannot be shown in code). The criterion above is the filter; expect disagreement at
  the margin, and expect the reviewer to be the tiebreak.
* This is the **largest reversible-looking change with the least mechanical safety net** in the
  roadmap. Nothing compiles differently. The only protection is review and the fact that goldens
  cannot move — which makes "goldens unchanged" a necessary but very weak gate.
* Volume: `grep -rn '// Java bug:' crates` alone is ~165 sites, and the total marker + citation
  surface is several thousand lines. Budget it per crate, alongside W19, and never as one commit.

---

### W19 — Per-crate idiomatic Rust rewrite, interfaces preserved

**Benefit: maintainability (and it unlocks the rest) · Size: XL — likely the largest single item ·
PENDING-on: W18's provenance ruling; T23/T24's shim inventory**

**What it is.** Plan 9's Tasks 23 and 24 do de-Java-ification at the **shim** level: DJ1 replaces
~110 sites where std already has the thing (`java_*` helpers with std equivalents), DJ2 deletes ~150
shims a fix made dead, and the KEEP list (~700 occurrences: `java_double_to_string`, the `java_round`
family, `java_min`/`java_max`, `JavaNumberFormatter`, `java_format_fixed`, `java_double_stream_sum`)
survives because those are **wire contracts and numerical semantics**, not stylistic residue.

**W19 is the level above that:** rewrite each crate's **internals** the way they would have been
written if the program had been designed in Rust — ownership instead of index handles where the
handles exist only because Java had references; iterators and combinators instead of transcribed
loops; `Result` and typed errors instead of sentinel values; enums instead of `i32` tag fields;
`&self` where nothing is written (this is exactly **W4**, which folds in here). **Public interfaces
are preserved** crate by crate, so the blast radius of each landing is one crate.

**The safety net is the whole reason this is possible at all.** Nothing else in this roadmap depends
so completely on Plan 9's output: the port-golden suite (`tests/reference/**`, the converted
drivers), the two-run identity check, the 2400+ test workspace, `structural_hash` decision parity,
and the 29-stem quality A/B with its `cpu_s` gate. A rewrite of this size is defensible **only**
because a behaviour change is mechanically detectable. State that in the plan's acceptance: **every
W19 commit moves zero golden bytes**, and a commit that moves one is either reverted or has become a
different workstream.

**What it absorbs.**
* **W4** — the `&mut Board` read paths and the `set_shove_failing_obstacle` diagnostic (survey §1);
  this is the archetypal W19 change and should not be scheduled separately.
* **W18** — per the pairing above; one effort per crate.
* **The `java_*` naming layer.** Every surviving Java-named identifier gets renamed:
  `java_round` → the rounding rule it implements, `java_double_to_string` → the format it produces,
  any `JavaTreeSet`/`java_*` remnant, and the KEEP shims themselves. **Note the tension:** the KEEP
  list survives T24 *because it is a contract*, and renaming it does not change that — it changes only
  the name. Renaming must therefore be accompanied by a doc comment stating the contract in its own
  terms (a format's requirement, an IEEE semantic), never "matches Java".
* **W16**'s hygiene rows (lawful `Ord`, `pub(crate)` fields, the `determinant` rename).
* **Register rows #42 and #49 — the accessor bounds checks** (`Packages::get`, `LogicalParts::get`,
  `Components::get`/`get_mut`). Plan 9 Task 6 investigated them and left them `pinned` under
  **ruling BX(a)**: the fix is `Padstacks::get`'s bounds check, which in Rust means returning
  `Option` across ~30 call sites in `copper-board`, `copper-dsn` and `copper-router` — a signature change T6's
  brief forbade, for a crash that is unreachable today (`Components` is append-only, and every
  argument is a live 1-based id, a `1..=count()` loop index or already range-checked; the one
  caller with no check of its own is `RoutingBoardExt`'s `fanout_start_pin_name`, whose safety
  rests on that invariant rather than on a guard). Moving a per-caller convention into a
  type-level guarantee is exactly a W19 change, and W19 is the first workstream whose blast radius
  already includes the consuming crates.
* **W9**'s `Line` identity-counter deletion, if T18 leaves it standing.

**Sequencing and shape.**
* **Crate by crate, in dependency order**, landing one at a time: `copper-geometry` → `copper-board` →
  `copper-dsn` / `copper-drc` → `copper-router` → `copper-core` / `copperroute`. The leaf crates are the cheapest
  rehearsal and the router is the one with the least slack (survey §4.3), so it goes last.
* **After** the quality workstreams that change routing behaviour (**W11** phase 3, **W1**), not
  before — a rewrite racing a behaviour change makes both unreviewable. The exception is `copper-geometry`,
  which nothing else in the roadmap touches and which can start immediately.
* Each crate's landing wants its own review pass; this is not a task, it is a programme.

**Open questions.** Does "interfaces preserved" mean the *public* API only, or the crate-to-crate
seams too (the latter is much stricter and much safer)? Is there a per-crate acceptance beyond "zero
golden bytes" — e.g. a clippy pedantic lane turned on crate by crate as each is finished? Should the
rewrite be allowed to *delete* public API that exists only because Java had it (there is a real
candidate set here), or is that a separate, later decision?

---

## 3. Explicit non-goals — considered and declined, with the reason

Sixteen. Each is declined on stated evidence or an explicit user decision, not on taste. Re-opening
one means moving the evidence first.

| # | non-goal | why it is declined | source |
|---|---|---|---|
| **N1** | **Upstream contributions to freerouting (Java) — any PR, patch, issue or report** | **User decision, 2026-09-03.** The project is a fork with no return path (§1). This retires `docs/plan-8-handoff.md` §10's ten-row candidate list, survey §11 question 8's standing list, and the register's "Java-side fix owed" column, all of which become historical text. The wave-1 drafting was already stopped by the user mid-flight (`progress.md:50`); **the salvaged `r1.patch` and `REPORT.md` stay in that session's scratchpad as an artifact and nothing more** — not referenced, not maintained, not moved into the repo | user, 2026-09-03; `progress.md:43`, `:50` |
| **N2** | **Re-establishing byte parity with the jar, or running any retired differential driver against it as a gate** | The fork (§1.1). The `--against-jar` hatch is removed rather than merely unused; a hatch is a live dependency | §1.1; plan § "The harness transition" |
| **N3** | **Reading the Java clone to decide a behaviour question** | §1.3. Merit and measurement decide; the register is history, not evidence about what we should do | §1.3 |
| **N4** | **`rayon`, or any work-stealing pool** | W1's variants are a fixed-size fork/join with no work-stealing, so `rayon` buys nothing; `std::thread::scope` suffices. The Global Constraint refuses it by name | multithreading-survey §6; plan Global Constraints |
| **N5** | **Threading the DRC per-item pair checks** | Provably determinism-safe *and* not worth doing: **measured 0.05 s on the routed DAC board = 0.2 % of a 27 s route.** `#153`'s caching fix is the right lever, and it is sequential | multithreading-survey §0, §5 item 7 |
| **N6** | **Threading the Delaunay / ratsnest per-net build** | "The single cleanest embarrassingly-parallel unit in the codebase" — and it lives entirely inside N5's 0.05 s | multithreading-survey §5 item 8 |
| **N7** | **Threading DSN/SES parse and write** | Measured **0.56–0.58 s** on the three largest fixtures we own, against a 27 s route: under 2 % | multithreading-survey §0, §5 item 9 |
| **N8** | **Threading the maze frontier** (65.7 % of the run) | Best-first search where each expansion pushes what the next reads; **the frontier order *is* the route.** No deterministic scheme short of W1's whole-pass portfolio | multithreading-survey §5 item 1 |
| **N9** | **Threading room completion, shove, pull-tight, the fanout per-pin loop, or the batch pass loop** | Each is a mutation whose result is the state the next step reads; `complete_neighbour_rooms` even restarts its walk because completion adds doors to the room being walked (`autoroute/maze/engine.rs:1063`). The pass loop is sequential by definition | multithreading-survey §5 items 2–6 |
| **N10** | **Threading `BoardStatistics::compute`** (7.6 %) | Its cost is almost entirely N5's work. `#153` **removes** the work rather than spreading it. *"Cache first; never thread what you can delete"* | multithreading-survey §5 item 10 |
| **N11** | **Adopting the forked-from program's multi-threaded router or optimizer design as written** | Its winner selection is a 1000 ms timed join that serialises a board a live daemon thread is still mutating, over `MIN_PRIORITY` daemons, with two disagreeing best-selections and a shared static RNG. W1 takes the *idea* and leaves the implementation | multithreading-survey §2, §5 item 12 |
| **N12** | **Reviving `-mt` or building a threading policy on the dead thread-count fields** | `#143`: `-mt` is dead everywhere; neither field is read on any live path. W1 wants a new, honestly-named setting instead | multithreading-survey §6 row `#143` |
| **N13** | **Parallelising `cargo test`** | Already parallel: no `test-threads` setting exists anywhere; libtest's thread-per-core default is in force. **No work owed** | multithreading-survey §5 item 11 |
| **N14** | **The `Compat::{Java, Fixed}` switch** | Retired by user directive before Plan 9 (BL1); the fork retires the idea permanently. Do not resurrect it to make a Plan 10 change "safe" | plan Global Constraints, BL1 |
| **N15** | **The cosmetic / diagnostic / log-only rows** (`#3`, `#70`, `#97`, `#98`, `#107`, `#108`, `#117`, `#190`, `#199`, `#201`) and the **GUI-only** rows (`#36`, `#59`, `#129`, `#130`, `#138`, `#204`) | No user-visible behaviour changes, or the only callers were GUI classes this program does not have. Verified still un-absorbed: none of them appears in Plan 9's 121 rows. Post-fork most of them stop being *rows* at all — they describe a program we no longer track | imported roadmap §6.2, §6.3; grep over the plan |
| **N16** | **Inheriting Plan 9's review findings** | Task 25 forbids it: a finding that cannot be fixed is *"a recorded, closed decision with its evidence … never a deferral to a Plan 10"* | plan § "Task 25" part 2 |

**Two imported-roadmap non-goals that turned out to be Plan 9 rows** — recorded so nobody re-lists
them as deferred: the "quadratic and repeated work" cluster (`#71`, `#146`, `#153`, `#149`, `#178`,
`#213`, `#105`) is **entirely inside Plan 9**, and §6.5's "open Plan 8 wiring" list was discharged by
Plan 8 itself. The only survivors of §6.1 are `#80` (three dead allocations, mutation-verified
unobservable) and the `i128` candidate, both parked in **W16**.

---

## 4. The finalization checklist

Run at Plan 9 close, in this order. **Do not finalize a PENDING row without its trigger's artefact in
hand.**

**A. The milestone evidence — and the handover of the baseline**

1. `benchmark/reports/plan9-m1-vs-java.{json,md}` — in hand; ruling BV's accept-with-gap and the
   residual bar gap.
2. `benchmark/reports/plan9-m2-vs-java-278fe14.{json,md}` — the `#296` via column per D3 subset and
   the `#231` stem-A/B arm. → **W6**, **W13**.
3. `benchmark/reports/plan9-m3-vs-java-278fe14.{json,md}` — `overall.verdict`, hard losses, D3-small
   vs M1, and the **corpus-median `cpu_s` chain `v1.0.0-rs → M1 → M2 → M3`** with every BO escalation
   and its ruling.
4. **Declare M3 the new baseline of record**, replacing the `java-278fe14` view, and write the
   replacement into Plan 10's own global constraints. Confirm the gate-version in force (BP8) and the
   workbench-cut `benchmark/baselines/stem-times.tsv` (ruling BW) before comparing anything.

**B. The fork's own closing acts** (§1.1, §1.4, §1.5)

5. Enumerate every live reference to the jar: `grep -rn "against-jar\|java-current\|java-278fe14\|
   reference-frozen" crates/ scripts/ benchmark/ tests/`. Everything that is not prose is a **W15.3**
   deletion.
6. Confirm `tests/reference-frozen/` is still read by nothing (BL8's own check) and re-write its
   README in the past tense.
7. Decide the `java-current` bench candidate: **Option A** (competitor column, never a gate) or
   **Option B** (drop). Record the decision and, if A, the mechanical enforcement of "never a gate".
8. Close `docs/java-quirks.md` with a header saying so, and stand up the successor trackers
   (`docs/decisions/`, `docs/defects/`), migrating only the live rows (**W16**, **W2**).

**C. The six T18 verdicts** (plan § "Task 18")

9. For each of `#172`, `#235`, `#104`, `#210`, `#44+#63+#74`: confirmed or **measured and
   declined**, with its number. → **W9**; `#172`'s outcome also feeds **W6**.
10. Did the ordering flip survive? If not, `copper_geometry::Line`'s identity counter survives → a **W19**
    deletion row.
11. Did T18 take the `pass_variants` setting + seeded-PRNG plumbing? → **W1**'s size.
12. The large-board sort policy row's A/B at M3. → **W7**.

**D. The discovery deliverables**

13. `docs/plan-9-prep/stale-index-report.md` — per guard, per stem: fire count, preceding mutation,
    recoverability verdict, **and which of the three options is recommended**. → **W8**.
14. Task 8's report: the `#295` measurement after `#159`, and the `incomplete_count` on
    `docs/plan-9-prep/fixtures/real-world-screen-keys/`. → **W10**, **W12**. If the corridor is still
    open, read T9's and T10's escalation results too.
15. Task 9's `#227` A/B: the optimizer's measured **acceptance rate** and its cpu cost. → **W3**.
16. Task 19's outcome and the id of the `track_width`-gap row filed under ruling BU(c) — plus which
    of the referee's 23 routing types `copper-drc` can express after T19. → **W11**'s size and phase 1
    scope.

**E. The parked / unowned list**

17. Re-read the whole ledger `.superpowers/sdd/2026-09-03-plan-9-post-parity/progress.md` for
    `PARKED`, `QUEUED`, `HELD`, `owes`, `accept-wave`, `final sweep`, and re-judge each. Known at
    draft time and judged **sweep-shaped**: the second `design_name` copy in `parity_ses.rs` (`:44`),
    the 135-col frozen-README line (`:67`), the bracket-matcher dedup and the literal+relationship
    assertions (`:80`), the concern-8 symlink (`:72`), the wave's three out-of-scope doc touches
    (`:91`), `p8t0.rs` (`:86`). Judged **Plan-10-shaped**: `compare.py`'s same-name pooling (`:42`)
    and the fanout budget.
18. `git log main..plan-9-post-parity -- benchmark/bench/compare.py` — if still empty, **W15.1 is
    Plan 10's, and it is now on the main measurement path** (every Plan 10 compare is rs-vs-rs).
19. `grep -rn fanout_ms_per_pin crates/` — if `stop.rs:529-534` still carries `10_000`, **W2 is Plan
    10's**.
20. Did the Plan 9 tail take multithreading-survey shortlist items 2 and 3? Check `quality-ab.sh`,
    `gen-*-reference.sh` and the T23/T24/T25 commit messages. → **W17** stands or is struck.

**F. The provenance and rewrite inventory** (W18, W19)

21. The final marker census from Task 25's check 3: `grep -rn '// Java bug:' crates` (≥ 165), plus
    `// fixed: T<n> (#id)`, `// not ported:`, `// totalized:`, `// renamed:`, `// obligation:`. **This
    census is W18's work inventory** — capture the numbers at T25 and never re-derive them later.
22. `grep -rni 'java' crates/*/src crates/*/tests | wc -l` — the true size of the Java surface in the
    code, doc comments and identifiers included. This is the number W18's inverted gate drives to
    zero.
23. T23/T24's outcome: which shims survive as contracts (the KEEP ~700) and therefore need **renaming
    plus a contract-stated doc comment** under W19, and whether `copper_geometry::Line`'s counter was
    deleted.
24. Confirm the ordering: **W18's gate inversion cannot land until T25 has merged**, because Task
    25's report depends on the marker census being intact.

**G. Re-derive the priorities**

25. Re-read `docs/plan-9-prep/multithreading-survey.md` §7's ranked shortlist against the M3 numbers.
    It was written before `#227` was fixed: **row 4's benefit estimate and row 1's memory estimate
    both assume the pre-`#227` program.**
26. Restate the standing priority in Plan 10's own Global Constraints — **quality first, robustness
    second, speed last** — and confirm the two headline candidates (**W11**, **W1**) are sequenced
    ahead of the XL maintainability programme (**W19**), which is scheduled *around* them per crate.

---

*Draft ends. Nothing here is a task; every row is a candidate with an evidence trail. The finalizer
replaces every PENDING marker with the artefact that answered it, or strikes the row.*
